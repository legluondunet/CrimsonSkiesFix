//! Initial (once-only) High lighting suggestion for the verified v1.02
//! in-game Video Preferences screen in the experimental split WinMM runtime.
//!
//! The original game's uiData opcode 0x855 is called by VIDEO.SCRIPT with
//! arg=0 to read the nine pending preference fields *before* the dropdowns
//! initialize. VIDEO.SCRIPT gives Lighting Quality its own 5-choice widget
//! (opcode 0x847) and obtains its selection from pending nLightingQuality.
//! No Video widget objects, registry, game binary or DLL are modified.
//!
//! On the exact admitted executable the lighting selector is at 0x64B484
//! (pending copy 0x648364). The original getter 0x4190E6 maps the 0x60001
//! full-lighting hardware flags to selector=1. Default flags 0x1 yield
//! selector=2 (Medium). Do not make a generic "all quality" adjustment.
//!
//! A marker is written inside the existing, user-editable
//! CrimsonSkiesFix.ini AFTER the original Video Apply callback completes.
//! This preserves subsequent changes. If the player Cancels before applying,
//! the initial High proposal can be made again at the next game start.
use std::{
    fs::{self,OpenOptions},io::Write,path::Path,
    sync::atomic::{AtomicBool,Ordering},
};
use crate::{log,win32};

pub(crate) const ACTIVE_LIGHTING:usize=0x0064_B484;
pub(crate) const PENDING_LIGHTING:usize=0x0064_8364;
const HIGH:u32=1;
const MEDIUM:u32=2;
const MARKER:&str="CrimsonSkiesFixLightingHighDefaultApplied";
const MAX_INI_BYTES:u64=128*1024;
const LIGHTING_REGISTER_SITE:usize=0x0040_21C4;
const ORIGINAL_LIGHTING_REGISTER:[u8;10]=[
    0x68,0x84,0xb4,0x64,0x00,  // push nLightingQuality storage
    0x68,0x9c,0xe8,0x61,0x00, // push "nLightingQuality"
];
static OFFERED:AtomicBool=AtomicBool::new(false);

fn marker_applied(ini:&str)->bool{
    let mut compatibility=false;
    for line in ini.lines(){
        let item=line.split([';','#']).next().unwrap_or("").trim();
        if item.starts_with('[')&&item.ends_with(']'){
            compatibility=item[1..item.len()-1].eq_ignore_ascii_case("Compatibility");
            continue;
        }
        if compatibility{
            if let Some((name,value))=item.split_once('='){
                if name.trim().eq_ignore_ascii_case(MARKER)&&value.trim()=="1"{
                    return true;
                }
            }
        }
    }
    false
}
fn valid_ini(path:&Path)->Option<String>{
    let metadata=fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len()>MAX_INI_BYTES{return None;}
    fs::read_to_string(path).ok()
}
fn should_offer(active:u32,pending:u32,already_applied:bool,video_read:bool)->bool{
    video_read&&!already_applied&&active==MEDIUM&&pending==MEDIUM
}
fn exact_lighting_registration()->bool{
    unsafe{
        std::slice::from_raw_parts(
            LIGHTING_REGISTER_SITE as *const u8,ORIGINAL_LIGHTING_REGISTER.len())
            ==ORIGINAL_LIGHTING_REGISTER
    }
}

/// Called only from the verified game's original uiData 0x855 VIDEO GET,
/// after original GOS command 13 copied the active preferences to pending.
/// One DWORD in *pending* memory, before the game itself reads it and builds
/// the dropdown. The original UI and Apply/Cancel handlers stay authoritative.
pub(crate) unsafe fn before_video_get()->bool{
    if OFFERED.load(Ordering::Acquire) || !exact_lighting_registration() ||
        !crate::menu_video_probe::verified_video_fields() ||
        !crate::menu_video_probe::legacy_ui_active() ||
        !crate::menu_scene::main_menu_was_seen() ||
        !crate::menu_gos_event::preferences_transaction_active() {
        return false;
    }
    let Some(path)=win32::ini_path() else{return false;};
    let Some(ini)=valid_ini(&path) else{return false;};
    let active=std::ptr::read_volatile(ACTIVE_LIGHTING as *const u32);
    let pending=std::ptr::read_volatile(PENDING_LIGHTING as *const u32);
    if !should_offer(active,pending,marker_applied(&ini),true){return false;}
    if OFFERED.swap(true,Ordering::AcqRel){return false;}
    std::ptr::write_volatile(PENDING_LIGHTING as *mut u32,HIGH);
    log("MENU-LIGHTING-DEFAULT: initial Video Preferences GET pending LightingQuality Medium(2)->High(1); awaiting original Apply to persist, cancel preserves previous choice");
    true
}
pub(crate) fn after_video_apply(original_success:bool){
    if !original_success || !OFFERED.load(Ordering::Acquire){return;}
    let Some(path)=win32::ini_path() else{return;};
    let Some(ini)=valid_ini(&path) else{return;};
    if marker_applied(&ini){return;}
    // Append only: never rewrite or truncate an existing INI containing
    // the user's AspectRatio, FPSLimit, Log or other custom preferences.
    let Ok(mut file)=OpenOptions::new().append(true).open(&path) else{return;};
    let suffix=if ini.ends_with('\n'){"\n"}else{"\n\n"};
    if file.write_all(format!(
        "{suffix}[Compatibility]\n{MARKER}=1\n").as_bytes()).is_ok() &&
        file.sync_all().is_ok(){
        log("MENU-LIGHTING-DEFAULT: original Video Apply completed; initial High default marked once in existing INI; future user lighting changes untouched");
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_game_lighting_field_and_read_registration_offsets(){
        assert_eq!(ACTIVE_LIGHTING-0x64B450,PENDING_LIGHTING-0x648330);
        assert_eq!(ORIGINAL_LIGHTING_REGISTER,
            [0x68,0x84,0xb4,0x64,0x00,0x68,0x9c,0xe8,0x61,0x00]);
        assert_eq!(HIGH,1);
        assert_eq!(MEDIUM,2);
    }
    #[test]
    fn only_initial_medium_is_replaced_and_other_quality_settings_are_untouched(){
        assert!(should_offer(2,2,false,true));
        assert!(!should_offer(2,2,true,true));
        assert!(!should_offer(2,2,false,false));
        for value in [0,1,3,4] {
            assert!(!should_offer(value,value,false,true));
        }
        assert!(!should_offer(2,3,false,true));
        assert!(!should_offer(1,2,false,true));
    }
    #[test]
    fn marker_requires_exact_compatibility_ini_key(){
        assert!(!marker_applied("[Graphics]\nCrimsonSkiesFixLightingHighDefaultApplied=1\n"));
        assert!(!marker_applied("[Compatibility]\n; CrimsonSkiesFixLightingHighDefaultApplied=1\n"));
        assert!(marker_applied("[Graphics]\nAspectRatio=auto\n\n[Compatibility]\ncrimsonskiesfixlightinghighdefaultapplied=1\n"));
        assert!(!marker_applied("[Compatibility]\nCrimsonSkiesFixLightingHighDefaultApplied=0\n"));
        assert!(!marker_applied("[Compatibility]\nCrimsonSkiesFixLightingHighDefaultApplied=11\n"));
    }
}