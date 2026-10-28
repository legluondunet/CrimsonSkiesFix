// Read-only static inspection of the exact original SafeDisc DLL's
// startup section-selector chain. No protected instructions are executed.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// DllMain -> A140 -> five helpers -> .txt2 selector dispatch -> 18990.
// The original x86 selector reads a raw 32-bit word from the PE NT
// header scratch buffer, then searches section VirtualAddress ranges.
// Four of the five source words are ZERO for this exact retail DLL.
// These are static *candidate* selections; no runtime reachability,
// registry write or successful decoding of .txt is asserted.

use super::{
    Pe32, Result, dll_relative_call_target, dll_expect_raw_x86,
    dll_backed_rva_offset, read_u32,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum DirectoryField { Rva, Size }

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BootstrapStep {
    pub category:u8,
    pub selector:u8,
    pub directory_index:usize,
    pub directory_field:DirectoryField,
    pub candidate_rva:u32,
    pub matching_section:Option<String>,
}

#[derive(Debug,Clone,Copy)]
struct Route {
    a140_call:u32,
    helper:u32,
    category_push:u32,
    category:u8,
    wrapper_call:u32,
    wrapper:u32,
    selector_push:u32,
    selector:u8,
    case_rva:u32,
    case_load:u32,
    case_load_bytes:[u8;7],
    find_section_call:u32,
    scratch_field_offset:u32,
}

const ROUTES:[Route;5]=[
    Route {
        a140_call:0xA181,helper:0xB3D0,
        category_push:0xB3E9,category:1,
        wrapper_call:0xB40B,wrapper:0x18890,
        selector_push:0x188AA,selector:11,
        case_rva:0x1841D,case_load:0x18424,
        case_load_bytes:[0x8B,0x84,0x24,0x1C,0x01,0x00,0x00],
        find_section_call:0x1842D,scratch_field_offset:0x11C,
    },
    Route {
        a140_call:0xA192,helper:0xB1C0,
        category_push:0xB1DA,category:1,
        wrapper_call:0xB201,wrapper:0x18860,
        selector_push:0x1887A,selector:6,
        case_rva:0x18357,case_load:0x1835E,
        case_load_bytes:[0x8B,0x84,0x24,0xC4,0x00,0x00,0x00],
        find_section_call:0x18367,scratch_field_offset:0xC4,
    },
    Route {
        a140_call:0xA1A3,helper:0xAFB0,
        category_push:0xAFC9,category:2,
        wrapper_call:0xAFED,wrapper:0x18830,
        // This wrapper forwards the selector from its caller.
        selector_push:0xAFEA,selector:5,
        case_rva:0x18304,case_load:0x18304,
        case_load_bytes:[0x8B,0x94,0x24,0xC0,0x00,0x00,0x00],
        find_section_call:0x18314,scratch_field_offset:0xC0,
    },
    Route {
        a140_call:0xA1B4,helper:0xAE10,
        category_push:0xAE27,category:0,
        wrapper_call:0xAE44,wrapper:0x188C0,
        selector_push:0x188DA,selector:7,
        case_rva:0x185D5,case_load:0x185D5,
        case_load_bytes:[0x8B,0x94,0x24,0xB0,0x00,0x00,0x00],
        find_section_call:0x185E5,scratch_field_offset:0xB0,
    },
    Route {
        a140_call:0xA1CB,helper:0xAC10,
        category_push:0xAC28,category:7,
        wrapper_call:0xAC45,wrapper:0x188F0,
        selector_push:0x1890A,selector:9,
        case_rva:0x18486,case_load:0x1848D,
        case_load_bytes:[0x8B,0x84,0x24,0x04,0x01,0x00,0x00],
        find_section_call:0x18496,scratch_field_offset:0x104,
    },
];

// The 0x181E0 function uses ESP+0x38 for the beginning of the
// copied NT header, and NT+0x78 is IMAGE_DATA_DIRECTORY[0].
// Therefore an original case reference to ESP+0xB0 selects the
// first directory RVA (not the PE section table or a byte key).
pub fn scratch_directory_field(stack_offset:u32)->Result<(usize,DirectoryField)> {
    let delta=stack_offset.checked_sub(0xB0)
        .ok_or_else(||"DLL selector scratch offset before first directory".to_string())?;
    if delta>=16*8 || (delta%4)!=0 {
        return Err("DLL selector scratch field is not within 16 PE data directories".into());
    }
    let field=if delta%8==0 {DirectoryField::Rva} else {DirectoryField::Size};
    Ok(((delta/8) as usize,field))
}

pub fn containing_section<'a>(
    requested:u32, sections:&'a [(String,u32,u32,u32)]
)->Option<&'a str> {
    if requested==0{return None;}
    sections.iter().find(|(_,va,virtual_size,raw_size)|{
        let size=if *virtual_size==0 {*raw_size} else {*virtual_size};
        requested>=*va && requested.checked_sub(*va).is_some_and(|delta|delta<size)
    }).map(|(name,_,_,_)|name.as_str())
}

