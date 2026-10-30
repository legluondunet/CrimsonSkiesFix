//! Experimental x86 DirectDraw proxy for the locally unpacked Crimson Skies 1.0.2.0.
//! Place this built DLL under the name ddraw.dll beside CRIMSON.EXE.
#![cfg(windows)]
mod config;
mod win32;
mod diagnostics;
mod diagnostics_graphics;

use config::Config;
use std::{collections::HashMap, ffi::c_void, fs::{self,File,OpenOptions}, io::Write,
          ptr, sync::{atomic::{AtomicUsize,Ordering}, Mutex,OnceLock,Once}};
use win32::Hwnd;

type Hr=i32;
type CreateFn=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*mut c_void)->Hr;
type EnumerateFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type CreateExFn=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*const c_void,*mut c_void)->Hr;
type CooperativeFn=unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr;
type DisplayModeFn=unsafe extern "system" fn(*mut c_void,u32,u32,u32)->Hr;
type SurfaceFn=unsafe extern "system" fn(*mut c_void,*mut c_void,*mut *mut c_void,*mut c_void)->Hr;
type FlipFn=unsafe extern "system" fn(*mut c_void,*mut c_void,u32)->Hr;
type BltFn=unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,*const c_void,u32,*mut c_void)->Hr;
type GetCapsFn=unsafe extern "system" fn(*mut c_void,*mut u32)->Hr;
type ModernDisplayModeFn=unsafe extern "system" fn(*mut c_void,u32,u32,u32,u32,u32)->Hr;
type QueryInterfaceFn=unsafe extern "system" fn(*mut c_void,*const Guid,*mut *mut c_void)->Hr;
type BltFastFn=unsafe extern "system" fn(*mut c_void,u32,u32,*mut c_void,*const c_void,u32)->Hr;
type UnlockFn=unsafe extern "system" fn(*mut c_void,*const c_void)->Hr;

#[repr(C)]
#[derive(Clone,Copy)]
struct Guid {d1:u32,d2:u16,d3:u16,d4:[u8;8]}
fn guid_label(iid:*const Guid)->String {
    if iid.is_null(){return "NULL".to_string();}
    let g=unsafe{&*iid};
    format!("{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.d1,g.d2,g.d3,g.d4[0],g.d4[1],g.d4[2],g.d4[3],
        g.d4[4],g.d4[5],g.d4[6],g.d4[7])
}

const S_OK:Hr=0;
const DDERR_GENERIC:Hr=0x8876_0000u32 as i32;
const DDSCL_NORMAL:u32=0x0000_0008;
const DDSCAPS_PRIMARYSURFACE:u32=0x0000_0200;
const DLL_PROCESS_ATTACH:u32=1;
// IDirectDraw v1 slots. Surface Flip/Blt/GetCaps slots are also stable in
// later surface interface versions; QueryInterface interception is TODO.
const COM_QUERY_INTERFACE:usize=0;
const DD_CREATE_SURFACE:usize=6;
const DD_SET_COOPERATIVE:usize=20;
const DD_SET_DISPLAY_MODE:usize=21;
const SURFACE_BLT:usize=5;
const SURFACE_BLT_FAST:usize=7;
const SURFACE_UNLOCK:usize=32;
const SURFACE_FLIP:usize=11;
const SURFACE_GET_CAPS:usize=14;

static MODULE:AtomicUsize=AtomicUsize::new(0);
static CONFIG:OnceLock<Config>=OnceLock::new();
static LOG:OnceLock<Mutex<File>>=OnceLock::new();
static REAL_DLL:OnceLock<Option<usize>>=OnceLock::new();
static ORIGINALS:OnceLock<Mutex<HashMap<(usize,usize),usize>>>=OnceLock::new();
static LAST_FRAME:Mutex<i64>=Mutex::new(0);
static FREQUENCY:OnceLock<i64>=OnceLock::new();
static FIRST_FLIP:Once=Once::new();
static FIRST_BLT:Once=Once::new();
static COOPERATIVE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static DDRAW_QI_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static SURFACE_QI_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static WINDOW_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static FIRST_BLT_FAST:Once=Once::new();
static FIRST_UNLOCK:Once=Once::new();
static FIRST_MODERN_FLIP:Once=Once::new();
static FIRST_MODERN_BLT:Once=Once::new();
static FIRST_MODERN_FAST:Once=Once::new();
static FIRST_MODERN_UNLOCK:Once=Once::new();
static MODERN_COOPERATIVE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_DISPLAY_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_SURFACE_CREATE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_SURFACE_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_DDRAW_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);

