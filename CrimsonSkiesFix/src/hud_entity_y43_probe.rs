//! Readily reversible *experimental* HUD 2D-entity vertical-axis correction.
//!
//! A/B screenshots on native Windows 1920x1200 demonstrate that all five
//! instrument diameters are ~147x125 in widescreen and ~147x150 in native
//! centered 4:3, while their horizontal anchors are identical. The former
//! camera-entity FOV reference experiment enlarged BOTH axes and moved
//! all gauges outwards, without curing the ~1.2 ellipse ratio.
//!
//! The original decrypted EXE's camera-entity overlay pass 0x49FCC3 calls
//! function 0x455BE0. Inside that function, the two direct CDECL calls
//! at 0x455C40 (SetScale) and 0x455C54 (SetPosition) are passed X,Y,Z
//! values independently. This experiment modifies ONLY the *Y arguments*
//! at those two call sites. The shared 3D FOV, matrices, world projection,
//! objects' stored fields and original on-disk game EXE are untouched.
//!
//! This remains a falsifiable A/B and is OFF by default. The callsite owner
//! is the game's scene-camera entity pass, so 3D effects are possible; do
//! not promote without in-game validation of HUD and world proportions.
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool,AtomicU32,AtomicUsize,Ordering},
};
use super::{PatchResult,IMAGE_BASE};

const SET_SCALE_SITE:usize=0x0045_5C40;
const SET_POSITION_SITE:usize=0x0045_5C54;
const SET_SCALE_ORIGINAL:[u8;5]=[0xE8,0x0B,0xC1,0x07,0x00]; // call 0x4D1D50
const SET_POSITION_ORIGINAL:[u8;5]=[0xE8,0xD7,0xBD,0x07,0x00]; // call 0x4D1A30
const SET_SCALE_FUNCTION:usize=0x004D_1D50;
const SET_POSITION_FUNCTION:usize=0x004D_1A30;
const PAGE_EXECUTE_READWRITE:u32=0x40;
const LOG_LIMIT:usize=12;
static INSTALLED:AtomicBool=AtomicBool::new(false);
static FACTOR_BITS:AtomicU32=AtomicU32::new(0);
static SCALE_CALLS:AtomicUsize=AtomicUsize::new(0);
static POSITION_CALLS:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(ptr:*mut c_void,len:usize,new:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,ptr:*const c_void,len:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

type ScaleOrPosition=unsafe extern "C" fn(*mut c_void,f32,f32,f32)->i32;

fn correction(w:u32,h:u32)->Option<f32>{
    if w<960 || h<600 || w>8192 || h>4320 ||
       (w as u64)*(h as u64)>33_554_432 ||
       (w as u64)*3<=(h as u64)*4 ||
       (w as u64)>(h as u64)*4 {return None;}
    let v=(3.0*w as f32)/(4.0*h as f32);
    (v.is_finite() && v>1.0 && v<=3.0).then_some(v)
}

fn adjusted_y(value:f32,factor:f32)->f32{
    if value.is_finite() && value.abs()<1.0e8 {
        let corrected=value*factor;
        if corrected.is_finite(){return corrected;}
    }
    value
}

unsafe extern "C" fn scale_y(
    object:*mut c_void,x:f32,y:f32,z:f32
)->i32{
    let original:ScaleOrPosition=std::mem::transmute(SET_SCALE_FUNCTION);
    let factor=f32::from_bits(FACTOR_BITS.load(Ordering::Relaxed));
    let target_y=adjusted_y(y,factor);
    if SCALE_CALLS.fetch_add(1,Ordering::Relaxed)<LOG_LIMIT {
        crate::log(&format!(
            "HUD-ENTITY-Y43: call=SetScale original_y={y:.6} new_y={target_y:.6} x_unchanged=true z_unchanged=true factor={factor:.6}"));
    }
    original(object,x,target_y,z)
}

unsafe extern "C" fn position_y(
    object:*mut c_void,x:f32,y:f32,z:f32
)->i32{
    let original:ScaleOrPosition=std::mem::transmute(SET_POSITION_FUNCTION);
    let factor=f32::from_bits(FACTOR_BITS.load(Ordering::Relaxed));
    let target_y=adjusted_y(y,factor);
    if POSITION_CALLS.fetch_add(1,Ordering::Relaxed)<LOG_LIMIT {
        crate::log(&format!(
            "HUD-ENTITY-Y43: call=SetPosition original_y={y:.6} new_y={target_y:.6} x_unchanged=true z_unchanged=true factor={factor:.6}"));
    }
    original(object,x,target_y,z)
}

fn relative_call(site:usize,dest:usize)->[u8;5]{
    let relative=(dest as u32).wrapping_sub((site as u32).wrapping_add(5));
    let mut code=[0u8;5];
    code[0]=0xE8;
    code[1..5].copy_from_slice(&relative.to_le_bytes());
    code
}

unsafe fn read_call(site:usize)->[u8;5]{
    let mut b=[0u8;5];
    ptr::copy_nonoverlapping(site as *const u8,b.as_mut_ptr(),5);
    b
}

unsafe fn commit_call(site:usize,bytes:&[u8;5])->bool{
    let mut old=0u32;
    if VirtualProtect(site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut old)==0{
        return false;
    }
    ptr::copy_nonoverlapping(bytes.as_ptr(),site as *mut u8,5);
    let flushed=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5)!=0;
    let mut ignored=0u32;
    let restored=VirtualProtect(site as *mut c_void,5,old,&mut ignored)!=0;
    flushed&&restored
}

