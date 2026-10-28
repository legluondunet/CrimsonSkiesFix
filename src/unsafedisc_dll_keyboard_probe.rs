// Original-byte-anchored, read-only keyboard environment gate research.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Static source evidence only; original DLL code is never executed.
// Neither an executed INT instruction nor a decoder is claimed.

use super::{
    Pe32, Result, dll_expect_raw_x86, dll_relative_call_target,
    dll_indirect_import_call, dll_import_bindings, read_u32,
};

pub const GATE_RVA:u32=0x1D42F;
pub const CALLSITE_RVA:u32=0x1CD46;


/// If 1CD31 is entered, JL is taken when SF!=OF; otherwise the
/// fallthrough has only two NOPs and complementary JGE at 1CD39
/// takes the same callsite. No flags are changed between the Jccs.
pub fn conditional_predecessor_target(sf:bool,of:bool)->u32 {
    if sf!=of {CALLSITE_RVA}
    else {CALLSITE_RVA}
}

/// Original helper 1D42F returns AX=1 only if GetKeyboardType(0)
/// returns 7 and GetKeyboardType(1) returns 0xD01..=0xD07.
/// Purely synthetic evaluation; does not call USER32.
pub fn keyboard_gate(kind:u32,subtype:u32)->bool {
    kind==7 && (0xD01..=0xD07).contains(&subtype)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ConditionalRoute {
    BypassInt68,
    EnterInt68,
}

/// If execution arrives at 1CD46 and the call returns, a nonzero
/// EAX&0xFFFF takes JNE 1CD52 past the static INT 68h instruction.
/// Reachability from the DLL's entrypoint is NOT established.
pub fn conditional_route(gate:bool)->ConditionalRoute {
    if gate {ConditionalRoute::BypassInt68}
    else {ConditionalRoute::EnterInt68}
}


/// Conditional local (not global) DWORD update on the INT 68h path:
/// original 1CD63 compares the returned EAX with the input AX=4300h.
/// Only a DIFFERENT return sets the [EBP-0x98] local flag to one.
/// Neither the actual interrupt return nor branch reachability is known.
pub fn local_interrupt_probe_transition(prior:u32,returned_eax:u32)->u32 {
    if returned_eax==0x4300 {prior} else {1}
}

/// Caller first checks exact original DLL SHA-256 and PE32 format.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x1D42Fu32,&[0x55,0x8B,0xEC,0x51,0x6A,0x00][..]),
        (0x1D43B,&[0x83,0xF8,0x07,0x75,0x50,0x6A,0x01][..]),
        (0x1D448,&[0x89,0x45,0xFC][..]),
        (0x1D48A,&[0x66,0xB8,0x01,0x00,0xEB,0x03][..]),
        (0x1D490,&[0x66,0x33,0xC0][..]),
        (0x1D493,&[0x8B,0xE5,0x5D,0xC3][..]),
        (0x1C7FC,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0,0,0,0][..]),
        (0x1CD31,&[0x0F,0x8C,0x0F,0,0,0,0x90,0x90,0x7D,0x0B][..]),
        (0x1CD4B,&[0x25,0xFF,0xFF,0x00,0x00,0x85,0xC0,0x75,0x27][..]),
        (0x1CD54,&[0x60,0x33,0xC0,0x66,0xB8,0x00,0x43][..]),
        (0x1CD5B,&[0xCD,0x68][..]),
        (0x1CD5D,&[0x89,0x85,0x64,0xFF,0xFF,0xFF][..]),
        (0x1CD63,&[0x3D,0x00,0x43,0x00,0x00,0x74,0x0B][..]),
        (0x1CD6A,&[0xB8,0x01,0x00,0x00,0x00][..]),
        (0x1CD6F,&[0x89,0x85,0x68,0xFF,0xFF,0xFF][..]),
        (0x1CD75,&[0x61,0xE9,0xD4,0x00,0x00,0x00][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if dll_relative_call_target(dll,pe,CALLSITE_RVA)?!=GATE_RVA {
        return Err("original keyboard gate direct CALL target changed".into());
    }
    // Seven contiguous comparisons on the same local DWORD; the first
    // six JE to AX=1 and the last JNE to AX=0.
    for (i,subtype) in (1u8..=7).enumerate() {
        let cmp=0x1D44Bu32+9*i as u32;
        dll_expect_raw_x86(dll,pe,cmp,
            &[0x81,0x7D,0xFC,subtype,0x0D,0,0])?;
        let branch=cmp+7;
        let code=if subtype<7 {[0x74,0x36-9*(i as u8)]}
                 else {[0x75,0x06]};
        dll_expect_raw_x86(dll,pe,branch,&code)?;
        let target=branch+2+u32::from(code[1]);
        let expected=if subtype<7 {0x1D48A} else {0x1D490};
        if target!=expected {
            return Err("original keyboard subtype branch target changed".into());
        }
    }
    // Verify actual PE import binding, not a guessed FF15 target.
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    let imports=dll_import_bindings(dll,pe)?;
    for call in [0x1D435u32,0x1D442] {
        let (_,slot)=dll_indirect_import_call(dll,pe,call,base)?;
        if slot!=0x1E110 || !imports.iter().any(|imp|
            imp.iat_rva==slot && imp.symbol=="GetKeyboardType" &&
            imp.library.eq_ignore_ascii_case("USER32.dll")) {
            return Err(format!("keyboard gate IAT identity mismatch at 0x{call:05X}"));
        }
    }
    for (actual,want) in [
        (0x1CD31u32+6+0x0F,0x1CD46),
        (0x1CD39+2+0x0B,0x1CD46),
        (0x1D43Eu32+2+0x50,0x1D490),
        (0x1CD52+2+0x27,0x1CD7B),
        (0x1CD76+5+0xD4,0x1CE4F),
    ] {
        if actual!=want {return Err("keyboard/int68 branch displacement changed".into());}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_accepts_exactly_seven_subtypes_for_type_seven() {
        for n in 0u32..=0xE00 {
            assert_eq!(keyboard_gate(7,n),(0xD01..=0xD07).contains(&n));
        }
        for kind in [0,1,4,6,8,u32::MAX] {
            for subtype in [0,0xD01,0xD04,0xD07,0xD08] {
                assert!(!keyboard_gate(kind,subtype));
            }
        }
    }
    #[test]
    fn complementary_signed_condition_codes_join_at_original_callsite() {
        for sf in [false,true] {
            for of in [false,true] {
                assert_eq!(conditional_predecessor_target(sf,of),CALLSITE_RVA);
                let jl=sf!=of;
                let jge=sf==of;
                assert_ne!(jl,jge);
            }
        }
    }

    #[test]
    fn zero_return_enters_candidate_interrupt_branch() {
        assert_eq!(conditional_route(true),ConditionalRoute::BypassInt68);
        assert_eq!(conditional_route(false),ConditionalRoute::EnterInt68);
    }
    #[test]
    fn interrupt_return_comparison_changes_only_an_abstract_local_flag() {
        for old in [0,1,42,u32::MAX] {
            assert_eq!(local_interrupt_probe_transition(old,0x4300),old);
        }
        for ret in [0,1,0x42FF,0x4301,0xFFFF_FFFF] {
            assert_eq!(local_interrupt_probe_transition(0,ret),1);
        }
    }

    #[test]
    fn key_sites_are_backed_by_original_txt2_geometry() {
        assert!(CALLSITE_RVA<GATE_RVA);
        for site in [CALLSITE_RVA,GATE_RVA,0x1D435,0x1D442] {
            assert!((0x18000..0x1D497).contains(&site));
        }
    }
}