fn log(message:&str) {
    if let Some(log)=LOG.get() {
        if let Ok(mut file)=log.lock(){let _=writeln!(file,"{message}");}
    }
}

// All system inventory code now lives in Rust (no C FFI module).
fn cfg()->Config {
    *CONFIG.get_or_init(|| {
        // No filesystem operations occur until a game DirectDraw import is called.
        let ini=win32::ini_path();
        let parsed=ini.as_ref().and_then(|p|fs::read_to_string(p).ok())
            .map(|s|config::parse(&s)).unwrap_or_default();
        if parsed.log_enabled {
            if let Some(path)=win32::log_path() {
                if let Ok(file)=OpenOptions::new().create(true).write(true).truncate(true).open(path) {
                    let _=LOG.set(Mutex::new(file));
                }
            }
            if LOG.get().is_some() {
                log(&format!("CrimsonFix {} x86 - init; configuration={parsed:?}; DirectDraw proxy",env!("CARGO_PKG_VERSION")));
                // Only one read-only inventory, gated by the explicit opt-in.
                diagnostics::report(log);
            }
        }
        parsed
    })
}

fn real_module()->Option<*mut c_void> {
    let result=(*REAL_DLL.get_or_init(|| unsafe {win32::real_ddraw().map(|x|x as usize)}))
        .map(|x|x as *mut c_void);
    if result.map(|x|x as usize)==Some(MODULE.load(Ordering::Relaxed)) {
        log("ERROR: loading the system DirectDraw resolved back to CrimsonFix proxy; aborting to avoid recursion");
        return None;
    }
    result
}

unsafe fn symbol(name:&[u8])->*const c_void {
    let _=cfg();
    let Some(real)=real_module() else {log("ERROR: system32\\ddraw.dll cannot be loaded");return ptr::null()};
    win32::system_export(real,name)
}

fn originals()->&'static Mutex<HashMap<(usize,usize),usize>> {
    ORIGINALS.get_or_init(||Mutex::new(HashMap::new()))
}

unsafe fn method(object:*mut c_void,index:usize)->usize {
    if object.is_null(){return 0;}
    let table=*(object as *const *const usize);
    if table.is_null(){return 0;}
    *table.add(index)
}

unsafe fn orig(object:*mut c_void,index:usize)->Option<usize> {
    if object.is_null(){return None;}
    let table=*(object as *const *const usize);
    if table.is_null(){return None;}
    originals().lock().ok()?.get(&(table as usize,index)).copied()
}

unsafe fn replace_method(object:*mut c_void,index:usize,hook:usize)->bool {
    if object.is_null(){return false;}
    let table=*(object as *const *mut usize);
    if table.is_null(){return false;}
    let slot=table.add(index);
    let Ok(mut map)=originals().lock() else {return false};
    if map.contains_key(&(table as usize,index)){return true;}
    let original=*slot;
    if original==0 || original==hook{return false;}
    // Publish the original before changing the pointer to avoid a hook
    // seeing an uninitialized trampoline on another thread.
    map.insert((table as usize,index),original);
    if !win32::patch_vtable_slot(slot,hook) {
        map.remove(&(table as usize,index));
        return false;
    }
    true
}

fn pace_frame() {
    let fps=cfg().fps_limit;
    let frequency=*FREQUENCY.get_or_init(|| unsafe {
        let mut value=0;
        if win32::QueryPerformanceFrequency(&mut value)==0 || value<=0 {0}else{value}
    });
    if frequency<=0 {return;}
    let Ok(mut previous)=LAST_FRAME.lock() else {return};
    let mut now=0;
    if unsafe {win32::QueryPerformanceCounter(&mut now)}==0{return;}
    let interval=(frequency/fps as i64).max(1);
    if *previous>0 && now<*previous+interval {
        let deadline=*previous+interval;
        while now<deadline {
            let ticks=deadline-now;
            let millis=ticks.saturating_mul(1000)/frequency;
            if millis>2 {unsafe {win32::Sleep((millis-1).min(50) as u32)};}
            else {std::hint::spin_loop();}
            if unsafe {win32::QueryPerformanceCounter(&mut now)}==0{break;}
        }
    }
    *previous=now;
}