pub(super) unsafe fn install(w:u32,h:u32,patch:PatchResult){
    let cfg=crate::cfg();
    if !cfg.experimental_hud_entity_y43_probe{return;}
    if !crate::diagnostics::is_native_windows() ||
       cfg.force_4_by_3 || cfg.experimental_hud_fov43_probe ||
       !cfg.log_enabled || !cfg.native_3d_widescreen ||
       !cfg.experimental_windowed_adapter || patch!=PatchResult::Patched {
        crate::log("HUD-ENTITY-Y43: refused unsupported mode, FOV A/B overlap or inactive widescreen patch");
        return;
    }
    let Some(factor)=correction(w,h) else {
        crate::log("HUD-ENTITY-Y43: refused invalid dimensions/aspect");
        return;
    };
    if INSTALLED.load(Ordering::Acquire){return;}
    let base=GetModuleHandleW(ptr::null()) as usize;
    if base!=IMAGE_BASE ||
       read_call(SET_SCALE_SITE)!=SET_SCALE_ORIGINAL ||
       read_call(SET_POSITION_SITE)!=SET_POSITION_ORIGINAL {
        crate::log("HUD-ENTITY-Y43: refused unexpected exact game callsite bytes; untouched");
        return;
    }
    FACTOR_BITS.store(factor.to_bits(),Ordering::Release);
    let scale_patch=relative_call(SET_SCALE_SITE,scale_y as usize);
    let position_patch=relative_call(SET_POSITION_SITE,position_y as usize);
    if !commit_call(SET_SCALE_SITE,&scale_patch){
        let _=commit_call(SET_SCALE_SITE,&SET_SCALE_ORIGINAL);
        crate::log("HUD-ENTITY-Y43: could not patch SetScale site; rollback attempted");
        return;
    }
    if !commit_call(SET_POSITION_SITE,&position_patch){
        let ok1=commit_call(SET_SCALE_SITE,&SET_SCALE_ORIGINAL);
        let ok2=commit_call(SET_POSITION_SITE,&SET_POSITION_ORIGINAL);
        crate::log(&format!(
            "HUD-ENTITY-Y43: could not patch SetPosition site; rollback scale={ok1} position={ok2}"));
        return;
    }
    INSTALLED.store(true,Ordering::Release);
    crate::log(&format!(
        "HUD-ENTITY-Y43: enabled=1 sites=2 vertical_factor={factor:.6} dimensions={w}x{h} \
         camera_fov_untouched=true original_entity_fields_untouched=true experiment_only=true"
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn native_wide_axis_math_matches_4by3_reference(){
        assert_eq!(correction(1920,1200),Some(1.2));
        assert!((correction(1920,1080).unwrap()-4.0/3.0).abs()<0.0001);
        assert_eq!(correction(1600,1200),None);
        assert_eq!(correction(800,600),None);
        assert_eq!(correction(0,0),None);
        assert_eq!(correction(1080,1920),None);
        assert!((adjusted_y(-1.0,1.2)+1.2).abs()<0.0001);
        assert_eq!(adjusted_y(0.0,1.2),0.0);
        assert!(adjusted_y(f32::NAN,1.2).is_nan());
    }
    #[test]
    fn direct_call_preimages_match_verified_original_targets(){
        let displacement=i32::from_le_bytes(SET_SCALE_ORIGINAL[1..5].try_into().unwrap());
        assert_eq!((SET_SCALE_SITE+5) as i64+displacement as i64,SET_SCALE_FUNCTION as i64);
        let displacement=i32::from_le_bytes(SET_POSITION_ORIGINAL[1..5].try_into().unwrap());
        assert_eq!((SET_POSITION_SITE+5) as i64+displacement as i64,SET_POSITION_FUNCTION as i64);
        assert_eq!(relative_call(SET_SCALE_SITE,SET_SCALE_FUNCTION),SET_SCALE_ORIGINAL);
        assert_eq!(relative_call(SET_POSITION_SITE,SET_POSITION_FUNCTION),SET_POSITION_ORIGINAL);
    }
}
