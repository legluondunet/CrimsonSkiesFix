//! Diagnostic-only native Windows game IAT cursor provenance.
//! The SHA256-admitted EXE imports USER32!SetCursorPos, ClipCursor, and
//! ClientToScreen at exact PE32 IAT RVAs. Before ever remapping mouse
//! coordinates, trace the game's requests and returned Win32 results.
//! NEVER changes an argument, cursor clipping, process DPI or input state;
//! no Wine/Proton or normal (NativeDD4Bootstrap=0) activation.
use std::{ffi::{c_char,c_void},sync::atomic::{AtomicUsize,Ordering}};
use crate::{diagnostics,log,win32};

#[repr(C)]
#[derive(Copy,Clone)]
struct Point{x:i32,y:i32}
#[repr(C)]
#[derive(Copy,Clone)]
struct Rect{left:i32,top:i32,right:i32,bottom:i32}
type SetPosFn=unsafe extern "system" fn(i32,i32)->i32;
type ClipFn=unsafe extern "system" fn(*const Rect)->i32;
type ClientFn=unsafe extern "system" fn(*mut c_void,*mut Point)->i32;

const SET_CURSOR_POS_RVA:usize=0x0062_0434;
const CLIP_CURSOR_RVA:usize=0x0062_0460;
const CLIENT_TO_SCREEN_RVA:usize=0x0062_03e8;

static SET_ORIG:AtomicUsize=AtomicUsize::new(0);
static CLIP_ORIG:AtomicUsize=AtomicUsize::new(0);
static CLIENT_ORIG:AtomicUsize=AtomicUsize::new(0);
static SET_COUNT:AtomicUsize=AtomicUsize::new(0);
static CLIP_COUNT:AtomicUsize=AtomicUsize::new(0);
static CLIENT_COUNT:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}

fn sample(n:usize)->bool {n<12 || (n<480 && n%120==0)}

unsafe extern "system" fn observe_set_cursor_pos(x:i32,y:i32)->i32{
    let original=SET_ORIG.load(Ordering::Acquire);
    if original==0 || original==observe_set_cursor_pos as usize {return 0;}
    let call:SetPosFn=std::mem::transmute(original);
    let result=call(x,y);
    let n=SET_COUNT.fetch_add(1,Ordering::Relaxed);
    if sample(n) {
        log(&format!(
            "NATIVE-CURSOR: game SetCursorPos({x},{y}) result={result} count={} (unchanged)",
            n+1
        ));
    }
    result
}
unsafe extern "system" fn observe_clip_cursor(rect:*const Rect)->i32{
    let original=CLIP_ORIG.load(Ordering::Acquire);
    if original==0 || original==observe_clip_cursor as usize {return 0;}
    let requested=if rect.is_null() {None} else {
        Some(((*rect).left,(*rect).top,(*rect).right,(*rect).bottom))
    };
    let call:ClipFn=std::mem::transmute(original);
    let result=call(rect);
    let n=CLIP_COUNT.fetch_add(1,Ordering::Relaxed);
    if sample(n) {
        log(&format!(
            "NATIVE-CURSOR: game ClipCursor requested={requested:?} result={result} count={} (unchanged)",
            n+1
        ));
    }
    result
}
unsafe extern "system" fn observe_client_to_screen(hwnd:*mut c_void,point:*mut Point)->i32{
    let original=CLIENT_ORIG.load(Ordering::Acquire);
    if original==0 || original==observe_client_to_screen as usize {return 0;}
    let before=if point.is_null(){None}else{Some(((*point).x,(*point).y))};
    let call:ClientFn=std::mem::transmute(original);
    let result=call(hwnd,point);
    let n=CLIENT_COUNT.fetch_add(1,Ordering::Relaxed);
    if sample(n) {
        let after=if point.is_null(){None}else{Some(((*point).x,(*point).y))};
        log(&format!(
            "NATIVE-CURSOR: game ClientToScreen point={before:?} mapped={after:?} success={} hwnd_provided={} count={} (unchanged)",
            result!=0,!hwnd.is_null(),n+1
        ));
    }
    result
}

unsafe fn hook_import(
    base:usize,user32:*mut c_void,
    rva:usize,name:&'static [u8],replacement:usize,
    slot_orig:&AtomicUsize
)->bool{
    if base==0 || base.checked_add(rva).is_none(){return false;}
    let expected=GetProcAddress(user32,name.as_ptr().cast()) as usize;
    let slot=(base+rva) as *mut usize;
    let previous=slot.read();
    // Verifies this really is the OS-provided named function at the
    // authenticated game's exact IAT address. Fail closed for aliases.
    if expected==0 || previous!=expected || previous==replacement{return false;}
    if slot_orig.compare_exchange(0,previous,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(slot,replacement){
        slot_orig.store(0,Ordering::Release);
        return false;
    }
    true
}
pub fn install(){
    let cfg=crate::cfg();
    if !diagnostics::is_native_windows() || !cfg.log_enabled ||
       !cfg.native_dd4_bootstrap || cfg.directdraw_passthrough {return;}
    unsafe {
        let base=GetModuleHandleW(std::ptr::null()) as usize;
        let mut module:[u16;11]=[0;11];
        for (i,ch) in "user32.dll".encode_utf16().enumerate(){module[i]=ch;}
        let user32=GetModuleHandleW(module.as_ptr());
        if user32.is_null(){
            log("NATIVE-CURSOR: no USER32 module; game IAT trace not installed");
            return;
        }
        let set=hook_import(base,user32,SET_CURSOR_POS_RVA,
            b"SetCursorPos\0",observe_set_cursor_pos as usize,&SET_ORIG);
        let clip=hook_import(base,user32,CLIP_CURSOR_RVA,
            b"ClipCursor\0",observe_clip_cursor as usize,&CLIP_ORIG);
        let client=hook_import(base,user32,CLIENT_TO_SCREEN_RVA,
            b"ClientToScreen\0",observe_client_to_screen as usize,&CLIENT_ORIG);
        log(&format!(
            "NATIVE-CURSOR: read-only game IAT trace SetCursorPos={set} ClipCursor={clip} ClientToScreen={client}; original calls preserved"
        ));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user32_game_import_rvas_and_sample_bound(){
        assert_eq!(SET_CURSOR_POS_RVA,0x620434);
        assert_eq!(CLIP_CURSOR_RVA,0x620460);
        assert_eq!(CLIENT_TO_SCREEN_RVA,0x6203e8);
        assert!(sample(0));
        assert!(sample(11));
        assert!(!sample(12));
        assert!(sample(120));
        assert!(!sample(481));
    }
    #[test]
    fn win32_cursor_pointer_types_match_win32_i686_abi(){
        let _:SetPosFn=observe_set_cursor_pos;
        let _:ClipFn=observe_clip_cursor;
        let _:ClientFn=observe_client_to_screen;
        assert_eq!(std::mem::size_of::<Rect>(),16);
        assert_eq!(std::mem::size_of::<Point>(),8);
    }
}
