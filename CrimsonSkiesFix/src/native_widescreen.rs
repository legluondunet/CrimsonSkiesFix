//! Optional runtime-native dimensions for the owner-supplied, already-patched
//! 1.0.2.0 executable. This NEVER patches an arbitrary stock executable.
//!
//! Binary analysis of CRIMSON(5).EXE (SHA-256 starts 5578a15ce072d096):
//!   preferred image base 0x00400000;
//!   DWORD game 3D width  RVA 0x0020254b;
//!   DWORD game 3D height RVA 0x0020254f;
//!   mode select xrefs at RVA 0x00041229 / 0x0004122f;
//!   widescreen mode globals xrefs RVA 0x001a8eba / 0x001a8fc9;
//!   injected FPU camera code RVA 0x002024bd.
//! The patched EXE reads the same two DWORDs in both mode selection and
//! its horizontal-FOV calculations. These are *patch fingerprints*, not
//! screen resolution constants. Screen dimensions always come from
//! MonitorFromWindow / GetMonitorInfoW after validating the game's HWND.
//!
//! The Windows-only mutator applies only to an EXE with the exact shape
//! and code signatures. Patch is default-off and must be live-tested.
#![allow(dead_code)]
use std::{ffi::c_void,ptr};

const IMAGE_BASE:usize=0x0040_0000;
const IMAGE_SIZE:usize=0x0065_2000;
const WIDTH_RVA:usize=0x0020_254b;
const HEIGHT_RVA:usize=0x0020_254f;
const PAGE_EXECUTE_READWRITE:u32=0x40;

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(module:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,len:usize,new:u32,old:*mut u32)->i32;
}

fn valid_landscape_dimensions(w:u32,h:u32)->bool{
    w>=960 && h>=600 && w<=8192 && h<=4320 &&
    (w as u64)*(h as u64)<=33_554_432 &&
    (w as u64)*3 >= (h as u64)*4 && // >= 4:3
    (w as u64)<= (h as u64)*4        // guard implausible ratios
}

fn get32(bytes:&[u8],at:usize)->Option<u32>{
    Some(u32::from_le_bytes(bytes.get(at..at+4)?.try_into().ok()?))
}

fn check_abs_load(bytes:&[u8],rva:usize,prefix:[u8;2],target:usize)->bool{
    bytes.get(rva..rva+2)==Some(&prefix[..]) &&
    get32(bytes,rva+2)==Some(target as u32)
}

