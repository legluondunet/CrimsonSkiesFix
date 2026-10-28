// Original SafeDisc 0xBED0 stage orchestrator, inspect only.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The original DLL is read only after parent SHA-256 validation.
// No protected x86, Win32 APIs, file IO or game code is executed.

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target};

pub const BED0_STAGE_COUNT:usize=8;
pub const ORIGINAL_WORKER_SELECTED_INDEX:usize=4;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ConditionalBed0Trace {
    /// Ordered 0-based indices passed to original BCA0. Its
    /// original result for each index must be supplied by caller.
    pub secondary_indices:Vec<usize>,
    /// Low AX from BDD0 if no second call; otherwise from the
    /// last BCA0 call. This is only a conditional outcome.
    pub returned_ax:u16,
    /// Original BED0 copies its two local working DWORDs to
    /// caller outputs unconditionally before returning, even
    /// when returned_ax == 0. This field is deliberately true.
    pub output_dwords_written:bool,
}

/// Pure model of BED0's original 8-slot control flow.
/// BDD0 is always called once. If BDD0's AX is zero, or the
/// selector is the special value 3, BED0 does not call BCA0.
/// Otherwise it iterates 0..8, skipping the selected index,
/// and stops on the first BCA0 AX==0. The provided [u16;8]
/// contains hypothetical results, NOT observed original code.
///
/// Original 0xBF03 uses the selector as an unguarded stack
/// index: passing a selector outside 0..8 is not considered
/// safe and this model rejects it. The real A310 caller uses
/// exactly index 4, anchored below.
pub fn conditional_bed0_stage_trace(
    selected_index:usize,
    bdd0_ax:u16,
    bca0_ax_by_index:[u16;BED0_STAGE_COUNT],
)->Result<ConditionalBed0Trace> {
    if selected_index>=BED0_STAGE_COUNT {
        return Err("BED0 original unbounded stack selector is outside 0..7".into());
    }
    let mut trace=ConditionalBed0Trace {
        secondary_indices:Vec::new(),
        returned_ax:bdd0_ax,
        output_dwords_written:true,
    };
    if bdd0_ax==0 || selected_index==3 {
        return Ok(trace);
    }
    for index in 0..BED0_STAGE_COUNT {
        if index==selected_index {continue;}
        trace.secondary_indices.push(index);
        trace.returned_ax=bca0_ax_by_index[index];
        if trace.returned_ax==0 {break;}
    }
    Ok(trace)
}

/// Only the known original 196AE worker->A310 path, which
/// pushes literal 4 before calling A310 and forwards that
/// same fourth argument to BED0 at A3F2. No claim is made
/// that this worker path actually executes at startup.
pub fn conditional_original_worker_bed0_trace(
    bdd0_ax:u16,
    bca0_ax_by_index:[u16;BED0_STAGE_COUNT],
)->ConditionalBed0Trace {
    conditional_bed0_stage_trace(
        ORIGINAL_WORKER_SELECTED_INDEX,bdd0_ax,bca0_ax_by_index
    ).expect("4 is always a valid fixed original selector")
}

/// Original C430 lower-bound helper computes an unsigned MAX
/// of the candidate's first DWORD and the supplied lower bound.
pub fn original_c430_lower(start:u32,requested_lower:u32)->u32 {
    start.max(requested_lower)
}

/// Original C450 upper-bound helper computes an unsigned MIN
/// of the candidate's inclusive end (start + len - 1)
/// and supplied upper limit; addition WRAPS on 32 bits.
pub fn original_c450_upper(
    start:u32,len:u32,requested_upper:u32
)->u32 {
    start.wrapping_add(len).wrapping_sub(1).min(requested_upper)
}

