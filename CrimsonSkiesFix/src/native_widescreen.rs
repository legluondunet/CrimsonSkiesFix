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
pub unsafe fn apply_unwrapped_full_patch(w:u32,h:u32)->PatchResult{full_patch::apply(w,h)}

const IMAGE_BASE:usize=0x0040_0000;
const IMAGE_SIZE:usize=0x0065_2000;
const WIDTH_RVA:usize=0x0020_254b;
const HEIGHT_RVA:usize=0x0020_254f;
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
