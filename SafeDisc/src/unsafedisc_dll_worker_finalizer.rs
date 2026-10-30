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


/// Original 196AE calls an opaque helper A310. The caller checks
/// the returned low 16 bits ONLY: AND EAX,FFFFh; TEST; JE 19819.
/// A zero result bypasses this iteration's mixer entry, while a
/// nonzero result continues via the following source-level mazes.
/// No claim is made about A310's behavior or the actual return value.
pub fn worker_setup_skips_mixer(eax_from_helper:u32)->bool {
    eax_from_helper & 0xFFFF == 0
}

/// Hypothetical arithmetic at three original instruction runs on
/// a path that has passed A310's nonzero-low16 gate:
/// 196DB: local[-0x10] = local[-0x0C] - argument[+0x1C]
/// 196F8: local[-0x08] = local[-0x14] - local[-0x0C] + 1
/// 1976F: ECX = argument[+0x20] + local[-0x10]
/// Only modulo-2^32 arithmetic is modeled. In particular a negative
/// mathematical span is NOT silently reinterpreted as a safe range.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ConditionalWorkerLocals {
    pub offset_at_196db:u32,
    pub step_at_196f8:u32,
    pub address_sum_at_1976f:u32,
}

pub fn hypothetical_worker_locals(
    cursor:u32, end_like:u32, argument_1c:u32, argument_20:u32,
)->ConditionalWorkerLocals {
    let offset_at_196db=cursor.wrapping_sub(argument_1c);
    let step_at_196f8=end_like.wrapping_sub(cursor).wrapping_add(1);
    let address_sum_at_1976f=argument_20.wrapping_add(offset_at_196db);
    ConditionalWorkerLocals {
        offset_at_196db,step_at_196f8,address_sum_at_1976f,
    }
}

/// At 19728 the ORIGINAL instruction is 66 8B 45 F8 (MOV AX,
/// WORD [EBP-8]), followed immediately by 50 (PUSH EAX).
/// A 16-bit MOV updates only AX. It does not clear EAX[31:16].
/// This isolated MOV AX/PUSH model is distinct from the known
/// nonzero-A310 path. On THAT verified path, an earlier AND EAX,FFFF
/// clears the upper word (see conditional_original_19728_push).
/// No later helper argument or entrypoint reachability is inferred.
pub fn hypothetical_19728_push_eax(eax_before:u32,local_step:u32)->u32 {
    (eax_before & 0xFFFF_0000) | (local_step & 0xFFFF)
}

/// Unlike MOV AX considered in isolation, the ORIGINAL worker
/// nonzero A310-return path first executes AND EAX,0xFFFF at
/// 196B6. The three separately audited opaque segments up to
/// 19728 contain only branches, NOP and self-MOV/XCHG, while
/// the intervening 196DB and 196F8 local arithmetic changes
/// ECX and EDX, NOT EAX. Hence, on this conditional path,
/// the 19728 PUSH EAX is exactly local_step & 0xFFFF.
///
/// None means the low16 A310 test takes the 19819 bypass,
/// so the 19728 PUSH is not reached on this modeled pass.
/// This conclusion is local and does not infer any later helper
/// argument or real original startup reachability.
pub fn conditional_original_19728_push(
    a310_return_eax:u32,local_step:u32
)->Option<u32> {
    let eax_after_and=a310_return_eax & 0xFFFF;
    if eax_after_and==0 {return None;}
    Some(hypothetical_19728_push_eax(eax_after_and,local_step))
}

/// Cross-stage model of a single ORIGINAL worker iteration, but
/// CONDITIONAL on BOTH helper calls returning normally and no
/// intervening write to caller local [EBP-8]. 19728 prepares a
/// low16-only PUSH before 19965, while 197D4 adds the full
/// 32-bit local [EBP-8] to [EBP-0x0C]. This models the potential
/// size/progress split, not the effective 19965 argument value:
/// the intermediate 19729..1974A argument chain remains unknown.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HypotheticalWorkerStepProgress {
    pub value_pushed_at_19728:u32,
    pub full_step_at_197d4:u32,
    pub next_cursor:u32,
    pub truncated_at_push:bool,
}

