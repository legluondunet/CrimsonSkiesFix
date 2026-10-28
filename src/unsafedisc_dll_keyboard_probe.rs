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


/// EFLAGS-only branch evaluation for the exact x86 Jcc conditions.
/// The original Win9x helper's prologue SUB ESP,0x9C changes flags,
/// but these five values do not need to be inferred: the caller
/// exhaustively enumerates ALL 32 possible combinations.
pub fn branch_from_eflags(condition:u8,f:GatePredecessorFlags)->Result<bool> {
    Ok(match condition {
        0 => f.of,
        1 => !f.of,
        2 => f.cf,
        3 => !f.cf,
        4 => f.zf,
        5 => !f.zf,
        6 => f.cf || f.zf,
        7 => !f.cf && !f.zf,
        8 => f.sf,
        9 => !f.sf,
        10 => f.pf,
        11 => !f.pf,
        12 => f.sf != f.of,
        13 => f.sf == f.of,
        14 => f.zf || f.sf != f.of,
        15 => !f.zf && f.sf == f.of,
        _ => return Err(format!("not an original x86 Jcc condition: {condition}")),
    })
}

/// Decode a single safe instruction from the SHA-pinned ORIGINAL DLL
/// at a known instruction boundary. Only x86 Jcc rel8/rel32, JMP
/// rel8/rel32, NOP and register SELF-MOV/SELF-XCHG are recognized.
/// Any flag mutation, memory access, call or unfamiliar opcode fails
/// closed. This is STATIC flow analysis, NEVER native DLL execution.
pub fn original_eflags_step_in_range(dll:&[u8],pe:&Pe32,pc:u32,f:GatePredecessorFlags,minimum:u32,maximum:u32)->Result<u32> {
    let idx=super::dll_backed_rva_offset(pe,pc,1)?;
    let opcode=dll[idx];
    let (size,displacement)=if (0x70..=0x7F).contains(&opcode) {
        let off=super::dll_backed_rva_offset(pe,pc,2)?;
        let delta=if branch_from_eflags(opcode & 15,f)? {
            dll[off+1] as i8 as i64
        } else {0};
        (2i64,delta)
    } else if opcode==0x0F {
        let off=super::dll_backed_rva_offset(pe,pc,6)?;
        let second=dll[off+1];
        if !(0x80..=0x8F).contains(&second) {
            return Err(format!("non-Jcc 0F opcode at 0x{pc:05X}"));
        }
        let delta=if branch_from_eflags(second & 15,f)? {
            i32::from_le_bytes(dll[off+2..off+6].try_into()
                .map_err(|_|"truncated Jcc rel32")?) as i64
        }else{0};
        (6,delta)
    } else if opcode==0xEB {
        let off=super::dll_backed_rva_offset(pe,pc,2)?;
        (2,dll[off+1] as i8 as i64)
    } else if opcode==0xE9 {
        let off=super::dll_backed_rva_offset(pe,pc,5)?;
        (5,i32::from_le_bytes(dll[off+1..off+5].try_into()
            .map_err(|_|"truncated JMP rel32")?) as i64)
    } else if opcode==0x90 {
        (1,0)
    } else if matches!(opcode,0x8B|0x89|0x87) {
        let off=super::dll_backed_rva_offset(pe,pc,2)?;
        let modrm=dll[off+1];
        if modrm>>6!=3 || ((modrm>>3)&7)!=(modrm&7) {
            return Err(format!("non-self MOV/XCHG at 0x{pc:05X}"));
        }
        (2,0)
    } else {
        return Err(format!("unexpected active instruction at 0x{pc:05X}, opcode 0x{opcode:02X}"));
    };
    let next=i64::from(pc)+size+displacement;
    let next=u32::try_from(next)
        .map_err(|_|format!("x86 branch outside 32-bit RVA at 0x{pc:05X}"))?;
    if next<minimum || next>maximum {
        return Err(format!("Win9x branch exits vetted block: 0x{pc:05X} -> 0x{next:05X}"));
    }
    Ok(next)
}

