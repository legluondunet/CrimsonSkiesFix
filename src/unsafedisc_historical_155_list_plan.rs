// Read-only, synthetic-only bounds model for unSafeDisc 1.5.5's
// historical PE section-header pointer-list builder and iterator.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155::historical_155_selected_section_indices;

/// At 0x4022B0 the instruction is LOOP rel8, not a zero-checked
/// for-loop. Entry into that loop with ECX=0 executes 2^32 bodies,
/// because the first decrement wraps to 0xFFFF_FFFF. This function
/// models only the loop count, not the intervening memory operations.
pub fn historical_155_loop_iterations(initial_ecx: u32) -> u64 {
    if initial_ecx == 0 {
        1u64 << 32
    } else {
        u64::from(initial_ecx)
    }
}

/// A SAFE synthetic preview, never a Windows process-pointer writer.
/// The historical builder at 0x4022A8 stores each selected section
/// header's 32-bit address and advances EDI by four. The AUDITED LOOP
/// does not write a final NULL; subsequent code is not excluded.
/// The historical iterator at 0x401D06 reads pointers
/// until it encounters zero. A previously zero-filled next slot is
/// therefore a precondition for the isolated loop-only model, not
/// necessarily for the complete historical program. Later writes,
/// runtime initialization and intervening changes remain unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPointerListPreview {
    pub selected_indices: Vec<usize>,
    pub selected_header_vas: Vec<u32>,
    pub resulting_pointer_words: Vec<u32>,
    pub terminating_slot: usize,
}

/// Bounded native-port-only model for invented or caller-owned
/// section records and an explicit snapshot of destination slots.
/// It must NOT silently invent a NULL terminator absent from the
/// audited loop. This does not assert the rest of the function lacks one.
/// Successful validation is not proof of real runtime conditions.
pub fn hypothetical_155_pointer_list_preview(
    section_headers: &[u8],
    section_count: usize,
    section_table_va: u32,
    initial_destination_words: &[u32],
) -> Result<HistoricalPointerListPreview, String> {
    if section_count == 0 {
        return Err("historical x86 LOOP with section count zero wraps for 2^32 iterations".into());
    }
    let selected_indices =
        historical_155_selected_section_indices(section_headers, section_count)?;
    if selected_indices.len() >= initial_destination_words.len() {
        return Err("historical pointer list lacks a complete terminator DWORD slot".into());
    }
    let terminating_slot = selected_indices.len();
    if initial_destination_words[terminating_slot] != 0 {
        return Err("audited historical list loop does not overwrite a stale nonzero terminator".into());
    }
    let mut result = initial_destination_words.to_vec();
    let mut selected_header_vas = Vec::with_capacity(selected_indices.len());
    for (slot, &index) in selected_indices.iter().enumerate() {
        let offset = u32::try_from(index)
            .map_err(|_| "section header index exceeds 32-bit width")?
            .checked_mul(40)
            .ok_or("historical section header address multiplication overflow")?;
        let header_va = section_table_va.checked_add(offset)
            .ok_or("historical 32-bit section header pointer overflow")?;
        if header_va == 0 {
            return Err("historical null section header pointer would end iterator prematurely".into());
        }
        result[slot] = header_va;
        selected_header_vas.push(header_va);
    }
    Ok(HistoricalPointerListPreview {
        selected_indices,
        selected_header_vas,
        resulting_pointer_words: result,
        terminating_slot,
    })
}

