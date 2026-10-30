// Exact-image static x86 control-flow constraints for the original
// SafeDisc module-category dispatcher at RVA 0xA2A0.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This models only the dispatch rules. None of the three downstream
// routines is simulated, no protected DLL instructions are executed.

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct DispatchModel {
    pub registered_module_found:bool,
    pub first_result:Option<u16>,
    pub second_result:Option<u16>,
    pub b850_calls:usize,
    pub b880_calls:usize,
    pub b8e0_calls:usize,
}

/// The original code branches to return when the module record is absent.
/// Otherwise 0xB850 runs first. A nonzero AX return suppresses the
/// second and third handlers, and 0xB880 similarly suppresses 0xB8E0.
pub fn simulate_dispatch(module_present:bool, first:u16,second:u16)->DispatchModel {
    if !module_present {
        return DispatchModel{
            registered_module_found:false,first_result:None,second_result:None,
            b850_calls:0,b880_calls:0,b8e0_calls:0
        };
    }
    let second_invoked=first==0;
    let third_invoked=second_invoked&&second==0;
    DispatchModel{
        registered_module_found:true,
        first_result:Some(first),
        second_result:second_invoked.then_some(second),
        b850_calls:1,
        b880_calls:usize::from(second_invoked),
        b8e0_calls:usize::from(third_invoked)
    }
}

pub fn verify_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (at,target) in [
        (0xA2A9u32,0xB5D0u32),
        (0xA2C3,0xBF70),
        (0xA2D3,0xB850),
        (0xA2E4,0xB880),
        (0xA2F5,0xB8E0),
    ] {
        if dll_relative_call_target(dll,pe,at)?!=target {
            return Err(format!("unexpected A2A0 call target at original RVA 0x{at:08X}"));
        }
    }
    // The two TEST AX,AX / JNE pairs enforce priority.
    for (site,bytes) in [
        (0xA2B1u32,&[0x85,0xC0,0x74,0x48][..]),
        (0xA2DB,&[0x66,0x85,0xC0,0x75,0x1D][..]),
        (0xA2EC,&[0x66,0x85,0xC0,0x75,0x0C][..]),
    ]{
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_record_skips_all_handlers(){
        let r=simulate_dispatch(false,0,0);
        assert_eq!((r.b850_calls,r.b880_calls,r.b8e0_calls),(0,0,0));
        assert_eq!(r.first_result,None);
        assert_eq!(r.second_result,None);
    }
    #[test]
    fn first_nonzero_suppresses_remaining_handlers(){
        for first in [1,2,0x8000,0xFFFF]{
            let r=simulate_dispatch(true,first,0);
            assert_eq!((r.b850_calls,r.b880_calls,r.b8e0_calls),(1,0,0));
            assert_eq!(r.first_result,Some(first));
            assert_eq!(r.second_result,None);
        }
    }
    #[test]
    fn second_nonzero_suppresses_third_handler(){
        for second in [1,2,0xFFFF]{
            let r=simulate_dispatch(true,0,second);
            assert_eq!((r.b850_calls,r.b880_calls,r.b8e0_calls),(1,1,0));
            assert_eq!(r.first_result,Some(0));
            assert_eq!(r.second_result,Some(second));
        }
    }
    #[test]
    fn only_two_zero_returns_reach_final_handler(){
        let r=simulate_dispatch(true,0,0);
        assert_eq!((r.b850_calls,r.b880_calls,r.b8e0_calls),(1,1,1));
        assert_eq!(r.second_result,Some(0));
    }
}
