// Experimental aggregate-only GF(2) model for indexed SafeDisc EAX pairs.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// If BOTH protected EAX functions are bit-affine, the observed relation
// y = F2(F1(x) XOR mask(index)) must be of the form
// y = A*x XOR B*mask(index) XOR c over GF(2).
// Testing this necessary condition does not decrypt the game or reconstruct
// either EAX routine. The pinned original/reference bytes remain local.

use std::collections::HashMap;

pub fn index_mask(index: u32) -> u32 {
    (index << 24) | (index << 16) | (index << 8) | index
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IndexedAffineAudit {
    pub samples: usize,
    pub feature_rank: usize,
    pub contradictory_samples: usize,
}

pub fn diagnose(pairs: &HashMap<(u32, u32), u32>) -> IndexedAffineAudit {
    // Explicit ordering ensures that every run reports the same number of
    // dependent contradictions, regardless of HashMap randomization.
    let mut ordered: Vec<_> = pairs.iter()
        .map(|(&(index, input), &output)| (index, input, output))
        .collect();
    ordered.sort_unstable();
    let mut report = IndexedAffineAudit {
        samples: ordered.len(), ..IndexedAffineAudit::default()
    };
    let Some(&(base_index, base_input, base_output)) = ordered.first() else {
        return report;
    };
    let base_features = (u64::from(index_mask(base_index)) << 32)
        | u64::from(base_input);
    // A GF(2) linear map 64 -> 32 is represented as independent
    // 64-bit input pivot rows with corresponding 32-bit output deltas.
    let mut pivots: [Option<(u64, u32)>; 64] = [None; 64];
    for &(index, input, output) in &ordered {
        let features = (u64::from(index_mask(index)) << 32)
            | u64::from(input);
        let mut x = features ^ base_features;
        let mut y = output ^ base_output;
        let mut independent = false;
        for bit in (0..64).rev() {
            if (x & (1u64 << bit)) == 0 { continue; }
            if let Some((prior_x, prior_y)) = pivots[bit] {
                x ^= prior_x;
                y ^= prior_y;
            } else {
                pivots[bit] = Some((x, y));
                report.feature_rank += 1;
                independent = true;
                break;
            }
        }
        if !independent && y != 0 {
            report.contradictory_samples += 1;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_full_width_mask_and_boundary() {
        assert_eq!(index_mask(0), 0);
        assert_eq!(index_mask(1), 0x0101_0101);
        assert_eq!(index_mask(0xFF), 0xFFFF_FFFF);
        assert_eq!(index_mask(0x100), 0x0101_0100);
        assert_eq!(index_mask(0x101), 0x0101_0101);
        assert_eq!(index_mask(0x3EE), 0x03EF_EFEE);
    }

    #[test]
    fn bit_affine_two_stage_family_is_consistent_across_masks() {
        let f1 = |v: u32| v.rotate_left(9) ^ 0xABCD_1234;
        let f2 = |v: u32| v.rotate_right(7) ^ 0x4192_01A5;
        let mut examples = HashMap::new();
        // Input feature rank 32 from 32 basis bits.
        for bit in 0..32 {
            let input = 1u32 << bit;
            examples.insert((0, input), f2(f1(input) ^ index_mask(0)));
        }
        examples.insert((0, 0), f2(f1(0)));
        // Mask feature rank from independent fragment-local indices.
        for index in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1006] {
            examples.insert((index, 0), f2(f1(0) ^ index_mask(index)));
            examples.insert((index, 0x1234_5678),
                f2(f1(0x1234_5678) ^ index_mask(index)));
        }
        let result = diagnose(&examples);
        assert_eq!(result.samples, 55);
        assert!(result.feature_rank > 32);
        assert_eq!(result.contradictory_samples, 0);
    }

    #[test]
    fn nonaffine_carry_is_detected_in_dependent_observations() {
        let mut examples = HashMap::new();
        for input in [0u32, 1, 2, 3, 4, 5, 6, 7] {
            examples.insert((0, input), input.wrapping_mul(3));
        }
        let result = diagnose(&examples);
        assert_eq!(result.samples, 8);
        assert_eq!(result.feature_rank, 3);
        assert!(result.contradictory_samples > 0);
    }

    #[test]
    fn same_samples_are_independent_of_hash_iteration_order() {
        let mut a = HashMap::new();
        let mut b = HashMap::new();
        let entries = [(0,0,5),(1,3,12),(2,5,41),(1,2,18),
                       (2,3,48),(1,1,12),(3,8,17),(0,7,45)];
        for &(i,x,y) in &entries { a.insert((i,x),y); }
        for &(i,x,y) in entries.iter().rev() { b.insert((i,x),y); }
        assert_eq!(diagnose(&a), diagnose(&b));
    }

    #[test]
    fn no_data_cannot_support_affinity() {
        assert_eq!(diagnose(&HashMap::new()), IndexedAffineAudit::default());
        let one=HashMap::from([((0,0), 123)]);
        assert_eq!(diagnose(&one),
            IndexedAffineAudit { samples:1, feature_rank:0, contradictory_samples:0 });
    }
}