/// How many selected entries fit within an unchanged initial run
/// of zero DWORDs while retaining one zero slot for iteration.
/// On-disk snapshot only; it does not establish a runtime capacity.
pub fn hypothetical_zero_terminated_capacity(leading_zero_words: usize) -> usize {
    leading_zero_words.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(names: &[[u8; 8]]) -> Vec<u8> {
        let mut records = vec![0u8; names.len() * 40];
        for (i, name) in names.iter().enumerate() {
            records[i*40..i*40+8].copy_from_slice(name);
        }
        records
    }

    #[test]
    fn zero_count_is_not_a_zero_iteration_loop() {
        assert_eq!(historical_155_loop_iterations(0), 4_294_967_296);
        assert_eq!(historical_155_loop_iterations(1), 1);
        assert_eq!(historical_155_loop_iterations(96), 96);
        assert_eq!(historical_155_loop_iterations(u32::MAX), 4_294_967_295);
        assert!(hypothetical_155_pointer_list_preview(&[], 0, 0x401000, &[0]).is_err());
    }

    #[test]
    fn selected_pointers_keep_header_order_and_existing_zero_sentinel() {
        let records = headers(&[
            *b".text\0\0\0",
            *b".rsrc\0\0\0",
            *b".data\0\0\0",
            *b".idata\0\0",
            *b".rdata\0\0",
        ]);
        let preview =
            hypothetical_155_pointer_list_preview(&records, 5, 0x501000, &[0, 0, 0, 0])
            .unwrap();
        assert_eq!(preview.selected_indices, vec![0, 2]);
        assert_eq!(preview.selected_header_vas, vec![0x501000, 0x501050]);
        assert_eq!(preview.resulting_pointer_words, vec![0x501000, 0x501050, 0, 0]);
        assert_eq!(preview.terminating_slot, 2);
    }

    #[test]
    fn no_selected_sections_still_require_zero_first_slot() {
        let records = headers(&[*b".reloc\0\0", *b".rsrc\0\0\0"]);
        let preview =
            hypothetical_155_pointer_list_preview(&records, 2, 0x501000, &[0, 0])
            .unwrap();
        assert!(preview.selected_indices.is_empty());
        assert_eq!(preview.terminating_slot, 0);
        assert_eq!(preview.resulting_pointer_words, vec![0, 0]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0xDEAD_BEEF, 0]
        ).is_err());
    }

    #[test]
    fn stale_sentinel_or_missing_capacity_fails_closed() {
        let records = headers(&[*b".text\0\0\0", *b".data\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0, 0]
        ).is_err());
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0, 0, 0x1234]
        ).is_err());
        assert!(hypothetical_155_pointer_list_preview(
            &records[..39], 2, 0x501000, &[0, 0, 0]
        ).is_err());
        assert_eq!(hypothetical_zero_terminated_capacity(0), 0);
        assert_eq!(hypothetical_zero_terminated_capacity(1), 0);
        assert_eq!(hypothetical_zero_terminated_capacity(28), 27);
    }

    #[test]
    fn checked_32bit_pointers_refuse_overflow_or_null() {
        let records = headers(&[*b".rdata\0\0", *b".text\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, u32::MAX - 39, &[0, 0]
        ).is_err());
        let first = headers(&[*b".text\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &first, 1, 0, &[0, 0]
        ).is_err());
        let preview = hypothetical_155_pointer_list_preview(
            &first, 1, u32::MAX, &[0, 0]
        ).unwrap();
        assert_eq!(preview.selected_header_vas, vec![u32::MAX]);
    }
}


// Public archive Git blob 825c467b6b403e1c85a5875ede5696dedfe17652:
// instruction-aligned original bytes, observed from pinned CI objdump.
// This is an audit of the *public modified* 18,944-byte program only.
const ARCHIVED_155_POST_BUILDER: &[u8] = &[
    0xA1,0x17,0x3B,0x40,0,0x8B,0x15,0x43,0x3B,0x40,0,
    0x8B,0x58,0x0C,0x8B,0x48,0x10,0x03,0xCB,0x3B,0xD1,
    0x76,0x05,0x83,0xC0,0x28,0xEB,0xEF,0x3B,0xD3,0x73,
    0x05,0x83,0xC0,0x28,0xEB,0xE6,0x39,0x50,0x34,0x75,
    0x05,0x83,0xC0,0x28,0xEB,0xDC,
    0x2B,0x58,0x14,0x89,0x1D,0x3F,0x3B,0x40,0,
    0x29,0x1D,0x43,0x3B,0x40,0,0x8B,0x50,0x0C,
    0x8B,0x58,0x10,0x8B,0x48,0x14,
    0x89,0x15,0x53,0x3B,0x40,0,
    0x89,0x1D,0x4B,0x3B,0x40,0,
    0x89,0x0D,0x4F,0x3B,0x40,0,
];

