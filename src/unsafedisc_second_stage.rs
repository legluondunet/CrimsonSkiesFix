// Synthetic-tested, aggregate-only second-stage EAX differential probe.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The historical extra layer yields y = F2(F1(x) XOR m).
// If F2 (alone) is affine over GF(2), holding x constant gives
// y(x,m) XOR y(x,n) = L(m XOR n), independent of F1 or its nonlinearity.
// This is a falsifiable *necessary* condition, not a decoder or a means
// of predicting missing output values without further validation.

use std::collections::HashMap;

use super::unsafedisc_indexed_affine::index_mask;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SecondStageAudit {
    pub raw_observations: usize,
    pub canonical_observations: usize,
    pub conflicting_canonical_pairs: usize,
    pub multi_mask_inputs: usize,
    pub differential_observations: usize,
    pub differential_rank: usize,
    /// Independent XOR rank of the associated output differences.
    pub output_differential_rank: usize,
    /// Conditional necessary test for an INJECTIVE and GF(2)-affine F2:
    /// a linear map cannot lose rank on the observed mask subspace.
    /// Not a claim that the opaque original F2 is affine or bijective.
    pub injective_affine_rank_deficit: usize,
    pub differential_contradictions: usize,
    pub distinct_mask_difference_classes: usize,
    pub repeated_mask_difference_classes: usize,
    pub conflicting_mask_difference_classes: usize,
    pub distinct_mask_output_collisions: usize,
    pub distinct_masks_observed: usize,
}

/// Canonicalize each (input, mask) before comparing *different* masks.
/// Mask aliases do not provide differential evidence and are counted
/// separately by the existing mask-alias audit. Sorting keeps the
/// result independent of hash-table iteration order.
pub fn diagnose(samples: &HashMap<(u32, u32), u32>) -> SecondStageAudit {
    let mut ordered: Vec<(u32, u32, u32)> = samples.iter()
        .map(|(&(index, input), &output)| (input, index_mask(index), output))
        .collect();
    ordered.sort_unstable();
    let mut report=SecondStageAudit {
        raw_observations: ordered.len(), ..SecondStageAudit::default()
    };
    let mut canonical=Vec::with_capacity(ordered.len());
    for (input,mask,output) in ordered {
        if let Some(&(old_input,old_mask,old_output))=canonical.last() {
            if old_input==input && old_mask==mask {
                if old_output!=output { report.conflicting_canonical_pairs+=1; }
                continue;
            }
        }
        canonical.push((input,mask,output));
    }
    report.canonical_observations=canonical.len();

    let mut mask_set=std::collections::HashSet::new();
    for &(_,mask,_) in &canonical {mask_set.insert(mask);}
    report.distinct_masks_observed=mask_set.len();

    // Each relation supplies a constraint (mask XOR mask0) ->
    // (output XOR output0). A 32-vector basis checks consistency
    // of all such equations without recovering/publishing F2.
    let mut pivots:[Option<(u32,u32)>;32]=[None;32];
    let mut output_basis=[0u32;32];
    // An especially readable F2-only counterexample is when two
    // observations use the same mask difference but DIFFERENT output
    // differences. Such an equality conflict alone refutes an affine
    // second modifier without reconstructing any basis coefficient.
    let mut seen_delta=HashMap::<u32,u32>::new();
    let mut repeated_delta=std::collections::HashSet::<u32>::new();
    let mut contradictory_delta=std::collections::HashSet::<u32>::new();
    let mut cursor=0usize;
    while cursor<canonical.len() {
        let begin=cursor;
        let input=canonical[cursor].0;
        while cursor<canonical.len() && canonical[cursor].0==input {
            cursor+=1;
        }
        if cursor-begin<2 {continue;}
        report.multi_mask_inputs+=1;
        let (_,base_mask,base_output)=canonical[begin];
        let mut outputs=HashMap::<u32,u32>::new();
        for &(_,mask,output) in &canonical[begin..cursor] {
            if let Some(&prior_mask)=outputs.get(&output) {
                if prior_mask!=mask {
                    report.distinct_mask_output_collisions+=1;
                }
            } else {
                outputs.insert(output,mask);
            }
        }
        for &(_,mask,output) in &canonical[begin+1..cursor] {
            report.differential_observations+=1;
            let delta_mask=mask ^ base_mask;
            let delta_out=output ^ base_output;
            let mut output_vector=delta_out;
            for bit in (0..32).rev(){
                if output_vector & (1u32<<bit)==0 {continue;}
                if output_basis[bit]!=0{output_vector^=output_basis[bit];}
                else {
                    output_basis[bit]=output_vector;
                    report.output_differential_rank+=1;
                    break;
                }
            }
            if let Some(&prior_out)=seen_delta.get(&delta_mask) {
                repeated_delta.insert(delta_mask);
                if prior_out!=delta_out {
                    contradictory_delta.insert(delta_mask);
                }
            } else {
                seen_delta.insert(delta_mask,delta_out);
            }
            let mut dx=delta_mask;
            let mut dy=delta_out;
            let mut independent=false;
            for bit in (0..32).rev() {
                if dx & (1u32 << bit)==0 {continue;}
                if let Some((px,py))=pivots[bit] {
                    dx^=px;
                    dy^=py;
                } else {
                    pivots[bit]=Some((dx,dy));
                    report.differential_rank+=1;
                    independent=true;
                    break;
                }
            }
            if !independent && dy!=0 {report.differential_contradictions+=1;}
        }
    }
    if report.differential_contradictions==0 {
        report.injective_affine_rank_deficit=report.differential_rank
            .saturating_sub(report.output_differential_rank);
    }
    report.distinct_mask_difference_classes=seen_delta.len();
    report.repeated_mask_difference_classes=repeated_delta.len();
    report.conflicting_mask_difference_classes=contradictory_delta.len();
    report
}

