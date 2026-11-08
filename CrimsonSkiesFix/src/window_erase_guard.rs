//! Opt-in, narrowly scoped Win32 background-erase suppression experiment.
//!
//! Only the exact process-owned game HWND provided by the DD4 adapter can
//! be subclassed. We do not paint, call DirectDraw, allocate or log inside
//! the window procedure. All messages except WM_ERASEBKGND go untouched
//! to the original procedure. It is unhooked during WM_NCDESTROY.
//! This does not guarantee that WM_PAINT never paints white itself.
#![allow(dead_code)]
use std::{ffi::c_void,sync::atomic::{AtomicIsize,AtomicUsize,Ordering}};

type Hwnd=*mut c_void;
const GWL_WNDPROC:i32=-4;
const WM_ERASEBKGND:u32=0x0014;
const WM_PAINT:u32=0x000f;
const WM_NCDESTROY:u32=0x0082;
const WM_KEYDOWN:u32=0x0100;
const WM_KEYUP:u32=0x0101;
const WM_CHAR:u32=0x0102;
const WM_SYSKEYDOWN:u32=0x0104;
const WM_SYSKEYUP:u32=0x0105;
const WM_SETFOCUS:u32=0x0007;
const WM_KILLFOCUS:u32=0x0008;
const WM_ACTIVATEAPP:u32=0x001c;