pub fn original_eflags_step(dll:&[u8],pe:&Pe32,pc:u32,f:GatePredecessorFlags)->Result<u32> {
    original_eflags_step_in_range(dll,pe,pc,f,0x1C806,CALLSITE_RVA)
}

/// Walk a source-only flag-preserving branch segment between two
/// known original instruction boundaries. Reject side effects and
/// cycles, *including the expected import CALL at the destination*.
/// All original instructions are read as data, never executed.
pub fn trace_original_flag_segment(
    dll:&[u8],pe:&Pe32,start:u32,target:u32,f:GatePredecessorFlags
)->Result<Vec<u32>> {
    if !(0x1C806..=0x1D425).contains(&start) || target<=start ||
       target>0x1D425 {
        return Err("Win9x flag-only segment boundaries are invalid".into());
    }
    let mut pc=start;
    let mut path=Vec::new();
    for _ in 0..512 {
        if pc==target {
            path.push(pc);
            return Ok(path);
        }
        if path.contains(&pc) {
            return Err(format!("Win9x flag-only segment loops at 0x{pc:05X}"));
        }
        path.push(pc);
        // The original tail segment starting at 1D2C7 has backward
        // obfuscated Jcc edges into 1D2B7. These are finite,
        // flag-preserving branches (not necessarily cycles).
        let allowed_min=if start==0x1D2C7 {0x1D2B7} else {start};
        pc=original_eflags_step_in_range(dll,pe,pc,f,allowed_min,target)?;
    }
    Err("Win9x flag-only segment did not converge within 512 boundaries".into())
}

/// The gate-success branch, when it returns AX=1, avoids INT 68h
/// and crosses three more flag-only mazes. The intermediate original
/// instructions initialize stack locals; the first following API
/// call is the real GetVersionExA import at original RVA 1D058.
/// No result of that API or subsequent DWORD state is inferred.
pub fn audit_win9x_gate_success_to_version_api(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,expected) in [
        (0x1CD46u32,&[0xE8,0xE4,0x06,0,0,0x25,0xFF,0xFF,0,0,0x85,0xC0,0x75,0x27][..]),
        (0x1CE3B,&[0xC7,0x85,0x64,0xFF,0xFF,0xFF,0,0x43,0,0][..]),
        (0x1CE45,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0,0,0,0][..]),
        (0x1CFFF,&[0xC7,0x85,0x6C,0xFF,0xFF,0xFF,0x94,0,0,0][..]),
        (0x1D051,&[0x8D,0x85,0x6C,0xFF,0xFF,0xFF,0x50][..]),
        (0x1D058,&[0xFF,0x15,0x58,0xE0,0x8F,0][..]),
        (0x1D05E,&[0x85,0xC0,0x0F,0x84,0x37,0x01,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,expected)?;
    }
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    let (_,slot)=dll_indirect_import_call(dll,pe,0x1D058,base)?;
    let imports=dll_import_bindings(dll,pe)?;
    if slot!=0x1E058 || !imports.iter().any(|imp|
        imp.iat_rva==slot && imp.symbol=="GetVersionExA" &&
        imp.library.eq_ignore_ascii_case("KERNEL32.dll")) {
        return Err("original Win9x gate-success GetVersionExA binding changed".into());
    }
    let stages=[
        (0x1CD7Bu32,0x1CE3Bu32,30usize,58usize),
        (0x1CE4F,0x1CFFF,68,125),
        (0x1D009,0x1D051,12,25),
    ];
    for (start,end,min_expected,max_expected) in stages {
        let mut distinct=std::collections::HashSet::new();
        let mut observed_min=usize::MAX;
        let mut observed_max=0usize;
        for bits in 0u8..32 {
            let flags=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let trace=trace_original_flag_segment(dll,pe,start,end,flags)?;
            observed_min=observed_min.min(trace.len());
            observed_max=observed_max.max(trace.len());
            distinct.insert(trace);
        }
        if distinct.len()!=32 || observed_min!=min_expected ||
           observed_max!=max_expected {
            return Err(format!(
                "original gate-success segment 0x{start:05X}..0x{end:05X} changed: \
                 {} paths {observed_min}..{observed_max}",distinct.len()
            ));
        }
    }
    Ok(())
}


