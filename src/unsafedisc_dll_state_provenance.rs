// Original SafeDisc DLL byte-mixer state provenance (read-only).
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The global 32-bit state at preferred VA 0x00905010 starts in
// PE .data zero-filled virtual memory, but other reachable or
// potentially reachable routines write it. The static entries
// here are NOT a proven temporal execution trace.

use super::{
    Pe32, Result, read_u32, dll_expect_raw_x86, dll_relative_call_target,
    dll_indirect_import_call, dll_import_bindings,
};

pub const STATE_PREFERRED_VA:u32=0x0090_5010;
pub const STATE_RVA:u32=0x0002_5010;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum StateOperation {
    ConditionalAdd4f3,
    AddArgumentLow16,
    AddOutputByte,
    AddStackLocal,
    ResetToZero,
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct StateWrite {
    pub instruction_rva:u32,
    pub operation:StateOperation,
}

// Explicit, instruction-aligned writes to the state DWORD in original
// readable .txt2. They do NOT include pointer-mediated mutations via
// helper routines such as A77F, so this is not an exhaustive list of
// effects on the global state.
pub const VERIFIED_DIRECT_WRITES:[StateWrite;7]=[
    StateWrite{instruction_rva:0x198A0,operation:StateOperation::ConditionalAdd4f3},
    StateWrite{instruction_rva:0x19A8B,operation:StateOperation::AddArgumentLow16},
    StateWrite{instruction_rva:0x19DE1,operation:StateOperation::AddOutputByte},
    StateWrite{instruction_rva:0x19E79,operation:StateOperation::AddArgumentLow16},
    StateWrite{instruction_rva:0x19F18,operation:StateOperation::AddStackLocal},
    StateWrite{instruction_rva:0x1A360,operation:StateOperation::ResetToZero},
    StateWrite{instruction_rva:0x1A3C0,operation:StateOperation::ResetToZero},
];

/// A PE32 section is zero-filled at offset >= SizeOfRawData only up
/// to VirtualSize. This models initial image mapping, not the value
/// at a later byte-mixer call (other functions can have changed it).
pub fn virtual_zero_filled(section_rva:u32,raw_len:u32,virtual_size:u32,rva:u32)->bool {
    rva.checked_sub(section_rva).is_some_and(|delta|
        delta>=raw_len && delta<virtual_size)
}

/// x86 NEG followed by SBB EAX,EAX turns the carry flag into
/// either 0 or -1; AND 0x4f3 selects a conditional increment.
pub fn conditional_4f3_delta(original:u32)->u32 {
    if original==0 {0} else {0x4F3}
}

/// Two different original routines add EBP+0x18 masked to 16 bits
/// to the shared state, without knowing those argument values.
pub fn increment_low16(state:u32,argument:u32)->u32 {
    state.wrapping_add(argument & 0xFFFF)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WindowsVersionPath {
    QueryFailure,
    Win32Nt,
    OtherPlatform,
}

/// Historical OSVERSIONINFOA::dwPlatformId is at struct offset
/// 0x10; value 2 is VER_PLATFORM_WIN32_NT.
pub fn get_version_path(api_success:bool,platform_id:u32)->WindowsVersionPath {
    if !api_success {WindowsVersionPath::QueryFailure}
    else if platform_id==2 {WindowsVersionPath::Win32Nt}
    else {WindowsVersionPath::OtherPlatform}
}

/// Audit only original file-backed instruction sites and PE geometry.
/// The caller enforces the canonical dplayerx.dll SHA-256 first.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    if base!=0x008E_0000 || base+STATE_RVA!=STATE_PREFERRED_VA {
        return Err("original DLL state RVA/image base changed".into());
    }
    let data=pe.section(b".data")?;
    if data.virtual_address!=0x20000 || data.raw_len!=0x3E00 ||
       data.virtual_size!=0x6C80 ||
       !virtual_zero_filled(data.virtual_address,data.raw_len as u32,
                            data.virtual_size,STATE_RVA) {
        return Err("original DLL byte mixer state not in expected PE .data zero-fill".into());
    }

    // The exact original state writer instruction spans:
    // conditional 0x4F3, two low16 argument additions,
    // the per-output-byte add, an extra local add and two resets.
    for (at,bytes) in [
        (0x1988Fu32,&[0xF7,0xD8,0x1B,0xC0,0x25,0xF3,0x04,0x00,0x00][..]),
        (0x19898,&[0x8B,0x15,0x10,0x50,0x90,0x00,0x03,0xD0][..]),
        (0x198A0,&[0x89,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19A7B,&[0x8B,0x45,0x18,0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x19A83,&[0x8B,0x0D,0x10,0x50,0x90,0x00,0x03,0xC8][..]),
        (0x19A8B,&[0x89,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19DE1,&[0x89,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19E69,&[0x8B,0x45,0x18,0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x19E71,&[0x8B,0x0D,0x10,0x50,0x90,0x00,0x03,0xC8][..]),
        (0x19E79,&[0x89,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19F10,&[0xA1,0x10,0x50,0x90,0x00,0x03,0x45,0xFC][..]),
        (0x19F18,&[0xA3,0x10,0x50,0x90,0x00][..]),
        (0x1A360,&[0xC7,0x05,0x10,0x50,0x90,0x00,0,0,0,0][..]),
        (0x1A3C0,&[0xC7,0x05,0x10,0x50,0x90,0x00,0,0,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }

    // Two site-specific direct calls pass POINTERS to the same global
    // state to A77F. A77F may modify it; these are not ordinary
    // local DWORD assignments and must not be included in the
    // seven explicitly validated direct writes.
    for (push_at,call_at) in [
        (0x19A48u32,0x19A4Du32),
        (0x19E2B,0x19E30),
    ] {
        dll_expect_raw_x86(dll,pe,push_at,&[0x68,0x10,0x50,0x90,0x00])?;
        if dll_relative_call_target(dll,pe,call_at)?!=0xA77F {
            return Err(format!("A77F state-pointer call mismatch at 0x{call_at:05X}"));
        }
    }

    // A77F builds OSVERSIONINFOA (148 bytes), calls the *actual*
    // original KERNEL32!GetVersionExA import, and branches on
    // OSVERSIONINFOA::dwPlatformId at +0x10. Platform ID 2
    // dispatches down the Win32 NT branch; other IDs down another.
    for (at,bytes) in [
        (0xA79Fu32,&[0xC7,0x85,0x6C,0xFF,0xFF,0xFF,0x94,0,0,0][..]),
        (0xA7A9,&[0x8D,0x85,0x6C,0xFF,0xFF,0xFF,0x50][..]),
        (0xA7B0,&[0xFF,0x15,0x58,0xE0,0x8F,0x00][..]),
        (0xA7B6,&[0x85,0xC0,0x0F,0x84,0x82,0,0,0][..]),
        (0xA80B,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x02,0x75,0x2C][..]),
        (0xA824,&[0x8B,0x55,0x08,0x52][..]),
        (0xA7EF,&[0x8B,0x4D,0x08,0x51][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }
    for (at,target) in [
        (0xA828u32,0xAE3Fu32),
        (0xA7F3,0xBEEE),
    ] {
        if dll_relative_call_target(dll,pe,at)?!=target {
            return Err(format!("A77F platform-specific helper call mismatch at 0x{at:05X}"));
        }
    }
    let (_,iat_slot)=dll_indirect_import_call(dll,pe,0xA7B0,base)?;
    if iat_slot!=0x1E058 {
        return Err("original A77F platform-detection import slot changed".into());
    }
    let imports=dll_import_bindings(dll,pe)?;
    if !imports.iter().any(|imp|imp.iat_rva==iat_slot &&
        imp.symbol=="GetVersionExA" &&
        imp.library.eq_ignore_ascii_case("KERNEL32.dll")) {
        return Err("original A77F GetVersionExA symbol verification failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_state_is_in_unbacked_virtual_data() {
        assert!(virtual_zero_filled(0x20000,0x3E00,0x6C80,STATE_RVA));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x23DFF));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x26C80));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x1FFFF));
    }

    #[test]
    fn conditional_4f3_x86_carry_gate_is_binary() {
        assert_eq!(conditional_4f3_delta(0),0);
        for x in [1u32,0x100,0x8000_0000,u32::MAX]{
            assert_eq!(conditional_4f3_delta(x),0x4F3);
        }
    }

    #[test]
    fn two_low16_additions_ignore_high_argument_bits_and_wrap() {
        assert_eq!(increment_low16(42,0xABCD_1001),42+0x1001);
        assert_eq!(increment_low16(42,0x0000_1001),42+0x1001);
        assert_eq!(increment_low16(u32::MAX,1),0);
    }

    #[test]
    fn exact_state_write_sites_and_categories_are_unique() {
        let sites=VERIFIED_DIRECT_WRITES.map(|w|w.instruction_rva);
        let set:std::collections::HashSet<_>=sites.into_iter().collect();
        assert_eq!(set.len(),7);
        assert_eq!(VERIFIED_DIRECT_WRITES.iter()
                   .filter(|w|w.operation==StateOperation::ResetToZero).count(),2);
        assert_eq!(VERIFIED_DIRECT_WRITES.iter()
                   .filter(|w|w.operation==StateOperation::AddArgumentLow16).count(),2);
        assert!(VERIFIED_DIRECT_WRITES.windows(2).all(|p|
            p[0].instruction_rva<p[1].instruction_rva));
    }

    #[test]
    fn nt_branch_is_distinguished_from_os_version_query_failure() {
        assert_eq!(get_version_path(false,2),WindowsVersionPath::QueryFailure);
        assert_eq!(get_version_path(true,2),WindowsVersionPath::Win32Nt);
        for platform in [0,1,3,4,u32::MAX] {
            assert_eq!(get_version_path(true,platform),WindowsVersionPath::OtherPlatform);
        }
    }

    #[test]
    fn a77f_version_structure_has_correct_platform_field_offset() {
        // OSVERSIONINFOA: dwOSVersionInfoSize at -0x94, dwPlatformId
        // at -0x84, a relative distance of 16 bytes.
        assert_eq!(0x94u32-0x84,0x10);
        assert_eq!(0x94,148);
    }
}