#[link(name="user32")]
extern "system"{
    fn IsWindow(hwnd:Hwnd)->i32;
    fn GetForegroundWindow()->Hwnd;
    fn GetInputState()->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,pid:*mut u32)->u32;
    fn GetWindowLongW(hwnd:Hwnd,index:i32)->i32;
    fn SetWindowLongW(hwnd:Hwnd,index:i32,value:i32)->i32;
    fn CallWindowProcW(proc:isize,hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize;
    fn DefWindowProcW(hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize;
}
#[link(name="kernel32")]
extern "system"{
    fn GetCurrentProcessId()->u32;
    fn GetCurrentThreadId()->u32;
    fn GetLastError()->u32;
    fn SetLastError(code:u32);
}

static HOOK_HWND:AtomicUsize=AtomicUsize::new(0);
static PREVIOUS:AtomicIsize=AtomicIsize::new(0);
static ERASE_COUNT:AtomicUsize=AtomicUsize::new(0);
static PAINT_COUNT:AtomicUsize=AtomicUsize::new(0);
// Passive observations only. Never swallow, synthesize or dispatch input.
static KEYDOWN_COUNT:AtomicUsize=AtomicUsize::new(0);
static KEYUP_COUNT:AtomicUsize=AtomicUsize::new(0);
static CHAR_COUNT:AtomicUsize=AtomicUsize::new(0);
static FOCUS_COUNT:AtomicUsize=AtomicUsize::new(0);

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowPaintStats{
    pub erased:usize,pub painted:usize,
    pub keydown:usize,pub keyup:usize,pub character:usize,pub focus_changes:usize,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowInputState{
    pub counts:WindowPaintStats,
    pub game_is_foreground:bool,
    // GetInputState reports the CALLING thread's queue, not all windows.
    pub queue_has_new_input:bool,
    pub window_thread:u32,
    pub current_thread:u32,
}
fn input_event(msg:u32)->(bool,bool,bool,bool){
    (matches!(msg,WM_KEYDOWN|WM_SYSKEYDOWN),
     matches!(msg,WM_KEYUP|WM_SYSKEYUP),
     msg==WM_CHAR,
     matches!(msg,WM_SETFOCUS|WM_KILLFOCUS|WM_ACTIVATEAPP))
}

fn intercept_erase(message:u32,matching:bool)->bool{
    message==WM_ERASEBKGND && matching
}

unsafe extern "system" fn guarded_proc(hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize{
    let matching=HOOK_HWND.load(Ordering::Acquire)==hwnd as usize;
    let previous=PREVIOUS.load(Ordering::Acquire);

    if matching && intercept_erase(msg,true){
        ERASE_COUNT.fetch_add(1,Ordering::Relaxed);
        return 1; // WM_ERASEBKGND handled: leave DirectDraw pixels intact.
    }
    if matching && msg==WM_PAINT{
        PAINT_COUNT.fetch_add(1,Ordering::Relaxed);
    }
    if matching{
        let (down,up,character,focus)=input_event(msg);
        if down{KEYDOWN_COUNT.fetch_add(1,Ordering::Relaxed);}
        if up{KEYUP_COUNT.fetch_add(1,Ordering::Relaxed);}
        if character{CHAR_COUNT.fetch_add(1,Ordering::Relaxed);}
        if focus{FOCUS_COUNT.fetch_add(1,Ordering::Relaxed);}
    }
    if matching && msg==WM_NCDESTROY{
        // Keep the original chain in place for the final destroy callback.
        // Restore only when our hook is still the active window procedure.
        if previous!=0 && GetWindowLongW(hwnd,GWL_WNDPROC)==guarded_proc as *const () as usize as i32{
            let _=SetWindowLongW(hwnd,GWL_WNDPROC,previous as i32);
        }
        HOOK_HWND.store(0,Ordering::Release);
        PREVIOUS.store(0,Ordering::Release);
    }
    if previous==0{
        // Narrow installation race: do not recurse or dispatch via a NULL
        // callback. The original is captured immediately after SetWindowLongW.
        return DefWindowProcW(hwnd,msg,wparam,lparam);
    }
    CallWindowProcW(previous,hwnd,msg,wparam,lparam)
}

/// Install once on the exact user-supplied, process-owned gameplay HWND.
/// Must only be invoked after borderless window setup and successful DD4
/// geometry validation. No process-wide hook and no class brush changes.
pub fn install(hwnd:usize)->Result<(),String>{
    if hwnd==0{return Err("NULL HWND".into());}
    let window=hwnd as Hwnd;
    unsafe{
        if IsWindow(window)==0{return Err("invalid game HWND".into());}
        let mut pid=0;
        if GetWindowThreadProcessId(window,&mut pid)==0 || pid!=GetCurrentProcessId(){
            return Err("HWND not owned by this process".into());
        }
        let old=HOOK_HWND.load(Ordering::Acquire);
        if old==hwnd{return if PREVIOUS.load(Ordering::Acquire)!=0{
            Ok(())
        }else{Err("window subclass still initializing".into())};}
        if old!=0{return Err("another HWND already guarded".into());}
        if HOOK_HWND.compare_exchange(0,hwnd,Ordering::AcqRel,Ordering::Acquire).is_err(){
            return Err("concurrent hook installation".into());
        }
        SetLastError(0);
        let previous=SetWindowLongW(window,GWL_WNDPROC,guarded_proc as *const () as usize as i32);
        let error=GetLastError();
        if previous==0 && error!=0{
            HOOK_HWND.store(0,Ordering::Release);
            return Err(format!("SetWindowLongW WNDPROC failed Win32={error}"));
        }
        if previous==0{
            HOOK_HWND.store(0,Ordering::Release);
            return Err("NULL previous game WndProc".into());
        }
        PREVIOUS.store(previous as isize,Ordering::Release);
        Ok(())
    }
}

pub fn snapshot(hwnd:usize)->Option<WindowPaintStats>{
    (hwnd!=0 && HOOK_HWND.load(Ordering::Acquire)==hwnd &&
        PREVIOUS.load(Ordering::Acquire)!=0).then_some(WindowPaintStats{
            erased:ERASE_COUNT.load(Ordering::Acquire),
            painted:PAINT_COUNT.load(Ordering::Acquire),
            keydown:KEYDOWN_COUNT.load(Ordering::Acquire),
            keyup:KEYUP_COUNT.load(Ordering::Acquire),
            character:CHAR_COUNT.load(Ordering::Acquire),
            focus_changes:FOCUS_COUNT.load(Ordering::Acquire),
        })
}

/// Read-only sample on the calling virtual Flip thread. This does not
/// call PeekMessage/DispatchMessage or modify DirectShow synchronization.
pub fn sample_input_state(hwnd:usize)->Option<WindowInputState>{
    let counts=snapshot(hwnd)?;
    let hwnd=hwnd as Hwnd;
    unsafe{
        if IsWindow(hwnd)==0{return None;}
        let window_thread=GetWindowThreadProcessId(hwnd,std::ptr::null_mut());
        if window_thread==0{return None;}
        Some(WindowInputState{
            counts,
            game_is_foreground:GetForegroundWindow()==hwnd,
            queue_has_new_input:GetInputState()!=0,
            window_thread,
            current_thread:GetCurrentThreadId(),
        })
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_the_guarded_window_erase_message_is_suppressed(){
        assert!(intercept_erase(WM_ERASEBKGND,true));
        assert!(!intercept_erase(WM_ERASEBKGND,false));
        for m in [WM_PAINT,WM_NCDESTROY,0,0x0006,0x0007,0x0010]{
            assert!(!intercept_erase(m,true));
        }
    }
    #[test]
    fn passive_input_message_classification_forwards_every_message(){
        assert_eq!(input_event(WM_KEYDOWN),(true,false,false,false));
        assert_eq!(input_event(WM_SYSKEYDOWN),(true,false,false,false));
        assert_eq!(input_event(WM_KEYUP),(false,true,false,false));
        assert_eq!(input_event(WM_SYSKEYUP),(false,true,false,false));
        assert_eq!(input_event(WM_CHAR),(false,false,true,false));
        assert_eq!(input_event(WM_KILLFOCUS),(false,false,false,true));
        assert_eq!(input_event(WM_ACTIVATEAPP),(false,false,false,true));
        assert_eq!(input_event(WM_PAINT),(false,false,false,false));
    }
    #[test]
    fn guarded_window_stats_require_installed_window(){
        assert!(snapshot(0).is_none());
    }
}