/// Narrow keyboard-accepted path ONLY. A second GetVersionExA
/// call inside the Win9x helper decides whether the original
/// local-flag assignment [EBP-0x98]=15 can be reached.
/// On failure or dwPlatformId==1 the accepted-keyboard local is zero
/// and the original JBE at 1D1A4 bypasses the pointer-add guard.
/// On API success with dwPlatformId!=1 it is set to fifteen and
/// the path reaches 1D26A (subject to the pointer-null guard).
/// The keyboard-rejected/INT 68h outcome is NOT modeled here.
pub fn accepted_keyboard_nested_version_to_guard(
    nested_api_success:bool,platform_id:u32
)->bool {
    nested_api_success && platform_id!=1
}

/// Original-image proof of a conditional, not universal, path
/// through the four remaining flag-only obfuscation networks.
/// No OS API is called, no instruction from the DLL is executed.
pub fn audit_win9x_nested_version_to_pointer_guard(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,expected) in [
        (0x1D05Eu32,&[0x85,0xC0,0x0F,0x84,0x37,0x01,0,0][..]),
        (0x1D0C6,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x01][..]),
        (0x1D0CD,&[0x0F,0x84,0xCA,0,0,0][..]),
        (0x1D133,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0x0F,0,0,0][..]),
        (0x1D19D,&[0x83,0xBD,0x68,0xFF,0xFF,0xFF,0][..]),
        (0x1D1A4,&[0x0F,0x86,0x5B,0x01,0,0][..]),
        (0x1D26A,&[0x83,0x7D,0x08,0,0x0F,0x84,0x88,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,expected)?;
    }
    for (pc,size,delta,target) in [
        (0x1D060u32,6u32,0x137u32,0x1D19Du32), // API failure
        (0x1D0CD,6,0xCA,0x1D19D), // dwPlatformId==1
        (0x1D1A4,6,0x15B,0x1D305), // local flag ==0
        (0x1D26E,6,0x88,0x1D2FC), // passed pointer==NULL
    ] {
        if pc+size+delta!=target {
            return Err("original nested Win9x version-to-pointer guard branch changed".into());
        }
    }
    for (start,end,want_unique,want_min,want_max) in [
        (0x1D066u32,0x1D0C6u32,16usize,15usize,33usize),
        (0x1D0D3,0x1D133,32,16,28),
        (0x1D13D,0x1D19D,16,15,27),
        (0x1D1AA,0x1D26A,32,31,60),
    ] {
        let mut unique=std::collections::HashSet::new();
        let mut min=usize::MAX;
        let mut max=0usize;
        for bits in 0u8..32 {
            let f=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let trace=trace_original_flag_segment(dll,pe,start,end,f)?;
            min=min.min(trace.len()); max=max.max(trace.len());
            unique.insert(trace);
        }
        if unique.len()!=want_unique || min!=want_min || max!=want_max {
            return Err(format!(
                "original nested Win9x segment 0x{start:05X}..0x{end:05X} changed: \
                {} unique paths of {min}..{max} instruction sites",unique.len()
            ));
        }
    }
    Ok(())
}

/// Starting after the original 1C7F0 prologue, prove that the
/// keyboard gate at 1CD46 is reached using ONLY control flow and
/// flag-preserving instructions. All 32 EFLAGS assignments are checked
/// by audit_original; no API calls or protected x86 code are executed.
pub fn trace_original_win9x_entry_to_gate(
    dll:&[u8],pe:&Pe32,f:GatePredecessorFlags
)->Result<Vec<u32>> {
    let mut pc=0x1C806u32;
    let mut trace=Vec::new();
    for _ in 0..512 {
        if pc==CALLSITE_RVA {
            trace.push(pc);
            return Ok(trace);
        }
        if trace.contains(&pc) {
            return Err(format!("Win9x EFLAGS-only path loops at 0x{pc:05X}"));
        }
        trace.push(pc);
        pc=original_eflags_step(dll,pe,pc,f)?;
    }
    Err("Win9x entry path did not converge within 512 instruction boundaries".into())
}

