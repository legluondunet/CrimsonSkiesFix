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

// Original index masks alias: mask(1) == mask(257) == 0x01010101.
// The actual extra layer is indexed by its *mask value*, not by the
// integer index as an independent family. Canonicalizing across distinct
// indices tests an additional necessary deterministic/injective property.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MaskAliasAudit {
    pub samples: usize,
    pub distinct_masks: usize,
    pub masks_with_multiple_indices: usize,
    pub distinct_masked_input_pairs: usize,
    pub repeated_masked_input_pairs: usize,
    pub forward_conflicts: usize,
    pub inverse_conflicts: usize,
}

pub fn audit_mask_aliases(pairs: &HashMap<(u32, u32), u32>) -> MaskAliasAudit {
    let mut ordered: Vec<_> = pairs.iter()
        .map(|(&(index, input), &output)| (index, input, output))
        .collect();
    ordered.sort_unstable();
    let mut audit = MaskAliasAudit {
        samples: ordered.len(), ..MaskAliasAudit::default()
    };
    let mut masks = HashMap::<u32, u32>::new();
    let mut aliased = std::collections::HashSet::<u32>::new();
    let mut forward = HashMap::<(u32, u32), u32>::new();
    let mut inverse = HashMap::<(u32, u32), u32>::new();
    for (index, input, output) in ordered {
        let mask=index_mask(index);
        if let Some(&prior) = masks.get(&mask) {
            if prior!=index { aliased.insert(mask); }
        } else {
            masks.insert(mask,index);
        }
        let key=(mask,input);
        if let Some(&prior_output) = forward.get(&key) {
            audit.repeated_masked_input_pairs+=1;
            if prior_output!=output { audit.forward_conflicts+=1; }
        } else {
            forward.insert(key,output);
        }
        if let Some(&prior_input) = inverse.get(&(mask,output)) {
            if prior_input!=input { audit.inverse_conflicts+=1; }
        } else {
            inverse.insert((mask,output),input);
        }
    }
    audit.distinct_masks=masks.len();
    audit.masks_with_multiple_indices=aliased.len();
    audit.distinct_masked_input_pairs=forward.len();
    audit
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
        assert_eq!(index_mask(0x3EE), 0xEFEF_EFEE);
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
    fn full_width_masks_are_not_injective_above_index_255() {
        assert_eq!(index_mask(1),index_mask(257));
        assert_eq!(index_mask(3),index_mask(259));
        assert_eq!(index_mask(3),index_mask(515));
        assert_eq!(index_mask(3),index_mask(771));
        let mut families=HashMap::<u32, usize>::new();
        for index in 0..=1006u32 {
            *families.entry(index_mask(index)).or_default()+=1;
        }
        assert_eq!(families.len(),572);
        assert_eq!(families.values().filter(|&&count|count>=2).count(),312);
        // For 0 <= index < 1024, write index = a + 256*b, with
        // a in 0..255 and b in 0..3. Each mask consists of three
        // high bytes (a|b) and one low byte a, hence only 576
        // different masks in the entire 1024-index domain.
        let full: std::collections::HashSet<u32> =
            (0..1024u32).map(index_mask).collect();
        assert_eq!(full.len(),576);
        for index in 0..1024u32 {
            let a=index & 0xFF;
            let b=index >> 8;
            let high=a | b;
            assert_eq!(index_mask(index),
                (high << 24) | (high << 16) | (high << 8) | a);
        }
    }

    #[test]
    fn equal_masks_must_produce_equal_results_for_equal_inputs() {
        let pairs=HashMap::from([
            ((1u32,123u32),7u32),((257,123),7),((3,44),19),((259,44),19)
        ]);
        let audit=audit_mask_aliases(&pairs);
        assert_eq!(audit.samples,4);
        assert_eq!(audit.distinct_masks,2);
        assert_eq!(audit.masks_with_multiple_indices,2);
        assert_eq!(audit.distinct_masked_input_pairs,2);
        assert_eq!(audit.repeated_masked_input_pairs,2);
        assert_eq!(audit.forward_conflicts,0);
        assert_eq!(audit.inverse_conflicts,0);
        let bad=HashMap::from([
            ((1,123),7),((257,123),8),
            ((3,100),19),((259,101),19)
        ]);
        let report=audit_mask_aliases(&bad);
        assert_eq!(report.forward_conflicts,1);
        assert_eq!(report.inverse_conflicts,1);
    }

    #[test]
    fn mask_alias_audit_is_insertion_order_independent() {
        let entries=[((1,101),7),((257,101),8),((3,9),12),((259,10),12)];
        let a=HashMap::from(entries);
        let mut b=HashMap::new();
        for &(key,value) in entries.iter().rev() {b.insert(key,value);}
        assert_eq!(audit_mask_aliases(&a),audit_mask_aliases(&b));
    }

    #[test]
    fn no_data_cannot_support_affinity() {
        assert_eq!(diagnose(&HashMap::new()), IndexedAffineAudit::default());
        let one=HashMap::from([((0,0), 123)]);
        assert_eq!(diagnose(&one),
            IndexedAffineAudit { samples:1, feature_rank:0, contradictory_samples:0 });
    }
}
