// Static PE-code-section discovery research from original SafeDisc DLL.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// No executable bytes are executed. This is an *independent hardened*
// PE32 metadata parser. Its validation is stricter than the original
// assembly and it must not be represented as original execution.

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target,read_u16,read_u32};

pub const PE_CODE_EXECUTE_MASK:u32=0x2000_0020;
pub const SECTION_HEADER_SIZE:usize=40;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct FirstExecutableSection {
    pub header_offset:usize,
    pub name:String,
    pub characteristics:u32,
    pub virtual_address:u32,
    pub raw_file_offset:u32,
    /// Original 18B90 chooses the lesser of VirtualSize and
    /// SizeOfRawData, except VirtualSize==0 selects raw size.
    /// This is NOT necessarily the actual file-backed length.
    pub original_selected_len:u32,
}

pub fn original_section_scan_length(virtual_size:u32,raw_size:u32)->u32 {
    if virtual_size==0 {raw_size} else {virtual_size.min(raw_size)}
}

fn bounded_slice<'a>(bytes:&'a[u8],at:usize,n:usize)->Result<&'a[u8]> {
    let end=at.checked_add(n).ok_or("PE metadata offset overflow")?;
    bytes.get(at..end).ok_or_else(||format!(
        "truncated PE metadata: offset 0x{at:X}, length 0x{n:X}"
    ))
}

/// Pure, safe analogue of the first case 183C2: pick the first
/// IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE section, derive the
/// raw offset and RVA, and apply original 18B90 length selection.
/// Original 18940 checks only NT 'PE' signature and calls A810
/// without inspecting its return values; this independent parser
/// deliberately ALSO requires MZ and bounded valid PE32 layout.
/// Results on a local original are source-metadata facts, NOT
/// proof which source descriptor the running SafeDisc chooses.
pub fn first_code_section_from_pe32(bytes:&[u8])->Result<Option<FirstExecutableSection>> {
    if bounded_slice(bytes,0,2)?!=b"MZ" {
        return Err("input has no DOS MZ signature".into());
    }
    let nt=read_u32(bytes,0x3C)? as usize;
    if bounded_slice(bytes,nt,4)?!=b"PE\0\0" {
        return Err("input has no PE NT signature".into());
    }
    let nsections=read_u16(bytes,nt+6)? as usize;
    let opt_len=read_u16(bytes,nt+20)? as usize;
    if nsections==0 || nsections>96 || opt_len<2 ||
       read_u16(bytes,nt+24)?!=0x10B {
        return Err("unsupported or invalid PE32 section layout".into());
    }
    let first=nt.checked_add(24).and_then(|n|n.checked_add(opt_len))
        .ok_or("PE32 first section offset overflow")?;
    let table_length=nsections.checked_mul(SECTION_HEADER_SIZE)
        .ok_or("PE32 section header count overflow")?;
    bounded_slice(bytes,first,table_length)?;
    for i in 0..nsections {
        let offset=first+i*SECTION_HEADER_SIZE;
        let characteristics=read_u32(bytes,offset+36)?;
        if characteristics&PE_CODE_EXECUTE_MASK!=PE_CODE_EXECUTE_MASK {continue}
        let name_bytes=bounded_slice(bytes,offset,8)?;
        let name_end=name_bytes.iter().position(|&b|b==0).unwrap_or(8);
        let name=String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
        let virtual_size=read_u32(bytes,offset+8)?;
        let virtual_address=read_u32(bytes,offset+12)?;
        let raw_size=read_u32(bytes,offset+16)?;
        let raw_file_offset=read_u32(bytes,offset+20)?;
        return Ok(Some(FirstExecutableSection {
            header_offset:offset,
            name, characteristics, virtual_address, raw_file_offset,
            original_selected_len:original_section_scan_length(virtual_size,raw_size),
        }));
    }
    Ok(None)
}

