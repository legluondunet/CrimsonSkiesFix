//! Experimental, native-Windows-only fallback for the VIDEO Preferences
//! resolution *text*. The original 0x845 GOS format call may not execute
//! on the WinMM Win11 path even though a 640x480 label is displayed.
//!
//! The game's genuine 0x403350 text-copy routine is a second, documented
//! text-ownership boundary: x86 original body allocates/duplicates the
//! supplied ANSI source string into the destination GOS text object.
//! Our wrapper forwards the original destination, type, ABI and return value.
//! Only an exact "640x480" or "640 x 480" label is replaced with the already
//! byte-authenticated *effective 3D raster* dimensions. This differs from the
//! physical monitor in forced 4:3 (e.g. 1600x1200 inside 1920x1200).
//! No mode table, GOS index,
//! rendering surface, widget flags, or game EXE on disk are modified.
//!
//! The x86 EXE's immutable prologue is checked at VA 0x403350 before patch.
//! The trampoline copies complete instructions (3+1+6 bytes) with no relative
//! operands, then jumps back at +10. Only WinMM on *native* Windows installs
//! it; Wine, legacy ddraw.dll and unknown game EXEs cannot use this route.
use crate::{diagnostics,log,native_widescreen};
use std::{
    ffi::{c_char,c_void,CStr,CString},
    ptr,
    sync::atomic::{AtomicU32,AtomicUsize,Ordering}
};

const TEXT_FN:usize=0x0040_3350;
const IMAGE_BASE:usize=0x0040_0000;
const PROLOGUE:[u8;10]=[0x83,0xec,0x18,0x53,0x8b,0x1d,0x88,0xfc,0xa1,0x00];
const PAGE_EXECUTE_READWRITE:u32=0x40;
const PAGE_EXECUTE_READ:u32=0x20;
const MEM_COMMIT:u32=0x1000;
const MEM_RESERVE:u32=0x2000;
const MEM_RELEASE:u32=0x8000;

static ORIGINAL_TRAMPOLINE:AtomicUsize=AtomicUsize::new(0);
static MATCHES:AtomicU32=AtomicU32::new(0);
static REPLACEMENTS:AtomicU32=AtomicU32::new(0);