/// Calculate the linear span of all original masks in a bounded index
/// range. Rank on 0..=1006 is 10, not 32: consequently a joint
/// "32 input bits + 32 mask bits" feature matrix cannot have rank >42.
pub fn original_mask_rank(last_index:u32)->usize {
    let mut pivots=[0u32;32];
    let mut rank=0;
    for i in 0..=last_index {
        let mut x=index_mask(i);
        for bit in (0..32).rev() {
            if x & (1u32<<bit)==0 {continue;}
            if pivots[bit]!=0 {
                x^=pivots[bit];
            } else {
                pivots[bit]=x;
                rank+=1;
                break;
            }
        }
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_index_range_has_mask_rank_ten() {
        assert_eq!(original_mask_rank(0),0);
        assert_eq!(original_mask_rank(255),8);
        assert_eq!(original_mask_rank(256),9);
        assert_eq!(original_mask_rank(511),9);
        assert_eq!(original_mask_rank(512),10);
        assert_eq!(original_mask_rank(1006),10);
        assert_eq!(original_mask_rank(1023),10);
    }

    #[test]
    fn arbitrary_nonlinear_first_stage_does_not_invalidate_affine_second_stage() {
        let f1=|x:u32| x.wrapping_mul(0x03AB_5791).rotate_left(13);
        let f2=|x:u32| x.rotate_left(7)^0xFAC3_1298;
        let mut samples=HashMap::new();
        for x in [0u32,1,3,0x0123_4567,0xFFFF_FFFF] {
            for i in 0..16u32 {
                let m=index_mask(i);
                samples.insert((i,x),f2(f1(x)^m));
            }
        }
        let r=diagnose(&samples);
        assert_eq!(r.raw_observations,80);
        assert_eq!(r.multi_mask_inputs,5);
        assert_eq!(r.differential_observations,75);
        assert_eq!(r.differential_rank,4);
        assert_eq!(r.differential_contradictions,0);
    }

    #[test]
    fn detect_nonlinear_second_stage_without_any_assumption_about_first() {
        let f2=|v:u32| v ^ (((v&1) & ((v>>1)&1))<<22);
        let mut pairs=HashMap::new();
        for i in 0..4u32 {
            let m=index_mask(i);
            pairs.insert((i,0),f2(m));
        }
        let r=diagnose(&pairs);
        assert_eq!(r.multi_mask_inputs,1);
        assert_eq!(r.differential_rank,2);
        assert!(r.differential_contradictions>=1);
    }

    #[test]
    fn direct_matching_mask_deltas_with_incompatible_output_deltas() {
        let pairs=HashMap::from([
            // Two distinct inputs (11 and 22) are both seen under the
            // same pair of masks. Their output-difference signatures
            // disagree, directly falsifying the F2-affine hypothesis.
            ((0,11),100), ((1,11),101),
            ((0,22),200), ((1,22),202),
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.multi_mask_inputs,2);
        assert_eq!(r.differential_observations,2);
        assert_eq!(r.distinct_mask_difference_classes,1);
        assert_eq!(r.repeated_mask_difference_classes,1);
        assert_eq!(r.conflicting_mask_difference_classes,1);
        assert_eq!(r.differential_rank,1);
        assert_eq!(r.differential_contradictions,1);
    }

    #[test]
    fn matching_mask_deltas_with_matching_outputs_are_compatible() {
        let pairs=HashMap::from([
            ((0,11),100), ((1,11),101),
            ((0,22),200), ((1,22),201),
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.distinct_mask_difference_classes,1);
        assert_eq!(r.repeated_mask_difference_classes,1);
        assert_eq!(r.conflicting_mask_difference_classes,0);
        assert_eq!(r.differential_contradictions,0);
    }

    #[test]
    fn rank_deficit_excludes_affine_bijective_f2_without_output_collisions(){
        // Three independent mask deltas mapped to rank-two output
        // deltas. All four observed outputs remain DISTINCT and the
        // affine consistency equations are satisfiable. This rules
        // out an injective affine F2 even though a simple duplicate-
        // output collision test would miss the problem.
        let pairs=HashMap::from([
            ((0u32,9u32),0u32), ((1,9),1),
            ((2,9),2), ((4,9),3),
        ]);
        let report=diagnose(&pairs);
        assert_eq!(report.differential_rank,3);
        assert_eq!(report.output_differential_rank,2);
        assert_eq!(report.injective_affine_rank_deficit,1);
        assert_eq!(report.differential_contradictions,0);
        assert_eq!(report.distinct_mask_output_collisions,0);
    }

    #[test]
    fn bijective_affine_second_stage_retains_mask_subspace_rank(){
        let mut pairs=HashMap::new();
        for index in 0..64u32 {
            pairs.insert((index,1234),index_mask(index).rotate_left(9)^0xABCD_1234);
        }
        let report=diagnose(&pairs);
        assert_eq!(report.differential_rank,6);
        assert_eq!(report.output_differential_rank,6);
        assert_eq!(report.injective_affine_rank_deficit,0);
        assert_eq!(report.differential_contradictions,0);
    }

    #[test]
    fn no_cross_mask_reuse_is_inconclusive() {
        let samples=HashMap::from([
            ((0,11),42),((1,22),43),((2,33),44)
        ]);
        let report=diagnose(&samples);
        assert_eq!(report.multi_mask_inputs,0);
        assert_eq!(report.differential_observations,0);
        assert_eq!(report.differential_rank,0);
        assert_eq!(report.differential_contradictions,0);
    }

    #[test]
    fn colliding_indices_do_not_count_as_independent_mask_differences() {
        let samples=HashMap::from([
            ((1,123),50),((257,123),50),((3,123),51),
            ((259,123),51)
        ]);
        let r=diagnose(&samples);
        assert_eq!(r.canonical_observations,2);
        assert_eq!(r.distinct_masks_observed,2);
        assert_eq!(r.multi_mask_inputs,1);
        assert_eq!(r.differential_observations,1);
        assert_eq!(r.differential_contradictions,0);
        assert_eq!(r.conflicting_canonical_pairs,0);
    }

    #[test]
    fn conflicting_aliases_are_counted_and_cannot_be_hidden() {
        let samples=HashMap::from([
            ((1,123),50),((257,123),52),((3,123),54)
        ]);
        let r=diagnose(&samples);
        assert_eq!(r.conflicting_canonical_pairs,1);
        assert_eq!(r.canonical_observations,2);
    }

    #[test]
    fn same_source_distinct_mask_output_collision_rejects_f2_injectivity() {
        // This extra necessary condition assumes only that F2 is
        // injective; F1 remains entirely unrestricted.
        let pairs=HashMap::from([
            ((0,123),77), ((1,123),77), ((2,123),79)
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.multi_mask_inputs,1);
        assert_eq!(r.distinct_mask_output_collisions,1);
        assert_eq!(r.differential_observations,2);
    }

    #[test]
    fn an_affine_but_noninjective_f2_may_have_output_collisions() {
        // Bit-affinity does NOT imply bijectivity. Keep these two
        // independent necessary conditions separate.
        let mut samples=HashMap::new();
        for i in 0..8u32 {
            samples.insert((i,1234), index_mask(i)&!0x0101_0101);
        }
        let r=diagnose(&samples);
        assert_eq!(r.differential_contradictions,0);
        assert!(r.distinct_mask_output_collisions>0);
    }

    #[test]
    fn deterministic_results_for_reordered_input() {
        let samples=[
            ((0,11),0),((1,11),3),((2,11),7),((3,11),22),
            ((0,14),1),((1,14),2),((257,14),2),
            ((512,14),90),((0,21),44)
        ];
        let a=HashMap::from(samples);
        let mut b=HashMap::new();
        for &(key,value) in samples.iter().rev(){b.insert(key,value);}
        assert_eq!(diagnose(&a),diagnose(&b));
    }
}