/// Check the TRUE original x86 semantics: 18940 retrieves DOS
/// and NT headers via A810 and tests PE signature at returned
/// output buffer; 181E0 computes section-header start/end using
/// SizeOfOptionalHeader and NumberOfSections; 183C2 finds the
/// first section whose Characteristics contain 0x20000020 and
/// writes raw offset, RVA and 18B90-selected length to outputs.
/// The original DLL itself is verified by the parent SHA-256 gate.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (at,expected) in [
        (0x18940u32,&[
            0x53,0x56,0x8B,0x74,0x24,0x10,0x57,
            0x8B,0x7C,0x24,0x10,0x56,0x6A,0x40,0x6A,0,
            0x57,0xE8,0xBA,0x1E,0xFF,0xFF,
        ][..]),
        (0x18956,&[
            0x8B,0x5C,0x24,0x28,0x8B,0x46,0x3C,
            0x83,0xC4,0x10,0x53,0x68,0xF8,0,0,0,
            0x50,0x57,0xE8,0xA3,0x1E,0xFF,0xFF,
        ][..]),
        (0x1896D,&[
            0x8B,0x0B,0x83,0xC4,0x10,0x33,0xC0,
            0x81,0xF9,0x50,0x45,0,0,0x5F,0x5E,
            0x0F,0x94,0xC0,
        ][..]),
        (0x18204,&[
            0x8B,0x54,0x24,0x58,0x8B,0x4C,0x24,0x4A,
            0x8B,0x84,0x24,0x78,0x01,0,0,
            0x81,0xE2,0xFF,0xFF,0,0,
            0x81,0xE1,0xFF,0xFF,0,0,
            0x83,0xC4,0x0C,
            0x8D,0x44,0x10,0x18,
            0x8D,0x0C,0x89,
            0x8D,0x1C,0xC8,
        ][..]),
        (0x183C2,&[
            0x8D,0x54,0x24,0x10,0x52,0x53,0x50,
            0x68,0x20,0,0,0x20,
            0x68,0x20,0,0,0x20,0x57,
        ][..]),
        (0x183D4,&[0xE8,0x17,0x06,0,0][..]),
        (0x183DC,&[
            0x8B,0xF0,0x85,0xF6,0x0F,0x84,0x1A,0x01,0,0,
        ][..]),
        (0x183E6,&[
            0x8B,0x8C,0x24,0x80,0x01,0,0,
            0x8B,0x44,0x24,0x24,0x8B,0x54,0x24,0x1C,
            0x89,0x01,
        ][..]),
        (0x183F7,&[
            0x8B,0x84,0x24,0x88,0x01,0,0,
            0x8D,0x4C,0x24,0x10,0x51,0x89,0x10,
        ][..]),
        (0x18405,&[0xE8,0x86,0x07,0,0,0x89,0x45,0][..]),
        (0x18B90,&[
            0x8B,0x4C,0x24,0x04,0x8B,0x41,0x08,
            0x85,0xC0,0x74,0x0A,0x8B,0x49,0x10,
            0x3B,0xC1,0x72,0x06,0x8B,0xC1,0xC3,
            0x8B,0x41,0x10,0xC3,
        ][..]),
        (0x189F0,&[
            0x8B,0x44,0x24,0x14,0x53,0x55,0x56,
            0x8B,0x74,0x24,0x1C,0x57,0x3B,0xF0,0x73,0x41,
        ][..]),
        (0x18A1A,&[
            0x8B,0x57,0x24,0x8B,0xCB,0x23,0xCA,
            0x83,0xC4,0x10,0x3B,0xCD,0x74,0x12,
        ][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,expected)?;
    }
    for (source,expected) in [
        (0x18951u32,0xA810u32),
        (0x18968,0xA810),
        (0x181FF,0x18940),
        (0x183D4,0x189F0),
        (0x18405,0x18B90),
        (0x18A15,0xA810),
    ] {
        if dll_relative_call_target(dll,pe,source)?!=expected {
            return Err(format!(
                "original PE section scan CALL at 0x{source:05X} changed"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(sections:&[(&str,u32,u32,u32,u32,u32)])->Vec<u8>{
        let nt=0x80usize;
        let first=nt+24+0xE0;
        let mut bytes=vec![0u8;first+40*sections.len()];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&(nt as u32).to_le_bytes());
        bytes[nt..nt+4].copy_from_slice(b"PE\0\0");
        bytes[nt+6..nt+8].copy_from_slice(&(sections.len() as u16).to_le_bytes());
        bytes[nt+20..nt+22].copy_from_slice(&0xE0u16.to_le_bytes());
        bytes[nt+24..nt+26].copy_from_slice(&0x10Bu16.to_le_bytes());
        for (i,&(name,flags,vs,rva,rawsz,rawat)) in sections.iter().enumerate() {
            let at=first+i*40;
            let name=name.as_bytes();
            bytes[at..at+name.len()].copy_from_slice(name);
            for (offset,value) in [
                (8,vs),(12,rva),(16,rawsz),(20,rawat),(36,flags),
            ] {
                bytes[at+offset..at+offset+4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    #[test]
    fn first_case_requires_both_code_and_execute_characteristics() {
        let data=fixture(&[
            (".rdata",0x4000_0040,0x100,0x3000,0x200,0x400),
            (".exec",0x2000_0000,0x100,0x4000,0x200,0x600),
            (".code",0x0000_0020,0x100,0x5000,0x200,0x800),
            (".text",0x6000_0020,0x123,0x6000,0x200,0xA00),
        ]);
        let found=first_code_section_from_pe32(&data).unwrap().unwrap();
        assert_eq!(found.name,".text");
        assert_eq!(found.header_offset,0x80+24+0xE0+3*40);
        assert_eq!(found.virtual_address,0x6000);
        assert_eq!(found.raw_file_offset,0xA00);
        assert_eq!(found.original_selected_len,0x123);
    }
    #[test]
    fn original_size_selector_handles_zero_and_clips_to_raw_size() {
        assert_eq!(original_section_scan_length(0,0x200),0x200);
        assert_eq!(original_section_scan_length(0x300,0x200),0x200);
        assert_eq!(original_section_scan_length(0x100,0x200),0x100);
        assert_eq!(original_section_scan_length(0x200,0x200),0x200);
        assert_eq!(original_section_scan_length(0,0),0);
    }
    #[test]
    fn safely_handles_no_executable_sections() {
        let data=fixture(&[(".data",0xC0000040,0x80,0x1000,0x200,0x200)]);
        assert_eq!(first_code_section_from_pe32(&data).unwrap(),None);
    }
    #[test]
    fn rejects_truncated_or_invalid_pe32_metadata() {
        let bytes=fixture(&[(".text",0x60000020,0x80,0x1000,0x200,0x200)]);
        for len in [0usize,1,0x3F,0x80,bytes.len()-1] {
            assert!(first_code_section_from_pe32(&bytes[..len]).is_err());
        }
        let mut corrupted=bytes.clone();
        corrupted[0..2].copy_from_slice(b"ZZ");
        assert!(first_code_section_from_pe32(&corrupted).is_err());
        corrupted=bytes.clone();
        corrupted[0x80..0x84].copy_from_slice(b"XXXX");
        assert!(first_code_section_from_pe32(&corrupted).is_err());
    }
}
