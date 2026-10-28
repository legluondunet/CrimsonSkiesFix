// Original SafeDisc DLL memory I/O: static-only reconstruction of
// A620 (read), A720 (write) and A870 (dynamic Windows API resolver).
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The only private input is the already-SHA256-verified original
// dplayerx.dll in the owner's local process memory. No Windows APIs
// are invoked, no module memory is patched, and no code is decrypted.

use super::{
    Pe32, Result, dll_backed_rva_offset, dll_expect_raw_x86,
    dll_relative_call_target, read_u32,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum MemoryIoRoute {
    Unsupported,
    ProcessApi,
    MainModuleMemory,
}

pub fn read_route(mode:u32)->MemoryIoRoute {
    match mode {
        0x66=>MemoryIoRoute::ProcessApi,
        0x67=>MemoryIoRoute::MainModuleMemory,
        _=>MemoryIoRoute::Unsupported,
    }
}

pub fn write_route(mode:u32)->MemoryIoRoute {
    read_route(mode)
}
 
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum DescriptorIoRoute {
    Rejected,
    FileIo,
    ProcessIo,
    ModuleIo,
}

pub fn wrapper_route(mode:u32)->DescriptorIoRoute {
    match mode {
        0x65=>DescriptorIoRoute::FileIo,
        0x66=>DescriptorIoRoute::ProcessIo,
        0x67=>DescriptorIoRoute::ModuleIo,
        _=>DescriptorIoRoute::Rejected,
    }
}


#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ShadowWindow {
    pub offset:u32,
    pub available:u32,
}

/// The original A620/A720 mode-0x67 path only *considers* an
/// alternate backing when a module record and its +0x3c pointer
/// exist, the signed offset is nonnegative, and offset < +0x40.
///
/// This reports bounds for a synthetic guarded model, NOT a
/// promise that the x86 always copies the requested full size.
pub fn shadow_window(
    mode:u32,record_present:bool,shadow_ptr_present:bool,
    shadow_length:u32,offset:u32,
)->Option<ShadowWindow>{
    if mode!=0x67 || !record_present || !shadow_ptr_present ||
       (offset as i32)<0 || offset>=shadow_length {return None;}
    Some(ShadowWindow{offset,available:shadow_length-offset})
}