/// Original-BDD0 lower/upper limit helper byte audit. The
/// clipping describes only these TWO original utility functions,
/// not a complete valid or safe memory intersection.
pub fn audit_original_bound_clipping(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0xC430u32,&[
            0x8B,0x44,0x24,0x04,0x8B,0x4C,0x24,0x08,
            0x8B,0,0x3B,0xC8,0x72,0x02,0x8B,0xC1,0xC3
        ][..]),
        (0xC450,&[
            0x8B,0x44,0x24,0x04,0x8B,0x48,0x04,
            0x8B,0x10,0x8D,0x44,0x11,0xFF,
            0x8B,0x4C,0x24,0x08,0x3B,0xC8,
            0x73,0x02,0x8B,0xC1,0xC3
        ][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    if 0xC43Cu32+2+2!=0xC440 ||
       0xC463+2+2!=0xC467 ||
       dll_relative_call_target(dll,pe,0xBE24)?!=0xC430 ||
       dll_relative_call_target(dll,pe,0xBE34)?!=0xC450 {
        return Err("original BDD0 bound-clipping helper changed".into());
    }
    Ok(())
}

/// On the original BDD0 failure branch at RVA BE52, EBP is
/// already initialized to zero. The routine returns AX=0,
/// but first stores (*third_argument - 1) modulo 2^32
/// through the pointer passed as the FIRST argument.
/// In the known BED0 call, that first argument is the
/// selected slot's POINTER. This synthetic arithmetic does
/// not dereference anything or establish pointer safety.
pub fn bdd0_failure_selected_dword(third_arg_value:u32)->u32 {
    third_arg_value.wrapping_sub(1)
}

/// Exact original C550 unsigned range predicate used by BDD0
/// (and the node scan inside BE60). It is a *predicate*, not a
/// bounds-safe range abstraction: length is not validated and
/// addition is 32-bit wrapping, as in the original x86.
pub fn original_c550_range_predicate(
    range_start:u32,range_len:u32,query_low:u32,query_high:u32,
)->bool {
    if range_start<query_low {
        return query_low<range_start.wrapping_add(range_len);
    }
    if range_start<=query_high {
        return true;
    }
    if range_start>query_low {
        return false;
    }
    query_low<range_start.wrapping_add(range_len)
}

/// Strict SYNTHETIC interval interpretation for future safe tooling.
/// This intentionally differs from original_c550_range_predicate:
/// it rejects empty, reversed and 32-bit wrapping intervals.
/// The original SafeDisc x86 does NOT perform these checks.
/// Endpoints are inclusive at the public API, and the internal
/// exclusive upper bound uses u64 to represent 0x1_0000_0000.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum CheckedRangeIntersection {
    InvalidGeometry,
    Disjoint,
    Overlap { start:u32, len:u32 },
}

pub fn checked_contiguous_intersection(
    range_start:u32, range_len:u32, query_low:u32, query_high:u32,
)->CheckedRangeIntersection {
    if range_len==0 || query_low>query_high {
        return CheckedRangeIntersection::InvalidGeometry;
    }
    let end_exclusive=u64::from(range_start)+u64::from(range_len);
    let address_space_end=u64::from(u32::MAX)+1;
    if end_exclusive>address_space_end {
        return CheckedRangeIntersection::InvalidGeometry;
    }
    let first=u64::from(range_start.max(query_low));
    let last=end_exclusive.min(u64::from(query_high)+1);
    if first>=last {
        CheckedRangeIntersection::Disjoint
    } else {
        CheckedRangeIntersection::Overlap {
            start:first as u32,
            len:(last-first) as u32,
        }
    }
}

