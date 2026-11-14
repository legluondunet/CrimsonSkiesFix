//! Native 3D resolution and FOV/HUD adaptation of the single verified
//! SafeDisc-free Crimson Skies 1.02 executable. No third-party widescreen
//! EXE is recognized or modified. The exact on-disk EXE is authenticated
//! before this module can run; only a checked, in-memory transformation
//! is performed. Nothing changes on disk.
use std::ffi::c_void;

#[path="startup_selector.rs"]
mod startup_selector;
pub unsafe fn suppress_video_selector()->bool{startup_selector::apply()}

#[path="native_widescreen_full.rs"]
mod full_patch;

// Static x86 operand reference scan; debug-log-only WinMM diagnosis.
// Does not modify the verified image or hook any game entrypoint.
#[cfg(feature="winmm-bootstrap")]
#[path="aspect_reference_audit.rs"]
mod aspect_reference_audit;

#[cfg(feature="winmm-bootstrap")]
#[path="hud_fov43_probe.rs"]
mod hud_fov43_probe;

#[cfg(feature="winmm-bootstrap")]
#[path="hud_entity_y43_probe.rs"]
mod hud_entity_y43_probe;

#[cfg(feature="winmm-bootstrap")]
#[path="hud_canvas43_probe.rs"]
mod hud_canvas43_probe;

#[cfg(feature="winmm-bootstrap")]
#[path="hud_render_branch_audit.rs"]
mod hud_render_branch_audit;

#[cfg(feature="winmm-bootstrap")]
pub(crate) fn sample_hud_render_branch_on_game_flip(w:u32,h:u32){
    let authenticated=verified_launch_widescreen_dimensions().is_some();
    unsafe{hud_render_branch_audit::sample((w,h),authenticated);}
}

#[cfg(feature="winmm-bootstrap")]
fn audit_aspect_xrefs_once(result:PatchResult){
    if !crate::cfg().log_enabled ||
       !matches!(result,PatchResult::Patched|PatchResult::AlreadyNative){
        return;
    }
    static ONCE:std::sync::Once=std::sync::Once::new();
    ONCE.call_once(||unsafe{aspect_reference_audit::audit_verified_loaded_exe()});
}

use std::sync::OnceLock;

// The historical prepatched EXE had the FOV/HUD code present when the
// process started. Applying that same transformation only at the game's
// late DD4 exclusive negotiation can miss 4:3 HUD/camera initialization.
// Prime on the earliest DirectDraw export when exactly one monitor exists,
// before DirectDraw setup, never in DllMain. Defer on multi-monitor hosts.
static EARLY_NATIVE_PATCH:OnceLock<(u32,u32,PatchResult)>=OnceLock::new();

// The game can switch the actual monitor to 640x480/800x600 while in
// Preferences, even though its native target was captured at first DDRAW use.
// Share the byte-verified, frozen launch resolution with the resolution text
// formatter. Never re-read a transient current display mode for that label.
fn confirmed_early_pixels(entry:Option<(u32,u32,PatchResult)>)->Option<(u32,u32)>{
    match entry {
        Some((w,h,PatchResult::Patched|PatchResult::AlreadyNative))=>Some((w,h)),
        _=>None,
    }
}
pub fn verified_launch_widescreen_dimensions()->Option<(u32,u32)>{
    confirmed_early_pixels(EARLY_NATIVE_PATCH.get().copied())
}

pub unsafe fn prime_native_widescreen_early(w:u32,h:u32){
    if EARLY_NATIVE_PATCH.get().is_some(){return;}
    // apply() itself verifies exact PE headers and every original byte,
    // and performs fail-closed memory protection and rollback.
    let result=full_patch::apply(w,h);
    let _=EARLY_NATIVE_PATCH.set((w,h,result));
    #[cfg(feature="winmm-bootstrap")]
    audit_aspect_xrefs_once(result);
    #[cfg(feature="winmm-bootstrap")]
    hud_fov43_probe::apply(w,h,result);
    #[cfg(feature="winmm-bootstrap")]
    hud_entity_y43_probe::install(w,h,result);
    #[cfg(feature="winmm-bootstrap")]
    hud_canvas43_probe::install(w,h,result);
    crate::log(&format!(
        "WINDOWED-WIDE: early DirectDraw native FOV/HUD patch monitor={}x{} result={result:?}",
        w,h
    ));
}