/// Strong SHA-pinned local reachability result: the first side-effecting
/// instruction after helper prologue, on *every* flag assignment, is
/// its known keyboard helper CALL. Checks original function prologue
/// before the bounded symbolic walk, including the stack-local init.
pub fn audit_win9x_entry_to_keyboard_gate(dll:&[u8],pe:&Pe32)->Result<()> {
    dll_expect_raw_x86(dll,pe,0x1C7F0,&[
        0x55,0x8B,0xEC,0x81,0xEC,0x9C,0,0,0,0x53,0x56,0x57,
        0xC7,0x85,0x68,0xFF,0xFF,0xFF,0,0,0,0,
    ])?;
    dll_expect_raw_x86(dll,pe,0x1C806,&[0x7C,0x11,0x7D,0x0F])?;
    let mut paths=std::collections::HashSet::new();
    let mut min=usize::MAX;
    let mut max=0usize;
    for mask in 0u8..32 {
        let flags=GatePredecessorFlags {
            cf:mask&1!=0,zf:mask&2!=0,pf:mask&4!=0,
            sf:mask&8!=0,of:mask&16!=0,
        };
        let path=trace_original_win9x_entry_to_gate(dll,pe,flags)?;
        if path.first()!=Some(&0x1C806) || path.last()!=Some(&CALLSITE_RVA) {
            return Err("Win9x entry does not converge on keyboard helper".into());
        }
        min=min.min(path.len());max=max.max(path.len());
        paths.insert(path);
    }
    // Counts independently calculated by a separate read-only
    // Python walker against the exact retail DLL, not by x86 execution.
    if paths.len()!=32 || min!=210 || max!=395 {
        return Err(format!(
            "Win9x original keyboard predecessor topology changed: \
             {} paths of {min}..{max} boundaries",paths.len()
        ));
    }
    Ok(())
}


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