/// The original at RVA C550 contains this exact standalone,
/// side-effect-free unsigned x86 range test. BDD0 uses it
/// to decide whether a candidate node can participate in
/// further transformations. Other helper outcomes are unknown.
pub fn audit_original_c550_predicate(dll:&[u8],pe:&Pe32)->Result<()> {
    for (at,bytes) in [
        (0xC550u32,&[
            0x8B,0x54,0x24,0x08,0x56,0x8B,0x74,0x24,0x08,
            0x33,0xC0,0x8B,0x0E,0x3B,0xCA,0x72,0x0A,
            0x3B,0x4C,0x24,0x10,0x76,0x0D,
            0x3B,0xCA,0x77,0x0E,
            0x8B,0x76,0x04,0x03,0xF1,0x3B,0xD6,0x73,0x05,
            0xB8,0x01,0,0,0,0x5E,0xC3
        ][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }
    for (site,len,delta,target) in [
        (0xC55Fu32,2i32,0x0Ai32,0xC56Bu32),
        (0xC565,2,0x0D,0xC574),
        (0xC569,2,0x0E,0xC579),
        (0xC572,2,0x05,0xC579),
    ] {
        if (site as i64)+(len as i64)+(delta as i64)!=(target as i64) {
            return Err("original C550 unsigned range conditional changed".into());
        }
    }
    if dll_relative_call_target(dll,pe,0xBE11)?!=0xC550 ||
       dll_relative_call_target(dll,pe,0xBE7D)?!=0xC550 {
        return Err("original BDD0 / BE60 C550 range CALL target changed".into());
    }
    Ok(())
}

/// Validate exact original instruction bytes (on the SHA-pinned
/// original user DLL only), direct relative calls, and jump
/// targets. The x86 control effects described here are
/// source-level facts, never simulated as an x86 CPU.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    audit_original_bound_clipping(dll,pe)?;
    audit_original_c550_predicate(dll,pe)?;
    for (site,expected) in [
        (0x1969Fu32,&[
            0x50,0x6A,0x04,0x8D,0x4D,0xEC,0x51,
            0x8D,0x55,0xF4,0x52,0x8D,0x45,0x08,0x50
        ][..]),
        (0xA3E3,&[
            0x8B,0x54,0x24,0x20,0x8B,0x44,0x24,0x18,
            0x52,0x55,0x83,0xC6,0x14,0x50,0x56,
            0xE8,0xD9,0x1A,0,0,0x83,0xC4,0x10
        ][..]),
        (0xBED0,&[
            0x83,0xEC,0x24,0x8B,0x4C,0x24,0x30,
            0x53,0x8B,0x5C,0x24,0x38,0x55,0x8B,
            0x6C,0x24,0x34,0x8B,0x11,0x56,0x8B,
            0x74,0x24,0x34,0x8B,0x45,0,0x57,
            0xB9,0x08,0,0,0,0x8D,0x7C,0x24,0x14,
            0x89,0x44,0x24,0x3C,0x8D,0x44,0x24,
            0x10,0xF3,0xA5
        ][..]),
        (0xBEFF,&[
            0x89,0x54,0x24,0x10,0x8B,0x54,0x9C,0x14,
            0x53,0x8D,0x4C,0x24,0x40,0x50,0x51,
            0x52,0xE8,0xBC,0xFE,0xFF,0xFF
        ][..]),
        (0xBF14,&[
            0x83,0xC4,0x10,0x66,0x85,0xC0,0x74,0x32,
            0x83,0xFB,0x03,0x74,0x2D,0x33,0xF6,
            0x8D,0x7C,0x24,0x14,0x3B,0xDE,0x74,0x1A
        ][..]),
        (0xBF2B,&[
            0x8B,0x17,0x8D,0x44,0x24,0x10,
            0x8D,0x4C,0x24,0x3C,0x50,0x51,0x52,
            0xE8,0x63,0xFD,0xFF,0xFF,
            0x83,0xC4,0x0C,0x66,0x85,0xC0,0x74,0x09
        ][..]),
        (0xBF45,&[0x46,0x83,0xC7,0x04,0x83,0xFE,0x08,0x7C,0xD9][..]),
        (0xBF4E,&[
            0x8B,0x4C,0x24,0x3C,0x8B,0x54,0x24,0x10,
            0x89,0x4D,0,0x8B,0x4C,0x24,0x40,
            0x5F,0x5E,0x5D,0x89,0x11,
            0x5B,0x83,0xC4,0x24,0xC3
        ][..]),
        (0xBE02,&[0x33,0xED,0x85,0xF6,0x74,0x4A][..]),
        (0xBE39,&[
            0x8B,0x4C,0x24,0x1C,0xBD,0x01,0,0,0,
            0x89,0x03,0x83,0xC4,0x08,
            0x89,0x71,0x0C,0x66,0x8B,0xC5
        ][..]),
        (0xBE52,&[
            0x8B,0x13,0x66,0x8B,0xC5,0x4A,
            0x89,0x17,0x5F,0x5E,0x5D,0x5B,0xC3
        ][..]),
        (0xBCF4,&[
            0x66,0x3D,0x01,0,0x75,0x0F,
            0x8B,0x4C,0x24,0x18,0x8B,0x54,0x24,
            0x14,0x89,0x0E,0x89,0x13,0x89,0x7D,0x0C
        ][..]),
    ]{
        dll_expect_raw_x86(dll,pe,site,expected)?;
    }
    for (site,target) in [
        (0x196AEu32,0xA310u32),
        (0xA3F2,0xBED0),
        (0xBF0F,0xBDD0),
        (0xBF38,0xBCA0),
        (0xBDE0,0xBD10),
        (0xBDF8,0xBE60),
        (0xBE11,0xC550),
        (0xBE24,0xC430),
        (0xBE34,0xC450),
        (0xBCAF,0xBD10),
        (0xBCBB,0xBC00),
        (0xBCEC,0xBD50),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!(
                "original BED0 stage CALL 0x{site:05X} changed"
            ));
        }
    }
    for (site,len,delta,target) in [
        (0xBF1Au32,2u32,0x32i32,0xBF4Eu32),
        (0xBF1F,2,0x2D,0xBF4E),
        (0xBF29,2,0x1A,0xBF45),
        (0xBF43,2,0x09,0xBF4E),
        (0xBF4C,2,-39,0xBF27),
        (0xBE06,2,0x4A,0xBE52),
        (0xBE1C,2,0x34,0xBE52),
        (0xBCF8,2,0x0F,0xBD09),
    ] {
        if (site as i64)+(len as i64)+(delta as i64)!=(target as i64){
            return Err("original BED0 conditional branch destination changed".into());
        }
    }
    // This test is intentionally conditional: verify only the
    // original control topology under supplied helper return values.
    let all_one=[1u16;BED0_STAGE_COUNT];
    let expected=vec![0,1,2,3,5,6,7];
    let t=conditional_original_worker_bed0_trace(1,all_one);
    if t.secondary_indices!=expected || t.returned_ax!=1 ||
       !t.output_dwords_written {
        return Err("original worker BED0 7-followup topology changed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_bdd0_clips_low_with_max_and_high_with_min() {
        assert_eq!(original_c430_lower(10,12),12);
        assert_eq!(original_c430_lower(10,8),10);
        assert_eq!(original_c450_upper(10,10,15),15);
        assert_eq!(original_c450_upper(10,10,25),19);
        // A ten-byte candidate [10,19] clipped to [12,15].
        assert_eq!((
            original_c430_lower(10,12),
            original_c450_upper(10,10,15),
        ),(12,15));
    }

    #[test]
    fn original_upper_bound_uses_inclusive_wrapping_32bit_end() {
        assert_eq!(original_c450_upper(0,0,u32::MAX),u32::MAX);
        assert_eq!(original_c450_upper(u32::MAX,2,u32::MAX),0);
        assert_eq!(original_c450_upper(10,0,100),9);
        assert_eq!(original_c430_lower(0,u32::MAX),u32::MAX);
    }

    #[test]
    fn bdd0_failed_predicate_still_writes_selected_pointee() {
        assert_eq!(bdd0_failure_selected_dword(0),u32::MAX);
        assert_eq!(bdd0_failure_selected_dword(1),0);
        assert_eq!(bdd0_failure_selected_dword(0x100),0xFF);
        // The return is AX=0, but the STORE is real original x86.
        let t=conditional_original_worker_bed0_trace(0,[1;8]);
        assert_eq!(t.returned_ax,0);
        assert!(t.output_dwords_written);
    }

    #[test]
    fn exact_c550_unsigned_range_intersection_and_end_boundary() {
        assert!(original_c550_range_predicate(10,5,12,20));
        assert!(original_c550_range_predicate(10,5,10,10));
        assert!(original_c550_range_predicate(10,5,0,20));
        assert!(!original_c550_range_predicate(10,5,15,20));
        assert!(!original_c550_range_predicate(10,5,0,9));
        assert!(!original_c550_range_predicate(10,0,10,9));
    }

    #[test]
    fn c550_keeps_x86_wraparound_without_assuming_safe_intervals() {
        // The original unsigned predicate has no 64-bit wrap repair.
        assert!(!original_c550_range_predicate(
            0xFFFF_FFF0,0x20,0xFFFF_FFF8,u32::MAX));
        assert!(original_c550_range_predicate(10,0,0,20));
        assert!(!original_c550_range_predicate(0,0,1,2));
    }

    #[test]
    fn checked_interval_exposes_x86_predicate_false_positives() {
        use CheckedRangeIntersection::*;
        // C550 treats an empty candidate as a hit if its start is
        // inside the requested bounds; it is not a safe interval.
        assert!(original_c550_range_predicate(10,0,0,20));
        assert_eq!(checked_contiguous_intersection(10,0,0,20),InvalidGeometry);
        // A reversed query is also accepted on one original branch.
        assert!(original_c550_range_predicate(0,10,9,3));
        assert_eq!(checked_contiguous_intersection(0,10,9,3),InvalidGeometry);
        // Original x86 wrapping on the end DWORD can discard a
        // geometrically intended overlap; checked mode rejects it.
        assert!(!original_c550_range_predicate(
            0xFFFF_FFF0,0x20,0xFFFF_FFF8,u32::MAX));
        assert_eq!(checked_contiguous_intersection(
            0xFFFF_FFF0,0x20,0xFFFF_FFF8,u32::MAX),InvalidGeometry);
    }

    #[test]
    fn checked_interval_keeps_last_u32_address_without_wrapping() {
        use CheckedRangeIntersection::*;
        assert_eq!(checked_contiguous_intersection(
            u32::MAX,1,u32::MAX,u32::MAX),
            Overlap{start:u32::MAX,len:1});
        assert_eq!(checked_contiguous_intersection(10,10,12,15),
            Overlap{start:12,len:4});
        assert_eq!(checked_contiguous_intersection(10,10,20,30),
            Disjoint);
        assert_eq!(checked_contiguous_intersection(10,10,0,9),
            Disjoint);
    }

    #[test]
    fn x86_predicate_agrees_with_checked_overlap_on_valid_small_intervals() {
        // Compare independently organized logic on more than 30k
        // ordinary, nonwrapping candidates. Deliberately do NOT
        // assert equivalence on the invalid geometry tested above.
        for start in 0..16u32 {
            for len in 1..16u32 {
                for low in 0..16u32 {
                    for high in low..16u32 {
                        let safe=checked_contiguous_intersection(start,len,low,high);
                        let expected=matches!(safe,CheckedRangeIntersection::Overlap{..});
                        assert_eq!(original_c550_range_predicate(start,len,low,high),
                            expected,"start={start},len={len},low={low},high={high}");
                    }
                }
            }
        }
    }

    #[test]
    fn original_bdd0_failure_write_can_precede_worker_mixer_bypass() {
        // Conditional chain: BDD0 reaches BE52 (AX=0, but stores
        // selected third-argument minus one); BED0 copies its two
        // local output DWORDs on its unconditional epilogue; A310
        // returns the same AX to the caller's low-word gate at
        // 196AE, which skips this iteration's 19C78 mixer.
        //
        // A zero AX alone does NOT imply the BE52 branch executed;
        // this test explicitly assumes the BE52 path.
        let changed_selected=bdd0_failure_selected_dword(0x100);
        assert_eq!(changed_selected,0xFF);
        let bed0=conditional_original_worker_bed0_trace(0,[1;8]);
        assert_eq!(bed0.returned_ax,0);
        assert!(bed0.output_dwords_written);
        assert!(bed0.secondary_indices.is_empty());
        assert!(crate::unsafedisc_dll_worker_finalizer::
            worker_setup_skips_mixer(u32::from(bed0.returned_ax)));
        // This composition cannot establish actual startup
        // reachability, pointer validity or any mixer input state.
    }

    #[test]
    fn original_worker_index_four_checks_other_seven_slots() {
        let t=conditional_original_worker_bed0_trace(1,[1;8]);
        assert_eq!(t.secondary_indices,vec![0,1,2,3,5,6,7]);
        assert_eq!(t.returned_ax,1);
        assert!(t.output_dwords_written);
    }

    #[test]
    fn failed_initial_bdd0_skips_remaining_calls_but_writes_outputs() {
        let t=conditional_original_worker_bed0_trace(0,[1;8]);
        assert!(t.secondary_indices.is_empty());
        assert_eq!(t.returned_ax,0);
        assert!(t.output_dwords_written);
    }

    #[test]
    fn secondary_failure_terminates_after_first_zero_ax() {
        let t=conditional_original_worker_bed0_trace(
            1,[1,1,1,0,1,1,1,1]
        );
        assert_eq!(t.secondary_indices,vec![0,1,2,3]);
        assert_eq!(t.returned_ax,0);
        assert!(t.output_dwords_written);
    }

    #[test]
    fn only_selector_three_bypasses_secondary_iteration_on_success() {
        let t=conditional_bed0_stage_trace(3,1,[1;8]).unwrap();
        assert!(t.secondary_indices.is_empty());
        assert_eq!(t.returned_ax,1);
        assert_eq!(
            conditional_bed0_stage_trace(0,1,[1;8])
                .unwrap().secondary_indices,
            vec![1,2,3,4,5,6,7]
        );
    }

    #[test]
    fn arbitrary_out_of_range_original_selector_is_not_modeled_as_safe() {
        assert!(conditional_bed0_stage_trace(8,1,[1;8]).is_err());
        assert!(conditional_bed0_stage_trace(15,1,[1;8]).is_err());
    }

    #[test]
    fn low_ax_not_equal_one_can_be_nonzero_and_continue() {
        let t=conditional_original_worker_bed0_trace(2,[2;8]);
        assert_eq!(t.secondary_indices,vec![0,1,2,3,5,6,7]);
        assert_eq!(t.returned_ax,2);
    }
}