pub unsafe fn apply_or_reuse_early_patch(w:u32,h:u32)->PatchResult{
    if let Some(&(early_w,early_h,result))=EARLY_NATIVE_PATCH.get(){
        if (early_w,early_h)!=(w,h){
            crate::log("WINDOWED-WIDE: early monitor and game HWND monitor differ; native mode refused");
            return PatchResult::InvalidMonitor;
        }
        crate::log(&format!(
            "WINDOWED-WIDE: reused early native FOV/HUD patch; monitor={}x{} result={result:?}",
            w,h
        ));
        return result; // NEVER apply a second time to already-patched bytes.
    }
    // Multi-monitor fallback retains the previous, late HWND-specific path.
    let result=full_patch::apply(w,h);
    #[cfg(feature="winmm-bootstrap")]
    audit_aspect_xrefs_once(result);
    #[cfg(feature="winmm-bootstrap")]
    hud_fov43_probe::apply(w,h,result);
    #[cfg(feature="winmm-bootstrap")]
    hud_entity_y43_probe::install(w,h,result);
    #[cfg(feature="winmm-bootstrap")]
    hud_canvas43_probe::install(w,h,result);
    result
}

const IMAGE_BASE:usize=0x0040_0000;
const IMAGE_SIZE:usize=0x0065_2000;
const PAGE_EXECUTE_READWRITE:u32=0x40;

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(module:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,len:usize,new:u32,old:*mut u32)->i32;
    fn GetCurrentProcess()->*mut c_void;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,len:usize)->i32;
}

fn valid_landscape_dimensions(w:u32,h:u32)->bool{
    w>=960 && h>=600 && w<=8192 && h<=4320 &&
    (w as u64)*(h as u64)<=33_554_432 &&
    (w as u64)*3 >= (h as u64)*4 && // >= 4:3
    (w as u64)<= (h as u64)*4       // reject implausible ratios
}

fn get32(bytes:&[u8],at:usize)->Option<u32>{
    Some(u32::from_le_bytes(bytes.get(at..at+4)?.try_into().ok()?))
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PatchResult{
    Patched,
    AlreadyNative,
    UnsupportedExecutable,
    InvalidMonitor,
    ProtectFailed,
    RestoreProtectionFailed,
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn frozen_native_pixels_require_authenticated_successful_patch(){
        assert_eq!(confirmed_early_pixels(Some((1920,1200,PatchResult::Patched))),
                   Some((1920,1200)));
        assert_eq!(confirmed_early_pixels(Some((1920,1200,PatchResult::AlreadyNative))),
                   Some((1920,1200)));
        assert_eq!(confirmed_early_pixels(Some((1920,1200,PatchResult::UnsupportedExecutable))),
                   None);
        assert_eq!(confirmed_early_pixels(Some((1920,1200,PatchResult::InvalidMonitor))),None);
        assert_eq!(confirmed_early_pixels(None),None);
    }

    #[test]
    fn valid_landscape_resolution_bounds(){
        for (w,h) in [(1280,720),(1680,1050),(1920,1080),(1920,1200),
                      (2560,1080),(3440,1440),(3840,2160),(5120,1440)]{
            assert!(valid_landscape_dimensions(w,h));
        }
        for (w,h) in [(0,0),(800,600),(1080,1920),(1024,4096),
                      (20_000,500),(8192,8192)]{
            assert!(!valid_landscape_dimensions(w,h));
        }
    }
}