/// Audit the archived PE RVA -> raw address mapper starting at
/// historical 0x4022B2, after the original 40-byte section-name
/// filter. No executable is ever run or mapped as program code.
/// Independently SHA-verified against the owner's exact original 20,480-byte
/// tool in a read-only GitHub Actions audit on 2026-10-28.
pub fn audit_155_post_builder_rva_mapper(
    code: &[u8], image_base: u32, code_rva: u32,
) -> Result<(), String> {
    use crate::unsafedisc_historical_155::historical_155_branch_target;
    let base=image_base.checked_add(code_rva)
        .ok_or("historical RVA mapper CODE address overflow")?;
    if base!=0x401000 {
        return Err("historical RVA mapper CODE base mismatch".into());
    }
    let at=(0x4022B2u32-base) as usize;
    if code.get(at..at+ARCHIVED_155_POST_BUILDER.len())
        !=Some(ARCHIVED_155_POST_BUILDER) {
        return Err("historical post-builder RVA mapper opcode mismatch".into());
    }
    for (source,target) in [
        (0x4022C7u32,0x4022CEu32),(0x4022CC,0x4022BD),
        (0x4022D0,0x4022D7),(0x4022D5,0x4022BD),
        (0x4022DA,0x4022E1),(0x4022DF,0x4022BD),
    ] {
        let actual=historical_155_branch_target(code,base,source)?;
        if actual!=target {
            return Err(format!("historical mapper branch at 0x{source:08X} -> 0x{actual:08X}"));
        }
    }
    Ok(())
}

/// The x86 branch compares target against VirtualAddress + RawSize
/// INCLUSIVELY and tests the next header's VirtualAddress using
/// [EAX+0x34] (40+12). The original retry loop has no local count
/// bound. This synthetic-only model refuses to read past N headers.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalRvaMapCandidate {
    pub section_index: usize,
    pub raw_offset: u32,
    pub equals_exclusive_raw_end: bool,
}

pub fn hypothetical_155_rva_map_candidate(
    headers: &[u8], count: usize, target: u32,
) -> Result<Option<HistoricalRvaMapCandidate>,String> {
    if count==0 || count>96 ||
        headers.len()!=count.checked_mul(40)
            .ok_or("historical section count overflow")? {
        return Err("expected 1..96 complete 40-byte section headers".into());
    }
    let read=|record:&[u8],at:usize|->u32{
        u32::from_le_bytes(record[at..at+4].try_into().unwrap())
    };
    for i in 0..count {
        let r=&headers[i*40..(i+1)*40];
        let base=read(r,0x0C);
        let raw_size=read(r,0x10);
        let end=base.checked_add(raw_size)
            .ok_or("section raw RVA extent overflow")?;
        if target<base || target>end {continue;}
        // Historical [EAX+0x34] reads the next section's +0x0C.
        // Do not synthesize data beyond the last known header.
        let next=headers.get((i+1)*40+0x0C..(i+1)*40+0x10)
            .ok_or("last candidate requires unavailable next-section RVA")?;
        let next_rva=u32::from_le_bytes(next.try_into().unwrap());
        if next_rva==target {continue;}
        let raw_pointer=read(r,0x14);
        let raw_offset=raw_pointer.checked_add(target-base)
            .ok_or("section RVA to raw file offset overflow")?;
        return Ok(Some(HistoricalRvaMapCandidate{
            section_index:i,raw_offset,
            equals_exclusive_raw_end:target==end,
        }));
    }
    Ok(None)
}