// IID matching is deliberately exhaustive rather than assuming a vtable
// layout from the major GUID prefix. Definitions come from Wine's ddraw.h.
fn ddraw_iface(iid:*const Guid)->Option<&'static str> {
    let name=guid_label(iid);
    match name.as_str() {
        "b3a6f3e0-2b43-11cf-a2de-00aa00b93356"=>Some("IDirectDraw2"),
        "9c59509a-39bd-11d1-8c4a-00c04fd930c5"=>Some("IDirectDraw4"),
        "15e65ec0-3b9c-11d2-b92f-00609797ea5b"=>Some("IDirectDraw7"),
        _=>None,
    }
}
fn surface_iface(iid:*const Guid)->Option<&'static str> {
    let name=guid_label(iid);
    match name.as_str() {
        "57805885-6eec-11cf-9441-a82303c10e27"=>Some("IDirectDrawSurface2"),
        "da044e00-69b2-11d0-a1d5-00aa00b8dfbb"=>Some("IDirectDrawSurface3"),
        "0b2b8630-ad35-11d0-8ea6-00609797ea5b"=>Some("IDirectDrawSurface4"),
        "06675a80-3b9b-11d2-b92f-00609797ea5b"=>Some("IDirectDrawSurface7"),
        _=>None,
    }
}

unsafe extern "system" fn modern_set_cooperative(this:*mut c_void,hwnd:Hwnd,flags:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_COOPERATIVE) else{return DDERR_GENERIC};
    let call:CooperativeFn=std::mem::transmute(addr);
    let hr=call(this,hwnd,flags); // OBSERVATION ONLY: keep original HWND and flags
    if MODERN_COOPERATIVE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: SetCooperativeLevel hwnd={} requested_flags={flags:#x} result={hr:#x} (unchanged)",
            if hwnd.is_null(){"NULL"}else{"provided"}));
        trace_game_windows("modern DirectDraw SetCooperativeLevel");
    }
    hr
}

unsafe extern "system" fn modern_set_display_mode(
    this:*mut c_void,w:u32,h:u32,bpp:u32,refresh:u32,flags:u32
)->Hr {
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:ModernDisplayModeFn=std::mem::transmute(addr);
    let hr=call(this,w,h,bpp,refresh,flags); // 5 arguments since IDirectDraw2
    if MODERN_DISPLAY_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: SetDisplayMode {w}x{h} bpp={bpp} refresh={refresh} flags={flags:#x} result={hr:#x} (unchanged)"));
    }
    hr
}

// Newer DirectDraw versions share the opaque CreateSurface calling
// convention, though the descriptor and surface types differ.
unsafe extern "system" fn modern_create_surface(
    this:*mut c_void,desc:*mut c_void,out:*mut *mut c_void,outer:*mut c_void
)->Hr {
    let Some(addr)=orig(this,DD_CREATE_SURFACE) else{return DDERR_GENERIC};
    let call:SurfaceFn=std::mem::transmute(addr);
    let hr=call(this,desc,out,outer);
    if MODERN_SURFACE_CREATE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: CreateSurface result={hr:#x} object_returned={} (unchanged)",
            hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        observe_modern_surface(*out,"CreateSurface");
    }
    hr
}

// A single hook can be used on all newer surface versions because these
// specific method ABIs (and vtable indices) are stable, per ddraw.h.
// Unlike legacy v1 pacing, this instrumentation NEVER sleeps or edits frames.
unsafe extern "system" fn modern_surface_flip(this:*mut c_void,target:*mut c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_FLIP) else{return DDERR_GENERIC};
    FIRST_MODERN_FLIP.call_once(||{
        log("RENDER-MODERN: first newer-surface Flip (observed; NOT FPS-paced)");
        trace_game_windows("first modern Flip");
    });
    let call:FlipFn=std::mem::transmute(addr);
    call(this,target,flags)
}
unsafe extern "system" fn modern_surface_blt(this:*mut c_void,dst:*const c_void,
    src:*mut c_void,src_rect:*const c_void,flags:u32,fx:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT) else{return DDERR_GENERIC};
    FIRST_MODERN_BLT.call_once(||{
        log("RENDER-MODERN: first newer-surface Blt (observed; NOT FPS-paced)");
        trace_game_windows("first modern Blt");
    });
    let call:BltFn=std::mem::transmute(addr);
    call(this,dst,src,src_rect,flags,fx)
}
unsafe extern "system" fn modern_surface_blt_fast(this:*mut c_void,x:u32,y:u32,
    src:*mut c_void,rect:*const c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT_FAST) else{return DDERR_GENERIC};
    FIRST_MODERN_FAST.call_once(||log("RENDER-MODERN: first newer-surface BltFast (observed; NOT FPS-paced)"));
    let call:BltFastFn=std::mem::transmute(addr);
    call(this,x,y,src,rect,flags)
}
unsafe extern "system" fn modern_surface_unlock(this:*mut c_void,data:*const c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_UNLOCK) else{return DDERR_GENERIC};
    FIRST_MODERN_UNLOCK.call_once(||{
        log("RENDER-MODERN: first newer-surface Unlock (observed; NOT FPS-paced)");
        trace_game_windows("first modern Unlock");
    });
    let call:UnlockFn=std::mem::transmute(addr);
    call(this,data)
}