type TextFn=unsafe extern "C" fn(*mut c_void,*const c_char,u32)->u32;

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualAlloc(address:*mut c_void,size:usize,kind:u32,protection:u32)->*mut c_void;
    fn VirtualFree(address:*mut c_void,size:usize,kind:u32)->i32;
    fn VirtualProtect(address:*mut c_void,len:usize,new:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,len:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

fn is_resolution_label(raw:&[u8])->bool{
    matches!(raw,b"640x480"|b"640 x 480"|b"640 X 480")
}
fn replacement_for_label(raw:&[u8],verified_3d:Option<(u32,u32)>,menu_was_seen:bool,
                         graphics_features:bool,forced_4by3:bool)->Option<String>{
    if !menu_was_seen || !graphics_features || !is_resolution_label(raw){return None;}
    let (w,h)=verified_3d?;
    // Explicit 4:3 *must* use the centered 3D raster. Never claim
    // the full monitor width or the historic 640x480 GOS selection.
    if forced_4by3 && (w as u64)*3!=(h as u64)*4{return None;}
    if !(960..=8192).contains(&w) || !(600..=4320).contains(&h) ||
       (w as u64)*3<(h as u64)*4 || w as u64>(h as u64)*4 {return None;}
    if raw.contains(&b' ') {
        Some(format!("{w} x {h}"))
    }else{
        Some(format!("{w}x{h}"))
    }
}
fn relative_jump(from:usize,to:usize)->[u8;5]{
    let mut b=[0xe9u8,0,0,0,0];
    b[1..].copy_from_slice(&(to as u32).wrapping_sub((from as u32).wrapping_add(5)).to_le_bytes());
    b
}

unsafe extern "C" fn copy_text(dst:*mut c_void,source:*const c_char,flags:u32)->u32{
    let original=ORIGINAL_TRAMPOLINE.load(Ordering::Acquire);
    if original==0{return 0;}
    let call:TextFn=std::mem::transmute(original);
    if source.is_null(){return call(dst,source,flags);}
    // The original function itself invokes strlen() on this parameter.
    // No additional, unverified pointer chain is dereferenced.
    let text=CStr::from_ptr(source).to_bytes();
    if !is_resolution_label(text){return call(dst,source,flags);}

    let config=crate::cfg();
    let verified_3d=native_widescreen::verified_launch_widescreen_dimensions();
    let matched=MATCHES.fetch_add(1,Ordering::Relaxed);
    let target=replacement_for_label(
        text,verified_3d,crate::menu_scene::main_menu_was_seen(),
        config.experimental_windowed_adapter && config.native_3d_widescreen &&
        config.desktop_resolution, config.force_4_by_3,
    );
    if let Some(target)=target {
        // The string is copied synchronously by the original GOS routine;
        // it never receives a pointer to temporary Rust-owned data afterward.
        let replacement=CString::new(target.as_str()).expect("ASCII dimensions only");
        let ret=call(dst,replacement.as_ptr(),flags);
        let done=REPLACEMENTS.fetch_add(1,Ordering::Relaxed);
        if done<12 {
            log(&format!("MENU-WINMM-TEXT: original_GOS=640x480 shown_3D_render={target} aspect_mode={} source=verified_patched_3d_raster kind=text-only result=forwarded; GOS mode index/table unchanged",
                if config.force_4_by_3{"4:3"}else{"auto"}
            ));
        }
        return ret;
    }
    if matched<8 {
        log(&format!(
            "MENU-WINMM-TEXT: original_GOS=640x480 not-replaced; main_menu_seen={} verified_3d={verified_3d:?} aspect_mode={} features_enabled={}; text call forwarded",
            crate::menu_scene::main_menu_was_seen(),
            if config.force_4_by_3{"4:3"}else{"auto"},
            config.experimental_windowed_adapter && config.native_3d_widescreen && config.desktop_resolution));
    }
    call(dst,source,flags)
}

pub unsafe fn install()->bool{
    if !diagnostics::is_native_windows(){return false;}
    let config=crate::cfg();
    if !config.experimental_windowed_adapter || !config.native_3d_widescreen ||
       !config.desktop_resolution {return false;}
    if ORIGINAL_TRAMPOLINE.load(Ordering::Acquire)!=0 {return true;}
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE {
        log("MENU-WINMM-TEXT: original game image base mismatch; no installation");
        return false;
    }
    let entry=TEXT_FN as *mut u8;
    if std::slice::from_raw_parts(entry,PROLOGUE.len())!=PROLOGUE {
        log("MENU-WINMM-TEXT: verified GOS text writer prologue differs; no installation");
        return false;
    }
    let trampoline=VirtualAlloc(ptr::null_mut(),32,MEM_RESERVE|MEM_COMMIT,
                                PAGE_EXECUTE_READWRITE).cast::<u8>();
    if trampoline.is_null(){
        log("MENU-WINMM-TEXT: trampoline allocation refused");
        return false;
    }
    ptr::copy_nonoverlapping(PROLOGUE.as_ptr(),trampoline,PROLOGUE.len());
    let back=relative_jump(trampoline as usize+PROLOGUE.len(),TEXT_FN+PROLOGUE.len());
    ptr::copy_nonoverlapping(back.as_ptr(),trampoline.add(PROLOGUE.len()),back.len());
    let mut old_trampoline=0;
    if FlushInstructionCache(GetCurrentProcess(),trampoline.cast(),15)==0 ||
       VirtualProtect(trampoline.cast(),32,PAGE_EXECUTE_READ,&mut old_trampoline)==0{
        let _=VirtualFree(trampoline.cast(),0,MEM_RELEASE);
        log("MENU-WINMM-TEXT: protected trampoline creation failed; game unchanged");
        return false;
    }
    let mut previous=0;
    if VirtualProtect(entry.cast(),PROLOGUE.len(),PAGE_EXECUTE_READWRITE,
                      &mut previous)==0 {
        let _=VirtualFree(trampoline.cast(),0,MEM_RELEASE);
        log("MENU-WINMM-TEXT: game text writer is not writable; game unchanged");
        return false;
    }
    // Publish trampoline before installing jump. No call to the new hook
    // may encounter a not-yet-published original pointer.
    ORIGINAL_TRAMPOLINE.store(trampoline as usize,Ordering::Release);
    let jump=relative_jump(TEXT_FN,copy_text as usize);
    ptr::copy_nonoverlapping(jump.as_ptr(),entry,5);
    ptr::write_bytes(entry.add(5),0x90,PROLOGUE.len()-5);
    let flushed=FlushInstructionCache(GetCurrentProcess(),entry.cast(),PROLOGUE.len())!=0;
    if !flushed {
        ptr::copy_nonoverlapping(PROLOGUE.as_ptr(),entry,PROLOGUE.len());
        let _=FlushInstructionCache(GetCurrentProcess(),entry.cast(),PROLOGUE.len());
        ORIGINAL_TRAMPOLINE.store(0,Ordering::Release);
    }
    let mut ignored=0;
    let protected=VirtualProtect(entry.cast(),PROLOGUE.len(),previous,&mut ignored)!=0;
    if !flushed {
        log("MENU-WINMM-TEXT: hook flush failed; original bytes restored");
        return false;
    }
    if !protected {
        log("MENU-WINMM-TEXT: warning: page protection restore failed, hook active");
    }
    log("MENU-WINMM-TEXT: validated native GOS 0x403350 text writer hooked (exact 640x480 labels only; no GOS modes/controls changed)");
    true
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_original_gos_text_copy_abi(){
        let _:TextFn=copy_text;
        assert_eq!(TEXT_FN,0x403350);
        assert_eq!(PROLOGUE,[0x83,0xec,0x18,0x53,0x8b,0x1d,0x88,0xfc,0xa1,0x00]);
        assert_eq!(relative_jump(TEXT_FN,TEXT_FN+0x100),
                   [0xe9,0xfb,0,0,0]);
    }
    #[test]
    fn native_winmm_text_rewrite_requires_verified_context(){
        let valid=Some((1920,1200));
        for old in [b"640x480".as_slice(),b"640 x 480",b"640 X 480"] {
            let updated=replacement_for_label(old,valid,true,true,false).unwrap();
            assert_eq!(updated,if old.contains(&b' '){"1920 x 1200"}else{"1920x1200"});
            assert_eq!(replacement_for_label(old,valid,false,true,false),None);
            assert_eq!(replacement_for_label(old,valid,true,false,false),None);
            assert_eq!(replacement_for_label(old,None,true,true,false),None);
            let four_by_three=replacement_for_label(
                old,Some((1600,1200)),true,true,true).unwrap();
            assert_eq!(four_by_three,if old.contains(&b' '){"1600 x 1200"}else{"1600x1200"});
            assert_eq!(replacement_for_label(old,valid,true,true,true),None);
        }
        assert_eq!(replacement_for_label(b"800x600",valid,true,true,false),None);
        assert_eq!(replacement_for_label(b"640x480 Hz",valid,true,true,false),None);
        assert_eq!(replacement_for_label(b"graphics 640x480",valid,true,true,false),None);
        assert_eq!(replacement_for_label(b"640x480",Some((0,0)),true,true,false),None);
        assert_eq!(replacement_for_label(b"640x480",Some((1920,1080)),true,true,false),
                   Some("1920x1080".into()));
    }
}
