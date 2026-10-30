// Aggregate-only, conditional F1 equal-output XOR graph research.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// y(x,m) = F2(F1(x) XOR m). IF F2 is injective, equal observed y
// requires F1(x0) XOR F1(x1) = m0 XOR m1. No affinity assumption
// about F2, and the graph is NOT a recovered F1 implementation.

use std::collections::{HashMap,HashSet};
use super::unsafedisc_indexed_affine::index_mask;

#[derive(Debug,Default,Clone,Copy,PartialEq,Eq)]
pub struct FirstStageAudit {
    pub raw_observations: usize,
    pub canonical_observations: usize,
    pub canonical_conflicts: usize,
    pub multi_context_outputs: usize,
    pub xor_relations: usize,
    pub graph_vertices: usize,
    pub graph_components: usize,
    pub largest_component: usize,
    pub compatible_cycles: usize,
    pub contradictory_cycles: usize,
    pub affine_input_rank: usize,
    pub augmented_rank: usize,
    pub repeated_input_delta_classes: usize,
    pub contradictory_input_delta_classes: usize,
    pub rotate_then_add_lsb_candidates: usize,
    pub add_then_rotate_lsb_candidates: usize,
}

fn add_rank(mut x:u64,basis:&mut [u64;64])->bool {
    for bit in (0..64).rev() {
        if x&(1u64<<bit)==0 {continue;}
        if basis[bit]!=0{x^=basis[bit];}
        else {basis[bit]=x;return true;}
    }
    false
}

#[derive(Default)]
struct XorGraph {
    nodes:HashMap<u32,usize>,
    parent:Vec<usize>,
    weight:Vec<u32>,
    size:Vec<usize>,
    compatible_cycles:usize,
    contradictory_cycles:usize,
}

impl XorGraph {
    fn node(&mut self,x:u32)->usize {
        if let Some(&id)=self.nodes.get(&x){return id;}
        let id=self.parent.len();
        self.nodes.insert(x,id);
        self.parent.push(id);
        self.weight.push(0);
        self.size.push(1);
        id
    }

    // XOR potential between a vertex and its current set root.
    fn find(&mut self,x:usize)->(usize,u32){
        if self.parent[x]==x{return(x,0);}
        let old=self.parent[x];
        let (root,parent_weight)=self.find(old);
        self.weight[x]^=parent_weight;
        self.parent[x]=root;
        (root,self.weight[x])
    }

    fn link(&mut self,x:u32,y:u32,delta:u32){
        let a=self.node(x);let b=self.node(y);
        let (mut ra,wa)=self.find(a);
        let (mut rb,wb)=self.find(b);
        if ra==rb {
            if (wa^wb)==delta {self.compatible_cycles+=1;}
            else {self.contradictory_cycles+=1;}
            return;
        }
        if self.size[ra]<self.size[rb]{std::mem::swap(&mut ra,&mut rb);}
        self.parent[rb]=ra;
        self.weight[rb]=wa^wb^delta;
        self.size[ra]+=self.size[rb];
    }

    fn summarize(&mut self)->(usize,usize,usize){
        let vertices=self.parent.len();
        let mut roots=HashSet::new();
        for x in 0..vertices{roots.insert(self.find(x).0);}
        let biggest=roots.iter().map(|&x|self.size[x]).max().unwrap_or(0);
        (vertices,roots.len(),biggest)
    }
}

