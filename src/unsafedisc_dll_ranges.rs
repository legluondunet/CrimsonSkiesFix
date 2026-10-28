// Exact-original static call graph for SafeDisc category range handlers.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This is an inspection tool, NOT a memory emulator or decrypted DLL.
// Calls and selected opcodes are checked against the pinned original
// dplayerx.dll before classifying any behavior. Synthetic range helpers
// illustrate only the independently verified comparison/merge semantics.

use super::{Pe32, Result, dll_relative_call_target, dll_expect_raw_x86};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: u32,
    pub length: u32,
    pub category: u32,
}

/// Synthetic model of the signed-independent 32-bit interval geometry.
/// Real C2F0 and C310 use 32-bit x86 arithmetic with no overflow guard;
/// we reject overflow in this research helper instead of silently
/// treating wrapped addresses as a normal physical-memory interval.
pub fn exclusive_end(record: Range) -> Option<u32> {
    if record.length == 0 { return None; }
    record.start.checked_add(record.length)
}

/// C4B0 checks the category, then accepts touching (not overlapping)
/// half-open intervals in either order. This synthetic helper deliberately
/// does not treat a mismatched category or an overlapping range as adjacent.
pub fn touching_same_category(a: Range, b: Range) -> bool {
    if a.category != b.category { return false; }
    match (exclusive_end(a), exclusive_end(b)) {
        (Some(a_end), Some(b_end)) => a_end == b.start || b_end == a.start,
        _ => false,
    }
}

/// Range merge behavior of C2F0 after a valid adjacency decision.
/// C2F0 lowers the start when appropriate and adds the lengths; it
/// does not itself establish that input ranges were adjacent.
pub fn join_touching(a: Range, b: Range) -> Option<Range> {
    if !touching_same_category(a, b) { return None; }
    let start = a.start.min(b.start);
    let length = a.length.checked_add(b.length)?;
    let merged = Range { start, length, category: a.category };
    exclusive_end(merged)?;
    Some(merged)
}

/// Fixed bounded rel32 graph from the verified original x86 of B850,
/// B880, B8E0 and their immediate range/list subroutines.
/// Import names and effect-free purity are NOT inferred from a call edge.
pub const VERIFIED_CALL_EDGES: [(u32, u32); 20] = [
    (0xB865, 0xBAD0), // B850 range search
    (0xB8A1, 0xBC40), // B880 search
    (0xB8B4, 0xC2F0), // range start/length adjustment
    (0xB8BE, 0xB9D0), // linked record insertion
    (0xB8F6, 0xBC00), // B8E0 search
    (0xB90F, 0xB7C0), // allocate a new record
    (0xB919, 0xB940), // link record
    (0xBAF5, 0xC3D0), // range ordering comparison
    (0xBB08, 0xC350), // range containment and category check
    (0xBB29, 0xB9D0), // link existing record
    (0xBC5E, 0xC4B0), // touching-range and category predicate
    (0xBC6F, 0xC3D0), // range ordering comparison
    (0xBC12, 0xC3D0), // range ordering comparison
    (0xB7C8, 0xB610), // 12-byte record allocation
    (0xB7F3, 0xC2A0), // initialize range record data
    (0xB964, 0xB980), // follow-up linked-list check
    (0xB9E6, 0xC4F0), // adjacency predicate
    (0xB9FC, 0xC530), // range crossing predicate
    (0xBA12, 0xC310), // range-span union
    (0xBA1F, 0xBA90), // unlink/relink list record
];

pub fn verify_original(dll: &[u8], pe: &Pe32) -> Result<usize> {
    for (site, expected) in VERIFIED_CALL_EDGES {
        let actual = dll_relative_call_target(dll, pe, site)?;
        if actual != expected {
            return Err(format!(
                "original SafeDisc range-list call at RVA 0x{site:05X} \
                 points to 0x{actual:05X}, expected 0x{expected:05X}"
            ));
        }
    }

    // Confirm mutations are to list node fields/range metadata.
    // A call's destination alone does not prove its semantics.
    for (site, expected) in [
        (0xB86Du32, &[0x85,0xC0,0x74,0x09][..]), // search result branch
        (0xB871, &[0x89,0x46,0x08][..]), // selected record [+8]
        (0xB8AB, &[0x85,0xF6,0x74,0x23][..]), // adjacent search result
        (0xB8C6, &[0x89,0x77,0x08][..]), // selected list node [+8]
        (0xB900, &[0x85,0xF6,0x74,0x2B][..]), // new range candidate
        (0xB924, &[0x89,0x53,0x08][..]), // selected node [+8]
        (0xB7C1, &[0x6A,0x0C][..]), // allocation size 12
        (0xC2F8, &[0x3B,0x08,0x73,0x02,0x89,0x08][..]), // lower start
        (0xC305, &[0x03,0xD1,0x89,0x50,0x04][..]), // increase length
        (0xC3D8, &[0x3B,0x01,0x1B,0xC0,0xF7,0xD8,0xC3][..]),
        (0xB950, &[0x89,0x48,0x04][..]), // link old prev -> new
        (0xB95B, &[0x89,0x50,0x08][..]), // new -> successor
        (0xB95E, &[0x89,0x42,0x04][..]), // successor -> new
        (0xB961, &[0x89,0x41,0x08][..]), // predecessor -> new
        (0xC4BB, &[0x8B,0x4A,0x08,0x3B,0xCE][..]), // category equality
    ] {
        dll_expect_raw_x86(dll, pe, site, expected)?;
    }
    Ok(VERIFIED_CALL_EDGES.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_branch_graph_has_unique_call_sites() {
        let mut sites = std::collections::HashSet::new();
        for (site,target) in VERIFIED_CALL_EDGES {
            assert!(sites.insert(site));
            assert!((0xB000..0xC600).contains(&target));
        }
        assert_eq!(sites.len(),20);
    }

    #[test]
    fn neighboring_ranges_join_in_both_orders() {
        let a = Range { start:0x1000,length:0x200,category:4 };
        let b = Range { start:0x1200,length:0x100,category:4 };
        let expected = Some(Range { start:0x1000,length:0x300,category:4 });
        assert_eq!(join_touching(a,b),expected);
        assert_eq!(join_touching(b,a),expected);
    }

    #[test]
    fn category_conflict_and_overlap_do_not_count_as_touching() {
        let a=Range{start:100,length:20,category:4};
        assert_eq!(join_touching(a,Range{start:120,length:20,category:5}),None);
        assert_eq!(join_touching(a,Range{start:110,length:20,category:4}),None);
        assert_eq!(join_touching(a,Range{start:121,length:20,category:4}),None);
    }

    #[test]
    fn zero_length_and_wrapping_intervals_are_rejected() {
        let a=Range{start:0,length:0,category:1};
        let b=Range{start:0,length:10,category:1};
        assert_eq!(join_touching(a,b),None);
        let high=Range{start:u32::MAX-2,length:4,category:1};
        assert_eq!(exclusive_end(high),None);
        assert!(!touching_same_category(high,b));
    }

    #[test]
    fn linked_list_call_sites_cover_all_three_handlers() {
        assert!(VERIFIED_CALL_EDGES.contains(&(0xB865,0xBAD0)));
        assert!(VERIFIED_CALL_EDGES.contains(&(0xB8B4,0xC2F0)));
        assert!(VERIFIED_CALL_EDGES.contains(&(0xB919,0xB940)));
        assert!(VERIFIED_CALL_EDGES.contains(&(0xB7C8,0xB610)));
    }
}
