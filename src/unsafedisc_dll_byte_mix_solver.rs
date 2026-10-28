// Exact bounded initial-state inverse for the isolated original
// dplayerx.dll 19D45..19DF6 byte-mixer primitive. SYNTHETIC INPUTS ONLY.
// This does not implement Windows initialization, recover protected
// functions, or claim the historical loop runs consecutively.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap,HashMap};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExactMixerStateAudit {
    pub observations:usize,
    pub compatible_initial_states:u64,
    pub distinct_low16_carry_patterns:usize,
    /// Deterministic capped candidates: never a unique-key claim.
    pub example_initial_states:Vec<u32>,
}

fn xor_two_bytes(x:u16)->u8 {
    x as u8 ^ (x>>8) as u8
}

/// Solve the 32-bit initial state for a series of SYNTHETIC
/// destination, secondary source, and known output bytes under
/// exactly the isolated local primitive:
///
///   output = destination XOR secondary XOR xor_four_bytes(state)
///   state = state + output (mod 2^32).
///
/// Carries are handled EXACTLY, including low16 -> high16 and
/// high16 modular wraparound. We partition all 2^16 possible low
/// halves by their carry vectors for each known cumulative output.
/// For each distinct vector, a lookup over all 2^16 high halves
/// matches the 8-bit xor signatures. This is an exact meet-in-the-
/// middle calculation without enumerating all 2^32 states.
///
/// 1..=12 observations and <=64 examples bound CPU/memory work.
/// The returned COUNT is exact for this CONDITIONAL LOCAL MODEL,
/// NOT an original DLL state recovery; the true input buffers,
/// Windows-version state and executable branch schedule are unknown.
pub fn hypothetical_exact_mixer_initial_states(
    destination:&[u8],secondary:&[u8],outputs:&[u8],max_examples:usize,
)->Result<ExactMixerStateAudit,String>{
    let n=outputs.len();
    if n==0 || n>12 || destination.len()!=n || secondary.len()!=n {
        return Err("exact mixer inverse requires three equal 1..12 byte buffers".into());
    }
    if max_examples>64 {
        return Err("exact mixer inverse caps example state count at 64".into());
    }
    let mut prefix=Vec::with_capacity(n);
    let mut cumulative=0u32;
    for &output in outputs {
        prefix.push(cumulative);
        cumulative+=u32::from(output);
    }
    let observed:Vec<u8>=(0..n)
        .map(|i|destination[i]^secondary[i]^outputs[i]).collect();

    // All candidate low16 values sharing the same per-observation
    // high-half carry vector have the same high16 signature table.
    let mut low_groups=BTreeMap::<Vec<u16>,Vec<u16>>::new();
    for low in 0..=65535u32 {
        let carry:Vec<u16>=prefix.iter()
            .map(|&p|((low+p)>>16) as u16).collect();
        low_groups.entry(carry).or_default().push(low as u16);
    }
    let group_count=low_groups.len();
    let mut total=0u64;
    let mut examples=Vec::new();
    for (carry_vector,lows) in low_groups {
        // At most 12 observed XOR bytes fit into a stack-owned
        // u128 signature. Avoid allocating a Vec<u8> for EACH of
        // 65,536 high-half probes (especially costly in debug CI).
        let mut high_signatures=HashMap::<u128,Vec<u16>>::new();
        for high in 0..=65535u32 {
            let mut signature=0u128;
            for (i,&carry) in carry_vector.iter().enumerate() {
                let byte=xor_two_bytes(high.wrapping_add(u32::from(carry)) as u16);
                signature |= u128::from(byte) << (8*i);
            }
            high_signatures.entry(signature).or_default().push(high as u16);
        }
        for low in lows {
            let mut expected=0u128;
            for (i,&prefix_sum) in prefix.iter().enumerate() {
                let low_at=(u32::from(low)+prefix_sum) as u16;
                let byte=observed[i]^xor_two_bytes(low_at);
                expected |= u128::from(byte) << (8*i);
            }
            if let Some(highs)=high_signatures.get(&expected) {
                total+=highs.len() as u64;
                for &high in highs {
                    if examples.len()==max_examples {break;}
                    examples.push((u32::from(high)<<16)|u32::from(low));
                }
            }
        }
    }
    Ok(ExactMixerStateAudit{
        observations:n,compatible_initial_states:total,
        distinct_low16_carry_patterns:group_count,
        example_initial_states:examples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::unsafedisc_dll_byte_mix::conditional_byte_step;

    fn generate(initial:u32,outputs:&[u8])->(Vec<u8>,Vec<u8>){
        let mut state=initial;
        let mut secondary=Vec::new();
        let mut destination=Vec::new();
        for (i,&required) in outputs.iter().enumerate(){
            let dst=(i as u8).wrapping_mul(29).wrapping_add(11);
            let key=(state as u8)^((state>>8) as u8)^
                ((state>>16) as u8)^((state>>24) as u8);
            let other=dst^key^required;
            let (got,next)=conditional_byte_step(dst,other,state);
            assert_eq!(got,required);
            destination.push(dst);
            secondary.push(other);
            state=next;
        }
        (destination,secondary)
    }

    #[test]
    fn single_byte_has_exactly_2_to_24_compatible_initial_states(){
        let got=hypothetical_exact_mixer_initial_states(
            &[0x10],&[0x20],&[0x30],3).unwrap();
        assert_eq!(got.compatible_initial_states,1u64<<24);
        assert_eq!(got.distinct_low16_carry_patterns,1);
        assert_eq!(got.example_initial_states.len(),3);
        assert_eq!(got.observations,1);
    }

    #[test]
    fn state_with_low_byte_carry_is_compatible_with_exact_solver(){
        let desired=[200u8,35,128,15,31,7,99,1];
        let initial=0xAABB_CCFAu32;
        let (destination,secondary)=generate(initial,&desired);
        let result=hypothetical_exact_mixer_initial_states(
            &destination,&secondary,&desired,8).unwrap();
        assert!(result.compatible_initial_states>0);
        assert!(result.distinct_low16_carry_patterns>=2);
        assert!(result.example_initial_states.len()<=8);
        for &candidate in &result.example_initial_states{
            let (mut state,mut good)=(candidate,true);
            for i in 0..desired.len(){
                let (out,next)=conditional_byte_step(destination[i],secondary[i],state);
                if out!=desired[i] {good=false;break;}
                state=next;
            }
            assert!(good,"solver produced incompatible sample state");
        }
    }

    #[test]
    fn state_with_low16_carry_is_compatible_with_exact_solver(){
        let desired=[40u8,72,5,17,18,29];
        let initial=0x1234_FFF0u32;
        let (destination,secondary)=generate(initial,&desired);
        let result=hypothetical_exact_mixer_initial_states(
            &destination,&secondary,&desired,5).unwrap();
        assert!(result.compatible_initial_states>0);
        assert!(result.distinct_low16_carry_patterns>=2);
    }

    #[test]
    fn state_wrap_at_u32_max_is_represented_by_high16_modular_add(){
        let desired=[80u8,120,9,13,77,8];
        let initial=0xFFFF_FFF8u32;
        let (destination,secondary)=generate(initial,&desired);
        let exact=hypothetical_exact_mixer_initial_states(
            &destination,&secondary,&desired,12).unwrap();
        assert!(exact.compatible_initial_states>0);
        assert!(exact.distinct_low16_carry_patterns>=2);
        for candidate in exact.example_initial_states {
            let mut state=candidate;
            for i in 0..desired.len() {
                let (out,next)=conditional_byte_step(destination[i],secondary[i],state);
                assert_eq!(out,desired[i]);
                state=next;
            }
        }
    }

    #[test]
    fn two_inconsistent_zero_output_measurements_have_no_solution(){
        let result=hypothetical_exact_mixer_initial_states(
            &[0,0],&[7,9],&[0,0],5).unwrap();
        assert_eq!(result.compatible_initial_states,0);
        assert!(result.example_initial_states.is_empty());
    }

    #[test]
    fn exact_count_contains_every_state_from_no_carry_special_case(){
        // Two mathematically INDEPENDENT solvers on identical,
        // generated synthetic observations. Every zero-carry state
        // must also be accepted by the exact carry-aware solver.
        let outputs=[3u8,5,1,4];
        let (destination,secondary)=generate(0x1234_5610,&outputs);
        let restricted=
            super::super::unsafedisc_dll_byte_mix::hypothetical_no_carry_initial_states(
                &destination,&secondary,&outputs).unwrap();
        let exact=hypothetical_exact_mixer_initial_states(
            &destination,&secondary,&outputs,0).unwrap();
        assert!(!restricted.candidates.is_empty());
        assert!(exact.compatible_initial_states>=
            restricted.equivalent_full_initial_states);
        assert!(exact.example_initial_states.is_empty());
    }

    #[test]
    fn identical_zero_increment_observations_cannot_create_information(){
        let single=hypothetical_exact_mixer_initial_states(&[0],&[7],&[0],0)
            .unwrap();
        let repeated=hypothetical_exact_mixer_initial_states(
            &[0,0,0],&[7,7,7],&[0,0,0],0).unwrap();
        assert_eq!(single.compatible_initial_states,1u64<<24);
        assert_eq!(repeated.compatible_initial_states,1u64<<24);
    }

    #[test]
    fn limits_are_enforced_and_zero_example_count_preserves_full_result(){
        assert!(hypothetical_exact_mixer_initial_states(&[],&[],&[],1).is_err());
        assert!(hypothetical_exact_mixer_initial_states(&[1],&[],&[1],1).is_err());
        assert!(hypothetical_exact_mixer_initial_states(
            &vec![0;13],&vec![0;13],&vec![0;13],1).is_err());
        assert!(hypothetical_exact_mixer_initial_states(&[0],&[0],&[0],65).is_err());
        let a=hypothetical_exact_mixer_initial_states(&[0],&[0],&[0],0).unwrap();
        assert!(a.example_initial_states.is_empty());
        assert_eq!(a.compatible_initial_states,1u64<<24);
    }
}
