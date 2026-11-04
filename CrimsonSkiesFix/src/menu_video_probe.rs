//! Read-only observation of the verified v1.02 executable's two video
//! preference variables. Does not modify UI, choices or graphical state.
//! No guessed click rectangles or broad input interception.
use std::{ptr,sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}}};
use crate::{log,menu_scene};

const IMAGE_BASE:usize=0x400000;
const DEVICE_SITE:usize=0x419208;
const DEVICE_READ:[u8;5]=[0xa1,0x90,0xb4,0x64,0x00];
const MODE_SITE:usize=0x419235;
const MODE_READ:[u8;6]=[0x8b,0x15,0x8c,0xb4,0x64,0x00];
const SELECTED_DEVICE:usize=0x64b490; // script variable: nSelectedVideoDevice
const SELECTED_MODE:usize=0x64b48c; // active game mode index
// The original prefs UI operates on a separate 0x81-DWORD scratch copy:
// command 13 copies [0x64B450..] -> [0x648330..], while command 14
// copies the edited scratch fields back before applying settings.
pub const PENDING_DEVICE:usize=0x648370;
pub const PENDING_MODE:usize=0x64836c;
const DEVICE_ACCESS_SITE:usize=0x40cce8;
const DEVICE_ACCESS_BYTES:[u8;5]=[0xa1,0x70,0x83,0x64,0x00];
const MODE_ACCESS_SITE:usize=0x40ccf2;
const MODE_ACCESS_BYTES:[u8;6]=[0x8b,0x15,0x6c,0x83,0x64,0x00];
const MAX_TRANSITIONS:u32=32;

#[derive(Default)]
struct State {last:Option<((u32,u32),(u32,u32))>,changes:u32,valid:Option<bool>}
static STATE:OnceLock<Mutex<State>>=OnceLock::new();
static LEGACY_UI_ACTIVE:AtomicBool=AtomicBool::new(false);
/// Updated for every virtual primary Flip, independently of Log=1.
pub fn legacy_ui_active()->bool{LEGACY_UI_ACTIVE.load(Ordering::Acquire)}


fn changed(state:&mut State,now:((u32,u32),(u32,u32)))->bool{
    if state.last==Some(now)||state.changes>=MAX_TRANSITIONS{return false;}
    state.last=Some(now);
    state.changes+=1;
    true
}
fn image_verified()->bool{
    unsafe{
        let device=std::slice::from_raw_parts(DEVICE_SITE as *const u8,DEVICE_READ.len());
        let mode=std::slice::from_raw_parts(MODE_SITE as *const u8,MODE_READ.len());
        let pending_device=std::slice::from_raw_parts(DEVICE_ACCESS_SITE as *const u8,DEVICE_ACCESS_BYTES.len());
        let pending_mode=std::slice::from_raw_parts(MODE_ACCESS_SITE as *const u8,MODE_ACCESS_BYTES.len());
        device==DEVICE_READ && mode==MODE_READ &&
        pending_device==DEVICE_ACCESS_BYTES && pending_mode==MODE_ACCESS_BYTES
    }
}
/// Revalidate the game's exact original video-setting access instructions
/// immediately before any selective Preferences commit protection. The
/// authenticated EXE gate alone does not prove these instructions survived.
pub(crate) fn verified_video_fields()->bool{image_verified()}

fn is_legacy_ui_mode(w:u32,h:u32)->bool{
    // The game uses a fixed 4:3 virtual 640x480 or 800x600 surface for
    // menus. Never assume the observed script tag includes every submenu.
    matches!((w,h),(640,480)|(800,600))
}
pub fn observe_ui_flip(w:u32,h:u32){
    LEGACY_UI_ACTIVE.store(is_legacy_ui_mode(w,h),Ordering::Release);
    if !crate::cfg().log_enabled || !crate::cfg().experimental_windowed_adapter{return;}
    let state=STATE.get_or_init(||Mutex::new(State::default()));
    let Ok(mut state)=state.try_lock() else{return;};
    if !is_legacy_ui_mode(w,h){
        state.last=None;
        return;
    }
    // Only the verified original EXE is admitted by exe_gate. Verify two
    // immutable instruction preimages before ever reading these globals.
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
    let pending_device=unsafe{ptr::read_volatile(PENDING_DEVICE as *const u32)};
    let pending_mode=unsafe{ptr::read_volatile(PENDING_MODE as *const u32)};
    if !changed(&mut state,((device,index),(pending_device,pending_mode))){return;}
    let scene=if menu_scene::preferences_active(){"Preferences"}
       else if menu_scene::main_menu_active(){"MainMenu-or-unobserved-submenu"}
       else{"unidentified"};
    log(&format!("MENU-VIDEO-PROBE: ui={}x{} scene={} active_device={} active_mode_index={} pending_device={} pending_mode_index={} [READ-ONLY; indices are NOT desktop dimensions]",
        w,h,scene,device,index,pending_device,pending_mode));
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn change_detection_is_bounded_and_deduplicated(){
        let mut state=State::default();
        assert!(changed(&mut state,((0,1),(0,1))));
        assert!(!changed(&mut state,((0,1),(0,1))));
        // A Graphics menu selection may change only the pending copy.
        assert!(changed(&mut state,((0,1),(1,1))));
        assert!(changed(&mut state,((0,1),(1,0))));
        state.changes=MAX_TRANSITIONS;
        assert!(!changed(&mut state,((2,3),(2,3))));
    }
    #[test]
    fn traces_only_legacy_ui_frame_geometry(){
        assert!(is_legacy_ui_mode(800,600));
        assert!(is_legacy_ui_mode(640,480));
        assert!(!is_legacy_ui_mode(1920,1080));
        assert!(!is_legacy_ui_mode(2560,1440));
        assert!(!is_legacy_ui_mode(320,240));
    }
    #[test]
    fn x86_preimages_reference_exact_script_variables(){
        assert_eq!(IMAGE_BASE,0x400000);
        assert_eq!(&DEVICE_READ[1..],&(SELECTED_DEVICE as u32).to_le_bytes());
        assert_eq!(&MODE_READ[2..],&(SELECTED_MODE as u32).to_le_bytes());
        assert_eq!(&DEVICE_ACCESS_BYTES[1..],&(PENDING_DEVICE as u32).to_le_bytes());
        assert_eq!(&MODE_ACCESS_BYTES[2..],&(PENDING_MODE as u32).to_le_bytes());
        assert_eq!(PENDING_DEVICE-0x648330,SELECTED_DEVICE-0x64b450);
        assert_eq!(PENDING_MODE-0x648330,SELECTED_MODE-0x64b450);
    }
}