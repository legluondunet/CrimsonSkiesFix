// Aggregate-only tests of simple algebraic candidates for the Crimson
// SafeDisc EAX composite on reference-backed, fragment-index-zero DWORDs.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Nothing here decodes a protected binary, executes a Windows DLL or
// claims that a fitted function generalizes to unseen values.

use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlgebraAudit {
    pub pairs: usize,
    pub affine_input_rank: usize,
    pub affine_violations: usize,
    pub rotate_then_add_matches: usize,
    pub add_then_rotate_matches: usize,
}

/// Assess three deliberately narrow hypotheses without printing source
/// or reference DWORDs:
///  1. y = A*x XOR b for a 32x32 binary matrix A (GF(2));
///  2. y = rotate_left(x, r) + k modulo 2^32;
///  3. y = rotate_left(x + k, r) modulo 2^32.
/// The affine residual count measures contradictions to one deterministically
/// selected independent basis. Zero contradictions means the finite sample
/// fits; it is NOT proof for inputs absent from the plaintext reference.
pub fn diagnose(pairs: &HashMap<u32, u32>) -> AlgebraAudit {
    let mut ordered: Vec<(u32, u32)> = pairs.iter()
        .map(|(&source, &output)| (source, output)).collect();
    ordered.sort_unstable_by_key(|&(source, _)| source);
    let mut audit = AlgebraAudit { pairs: ordered.len(), ..AlgebraAudit::default() };
    let Some(&(anchor_in, anchor_out)) = ordered.first() else {
        return audit;
    };

    // Triangular basis for input deltas and their corresponding output
    // deltas. Pivot subtraction uses XOR, not wrapping subtraction.
    // Sorting prevents HashMap iteration order from changing diagnostics.
    let mut basis: [Option<(u32, u32)>; 32] = [None; 32];
    for &(source, output) in &ordered {
        let mut dx = source ^ anchor_in;
        let mut dy = output ^ anchor_out;
        let mut inserted = false;
        for bit in (0..32).rev() {
            if dx & (1u32 << bit) == 0 { continue; }
            if let Some((prior_dx, prior_dy)) = basis[bit] {
                dx ^= prior_dx;
                dy ^= prior_dy;
            } else {
                basis[bit] = Some((dx, dy));
                audit.affine_input_rank += 1;
                inserted = true;
                break;
            }
        }
        if !inserted && dy != 0 {
            audit.affine_violations += 1;
        }
    }

    // Each constant is determined by the same first pair, then tested
    // against *all* distinct reference-backed pairs. No fitting values
    // or game bytes are exposed by the CLI.
    for rotate in 0..32 {
        let k_after = anchor_out.wrapping_sub(anchor_in.rotate_left(rotate));
        if ordered.iter().all(|&(x, y)| x.rotate_left(rotate).wrapping_add(k_after) == y) {
            audit.rotate_then_add_matches += 1;
        }
        let k_before = anchor_out.rotate_right(rotate).wrapping_sub(anchor_in);
        if ordered.iter().all(|&(x, y)| x.wrapping_add(k_before).rotate_left(rotate) == y) {
            audit.add_then_rotate_matches += 1;
        }
    }
    audit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single_observation_are_inconclusive() {
        assert_eq!(diagnose(&HashMap::new()), AlgebraAudit::default());
        let one = HashMap::from([(0x1234_5678, 0xDEAD_BEEF)]);
        let report = diagnose(&one);
        assert_eq!((report.pairs, report.affine_input_rank, report.affine_violations), (1, 0, 0));
        assert_eq!((report.rotate_then_add_matches, report.add_then_rotate_matches), (32, 32));
    }

    #[test]
    fn full_rank_bit_affine_family_has_no_contradictions() {
        let transform = |x: u32| x.rotate_left(11) ^ 0xDEAD_BEEF;
        let mut pairs = HashMap::from([(0, transform(0))]);
        for bit in 0..32 {
            let input = 1u32 << bit;
            pairs.insert(input, transform(input));
        }
        for x in [3, 7, 0xFEED_BEEF, 0xF123_4567] {
            pairs.insert(x, transform(x));
        }
        let report = diagnose(&pairs);
        assert_eq!(report.pairs, 37);
        assert_eq!(report.affine_input_rank, 32);
        assert_eq!(report.affine_violations, 0);
    }

    #[test]
    fn nonlinear_carry_and_corrupt_pair_reject_gf2_affinity() {
        let pairs = HashMap::from([(0, 0), (1, 3), (2, 6), (3, 9)]);
        let report = diagnose(&pairs);
        assert_eq!(report.affine_input_rank, 2);
        assert!(report.affine_violations >= 1);
        assert_eq!(report.rotate_then_add_matches, 0);
        assert_eq!(report.add_then_rotate_matches, 0);
    }

    #[test]
    fn detect_both_modular_rotation_families() {
        let mut rotate_add = HashMap::new();
        let mut add_rotate = HashMap::new();
        for x in [0u32, 1, 2, 3, 7, 0x8000_0000, 0x9876_5432, 0xFFFF_FFFF] {
            rotate_add.insert(x, x.rotate_left(7).wrapping_add(0xA123_4567));
            add_rotate.insert(x, x.wrapping_add(0xA123_4567).rotate_left(7));
        }
        assert!(diagnose(&rotate_add).rotate_then_add_matches >= 1);
        assert!(diagnose(&add_rotate).add_then_rotate_matches >= 1);
    }

    #[test]
    fn same_input_order_cannot_change_probe_result() {
        let a = HashMap::from([(33, 101), (4, 50), (12, 17), (1, 90)]);
        let mut b = HashMap::new();
        for (k,v) in [(1,90),(12,17),(33,101),(4,50)] { b.insert(k,v); }
        assert_eq!(diagnose(&a), diagnose(&b));
    }
}
