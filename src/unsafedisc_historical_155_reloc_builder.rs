// Static, read-only reconstruction of the archived unSafeDisc 1.5.5
// PE relocation cutpoint builder. Public compatibility variant only.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155::historical_155_branch_target;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalRelocCutpointAudit {
    pub mode_global_va: u32,
    pub reloc_mode_value: u32,
    pub first_cutpoint_va: u32,
    pub cutpoint_guard_va: u32,
    pub skip_zero_words: bool,
    pub filters_reloc_type: bool,
}

const ARCHIVED_RELOC_WINDOWS: &[(u32,&[u8])] = &[
        // Look for ".rel" in a first-four-byte section-name compare;
        // section-count LOOP (when this fallback path is entered).
        (0x402358,&[
            0x81,0x38,0x2E,0x72,0x65,0x6C,0x74,0x06,
            0x83,0xC0,0x28,0xE2,0xF3,0xC3,
        ]),
        // Enter builder, place 4 before first cutpoint at 0x403CB0,
        // read relocation section file pointer.
        (0x402366,&[
            0x60,0xBF,0xB0,0x3C,0x40,0,
            0xC7,0x47,0xFC,0x04,0,0,0,
            0x8B,0x70,0x14,0x03,0x35,0x0F,0x3B,0x40,0,
            0x89,0x35,0xBC,0xBC,0x60,0,
        ]),
        // relocation block VirtualAddress/SizeOfBlock and page-end
        // cutpoint (VA+0x1FFF)&0x7FFFF000.
        (0x402382,&[
            0x8B,0x1E,0x8B,0x4E,0x04,0x0B,0xDB,
            0x75,0x03,0x61,0xEB,0xD7,
            0x8B,0xC3,0x05,0xFF,0x1F,0,0,
            0x25,0,0xF0,0xFF,0x7F,
            0xE8,0x2B,0,0,0,
        ]),
        // DWORD block-header skip, then 16-bit reloc words.
        // Only a zero *whole WORD* is excluded; no >>12 type test.
        (0x40239F,&[
            0x83,0xC6,0x08,0x83,0xE9,0x08,0xD1,0xE9,
            0x66,0x8B,0x06,0x83,0xC6,0x02,0x66,0x0B,0xC0,
            0x74,0x14,0x25,0xFF,0x0F,0,0,0x03,0xC3,
            0xE8,0x0C,0,0,0,0x83,0xC0,0x04,
            0xE8,0x04,0,0,0,0xE2,0xDF,0xEB,0xB8,
        ]),
        // Sorted insertion using sentinel DWORD=4. Equal values
        // are shifted, not deduplicated.
        (0x4023CA,&[
            0x83,0xC7,0x04,0x57,0x83,0xEF,0x04,
            0x8B,0x57,0xFC,0x3B,0xC2,0x77,0x04,
            0x89,0x17,0xEB,0xF2,0x89,0x07,0x5F,0xC3,
        ]),
];

/// The exact public archival x86 bytes came from the PINNED 18,944-byte
/// compatibility variant (Git blob 825c467b6b403e1c85a5875ede5696dedfe17652).
/// The owner's 20,480-byte original has not been freshly compared.
pub fn audit_155_reloc_cutpoint_builder(
    code:&[u8],image_base:u32,code_rva:u32
)->Result<HistoricalRelocCutpointAudit,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("relocation builder base overflow")?;
    if base!=0x401000 {
        return Err("archived relocation builder CODE base mismatch".into());
    }
    for &(va,bytes) in ARCHIVED_RELOC_WINDOWS {
        let at=va.checked_sub(base)
            .ok_or("archived reloc opcode before CODE")? as usize;
        if code.get(at..at.checked_add(bytes.len())
            .ok_or("archived reloc opcode span overflow")?)!=Some(bytes){
            return Err(format!("archived reloc cutpoint opcode mismatch at 0x{va:08X}"));
        }
    }
    for (from,to) in [
        (0x40235Eu32,0x402366u32),
        (0x402387,0x40238E),
        (0x40238C,0x402365),
        (0x40239A,0x4023CA),
        (0x4023B0,0x4023C6),
        (0x4023B9,0x4023CA),
        (0x4023C1,0x4023CA),
        (0x4023C8,0x402382),
        (0x4023D6,0x4023DC),
        (0x4023DA,0x4023CE),
    ] {
        if historical_155_branch_target(code,base,from)?!=to {
            return Err(format!("archived cutpoint branch mismatch at 0x{from:08X}"));
        }
    }
    // LOOP is not supported by the small generic Jcc decoder.
    for (from,to) in [(0x402363u32,0x402358u32),
                     (0x4023C6,0x4023A7)] {
        let at=(from-base) as usize;
        let word=code.get(at..at+2).ok_or("cutpoint LOOP truncated")?;
        if word[0]!=0xE2 ||
            i64::from(from)+2+i64::from(word[1] as i8)!=i64::from(to){
            return Err("archived cutpoint LOOP target mismatch".into());
        }
    }
    Ok(HistoricalRelocCutpointAudit{
        mode_global_va:0x403CAC,
        reloc_mode_value:4,
        first_cutpoint_va:0x403CB0,
        cutpoint_guard_va:0x403CAC,
        skip_zero_words:true,
        filters_reloc_type:false,
    })
}

