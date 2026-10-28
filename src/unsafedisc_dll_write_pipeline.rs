// Static provenance of the original .txt2 dual-read / local-processing /
// single-write chain. Never execute the proprietary DLL or write its memory.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Pe32, Result, dll_expect_raw_x86, dll_relative_call_target};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct DescriptorIoCall {
    pub call_rva:u32,
    pub backend_rva:u32,
    /// All operations pass the address of the caller's 12-byte descriptor
    /// beginning at [EBP+8], not the raw module base as an argument.
    pub descriptor_ebp_offset:i32,
    pub offset_ebp_offset:i32,
    pub length_ebp_offset:i32,
    pub buffer_ebp_offset:i32,
}

pub const READ_CURRENT:DescriptorIoCall=DescriptorIoCall {
    call_rva:0x19327,backend_rva:0xA810,
    descriptor_ebp_offset:8,offset_ebp_offset:-0x1008,
    length_ebp_offset:-0x1014,buffer_ebp_offset:-0x1004,
};
pub const READ_OTHER:DescriptorIoCall=DescriptorIoCall {
    call_rva:0x19348,backend_rva:0xA810,
    descriptor_ebp_offset:8,offset_ebp_offset:-0x1010,
    length_ebp_offset:-0x1018,buffer_ebp_offset:-0x2018,
};
pub const WRITE_CURRENT:DescriptorIoCall=DescriptorIoCall {
    call_rva:0x19448,backend_rva:0xAA00,
    descriptor_ebp_offset:8,offset_ebp_offset:-0x1008,
    length_ebp_offset:-0x1014,buffer_ebp_offset:-0x1004,
};

/// The read and write use the SAME descriptor, offset, size, and local
/// buffer slots. This does not prove the opaque paths reach either call.
pub fn same_range_and_buffer(a:&DescriptorIoCall,b:&DescriptorIoCall)->bool {
    a.descriptor_ebp_offset==b.descriptor_ebp_offset &&
    a.offset_ebp_offset==b.offset_ebp_offset &&
    a.length_ebp_offset==b.length_ebp_offset &&
    a.buffer_ebp_offset==b.buffer_ebp_offset
}

/// The helper called between the reads and write takes the same
/// descriptor and both local buffer/range contexts. Its internal
/// algorithm and actual side effects are still unresolved.
pub const TRANSFORM_RVA:u32=0x195DF;
pub const TRANSFORM_CALL_RVA:u32=0x1940B;
pub const CODE_REGION_CATEGORY:u32=4;
pub const CATEGORY_CHECK_CALL_RVA:u32=0x196AE;
pub const CATEGORY_CHECK_RVA:u32=0xA310;

/// A310 adjusts a nonzero 4-KiB-page-aligned start by one before
/// comparing it to a record's page-boundary markers. This is only
/// a local arithmetic operation, not a model of the entire A310.
pub fn adjusted_range_start(start:u32)->u32 {
    if start!=0 && start&0xFFF==0 {start-1} else {start}
}

pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<usize> {
    for (site,dest) in [
        (READ_CURRENT.call_rva,READ_CURRENT.backend_rva),
        (READ_OTHER.call_rva,READ_OTHER.backend_rva),
        (TRANSFORM_CALL_RVA,TRANSFORM_RVA),
        (WRITE_CURRENT.call_rva,WRITE_CURRENT.backend_rva),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=dest {
            return Err(format!("original .txt2 read-transform-write CALL mismatch at RVA 0x{site:05X}"));
        }
    }
    // 195DF is not just an unclassified byte mixer: it calls A310
    // with literal category 4 (the original code-section category).
    // The callee looks up a module record and compares the request
    // with its recorded page limits at +0x34/+0x38, then invokes
    // additional original range-management helpers. The actual
    // effect and return values of those helpers remain unproven.
    for (site,dest) in [
        (CATEGORY_CHECK_CALL_RVA,CATEGORY_CHECK_RVA),
        (0xA33Eu32,0xB5D0u32),
        (0xA394,0xC0D0),
        (0xA3A2,0xC0C0),
        (0xA3C8,0xBFB0),
        (0xA3F2,0xBED0),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=dest {
            return Err(format!("original 195DF/A310 code-range helper CALL mismatch at 0x{site:05X}"));
        }
    }
    for (site,bytes) in [
        (0x19692u32,&[0x8B,0x45,0x24][..]),
        (0x19695,&[0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x196A0,&[0x6A,0x04][..]),
        (0x196A2,&[0x8D,0x4D,0xEC,0x51][..]),
        (0x196A6,&[0x8D,0x55,0xF4,0x52][..]),
        (0x196AA,&[0x8D,0x45,0x08,0x50][..]),
        (0x196B3,&[0x83,0xC4,0x14][..]),
        (0x196B6,&[0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x196BB,&[0x85,0xC0][..]),
        (0xA314,&[0x8B,0x7C,0x24,0x18][..]),
        (0xA318,&[0x85,0xFF][..]),
        (0xA320,&[0x8B,0x6C,0x24,0x1C][..]),
        (0xA324,&[0x85,0xED][..]),
        (0xA32C,&[0x8B,0x07,0x8B,0x4D,0x00,0x3B,0xC1][..]),
        (0xA355,&[0x85,0xFF,0x74,0x09][..]),
        (0xA359,&[0xF7,0xC7,0xFF,0x0F,0x00,0x00][..]),
        (0xA361,&[0x4F][..]),
        (0xA362,&[0x8B,0x46,0x34][..]),
        (0xA369,&[0x3B,0x5E,0x38][..]),
        (0xA3DD,&[0x89,0x7E,0x34][..]),
        (0xA3E0,&[0x89,0x5E,0x38][..]),
        (0xA3ED,&[0x83,0xC6,0x14][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    // Recover both reads' four push operands from the actually reached
    // x86 instruction boundaries. All lengths and offsets are locals:
    // therefore the equality proved here concerns STACK SLOTS,
    // not necessarily valid computed runtime addresses or byte counts.
    for (site,bytes) in [
        (0x1930Eu32,&[0x8D,0x8D,0xFC,0xEF,0xFF,0xFF,0x51][..]),
        (0x19315,&[0x8B,0x95,0xEC,0xEF,0xFF,0xFF,0x52][..]),
        (0x1931C,&[0x8B,0x85,0xF8,0xEF,0xFF,0xFF,0x50][..]),
        (0x19323,&[0x8D,0x4D,0x08,0x51][..]),
        (0x1932C,&[0x83,0xC4,0x10][..]),
        (0x1932F,&[0x8D,0x95,0xE8,0xDF,0xFF,0xFF,0x52][..]),
        (0x19336,&[0x8B,0x85,0xE8,0xEF,0xFF,0xFF,0x50][..]),
        (0x1933D,&[0x8B,0x8D,0xF0,0xEF,0xFF,0xFF,0x51][..]),
        (0x19344,&[0x8D,0x55,0x08,0x52][..]),
        (0x1934D,&[0x83,0xC4,0x10][..]),
        // The processing helper is given FIVE logical arguments:
        // pointer to the caller's 12-byte descriptor (copied onto
        // its stack), local work-buffer pointer, offsets/count,
        // and the second input buffer pointer.
        (0x193D9,&[0x8D,0x8D,0xE8,0xDF,0xFF,0xFF,0x51][..]),
        (0x193E0,&[0x8B,0x95,0xF8,0xEF,0xFF,0xFF,0x52][..]),
        (0x193E7,&[0x8B,0x85,0xEC,0xEF,0xFF,0xFF,0x50][..]),
        (0x193EE,&[0x8D,0x8D,0xFC,0xEF,0xFF,0xFF,0x51][..]),
        (0x193F5,&[0x83,0xEC,0x0C][..]),
        (0x193F8,&[0x8B,0xD4,0x8B,0x45,0x08,0x89,0x02][..]),
        (0x193FF,&[0x8B,0x4D,0x0C,0x89,0x4A,0x04][..]),
        (0x19405,&[0x8B,0x45,0x10,0x89,0x42,0x08][..]),
        (0x19410,&[0x83,0xC4,0x20][..]),
        // Final write reuses EXACTLY the first read's buffer,
        // length and offset locals, plus the descriptor at EBP+8.
        (0x1942F,&[0x8D,0x8D,0xFC,0xEF,0xFF,0xFF,0x51][..]),
        (0x19436,&[0x8B,0x95,0xEC,0xEF,0xFF,0xFF,0x52][..]),
        (0x1943D,&[0x8B,0x85,0xF8,0xEF,0xFF,0xFF,0x50][..]),
        (0x19444,&[0x8D,0x4D,0x08,0x51][..]),
        (0x1944D,&[0x83,0xC4,0x10][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if !same_range_and_buffer(&READ_CURRENT,&WRITE_CURRENT) ||
       same_range_and_buffer(&READ_OTHER,&WRITE_CURRENT) {
        return Err("original .txt2 read/write local-slot model is internally inconsistent".into());
    }
    Ok(10) // four pipeline E8 targets + six code-region/range helper E8 targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_read_and_final_write_reuse_four_stack_slots(){
        assert!(same_range_and_buffer(&READ_CURRENT,&WRITE_CURRENT));
        assert_eq!(READ_CURRENT.backend_rva,0xA810);
        assert_eq!(WRITE_CURRENT.backend_rva,0xAA00);
    }
    #[test]
    fn second_read_uses_same_descriptor_but_distinct_buffer_and_range(){
        assert_eq!(READ_OTHER.descriptor_ebp_offset,READ_CURRENT.descriptor_ebp_offset);
        assert_ne!(READ_OTHER.buffer_ebp_offset,READ_CURRENT.buffer_ebp_offset);
        assert_ne!(READ_OTHER.offset_ebp_offset,READ_CURRENT.offset_ebp_offset);
        assert_ne!(READ_OTHER.length_ebp_offset,READ_CURRENT.length_ebp_offset);
    }
    #[test]
    fn original_static_call_order_is_two_reads_then_processing_then_write(){
        assert!(READ_CURRENT.call_rva<READ_OTHER.call_rva);
        assert!(READ_OTHER.call_rva<TRANSFORM_CALL_RVA);
        assert!(TRANSFORM_CALL_RVA<WRITE_CURRENT.call_rva);
    }
    #[test]
    fn original_mixer_uses_code_section_category() {
        assert_eq!(CODE_REGION_CATEGORY,4);
        assert_eq!(CATEGORY_CHECK_CALL_RVA,0x196AE);
        assert_eq!(CATEGORY_CHECK_RVA,0xA310);
        assert!(TRANSFORM_CALL_RVA<CATEGORY_CHECK_CALL_RVA);
    }
    #[test]
    fn original_a310_page_boundary_adjustment_is_bounded() {
        assert_eq!(adjusted_range_start(0),0);
        assert_eq!(adjusted_range_start(0x1000),0x0FFF);
        assert_eq!(adjusted_range_start(0x1001),0x1001);
        assert_eq!(adjusted_range_start(0xFFFF_F000),0xFFFF_EFFF);
        assert_eq!(adjusted_range_start(u32::MAX),u32::MAX);
    }

    #[test]
    fn unrelated_stack_offsets_do_not_qualify_as_same_read_write_range(){
        let mut wrong=WRITE_CURRENT;
        wrong.offset_ebp_offset-=4;
        assert!(!same_range_and_buffer(&READ_CURRENT,&wrong));
        wrong=WRITE_CURRENT;
        wrong.length_ebp_offset+=4;
        assert!(!same_range_and_buffer(&READ_CURRENT,&wrong));
        wrong=WRITE_CURRENT;
        wrong.descriptor_ebp_offset=0xC;
        assert!(!same_range_and_buffer(&READ_CURRENT,&wrong));
    }
}