unsafe fn observe_modern_surface(surface:*mut c_void,source:&str){
    if surface.is_null() || !cfg().log_enabled {return;}
    let qi=replace_method(surface,COM_QUERY_INTERFACE,surface_query_interface as usize);
    let flip=replace_method(surface,SURFACE_FLIP,modern_surface_flip as usize);
    let blt=replace_method(surface,SURFACE_BLT,modern_surface_blt as usize);
    let fast=replace_method(surface,SURFACE_BLT_FAST,modern_surface_blt_fast as usize);
    let unlock=replace_method(surface,SURFACE_UNLOCK,modern_surface_unlock as usize);
    if MODERN_SURFACE_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<24 {
        log(&format!("MODERN-SURFACE: from={source} query_interface={qi} flip={flip} blt={blt} blt_fast={fast} unlock={unlock} (observation only)"));
    }
}
unsafe fn observe_modern_ddraw(object:*mut c_void,name:&str){
    if object.is_null() || !cfg().log_enabled {return;}
    let qi=replace_method(object,COM_QUERY_INTERFACE,ddraw_query_interface as usize);
    let cooperative=replace_method(object,DD_SET_COOPERATIVE,modern_set_cooperative as usize);
    let mode=replace_method(object,DD_SET_DISPLAY_MODE,modern_set_display_mode as usize);
    let surface=replace_method(object,DD_CREATE_SURFACE,modern_create_surface as usize);
    if MODERN_DDRAW_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("MODERN-DDRAW: interface={name} query_interface={qi} cooperative={cooperative} mode={mode} surface={surface} (observation only)"));
    }
}

// Read-only interface discovery: the original COM QueryInterface is called
// without changing IID, return value or output. This identifies whether the
// 3D renderer uses other DirectDraw/Direct3D interfaces beyond the v1 hooks.
unsafe extern "system" fn ddraw_query_interface(this:*mut c_void,iid:*const Guid,out:*mut *mut c_void)->Hr {
    let Some(addr)=orig(this,COM_QUERY_INTERFACE) else{return DDERR_GENERIC};
    let call:QueryInterfaceFn=std::mem::transmute(addr);
    let hr=call(this,iid,out);
    if DDRAW_QI_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("COM-TRACE: IDirectDraw::QueryInterface iid={} result={hr:#x} object_returned={}",
            guid_label(iid),hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        if let Some(version)=ddraw_iface(iid){observe_modern_ddraw(*out,version);}
    }
    hr
}

unsafe extern "system" fn surface_query_interface(this:*mut c_void,iid:*const Guid,out:*mut *mut c_void)->Hr {
    let Some(addr)=orig(this,COM_QUERY_INTERFACE) else{return DDERR_GENERIC};
    let call:QueryInterfaceFn=std::mem::transmute(addr);
    let hr=call(this,iid,out);
    if SURFACE_QI_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("COM-TRACE: IDirectDrawSurface::QueryInterface iid={} result={hr:#x} object_returned={}",
            guid_label(iid),hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        if let Some(version)=surface_iface(iid){observe_modern_surface(*out,version);}
    }
    hr
}

// Trace only. Neither method is paced yet because a CPU lock/unlock
// or offscreen BltFast is not necessarily a displayed video frame.
unsafe extern "system" fn surface_blt_fast(this:*mut c_void,x:u32,y:u32,source:*mut c_void,src_rect:*const c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT_FAST) else{return DDERR_GENERIC};
    FIRST_BLT_FAST.call_once(||log("RENDER-TRACE: first IDirectDrawSurface::BltFast call (no FPS pacing on this path)"));
    let call:BltFastFn=std::mem::transmute(addr);
    call(this,x,y,source,src_rect,flags)
}

unsafe extern "system" fn surface_unlock(this:*mut c_void,rect_or_ptr:*const c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_UNLOCK) else{return DDERR_GENERIC};
    FIRST_UNLOCK.call_once(||log("RENDER-TRACE: first IDirectDrawSurface::Unlock call (no FPS pacing on this path)"));
    let call:UnlockFn=std::mem::transmute(addr);
    call(this,rect_or_ptr)
}