pub fn hypothetical_worker_step_progress(
    a310_return_eax:u32,cursor:u32,local_step:u32,
)->Option<HypotheticalWorkerStepProgress> {
    let pushed=conditional_original_19728_push(a310_return_eax,local_step)?;
    Some(HypotheticalWorkerStepProgress {
        value_pushed_at_19728:pushed,
        full_step_at_197d4:local_step,
        next_cursor:cursor.wrapping_add(local_step),
        truncated_at_push:pushed!=local_step,
    })
}

/// Cross-check the THREE x86 instructions defining the conditional
/// local-value provenance. No original code is executed, and the
/// raw bytes alone cannot prove preservation of the step across
/// either opaque callee.
pub fn audit_original_worker_step_local(
    dll:&[u8],pe:&Pe32,
)->Result<()> {
    for (site,bytes) in [
        (0x196F8u32,&[
            0x8B,0x55,0xEC,0x2B,0x55,0xF4,0x83,0xC2,0x01,
            0x89,0x55,0xF8,
        ][..]),
        (0x19728,&[0x66,0x8B,0x45,0xF8,0x50][..]),
        (0x197D4,&[
            0x8B,0x45,0xF4,0x03,0x45,0xF8,0x89,0x45,0xF4,
        ][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    Ok(())
}

/// A separate safe-analysis predicate, never an assertion that
/// SafeDisc applied these checks at runtime. The original x86 can
/// produce zero steps, wrapped counters and low16 truncations.
pub fn hypothetical_step_is_contiguous_and_word_sized(
    cursor:u32,end_like:u32,
)->bool {
    end_like>=cursor &&
    u64::from(end_like)-u64::from(cursor)+1<=u64::from(u16::MAX)
}

/// Conditional flag-only networks after the A310 helper's nonzero
/// masked return. Three distinct original byte sequences converge
/// before the stack-local state preparation and 19965 call.
/// No side effect is emulated; these original Jcc/JMP/self-MOV/NOP
/// sites are read as data, without running any protected code.
pub fn trace_original_worker_setup_segment(
    dll:&[u8],pe:&Pe32,start:u32,end:u32,f:GatePredecessorFlags
)->Result<Vec<u32>> {
    if !matches!((start,end),
        (0x196C3,0x196DB)|(0x196E4,0x196F8)|(0x19704,0x19728))
    {
        return Err("unsupported original worker-to-mixer segment".into());
    }
    let mut pc=start;
    let mut trace=Vec::new();
    for _ in 0..48 {
        if pc==end {
            trace.push(pc);
            return Ok(trace);
        }
        if trace.contains(&pc) {
            return Err(format!("worker-to-mixer setup loops at 0x{pc:05X}"));
        }
        trace.push(pc);
        pc=original_eflags_step_in_range(dll,pe,pc,f,start,end)?;
    }
    Err("original worker-to-mixer stage exceeded 48 instructions".into())
}

/// Pinned original-image verification of the conditional A310
/// return gate and three original flag-only setup mazes.
pub fn audit_original_worker_mixer_predecessor(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x196B3u32,&[0x83,0xC4,0x14,0x25,0xFF,0xFF,0,0,0x85,0xC0][..]),
        (0x196BD,&[0x0F,0x84,0x56,0x01,0,0][..]), // JE 19819
        (0x196C3,&[0x7B,0x11,0x7A,0x0F][..]),
        (0x196DB,&[0x8B,0x4D,0xF4,0x2B,0x4D,0x1C,0x89,0x4D,0xF0][..]),
        (0x196E4,&[0x7B,0x07][..]),
        (0x196F8,&[0x8B,0x55,0xEC,0x2B,0x55,0xF4,0x83,0xC2,0x01,
                    0x89,0x55,0xF8][..]),
        (0x19704,&[0x73,0x0B,0x74,0x07][..]),
        (0x19728,&[0x66,0x8B,0x45,0xF8,0x50][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if dll_relative_call_target(dll,pe,0x196AE)?!=0xA310 {
        return Err("original 196AE worker setup helper CALL target changed".into());
    }
    if 0x196BDu32+6+0x156!=0x19819 {
        return Err("original worker low16-zero JE branch target changed".into());
    }
    for (start,end,want_paths,want_min,want_max) in [
        (0x196C3u32,0x196DBu32,8usize,4usize,7usize),
        (0x196E4,0x196F8,3,7,11),
        (0x19704,0x19728,4,10,18),
    ] {
        let mut unique=std::collections::HashSet::new();
        let mut min=usize::MAX;
        let mut max=0usize;
        for bits in 0u8..32 {
            let flags=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let p=trace_original_worker_setup_segment(
                dll,pe,start,end,flags
            )?;
            min=min.min(p.len());
            max=max.max(p.len());
            unique.insert(p);
        }
        if unique.len()!=want_paths || min!=want_min || max!=want_max {
            return Err(format!(
                "original worker-mixer segment 0x{start:05X}..0x{end:05X} changed: \
                 {} distinct paths, {min}..{max} boundaries",
                unique.len()
            ));
        }
    }
    Ok(())
}

/// After original helper CALL 19965 at 1974A returns, ADD ESP,14h
/// sets flags. The obfuscated source between 19752 and 1976F
/// conditionally calls original 197F8, which is a bare RET.
/// Exactly the CF=0,ZF=0 assignments can enter this no-op CALL;
/// their following JA must be taken, so the fake premature RET
/// at 1976D is not reached. No real original x86 is executed.
pub fn trace_original_mixer_setup_detour(
    dll:&[u8],pe:&Pe32,f:GatePredecessorFlags
)->Result<Vec<u32>> {
    let mut pc=0x19752u32;
    let mut trace=Vec::new();
    for _ in 0..24 {
        if pc==0x1976F {
            trace.push(pc);
            return Ok(trace);
        }
        if trace.contains(&pc) {
            return Err(format!("mixer setup detour loops at 0x{pc:05X}"));
        }
        trace.push(pc);
        pc=match pc {
            0x19764 => 0x19765, // PUSH EDX preserves flags
            0x19765 => 0x1976A, // CALL 197F8: bare RET
            0x1976A => 0x1976B, // POP EDX preserves flags
            0x1976B => {
                if !f.cf && !f.zf {0x1976F}
                else {return Err(
                    "mixer setup no-op CALL reached a potentially premature RET".into())}
            }
            _ => original_eflags_step_in_range(
                dll,pe,pc,f,0x19752,0x1976F
            )?,
        };
    }
    Err("original mixer setup detour did not converge in 24 steps".into())
}

pub fn audit_original_mixer_setup_detour(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x1974Au32,&[0xE8,0x16,0x02,0,0,0x83,0xC4,0x14][..]),
        (0x19752,&[0x70,0x0B,0x77,0x07,0x87,0xFF][..]),
        (0x1975D,&[0xEB,0x03][..]),
        (0x19764,&[0x52][..]),
        (0x19765,&[0xE8,0x8E,0x00,0,0][..]),
        (0x1976A,&[0x5A,0x77,0x02,0xC3][..]),
        (0x1976F,&[0x8B,0x4D,0x20,0x03,0x4D,0xF0][..]),
        (0x197F8,&[0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for (site,expected) in [
        (0x1974Au32,0x19965u32),
        (0x19765,0x197F8),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=expected {
            return Err(format!(
                "original mixer setup CALL target at 0x{site:05X} changed"
            ));
        }
    }
    let mut unique=std::collections::HashSet::new();
    let mut shortest=usize::MAX;
    let mut longest=0usize;
    let mut detours=0usize;
    for bits in 0u8..32 {
        let f=GatePredecessorFlags {
            cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
            sf:bits&8!=0,of:bits&16!=0,
        };
        let p=trace_original_mixer_setup_detour(dll,pe,f)?;
        if p.first()!=Some(&0x19752) || p.last()!=Some(&0x1976F) {
            return Err("mixer setup did not converge on 1976F".into());
        }
        let took_detour=p.contains(&0x19765);
        if took_detour!=(!f.cf && !f.zf) {
            return Err("original mixer setup no-op CALL flag guard changed".into());
        }
        if took_detour {detours+=1;}
        shortest=shortest.min(p.len());
        longest=longest.max(p.len());
        unique.insert(p);
    }
    if unique.len()!=6 || shortest!=7 || longest!=10 || detours!=8 {
        return Err(format!(
            "mixer setup topology changed: {} paths, {shortest}..{longest}, {detours} detours",
            unique.len()
        ));
    }
    Ok(())
}

/// A disjoint original-source callsite/cleanup audit:
///   1974A CALL 19965; 1974F ADD ESP,0x14
///   19798 CALL 19C78; 1979D ADD ESP,0x18
/// These are net caller stack adjustments of 20 and 24 bytes
/// respectively. The arithmetic is CONSISTENT with five and six
/// DWORD push-sized arguments, but does not prove every byte
/// belongs to an argument: local stack reservations and register
/// manipulation between pushes have not been fully recovered.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct OriginalCallerCleanup {
    pub call_rva:u32,
    pub callee_rva:u32,
    pub cleanup_rva:u32,
    pub caller_reclaims_bytes:u32,
}

pub const ORIGINAL_WORKER_CALL_CLEANUPS:[OriginalCallerCleanup;2]=[
    OriginalCallerCleanup {
        call_rva:0x1974A,callee_rva:0x19965,
        cleanup_rva:0x1974F,caller_reclaims_bytes:0x14,
    },
    OriginalCallerCleanup {
        call_rva:0x19798,callee_rva:0x19C78,
        cleanup_rva:0x1979D,caller_reclaims_bytes:0x18,
    },
];

/// Validate the actual immediate return-address stack adjustment
/// against the genuine SHA-pinned DLL. Not an inferred C ABI or
/// a recovery of the argument list to either helper.
pub fn audit_original_worker_call_cleanups(
    dll:&[u8],pe:&Pe32
)->Result<()> {
    for call in ORIGINAL_WORKER_CALL_CLEANUPS {
        if call.cleanup_rva!=call.call_rva+5 ||
           call.caller_reclaims_bytes%4!=0 {
            return Err("original worker call/cleanup layout changed".into());
        }
        if dll_relative_call_target(dll,pe,call.call_rva)?
            !=call.callee_rva {
            return Err(format!(
                "original helper call target changed at 0x{:05X}",
                call.call_rva
            ));
        }
        dll_expect_raw_x86(dll,pe,call.cleanup_rva,
            &[0x83,0xC4,call.caller_reclaims_bytes as u8])?;
    }
    Ok(())
}

/// The original worker directly CALLs the byte mixer 19C78 from
/// 19798. Its return is followed by ADD ESP,0x18 (which sets EFLAGS),
/// then a bounded maze comprising ONLY flag-preserving original
/// Jcc, JMP, self-MOV/XCHG and NOP instructions until 197D4.
/// All 32 EFLAGS assignments *after* ADD ESP reach 197D4.
/// This is a conditional call-return-to-counter bridge; it does
/// NOT prove that the original initializer enters 19798, or that
/// the mixer always returns normally.
pub fn trace_mixer_return_to_worker_counter(
    dll:&[u8],pe:&Pe32,f:GatePredecessorFlags
)->Result<Vec<u32>> {
    let mut pc=0x197A0u32;
    let mut trace=Vec::new();
    for _ in 0..32 {
        if pc==0x197D4 {
            trace.push(pc);
            return Ok(trace);
        }
        if trace.contains(&pc) {
            return Err(format!("mixer-return branch loops at 0x{pc:05X}"));
        }
        trace.push(pc);
        pc=original_eflags_step_in_range(
            dll,pe,pc,f,0x197A0,0x197D4
        )?;
    }
    Err("mixer-return to counter trace did not converge".into())
}

/// Audit the original 19798 E8 rel32 target, the stack adjustment
/// and exhaustive 32-EFLAGS continuation using the exact-hash DLL.
/// This never executes the original code, reads its executable bytes
/// as data and refuses any instruction with side effects.
pub fn audit_original_mixer_to_counter_bridge(dll:&[u8],pe:&Pe32)->Result<()> {
    if dll_relative_call_target(dll,pe,0x19798)?!=0x19C78 {
        return Err("original worker byte-mixer CALL no longer targets 19C78".into());
    }
    dll_expect_raw_x86(dll,pe,0x19798,
        &[0xE8,0xDB,0x04,0,0,0x83,0xC4,0x18])?;
    for (site,bytes) in [
        (0x197A0u32,&[0x70,0x0B,0x77,0x07,0x87,0xD2][..]),
        (0x197AB,&[0xEB,0x03][..]),
        (0x197B4,&[0x78,0x06,0x90,0x79,0x03][..]),
        (0x197BC,&[0x7B,0x11,0x7A,0x0F][..]),
        (0x197CF,&[0x70,0xEE,0x71,0xEC][..]),
        (0x197D4,&[0x8B,0x45,0xF4,0x03,0x45,0xF8][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    let mut distinct=std::collections::HashSet::new();
    let mut shortest=usize::MAX;
    let mut longest=0usize;
    for bits in 0u8..32 {
        let flags=GatePredecessorFlags {
            cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
            sf:bits&8!=0,of:bits&16!=0,
        };
        let path=trace_mixer_return_to_worker_counter(dll,pe,flags)?;
        if path.first()!=Some(&0x197A0) || path.last()!=Some(&0x197D4) {
            return Err("mixer return did not join the counter update".into());
        }
        shortest=shortest.min(path.len());
        longest=longest.max(path.len());
        distinct.insert(path);
    }
    if distinct.len()!=24 || shortest!=12 || longest!=23 {
        return Err(format!(
            "mixer-return to counter graph changed: {} paths {shortest}..{longest}",
            distinct.len()
        ));
    }
    Ok(())
}

/// At original 197D4 the worker loads [EBP-0x0C], adds the DWORD
/// from [EBP-0x08] modulo 2^32, then stores it back. Its
/// 197FA/197FD continuation sets [EBP-0x14]=[EBP-0x04].
/// CMP/JAE at 19689/1968C selects the finalizer precisely when
/// this UINT32 sum is >= the supplied UINT32 limit.
/// This is a local condition, NOT DLL-entrypoint reachability.
pub fn unsigned_worker_exit(cursor:u32,increment:u32,limit:u32)->bool {
    cursor.wrapping_add(increment)>=limit
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WorkerPreExitArm {
    Direct,
    NoopCallRetThenJge,
}
/// The exact original flag maze chooses the 197EF no-op CALL
/// detour iff SF==OF. This is cross-checked against retail bytes
/// for all 32 flag combinations during audit_original.
pub fn expected_pre_exit_arm(flags:GatePredecessorFlags)->WorkerPreExitArm {
    if flags.sf==flags.of {WorkerPreExitArm::NoopCallRetThenJge}
    else {WorkerPreExitArm::Direct}
}

/// The original 197DD flag-only network joins at 197FA for every
/// flag assignment. Some routes pass 197EF PUSH EDX, 197F0
/// CALL 1965A (exact original bare RET), 197F5 POP EDX and
/// 197F6 JGE. The indirect-looking detour cannot take 197F8
/// RET: arriving there implies SF==OF, so JGE is always true.
pub fn trace_original_pre_exit(
    dll:&[u8],pe:&Pe32,flags:GatePredecessorFlags
)->Result<Vec<u32>> {
    let mut pc=0x197DDu32;
    let mut path=Vec::new();
    for _ in 0..24 {
        if pc==0x197FA {
            path.push(pc);
            return Ok(path);
        }
        if path.contains(&pc) {
            return Err(format!("original worker pre-exit loops at 0x{pc:05X}"));
        }
        path.push(pc);
        pc=match pc {
            0x197EF=>0x197F0, // PUSH EDX
            0x197F0=>0x197F5, // CALL original RET 1965A
            0x197F5=>0x197F6, // POP EDX
            0x197F6=>if flags.sf==flags.of {0x197FA} else {
                return Err("197F6 JGE would incorrectly reach a premature RET".into())
            },
            _=>original_eflags_step_in_range(
                dll,pe,pc,flags,0x197DD,0x197FA
            )?,
        };
    }
    Err("original worker pre-exit exceeded bounded trace".into())
}

pub fn audit_original_worker_counter_exit(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,expected) in [
        (0x197D4u32,&[
            0x8B,0x45,0xF4, // MOV EAX,[EBP-0x0C]
            0x03,0x45,0xF8, // ADD EAX,[EBP-0x08]
            0x89,0x45,0xF4, // MOV [EBP-0x0C],EAX
        ][..]),
        (0x197DD,&[0x73,0x0B][..]),
        (0x197EF,&[0x52][..]),
        (0x197F5,&[0x5A,0x7D,0x02,0xC3][..]),
        (0x197FA,&[0x8B,0x4D,0xFC,0x89,0x4D,0xEC][..]),
        (0x19686,&[0x8B,0x55,0xF4,0x3B,0x55,0xEC,0x0F,0x83,
                    0x87,0x01,0x00,0x00][..]),
        (0x1965A,&[0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,expected)?;
    }
    if dll_relative_call_target(dll,pe,0x197F0)?!=0x1965A {
        return Err("original worker pre-exit no-op CALL target changed".into());
    }
    if 0x197F6u32+2+2!=0x197FA {
        return Err("original 197F6 JGE return-avoidance changed".into());
    }
    let mut unique=std::collections::HashSet::new();
    let mut min=usize::MAX;
    let mut max=0usize;
    for bits in 0u8..32 {
        let flags=GatePredecessorFlags {
            cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
            sf:bits&8!=0,of:bits&16!=0,
        };
        let path=trace_original_pre_exit(dll,pe,flags)?;
        let observed=if path.contains(&0x197EF) {
            WorkerPreExitArm::NoopCallRetThenJge
        }else{WorkerPreExitArm::Direct};
        if observed!=expected_pre_exit_arm(flags) {
            return Err("original pre-exit JGE detour selection changed".into());
        }
        min=min.min(path.len());max=max.max(path.len());
        unique.insert(path);
    }
    if unique.len()!=8 || min!=5 || max!=13 {
        return Err(format!(
            "original pre-exit flag topology changed: {} paths {min}..{max}",
            unique.len()
        ));
    }
    Ok(())
}


/// Model ONLY the current worker loop-exit branch, not future
/// iterations. None means that this specific unsigned JAE does
/// not enter the finalizer, or that the outer Win9x path is outside
/// the model. It does not imply that the real game never reaches
/// the finalizer later.
pub fn conditional_finalizer_at_worker_exit(
    cursor:u32,increment:u32,limit:u32,
    outer_api_success:bool,outer_platform_id:u32,
    keyboard:KeyboardResult,nested:NestedVersionResult,
    initial_global_state:u32,
)->Option<ConditionalFinalizerOutcome> {
    if !unsigned_worker_exit(cursor,increment,limit) {
        return None;
    }
    conditional_win9x_finalizer(
        outer_api_success,outer_platform_id,keyboard,nested,
        initial_global_state
    )
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
    audit_original_worker_call_cleanups(dll,pe)?;
    audit_original_worker_step_local(dll,pe)?;
    audit_original_worker_mixer_predecessor(dll,pe)?;
    audit_original_mixer_setup_detour(dll,pe)?;
    audit_original_mixer_to_counter_bridge(dll,pe)?;
    audit_original_worker_counter_exit(dll,pe)?;
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
    fn original_local_arithmetic_has_unsigned_wrap_and_inclusive_count() {
        let v=hypothetical_worker_locals(0x1010,0x101F,0x1000,0x2000);
        assert_eq!(v,ConditionalWorkerLocals{
            offset_at_196db:0x10,step_at_196f8:0x10,
            address_sum_at_1976f:0x2010,
        });
        assert_eq!(hypothetical_worker_locals(5,4,10,20).step_at_196f8,0);
        assert_eq!(hypothetical_worker_locals(
            0xFFFF_FFFE,0xFFFF_FFFF,0,0).step_at_196f8,2);
        assert_eq!(hypothetical_worker_locals(
            u32::MAX,0,0,0).step_at_196f8,2);
        // Difference+one wraps, it is not a validated linear interval.
        assert_eq!(hypothetical_worker_locals(1,0,0,0).step_at_196f8,0);
    }

    #[test]
    fn mov_ax_then_push_eax_does_not_zero_upper_sixteen_bits() {
        let step=0x0001_0000u32;
        assert_eq!(hypothetical_19728_push_eax(0xDEAD_BEEF,step),0xDEAD_0000);
        assert_eq!(hypothetical_19728_push_eax(0x1234_0000,0x5678_ABCD),
                   0x1234_ABCD);
        assert_ne!(hypothetical_19728_push_eax(0xABCD_0000,step),
                   hypothetical_19728_push_eax(0x1234_0000,step));
        for high in [0,1,0x8000,0xFFFF] {
            for low in [0,1,0x8000,0xFFFF] {
                let eax=(high<<16)|0x4321;
                let pushed=hypothetical_19728_push_eax(eax,low);
                assert_eq!(pushed>>16,high);
                assert_eq!(pushed & 0xFFFF,low);
            }
        }
    }

    #[test]
    fn conditional_real_worker_path_clears_high_ax_before_push() {
        for ret in [1u32,0xFFFF,0x0001_0001,0xFFFF_FFFF,0x8000_0001] {
            for step in [0,1,0xFFFF,0x10000,0x1234_5678,u32::MAX] {
                assert_eq!(conditional_original_19728_push(ret,step),
                    Some(step & 0xFFFF));
            }
        }
        for ret in [0u32,0x0001_0000,0xFFFF_0000,0x8000_0000] {
            assert_eq!(conditional_original_19728_push(ret,u32::MAX),None);
        }
        // A 65536-byte hypothetical local step pushes a zero
        // DWORD, not an uninitialized high half.
        assert_eq!(conditional_original_19728_push(1,65536),Some(0));
    }

    #[test]
    fn conditional_full_step_and_low16_push_can_diverge() {
        let p=hypothetical_worker_step_progress(1,0x1000,0x10000).unwrap();
        assert_eq!(p,HypotheticalWorkerStepProgress {
            value_pushed_at_19728:0,
            full_step_at_197d4:0x10000,
            next_cursor:0x11000,
            truncated_at_push:true,
        });
        let normal=hypothetical_worker_step_progress(
            0xFFFF_0001,10,0xFFFF).unwrap();
        assert_eq!(normal.value_pushed_at_19728,0xFFFF);
        assert_eq!(normal.next_cursor,10+0xFFFF);
        assert!(!normal.truncated_at_push);
        assert!(hypothetical_worker_step_progress(0x1_0000,0,42).is_none());
    }

    #[test]
    fn full_step_cursor_uses_wrapping_arithmetic_independently() {
        for (cursor,step,pushed,after) in [
            (0u32,0u32,0u32,0u32),
            (u32::MAX,1,1,0),
            (0xFFFF_FFFF,0x1_0001,1,0x1_0000),
            (1,0xFFFF_FFFF,0xFFFF,0),
        ] {
            let r=hypothetical_worker_step_progress(1,cursor,step).unwrap();
            assert_eq!(r.value_pushed_at_19728,pushed);
            assert_eq!(r.full_step_at_197d4,step);
            assert_eq!(r.next_cursor,after);
            assert_eq!(r.truncated_at_push,step>u32::from(u16::MAX));
        }
    }

    #[test]
    fn word_sized_safety_model_is_not_original_chunk_control() {
        assert!(hypothetical_step_is_contiguous_and_word_sized(0,0));
        assert!(hypothetical_step_is_contiguous_and_word_sized(1,0xFFFF));
        assert!(!hypothetical_step_is_contiguous_and_word_sized(0,0xFFFF));
        assert!(!hypothetical_step_is_contiguous_and_word_sized(7,6));
        assert!(hypothetical_step_is_contiguous_and_word_sized(u32::MAX,u32::MAX));
        assert_eq!(hypothetical_worker_locals(0,0xFFFF,0,0).step_at_196f8,65536);
        // No assertion that SafeDisc actually enforces this guard.
    }

    #[test]
    fn opaque_setup_helper_checks_only_low16_before_mixer() {
        for result in [0u32,0x10000,0xFFFF_0000] {
            assert!(worker_setup_skips_mixer(result));
        }
        for result in [1u32,0xFFFF,0x10001,u32::MAX] {
            assert!(!worker_setup_skips_mixer(result));
        }
    }

    #[test]
    fn noop_mixer_setup_call_is_compatible_only_with_ja_flags() {
        let mut safe_detour_cases=0;
        for bits in 0u8..32 {
            let flags=GatePredecessorFlags{
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            if !flags.cf && !flags.zf {safe_detour_cases+=1;}
        }
        assert_eq!(safe_detour_cases,8);
        assert_eq!(0x19765u32+5+0x8E,0x197F8);
        assert_eq!(0x1976Bu32+2+2,0x1976F);
    }

    #[test]
    fn two_call_cleanups_are_distinct_net_stack_adjustments() {
        let [setup,mixer]=ORIGINAL_WORKER_CALL_CLEANUPS;
        assert_eq!((setup.call_rva,setup.callee_rva,
                    setup.cleanup_rva,setup.caller_reclaims_bytes),
                   (0x1974A,0x19965,0x1974F,20));
        assert_eq!((mixer.call_rva,mixer.callee_rva,
                    mixer.cleanup_rva,mixer.caller_reclaims_bytes),
                   (0x19798,0x19C78,0x1979D,24));
        for call in ORIGINAL_WORKER_CALL_CLEANUPS {
            assert_eq!(call.call_rva+5,call.cleanup_rva);
            assert_eq!(call.caller_reclaims_bytes%4,0);
        }
        // 20/24-byte caller cleanup is NOT proof of a 5/6-arg ABI.
    }

    #[test]
    fn original_mixer_bridge_and_counter_use_distinct_instruction_boundaries() {
        assert_eq!(0x19798u32+5,0x1979D);
        assert_eq!(0x1979D+3,0x197A0);
        assert!(0x197A0<0x197D4);
        assert!(0x197D4<0x197FA);
        // The 32-path analysis is checked against the exact original
        // bytes by audit_original, not simulated in synthetic tests.
    }

    #[test]
    fn original_worker_exit_is_unsigned_and_wraps_at_32bits() {
        assert!(!unsigned_worker_exit(0,0,1));
        assert!(unsigned_worker_exit(0,1,1));
        assert!(unsigned_worker_exit(0xFFFF_FFFE,1,0xFFFF_FFFF));
        assert!(!unsigned_worker_exit(0xFFFF_FFFF,1,1));
        assert!(unsigned_worker_exit(0xFFFF_FFFF,1,0));
        assert!(!unsigned_worker_exit(5,3,9));
        assert!(unsigned_worker_exit(5,4,9));
    }

    #[test]
    fn preexit_detour_is_exactly_the_signed_greater_equal_arm() {
        let mut via_trampoline=0;
        let mut direct=0;
        for bits in 0u8..32 {
            let f=GatePredecessorFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            match expected_pre_exit_arm(f) {
                WorkerPreExitArm::NoopCallRetThenJge=>{
                    assert_eq!(f.sf,f.of);
                    via_trampoline+=1;
                }
                WorkerPreExitArm::Direct=>{
                    assert_ne!(f.sf,f.of);
                    direct+=1;
                }
            }
        }
        assert_eq!((via_trampoline,direct),(16,16));
    }

    #[test]
    fn incomplete_worker_counter_defers_current_finalizer_call() {
        let result=conditional_finalizer_at_worker_exit(
            2,3,6,true,1,
            KeyboardResult::RejectedInterruptReturned{eax:0},
            NestedVersionResult::Succeeded{platform_id:1},0x1357
        );
        assert_eq!(result,None);
    }

    #[test]
    fn unsigned_counter_reaching_limit_enables_conditional_global_delta() {
        for (cursor,step,limit) in [
            (2,4,6),(0xFFFF_FFFE,1,0xFFFF_FFFF),(42,0,42)
        ] {
            let result=conditional_finalizer_at_worker_exit(
                cursor,step,limit,true,1,
                KeyboardResult::RejectedInterruptReturned{eax:0},
                NestedVersionResult::Succeeded{platform_id:1},0xFFFF_FFFF
            ).unwrap();
            assert_eq!(result.state_after_caller_add,0x841);
            assert_eq!(result.caller_add_delta,0x4F3);
        }
        assert!(conditional_finalizer_at_worker_exit(
            0xFFFF_FFFF,1,1,true,1,
            KeyboardResult::RejectedInterruptReturned{eax:0},
            NestedVersionResult::Succeeded{platform_id:1},7
        ).is_none()); // wrapped u32 counter is zero, below limit
    }

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
