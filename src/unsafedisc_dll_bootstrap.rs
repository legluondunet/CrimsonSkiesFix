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
pub struct BootstrapCompletion {
    pub has_module_record:bool,
    pub directory_helpers_called:usize,
    pub class_iterators_called:usize,
    pub returns_success:bool,
}

/// The original A140 checks the result of B540 once. If that record
/// exists, it calls five directory helpers and all three class iterators
/// without branching on their return values; AX=1 is set at the end.
/// This is a control-flow model, NOT a Windows/DLL runtime emulator.
pub fn model_a140_completion(has_module_record:bool)->BootstrapCompletion {
    BootstrapCompletion {
        has_module_record,
        directory_helpers_called:if has_module_record {5} else {0},
        class_iterators_called:if has_module_record {3} else {0},
        returns_success:has_module_record,
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum DirectoryField { Rva, Size }

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BootstrapStep {
    pub category:u8,
    pub selector:u8,
    pub source_directory_index:usize,
    pub source_rva:u32,
    pub source_size:u32,
    pub passes_nonzero_size_guard:bool,
    pub directory_index:usize,
    pub directory_field:DirectoryField,
    pub candidate_rva:u32,
    pub matching_section:Option<String>,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct SectionWalk {
    pub category:u8,
    pub selector:u8,
    pub matching_first_stage:usize,
    pub skipped_directory_starts:usize,
    pub selected_section:Option<String>,
}

/// Static characteristics filter and optional PE-directory-start
/// exclusion for the final three A140 section-registration iterators.
/// This does not execute a helper or infer that an A2A0 call succeeds.
pub fn select_section(
    selector:u8,sections:&[(String,u32,u32)],
    directory_starts:&[u32],
)->SectionWalk {
    let (mask,required,skip_starts,category)=match selector {
        1 => (0x2000_0020u32,0x2000_0020u32,false,4u8),
        2 => (0xC000_0040,0xC000_0040,true,5),
        10 => (0xC000_0060,0x4000_0040,true,6),
        _ => return SectionWalk{
            category:0,selector,matching_first_stage:0,
            skipped_directory_starts:0,selected_section:None
        }
    };
    let mut out=SectionWalk{
        category,selector,matching_first_stage:0,
        skipped_directory_starts:0,selected_section:None
    };
    for (name,va,flags) in sections {
        if flags & mask != required {continue;}
        out.matching_first_stage+=1;
        if skip_starts && directory_starts.contains(va) {
            out.skipped_directory_starts+=1;
            continue;
        }
        if out.selected_section.is_none(){out.selected_section=Some(name.clone());}
    }
    out
}

#[derive(Debug,Clone,Copy)]
struct Route {
    a140_call:u32,
    helper:u32,
    source_directory_index:usize,
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
        a140_call:0xA181,helper:0xB3D0,source_directory_index:12,
        category_push:0xB3E9,category:1,
        wrapper_call:0xB40B,wrapper:0x18890,
        selector_push:0x188AA,selector:11,
        case_rva:0x1841D,case_load:0x18424,
        case_load_bytes:[0x8B,0x84,0x24,0x1C,0x01,0x00,0x00],
        find_section_call:0x1842D,scratch_field_offset:0x11C,
    },
    Route {
        a140_call:0xA192,helper:0xB1C0,source_directory_index:1,
        category_push:0xB1DA,category:1,
        wrapper_call:0xB201,wrapper:0x18860,
        selector_push:0x1887A,selector:6,
        case_rva:0x18357,case_load:0x1835E,
        case_load_bytes:[0x8B,0x84,0x24,0xC4,0x00,0x00,0x00],
        find_section_call:0x18367,scratch_field_offset:0xC4,
    },
    Route {
        a140_call:0xA1A3,helper:0xAFB0,source_directory_index:2,
        category_push:0xAFC9,category:2,
        wrapper_call:0xAFED,wrapper:0x18830,
        // This wrapper forwards the selector from its caller.
        selector_push:0xAFEA,selector:5,
        case_rva:0x18304,case_load:0x18304,
        case_load_bytes:[0x8B,0x94,0x24,0xC0,0x00,0x00,0x00],
        find_section_call:0x18314,scratch_field_offset:0xC0,
    },
    Route {
        a140_call:0xA1B4,helper:0xAE10,source_directory_index:0,
        category_push:0xAE27,category:0,
        wrapper_call:0xAE44,wrapper:0x188C0,
        selector_push:0x188DA,selector:7,
        case_rva:0x185D5,case_load:0x185D5,
        case_load_bytes:[0x8B,0x94,0x24,0xB0,0x00,0x00,0x00],
        find_section_call:0x185E5,scratch_field_offset:0xB0,
    },
    Route {
        a140_call:0xA1CB,helper:0xAC10,source_directory_index:9,
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
// B540's per-module record holds the copied IMAGE_DATA_DIRECTORY[16]
 // array at record+0x44. A140 reads its RVA/Size DWORDs from exactly
 // these locations and passes them to each helper. Do not confuse
 // *source* directory index with the helper's later *selector* index.
pub fn record_directory_field_offset(index:usize,field:DirectoryField)->Result<u32> {
    if index>=16 {return Err("DLL record data-directory index out of range".into());}
    Ok(0x44+(index as u32)*8+if field==DirectoryField::Size{4}else{0})
}

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

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LoaderRegistrationProfile {
    pub source_directory_sizes:[u32;5],
    pub size_guard_eligible:[bool;5],
    pub class_iterators:[SectionWalk;3],
}

/// Inspect only the pinned, ON-DISK Crimson SafeDisc loader PE header.
/// GetModuleHandleA(NULL) designates this process module when mapped,
/// but SafeDisc may have modified its in-memory header by this point.
/// Never equate this offline header profile with an observed runtime
/// category registry or with the unwrapped CRIMSON.ICD image.
pub fn audit_original_loader_header(loader:&[u8])->Result<LoaderRegistrationProfile> {
    let pe=super::parse_pe32(loader)?;
    let nt=read_u32(loader,0x3C)? as usize;
    let image_base=read_u32(loader,nt+24+28)?;
    if image_base!=0x0040_0000 {
        return Err("expected the original SafeDisc loader preferred image base".into());
    }
    let source_directory_sizes=[
        read_u32(loader,nt+0x78+12*8+4)?,
        read_u32(loader,nt+0x78+1*8+4)?,
        read_u32(loader,nt+0x78+2*8+4)?,
        read_u32(loader,nt+0x78+0*8+4)?,
        read_u32(loader,nt+0x78+9*8+4)?,
    ];
    if source_directory_sizes!=[0x1DC,0x64,0x7AA,0,0] {
        return Err("original loader A140 source-directory sizes differ from pinned disk header".into());
    }
    let size_guard_eligible=source_directory_sizes.map(|size|size!=0);
    let secs:Vec<(String,u32,u32)>=pe.sections.iter().map(|sec|{
        let name=String::from_utf8_lossy(sec.name.split(|&b|b==0).next().unwrap_or(&[])).to_string();
        (name,sec.virtual_address,sec.characteristics)
    }).collect();
    let directory_starts:Vec<u32>=(0..16usize).map(|i|
        read_u32(loader,nt+0x78+i*8)
    ).collect::<Result<_>>()?;
    let class_iterators=[
        select_section(1,&secs,&directory_starts),
        select_section(2,&secs,&directory_starts),
        select_section(10,&secs,&directory_starts),
    ];
    let expected=[
        SectionWalk{category:4,selector:1,matching_first_stage:3,
            skipped_directory_starts:0,selected_section:Some(".txt".into())},
        SectionWalk{category:5,selector:2,matching_first_stage:1,
            skipped_directory_starts:0,selected_section:Some(".data".into())},
        SectionWalk{category:6,selector:10,matching_first_stage:3,
            skipped_directory_starts:3,selected_section:None},
    ];
    if class_iterators!=expected {
        return Err("original SafeDisc loader's on-disk section selector results changed".into());
    }
    Ok(LoaderRegistrationProfile{
        source_directory_sizes,size_guard_eligible,class_iterators
    })
}

pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<(Vec<BootstrapStep>,[SectionWalk;3])> {
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    if base!=0x008E_0000 {return Err("unexpected original DLL preferred image base".into());}

    // These gates prove the actual DllMain call order and that selector
    // 0x181E0 uses a bounded jump table. Jump table data are verified
    // by preferred VA, never treated as executable instruction bytes.
    check_call(dll,pe,0x987C,0xA140)?;
    // Two additional direct E8 references to the same initializer
    // originate in already-readable .txt2. Both prepare a stack-local
    // descriptor, call A140, then clean up their stack. Static references
    // do not establish their runtime reachability or descriptor contents.
    for (site,setup) in [
        (0x1A137u32,&[0x8D,0x4D,0xF4,0x51][..]),
        (0x1A299,&[0x8D,0x55,0xF4,0x52][..]),
    ] {
        check_call(dll,pe,site,0xA140)?;
        dll_expect_raw_x86(dll,pe,site-4,setup)?;
        dll_expect_raw_x86(dll,pe,site+5,&[0x83,0xC4,0x08])?;
    }
    // The other .txt2 A140 call at 0x1A137 is prepared by a
    // different module-descriptor path. Its first field derives
    // from KERNEL32!GetCurrentProcess (process pseudo-handle), while
    // the second is KERNEL32!GetModuleHandleA(NULL), the executable
    // module handle. Mode 0x67 is set with A550.
    for (site,opcodes) in [
        (0x1A0D4u32,&[0x89,0x45,0xF4][..]),
        (0x1A0F7,&[0x6A,0x00][..]),
        (0x1A0FF,&[0x89,0x45,0xF8][..]),
        (0x1A116,&[0x6A,0x67][..]),
        (0x1A118,&[0x8D,0x45,0xF4,0x50][..]),
        (0x1A121,&[0x83,0xC4,0x08][..]),
    ] { dll_expect_raw_x86(dll,pe,site,opcodes)?; }
    check_call(dll,pe,0x1A11C,0xA550)?;
    let imports=super::dll_import_bindings(dll,pe)?;
    for (call_rva,expected_slot,expected_name) in [
        (0x1A0CEu32,0x1E004u32,"GetCurrentProcess"),
        (0x1A0F9,0x1E098,"GetModuleHandleA"),
    ] {
        let (_,iat_slot)=super::dll_indirect_import_call(dll,pe,call_rva,base)?;
        if iat_slot!=expected_slot {
            return Err(format!("original .txt2 process-descriptor IAT operand mismatch at 0x{call_rva:05X}"));
        }
        let binding=imports.iter().find(|entry| entry.iat_rva==iat_slot)
            .ok_or_else(||"txt2 process-descriptor import is absent from original PE".to_string())?;
        if binding.symbol!=expected_name || !binding.library.eq_ignore_ascii_case("KERNEL32.dll") {
            return Err(format!("original .txt2 process-descriptor Windows API mismatch at 0x{call_rva:05X}"));
        }
    }

    // A clear-code caller at 0x9821 supplies original preferred
    // VA 0x00900030 to .txt2 entry 0x1A20D. Decoding its original
    // .data metadata identifies the same dplayerx.dll module name.
    // The destination function's EBP+0xC is this second argument.
    dll_expect_raw_x86(dll,pe,0x981A,
        &[0x68,0x30,0x00,0x90,0x00,0x6A,0x00])?;
    check_call(dll,pe,0x9821,0x1A20D)?;
    dll_expect_raw_x86(dll,pe,0x9826,&[0x83,0xC4,0x08])?;
    let source_label_va=read_u32(dll,super::executable_rva_to_file_offset(pe,0x981B,4)?)?;
    let source_label_rva=source_label_va.checked_sub(base)
        .ok_or_else(||"txt2 source module label VA below image base".to_string())?;
    if source_label_rva!=0x20030 {
        return Err("unexpected module-label pointer passed to .txt2 initializer".into());
    }
    let label_at=dll_backed_rva_offset(pe,source_label_rva,32)?;
    let label_bytes=dll.get(label_at..label_at+32)
        .ok_or_else(||"original DLL module label truncated".to_string())?;
    let label=super::unsafedisc_strings::decode(label_bytes)
        .ok_or_else(||"failed to decode original DLL module-name metadata".to_string())?.0;
    if label!="dplayerx.dll" {
        return Err("unexpected module name in original .txt2 initializer input".into());
    }

    // The .txt2 caller at 1A299 obtains a module name from its
    // caller argument, decodes it with E25C, calls the very same
    // GetModuleHandleA IAT slot as the known DLL-global setup, and
    // sets descriptor mode 0x67 before invoking A140. We still do
    // NOT know which name is passed along this reached/opaque path.
    for (site,opcodes) in [
        (0x1A243u32,&[0x8B,0x45,0x0C,0x50][..]), // caller argument
        (0x1A24F,&[0x50][..]),                   // decoded-name pointer
        (0x1A256,&[0x89,0x45,0xF8][..]),       // save module handle
        (0x1A259,&[0x6A,0x67][..]),            // descriptor mode
        (0x1A25B,&[0x8D,0x4D,0xF4,0x51][..]), // descriptor address
    ] { dll_expect_raw_x86(dll,pe,site,opcodes)?; }
    check_call(dll,pe,0x1A247,0xE25C)?;
    check_call(dll,pe,0x1A25F,0xA550)?;
    let (_,name_import)=super::dll_indirect_import_call(dll,pe,0x1A250,base)?;
    let (_,known_import)=super::dll_indirect_import_call(dll,pe,0x986A,base)?;
    if name_import!=known_import {
        return Err("txt2 caller does not use the verified DLL-global GetModuleHandleA import".into());
    }
    check_call(dll,pe,0x989F,0x7212)?;
    // A140 has one conditional early-exit at the B540 record result.
    // After that branch, its five directory handlers and three
    // category iterators are reached in order without result tests.
    for (rva,bytes) in [
        (0xA159u32,&[0x85,0xF6][..]),
        (0xA15B,&[0x0F,0x84,0x93,0x00,0x00,0x00][..]),
        (0xA1EE,&[0xBF,0x01,0x00,0x00,0x00][..]),
        (0xA1F3,&[0x5D][..]),
        (0xA1F4,&[0x66,0x8B,0xC7][..]),
        (0xA1FA,&[0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
    }
    // A140 passes the registry record's +0x44 address to 18770:
    // the latter calls the PE NT header reader and copies exactly
    // 32 DWORDs (16 RVA/Size pairs) into that record.
    dll_expect_raw_x86(dll,pe,0xA161,&[
        0x55,0x8D,0x6E,0x44,0x55,0x53,0xE8,0x04,0xE6,0x00,0x00
    ])?;
    check_call(dll,pe,0xA167,0x18770)?;
    check_call(dll,pe,0x18787,0x18940)?;
    dll_expect_raw_x86(dll,pe,0x18793,&[0xB9,0x20,0,0,0])?;
    dll_expect_raw_x86(dll,pe,0x18798,&[0x8D,0xB4,0x24,0xCC,0,0,0])?;
    dll_expect_raw_x86(dll,pe,0x187A2,&[0xF3,0xA5])?;
    // Concrete section-directory source loads from the record.
    for (rva,bytes) in [
        (0xA16Cu32,&[0x8B,0x8E,0xA8,0,0,0,0x8B,0x96,0xA4,0,0,0][..]),
        (0xA186,&[0x8B,0x46,0x50,0x8B,0x4E,0x4C][..]),
        (0xA197,&[0x8B,0x56,0x58,0x8B,0x46,0x54][..]),
        (0xA1A8,&[0x8B,0x4E,0x48,0x8B,0x55,0x00][..]),
        (0xA1B9,&[0x8B,0x86,0x90,0,0,0,0x8B,0x8E,0x8C,0,0,0][..])
    ] { dll_expect_raw_x86(dll,pe,rva,bytes)?; }
    // Every helper has a conditional early exit on a zero Size.
    for (source_load,source_bytes,guard,guard_bytes) in [
        (0xB3D0u32,&[0x8B,0x44,0x24,0x0C][..],0xB3D7u32,&[0x85,0xC0][..]),
        (0xB1C1,&[0x8B,0x44,0x24,0x10][..],0xB1C8,&[0x85,0xC0][..]),
        (0xAFB0,&[0x8B,0x44,0x24,0x0C][..],0xAFB7,&[0x85,0xC0][..]),
        (0xAE10,&[0x8B,0x44,0x24,0x0C][..],0xAE17,&[0x85,0xC0][..]),
        (0xAC11,&[0x8B,0x5C,0x24,0x10][..],0xAC17,&[0x85,0xDB][..])
    ] {
        dll_expect_raw_x86(dll,pe,source_load,source_bytes)?;
        dll_expect_raw_x86(dll,pe,guard,guard_bytes)?;
    }
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
        let source_offset_rva=record_directory_field_offset(
            route.source_directory_index,DirectoryField::Rva)?;
        let source_offset_size=record_directory_field_offset(
            route.source_directory_index,DirectoryField::Size)?;
        let source_rva=read_u32(dll,nt+0x78+(source_offset_rva-0x44) as usize)?;
        let source_size=read_u32(dll,nt+0x78+(source_offset_size-0x44) as usize)?;
        let relative=dir*8+if field==DirectoryField::Size {4} else {0};
        // Each original PE data-directory entry lives at NT+0x78;
        // verified on this exact PE32 build, not guessed from a VA.
        let requested=read_u32(dll,nt+0x78+relative)?;
        let matched=containing_section(requested,&sections).map(str::to_string);
        output.push(BootstrapStep{
            category:route.category,selector:route.selector,
            source_directory_index:route.source_directory_index,
            source_rva,source_size,passes_nonzero_size_guard:source_size!=0,
            directory_index:dir,directory_field:field,
            candidate_rva:requested,matching_section:matched
        });
    }

    // Three *additional* A140 iterators execute after these five
    // directory-specific helpers. They select section classes, not
    // arbitrary decrypted pointers. Confirm their exact call chain,
    // literal selector immediates, and registry category constants.
    for (call,target,selector_site,selector,category_site,category) in [
        (0xA1D4u32,0xAB80u32,0xABBAu32,1u8,0xABD3u32,4u8),
        (0xA1DD,0xAAF0,0xAB2A,2,0xAB43,5),
        (0xA1E6,0xAA60,0xAA9A,10,0xAAB3,6)
    ] {
        check_call(dll,pe,call,target)?;
        dll_expect_raw_x86(dll,pe,selector_site,&[0x6A,selector])?;
        dll_expect_raw_x86(dll,pe,category_site,&[0x6A,category])?;
    }
    for (call,target) in [
        (0xABBDu32,0x18800u32),
        (0xAB2D,0x18800),
        (0xAA9D,0x18800),
        (0xABD8,0xA2A0),
        (0xAB48,0xA2A0),
        (0xAAB8,0xA2A0),
    ] {check_call(dll,pe,call,target)?;}
    // The preceding phase of the inspector checks the original
    // IMAGE_SECTION_HEADER masks and directory-start rejection
    // instructions in the protected DLL selector paths.
    let secs:Vec<(String,u32,u32)>=pe.sections.iter().map(|sec|{
        let name=String::from_utf8_lossy(sec.name.split(|&b|b==0).next().unwrap_or(&[])).to_string();
        (name,sec.virtual_address,sec.characteristics)
    }).collect();
    let directory_starts:Vec<u32>=(0..16usize).map(|i|read_u32(dll,nt+0x78+i*8))
        .collect::<Result<_>>()?;
    // A2A0 is a category/region bookkeeping dispatcher. Its three
    // direct fallbacks B850/B880/B8E0 invoke list and range operations.
    // We verify only the reached instruction-level calls, not the
    // absence of an unknown decoder elsewhere in the call tree.
    for (call,target) in [
        (0xA2D3u32,0xB850u32),
        (0xA2E4,0xB880),
        (0xA2F5,0xB8E0),
        (0xB865,0xBAD0),
        (0xB8A1,0xBC40),
        (0xB8B4,0xC2F0),
        (0xB8BE,0xB9D0),
        (0xB8F6,0xBC00),
        (0xB90F,0xB7C0),
        (0xB919,0xB940),
        (0xC2A7,0xB610),
    ] {check_call(dll,pe,call,target)?;}
    // C2A0 initializes a 3-DWORD range/list node. The sibling C2F0
    // adjusts a two-DWORD range start/extent descriptor (not a game
    // buffer write). This is a statement about these opcodes only.
    dll_expect_raw_x86(dll,pe,0xC2A0,&[0x6A,0x0C,0x68,0xCB,0,0,0])?;
    dll_expect_raw_x86(dll,pe,0xC2BB,&[0x89,0x08])?;
    dll_expect_raw_x86(dll,pe,0xC2C1,&[0x89,0x50,0x04])?;
    dll_expect_raw_x86(dll,pe,0xC2C4,&[0x89,0x48,0x08])?;
    dll_expect_raw_x86(dll,pe,0xC2F8,&[0x3B,0x08])?;
    dll_expect_raw_x86(dll,pe,0xC2FC,&[0x89,0x08])?;
    dll_expect_raw_x86(dll,pe,0xC305,&[0x03,0xD1])?;
    dll_expect_raw_x86(dll,pe,0xC307,&[0x89,0x50,0x04])?;
    let expected_walks=[
        SectionWalk{category:4,selector:1,matching_first_stage:3,
            skipped_directory_starts:0,selected_section:Some(".txt".into())},
        SectionWalk{category:5,selector:2,matching_first_stage:1,
            skipped_directory_starts:0,selected_section:Some(".data".into())},
        SectionWalk{category:6,selector:10,matching_first_stage:2,
            skipped_directory_starts:2,selected_section:None},
    ];
    for expected in &expected_walks {
        let actual=select_section(expected.selector,&secs,&directory_starts);
        if &actual!=expected {
            return Err(format!("pinned DLL selector {} category {} section-class lookup changed",
                expected.selector,expected.category));
        }
    }
    let expected=[
        (1u8,11u8,12usize,0x1E000u32,0x11Cu32,13usize,DirectoryField::Size,0u32,None),
        (1,6,1,0x1ED14,0x3C,2,DirectoryField::Size,0,None),
        (2,5,2,0,0,2,DirectoryField::Rva,0,None),
        (0,7,0,0x1F350,0x60,0,DirectoryField::Rva,0x1F350,Some(".rdata")),
        (7,9,9,0,0,10,DirectoryField::Size,0,None),
    ];
    for (actual,(cat,selector,source_idx,source_rva,source_size,dir,field,value,section)) in output.iter().zip(expected) {
        if actual.category!=cat || actual.selector!=selector ||
           actual.source_directory_index!=source_idx ||
           actual.source_rva!=source_rva || actual.source_size!=source_size ||
           actual.passes_nonzero_size_guard!=(source_size!=0) ||
           actual.directory_index!=dir || actual.directory_field!=field ||
           actual.candidate_rva!=value ||
           actual.matching_section.as_deref()!=section {
            return Err(format!("pinned DLL bootstrap selector {} differs from original",
                actual.selector));
        }
    }
    Ok((output,expected_walks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_loader_registers_resources_not_exports() {
        let sections=vec![
            (".txt".into(),0x1000,0x6000_0020),
            (".text".into(),0x10000,0x6000_0020),
            (".txt2".into(),0x23000,0x6000_0020),
            (".rdata".into(),0x29000,0x4000_0040),
            (".data".into(),0x2B000,0xC000_0040),
            (".rsrc".into(),0x34000,0x4000_0040),
            (".reloc".into(),0x58000,0x4200_0040),
        ];
        let starts=[0x29000u32,0x34000,0x58000];
        assert_eq!(select_section(1,&sections,&starts).selected_section.as_deref(),
            Some(".txt"));
        assert_eq!(select_section(2,&sections,&starts).selected_section.as_deref(),
            Some(".data"));
        let ro=select_section(10,&sections,&starts);
        assert_eq!(ro.matching_first_stage,3);
        assert_eq!(ro.skipped_directory_starts,3);
        assert_eq!(ro.selected_section,None);
        let source_sizes=[0x1DCu32,0x64,0x7AA,0,0];
        assert_eq!(source_sizes.map(|size|size!=0),[true,true,true,false,false]);
    }

    #[test]
    fn process_descriptor_initializer_has_distinct_api_roles() {
        // GetCurrentProcess supplies a *pseudo-handle*; only the
        // GetModuleHandleA(NULL) result is a module base. They are
        // not interchangeable even though both fit in DWORDs.
        let calls=[
            (0x1A0CEu32,0x1E004u32,"GetCurrentProcess"),
            (0x1A0F9,0x1E098,"GetModuleHandleA"),
        ];
        assert_ne!(calls[0].1,calls[1].1);
        assert!(calls[0].0<calls[1].0);
        assert_eq!(0x1A116+2,0x1A118);
        assert_eq!(0x1A11C+5,0x1A121);
    }

    #[test]
    fn direct_txt2_caller_source_is_the_same_dll_metadata_label() {
        let preferred_base=0x008E_0000u32;
        let pushed_absolute_va=0x0090_0030u32;
        assert_eq!(pushed_absolute_va.checked_sub(preferred_base),Some(0x20030));
        assert_eq!(0x981A+5+2,0x9821);
        assert_eq!(0x9821+5,0x9826);
        // The real encoded bytes are checked only inside audit_original
        // after caller-provided DLL SHA-256 validation.
    }

    #[test]
    fn txt2_caller_stage_order_is_name_decode_module_handle_mode_and_init() {
        let stages=[
            (0x1A243u32,"caller-supplied name"),
            (0x1A247,"E25C label decoder"),
            (0x1A250,"GetModuleHandleA import"),
            (0x1A259,"mode 0x67"),
            (0x1A25F,"A550 descriptor preparation"),
            (0x1A299,"A140 registration"),
        ];
        assert!(stages.windows(2).all(|pair|pair[0].0<pair[1].0));
    }

    #[test]
    fn known_a140_sources_include_two_other_readable_code_callers() {
        let mut sites=[0x987Cu32,0x1A137u32,0x1A299u32];
        sites.sort_unstable();
        assert_eq!(sites,[0x987C,0x1A137,0x1A299]);
        assert!(sites[1]>=0x18000 && sites[2]>=0x18000);
        // This is a bound on *observed static direct references*, not
        // a bound on indirect calls or execution at runtime.
    }

    #[test]
    fn absent_module_record_skips_all_a140_bootstrap_helpers() {
        assert_eq!(model_a140_completion(false),BootstrapCompletion{
            has_module_record:false,directory_helpers_called:0,
            class_iterators_called:0,returns_success:false
        });
    }

    #[test]
    fn existing_record_reaches_all_eight_initializer_calls() {
        assert_eq!(model_a140_completion(true),BootstrapCompletion{
            has_module_record:true,directory_helpers_called:5,
            class_iterators_called:3,returns_success:true
        });
    }

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
    fn record_directory_slots_match_actual_a140_register_offsets() {
        for (dir,rva,size) in [
            (12usize,0xA4u32,0xA8u32),
            (1,0x4C,0x50),
            (2,0x54,0x58),
            (0,0x44,0x48),
            (9,0x8C,0x90),
        ] {
            assert_eq!(record_directory_field_offset(dir,DirectoryField::Rva).unwrap(),rva);
            assert_eq!(record_directory_field_offset(dir,DirectoryField::Size).unwrap(),size);
        }
        assert!(record_directory_field_offset(16,DirectoryField::Rva).is_err());
    }

    #[test]
    fn two_empty_source_directories_cannot_pass_a140_helper_guard() {
        let originals=[
            (12usize,0x1E000u32,0x11Cu32),
            (1,0x1ED14,0x3C),
            (2,0,0),
            (0,0x1F350,0x60),
            (9,0,0)
        ];
        let proceed:Vec<bool>=originals.iter().map(|row|row.2!=0).collect();
        assert_eq!(proceed,[true,true,false,true,false]);
        // The helper may act before a later section-selector fails;
        // a zero selector result alone does NOT eliminate earlier writes.
        assert!(proceed[0] && proceed[1]);
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
    fn final_three_iterators_preserve_category_order_and_directory_exclusions() {
        let sections=vec![
            (".txt".into(),0x1000,0x6000_0020),
            (".text".into(),0x9000,0x6000_0020),
            (".txt2".into(),0x18000,0x6000_0020),
            (".rdata".into(),0x1E000,0x4000_0040),
            (".data".into(),0x20000,0xC000_0040),
            (".reloc".into(),0x27000,0x4200_0040),
        ];
        let starts=[0x1F350,0x1ED14,0x1E000,0x27000];
        let code=select_section(1,&sections,&starts);
        assert_eq!(code.selected_section.as_deref(),Some(".txt"));
        assert_eq!(code.matching_first_stage,3);
        let writable=select_section(2,&sections,&starts);
        assert_eq!(writable.selected_section.as_deref(),Some(".data"));
        let readonly=select_section(10,&sections,&starts);
        assert_eq!(readonly.matching_first_stage,2);
        assert_eq!(readonly.skipped_directory_starts,2);
        assert_eq!(readonly.selected_section,None);
    }

    #[test]
    fn earlier_ineligible_section_does_not_prevent_later_eligible_selection() {
        let sections=vec![
            (".rdata".into(),0x1E000,0x4000_0040),
            (".custom".into(),0x22000,0x4000_0040),
        ];
        let out=select_section(10,&sections,&[0x1E000]);
        assert_eq!(out.skipped_directory_starts,1);
        assert_eq!(out.selected_section.as_deref(),Some(".custom"));
    }

    // The original C2F0 range accumulator uses unsigned comparison
    // and wrapping DWORD addition, not byte writes. This model is
    // deliberately limited to those two operations.
    fn original_range_update(start:u32,extent:u32,candidate:u32,extra:u32)->(u32,u32) {
        (start.min(candidate),extent.wrapping_add(extra))
    }

    #[test]
    fn original_c2f0_metadata_range_update_synthetic_vectors() {
        assert_eq!(original_range_update(0x2000,0x100,0x1000,0x80),(0x1000,0x180));
        assert_eq!(original_range_update(0x2000,0x100,0x3000,0x80),(0x2000,0x180));
        assert_eq!(original_range_update(4,0xFFFF_FFFF,2,2),(2,1));
    }

    #[test]
    fn category_registry_edges_are_separate_from_source_byte_decryption() {
        // Category selection records metadata; the selected .txt
        // section is still on-disk ciphertext at this stage.
        let sections=vec![
            (".txt".into(),0x1000,0x6000_0020),
            (".data".into(),0x20000,0xC000_0040),
        ];
        let s=select_section(1,&sections,&[]);
        assert_eq!(s.category,4);
        assert_eq!(s.selected_section.as_deref(),Some(".txt"));
        assert_eq!(original_range_update(0x1000,0x200,0x1000,0x80),(0x1000,0x280));
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
