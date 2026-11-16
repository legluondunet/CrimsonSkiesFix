//! Verified native gameplay camera aspect correction for Crimson Skies 1.02.
//!
//! The original 4:3 engine can regenerate its camera coefficients using the
//! legacy global H/W=0.75 after the reference FOV patch widens the horizontal
//! field of view. In a verified 1920x1080 game epoch, Y must use 0.5625;
//! in 1920x1200, 0.625. The original horizontal FOV remains unchanged.
//!
//! This production fix preserves the game's original resolution/FOV patch,
//! writes only the authenticated in-memory aspect global during gameplay,
//! reasserts it before real 3D scenes if the old game reset it, and restores
//! 0.75 for original 800x600/640x480 menu and movie modes. Disabled in
//! explicit 4:3 and passthrough modes. User INIs need no experimental keys.
//!
//! The global is never overwritten if its previous value is unknown; the
//! game EXE on disk and all 3D vertex data are never modified here.
use std::{
    ffi::c_void,
    mem::{size_of,MaybeUninit},
    ptr,
    sync::atomic::{AtomicUsize,Ordering},
};
const ASPECT_GLOBAL:usize=0x0071_B3F8;
const IMAGE_BASE:usize=0x0040_0000;
const FOUR_THIRDS_H_OVER_W:f32=0.75;
const MEM_COMMIT:u32=0x1000;
const PAGE_GUARD:u32=0x100;
static WRITES:AtomicUsize=AtomicUsize::new(0);
static RESTORES:AtomicUsize=AtomicUsize::new(0);
static NO_UPDATE:AtomicUsize=AtomicUsize::new(0);

#[repr(C)]
struct MemoryBasicInformation{
    base_address:*mut c_void,
    allocation_base:*mut c_void,
    allocation_protect:u32,
    region_size:usize,
    state:u32,
    protect:u32,
    kind:u32,
}
#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualQuery(address:*const c_void,
        info:*mut MemoryBasicInformation,size:usize)->usize;
}