/// Testable signature match, no writes. Uses actual instruction bytes
/// rather than just matching a resolution pair.
fn is_owner_supplied_patch(bytes:&[u8],base:usize)->bool{
    if base!=IMAGE_BASE || bytes.len()<WIDTH_RVA+8{return false;}
    if bytes.get(0..2)!=Some(b"MZ"){return false;}
    let Some(ntoff)=get32(bytes,0x3c).map(|x|x as usize) else{return false;};
    if ntoff<0x40||ntoff>0x400{return false;}
    if bytes.get(ntoff..ntoff+4)!=Some(b"PE\0\0") || 
        bytes.get(ntoff+24..ntoff+26)!=Some(&[0x0b,0x01][..]) ||
        get32(bytes,ntoff+24+56)!=Some(IMAGE_SIZE as u32) ||
        get32(bytes,ntoff+24+28)!=Some(IMAGE_BASE as u32){
        return false;
    }
    let absolute_width=base+WIDTH_RVA;
    let absolute_height=base+HEIGHT_RVA;
    // WSGF-style game mode selection: push the two exact global DWORDs
    // rather than hardcoded 1920/1080 immediates.
    if !check_abs_load(bytes,0x41229,[0xff,0x35],absolute_height) ||
       !check_abs_load(bytes,0x4122f,[0xff,0x35],absolute_width) ||
       !check_abs_load(bytes,0x41243,[0xff,0x35],absolute_height) ||
       !check_abs_load(bytes,0x41249,[0xff,0x35],absolute_width) ||
       !check_abs_load(bytes,0x1a8eba,[0x8b,0x0d],absolute_width) ||
       !check_abs_load(bytes,0x1a8fc9,[0x8b,0x0d],absolute_height) {
        return false;
    }
    // Injected x87 code computing height/width for 3D FOV.
    if !check_abs_load(bytes,0x2024bd,[0xdb,0x05],absolute_height) ||
       !check_abs_load(bytes,0x2024c3,[0xda,0x35],absolute_width) {
        return false;
    }
    let (Some(w),Some(h))=(get32(bytes,WIDTH_RVA),get32(bytes,HEIGHT_RVA))
        else{return false;};
    valid_landscape_dimensions(w,h)
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

/// # Safety
/// Only callable on the game's render/setup thread before the first
/// widescreen SetDisplayMode or 3D scene initialization, while this EXE
/// and its code/data are mapped and no other thread mutates the 8-byte
/// patch pair. It is disabled by default. Never call from DllMain.
pub unsafe fn apply_monitor_resolution(width:u32,height:u32)->PatchResult{
    if !valid_landscape_dimensions(width,height){
        return PatchResult::InvalidMonitor;
    }
    let base=GetModuleHandleW(ptr::null());
    if base.is_null() || base as usize!=IMAGE_BASE{
        return PatchResult::UnsupportedExecutable;
    }
    let data=std::slice::from_raw_parts(base.cast::<u8>(),IMAGE_SIZE);
    if !is_owner_supplied_patch(data,base as usize){
        return PatchResult::UnsupportedExecutable;
    }
    if get32(data,WIDTH_RVA)==Some(width) && get32(data,HEIGHT_RVA)==Some(height){
        return PatchResult::AlreadyNative;
    }
    let target=(base as usize+WIDTH_RVA) as *mut c_void;
    let mut old=0u32;
    if VirtualProtect(target,8,PAGE_EXECUTE_READWRITE,&mut old)==0{
        return PatchResult::ProtectFailed;
    }
    let p=(base as usize+WIDTH_RVA) as *mut u32;
    ptr::write_unaligned(p,width);
    ptr::write_unaligned((base as usize+HEIGHT_RVA) as *mut u32,height);
    let mut ignore=0u32;
    if VirtualProtect(target,8,old,&mut ignore)==0{
        return PatchResult::RestoreProtectionFailed;
    }
    PatchResult::Patched
}

#[cfg(test)]
mod tests{
    use super::*;
    fn synthetic_patched_exe()->Vec<u8>{
        let mut x=vec![0u8;WIDTH_RVA+8];
        x[0..2].copy_from_slice(b"MZ");
        x[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        x[0x80..0x84].copy_from_slice(b"PE\0\0");
        x[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        x[0x98+28..0x98+32].copy_from_slice(&(IMAGE_BASE as u32).to_le_bytes());
        x[0x98+56..0x98+60].copy_from_slice(&(IMAGE_SIZE as u32).to_le_bytes());
        for (rva,prefix,target) in [
            (0x41229,[0xff,0x35],HEIGHT_RVA),(0x4122f,[0xff,0x35],WIDTH_RVA),
            (0x41243,[0xff,0x35],HEIGHT_RVA),(0x41249,[0xff,0x35],WIDTH_RVA),
            (0x1a8eba,[0x8b,0x0d],WIDTH_RVA),(0x1a8fc9,[0x8b,0x0d],HEIGHT_RVA),
            (0x2024bd,[0xdb,0x05],HEIGHT_RVA),(0x2024c3,[0xda,0x35],WIDTH_RVA)
        ]{
            x[rva..rva+2].copy_from_slice(&prefix);
            x[rva+2..rva+6].copy_from_slice(&((IMAGE_BASE+target) as u32).to_le_bytes());
        }
        x[WIDTH_RVA..WIDTH_RVA+4].copy_from_slice(&1920u32.to_le_bytes());
        x[HEIGHT_RVA..HEIGHT_RVA+4].copy_from_slice(&1080u32.to_le_bytes());
        x
    }
    #[test]
    fn exact_owner_executable_patch_signature_required(){
        let mut image=synthetic_patched_exe();
        assert!(is_owner_supplied_patch(&image,IMAGE_BASE));
        assert!(!is_owner_supplied_patch(&image,IMAGE_BASE+0x10000));
        image[0x1a8eba]=0x90;
        assert!(!is_owner_supplied_patch(&image,IMAGE_BASE));
        image[0x1a8eba]=0x8b;
        image[WIDTH_RVA..WIDTH_RVA+4].copy_from_slice(&0u32.to_le_bytes());
        assert!(!is_owner_supplied_patch(&image,IMAGE_BASE));
    }
    #[test]
    fn any_reasonable_native_landscape_size_can_replace_original_pair(){
        for (w,h) in [(1280,720),(1680,1050),(1920,1080),
                      (2560,1080),(3440,1440),(3840,2160),(5120,1440)]{
            assert!(valid_landscape_dimensions(w,h));
        }
        for (w,h) in [(0,0),(800,600),(1080,1920),(1024,4096),
                      (20_000,500),(8192,8192)]{
            assert!(!valid_landscape_dimensions(w,h));
        }
    }
}
