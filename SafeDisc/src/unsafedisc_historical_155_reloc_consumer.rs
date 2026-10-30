// Read-only public-archive x86 relocation consumer and span-clamp
// research. No SafeDisc DLL execution and no game transformations.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155::historical_155_branch_target;

/// Source: exact SHA-pinned PUBLIC compatibility variant, 18,944 bytes,
/// Git blob 825c467b6b403e1c85a5875ede5696dedfe17652.
/// These opcode windows also passed the independent exact SHA-256-gated
/// original 20,480-byte tool audit in GitHub Actions on 2026-10-28.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalRelocConsumerAudit {
    pub paired_word_mask: u32,
    pub section_end_va: u32,
    pub clip_return_va: u32,
    pub normal_call_helper_va: u32,
}

pub fn audit_155_reloc_consumer(
    code:&[u8],image_base:u32,code_rva:u32,
)->Result<HistoricalRelocConsumerAudit,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("archived relocation consumer code address overflow")?;
    if base!=0x401000 {
        return Err("archived relocation consumer CODE base mismatch".into());
    }
    let exact_windows:&[(u32,&[u8])]=&[
        // Two packed WORD relocation entries at a time. Both
        // high type nibbles are stripped before writing the scratch
        // list of offsets at [0x60BCD8+EBX].
        (0x401DFB,&[
            0x8B,0x44,0x1D,0x00,0x25,0xFF,0x0F,0xFF,0x0F,
            0x89,0x04,0x1E,0x83,0xC3,0x04,0x3B,0xD9,0x72,0xED,
        ]),
        // Initial EAX=0 special case: do not call gap helper before
        // a relocation at offset zero. The original tests the low
        // 12-bit masked value, NOT its discarded type nibble.
        (0x401E0F,&[
            0xD1,0xE9,0x0F,0xB7,0x06,0x85,0xC0,0x74,0x10,
            0x2B,0xC5,0x8B,0xDF,0x03,0xDD,0x03,0xDA,
            0xE8,0xD0,0x00,0x00,0x00,0x0F,0xB7,0x06,
        ]),
        // Non-leading zero WORD triggers JE to the block-tail
        // handler at 401E52. In particular, zeros before later
        // nonzero entries can truncate the historical scan.
        (0x401E31,&[
            0x66,0x83,0x3E,0x00,0x74,0x1B,0x66,0x3B,0x06,
            0x74,0xEC,0x8B,0xE8,0x66,0x8B,0x06,0x2B,0xC5,
            0x8B,0xDF,0x03,0xDD,0x03,0xDA,
            0xE8,0xA7,0x00,0x00,0x00,0x03,0xC5,
            0xEB,0xD6,0x83,0xC6,0x02,
        ]),
        // Advance past a four-byte relocation and compare the next
        // WORD with the previous relocation end. Equality bypasses
        // the inter-relocation gap helper.
        (0x401E28,&[
            0x83,0xC0,0x04,0x83,0xC6,0x02,0x49,0x74,0x24,
            0x66,0x83,0x3E,0x00,0x74,0x1B,
            0x66,0x3B,0x06,0x74,0xEC,
        ]),
        // Overflow into section end rewrites the current helper
        // return address to 0x401D79 and jumps to 0x401F01.
        (0x401EE3,&[
            0x2B,0xD8,0xA1,0xC0,0xBC,0x60,0x00,
            0x2B,0xC3,0xC7,0x04,0x24,0x79,0x1D,0x40,0x00,
            0xEB,0x0C,
        ]),
        // EBX += EAX; if unsigned EBX > section end then
        // clip; otherwise restore original EBX and fall through.
        (0x401EF5,&[
            0x03,0xD8,0x3B,0x1D,0xC0,0xBC,0x60,0x00,
            0x77,0xE4,0x2B,0xD8,
        ]),
    ];
    for &(va,bytes) in exact_windows{
        let start=va.checked_sub(base)
            .ok_or("archived consumer x86 window before CODE")? as usize;
        let end=start.checked_add(bytes.len())
            .ok_or("archived consumer x86 window overflow")?;
        if code.get(start..end)!=Some(bytes){
            return Err(format!(
                "archived relocation consumer opcode differs at 0x{va:08X}"
            ));
        }
    }
    for (from,to) in [
        (0x401E0Cu32,0x401DFBu32),
        (0x401E16,0x401E28),
        (0x401E20,0x401EF5),
        (0x401E49,0x401EF5),
        (0x401E50,0x401E28),
        (0x401E2F,0x401E55),
        (0x401E35,0x401E52),
        (0x401E3A,0x401E28),
        (0x401EFD,0x401EE3),
        (0x401EF3,0x401F01),
    ]{
        if historical_155_branch_target(code,base,from)?!=to{
            return Err(format!(
                "archived relocation consumer branch mismatch at 0x{from:08X}"
            ));
        }
    }
    Ok(HistoricalRelocConsumerAudit{
        paired_word_mask:0x0FFF_0FFF,
        section_end_va:0x60BCC0,
        clip_return_va:0x401D79,
        normal_call_helper_va:0x401F01,
    })
}