fn expected_aspect(w:u32,h:u32)->Option<f32>{
    if w<960 || h<600 || w>8192 || h>4320 ||
       (w as u64)*3<=(h as u64)*4 ||
       (w as u64)>4*(h as u64) {return None;}
    let ratio=h as f32/w as f32;
    (ratio.is_finite() && ratio>0.2 && ratio<FOUR_THIRDS_H_OVER_W)
        .then_some(ratio)
}
fn eligible_config(c:&crate::config::Config)->bool{
    c.desktop_resolution && c.native_3d_widescreen &&
    c.experimental_windowed_adapter && !c.force_4_by_3 &&
    !c.directdraw_passthrough
}
fn enabled()->bool{eligible_config(&crate::cfg())}
unsafe fn verified_global()->Option<*mut u32>{
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE{return None;}
    let a=ASPECT_GLOBAL as *mut u32;
    let mut basic=MaybeUninit::<MemoryBasicInformation>::zeroed();
    if VirtualQuery(a.cast::<c_void>(),basic.as_mut_ptr(),
        size_of::<MemoryBasicInformation>())!=size_of::<MemoryBasicInformation>(){
        return None;
    }
    let info=basic.assume_init();
    if info.state!=MEM_COMMIT || info.protect&PAGE_GUARD!=0 ||
       !matches!(info.protect&0xff,0x04|0x08|0x40|0x80){
        return None;
    }
    let start=info.base_address as usize;
    let end=start.checked_add(info.region_size)?;
    if !(start<=ASPECT_GLOBAL && ASPECT_GLOBAL.checked_add(4)?<=end){
        return None;
    }
    Some(a)
}
unsafe fn replace_if_known(target:f32,restoring:bool,reason:&str){
    let Some(addr)=verified_global()else{
        let n=NO_UPDATE.fetch_add(1,Ordering::Relaxed)+1;
        if n<=3 && crate::cfg().log_enabled{
            crate::log("CAMERA-ASPECT: rejected protected/invalid aspect global; memory unchanged");
        }
        return;
    };
    let original=FOUR_THIRDS_H_OVER_W.to_bits();
    let expected=crate::native_widescreen::verified_launch_widescreen_dimensions()
        .and_then(|(w,h)|expected_aspect(w,h))
        .map(f32::to_bits);
    let Some(new_bits)=expected else{return;};
    let old=ptr::read_volatile(addr);
    let target_bits=target.to_bits();
    if old==target_bits{return;}
    // Reject unexpected mods rather than trampling another renderer/patch.
    if old!=original && old!=new_bits {
        let n=NO_UPDATE.fetch_add(1,Ordering::Relaxed)+1;
        if n<=5 && crate::cfg().log_enabled{
            crate::log(&format!(
                "CAMERA-ASPECT: rejected conflicting global={:.7} wanted={target:.7} reason={reason} untouched=true",
                f32::from_bits(old)
            ));
        }
        return;
    }
    ptr::write_volatile(addr,target_bits);
    let got=ptr::read_volatile(addr);
    if got!=target_bits {
        crate::log("CAMERA-ASPECT: failed to verify in-memory write; no camera fix claimed");
        return;
    }
    let ordinal=if restoring{
        RESTORES.fetch_add(1,Ordering::Relaxed)+1
    }else{
        WRITES.fetch_add(1,Ordering::Relaxed)+1
    };
    if crate::cfg().log_enabled && (ordinal<=8 || ordinal%4096==0){
        crate::log(&format!(
            "CAMERA-ASPECT: {}={} before={:.7} after={target:.7} source={reason} ordinal={ordinal} only_verified_original_global=true on_disk_exe_unchanged=true",
            if restoring{"restored_4by3"}else{"set_actual_h_over_w"},
            if restoring{1}else{1},f32::from_bits(old)
        ));
    }
}
/// Invoked only AFTER successful game-owned virtual DD4 mode preflight.
pub(super) unsafe fn after_verified_mode(w:u32,h:u32,is_gameplay:bool){
    if !enabled(){return;}
    let Some((expected_w,expected_h))=
        crate::native_widescreen::verified_launch_widescreen_dimensions()else{return;};
    if is_gameplay && (w,h)==(expected_w,expected_h){
        if let Some(aspect)=expected_aspect(w,h){
            replace_if_known(aspect,false,"verified-dd4-gameplay-mode");
        }
    }else if (w,h)==(800,600) || (w,h)==(640,480) {
        replace_if_known(FOUR_THIRDS_H_OVER_W,true,"verified-original-menu-or-movie-mode");
    }
}
/// Called before real D3DDevice3::BeginScene, but only with an already
/// verified, live 3D virtual DD4 epoch. If original 4:3 writer fired again
/// after SetDisplayMode, this can reassert H/W without touching 2D menus.
pub(super) unsafe fn on_begin_scene(){
    if !enabled(){return;}
    let Some((w,h))=crate::in_game_adapter::active_verified_widescreen_gameplay_raster()
        else{return;};
    let Some(aspect)=expected_aspect(w,h)else{return;};
    replace_if_known(aspect,false,"verified-gameplay-begin-scene");
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn normal_graphics_profile_automatically_enables_fix(){
        let mut c=crate::config::Config::default();
        // Crucial release regression: Log=0 on a fresh install must not
        // silently disable the verified camera fix.
        assert!(!c.log_enabled);
        assert!(eligible_config(&c));
        c.force_4_by_3=true;
        assert!(!eligible_config(&c));
        c.force_4_by_3=false;
        c.directdraw_passthrough=true;
        assert!(!eligible_config(&c));
        c.directdraw_passthrough=false;
        c.native_3d_widescreen=false;
        assert!(!eligible_config(&c));
        c.native_3d_widescreen=true;
        c.experimental_windowed_adapter=false;
        assert!(!eligible_config(&c));
        c.experimental_windowed_adapter=true;
        c.desktop_resolution=false;
        assert!(!eligible_config(&c));
    }
    #[test] fn widescreen_h_over_w_keeps_original_vertical_fov(){
        assert_eq!(expected_aspect(1920,1080),Some(0.5625));
        assert_eq!(expected_aspect(1920,1200),Some(0.625));
        assert_eq!(expected_aspect(1600,1200),None);
        assert_eq!(expected_aspect(800,600),None);
        assert_eq!(expected_aspect(0,1080),None);
    }
    #[test] fn expected_ab_camera_fov_preserves_original_4by3_vertical_fov(){
        let original_vertical=2.0*((60f64.to_radians()*0.5).tan()*(3.0/4.0)).atan();
        let wide_horizontal=2.0*((60f64.to_radians()*0.5).tan()*(4.0/3.0)).atan();
        let aspect=expected_aspect(1920,1080).unwrap() as f64;
        let wide_vertical=2.0*((wide_horizontal*0.5).tan()*aspect).atan();
        assert!((wide_vertical-original_vertical).abs()<1e-5);
    }
}
