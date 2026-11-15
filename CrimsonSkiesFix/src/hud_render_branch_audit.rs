//! Read-only native WinMM HUD renderer-path selection diagnosis.
//!
//! Original 1.02 disassembly:
//! 0x455F30 -> 0x455C60 HUD sprite pass
//! 0x455C60: if [0x009BE708] != 0 use accelerated branch 0x455C6C,
//!           which NEVER calls the legacy 640x480/320x240 canvas setters.
//!           Else enter 0x455C94 and query 0x440BB0 (Replicate config).
//!           Only if Replicate != 0 call the 0x4D7020 canvas setter.
//! 0x440BB0: mov eax,[0x0064F748]; mov eax,[eax]; ret
//! 0x44039F stores a pointer from registered "Replicate" into 0x64F748.
//!
//! We must not infer the live branch merely from the startup install
//! of ExperimentalHudCanvas43Probe, because that probe changes dormant
//! instruction bytes even when the game never executes their branch.
//!
//! This module samples the two read-only globals at the already-validated
//! virtual 3D Flip boundary, WITHOUT detours, modifications or delays.
//! No sample is taken during the 800x600 menu/movie mode.
use std::{
    ffi::c_void,
    mem::{size_of,MaybeUninit},
    ptr,
    sync::atomic::{AtomicUsize,Ordering},
};
use super::{GetModuleHandleW,IMAGE_BASE};

const RENDERER_MODE_PTR:usize=0x009B_E708;
const REPLICATE_CVAR_PTR:usize=0x0064_F748;
const MEM_COMMIT:u32=0x1000;
const PAGE_NOACCESS:u32=0x01;
const PAGE_GUARD:u32=0x100;
static SAMPLES:AtomicUsize=AtomicUsize::new(0);

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
    fn VirtualQuery(
        address:*const c_void,
        buffer:*mut MemoryBasicInformation,
        bytes:usize
    )->usize;
}

unsafe fn readable_u32(addr:usize)->Option<u32>{
    if addr==0 || addr%4!=0 {return None;}
    let mut info=MaybeUninit::<MemoryBasicInformation>::zeroed();
    if VirtualQuery(addr as *const c_void,info.as_mut_ptr(),size_of::<MemoryBasicInformation>())
        !=size_of::<MemoryBasicInformation>() {return None;}
    let region=info.assume_init();
    if region.state!=MEM_COMMIT || region.protect&PAGE_GUARD!=0 ||
        region.protect&0xff==PAGE_NOACCESS {return None;}
    let start=region.base_address as usize;
    let end=start.checked_add(region.region_size)?;
    if addr<start || addr.checked_add(4)?>end{return None;}
    Some(ptr::read_volatile(addr as *const u32))
}

fn classify_path(renderer:Option<u32>,replicate:Option<u32>)->&'static str{
    match renderer {
        Some(n) if n!=0=>"accelerated-no-legacy-canvas",
        Some(0)=>match replicate{
            Some(0)=>"legacy-replicate-off-no-canvas",
            Some(_)=>"legacy-replicate-on-canvas-setter",
            None=>"legacy-replicate-unavailable",
        },
        Some(_)=>"accelerated-no-legacy-canvas",
        None=>"renderer-unavailable",
    }
}

pub(super) unsafe fn sample(mode:(u32,u32),authenticated:bool){
    // The verified virtual Surface4 gameplay Flip exists under both
    // native Windows and Wine/Proton; keep this diagnostic cross-platform.
    // The two fixed addresses belong to the same SHA-locked 1.02 EXE.
    if !authenticated || !crate::cfg().log_enabled ||
        mode.0<960 || mode.1<600 ||
        (mode.0 as u64)*3<=(mode.1 as u64)*4 {return;}
    let n=SAMPLES.fetch_add(1,Ordering::Relaxed)+1;
    if n>4 && n%1024!=0{return;}
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE {return;}
    let renderer=readable_u32(RENDERER_MODE_PTR);
    let replicate=readable_u32(REPLICATE_CVAR_PTR)
        .and_then(|address|readable_u32(address as usize));
    let selected=classify_path(renderer,replicate);
    crate::log(&format!(
        "HUD-RENDER-PATH: sample={n} mode={}x{} renderer_mode={renderer:?} \
         replicate={replicate:?} branch={selected} \
         legacy_canvas_640x480_executed_possible={} read_only=true platform_native_windows={}",
        mode.0,mode.1,
        selected=="legacy-replicate-on-canvas-setter",
        crate::diagnostics::is_native_windows()
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn accelerated_hud_skips_legacy_canvas_even_with_replicate_enabled(){
        for renderer in [1,2,3]{
            for replicate in [None,Some(0),Some(1)]{
                assert_eq!(classify_path(Some(renderer),replicate),
                           "accelerated-no-legacy-canvas");
            }
        }
    }
    #[test]
    fn legacy_requires_explicit_replicate_enabled(){
        assert_eq!(classify_path(Some(0),Some(0)),"legacy-replicate-off-no-canvas");
        assert_eq!(classify_path(Some(0),Some(1)),"legacy-replicate-on-canvas-setter");
        assert_eq!(classify_path(Some(0),None),"legacy-replicate-unavailable");
        assert_eq!(classify_path(None,Some(1)),"renderer-unavailable");
    }
    #[test]
    fn memory_basic_information_is_x86_abi_sized(){
        #[cfg(target_pointer_width="32")]
        assert_eq!(size_of::<MemoryBasicInformation>(),28);
    }
}
