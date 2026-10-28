// An instruction-backed, *conditional* byte transform in the readable
// .txt2 helper 19C78. Not a reconstruction of the complete SafeDisc
// .txt decoder or an executable Windows code path.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target};

pub const HELPER_CALL_RVA:u32=0x19798;
pub const HELPER_RVA:u32=0x19C78;
pub const STATE_PREFERRED_VA:u32=0x0090_5010;

/// When the genuine instruction run 19D45..19DF6 executes as decoded,
/// the *single-byte* operation is equivalent to this expression:
///   out = dst ^ src ^ state.byte0 ^ byte1 ^ byte2 ^ byte3
///   state = state + out (mod 2^32)
/// Both destination and separate input pointers then advance by one.
/// This does not claim recovery of upstream state initialization,
/// loop exit conditions or the full opaque helper's effects.
pub fn conditional_byte_step(
    destination_byte:u8,second_buffer_byte:u8,state:u32
)->(u8,u32) {
    let key_xor=(state as u8)^((state>>8) as u8)^
        ((state>>16) as u8)^((state>>24) as u8);
    let output=destination_byte^second_buffer_byte^key_xor;
    (output,state.wrapping_add(u32::from(output)))
}

/// SYNTHETIC-ONLY composition of consecutive instances of the
/// verified local byte primitive. This does NOT assert that 19C78
/// necessarily loops over a contiguous block, or that other original
/// instructions cannot update the global between iterations.
/// Distinct source and destination slices avoid any inferred aliasing.
pub fn hypothetical_consecutive_steps(
    destination:&[u8],second_buffer:&[u8],initial_state:u32
)->Option<(Vec<u8>,u32)> {
    if destination.len()!=second_buffer.len() {return None;}
    let mut state=initial_state;
    let mut output=Vec::with_capacity(destination.len());
    for (&dst,&other) in destination.iter().zip(second_buffer) {
        let (byte,next)=conditional_byte_step(dst,other,state);
        output.push(byte);
        state=next;
    }
    Some((output,state))
}

/// Mathematical invariant for *only* the consecutive byte steps:
/// state_after = state_before + SUM(output_byte) (mod 2^32).
/// Thus an observed final state plus every output byte could
/// reconstruct the PRE-STEP state. No actual original output/state
/// bytes are claimed to be known by this helper.
pub fn hypothetical_initial_state(final_state:u32,outputs:&[u8])->u32 {
    outputs.iter().fold(final_state,
        |state,&output|state.wrapping_sub(u32::from(output)))
}

/// A reduced STATE-family candidate for the independently audited
/// 19D45..19DF6 byte primitive, conditional on there being NO carry
/// from its least significant state byte into byte 1 throughout the
/// entire observed buffer. This is not a recovered original state.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct NoCarryMixerCandidate {
    pub initial_low_byte:u8,
    /// XOR of the three unknown upper initial state bytes. The upper
    /// bytes themselves are NOT identified by this constraint.
    pub initial_upper_three_xor:u8,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NoCarryMixerAudit {
    pub observations:usize,
    pub output_byte_sum:u64,
    pub candidates:Vec<NoCarryMixerCandidate>,
    /// Each candidate represents 2^16 distinct full 32-bit initial
    /// states, as any pair of upper bytes determines the third via XOR.
    pub equivalent_full_initial_states:u64,
}