/// Conditional, observed event required. In particular, the
/// keyboard-rejected branch contains INT 68h. We do NOT simulate
/// or assume the interrupt succeeds, is callable on Wine, or returns.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum KeyboardResult {
    Accepted,
    /// This variant is meaningful ONLY if original INT 68h returned
    /// normally with an EAX result, leaving the caller's frame usable.
    RejectedInterruptReturned { eax:u32 },
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum NestedVersionResult {
    Failed,
    Succeeded { platform_id:u32 },
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum GuardRoute {
    BypassPointerGuard,
    ExaminePointerGuard,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct NestedGateState {
    /// Original [EBP-0x98] local at the CMP in 1D19D.
    pub local_flag:u32,
    pub route:GuardRoute,
}

/// A *conditional* model of the original source path, NOT a SafeDisc
/// decryption step or a Windows API emulator.
///
/// On entry to Win9x helper, local [EBP-0x98] is explicitly zero.
/// Accepted keyboard branch explicitly resets it to zero.
/// Rejected keyboard branch reaches INT 68h and, if the interrupt
/// returns normally, changes it to one iff full returned EAX != 4300h.
/// The subsequent nested GetVersionExA success with dwPlatformId!=1
/// overwrites it with fifteen. Otherwise it preserves the flag.
/// CMP local,0 / JBE 1D305 enters the pointer guard iff flag>0.
pub fn conditional_nested_pointer_route(
    keyboard:KeyboardResult,
    version:NestedVersionResult
)->NestedGateState {
    let from_keyboard=match keyboard {
        KeyboardResult::Accepted => 0,
        KeyboardResult::RejectedInterruptReturned{eax} =>
            if eax==0x4300 {0}else{1},
    };
    let local_flag=match version {
        NestedVersionResult::Succeeded{platform_id} if platform_id!=1 => 15,
        _ => from_keyboard,
    };
    NestedGateState {
        local_flag,
        route:if local_flag==0 {GuardRoute::BypassPointerGuard}
              else {GuardRoute::ExaminePointerGuard},
    }
}

/// Original-byte anchor for the interrupted path only. The
/// callsite-to-branch prefix, flag-only join at 1CE4F, and nested
/// GetVersionExA paths have their own independent audits.
pub fn audit_rejected_keyboard_interrupt_source(dll:&[u8],pe:&Pe32)->Result<()> {
    for (at,bytes) in [
        (0x1C7FCu32,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0,0,0,0][..]),
        (0x1CD4B,&[0x25,0xFF,0xFF,0,0,0x85,0xC0,0x75,0x27][..]),
        (0x1CD54,&[0x60,0x33,0xC0,0x66,0xB8,0x00,0x43][..]),
        (0x1CD5B,&[0xCD,0x68][..]),
        (0x1CD5D,&[0x89,0x85,0x64,0xFF,0xFF,0xFF][..]),
        (0x1CD63,&[0x3D,0x00,0x43,0,0,0x74,0x0B][..]),
        (0x1CD6A,&[0xB8,0x01,0,0,0][..]),
        (0x1CD6F,&[0x89,0x85,0x68,0xFF,0xFF,0xFF][..]),
        (0x1CD75,&[0x61,0xE9,0xD4,0,0,0][..]),
        (0x1CE45,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0,0,0,0][..]),
        (0x1D133,&[0xC7,0x85,0x68,0xFF,0xFF,0xFF,0x0F,0,0,0][..]),
        (0x1D19D,&[0x83,0xBD,0x68,0xFF,0xFF,0xFF,0,
                    0x0F,0x86,0x5B,0x01,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }
    for (at,len,offset,want) in [
        (0x1CD52i64,2i64,0x27i64,0x1CD7Bi64),
        (0x1CD68,2,0x0B,0x1CD75),
        (0x1CD76,5,0xD4,0x1CE4F),
        (0x1D1A4,6,0x15B,0x1D305),
    ] {
        if at+len+offset!=want {
            return Err("original keyboard rejected branch target changed".into());
        }
    }
    Ok(())
}

/// Caller-provided assumption: an argument is either null or a
/// valid writable DWORD with a known starting value. No original
/// pointer is dereferenced during this Rust conditional model.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Win9xPointer {
    Null,
    WritableDword(u32),
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ConditionalWin9xOutcome {
    /// The original MOV AX,1 / XOR AX,AX set ONLY low 16 bits.
    pub ax:u16,
    /// The value the caller supplied, perhaps incremented.
    pub pointed_value:Option<u32>,
    /// True only when original MOV [EAX],EDX was on this path.
    pub wrote_dword:bool,
}

/// Conditional composition of the audited source regions. This
/// presupposes entry to helper 1C7F0, observed kernel API outcomes
/// (and on rejection, a normally returning INT 68h), and valid
/// caller memory for any nonnull pointer. It is not DLL execution.
pub fn conditional_win9x_outcome(
    keyboard:KeyboardResult,
    version:NestedVersionResult,
    pointer:Win9xPointer
)->ConditionalWin9xOutcome {
    let gate=conditional_nested_pointer_route(keyboard,version);
    let before=match pointer {
        Win9xPointer::Null=>None,
        Win9xPointer::WritableDword(value)=>Some(value),
    };
    match (gate.route,pointer) {
        (GuardRoute::BypassPointerGuard,_) =>
            ConditionalWin9xOutcome{ax:0,pointed_value:before,wrote_dword:false},
        (GuardRoute::ExaminePointerGuard,Win9xPointer::Null) =>
            ConditionalWin9xOutcome{ax:1,pointed_value:None,wrote_dword:false},
        (GuardRoute::ExaminePointerGuard,Win9xPointer::WritableDword(value)) =>
            ConditionalWin9xOutcome {
                ax:1,
                pointed_value:Some(
                    super::unsafedisc_dll_state_provenance::
                        conditional_win9x_34f_step(value)
                ),
                wrote_dword:true,
            },
    }
}

/// Verify that BOTH branches after the local-flag CMP have only
/// original flag-preserving obfuscated flow to separate return-value
/// instructions. This does not predict API outcomes or full EAX.
pub fn audit_win9x_conditional_returns(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x1D19Du32,&[0x83,0xBD,0x68,0xFF,0xFF,0xFF,0,0x0F,0x86,0x5B,0x01,0,0][..]),
        (0x1D2FC,&[0x66,0xB8,0x01,0x00,0xE9,0x23,0x01,0,0][..]),
        (0x1D425,&[0x66,0x33,0xC0][..]),
        (0x1D428,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for (start,end,want_distinct,want_min,want_max) in [
        (0x1D2C7u32,0x1D2FCu32,16usize,10usize,21usize),
        (0x1D305,0x1D425,32,44,77),
    ] {
        let mut paths=std::collections::HashSet::new();
        let mut shortest=usize::MAX;
        let mut longest=0usize;
        for bits in 0u8..32 {
            let flags=GatePredecessorFlags{
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let path=trace_original_flag_segment(dll,pe,start,end,flags)?;
            shortest=shortest.min(path.len());
            longest=longest.max(path.len());
            paths.insert(path);
        }
        if paths.len()!=want_distinct ||
           shortest!=want_min || longest!=want_max {
            return Err(format!(
                "original Win9x return segment 0x{start:05X}..0x{end:05X} changed: \
                {} distinct paths, {shortest}..{longest} boundaries",paths.len()
            ));
        }
    }
    // Source verified: 1D2FC MOV AX,1; 1D300 JMP 1D428.
    // The 1D305-guard-bypass path reaches 1D425 XOR AX,AX,
    // followed by the same 1D428 function epilogue.
    if 0x1D300u32+5+0x123!=0x1D428 {
        return Err("original Win9x MOV AX,1 return jump changed".into());
    }
    Ok(())
}

/// The conditional global-state route begins only if the original
/// direct callsite at 1987D executes and outer GetVersionExA reports
/// platform 1, dispatching to 1C7F0. Other outer outcomes do NOT
/// use this modeled route. The pointer is the original 00905010h
/// state DWORD, *not* merely a scratch DWORD on the Win9x stack.
/// The returned value is the conditional state after THIS helper,
/// never the full eventual 19C78 mixer-entry state.
pub fn conditional_original_state_route(
    outer_api_success:bool, outer_platform_id:u32,
    keyboard:KeyboardResult, nested:NestedVersionResult, before:u32
)->Option<ConditionalWin9xOutcome> {
    if !outer_api_success || outer_platform_id!=1 {
        return None;
    }
    Some(conditional_win9x_outcome(
        keyboard,nested,Win9xPointer::WritableDword(before)
    ))
}

/// Original-pointer and rel32 CALL audit of the 1987D -> 1A9B6
/// -> 1C7F0 chain. Original binary not executed, all RVA bytes
/// are verified only after the parent SHA-256 gate succeeds.
pub fn audit_original_global_state_call_chain(dll:&[u8],pe:&Pe32)->Result<()> {
    let state=super::unsafedisc_dll_state_provenance::STATE_PREFERRED_VA;
    if state!=0x0090_5010 {
        return Err("the original global decoder state RVA changed".into());
    }
    dll_expect_raw_x86(dll,pe,0x1987D,&[
        0x68,0x10,0x50,0x90,0x00, // PUSH preferred global-state VA
    ])?;
    dll_expect_raw_x86(dll,pe,0x1AA9A,&[
        0x8B,0x4D,0x08,0x51, // MOV ECX,[EBP+8]; PUSH ECX
    ])?;
    dll_expect_raw_x86(dll,pe,0x1D2A4,&[
        0x8B,0x4D,0x08,0x8B,0x11,
        0x81,0xC2,0x4F,0x03,0,0, // ADD EDX,0x34F
        0x8B,0x45,0x08,0x89,0x10,
    ])?;
    for (site,expected) in [
        (0x19882u32,0x1A9B6u32),
        (0x1AA9Eu32,0x1C7F0u32),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=expected {
            return Err(format!(
                "original global-state pointer forwarding CALL 0x{site:05X} changed"
            ));
        }
    }
    Ok(())
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
    audit_original_global_state_call_chain(dll,pe)?;
    audit_win9x_conditional_returns(dll,pe)?;
    audit_rejected_keyboard_interrupt_source(dll,pe)?;
    audit_win9x_entry_to_keyboard_gate(dll,pe)?;
    audit_win9x_gate_success_to_version_api(dll,pe)?;
    audit_win9x_nested_version_to_pointer_guard(dll,pe)?;
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
    fn rejected_keyboard_return_equal_to_4300_retains_zero_local() {
        for version in [NestedVersionResult::Failed,
                        NestedVersionResult::Succeeded{platform_id:1}] {
            for keyboard in [
                KeyboardResult::Accepted,
                KeyboardResult::RejectedInterruptReturned{eax:0x4300},
            ] {
                assert_eq!(conditional_nested_pointer_route(keyboard,version),
                    NestedGateState{local_flag:0,route:GuardRoute::BypassPointerGuard});
            }
        }
    }

    #[test]
    fn rejected_keyboard_other_interrupt_return_reaches_pointer_guard() {
        for ret in [0u32,1,0x42FF,0x4301,0xFFFF_4300,u32::MAX] {
            for version in [
                NestedVersionResult::Failed,
                NestedVersionResult::Succeeded{platform_id:1},
            ] {
                assert_eq!(conditional_nested_pointer_route(
                    KeyboardResult::RejectedInterruptReturned{eax:ret},version),
                    NestedGateState{local_flag:1,route:GuardRoute::ExaminePointerGuard});
            }
        }
    }

    #[test]
    fn nested_version_other_platform_sets_fifteen_independently_of_keyboard() {
        for platform_id in [0u32,2,3,u32::MAX] {
            for keyboard in [
                KeyboardResult::Accepted,
                KeyboardResult::RejectedInterruptReturned{eax:0x4300},
                KeyboardResult::RejectedInterruptReturned{eax:0},
            ] {
                assert_eq!(conditional_nested_pointer_route(keyboard,
                    NestedVersionResult::Succeeded{platform_id}),
                    NestedGateState{local_flag:15,route:GuardRoute::ExaminePointerGuard});
            }
        }
    }

    #[test]
    fn exhaustive_conditional_route_truth_table() {
        for case in 0u32..3 {
            for version_case in 0u32..4 {
                let keyboard=match case {
                    0=>KeyboardResult::Accepted,
                    1=>KeyboardResult::RejectedInterruptReturned{eax:0x4300},
                    _=>KeyboardResult::RejectedInterruptReturned{eax:0x4301},
                };
                let version=match version_case {
                    0=>NestedVersionResult::Failed,
                    1=>NestedVersionResult::Succeeded{platform_id:1},
                    2=>NestedVersionResult::Succeeded{platform_id:2},
                    _=>NestedVersionResult::Succeeded{platform_id:0},
                };
                let result=conditional_nested_pointer_route(keyboard,version);
                let local=if version_case>=2 {15}
                    else if case==2 {1}else{0};
                assert_eq!(result.local_flag,local);
                assert_eq!(result.route,if local==0{
                    GuardRoute::BypassPointerGuard
                }else{
                    GuardRoute::ExaminePointerGuard
                });
            }
        }
    }

    #[test]
    fn bypass_guard_returns_ax_zero_and_preserves_writable_dword() {
        for keyboard in [
            KeyboardResult::Accepted,
            KeyboardResult::RejectedInterruptReturned{eax:0x4300},
        ] {
            for pointer in [Win9xPointer::Null,
                            Win9xPointer::WritableDword(0),
                            Win9xPointer::WritableDword(u32::MAX)] {
                let outcome=conditional_win9x_outcome(
                    keyboard,NestedVersionResult::Succeeded{platform_id:1},
                    pointer
                );
                assert_eq!(outcome.ax,0);
                assert!(!outcome.wrote_dword);
                assert_eq!(outcome.pointed_value,match pointer {
                    Win9xPointer::Null=>None,
                    Win9xPointer::WritableDword(v)=>Some(v),
                });
            }
        }
    }

    #[test]
    fn reached_guard_with_null_pointer_returns_ax_one_without_write() {
        for keyboard in [
            KeyboardResult::Accepted,
            KeyboardResult::RejectedInterruptReturned{eax:0x4300},
            KeyboardResult::RejectedInterruptReturned{eax:0x4301},
        ] {
            let outcome=conditional_win9x_outcome(
                keyboard,NestedVersionResult::Succeeded{platform_id:2},
                Win9xPointer::Null
            );
            assert_eq!(outcome,ConditionalWin9xOutcome{
                ax:1,pointed_value:None,wrote_dword:false
            });
        }
    }

    #[test]
    fn reached_guard_with_writable_pointer_wraps_and_returns_ax_one() {
        for before in [0u32,1,0xFFFF_FF00,u32::MAX] {
            let outcome=conditional_win9x_outcome(
                KeyboardResult::RejectedInterruptReturned{eax:0x4301},
                NestedVersionResult::Succeeded{platform_id:1},
                Win9xPointer::WritableDword(before)
            );
            assert_eq!(outcome,ConditionalWin9xOutcome{
                ax:1,pointed_value:Some(before.wrapping_add(0x34F)),
                wrote_dword:true
            });
        }
    }

    #[test]
    fn outer_query_must_select_original_windows9x_helper() {
        for platform_id in [0,1,2,3,u32::MAX] {
            for success in [false,true] {
                let result=conditional_original_state_route(
                    success,platform_id,KeyboardResult::Accepted,
                    NestedVersionResult::Succeeded{platform_id:2},42
                );
                assert_eq!(result.is_some(),success && platform_id==1);
            }
        }
    }

    #[test]
    fn debugger_like_68h_return_could_increment_original_global_state() {
        let result=conditional_original_state_route(
            true,1,
            KeyboardResult::RejectedInterruptReturned{eax:0},
            NestedVersionResult::Succeeded{platform_id:1},
            u32::MAX
        ).unwrap();
        assert_eq!(result,ConditionalWin9xOutcome{
            ax:1,
            pointed_value:Some(0x34E),
            wrote_dword:true,
        });
        let normal=conditional_original_state_route(
            true,1,
            KeyboardResult::RejectedInterruptReturned{eax:0x4300},
            NestedVersionResult::Succeeded{platform_id:1},
            u32::MAX
        ).unwrap();
        assert_eq!(normal,ConditionalWin9xOutcome{
            ax:0,
            pointed_value:Some(u32::MAX),
            wrote_dword:false,
        });
    }

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
    fn accepted_keyboard_nested_version_conditional_state_add_gate() {
        for platform in [0,1,2,3,u32::MAX] {
            assert!(!accepted_keyboard_nested_version_to_guard(false,platform));
            assert_eq!(
                accepted_keyboard_nested_version_to_guard(true,platform),
                platform!=1
            );
        }
        // The result models a path to the existing pointer-null guard
        // ONLY, not an unconditional write or the complete DLL state.
    }

    #[test]
    fn downstream_original_flag_only_segments_are_bounded_and_ordered() {
        for (a,b) in [(0x1CD7B,0x1CE3B),(0x1CE4F,0x1CFFF),(0x1D009,0x1D051)] {
            assert!(a<b);
            assert!(b<=0x1D051);
        }
        assert_eq!(0x1CD7B,0x1CD52+2+0x27);
        assert_eq!(0x1CE4F,0x1CD76+5+0xD4);
    }

    #[test]
    fn all_sixteen_x86_jcc_conditions_cover_complementary_flag_pairs() {
        for mask in 0u8..32 {
            let f=GatePredecessorFlags {
                cf:mask&1!=0,zf:mask&2!=0,pf:mask&4!=0,
                sf:mask&8!=0,of:mask&16!=0,
            };
            for even in (0u8..16).step_by(2) {
                assert_ne!(
                    branch_from_eflags(even,f).unwrap(),
                    branch_from_eflags(even+1,f).unwrap()
                );
            }
        }
        assert!(branch_from_eflags(16,GatePredecessorFlags {
            cf:false,zf:false,pf:false,sf:false,of:false,
        }).is_err());
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
