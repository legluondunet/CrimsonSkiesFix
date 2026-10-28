// Original SafeDisc IO backends and encoded API names, research only.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Do not execute the original DLL, invoke Windows APIs or read process
// memory. Parent inspector verifies SHA-256 of retail dplayerx.dll.

use super::{
    Pe32,Result,read_u32,dll_backed_rva_offset,dll_expect_raw_x86,
    dll_relative_call_target,dll_indirect_import_call,dll_import_bindings,
};

pub const ENCODED_NAME_RVAS:[u32;4]=[
    0x204B8,0x204C8,0x204E0,0x204F8,
];
pub const EXPECTED_API_NAMES:[&str;4]=[
    "Kernel32.dll","ReadProcessMemory",
    "WriteProcessMemory","VirtualProtect",
];

/// Faithful pure byte-level model of the ORIGINAL A970 decoder:
/// first byte is literal, subsequent output byte is previous
/// decoded output XOR (next encoded byte - 1), wrapping at 8 bits.
/// Input may contain exactly one terminating NUL; output is ASCII.
pub fn decode_original_api_name(encoded:&[u8])->Result<String> {
    let mut out=Vec::new();
    let mut terminated=false;
    for (index,&byte) in encoded.iter().enumerate() {
        if byte==0 {
            if index==0 || index+1!=encoded.len() {
                return Err("invalid original encoded API NUL terminator".into());
            }
            terminated=true;
            break;
        }
        if out.len()>=48 {
            return Err("original encoded API name longer than limit".into());
        }
        let decoded=if let Some(&previous)=out.last() {
            previous ^ byte.wrapping_sub(1)
        }else{byte};
        if !decoded.is_ascii_graphic() {
            return Err("decoded original API name is nonprintable".into());
        }
        out.push(decoded);
    }
    if !terminated {
        return Err("original encoded API name has no NUL terminator".into());
    }
    String::from_utf8(out).map_err(|_|"original decoded API name is non-UTF8".into())
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum OriginalIoBackend {
    Unsupported,
    FileRead,
    ProcessMemoryRead,
    InProcessMemoryCopy,
}

/// A810 checks type 0x64 before the initialization gate. If
/// initialization is accepted, type 0x65 uses A560 and all other
/// types use A620. A620 accepts ONLY 0x66 and 0x67.
pub fn backend_for_source_type(source_type:u32)->OriginalIoBackend {
    match source_type {
        0x65=>OriginalIoBackend::FileRead,
        0x66=>OriginalIoBackend::ProcessMemoryRead,
        0x67=>OriginalIoBackend::InProcessMemoryCopy,
        _=>OriginalIoBackend::Unsupported,
    }
}

/// A560 requires SetFilePointer's DWORD to differ from 0xFFFFFFFF;
/// ReadFile's returned AX to be nonzero; and lpNumberOfBytesRead
/// to equal the requested length. No Win32 function is invoked.
/// Note that A560 treats a 0xFFFFFFFF file offset as failure
/// without an additional GetLastError check.
pub fn file_read_accepted(
    set_file_pointer_return:u32,
    read_file_ax:u16,
    bytes_read:u32,
    requested:u32,
)->bool {
    set_file_pointer_return!=u32::MAX
        && read_file_ax!=0
        && bytes_read==requested
}

/// A620 type 0x66 invokes a dynamically resolved
/// Kernel32!ReadProcessMemory callback and accepts only nonzero
/// returned EAX plus exact returned bytes-read count.
pub fn process_read_accepted(
    read_process_memory_return:u32,
    bytes_read:u32,
    requested:u32,
)->bool {
    read_process_memory_return!=0 && bytes_read==requested
}

/// Local A620 type 0x67 source selection. The first branch
/// calls B5D0; when it yields a nonnull memory map, nonnull
/// mapped data at +0x3C, signed-nonnegative offset < +0x40,
/// the original code copies from mapped data (clamping only
/// the count to min(requested, total_size), NOT to size-offset).
/// Otherwise it attempts a memcpy from [source+4]+offset.
/// This MUST NOT be interpreted as a memory safety guarantee.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Type67CopySource {
    MappedData {count:u32},
    DirectSource {count:u32},
}
pub fn type67_copy_source(
    mapped_descriptor_present:bool,
    mapped_data_present:bool,
    signed_offset:i32,
    map_total_size:u32,
    requested:u32,
)->Type67CopySource {
    if mapped_descriptor_present && mapped_data_present &&
       signed_offset>=0 && (signed_offset as u32)<map_total_size {
        Type67CopySource::MappedData {
            count:requested.min(map_total_size)
        }
    }else{
        Type67CopySource::DirectSource{count:requested}
    }
}