fn trace_game_windows(stage:&str) {
    if CONFIG.get().is_some_and(|cfg|cfg.log_enabled) &&
       WINDOW_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<6 {
        log(&format!("WIN-TRACE: at {stage}: {}",win32::game_windows_snapshot()));
    }
}

unsafe extern "system" fn set_cooperative(this:*mut c_void,hwnd:Hwnd,flags:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_COOPERATIVE) else {return DDERR_GENERIC};
    let call:CooperativeFn=std::mem::transmute(addr);
    let settings=cfg();
    let trace=COOPERATIVE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12;
    if trace {
        log(&format!("DirectDraw SetCooperativeLevel requested_flags={flags:#x} hwnd={} borderless={}",
            if hwnd.is_null(){"NULL"}else{"provided"},settings.borderless));
    }
    if settings.borderless {
        // Experimental: DDSCL_NORMAL avoids switching into an exclusive
        // hardware mode, but legacy flip chains may need further shims.
        let hr=call(this,hwnd,DDSCL_NORMAL);
        if trace {log(&format!("DirectDraw SetCooperativeLevel DDSCL_NORMAL result={hr:#x}"));}
        if hr>=0 {
            if hwnd.is_null() {
                if trace {log("Borderless deferred: game passed a NULL HWND; waiting for a valid HWND");}
                if trace {trace_game_windows("SetCooperativeLevel NULL HWND");}
            } else if let Some(monitor)=win32::monitor_from_cursor_or_window(hwnd) {
                match win32::borderless_window(hwnd,monitor) {
                    Ok(())=>log(&format!("Borderless applied: monitor [{},{},{},{}]",
                        monitor.left,monitor.top,monitor.right,monitor.bottom)),
                    Err(reason)=>if trace {log(&format!("Borderless not applied: {reason}"));},
                }
            } else if trace {log("Borderless deferred: could not resolve a monitor");}
            return hr;
        }
        if trace {log(&format!("WARNING: DDSCL_NORMAL rejected ({hr:#x}); falling back")); }
    }
    call(this,hwnd,flags)
}

unsafe extern "system" fn set_display_mode(this:*mut c_void,w:u32,h:u32,bpp:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else {return DDERR_GENERIC};
    let call:DisplayModeFn=std::mem::transmute(addr);
    let settings=cfg();
    if settings.borderless && settings.desktop_resolution {
        // In windowed mode, avoid an exclusive monitor mode switch.
        // Does not fix internal game viewport and surface sizes yet.
        log(&format!("Keeping desktop mode; game SetDisplayMode({w},{h},{bpp}) suppressed"));
        return S_OK;
    }
    call(this,w,h,bpp)
}

unsafe extern "system" fn surface_flip(this:*mut c_void,target:*mut c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_FLIP) else{return DDERR_GENERIC};
    FIRST_FLIP.call_once(||log("PRESENT: first IDirectDrawSurface::Flip call received; FPS limiter active on this path"));
    pace_frame();
    let call:FlipFn=std::mem::transmute(addr);
    call(this,target,flags)
}

unsafe extern "system" fn surface_blt(this:*mut c_void,dst:*const c_void,source:*mut c_void,src:*const c_void,flags:u32,fx:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT) else{return DDERR_GENERIC};
    FIRST_BLT.call_once(||log("PRESENT: first primary IDirectDrawSurface::Blt call received; FPS limiter active on this path"));
    pace_frame();
    let call:BltFn=std::mem::transmute(addr);
    call(this,dst,source,src,flags,fx)
}