/// For verified SYNTHETIC output bytes only, solve the no-carry subset
/// of initial states without brute-forcing all 2^32 combinations.
///
/// Historical local step:
///   out_i = dst_i ^ other_i ^ XOR_BYTES(state_i)
///   state_(i+1) = state_i + out_i mod 2^32
///
/// If the low state byte never carries into the next state byte,
/// write state_i.low = low0 + SUM(out_0..out_(i-1)).
/// Then XOR_BYTES(state_i) = state_i.low ^ upper_three_xor,
/// so each observation constrains the same unknown upper-three-XOR.
///
/// Empty result means *only* that no initial state in this restricted
/// no-carry family fits; other valid states with a carry may exist.
/// An output sum >255 makes the restricted family empty automatically.
/// Never execute Windows DLL code, and never infer upstream state,
/// second buffer contents or the true encrypted .txt decode schedule.
pub fn hypothetical_no_carry_initial_states(
    destination:&[u8],second_buffer:&[u8],known_output:&[u8],
)->Result<NoCarryMixerAudit>{
    if destination.len()!=second_buffer.len() ||
       destination.len()!=known_output.len(){
        return Err("byte-mixer observations have different lengths".into());
    }
    if destination.is_empty() || destination.len()>4096{
        return Err("no-carry diagnostic requires 1..4096 observed bytes".into());
    }
    let total: u64=known_output.iter().map(|&b|u64::from(b)).sum();
    let mut candidates=Vec::new();
    if total<=255 {
        for low in 0..=255u16{
            if u64::from(low)+total>255 {continue;}
            let upper=destination[0]^second_buffer[0]^known_output[0]^(low as u8);
            let mut sum=0u16;
            let mut matches=true;
            for ((&dst,&other),&out) in destination.iter()
                .zip(second_buffer.iter()).zip(known_output.iter()){
                let current_low=(low+sum) as u8;
                if (dst^other^out)!=(current_low^upper){
                    matches=false;break;
                }
                sum+=u16::from(out);
            }
            if matches {
                candidates.push(NoCarryMixerCandidate{
                    initial_low_byte:low as u8,
                    initial_upper_three_xor:upper,
                });
            }
        }
    }
    let equivalent_full_initial_states=(candidates.len() as u64)*65536;
    Ok(NoCarryMixerAudit{
        observations:known_output.len(),
        output_byte_sum:total,
        candidates,
        equivalent_full_initial_states,
    })
}

/// The actual original code after XOR has a complementary JNP/JP
/// split and a PUSH/CALL-to-single-RET/POP no-op detour. Both parity
/// outcomes rejoin at the exact executable boundary RVA 0x19D85.
pub fn parity_resume_rva(pf:bool)->u32 {
    if !pf {
        // 19D78 JNP +0x0B
        0x19D78 + 2 + 0x0B
    }else{
        // 19D7A PUSH EDX; 19D7B CALL 19CE0 (RET);
        // 19D80 POP EDX; 19D81 JP +2
        0x19D81 + 2 + 0x02
    }
}

pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,dest) in [
        (HELPER_CALL_RVA,HELPER_RVA),
        (0x19D7Bu32,0x19CE0u32),
    ]{
        if dll_relative_call_target(dll,pe,site)?!=dest {
            return Err(format!("original 19C78 byte mixer E8 target mismatch at RVA 0x{site:05X}"));
        }
    }
    // Prove the parity-branch fake return really is a RET-only
    // target, and that the reached instruction sequence modifies
    // [EBP+0x14] using all four state bytes, an independent input
    // [EBP+0x1C], and a 32-bit state increment.
    for (site,bytes) in [
        (0x19CE0u32,&[0xC3][..]),
        (0x19D45,&[0x8B,0x55,0x14,0x33,0xC0][..]),
        (0x19D4A,&[0x8A,0x02][..]),
        (0x19D4E,&[0x8A,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19D54,&[0x33,0xC1][..]),
        (0x19D59,&[0x88,0x02][..]),
        (0x19D62,&[0x8B,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19D68,&[0xC1,0xEA,0x08][..]),
        (0x19D71,&[0x33,0xCA][..]),
        (0x19D76,&[0x88,0x08][..]),
        (0x19D78,&[0x7B,0x0B][..]),
        (0x19D7A,&[0x52][..]),
        (0x19D80,&[0x5A][..]),
        (0x19D81,&[0x7A,0x02][..]),
        (0x19D83,&[0xC3][..]), // unreachable on proved parity detour
        (0x19D85,&[0x8B,0x4D,0x14][..]),
        (0x19D8C,&[0xA1,0x10,0x50,0x90,0x00][..]),
        (0x19D91,&[0xC1,0xE8,0x10][..]),
        (0x19D99,&[0x33,0xD0][..]),
        (0x19D9E,&[0x88,0x11][..]),
        (0x19DA7,&[0x8B,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19DAD,&[0xC1,0xE9,0x18][..]),
        (0x19DB6,&[0x33,0xC1][..]),
        (0x19DBB,&[0x88,0x02][..]),
        (0x19DC4,&[0x8B,0x55,0x1C][..]),
        (0x19DC9,&[0x8A,0x02][..]),
        (0x19DCB,&[0x33,0xC8][..]),
        (0x19DD0,&[0x88,0x0A][..]),
        (0x19DD9,&[0x8B,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19DDF,&[0x03,0xD1][..]),
        (0x19DE1,&[0x89,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19DE7,&[0x8B,0x45,0x14][..]),
        (0x19DEA,&[0x83,0xC0,0x01][..]),
        (0x19DED,&[0x89,0x45,0x14][..]),
        (0x19DF0,&[0x8B,0x4D,0x1C][..]),
        (0x19DF3,&[0x83,0xC1,0x01][..]),
        (0x19DF6,&[0x89,0x4D,0x1C][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_fixed_expected_byte_vectors(){
        assert_eq!(conditional_byte_step(0x10,0x20,0),
                   (0x30,0x30));
        assert_eq!(conditional_byte_step(0x12,0x34,0x1122_3344),
                   (0x62,0x1122_33A6));
        assert_eq!(conditional_byte_step(0xFF,0x7F,0xFEDC_BA98),
                   (0x80,0xFEDC_BB18));
        assert_eq!(conditional_byte_step(0,0,u32::MAX),
                   (0,u32::MAX));
    }

    #[test]
    fn all_256_low_byte_parities_rejoin_after_fake_ret(){
        for x in 0..=255u16 {
            let byte=x as u8;
            let pf=byte.count_ones()%2==0;
            assert_eq!(parity_resume_rva(pf),0x19D85);
        }
    }

    #[test]
    fn byte_state_is_sensitive_to_each_of_the_four_key_bytes(){
        for shift in [0,8,16,24] {
            let (output,_)=conditional_byte_step(0,0,1u32<<shift);
            assert_eq!(output,1);
        }
    }

    #[test]
    fn second_buffer_changes_output_and_running_state(){
        let a=conditional_byte_step(0x45,0x00,0x0024_6810);
        let b=conditional_byte_step(0x45,0x80,0x0024_6810);
        assert_eq!(a.0^b.0,0x80);
        assert_ne!(a.1,b.1);
    }

    #[test]
    fn contiguous_synthetic_steps_preserve_modular_state_sum() {
        let dst=[0x10u8,0x20,0x30,0x40,0x50];
        let other=[0x55u8,0xAA,0x00,0xFF,0xC3];
        for start in [0u32,1,0x1122_3344,u32::MAX-7,u32::MAX] {
            let (out,end)=hypothetical_consecutive_steps(
                &dst,&other,start).unwrap();
            let sum=out.iter().fold(0u32,
                |acc,&byte|acc.wrapping_add(u32::from(byte)));
            assert_eq!(end,start.wrapping_add(sum));
            assert_eq!(hypothetical_initial_state(end,&out),start);
        }
    }

    #[test]
    fn synthetic_chain_composes_across_explicit_split_boundaries() {
        let dst=[0x12,0x34,0x56,0x78];
        let other=[0x89,0xAB,0xCD,0xEF];
        let start=0xFFFF_FFFCu32;
        let whole=hypothetical_consecutive_steps(
            &dst,&other,start).unwrap();
        let first=hypothetical_consecutive_steps(
            &dst[..2],&other[..2],start).unwrap();
        let second=hypothetical_consecutive_steps(
            &dst[2..],&other[2..],first.1).unwrap();
        let joined:Vec<u8>=first.0.iter()
            .chain(second.0.iter()).copied().collect();
        assert_eq!((joined,second.1),whole);
        assert_eq!(hypothetical_initial_state(whole.1,&whole.0),start);
    }

    #[test]
    fn hypothetical_chain_fails_closed_for_mismatched_buffers() {
        assert!(hypothetical_consecutive_steps(&[1,2],&[3],0).is_none());
        assert_eq!(hypothetical_consecutive_steps(&[],&[],42),
                   Some((Vec::new(),42)));
        assert_eq!(hypothetical_initial_state(u32::MAX,&[]),u32::MAX);
    }

    #[test]
    fn no_carry_candidates_contain_real_synthetic_state_but_not_unique_upper_bytes(){
        let initial=0x1234_5610u32;
        let dst=[0x10,0x20,0x30,0x40];
        let desired=[3u8,5,1,4];
        let mut other=[0u8;4];
        let mut state=initial;
        for i in 0..desired.len(){
            let xor=(state as u8)^((state>>8) as u8)^
                ((state>>16) as u8)^((state>>24) as u8);
            other[i]=dst[i]^xor^desired[i];
            state=state.wrapping_add(u32::from(desired[i]));
        }
        let (actual,after)=hypothetical_consecutive_steps(&dst,&other,initial).unwrap();
        assert_eq!(actual,desired);
        assert_eq!(after,state);
        let report=hypothetical_no_carry_initial_states(&dst,&other,&desired).unwrap();
        assert!(report.candidates.contains(&NoCarryMixerCandidate{
            initial_low_byte:0x10,
            initial_upper_three_xor:0x12^0x34^0x56,
        }));
        assert!(report.equivalent_full_initial_states>=65536);
        assert_eq!(report.equivalent_full_initial_states,
            report.candidates.len() as u64*65536);
    }
    #[test]
    fn no_carry_one_byte_never_recovers_full_32_bit_state(){
        let report=hypothetical_no_carry_initial_states(&[0],&[0],&[0]).unwrap();
        assert_eq!(report.candidates.len(),256);
        assert_eq!(report.equivalent_full_initial_states,1u64<<24);
        assert_eq!(report.output_byte_sum,0);
    }
    #[test]
    fn high_output_sum_excludes_no_carry_only_not_real_mixer_execution(){
        let initial=0xAABB_CCF0u32;
        let dst=[1u8,2u8];
        let wanted=[200u8,100u8];
        let mut other=[0u8;2];
        let mut state=initial;
        for i in 0..2{
            let k=(state as u8)^((state>>8) as u8)^
                ((state>>16) as u8)^((state>>24) as u8);
            other[i]=dst[i]^k^wanted[i];
            state=state.wrapping_add(u32::from(wanted[i]));
        }
        assert_eq!(hypothetical_consecutive_steps(&dst,&other,initial).unwrap().0,wanted);
        let report=hypothetical_no_carry_initial_states(&dst,&other,&wanted).unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(report.equivalent_full_initial_states,0);
        // The observed bytes are still consistent with an actual
        // original-state choice that DOES carry across byte 0.
    }
    #[test]
    fn no_carry_rejects_inconsistent_zero_output_observations(){
        let report=hypothetical_no_carry_initial_states(
            &[0,0],&[7,9],&[0,0]).unwrap();
        assert!(report.candidates.is_empty());
    }
    #[test]
    fn no_carry_rejects_unbounded_or_mismatched_observations(){
        assert!(hypothetical_no_carry_initial_states(&[],&[],&[]).is_err());
        assert!(hypothetical_no_carry_initial_states(&[1],&[],&[1]).is_err());
        assert!(hypothetical_no_carry_initial_states(
            &vec![0;4097],&vec![0;4097],&vec![0;4097]).is_err());
    }
    #[test]
    fn add_wraps_on_maximum_state(){
        let (output,updated)=conditional_byte_step(0xFF,0,0xFFFF_FF00);
        // 0xFF000000 XOR 0x0000FF00 -> key XOR 0;
        // 0xFFFF_FF00 is bytes [00, FF, FF, FF], whose XOR is FF.
        assert_eq!(output,0);
        assert_eq!(updated,0xFFFF_FF00);
        let (output,updated)=conditional_byte_step(0xFF,0xFF,0xFFFF_FFFE);
        assert_eq!(output,0x01);
        assert_eq!(updated,u32::MAX);
        let (output,updated)=conditional_byte_step(0xFF,0,0xFFFF_FFFF);
        assert_eq!(output,0xFF);
        assert_eq!(updated,0xFE);
    }
}