/// A970's exact first-byte passthrough and chained XOR/sub-1
/// decoding, used by A870 to obtain Kernel32 + three APIs.
pub fn decode_original_api_label(raw:&[u8])->Option<String> {
    let (&first,rest)=raw.split_first()?;
    if first==0 || !first.is_ascii_graphic(){return None;}
    let mut decoded=vec![first];
    let mut previous=first;
    for &encrypted in rest.iter().take(96) {
        if encrypted==0 {return String::from_utf8(decoded).ok();}
        let plain=encrypted.wrapping_sub(1)^previous;
        if !plain.is_ascii_graphic() {return None;}
        decoded.push(plain);
        previous=plain;
    }
    None
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ApiSlot {
    pub name:String,
    pub encoded_rva:u32,
    pub resolved_slot_rva:u32,
}

pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<Vec<ApiSlot>> {
    // A810 / AA00 are the public read/write descriptor wrappers.
    // They dispatch mode 0x65 to file I/O, 0x66/0x67 to process
    // and module memory handlers, and mode 0x64 to an early exit.
    // One original .txt2 E8 call supplies a clear-code path to AA00.
    for (site,target) in [
        (0xA84Eu32,0xA620u32),
        (0xA858,0xA560),
        (0xAA3E,0xA720),
        (0xAA48,0xA5C0),
        (0x19448,0xAA00),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!("SafeDisc original descriptor wrapper call at 0x{site:05X} changed"));
        }
    }
    for (site,bytes) in [
        (0xA815u32,&[0x83,0x7E,0x08,0x64][..]),
        (0xA841,&[0x83,0xF8,0x65][..]),
        (0xAA05,&[0x83,0x7E,0x08,0x64][..]),
        (0xAA31,&[0x83,0xF8,0x65][..]),
    ]{
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    // The init routine obtains the APIs by taking a DLL handle from
    // a chain-decoded name then resolving three chain-decoded labels.
    for (site,target) in [
        (0xA8FDu32,0xA930u32), // copy+decode library name
        (0xA958,0xA970),       // chained label transform
        (0xA89E,0xA9B0),
        (0xA8B7,0xA9B0),
        (0xA8D0,0xA9B0),
        (0xA9C2,0xA930),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!("SafeDisc encoded API resolver CALL at {site:05X} does not match the pinned original"));
        }
    }
    for (site,expected) in [
        (0xA8F6u32,&[0x33,0xFF][..]),
        (0xA970,&[0x8B,0x4C,0x24,0x08][..]),
        (0xA97A,&[0x8A,0x01,0x33,0xFF][..]),
        (0xA994,&[0xFE,0xCA,0x32,0xC2][..]),
        (0xA9D2,&[0xFF,0x15,0x10,0xE0,0x8F,0x00][..]),
        (0xA8A6,&[0xA3,0xE0,0x4A,0x90,0x00][..]),
        (0xA8BF,&[0xA3,0xDC,0x4A,0x90,0x00][..]),
        (0xA8D8,&[0xA3,0xD8,0x4A,0x90,0x00][..]),
        // A620 read: compare 0x67/0x66, registry fallback,
        // shadow [+0x3c] [+0x40], REP MOVS or dynamically
        // resolved ReadProcessMemory.
        // B540 initializes the ENTIRE fresh 0xD0-byte record
        // to zero, including shadow +0x3c and extent +0x40.
        (0xB573,&[0xB9,0x34,0x00,0x00,0x00][..]),
        (0xB578,&[0x33,0xC0][..]),
        (0xB57C,&[0xF3,0xAB][..]),
        // A200 cleanup releases optional shadow state.
        (0xA253,&[0x8B,0x46,0x3C][..]),
        (0xA25A,&[0x8B,0x56,0x40][..]),
        (0xA26C,&[0xC7,0x46,0x3C,0x00,0x00,0x00,0x00][..]),
        (0xA629,&[0x83,0xF8,0x67][..]),
        (0xA62E,&[0x83,0xF8,0x66][..]),
        (0xA63F,&[0xE8,0x8C,0x0F,0x00,0x00][..]),
        (0xA64F,&[0x8B,0x78,0x3C][..]),
        (0xA65A,&[0x8B,0x40,0x40][..]),
        (0xA64B,&[0x85,0xC0,0x74,0x38][..]),
        (0xA687,&[0x8B,0x76,0x04][..]),
        (0xA677,&[0xF3,0xA5][..]),
        (0xA682,&[0xF3,0xA4][..]),
        (0xA69D,&[0xF3,0xA5][..]),
        (0xA6A4,&[0xF3,0xA4][..]),
        (0xA6C3,&[0xFF,0x15,0xE0,0x4A,0x90,0x00][..]),
        // A720 write: mode-dependent memory operation, optional
        // shadow copy, original protection change and restore.
        (0xA72C,&[0x83,0xF9,0x67][..]),
        (0xA731,&[0x83,0xF9,0x66][..]),
        (0xA74D,&[0xE8,0x7E,0x0E,0x00,0x00][..]),
        (0xA75D,&[0x8B,0x50,0x3C][..]),
        (0xA768,&[0x8B,0x40,0x40][..]),
        (0xA759,&[0x85,0xC0,0x74,0x2C][..]),
        (0xA780,&[0xF3,0xA5][..]),
        (0xA787,&[0xF3,0xA4][..]),
        (0xA792,&[0xFF,0x15,0xD8,0x4A,0x90,0x00][..]),
        (0xA7A9,&[0xF3,0xA5][..]),
        (0xA7B5,&[0xF3,0xA4][..]),
        (0xA7BE,&[0xFF,0x15,0xD8,0x4A,0x90,0x00][..]),
        (0xA7E4,&[0xFF,0x15,0xDC,0x4A,0x90,0x00][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,expected)?;
    }

    let image_base=read_u32(dll,read_u32(dll,0x3c)? as usize+24+28)?;
    if image_base!=0x008E_0000 {
        return Err("unexpected SafeDisc DLL image base for encoded API references".into());
    }

    // Check the original imported function identity, not merely that
    // the DLL makes FF15 calls at these addresses.
    let original_imports=super::dll_import_bindings(dll,pe)?;
    for (site,expected_slot,expected_symbol) in [
        (0xA90Cu32,0x1E050u32,"LoadLibraryA"),
        (0xA9D2,0x1E010,"GetProcAddress"),
    ] {
        let (_,iat_slot)=super::dll_indirect_import_call(dll,pe,site,image_base)?;
        if iat_slot!=expected_slot {
            return Err(format!("original A870 imported DLL loader/resolver slot mismatch at 0x{site:05X}"));
        }
        let actual=original_imports.iter().find(|item|item.iat_rva==iat_slot)
            .ok_or_else(||"missing real PE import for A870 API resolver".to_string())?;
        if !actual.library.eq_ignore_ascii_case("KERNEL32.dll") || actual.symbol!=expected_symbol {
            return Err(format!("A870 imported API name mismatch at original RVA 0x{site:05X}"));
        }
    }

    let all=[
        ("Kernel32.dll",0x204B8u32,0u32),
        ("ReadProcessMemory",0x204C8,0x24AE0),
        ("WriteProcessMemory",0x204E0,0x24ADC),
        ("VirtualProtect",0x204F8,0x24AD8),
    ];
    let mut audit=Vec::new();
    for (name,label_rva,slot_rva) in all {
        let start=dll_backed_rva_offset(pe,label_rva,96)?;
        let bytes=dll.get(start..start+96)
            .ok_or_else(||"truncated original SafeDisc encoded API label".to_string())?;
        if decode_original_api_label(bytes).as_deref()!=Some(name) {
            return Err(format!("SafeDisc dynamically resolved API name mismatch at 0x{label_rva:05X}"));
        }
        if slot_rva!=0 {
            audit.push(ApiSlot{
                name:name.to_string(),encoded_rva:label_rva,
                resolved_slot_rva:slot_rva,
            });
        }
    }
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_label(s:&str)->Vec<u8>{
        let mut out=Vec::new();
        let mut previous=0u8;
        for (i,byte) in s.bytes().enumerate(){
            let encoded=if i==0 {byte} else {((byte^previous).wrapping_add(1))};
            out.push(encoded);
            previous=byte;
        }
        out.push(0);
        out
    }

    #[test]
    fn chained_api_name_codec_roundtrips_without_windows() {
        for label in ["Kernel32.dll","ReadProcessMemory",
                      "WriteProcessMemory","VirtualProtect"] {
            assert_eq!(decode_original_api_label(&encode_label(label)).as_deref(),Some(label));
        }
    }

    #[test]
    fn corrupted_strings_cannot_be_called_api_names(){
        assert_eq!(decode_original_api_label(&[]),None);
        assert_eq!(decode_original_api_label(&[0]),None);
        assert_eq!(decode_original_api_label(&[0xFF,0]),None);
        assert_eq!(decode_original_api_label(&[b'A',0xFF]),None);
    }

    #[test]
    fn wrapper_rejects_mode_64_and_selects_all_three_supported_modes() {
        assert_eq!(wrapper_route(0x64),DescriptorIoRoute::Rejected);
        assert_eq!(wrapper_route(0x65),DescriptorIoRoute::FileIo);
        assert_eq!(wrapper_route(0x66),DescriptorIoRoute::ProcessIo);
        assert_eq!(wrapper_route(0x67),DescriptorIoRoute::ModuleIo);
        assert_eq!(wrapper_route(0x68),DescriptorIoRoute::Rejected);
    }

    #[test]
    fn mode_66_uses_process_io_67_uses_module_memory() {
        assert_eq!(read_route(0x66),MemoryIoRoute::ProcessApi);
        assert_eq!(write_route(0x66),MemoryIoRoute::ProcessApi);
        assert_eq!(read_route(0x67),MemoryIoRoute::MainModuleMemory);
        assert_eq!(write_route(0x67),MemoryIoRoute::MainModuleMemory);
        for mode in [0,0x64,0x65,0x68,0xFFFF_FFFF] {
            assert_eq!(read_route(mode),MemoryIoRoute::Unsupported);
        }
    }

    #[test]
    fn shadow_window_requires_registered_nonempty_bounded_offset() {
        assert_eq!(shadow_window(0x67,true,true,100,99),
            Some(ShadowWindow{offset:99,available:1}));
        assert_eq!(shadow_window(0x67,true,true,100,0),
            Some(ShadowWindow{offset:0,available:100}));
        assert_eq!(shadow_window(0x67,true,true,100,100),None);
        assert_eq!(shadow_window(0x67,true,true,0,0),None);
        assert_eq!(shadow_window(0x67,false,true,100,0),None);
        assert_eq!(shadow_window(0x67,true,false,100,0),None);
        assert_eq!(shadow_window(0x66,true,true,100,0),None);
    }

    #[test]
    fn freshly_zeroed_record_has_no_alternate_backing() {
        let initial_record=[0u8;0xD0];
        let shadow_ptr=u32::from_le_bytes(initial_record[0x3C..0x40].try_into().unwrap());
        let shadow_size=u32::from_le_bytes(initial_record[0x40..0x44].try_into().unwrap());
        assert_eq!(shadow_ptr,0);
        assert_eq!(shadow_size,0);
        assert_eq!(shadow_window(0x67,true,shadow_ptr!=0,shadow_size,0),None);
        // The original B540 initializer also copies descriptor fields
        // and creates auxiliary category lists; this test establishes
        // only the *initial* absence of a shadow buffer.
    }

    #[test]
    fn signed_offsets_reject_high_bit_and_wrapping_indices() {
        assert_eq!(shadow_window(0x67,true,true,u32::MAX,0x8000_0000),None);
        assert_eq!(shadow_window(0x67,true,true,u32::MAX,u32::MAX),None);
        assert_eq!(shadow_window(0x67,true,true,u32::MAX,0x7FFF_FFFE),
            Some(ShadowWindow{offset:0x7FFF_FFFE,available:0x8000_0001}));
    }
}