/// Distinct safe native policy: refuse the historical one-past-end
/// candidate and every position outside the known file.
pub fn checked_155_rva_to_file(
    headers:&[u8],count:usize,target:u32,file_len:usize,
)->Result<Option<usize>,String>{
    let candidate=hypothetical_155_rva_map_candidate(headers,count,target)?;
    match candidate {
        None=>Ok(None),
        Some(m)=>{
            if m.equals_exclusive_raw_end {
                return Err("inclusive historical raw end is not backed byte data".into());
            }
            let off=m.raw_offset as usize;
            if off>=file_len {
                return Err("mapped section byte lies beyond file".into());
            }
            Ok(Some(off))
        }
    }
}

#[cfg(test)]
mod post_builder_rva_tests {
    use super::*;
    fn fixture()->Vec<u8>{
        let mut h=vec![0;40*3];
        for (i,va,raw,ptr) in [
            (0usize,0x1000u32,0x100u32,0x200u32),
            (1,0x1100,0x80,0x400),
            (2,0x2000,0x20,0x600),
        ] {
            let r=&mut h[i*40..(i+1)*40];
            r[0x0C..0x10].copy_from_slice(&va.to_le_bytes());
            r[0x10..0x14].copy_from_slice(&raw.to_le_bytes());
            r[0x14..0x18].copy_from_slice(&ptr.to_le_bytes());
        }
        h
    }
    #[test]
    fn archived_mapper_opcodes_and_branches_are_anchored() {
        let mut code=vec![0u8;0x2000];
        let at=(0x4022B2-0x401000) as usize;
        code[at..at+ARCHIVED_155_POST_BUILDER.len()]
            .copy_from_slice(ARCHIVED_155_POST_BUILDER);
        audit_155_post_builder_rva_mapper(&code,0x400000,0x1000).unwrap();
        for k in [0usize,10,16,21,27,40,47,65,80] {
            code[at+k]^=1;
            assert!(audit_155_post_builder_rva_mapper(&code,0x400000,0x1000).is_err());
            code[at+k]^=1;
        }
        assert!(audit_155_post_builder_rva_mapper(&code[..40],0x400000,0x1000).is_err());
    }
    #[test]
    fn adjacent_section_start_overrides_inclusive_old_end(){
        let h=fixture();
        assert_eq!(checked_155_rva_to_file(&h,3,0x1000,0x1000).unwrap(),Some(0x200));
        assert_eq!(checked_155_rva_to_file(&h,3,0x10FF,0x1000).unwrap(),Some(0x2FF));
        assert_eq!(checked_155_rva_to_file(&h,3,0x1100,0x1000).unwrap(),Some(0x400));
        assert_eq!(hypothetical_155_rva_map_candidate(&h,3,0x1180).unwrap(),
            Some(HistoricalRvaMapCandidate{
                section_index:1,raw_offset:0x480,equals_exclusive_raw_end:true}));
        assert!(checked_155_rva_to_file(&h,3,0x1180,0x1000).is_err());
    }
    #[test]
    fn native_boundaries_refuse_invalid_targets(){
        let h=fixture();
        assert_eq!(checked_155_rva_to_file(&h,3,0x1181,0x1000).unwrap(),None);
        assert_eq!(checked_155_rva_to_file(&h,3,0x0FFF,0x1000).unwrap(),None);
        assert!(checked_155_rva_to_file(&h,3,0x1101,0x401).is_err());
        assert!(checked_155_rva_to_file(&h,3,0x2000,0x1000).is_err());
        assert!(checked_155_rva_to_file(&[],0,0x1000,0x1000).is_err());
        assert!(checked_155_rva_to_file(&h[..79],2,0x1000,0x1000).is_err());
    }
    #[test]
    fn overflow_checks_reject_unrepresentable_section_extents(){
        let mut h=fixture();
        h[0x0C..0x10].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        h[0x10..0x14].copy_from_slice(&0x40u32.to_le_bytes());
        assert!(checked_155_rva_to_file(&h,3,0xFFFF_FFF1,0x1000).is_err());
        let mut h=fixture();
        h[0x14..0x18].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(checked_155_rva_to_file(&h,3,0x1001,0x1000).is_err());
    }
}
