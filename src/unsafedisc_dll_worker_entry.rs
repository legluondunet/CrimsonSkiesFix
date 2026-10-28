// Read-only SafeDisc worker entry and 11-way table dispatch research.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The parent inspect command enforces exact retail DLL SHA-256 first.
// This module reads original x86 as data. No DLL is loaded/executed.

use super::{Pe32,Result,read_u32,dll_expect_raw_x86,dll_relative_call_target};
use super::unsafedisc_dll_keyboard_probe::{
    GatePredecessorFlags,original_eflags_step_in_range,
};

pub const WORKER_ENTRY_RVA:u32=0x18F81;
pub const FIRST_WRAPPER_RVA:u32=0x18800;
pub const FIRST_DISPATCH_RVA:u32=0x181E0;
pub const SWITCH_TABLE_RVA:u32=0x18680;
pub const FALLBACK_RVA:u32=0x184EB;
pub const IMAGE_BASE:u32=0x008E0000;

/// The 11 ORIGINAL preferred-base x86 switch case entrypoints.
/// Entry 3 (original 1-based case ID 4) legitimately shares
/// the same target as the selector-out-of-bounds fallback.
pub const SWITCH_CASES:[u32;11]=[
    0x183C2,0x1850D,0x1857F,0x184EB,0x18304,0x18357,
    0x185D5,0x18279,0x18486,0x1863E,0x1841D,
];

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SelectorRoute {
    InRange {index:usize,entry_rva:u32},
    Fallback {entry_rva:u32},
}

/// Original 181E0 uses input selector-1 in ECX, CMP ECX,10 and JA
/// fallback; otherwise indirect JMP through 11 HIGHLOW-rebased
/// absolute addresses. No API, DLL code, or game data is executed.
pub fn original_selector_route(selector:u32)->SelectorRoute {
    if (1..=11).contains(&selector) {
        let index=(selector-1) as usize;
        SelectorRoute::InRange{index,entry_rva:SWITCH_CASES[index]}
    }else{
        SelectorRoute::Fallback{entry_rva:FALLBACK_RVA}
    }
}