/// Original A870 caches dynamically resolved Kernel32 functions
/// in three distinct PREFERRED VA .data slots. These slots are
/// virtual zero-fill in the retail image (no raw bytes at the
/// slot itself), not static PE IAT bindings.
pub const DYNAMIC_API_SLOTS:[(u32,&str);3]=[
    (0x00904AE0,"ReadProcessMemory"),
    (0x00904ADC,"WriteProcessMemory"),
    (0x00904AD8,"VirtualProtect"),
];

fn verified_original_encoded_name(
    dll:&[u8],pe:&Pe32,rva:u32,
)->Result<String> {
    let mut encoded=Vec::new();
    for i in 0..=48u32 {
        let position=rva.checked_add(i)
            .ok_or("encoded original API name RVA overflow")?;
        let at=dll_backed_rva_offset(pe,position,1)?;
        let byte=*dll.get(at).ok_or("truncated original API name")?;
        encoded.push(byte);
        if byte==0 {break;}
    }
    decode_original_api_name(&encoded)
}

/// All assertions are based on genuine, ORIGINAL, exact-hash
/// user-provided dplayerx.dll bytes. The original DLL's functions
/// are NEVER executed, and no protected .txt or .icd bytes are read.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    if base!=0x008E0000 {
        return Err("original SafeDisc IO DLL image base changed".into());
    }
    for (rva,name) in ENCODED_NAME_RVAS.into_iter()
        .zip(EXPECTED_API_NAMES)
    {
        let decoded=verified_original_encoded_name(dll,pe,rva)?;
        if decoded!=name {
            return Err(format!(
                "original XOR API name at 0x{rva:05X} changed: {decoded}"
            ));
        }
    }
    // Original A970 cumulative-XOR decode loop; A870 resolves three
    // names, writing their resulting pointers to virtual globals.
    for (site,bytes) in [
        (0xA970u32,&[0x8B,0x4C,0x24,0x08,0x55,0x8B,0x6C,0x24,0x08,
                       0x57,0x8A,0x01,0x33,0xFF,0x88,0x45,0][..]),
        (0xA98C,&[0x8A,0x51,0x01,0x8A,0x04,0x0E,0x47,0x41,
                   0xFE,0xCA,0x32,0xC2,0x88,0x04,0x0E][..]),
        (0xA885,&[0x68,0xB8,0x04,0x90,0x00,0xE8,0x61,0,0,0][..]),
        (0xA898,&[0x68,0xC8,0x04,0x90,0x00,0x56,0xE8,0x0D,0x01,0,0][..]),
        (0xA8A6,&[0xA3,0xE0,0x4A,0x90,0x00][..]),
        (0xA8B1,&[0x68,0xE0,0x04,0x90,0x00,0x56,0xE8,0xF4,0,0,0][..]),
        (0xA8BF,&[0xA3,0xDC,0x4A,0x90,0x00][..]),
        (0xA8CA,&[0x68,0xF8,0x04,0x90,0x00,0x56,0xE8,0xDB,0,0,0][..]),
        (0xA8D8,&[0xA3,0xD8,0x4A,0x90,0x00][..]),
        (0xA8E3,&[0x66,0x89,0x3D,0xE4,0x4A,0x90,0x00][..]),
        // A560 type-65 seek then ReadFile full-count test.
        (0xA560,&[0x56,0x8B,0x74,0x24,0x08,0x57,0x83,0x7E,0x08,0x65,0x75,0x43][..]),
        (0xA572,&[0x6A,0,0x6A,0,0x50,0x51,0xFF,0x15,0x44,0xE0,0x8F,0][..]),
        (0xA57E,&[0x83,0xF8,0xFF,0x74,0x2C][..]),
        (0xA591,&[0x6A,0,0x52,0x57,0x50,0x51,0xFF,0x15,0x40,0xE0,0x8F,0][..]),
        (0xA59D,&[0x66,0x85,0xC0,0x74,0x0D][..]),
        (0xA5A2,&[0x39,0x7C,0x24,0x0C,0x75,0x07][..]),
        (0xA5A8,&[0x66,0xB8,0x01,0,0x5F,0x5E,0xC3][..]),
        (0xA5AF,&[0x5F,0x66,0x33,0xC0,0x5E,0xC3][..]),
        // A620 type-66 dynamic process read or type-67 memcpy.
        (0xA620,&[0x56,0x8B,0x74,0x24,0x08,0x57,0x8B,0x46,0x08,
                  0x83,0xF8,0x67,0x74,0x10,0x83,0xF8,0x66,
                  0x74,0x06,0x66,0x33,0xC0,0x5F,0x5E,0xC3][..]),
        (0xA639,&[0x83,0xF8,0x67,0x75,0x6B][..]),
        (0xA63E,&[0x56,0xE8,0x8C,0x0F,0,0][..]),
        (0xA64F,&[0x8B,0x78,0x3C,0x85,0xFF,0x74,0x31][..]),
        (0xA656,&[0x85,0xD2,0x7C,0x2D][..]),
        (0xA65A,&[0x8B,0x40,0x40,0x3B,0xD0,0x7D,0x26][..]),
        (0xA66B,&[0x8B,0xC8,0x8D,0x34,0x3A,0x8B,0x7C,0x24,0x18,
                  0xC1,0xE9,0x02,0xF3,0xA5][..]),
        (0xA679,&[0x8B,0xC8,0x66,0xB8,0x01,0,0x83,0xE1,0x03,0xF3,0xA4][..]),
        (0xA687,&[0x8B,0x76,0x04,0x8B,0x4C,0x24,0x14,
                   0x8B,0x7C,0x24,0x18,0x03,0xF2][..]),
        (0xA69A,&[0xC1,0xE9,0x02,0xF3,0xA5][..]),
        (0xA6A9,&[0x8B,0x7C,0x24,0x14,0x8B,0x4C,0x24,0x18,
                   0x8B,0x56,0x04][..]),
        (0xA6B8,&[0x50,0x8B,0x06,0x57,0x51,0x03,0x54,0x24,0x1C,
                   0x52,0x50,0xFF,0x15,0xE0,0x4A,0x90,0][..]),
        (0xA6C9,&[0x8B,0x4C,0x24,0x0C,0x85,0xC0,0x74,0x04,
                   0x3B,0xCF,0x74,0x3C][..]),
        (0xA711,&[0x5F,0x66,0xB8,0x01,0,0x5E,0xC3][..]),
        (0xA7FE,&[0x5F,0x5E,0x5D,0x66,0x33,0xC0,0x5B,0x59,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for (site,target) in [
        (0xA88Au32,0xA8F0),(0xA89E,0xA9B0),
        (0xA8B7,0xA9B0),(0xA8D0,0xA9B0),
        (0xA8FD,0xA930),(0xA9C2,0xA930),
        (0xA63F,0xB5D0),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!(
                "original IO backend rel32 CALL at 0x{site:05X} changed"
            ));
        }
    }
    let bindings=dll_import_bindings(dll,pe)?;
    for (site,iat,symbol) in [
        (0xA578u32,0x1E044u32,"SetFilePointer"),
        (0xA597,0x1E040,"ReadFile"),
        (0xA6DD,0x1E04C,"GetLastError"),
        (0xA6F1,0x1E04C,"GetLastError"),
        (0xA705,0x1E04C,"GetLastError"),
        (0xA90C,0x1E050,"LoadLibraryA"),
        (0xA9D2,0x1E010,"GetProcAddress"),
    ] {
        let (_,slot)=dll_indirect_import_call(dll,pe,site,base)?;
        if slot!=iat || !bindings.iter().any(|entry|
            entry.iat_rva==slot && entry.symbol==symbol &&
            entry.library.eq_ignore_ascii_case("KERNEL32.dll")
        ) {
            return Err(format!(
                "original 0x{site:05X} IO IAT identity changed: {symbol}"
            ));
        }
    }
    // Indirect through the three dynamically populated slots:
    // source is 0xA870's original A3 global pointers, not an IAT.
    for (slot,loaded_name) in DYNAMIC_API_SLOTS {
        if slot<base || slot>=base+0x26C80 ||
           !EXPECTED_API_NAMES.contains(&loaded_name) {
            return Err("original dynamic IO API pointer identity changed".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_like_original(name:&str)->Vec<u8> {
        let bytes=name.as_bytes();
        let mut encoded=Vec::new();
        for (i,&byte) in bytes.iter().enumerate() {
            encoded.push(if i==0 {byte} else {
                (bytes[i-1]^byte).wrapping_add(1)
            });
        }
        encoded.push(0);
        encoded
    }

    #[test]
    fn cumulative_xor_name_decoder_roundtrips_original_four_apis() {
        for name in EXPECTED_API_NAMES {
            let encoded=encode_like_original(name);
            assert_eq!(decode_original_api_name(&encoded).unwrap(),name);
        }
    }
    #[test]
    fn encoded_name_decoder_rejects_missing_terminator_and_nongraphic() {
        assert!(decode_original_api_name(b"Kernel32.dll").is_err());
        assert!(decode_original_api_name(b"\0").is_err());
        assert!(decode_original_api_name(b"X\0Y").is_err());
    }
    #[test]
    fn original_io_backend_is_selected_by_type_value() {
        for (kind,route) in [
            (0x64,OriginalIoBackend::Unsupported),
            (0x65,OriginalIoBackend::FileRead),
            (0x66,OriginalIoBackend::ProcessMemoryRead),
            (0x67,OriginalIoBackend::InProcessMemoryCopy),
            (0x68,OriginalIoBackend::Unsupported),
            (u32::MAX,OriginalIoBackend::Unsupported),
        ] {
            assert_eq!(backend_for_source_type(kind),route);
        }
    }
    #[test]
    fn file_read_requires_seek_read_success_and_complete_byte_count() {
        assert!(file_read_accepted(0,1,40,40));
        assert!(file_read_accepted(0xFF_FFFF,2,0,0));
        assert!(!file_read_accepted(u32::MAX,1,40,40));
        assert!(!file_read_accepted(0,0,40,40));
        assert!(!file_read_accepted(0,1,39,40));
    }
    #[test]
    fn process_read_requires_nonzero_eax_and_exact_byte_count() {
        assert!(process_read_accepted(1,40,40));
        assert!(process_read_accepted(u32::MAX,0,0));
        assert!(!process_read_accepted(0,40,40));
        assert!(!process_read_accepted(1,39,40));
    }
    #[test]
    fn memory_source_67_uses_fallback_on_invalid_map_conditions() {
        assert_eq!(type67_copy_source(false,true,0,40,12),
            Type67CopySource::DirectSource{count:12});
        assert_eq!(type67_copy_source(true,false,0,40,12),
            Type67CopySource::DirectSource{count:12});
        assert_eq!(type67_copy_source(true,true,-1,40,12),
            Type67CopySource::DirectSource{count:12});
        assert_eq!(type67_copy_source(true,true,40,40,12),
            Type67CopySource::DirectSource{count:12});
        assert_eq!(type67_copy_source(true,true,39,40,12),
            Type67CopySource::MappedData{count:12});
        // Original code does not subtract offset from capacity
        // when selecting the mapped-data copy count.
        assert_eq!(type67_copy_source(true,true,39,40,100),
            Type67CopySource::MappedData{count:40});
    }
}
