//! Read-only observation of the verified v1.02 executable's two video
//! preference variables. Does not modify UI, choices or graphical state.
//! No guessed click rectangles or broad input interception.
use std::{ptr,sync::{Mutex,OnceLock}};
use crate::{log,menu_scene};

const IMAGE_BASE:usize=0x400000;
const DEVICE_SITE:usize=0x419208;
const DEVICE_READ:[u8;5]=[0xa1,0x90,0xb4,0x64,0x00];
const MODE_SITE:usize=0x419235;
const MODE_READ:[u8;6]=[0x8b,0x15,0x8c,0xb4,0x64,0x00];
const SELECTED_DEVICE:usize=0x64b490; // script variable: nSelectedVideoDevice
const SELECTED_MODE:usize=0x64b48c; // mode table index, NOT monitor dimensions
const MAX_TRANSITIONS:u32=32;

#[derive(Default)]
struct State {last:Option<(u32,u32)>,changes:u32,valid:Option<bool>}
static STATE:OnceLock<Mutex<State>>=OnceLock::new();

fn changed(state:&mut State,now:(u32,u32))->bool{
    if state.last==Some(now)||state.changes>=MAX_TRANSITIONS{return false;}
    state.last=Some(now);
    state.changes+=1;
    true
}
fn image_verified()->bool{
    unsafe{
        let device=std::slice::from_raw_parts(DEVICE_SITE as *const u8,DEVICE_READ.len());
        let mode=std::slice::from_raw_parts(MODE_SITE as *const u8,MODE_READ.len());
        device==DEVICE_READ && mode==MODE_READ
    }
}
pub fn observe_preferences_flip(){
    if !crate::cfg().log_enabled || !crate::cfg().experimental_windowed_adapter{return;}
    let state=STATE.get_or_init(||Mutex::new(State::default()));
    let Ok(mut state)=state.try_lock() else{return;};
    if !menu_scene::preferences_active(){state.last=None;return;}
    let valid=*state.valid.get_or_insert_with(image_verified);
    if !valid{
        if state.changes==0{
            log("MENU-VIDEO-PROBE: exact original device/mode read instructions absent; disabled");
            state.changes=MAX_TRANSITIONS;
        }
        return;
    }
    let device=unsafe{ptr::read_volatile(SELECTED_DEVICE as *const u32)};
    let index=unsafe{ptr::read_volatile(SELECTED_MODE as *const u32)};
    if !changed(&mut state,(device,index)){return;}
    log(&format!("MENU-VIDEO-PROBE: Preferences selected_device={device} selected_mode_index={index} [READ-ONLY; not desktop resolution]"));
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn change_detection_is_bounded_and_deduplicated(){
        let mut state=State::default();
        assert!(changed(&mut state,(0,1)));
        assert!(!changed(&mut state,(0,1)));
        assert!(changed(&mut state,(1,1)));
        state.changes=MAX_TRANSITIONS;
        assert!(!changed(&mut state,(2,3)));
    }
    #[test]
    fn x86_preimages_reference_exact_script_variables(){
        assert_eq!(IMAGE_BASE,0x400000);
        assert_eq!(&DEVICE_READ[1..],&(SELECTED_DEVICE as u32).to_le_bytes());
        assert_eq!(&MODE_READ[2..],&(SELECTED_MODE as u32).to_le_bytes());
    }
}