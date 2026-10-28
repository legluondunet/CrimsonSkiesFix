// Original SafeDisc DLL finalizer control-flow, read-only inspection.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// All results are conditional on entry to the original 19800 block,
// a normal return from the genuine Win9x helper and the original
// unsigned local counter comparison. No original DLL code is run.

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target};
use super::unsafedisc_dll_keyboard_probe::{
    GatePredecessorFlags,original_eflags_step_in_range,
    KeyboardResult,NestedVersionResult,conditional_original_state_route,
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ConditionalFinalizerOutcome {
    pub state_after_win9x_helper:u32,
    pub win9x_helper_ax:u16,
    pub state_after_caller_add:u32,
    pub caller_add_delta:u32,
    pub helper_wrote_dword:bool,
}

/// The original caller masks EAX with FFFFh; NEG/SBB/AND computes
/// 0 or 0x4F3 from its low 16 bits. This is NOT a new TEA key.
pub fn caller_delta_from_ax(ax:u16)->u32 {
    if ax==0 {0} else {0x4F3}
}

/// Source-level hypothetical composition. Conditions:
/// 1. execution has reached RVA 1987D through its preceding block;
/// 2. 19882 invokes the original 1A9B6 helper with the global DWORD;
/// 3. 1A9B6 selects Win9x (outer GetVersionExA reports platform 1);
/// 4. any taken INT 68h returns normally, and its EAX is supplied;
/// 5. original module global state memory is usable.
/// The helper's original AX return is preserved by 1A9B6 as audited.
/// Other outer API branches and entrypoint reachability are NOT modeled.
pub fn conditional_win9x_finalizer(
    outer_api_success:bool,
    outer_platform_id:u32,
    keyboard:KeyboardResult,
    nested:NestedVersionResult,
    initial_state:u32,
)->Option<ConditionalFinalizerOutcome> {
    let result=conditional_original_state_route(
        outer_api_success,outer_platform_id,keyboard,nested,initial_state
    )?;
    let state_after_helper=result.pointed_value?;
    let delta=caller_delta_from_ax(result.ax);
    Some(ConditionalFinalizerOutcome{
        state_after_win9x_helper:state_after_helper,
        win9x_helper_ax:result.ax,
        state_after_caller_add:state_after_helper.wrapping_add(delta),
        caller_add_delta:delta,
        helper_wrote_dword:result.wrote_dword,
    })
}

/// Original safe, flag-preserving maze only; the caller counter
/// comparison at RVA 19689 is handled separately. Never interpret
/// this as evidence that the DLL's DllMain reaches this block.
pub fn trace_original_finalizer_maze(
    dll:&[u8],pe:&Pe32,start:u32,end:u32,flags:GatePredecessorFlags
)->Result<Vec<u32>> {
    let (minimum,maximum)=match (start,end) {
        (0x19800,0x19686)=>(0x19686,0x19800),
        (0x19819,0x1987D)=>(0x19819,0x1987D),
        _=>return Err("unsupported original finalizer maze bounds".into()),
    };
    let mut pc=start;
    let mut path=Vec::new();
    for _ in 0..100 {
        if pc==end {
            path.push(pc);
            return Ok(path);
        }
        if path.contains(&pc) {
            return Err(format!("original finalizer maze loops at 0x{pc:05X}"));
        }
        path.push(pc);
        pc=original_eflags_step_in_range(
            dll,pe,pc,flags,minimum,maximum
        )?;
    }
    Err("original finalizer maze exceeds 100 instruction boundaries".into())
}

/// Audit original instruction boundaries and conditional caller
/// behavior against an already SHA-256 verified, user-owned DLL.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,expected) in [
        (0x19686u32,&[0x8B,0x55,0xF4,0x3B,0x55,0xEC,0x0F,0x83,0x87,0x01,0,0][..]),
        (0x1987D,&[0x68,0x10,0x50,0x90,0][..]),
        (0x19887,&[
            0x83,0xC4,0x04, // ADD ESP,4
            0x25,0xFF,0xFF,0,0, // AND EAX,0xFFFF
            0xF7,0xD8, // NEG EAX
            0x1B,0xC0, // SBB EAX,EAX
            0x25,0xF3,0x04,0,0, // AND EAX,0x4F3
            0x8B,0x15,0x10,0x50,0x90,0, // MOV EDX,[global]
            0x03,0xD0, // ADD EDX,EAX
            0x89,0x15,0x10,0x50,0x90,0, // MOV [global],EDX
        ][..]),
        // The Win9x 1A9B6 wrapper saves AX in local, then restores
        // the low 16 bits before its common epilogue. Its apparent
        // conditional call at 1AAB0 targets an original bare RET.
        (0x1AAA3,&[0x83,0xC4,0x04][..]),
        (0x1AAA6,&[0x66,0x89,0x85,0x68,0xFF,0xFF,0xFF][..]),
        (0x1AAAD,&[0x75,0x0B,0x52][..]),
        (0x1AAB5,&[0x5A,0x74,0x02,0xC3][..]),
        (0x1AABA,&[0x66,0x8B,0x85,0x68,0xFF,0xFF,0xFF][..]),
        (0x1AAC1,&[0xE9,0xB1,0,0,0][..]),
        (0x1AB75,&[0xC3][..]),
        (0x1AB77,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,expected)?;
    }
    for (source,length,displacement,want) in [
        (0x1968Cu32,6u32,0x187u32,0x19819u32),
        (0x1AAAD,2,0x0B,0x1AABA),
        (0x1AAB6,2,0x02,0x1AABA),
        (0x1AAC1,5,0xB1,0x1AB77),
    ] {
        if source+length+displacement!=want {
            return Err("original finalizer branch displacement changed".into());
        }
    }
    if dll_relative_call_target(dll,pe,0x19882)?!=0x1A9B6 ||
       dll_relative_call_target(dll,pe,0x1AA9E)?!=0x1C7F0 ||
       dll_relative_call_target(dll,pe,0x1AAB0)?!=0x1AB75 {
        return Err("original Win9x finalizer CALL target changed".into());
    }
    for (start,end,want_unique,want_min,want_max) in [
        (0x19800u32,0x19686u32,5usize,8usize,12usize),
        (0x19819,0x1987D,16,31,41),
    ] {
        let mut unique=std::collections::HashSet::new();
        let mut min=usize::MAX;
        let mut max=0usize;
        for bits in 0u8..32 {
            let flags=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let path=trace_original_finalizer_maze(dll,pe,start,end,flags)?;
            min=min.min(path.len());max=max.max(path.len());
            unique.insert(path);
        }
        if unique.len()!=want_unique || min!=want_min || max!=want_max {
            return Err(format!(
                "original finalizer maze 0x{start:05X} -> 0x{end:05X}: \
                 {} distinct paths, {min}..{max} boundaries",
                unique.len()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonzero_ax_always_produces_exact_caller_4f3_delta() {
        assert_eq!(caller_delta_from_ax(0),0);
        for ax in [1,2,0x8000,0xFFFF] {
            assert_eq!(caller_delta_from_ax(ax),0x4F3);
        }
    }
    #[test]
    fn normal_rejected_keyboard_int68_return_has_no_state_effect() {
        let r=conditional_win9x_finalizer(
            true,1,KeyboardResult::RejectedInterruptReturned{eax:0x4300},
            NestedVersionResult::Succeeded{platform_id:1},0xBEEF
        ).unwrap();
        assert_eq!((r.state_after_win9x_helper,r.win9x_helper_ax,
                    r.state_after_caller_add,r.caller_add_delta,r.helper_wrote_dword),
                   (0xBEEF,0,0xBEEF,0,false));
    }
    #[test]
    fn suspicious_interrupt_return_has_two_conditional_additions() {
        for eax in [0,0x4301,0xFFFF4300,u32::MAX] {
            for initial in [0u32,1,0xFFFF_FFFF,0xFFFF_F800] {
                let r=conditional_win9x_finalizer(
                    true,1,KeyboardResult::RejectedInterruptReturned{eax},
                    NestedVersionResult::Succeeded{platform_id:1},initial
                ).unwrap();
                assert_eq!(r.state_after_win9x_helper,initial.wrapping_add(0x34F));
                assert_eq!(r.win9x_helper_ax,1);
                assert_eq!(r.caller_add_delta,0x4F3);
                assert_eq!(r.state_after_caller_add,initial.wrapping_add(0x842));
                assert!(r.helper_wrote_dword);
            }
        }
    }
    #[test]
    fn accepted_keyboard_and_mismatched_nested_platform_are_conditional() {
        let accepted_matching=conditional_win9x_finalizer(
            true,1,KeyboardResult::Accepted,
            NestedVersionResult::Succeeded{platform_id:1},21
        ).unwrap();
        assert_eq!(accepted_matching.state_after_caller_add,21);
        assert_eq!(accepted_matching.win9x_helper_ax,0);
        let nested_other=conditional_win9x_finalizer(
            true,1,KeyboardResult::Accepted,
            NestedVersionResult::Succeeded{platform_id:2},21
        ).unwrap();
        assert_eq!(nested_other.state_after_caller_add,21+0x842);
        assert_eq!(nested_other.win9x_helper_ax,1);
        assert!(nested_other.helper_wrote_dword);
    }
    #[test]
    fn non_win9x_outer_query_is_not_speculatively_reconstructed() {
        for platform in [0,1,2,3,u32::MAX] {
            for success in [false,true] {
                let r=conditional_win9x_finalizer(
                    success,platform,KeyboardResult::Accepted,
                    NestedVersionResult::Succeeded{platform_id:2},0x1234
                );
                assert_eq!(r.is_some(),success && platform==1);
            }
        }
    }
}