unsafe extern "system" fn create_surface(this:*mut c_void,desc:*mut c_void,out:*mut *mut c_void,outer:*mut c_void)->Hr {
    let Some(addr)=orig(this,DD_CREATE_SURFACE) else {return DDERR_GENERIC};
    let call:SurfaceFn=std::mem::transmute(addr);
    let hr=call(this,desc,out,outer);
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        let surface=*out;
        if replace_method(surface,COM_QUERY_INTERFACE,surface_query_interface as usize) {
            log("COM-TRACE: IDirectDrawSurface::QueryInterface tracing installed");
        }
        if replace_method(surface,SURFACE_UNLOCK,surface_unlock as usize) {
            log("RENDER-TRACE: IDirectDrawSurface::Unlock tracing installed");
        }
        trace_game_windows("CreateSurface");
        if replace_method(surface,SURFACE_FLIP,surface_flip as usize) {
            log("IDirectDrawSurface::Flip paced (1..60 FPS)");
        }
        let getcaps_addr=method(surface,SURFACE_GET_CAPS);
        if getcaps_addr!=0 {
            let getcaps:GetCapsFn=std::mem::transmute(getcaps_addr);
            let mut caps=[0u32;4]; // accommodates DDSCAPS and DDSCAPS2
            if getcaps(surface,caps.as_mut_ptr())>=0 && caps[0]&DDSCAPS_PRIMARYSURFACE!=0 {
                if replace_method(surface,SURFACE_BLT,surface_blt as usize) {
                    log("Primary IDirectDrawSurface::Blt paced (1..60 FPS)");
                }
                if replace_method(surface,SURFACE_BLT_FAST,surface_blt_fast as usize) {
                    log("RENDER-TRACE: primary IDirectDrawSurface::BltFast tracing installed");
                }
            }
        }
    }
    hr
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawCreate(guid:*const c_void,out:*mut *mut c_void,outer:*mut c_void)->Hr {
    let function=symbol(b"DirectDrawCreate\0");
    if function.is_null(){return DDERR_GENERIC;}
    let real:CreateFn=std::mem::transmute(function);
    let hr=real(guid,out,outer);
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        let dd=*out;
        let qi=replace_method(dd,COM_QUERY_INTERFACE,ddraw_query_interface as usize);
        let a=replace_method(dd,DD_SET_COOPERATIVE,set_cooperative as usize);
        let b=replace_method(dd,DD_SET_DISPLAY_MODE,set_display_mode as usize);
        let c=replace_method(dd,DD_CREATE_SURFACE,create_surface as usize);
        log(&format!("DirectDrawCreate interception: query_interface={qi}, cooperative={a}, mode={b}, surface={c}"));
    }
    hr
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawEnumerateA(callback:*mut c_void,context:*mut c_void)->Hr {
    let function=symbol(b"DirectDrawEnumerateA\0");
    if function.is_null(){return DDERR_GENERIC;}
    let real:EnumerateFn=std::mem::transmute(function);
    real(callback,context)
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawCreateEx(guid:*const c_void,out:*mut *mut c_void,iid:*const c_void,outer:*mut c_void)->Hr {
    let function=symbol(b"DirectDrawCreateEx\0");
    if function.is_null(){return DDERR_GENERIC;}
    let real:CreateExFn=std::mem::transmute(function);
    // Diagnostic-only until newer DirectDraw COM interfaces are understood.
    let hr=real(guid,out,iid,outer);
    log(&format!("COM-TRACE: DirectDrawCreateEx iid={} result={hr:#x} object_returned={}",
        guid_label(iid.cast()),hr>=0 && !out.is_null() && !(*out).is_null()));
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        if let Some(version)=ddraw_iface(iid.cast()){observe_modern_ddraw(*out,version);}
    }
    hr
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(module:*mut c_void,reason:u32,_reserved:*mut c_void)->i32 {
    if reason==DLL_PROCESS_ATTACH {MODULE.store(module as usize,Ordering::Relaxed);}
    // No file I/O, DLL loading or hook installation under the loader lock.
    1
}

#[cfg(test)]
mod trace_tests {
    use super::*;
    #[test]
    fn discovers_actual_crimson_skies_directdraw_versions() {
        let dd2=Guid{d1:0xb3a6f3e0,d2:0x2b43,d3:0x11cf,
            d4:[0xa2,0xde,0x00,0xaa,0x00,0xb9,0x33,0x56]};
        let surface4=Guid{d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
            d4:[0x8e,0xa6,0x00,0x60,0x97,0x97,0xea,0x5b]};
        assert_eq!(ddraw_iface(&dd2),Some("IDirectDraw2"));
        assert_eq!(surface_iface(&surface4),Some("IDirectDrawSurface4"));
        assert_eq!(ddraw_iface(ptr::null()),None);
        assert_eq!(surface_iface(&dd2),None);
    }
    #[test]
    fn formats_interface_identifier_without_private_handles() {
        let g=Guid{d1:0x6c14db80,d2:0xa733,d3:0x11ce,
                   d4:[0xa5,0x21,0x00,0x20,0xaf,0x0b,0xe5,0x60]};
        assert_eq!(guid_label(&g),"6c14db80-a733-11ce-a521-0020af0be560");
        assert_eq!(guid_label(ptr::null()),"NULL");
    }
}