/// Synthetic-only cutpoint-generation model. The actual archived x86
/// accepts nonzero relocation words WITHOUT checking the type nibble.
/// Therefore a nonzero type-0 word contributes cutpoints here too.
/// Fail-closed strict bounds and block-size checks are native policy,
/// not behaviors established in historical source.
pub fn hypothetical_155_reloc_cutpoints(
    page_rva:u32,size_of_block:u32,entries:&[u16]
)->Result<Vec<u32>,String>{
    if page_rva==0 || size_of_block<8 ||
       (size_of_block-8)%2!=0 ||
       usize::try_from((size_of_block-8)/2).ok()!=Some(entries.len()){
        return Err("invalid bounded relocation block geometry".into());
    }
    let rounded=page_rva.checked_add(0x1FFF)
        .ok_or("archived rounded page end exceeds u32")? & 0x7FFF_F000;
    if rounded<=4 {
        return Err("historical sorted cutpoint is not above DWORD guard 4".into());
    }
    let mut out=vec![rounded];
    for &word in entries {
        if word==0 {continue;}
        let start=page_rva.checked_add(u32::from(word&0x0FFF))
            .ok_or("relocation start overflow")?;
        let end=start.checked_add(4)
            .ok_or("relocation cutpoint end overflow")?;
        if start<=4 {
            return Err("relocation cutpoint not above insertion guard 4".into());
        }
        out.push(start);
        out.push(end);
    }
    out.sort_unstable(); // duplicates intentionally retained
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_four_static_anchor_and_rel32_calls(){
        let mut code=vec![0u8;0x2000];
        assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
        for &(va,bytes) in ARCHIVED_RELOC_WINDOWS {
            let at=(va-0x401000) as usize;
            code[at..at+bytes.len()].copy_from_slice(bytes);
        }
        let result=audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).unwrap();
        assert_eq!(result.mode_global_va,0x403CAC);
        assert_eq!(result.reloc_mode_value,4);
        assert!(!result.filters_reloc_type);
        for &(va,_) in ARCHIVED_RELOC_WINDOWS {
            let at=(va-0x401000) as usize;
            code[at]^=1;
            assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
            code[at]^=1;
        }
        let at=(0x4023BA-0x401000) as usize;
        code[at]^=1;
        assert!(audit_155_reloc_cutpoint_builder(&code,0x400000,0x1000).is_err());
    }
    #[test]
    fn plain_zero_skipped_but_nonzero_type_zero_is_kept(){
        assert_eq!(hypothetical_155_reloc_cutpoints(
            0x1000,14,&[0,0x3000,0x0001]).unwrap(),
            vec![0x1000,0x1001,0x1004,0x1005,0x2000]);
    }
    #[test]
    fn duplicate_cutpoints_are_not_deduplicated(){
        assert_eq!(hypothetical_155_reloc_cutpoints(
            0x1000,12,&[0x3000,0x3000]).unwrap(),
            vec![0x1000,0x1000,0x1004,0x1004,0x2000]);
    }
    #[test]
    fn invalid_block_length_or_overflow_fails_closed(){
        assert!(hypothetical_155_reloc_cutpoints(0x1000,7,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0x1000,9,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0x1000,10,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(0,8,&[]).is_err());
        assert!(hypothetical_155_reloc_cutpoints(u32::MAX,8,&[]).is_err());
    }
}