pub fn diagnose(pairs:&HashMap<(u32,u32),u32>)->FirstStageAudit{
    let mut raw:Vec<_>=pairs.iter()
        .map(|(&(index,input),&output)| (input,index_mask(index),output))
        .collect();
    raw.sort_unstable();
    let mut audit=FirstStageAudit {
        raw_observations:raw.len(), ..FirstStageAudit::default()
    };
    let mut canonical=Vec::<(u32,u32,u32)>::new();
    for (input,mask,output) in raw {
        if let Some(&(px,pm,py))=canonical.last(){
            if px==input&&pm==mask{
                if py!=output{audit.canonical_conflicts+=1;}
                continue;
            }
        }
        canonical.push((input,mask,output));
    }
    audit.canonical_observations=canonical.len();
    let mut by_output:Vec<_>=canonical.iter()
        .map(|&(x,m,y)|(y,x,m)).collect();
    by_output.sort_unstable();
    let mut i=0usize;
    let mut linear_basis=[0u64;64];
    let mut augmented_basis=[0u64;64];
    let mut delta_outputs=HashMap::<u32,u32>::new();
    let mut repeated=HashSet::<u32>::new();
    let mut conflicting=HashSet::<u32>::new();
    let mut graph=XorGraph::default();
    let mut relations=Vec::<(u32,u32)>::new();
    while i<by_output.len(){
        let begin=i;
        let y=by_output[i].0;
        while i<by_output.len() && by_output[i].0==y {i+=1;}
        if i-begin<2{continue;}
        audit.multi_context_outputs+=1;
        let (_,x0,m0)=by_output[begin];
        for &(_,x,m) in &by_output[begin+1..i]{
            let dx=x^x0;
            let dm=m^m0;
            audit.xor_relations+=1;
            relations.push((dx,dm));
            if add_rank(u64::from(dx),&mut linear_basis) {audit.affine_input_rank+=1;}
            if add_rank((u64::from(dx)<<32)|u64::from(dm),
                        &mut augmented_basis) {audit.augmented_rank+=1;}
            if let Some(&old)=delta_outputs.get(&dx) {
                repeated.insert(dx);
                if old!=dm{conflicting.insert(dx);}
            }else{delta_outputs.insert(dx,dm);}
            graph.link(x0,x,dm);
        }
    }
    audit.repeated_input_delta_classes=repeated.len();
    audit.contradictory_input_delta_classes=conflicting.len();
    if !relations.is_empty() {
        // Only necessary conditions, but independent of unknown
        // additive offset k. For F1(x)=rotl(x,r)+k, the lowest
        // output XOR bit equals the lowest rotl input XOR bit:
        // carries cannot affect bit zero. For rotl(x+k,r),
        // output bit r corresponds to input bit zero.
        for rotate in 0..32u32 {
            let source_bit=(32-rotate)%32;
            if relations.iter().all(|&(dx,dm)|
                    (dm&1)==((dx>>source_bit)&1)) {
                audit.rotate_then_add_lsb_candidates+=1;
            }
            if relations.iter().all(|&(dx,dm)|
                    ((dm>>rotate)&1)==(dx&1)) {
                audit.add_then_rotate_lsb_candidates+=1;
            }
        }
    }
    let (vertices,components,biggest)=graph.summarize();
    audit.graph_vertices=vertices;
    audit.graph_components=components;
    audit.largest_component=biggest;
    audit.compatible_cycles=graph.compatible_cycles;
    audit.contradictory_cycles=graph.contradictory_cycles;
    audit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forest_of_equal_outputs_encodes_valid_identity_first_stage(){
        // Each output group uses F1(x)=x and the same x XOR mask,
        // with an arbitrary injective F2 mapping each preimage.
        let pairs=HashMap::from([
            ((0,0u32),77u32),
            ((1,0x0101_0101),77),
            ((2,0x0202_0202),77),
            ((0,3),55),
            ((1,3^0x0101_0101),55),
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.multi_context_outputs,2);
        assert_eq!(r.xor_relations,3);
        assert_eq!(r.graph_vertices,5);
        assert_eq!(r.graph_components,2);
        assert_eq!(r.largest_component,3);
        assert_eq!(r.contradictory_cycles,0);
        assert_eq!(r.affine_input_rank,r.augmented_rank);
        assert_eq!(r.contradictory_input_delta_classes,0);
    }

    #[test]
    fn incompatible_identical_input_deltas_reject_conditional_affinity(){
        let pairs=HashMap::from([
            ((0,0u32),10u32),((1,3),10),
            ((0,8),20),((2,11),20)
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.xor_relations,2);
        assert_eq!(r.affine_input_rank,1);
        assert_eq!(r.augmented_rank,2);
        assert_eq!(r.repeated_input_delta_classes,1);
        assert_eq!(r.contradictory_input_delta_classes,1);
        assert_eq!(r.contradictory_cycles,0);
    }

    #[test]
    fn cyclic_incompatible_xor_edges_detect_bad_injectivity_assumption(){
        let pairs=HashMap::from([
            ((0,1u32),50u32),((1,2),50),
            ((2,1),60),((4,2),60)
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.graph_vertices,2);
        assert_eq!(r.graph_components,1);
        assert_eq!(r.contradictory_cycles,1);
    }

    #[test]
    fn mask_alias_is_not_a_new_relation(){
        let pairs=HashMap::from([
            ((1,123u32),50u32),((257,123),50),
            ((3,124),50)
        ]);
        let r=diagnose(&pairs);
        assert_eq!(r.raw_observations,3);
        assert_eq!(r.canonical_observations,2);
        assert_eq!(r.xor_relations,1);
        assert_eq!(r.canonical_conflicts,0);
    }

    #[test]
    fn fixed_result_is_independent_of_hash_iteration_order(){
        let entries=[
            ((0,1u32),4u32),((1,3),4),((2,7),4),
            ((0,11),12),((1,15),12),((2,19),12)
        ];
        let a=HashMap::from(entries);
        let mut b=HashMap::new();
        for &(key,value) in entries.iter().rev(){b.insert(key,value);}
        assert_eq!(diagnose(&a),diagnose(&b));
    }

    #[test]
    fn known_identity_first_stage_passes_both_lsb_necessary_families(){
        let samples=HashMap::from([
            ((0,0u32),10u32),((1,0x0101_0101),10),
            ((0,3),20),((2,3^0x0202_0202),20),
        ]);
        let report=diagnose(&samples);
        assert!(report.rotate_then_add_lsb_candidates>=1);
        assert!(report.add_then_rotate_lsb_candidates>=1);
    }

    #[test]
    fn no_edges_do_not_vacuously_establish_any_rotation_family(){
        let samples=HashMap::from([((0,101),11),((1,102),12)]);
        let report=diagnose(&samples);
        assert_eq!(report.rotate_then_add_lsb_candidates,0);
        assert_eq!(report.add_then_rotate_lsb_candidates,0);
    }

    #[test]
    fn empty_and_singleton_inputs_have_no_graph_edges(){
        assert_eq!(diagnose(&HashMap::new()),FirstStageAudit::default());
        let p=HashMap::from([((0,123),999)]);
        let audit=diagnose(&p);
        assert_eq!(audit.xor_relations,0);
        assert_eq!(audit.graph_vertices,0);
        assert_eq!(audit.multi_context_outputs,0);
    }
}