/// A bounded native preview of the archived two-WORD copying loop
/// at 0x401DFB. The historical code fetches a *DWORD* and advances
/// by four, so a nonempty odd count could fetch past the supplied
/// relocation list. This native model fails closed instead.
/// Type nibbles are discarded for every WORD, even a nonzero
/// IMAGE_REL_BASED_ABSOLUTE padding entry. Actual game inputs are
/// NEVER processed by this synthetic-only inspector.
pub fn hypothetical_archived_reloc_offset_words(
    words:&[u16],
)->Result<Vec<u16>,String>{
    if words.len()%2!=0 {
        return Err("historical DWORD copy would access beyond an odd relocation-word count".into());
    }
    let mut out=Vec::with_capacity(words.len());
    for pair in words.chunks_exact(2){
        let packed=u32::from(pair[0])|(u32::from(pair[1])<<16);
        let filtered=packed&0x0FFF_0FFF;
        out.push(filtered as u16);
        out.push((filtered>>16) as u16);
    }
    Ok(out)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalSafeSpan {
    pub address:u32,
    pub length:u32,
    /// Historical 401EEc rewrites the caller's stack return address
    /// to 0x401D79 when an otherwise valid span is clipped.
    pub would_leave_section_iterator:bool,
}

/// A bounded model of the 401EF5 span helper. Rejects malformed
/// pointer arithmetic; does not read bytes or execute code.
/// If start+requested_length > section_end, the historical x86
/// clips the span and writes a nonlocal return address at [ESP].
pub fn checked_archived_reloc_span(
    start:u32, requested_length:u32, section_end:u32,
)->Result<HistoricalSafeSpan,String>{
    if start>section_end{
        return Err("fragment starts beyond selected section end".into());
    }
    let requested_end=start.checked_add(requested_length)
        .ok_or("fragment end overflows 32-bit address space")?;
    let clipped=requested_end>section_end;
    Ok(HistoricalSafeSpan{
        address:start,
        length:if clipped {section_end-start} else {requested_length},
        would_leave_section_iterator:clipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archived_fixture()->Vec<u8>{
        let mut bytes=vec![0u8;0x2000];
        for (va,window) in [
            (0x401DFBu32,&[
                0x8B,0x44,0x1D,0x00,0x25,0xFF,0x0F,0xFF,0x0F,
                0x89,0x04,0x1E,0x83,0xC3,0x04,0x3B,0xD9,0x72,0xED,
            ][..]),
            (0x401E0F,&[
                0xD1,0xE9,0x0F,0xB7,0x06,0x85,0xC0,0x74,0x10,
                0x2B,0xC5,0x8B,0xDF,0x03,0xDD,0x03,0xDA,
                0xE8,0xD0,0x00,0x00,0x00,0x0F,0xB7,0x06,
            ][..]),
            (0x401E31,&[
                0x66,0x83,0x3E,0x00,0x74,0x1B,0x66,0x3B,0x06,
                0x74,0xEC,0x8B,0xE8,0x66,0x8B,0x06,0x2B,0xC5,
                0x8B,0xDF,0x03,0xDD,0x03,0xDA,
                0xE8,0xA7,0x00,0x00,0x00,0x03,0xC5,
                0xEB,0xD6,0x83,0xC6,0x02,
            ][..]),
            (0x401E28,&[
                0x83,0xC0,0x04,0x83,0xC6,0x02,0x49,0x74,0x24,
                0x66,0x83,0x3E,0x00,0x74,0x1B,
                0x66,0x3B,0x06,0x74,0xEC,
            ][..]),
            (0x401EE3,&[
                0x2B,0xD8,0xA1,0xC0,0xBC,0x60,0x00,
                0x2B,0xC3,0xC7,0x04,0x24,0x79,0x1D,0x40,0x00,
                0xEB,0x0C,
            ][..]),
            (0x401EF5,&[
                0x03,0xD8,0x3B,0x1D,0xC0,0xBC,0x60,0x00,
                0x77,0xE4,0x2B,0xD8,
            ][..]),
        ]{
            let i=(va-0x401000) as usize;
            bytes[i..i+window.len()].copy_from_slice(window);
        }
        bytes
    }
    #[test]
    fn archived_exact_windows_and_branch_targets() {
        let mut code=archived_fixture();
        let a=audit_155_reloc_consumer(&code,0x400000,0x1000).unwrap();
        assert_eq!(a.paired_word_mask,0x0FFF_0FFF);
        assert_eq!(a.section_end_va,0x60BCC0);
        assert_eq!(a.clip_return_va,0x401D79);
        for va in [0x401DFB,0x401DFF,0x401E0F,0x401E14,0x401E16,
                   0x401E28,0x401E31,0x401E35,0x401E49,
                   0x401EE3,0x401EEC,0x401EF5]{
            let at=(va-0x401000) as usize;
            code[at]^=1;
            assert!(audit_155_reloc_consumer(&code,0x400000,0x1000).is_err());
            code[at]^=1;
        }
        assert!(audit_155_reloc_consumer(&code[..0x100],0x400000,0x1000).is_err());
        assert!(audit_155_reloc_consumer(&code,0x400001,0x1000).is_err());
    }
    #[test]
    fn packed_words_strip_both_types_without_changing_offsets(){
        assert_eq!(hypothetical_archived_reloc_offset_words(
            &[0x3001,0x0000,0x0007,0xAFFF]).unwrap(),
            vec![1,0,7,0xFFF]);
        assert_eq!(hypothetical_archived_reloc_offset_words(&[]).unwrap(),vec![]);
        assert!(hypothetical_archived_reloc_offset_words(&[0x3001]).is_err());
    }
    #[test]
    fn safe_span_exact_boundary_and_clipping(){
        assert_eq!(checked_archived_reloc_span(0x1000,0x100,0x1100).unwrap(),
            HistoricalSafeSpan{address:0x1000,length:0x100,
                would_leave_section_iterator:false});
        assert_eq!(checked_archived_reloc_span(0x1000,0x200,0x1100).unwrap(),
            HistoricalSafeSpan{address:0x1000,length:0x100,
                would_leave_section_iterator:true});
        assert_eq!(checked_archived_reloc_span(0x1100,10,0x1100).unwrap(),
            HistoricalSafeSpan{address:0x1100,length:0,
                would_leave_section_iterator:true});
    }
    #[test]
    fn unsafe_arithmetic_fails_closed(){
        assert!(checked_archived_reloc_span(0x1101,1,0x1100).is_err());
        assert!(checked_archived_reloc_span(u32::MAX-1,4,u32::MAX).is_err());
        assert!(checked_archived_reloc_span(0x1000,u32::MAX,u32::MAX).is_err());
    }
}