fn check_call(dll:&[u8],pe:&Pe32,site:u32,dest:u32)->Result<()> {
    if dll_relative_call_target(dll,pe,site)?!=dest {
        return Err(format!("unexpected original DLL section-bootstrap call RVA 0x{site:08X}"));
    }
    Ok(())
}

pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<Vec<BootstrapStep>> {
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    if base!=0x008E_0000 {return Err("unexpected original DLL preferred image base".into());}

    // These gates prove the actual DllMain call order and that selector
    // 0x181E0 uses a bounded jump table. Jump table data are verified
    // by preferred VA, never treated as executable instruction bytes.
    check_call(dll,pe,0x987C,0xA140)?;
    check_call(dll,pe,0x989F,0x7212)?;
    check_call(dll,pe,0x98C5,0x7C4F)?;
    dll_expect_raw_x86(dll,pe,0x18263,&[0x83,0xF9,0x0A])?;
    dll_expect_raw_x86(dll,pe,0x18272,&[0xFF,0x24,0x8D,0x80,0x86,0x8F,0x00])?;
    dll_expect_raw_x86(dll,pe,0x189B6,&[0x8B,0x46,0x08])?;
    dll_expect_raw_x86(dll,pe,0x189C0,&[0x8B,0x46,0x10])?;
    dll_expect_raw_x86(dll,pe,0x189C3,&[0x8B,0x4E,0x0C])?;
    // The selector-search routine tests section VA <= requested RVA
    // < section VA + VirtualSize (or RawSize if VirtualSize==0).
    dll_expect_raw_x86(dll,pe,0x189C6,&[0x3B,0xCB])?;
    dll_expect_raw_x86(dll,pe,0x189CE,&[0x72,0x12])?;

    let sections:Vec<(String,u32,u32,u32)>=pe.sections.iter().map(|sec|{
        let name=String::from_utf8_lossy(sec.name.split(|&b|b==0).next().unwrap_or(&[])).to_string();
        (name,sec.virtual_address,sec.virtual_size,sec.raw_len as u32)
    }).collect();

    let mut output=Vec::new();
    for route in ROUTES {
        check_call(dll,pe,route.a140_call,route.helper)?;
        check_call(dll,pe,route.wrapper_call,route.wrapper)?;
        check_call(dll,pe,route.find_section_call,0x18990)?;
        dll_expect_raw_x86(dll,pe,route.category_push,&[0x6A,route.category])?;
        dll_expect_raw_x86(dll,pe,route.selector_push,&[0x6A,route.selector])?;
        dll_expect_raw_x86(dll,pe,route.case_load,&route.case_load_bytes)?;

        let table_entry=0x18680u32+4*u32::from(route.selector-1);
        let file=dll_backed_rva_offset(pe,table_entry,4)?;
        if read_u32(dll,file)?!=base+route.case_rva {
            return Err(format!("original selector {} jump-table RVA unexpectedly changed",
                route.selector));
        }
        let (dir,field)=scratch_directory_field(route.scratch_field_offset)?;
        let relative=dir*8+if field==DirectoryField::Size {4} else {0};
        // Each original PE data-directory entry lives at NT+0x78;
        // verified on this exact PE32 build, not guessed from a VA.
        let requested=read_u32(dll,nt+0x78+relative)?;
        let matched=containing_section(requested,&sections).map(str::to_string);
        output.push(BootstrapStep{
            category:route.category,selector:route.selector,
            directory_index:dir,directory_field:field,
            candidate_rva:requested,matching_section:matched
        });
    }

    let expected=[
        (1u8,11u8,13usize,DirectoryField::Size,0u32,None),
        (1,6,2,DirectoryField::Size,0,None),
        (2,5,2,DirectoryField::Rva,0,None),
        (0,7,0,DirectoryField::Rva,0x1F350,Some(".rdata")),
        (7,9,10,DirectoryField::Size,0,None),
    ];
    for (actual,(cat,selector,dir,field,value,section)) in output.iter().zip(expected) {
        if actual.category!=cat || actual.selector!=selector ||
           actual.directory_index!=dir || actual.directory_field!=field ||
           actual.candidate_rva!=value ||
           actual.matching_section.as_deref()!=section {
            return Err(format!("pinned DLL bootstrap selector {} differs from original",
                actual.selector));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_offsets_resolve_to_directory_rva_or_size() {
        assert_eq!(scratch_directory_field(0xB0).unwrap(),(0,DirectoryField::Rva));
        assert_eq!(scratch_directory_field(0xC0).unwrap(),(2,DirectoryField::Rva));
        assert_eq!(scratch_directory_field(0xC4).unwrap(),(2,DirectoryField::Size));
        assert_eq!(scratch_directory_field(0x104).unwrap(),(10,DirectoryField::Size));
        assert_eq!(scratch_directory_field(0x11C).unwrap(),(13,DirectoryField::Size));
        assert!(scratch_directory_field(0xAC).is_err());
        assert!(scratch_directory_field(0xB2).is_err());
        assert!(scratch_directory_field(0x130).is_err());
    }

    #[test]
    fn original_like_sections_resolve_export_directory_to_rdata() {
        let secs=vec![
            (".txt".to_string(),0x1000,0x7671,0x7800),
            (".text".to_string(),0x9000,0xE506,0xE600),
            (".txt2".to_string(),0x18000,0x5497,0x5600),
            (".rdata".to_string(),0x1E000,0x13B0,0x1400),
        ];
        assert_eq!(containing_section(0x1F350,&secs),Some(".rdata"));
        assert_eq!(containing_section(0,&secs),None);
        assert_eq!(containing_section(0x1F3B0,&secs),None);
        assert_eq!(containing_section(0x1E000,&secs),Some(".rdata"));
        assert_eq!(containing_section(0x1FFFF,&secs),None);
    }

    #[test]
    fn section_span_uses_nonzero_virtual_size_not_raw_file_padding() {
        let sections=vec![(".s".to_string(),0x1000,10,512)];
        assert_eq!(containing_section(0x1009,&sections),Some(".s"));
        assert_eq!(containing_section(0x100A,&sections),None);
        let fallback=vec![(".r".to_string(),0x1000,0,512)];
        assert_eq!(containing_section(0x11FF,&fallback),Some(".r"));
    }

    #[test]
    fn only_one_of_five_original_directory_selector_values_is_nonzero() {
        let selections=[
            (11u8,13usize,DirectoryField::Size,0u32),
            (6,2,DirectoryField::Size,0),
            (5,2,DirectoryField::Rva,0),
            (7,0,DirectoryField::Rva,0x1F350),
            (9,10,DirectoryField::Size,0),
        ];
        assert_eq!(selections.iter().filter(|row|row.3!=0).count(),1);
        assert_eq!(selections.iter().find(|row|row.3!=0).unwrap().0,7);
    }
}