/// The obfuscated flag-preserving control flow at the start of the
/// original worker entry always reaches argument setup at 18FA9.
/// Importantly, this proves a LOCAL FIRST CALL after its prologue,
/// not DLL-entrypoint reachability of any caller of the worker.
pub fn trace_worker_entry_to_first_call_setup(
    dll:&[u8],pe:&Pe32,flags:GatePredecessorFlags
)->Result<Vec<u32>> {
    let mut pc=0x18F91u32;
    let mut visited=Vec::new();
    for _ in 0..24 {
        if pc==0x18FA9 {
            visited.push(pc);
            return Ok(visited);
        }
        if visited.contains(&pc) {
            return Err(format!("original 18F81 entry maze loops at 0x{pc:05X}"));
        }
        visited.push(pc);
        pc=original_eflags_step_in_range(
            dll,pe,pc,flags,0x18F91,0x18FA9
        )?;
    }
    Err("original 18F81 entry maze exceeds 24 instructions".into())
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FirstDispatchResultRoute {
    ZeroBypass {target_rva:u32},
    NonzeroNextHelper {target_rva:u32},
}

/// Original 18FCA CMP DWORD [EBP-0x10],0; 18FCE JE 19022.
/// The original branch tests the entire DWORD returned from 18800,
/// not its low 16 bits (unlike the later 196AE A310 check).
pub fn first_dispatch_return_route(eax:u32)->FirstDispatchResultRoute {
    if eax==0 {
        FirstDispatchResultRoute::ZeroBypass{target_rva:0x19022}
    } else {
        FirstDispatchResultRoute::NonzeroNextHelper{target_rva:0x18B00}
    }
}

/// EFLAGS from original CMP DWORD,equivalent to 0. This is a
/// mathematical 32-bit CMP model, never DLL or OS code execution.
/// CMP does not borrow or overflow when subtracting zero.
pub fn cmp_zero_eflags(value:u32)->GatePredecessorFlags {
    GatePredecessorFlags {
        cf:false,zf:value==0,
        pf:(value as u8).count_ones()%2==0,
        sf:value&0x8000_0000!=0,
        of:false,
    }
}

/// For an actually NONZERO result from 18800, source-only static
/// network 18FD0..18FF0 converges on argument setup for the next
/// known CALL at 18FFC -> 18B00. Two routes depend on the sign bit;
/// no assumption about what result the previous helper actually
/// returns is made.
pub fn trace_first_dispatch_nonzero_continuation(
    dll:&[u8],pe:&Pe32,eax:u32
)->Result<Vec<u32>> {
    if eax==0 {
        return Err("zero result takes bypass path 19022".into());
    }
    let flags=cmp_zero_eflags(eax);
    let mut pc=0x18FD0u32;
    let mut route=Vec::new();
    for _ in 0..24 {
        if pc==0x18FF0 {
            route.push(pc);
            return Ok(route);
        }
        if route.contains(&pc) {
            return Err(format!("18FD0 dispatch continuation loops at 0x{pc:05X}"));
        }
        route.push(pc);
        pc=original_eflags_step_in_range(
            dll,pe,pc,flags,0x18FD0,0x18FF0
        )?;
    }
    Err("first worker result continuation did not converge".into())
}

pub fn audit_first_dispatch_return_branch(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x18FC7u32,&[0x89,0x45,0xF0,0x83,0x7D,0xF0,0x00,0x74,0x52][..]),
        (0x18FD0,&[0x7C,0x0A,0x90,0x90,0x77,0x04][..]),
        (0x18FDA,&[0xEB,0x08,0x87,0xF6,0x90,0x7C,0xF1][..]),
        (0x18FE4,&[0xEB,0x07,0x8B,0xC9,0x70,0x06,0x90,0x71,0x03][..]),
        (0x18FED,&[0xEB,0xF7][..]),
        (0x18FF0,&[0x8B,0x55,0x18,0x52,0x8B,0x45,0xF0,0x50,
                    0x8D,0x4D,0x08,0x51][..]),
        (0x19022,&[0xC7,0x45,0xF0,0,0,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if 0x18FCEu32+2+0x52!=0x19022 ||
       dll_relative_call_target(dll,pe,0x18FFC)?!=0x18B00 {
        return Err("first worker return-value branch or next CALL changed".into());
    }
    // CMP x,0 fixes CF=OF=0; PF cannot affect the active graph.
    // Test both signs and parities, including signed-MSB edge
    // cases. Count every original instruction boundary.
    for (value,want_len) in [
        (1u32,12usize),(0x7FFF_FFFF,12),
        (0x8000_0000,15),(u32::MAX,15),
    ] {
        let trace=trace_first_dispatch_nonzero_continuation(dll,pe,value)?;
        if trace.first()!=Some(&0x18FD0) ||
           trace.last()!=Some(&0x18FF0) ||
           trace.len()!=want_len {
            return Err("first worker nonzero result path changed".into());
        }
    }
    Ok(())
}

/// Original worker starts the first 18800 call with six args, of
/// which arg2 is the literal DWORD 1 pushed at 18FB9. The original
/// 18800 wrapper forwards arg2 unchanged as the second of seven
/// arguments to 181E0. That dispatcher re-reads its arg2 from
/// [ESP+178h] after its frame and 4 callee-save pushes, then DEC
/// and dispatches index 0 to RVA 183C2. This is CONDITIONAL on
/// the intermediate 18940 helper returning normally.
pub fn initial_worker_first_case_rva()->u32 {
    match original_selector_route(1) {
        SelectorRoute::InRange{entry_rva,..}=>entry_rva,
        SelectorRoute::Fallback{entry_rva}=>entry_rva,
    }
}

/// Byte-anchored proof of arg2 preservation from the first
/// 18F81 invocation through the argument-shuffling 18800
/// wrapper to the original 11-case switch in 181E0.
/// No original Windows code is invoked.
pub fn audit_initial_worker_fixed_selector(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        // The 18F81 first-call arguments are: ptr, literal 1,
        // local 0, and three separate output addresses.
        (0x18FA9u32,&[
            0x8D,0x45,0xE4,0x50,
            0x8D,0x4D,0xEC,0x51,
            0x8D,0x55,0xF4,0x52,
            0x8B,0x45,0xF0,0x50,
            0x6A,0x01, // original arg2 MUST be 1
            0x8D,0x4D,0x08,0x51,
        ][..]),
        // Seven outgoing PUSHes and their MOV stack-offset
        // loads, in original 18800 wrapper instruction order.
        (0x18800,&[
            0x8B,0x4C,0x24,0x18,
            0x8B,0x54,0x24,0x14,
            0x8D,0x44,0x24,0x18,
            0x50,0x8B,0x44,0x24,0x14,
            0x51,0x8B,0x4C,0x24,0x14,
            0x52,0x8B,0x54,0x24,0x14,
            0x50,0x8B,0x44,0x24,0x14,
            0x51,0x52,0x50,
        ][..]),
        // 181E0 allocates 0x160 stack bytes and saves four
        // registers, pushes three temporary arguments for the
        // 18940 helper, later restores them via ADD ESP,0Ch.
        (0x181E0,&[
            0x81,0xEC,0x60,0x01,0,0,
            0x8D,0x44,0x24,0x28,
            0x8D,0x8C,0x24,0x20,0x01,0,0,
            0x53,0x55,0x56,0x57,
        ][..]),
        (0x181FC,&[0x50,0x51,0x57][..]),
        (0x1821F,&[0x83,0xC4,0x0C][..]),
        (0x18240,&[0x8B,0x8C,0x24,0x78,0x01,0,0][..]),
        (0x18255,&[0x49][..]), // DEC ECX gives zero index
        // Original case 1 starts with a distinct call to 189F0
        // after pushing its case arguments.
        (0x183C2,&[
            0x8D,0x54,0x24,0x10,0x52,0x53,0x50,
            0x68,0x20,0,0,0x20,
            0x68,0x20,0,0,0x20,0x57,
        ][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if 0x160u32+4*4+8!=0x178 ||
       initial_worker_first_case_rva()!=0x183C2 {
        return Err("original worker selector stack layout changed".into());
    }
    for (site,target) in [
        (0x18FBFu32,FIRST_WRAPPER_RVA),
        (0x18823,FIRST_DISPATCH_RVA),
        (0x181FF,0x18940),
        (0x183D4,0x189F0),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!(
                "original first-case CALL at 0x{site:05X} changed"
            ));
        }
    }
    Ok(())
}

/// Original first switch case 183C2 calls 189F0. The 189F0
/// source scans fixed-width 0x28 records: it invokes the original
/// 0xA810 helper with source address, current offset, count 0x28,
/// and an output buffer, then tests:
///   (output DWORD at +0x24 & mask) == expected
/// It returns the first matching offset or zero if no match;
/// offset arithmetic and comparison are original unsigned DWORDs.
/// No actual original record or 0xA810 implementation is supplied
/// to this pure helper, and no protected bytes are read.
pub fn first_case_record_matches(
    trailing_dword:u32,mask:u32,expected:u32
)->bool {
    trailing_dword & mask == expected
}
pub fn first_case_advance_offset(offset:u32)->u32 {
    offset.wrapping_add(0x28)
}
pub fn first_case_has_more_offsets(offset:u32,limit:u32)->bool {
    offset<limit
}

/// Anchor only exact original source bytes for the first case
/// matcher; no semantic assumption is made about the called
/// 0xA810 memory-reading helper beyond the CALL target.
/// This is a bounded local reconstruction, NOT an unpacker.
pub fn audit_original_first_case_record_matcher(
    dll:&[u8],pe:&Pe32
)->Result<()> {
    for (site,bytes) in [
        (0x189F0u32,&[
            0x8B,0x44,0x24,0x14,0x53,0x55,0x56,
            0x8B,0x74,0x24,0x1C,0x57,0x3B,0xF0,0x73,0x41,
        ][..]), // JAE return 0 when offset >= limit
        (0x18A0C,&[
            0x8B,0x44,0x24,0x14,0x57,0x6A,0x28,0x56,0x50,
        ][..]), // A810(base, offset, 0x28, output)
        (0x18A1A,&[
            0x8B,0x57,0x24,0x8B,0xCB,0x23,0xCA,
            0x83,0xC4,0x10,0x3B,0xCD,0x74,0x12,
        ][..]), // (record[0x24] & mask) == expected
        (0x18A28,&[
            0x8B,0x44,0x24,0x24,0x83,0xC6,0x28,0x3B,0xF0,0x72,0xD9,
        ][..]), // offset += 0x28; JB back into scan
        (0x18A33,&[0x33,0xC0,0x5F,0x5E,0x5D,0x5B,0xC3][..]),
        (0x18A3A,&[0x8B,0xC6,0x5F,0x5E,0x5D,0x5B,0xC3][..]),
        (0x18A41,&[0x5F,0x5E,0x5D,0x33,0xC0,0x5B,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if dll_relative_call_target(dll,pe,0x183D4)?!=0x189F0 ||
       dll_relative_call_target(dll,pe,0x18A15)?!=0xA810 ||
       0x189FEu32+2+0x41!=0x18A41 ||
       0x18A26+2+0x12!=0x18A3A ||
       0x18A31+2-39!=0x18A0C {
        return Err("original case1 record scan CALL/Jcc changed".into());
    }
    Ok(())
}

/// Audit the true first worker call chain, not a guessed entrypoint
/// route. The two known callers are proven CALL targets but their
/// reachability from DllMain is NOT established by this check.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    audit_original_first_case_record_matcher(dll,pe)?;
    audit_initial_worker_fixed_selector(dll,pe)?;
    audit_first_dispatch_return_branch(dll,pe)?;
    for (site,expected) in [
        (0x1A1B2u32,WORKER_ENTRY_RVA),
        (0x1A39C,WORKER_ENTRY_RVA),
        (0x18FBF,FIRST_WRAPPER_RVA),
        (0x18823,FIRST_DISPATCH_RVA),
        (0x181FF,0x18940),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=expected {
            return Err(format!(
                "original worker initial CALL at 0x{site:05X} changed"
            ));
        }
    }
    for (site,bytes) in [
        (0x18F81u32,&[0x55,0x8B,0xEC,0x83,0xEC,0x1C,
                        0x53,0x56,0x57,0xC7,0x45,0xF0,0,0,0,0][..]),
        (0x18F91,&[0x7C,0x11,0x7D,0x0F][..]),
        (0x18F94,&[0x0F,0x85,0x0F,0,0,0][..]),
        (0x18FA4,&[0x7F,0xEE,0x7E,0xEC][..]),
        (0x18FA9,&[0x8D,0x45,0xE4,0x50,0x8D,0x4D,0xEC,0x51,
                    0x8D,0x55,0xF4,0x52,0x8B,0x45,0xF0,0x50,
                    0x6A,0x01,0x8D,0x4D,0x08,0x51][..]),
        (0x18FC4,&[0x83,0xC4,0x18,0x89,0x45,0xF0,
                    0x83,0x7D,0xF0,0x00,0x74,0x52][..]),
        (0x18800,&[0x8B,0x4C,0x24,0x18,0x8B,0x54,0x24,0x14,
                    0x8D,0x44,0x24,0x18][..]),
        (0x18823,&[0xE8,0xB8,0xF9,0xFF,0xFF,0x83,0xC4,0x1C,0xC3][..]),
        (0x181E0,&[0x81,0xEC,0x60,0x01,0,0][..]),
        (0x18240,&[0x8B,0x8C,0x24,0x78,0x01,0,0][..]),
        (0x18255,&[0x49,0xC7,0x02,0,0,0,0,
                    0xC7,0x45,0,0,0,0,0,0x83,0xF9,0x0A,
                    0xC7,0x06,0,0,0,0][..]),
        (0x1826C,&[0x0F,0x87,0x79,0x02,0,0][..]),
        (0x18272,&[0xFF,0x24,0x8D,0x80,0x86,0x8F,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }

    let mut routes=std::collections::HashSet::new();
    let mut shortest=usize::MAX;
    let mut longest=0usize;
    for bits in 0u8..32 {
        let flags=GatePredecessorFlags {
            cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
            sf:bits&8!=0,of:bits&16!=0,
        };
        let route=trace_worker_entry_to_first_call_setup(dll,pe,flags)?;
        if route.first()!=Some(&0x18F91) || route.last()!=Some(&0x18FA9) {
            return Err("original worker initial maze did not reach first call".into());
        }
        shortest=shortest.min(route.len());
        longest=longest.max(route.len());
        routes.insert(route);
    }
    if routes.len()!=4 || shortest!=5 || longest!=8 {
        return Err(format!(
            "original worker initial maze changed: {} routes, {shortest}..{longest} steps",
            routes.len()
        ));
    }

    let nt=read_u32(dll,0x3C)? as usize;
    let preferred_base=read_u32(dll,nt+24+28)?;
    if preferred_base!=IMAGE_BASE ||
       0x1826Cu32+6+0x279!=FALLBACK_RVA ||
       SWITCH_CASES.len()!=11
    {
        return Err("original 181E0 dispatcher geometry changed".into());
    }
    let (relocations,_)=super::pe_relocation_blocks(dll,pe)?;
    let relocated_sites:std::collections::HashSet<u32>=relocations.iter()
        .flat_map(|block|block.highlow_offsets.iter()
            .map(move |&offset|block.page_rva+u32::from(offset)))
        .collect();
    let mut unique=std::collections::HashSet::new();
    for (i,&expected_rva) in SWITCH_CASES.iter().enumerate() {
        let site=SWITCH_TABLE_RVA+(i as u32)*4;
        if !relocated_sites.contains(&site) {
            return Err(format!(
                "original 181E0 switch table lacks HIGHLOW relocation at 0x{site:05X}"
            ));
        }
        let off=super::dll_backed_rva_offset(pe,site,4)?;
        let address=read_u32(dll,off)?;
        let want=preferred_base.checked_add(expected_rva)
            .ok_or("original switch address overflow")?;
        if address!=want ||
           !(0x18000..=0x1D497).contains(&expected_rva) ||
           !unique.insert(expected_rva)
        {
            return Err(format!("original 181E0 case {i} target invalid"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_result_comparison_uses_full_dword_not_just_low_word() {
        assert_eq!(first_dispatch_return_route(0),
            FirstDispatchResultRoute::ZeroBypass{target_rva:0x19022});
        for word in [1,0x10000,0xFFFF_0000,0x8000_0000,u32::MAX] {
            assert_eq!(first_dispatch_return_route(word),
                FirstDispatchResultRoute::NonzeroNextHelper{target_rva:0x18B00});
        }
    }

    #[test]
    fn cmp_zero_flags_have_correct_sign_parity_and_no_borrow() {
        let zero=cmp_zero_eflags(0);
        assert!(zero.zf && zero.pf);
        assert!(!zero.cf && !zero.of && !zero.sf);
        let positive=cmp_zero_eflags(1);
        assert!(!positive.zf && !positive.pf && !positive.sf);
        let negative=cmp_zero_eflags(0x8000_0000);
        assert!(negative.sf && !negative.zf && !negative.cf && !negative.of);
        assert_eq!(cmp_zero_eflags(0x10000).zf,false);
    }

    #[test]
    fn case_one_compares_masked_record_tail_to_expected() {
        assert!(first_case_record_matches(0xC0FF_EEAB,0x0000_00FF,0xAB));
        assert!(!first_case_record_matches(0xC0FF_EEAB,0x0000_00FF,0xAC));
        assert!(first_case_record_matches(0,0,0));
        assert!(!first_case_record_matches(0,0,1));
        assert!(!first_case_record_matches(0xFF,0x0F,0x1F));
    }
    #[test]
    fn case_one_scan_stride_and_limit_are_unsigned_wrapping_dwords() {
        assert_eq!(first_case_advance_offset(0),0x28);
        assert_eq!(first_case_advance_offset(0xFFFF_FFF0),0x18);
        assert!(!first_case_has_more_offsets(10,10));
        assert!(first_case_has_more_offsets(9,10));
        assert!(!first_case_has_more_offsets(u32::MAX,1));
    }

    #[test]
    fn first_entry_call_always_supplies_selector_one_to_wrapper() {
        assert_eq!(initial_worker_first_case_rva(),0x183C2);
        assert_eq!(original_selector_route(1),SelectorRoute::InRange{
            index:0,entry_rva:0x183C2
        });
        // After original wrapper forwards arg2 and dispatcher
        // restores its 3 temporary pushes, its stack displacement
        // addresses original second argument (+8).
        assert_eq!(0x160u32+0x10+8,0x178);
    }

    #[test]
    fn selector_is_one_based_and_never_indexes_out_of_bounds() {
        for case in 1u32..=11 {
            assert_eq!(original_selector_route(case),
                SelectorRoute::InRange{
                    index:(case-1) as usize,
                    entry_rva:SWITCH_CASES[(case-1) as usize]
                });
        }
        for case in [0,12,13,0x8000_0000,u32::MAX] {
            assert_eq!(original_selector_route(case),
                SelectorRoute::Fallback{entry_rva:FALLBACK_RVA});
        }
    }
    #[test]
    fn index_four_intentionally_shares_the_fallback_entrypoint() {
        assert_eq!(original_selector_route(4),
            SelectorRoute::InRange{index:3,entry_rva:FALLBACK_RVA});
        assert_eq!(SWITCH_CASES.iter().collect::<std::collections::HashSet<_>>().len(),11);
    }
    #[test]
    fn direct_caller_and_subdispatcher_locations_are_distinct() {
        assert_ne!(WORKER_ENTRY_RVA,FIRST_WRAPPER_RVA);
        assert_ne!(FIRST_WRAPPER_RVA,FIRST_DISPATCH_RVA);
        assert_eq!(SWITCH_TABLE_RVA+11*4,0x186AC);
        assert_eq!(0x1826Cu32+6+0x279,FALLBACK_RVA);
    }
}
