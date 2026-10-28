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


/// Only the x86 flags read by the eight pairs of complementary
/// Jcc instructions between 1CD16 and the keyboard-gate CALL.
/// This is a bounded, synthetic static control-flow model, not
/// instruction execution or a real Windows execution trace.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct GatePredecessorFlags {
    pub cf:bool, pub zf:bool, pub pf:bool, pub sf:bool, pub of:bool,
}

/// Follow one source-level path with immutable flags. Every visited
/// non-branch opcode is an audited NOP or register self-MOV.
/// The model intentionally begins AT 1CD16, not at the DLL entrypoint.
pub fn trace_gate_predecessor(flags:GatePredecessorFlags)->Result<Vec<u32>> {
    let mut pc=0x1CD16u32;
    let mut visited=Vec::new();
    for _ in 0..24 {
        visited.push(pc);
        if pc==CALLSITE_RVA {return Ok(visited);}
        pc=match pc {
            0x1CD16 => if flags.cf || flags.zf {0x1CD29} else {0x1CD18},
            0x1CD18 => if !flags.cf && !flags.zf {0x1CD29} else {
                return Err("JBE/JA flag complement violated".into());
            },
            0x1CD29 => if flags.sf {0x1CD19} else {0x1CD2B},
            0x1CD2B => if !flags.sf {0x1CD19} else {
                return Err("JS/JNS flag complement violated".into());
            },
            0x1CD19 => if !flags.pf {0x1CD2E} else {0x1CD1F},
            0x1CD1F => 0x1CD21, // MOV EDX,EDX
            0x1CD21 => 0x1CD22, // NOP
            0x1CD22 => 0x1CD23, // NOP
            0x1CD23 => if !flags.cf && !flags.zf {0x1CD2E} else {0x1CD25},
            0x1CD25 => if flags.pf {0x1CD2E} else {
                return Err("JP reached with PF=0 after JNP".into());
            },
            0x1CD2E => if !flags.pf {0x1CD41} else {0x1CD30},
            0x1CD30 => if flags.pf {0x1CD41} else {
                return Err("JNP/JP flag complement violated".into());
            },
            0x1CD41 => if flags.of {0x1CD31} else {0x1CD43},
            0x1CD43 => if !flags.of {0x1CD31} else {
                return Err("JO/JNO flag complement violated".into());
            },
            0x1CD31 => if flags.sf!=flags.of {CALLSITE_RVA} else {0x1CD37},
            0x1CD37 => 0x1CD38, // NOP
            0x1CD38 => 0x1CD39, // NOP
            0x1CD39 => if flags.sf==flags.of {CALLSITE_RVA} else {
                return Err("JL/JGE flag complement violated".into());
            },
            _ => return Err(format!(
                "unknown executable instruction boundary in 1CD16 predecessor trace: 0x{pc:05X}"
            )),
        };
    }
    Err("gate predecessor trace did not converge in 24 steps".into())
}

/// Original instruction bytes behind the bounded trace, excluding
/// unreachable overlapping junk opcode interpretations. These are
/// compared only after the SHA-256 verification at the CLI entry.
pub fn audit_gate_predecessor_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,bytes) in [
        (0x1CD16u32,&[0x76,0x11,0x77,0x0F][..]),
        (0x1CD19,&[0x0F,0x8B,0x0F,0x00,0x00,0x00][..]),
        (0x1CD1F,&[0x8B,0xD2,0x90,0x90][..]),
        (0x1CD23,&[0x77,0x09,0x7A,0x07][..]),
        (0x1CD29,&[0x78,0xEE,0x79,0xEC][..]),
        (0x1CD2E,&[0x7B,0x11,0x7A,0x0F][..]),
        (0x1CD31,&[0x0F,0x8C,0x0F,0x00,0x00,0x00,0x90,0x90,0x7D,0x0B][..]),
        (0x1CD41,&[0x70,0xEE,0x71,0xEC][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
    }
    // Independently of the synthetic trace, every signed relative
    // displacement in its original Jcc pairs resolves to the
    // previously established source-level instruction boundaries.
    for (from,len,delta,target) in [
        (0x1CD16i64,2i64,0x11i64,0x1CD29i64),
        (0x1CD18,2,0x0F,0x1CD29),
        (0x1CD19,6,0x0F,0x1CD2E),
        (0x1CD23,2,0x09,0x1CD2E),
        (0x1CD25,2,0x07,0x1CD2E),
        (0x1CD29,2,-18,0x1CD19),
        (0x1CD2B,2,-20,0x1CD19),
        (0x1CD2E,2,0x11,0x1CD41),
        (0x1CD30,2,0x0F,0x1CD41),
        (0x1CD41,2,-18,0x1CD31),
        (0x1CD43,2,-20,0x1CD31),
        (0x1CD31,6,0x0F,0x1CD46),
        (0x1CD39,2,0x0B,0x1CD46),
    ] {
        if from+len+delta!=target {
            return Err("original keyboard predecessor displacement invalid".into());
        }
    }
    Ok(())
}


/// If 1CD31 is entered, JL is taken when SF!=OF; otherwise the
/// fallthrough has only two NOPs and complementary JGE at 1CD39
/// takes the same callsite. No flags are changed between the Jccs.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum PredecessorArm { SignedLess, SignedGreaterEqual }

pub fn conditional_predecessor_arm(sf:bool,of:bool)->PredecessorArm {
    if sf!=of {PredecessorArm::SignedLess}
    else {PredecessorArm::SignedGreaterEqual}
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
    audit_gate_predecessor_original(dll,pe)?;
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
    fn all_32_flag_combinations_join_keyboard_gate_from_1cd16() {
        let mut shortest=usize::MAX;
        let mut longest=0usize;
        let mut variants=std::collections::HashSet::new();
        for bits in 0u8..32 {
            let f=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let path=trace_gate_predecessor(f).unwrap();
            assert_eq!(path.first(),Some(&0x1CD16));
            assert_eq!(path.last(),Some(&CALLSITE_RVA));
            assert!(!path.contains(&0x1CD5B));
            assert!(!path.contains(&0x1CD27)); // unreachable junk path
            assert!(!path.contains(&0x1CD45)); // unreachable overlapping x86
            shortest=shortest.min(path.len());
            longest=longest.max(path.len());
            variants.insert(path);
        }
        assert_eq!(shortest,8);
        assert_eq!(longest,18);
        assert_eq!(variants.len(),16);
    }

    #[test]
    fn keyboard_gate_predecessor_convergence_is_independent_of_eflags() {
        for cf in [false,true] {
            for zf in [false,true] {
                for pf in [false,true] {
                    for sf in [false,true] {
                        for of in [false,true] {
                            let flags=GatePredecessorFlags {cf,zf,pf,sf,of};
                            let trace=trace_gate_predecessor(flags).unwrap();
                            assert_eq!(trace.last(),Some(&CALLSITE_RVA));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn complementary_signed_condition_codes_join_at_original_callsite() {
        for sf in [false,true] {
            for of in [false,true] {
                let arm=conditional_predecessor_arm(sf,of);
                assert_eq!(arm,if sf!=of {
                    PredecessorArm::SignedLess
                }else{
                    PredecessorArm::SignedGreaterEqual
                });
                assert_ne!(sf!=of,sf==of);
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
