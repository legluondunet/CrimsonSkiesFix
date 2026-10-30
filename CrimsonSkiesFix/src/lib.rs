//! Experimental x86 DirectDraw proxy for the locally unpacked Crimson Skies 1.0.2.0.
//! Place this built DLL under the name ddraw.dll beside CRIMSON.EXE.
#![cfg(windows)]
mod config;
mod win32;
mod diagnostics;
mod diagnostics_graphics;
mod presentation;
mod rgb565_present;
mod win32_present;
mod scene_correlation;
mod borderless_contract; // pure, inactive COM/windowed-surface contract (Stage B2)
mod com_unknown; // inactive native Win32 IUnknown ABI scaffold (Stage B3a)
mod com_surface4_abi; // inactive, verified 45-slot Surface4 ABI (Stage B3b)
mod surface_usage; // opt-in passive COM surface audit for borderless prerequisite
mod blt_signature; // opt-in exact Blt flags + source-presence audit

use config::Config;
use std::{collections::HashMap, ffi::c_void, fs::{self,File,OpenOptions}, io::Write,
          ptr, sync::{atomic::{AtomicUsize,Ordering}, Mutex,OnceLock,Once}, time::Instant};
use win32::Hwnd;

// All user-facing diagnostic messages use the canonical project name.
// Historical INI names are recognized only by the import logic.
const PROJECT_NAME:&str="CrimsonSkiesFix";
const SYSTEM_DDRAW_RECURSION_ERROR:&str="ERROR: loading the system DirectDraw resolved back to CrimsonSkiesFix proxy; aborting to avoid recursion";

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
type GetAttachedSurfaceFn=unsafe extern "system" fn(*mut c_void,*const u32,*mut *mut c_void)->Hr;
type GetSurfaceDescFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type D3D3CreateDeviceFn=unsafe extern "system" fn(*mut c_void,*const Guid,*mut c_void,*mut *mut c_void,*mut c_void)->Hr;
type D3D3SceneFn=unsafe extern "system" fn(*mut c_void)->Hr;
type D3D3SetRenderTargetFn=unsafe extern "system" fn(*mut c_void,*mut c_void,u32)->Hr;

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
const SURFACE_USAGE_ADD_ATTACHED_SURFACE:usize=3;
const SURFACE_USAGE_DELETE_ATTACHED_SURFACE:usize=8;
const SURFACE_USAGE_GET_DC:usize=17;
const SURFACE_USAGE_IS_LOST:usize=24;
const SURFACE_USAGE_LOCK:usize=25;
const SURFACE_USAGE_RELEASE_DC:usize=26;
const SURFACE_USAGE_RESTORE:usize=27;
const SURFACE_USAGE_SET_CLIPPER:usize=28;
const SURFACE_USAGE_GET_DDINTERFACE:usize=36;
const SURFACE_BLT_FAST:usize=7;
const SURFACE_UNLOCK:usize=32;
const SURFACE_FLIP:usize=11;
const SURFACE_GET_ATTACHED:usize=12;
const SURFACE_GET_CAPS:usize=14;
const SURFACE_GET_DESC:usize=22;
const D3D3_CREATE_DEVICE:usize=8; // IUnknown(3) + 5 IDirect3D3 methods
// Verified against IDirect3DDevice3's original DX6 COM ABI.
const D3D3_DEVICE_BEGIN_SCENE:usize=9;
const D3D3_DEVICE_END_SCENE:usize=10;
const D3D3_DEVICE_SET_RENDER_TARGET:usize=14;
const DDSCAPS_BACKBUFFER:u32=0x0000_0004;
const DDSCAPS_FLIP:u32=0x0000_0010;
const DDSCAPS_COMPLEX:u32=0x0000_0008;

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
static FIRST_MODERN_VALID_HWND:Once=Once::new();
static FIRST_MODERN_PACED_FLIP:Once=Once::new();
static FIRST_MODERN_NONPRIMARY_FLIP:Once=Once::new();
static MODERN_FLIP_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MODERN_BLT_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MODERN_UNLOCK_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MODERN_BLTFAST_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MODERN_FLIP_START:OnceLock<Instant>=OnceLock::new();
static MODERN_BLT_START:OnceLock<Instant>=OnceLock::new();
static MODERN_UNLOCK_START:OnceLock<Instant>=OnceLock::new();
static MODERN_BLTFAST_START:OnceLock<Instant>=OnceLock::new();
static MODERN_FLIP_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static MODERN_BLT_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static MODERN_UNLOCK_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static MODERN_BLTFAST_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static LAST_GAME_HWND:AtomicUsize=AtomicUsize::new(0);
static SURFACE_CREATE_DETAIL_COUNT:AtomicUsize=AtomicUsize::new(0);
static SURFACE_ATTACH_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static SURFACE_DESC_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static SURFACE_FIRST_PRIMARY_ATTACH:Once=Once::new();
static PRESENTATION_SESSION:Mutex<presentation::Session>=Mutex::new(presentation::Session::new());
static PRE_EXCLUSIVE_DESKTOP:OnceLock<(u32,u32)>=OnceLock::new();
static SURFACE_FORMAT_PROBE_COUNT:AtomicUsize=AtomicUsize::new(0);
static D3D3_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static D3D3_CREATE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static D3D3_DEVICE_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static D3D3_RENDER_TARGET_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static D3D3_BEGIN_EVENTS:AtomicUsize=AtomicUsize::new(0);
static D3D3_END_EVENTS:AtomicUsize=AtomicUsize::new(0);
static D3D3_BEGIN_START:OnceLock<Instant>=OnceLock::new();
static D3D3_END_START:OnceLock<Instant>=OnceLock::new();
static D3D3_BEGIN_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static D3D3_END_WINDOW:OnceLock<Mutex<(usize,Instant)>>=OnceLock::new();
static FIRST_D3D3_SCENE:Once=Once::new();
// Diagnostic-only snapshot groups exactly 120 successful primary Flips.
// Mode changes reset the grouping; no frame timing or simulation changes.
static SCENE_FLIP_GROUP:Mutex<scene_correlation::SceneCorrelation>=
    Mutex::new(scene_correlation::SceneCorrelation::new());
static SCENE_FLIP_GROUP_LOGS:AtomicUsize=AtomicUsize::new(0);
static SURFACE_USAGE_AUDIT:Mutex<surface_usage::Audit>=Mutex::new(surface_usage::Audit::new());
static SURFACE_USAGE_LOGS:AtomicUsize=AtomicUsize::new(0);
static BLT_SIGNATURE_AUDIT:Mutex<blt_signature::Audit>=Mutex::new(blt_signature::Audit::new());
static BLT_SIGNATURE_LOGS:AtomicUsize=AtomicUsize::new(0);

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
                log(&format!("{PROJECT_NAME} {} x86 - init; configuration={parsed:?}; DirectDraw proxy",env!("CARGO_PKG_VERSION")));
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
        log(SYSTEM_DDRAW_RECURSION_ERROR);
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

// These cumulative rates are COM call rates, not proven on-screen FPS.
// A scene can issue several Blt/Unlock calls per one displayed frame.
// At most four samples per API, bounded without background threads.
fn sample_render_calls(path:&str,count:&AtomicUsize,start:&OnceLock<Instant>,
    window:&OnceLock<Mutex<(usize,Instant)>>) {
    if !cfg().log_enabled{return;}
    let n=count.fetch_add(1,Ordering::Relaxed)+1;
    let first=start.get_or_init(Instant::now);
    if [10,60,300,1200].contains(&n) {
        let elapsed=first.elapsed().as_millis();
        if elapsed>0 {
            let rate=(n.saturating_sub(1) as u128 * 1000) / elapsed;
            log(&format!("RENDER-SAMPLE: method={path} calls={n} elapsed_ms={elapsed} average_calls_per_s={rate} (NOT measured display FPS)"));
        }
    }
    // Also sample short intervals: full-run averages in the early Wine
    // log hid substantial differences between menus and actual gameplay.
    // Bounded to ten per API, not one log line per video frame.
    if n % 120==0 && n<=1200 {
        let guard=window.get_or_init(||Mutex::new((0,*first)));
        if let Ok(mut previous)=guard.lock() {
            let now=Instant::now();
            let ms=now.duration_since(previous.1).as_millis();
            let interval=n-previous.0;
            if ms>0 {
                let rate=interval as u128*1000/ms;
                log(&format!("RENDER-WINDOW: method={path} calls={interval} elapsed_ms={ms} calls_per_s={rate} (API rate, NOT verified display FPS)"));
            }
            *previous=(n,now);
        }
    }
}

unsafe fn modern_surface_caps(this:*mut c_void)->Option<u32> {
    // The first DWORD is identical in DDSCAPS and DDSCAPS2.
    let method=method(this,SURFACE_GET_CAPS);
    if method==0{return None;}
    let getcaps:GetCapsFn=std::mem::transmute(method);
    let mut caps=[0u32;4];
    (getcaps(this,caps.as_mut_ptr())>=0).then_some(caps[0])
}
unsafe fn modern_surface_primary(this:*mut c_void)->Option<bool> {
    modern_surface_caps(this).map(|caps|caps&DDSCAPS_PRIMARYSURFACE!=0)
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
    if !hwnd.is_null() && cfg().log_enabled && PRE_EXCLUSIVE_DESKTOP.get().is_none(){
        // Critical: capture monitor dimensions BEFORE exclusive DirectDraw
        // SetDisplayMode shrinks the Wine mode and game HWND to 640/800.
        if let Some(area)=win32::monitor_for_game_window(hwnd){
            let w=area.right.saturating_sub(area.left) as u32;
            let h=area.bottom.saturating_sub(area.top) as u32;
            if w>0 && h>0 {
                let _=PRE_EXCLUSIVE_DESKTOP.set((w,h));
                log(&format!("BORDERLESS-PREFLIGHT: captured pre-exclusive monitor={w}x{h} (read-only)"));
            }
        }
    }
    let hr=call(this,hwnd,flags); // OBSERVATION ONLY: keep original HWND and flags
    if !hwnd.is_null() && cfg().log_enabled {
        // Never print the numeric HWND. A stale HWND is checked before use.
        LAST_GAME_HWND.store(hwnd as usize,Ordering::Relaxed);
        FIRST_MODERN_VALID_HWND.call_once(||log(&format!(
            "WIN-TRACE: first game-supplied non-NULL HWND: {}",
            win32::game_window_snapshot(hwnd))));
    }
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
    if hr>=0 {
        if let Ok(mut session)=PRESENTATION_SESSION.lock(){
            session.begin_mode(presentation::Mode{width:w,height:h,bpp});
            if cfg().surface_usage_trace && cfg().log_enabled {
                if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
                    audit.reset(session.epoch);
                }
                if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
                    audit.reset(session.epoch);
                }
            }
            if cfg().log_enabled {
                if let Ok(mut group)=SCENE_FLIP_GROUP.lock(){
                    group.reset(session.epoch,
                        D3D3_BEGIN_EVENTS.load(Ordering::Relaxed),
                        D3D3_END_EVENTS.load(Ordering::Relaxed));
                }
            }
            if cfg().log_enabled{
                log(&format!("CHAIN-EPOCH: generation={} new_mode={w}x{h}x{bpp}; previous chain invalidated",
                    session.epoch));
            }
        }
    }
    if MODERN_DISPLAY_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: SetDisplayMode {w}x{h} bpp={bpp} refresh={refresh} flags={flags:#x} result={hr:#x} (unchanged)"));
        let hwnd=LAST_GAME_HWND.load(Ordering::Relaxed) as Hwnd;
        if !hwnd.is_null() && cfg().log_enabled{
            log(&format!("WIN-TRACE: after SetDisplayMode {w}x{h}: {}",
                win32::game_window_snapshot(hwnd)));
        }
    }
    hr
}

// A strictly read-only GetSurfaceDesc probe of the *existing* DirectDraw
// object. No Lock/Unlock, new surfaces, pixel copying, or COM ownership changes.
// Only modern 124-byte DDSURFACEDESC2 objects can be safely probed.
unsafe fn probe_modern_surface_layout(surface:*mut c_void,reason:&str) {
    if surface.is_null() || !cfg().log_enabled{return;}
    let Some(caps)=modern_surface_caps(surface) else{return;};
    if caps&(presentation::PRIMARY|presentation::BACKBUFFER|presentation::ZBUFFER)==0{
        return;
    }
    // Unrelated textures must not exhaust the bounded display-surface probe.
    if SURFACE_FORMAT_PROBE_COUNT.fetch_add(1,Ordering::Relaxed)>=20{return;}
    // Query original GetSurfaceDesc to avoid going through our own logging
    // hook. A method already patched on this vtable has an original mapping.
    let address=orig(surface,SURFACE_GET_DESC).or_else(||{
        let addr=method(surface,SURFACE_GET_DESC);
        (addr!=0 && addr!=modern_get_surface_desc as usize).then_some(addr)
    });
    let Some(address)=address else{return;};
    let call:GetSurfaceDescFn=std::mem::transmute(address);
    let mut desc=[0u32;31]; // 124-byte DDSURFACEDESC2, 32-bit Win32 ABI
    desc[0]=124;
    let hr=call(surface,desc.as_mut_ptr().cast());
    if hr<0{
        log(&format!("SURFACE-FORMAT: source={reason} role={} GetSurfaceDesc failed hr={hr:#x} (unchanged)",
            surface_roles(caps)));
        return;
    }
    let Some(info)=presentation::parse_surface_desc2(&desc) else{
        log(&format!("SURFACE-FORMAT: source={reason} unexpected descriptor size={} (unchanged)",desc[0]));
        return;
    };
    // A 16-bit game may use RGB565, RGB555 or an unusual format. Never
    // assume it is RGB565 without confirming the returned masks.
    let masks=info.color_masks.map(|m|format!("R={:#06x},G={:#06x},B={:#06x},A={:#06x}",
        m[0],m[1],m[2],m[3])).unwrap_or_else(||"not-RGB-or-unreported".into());
    let pixel=info.pixel_flags.map(|x|format!("{x:#x}")).unwrap_or_else(||"Unavailable".into());
    let bpp=info.bits_per_pixel.map(|x|x.to_string()).unwrap_or_else(||"Unavailable".into());
    let buffers=info.backbuffer_count.map(|x|x.to_string()).unwrap_or_else(||"Unavailable".into());
    log(&format!("SURFACE-FORMAT: source={reason} role={} dimensions={}x{} backbuffers={} pf_flags={} bits={} masks={} (read-only)",
        surface_roles(caps),info.width,info.height,buffers,pixel,bpp,masks));
}

// Named status based entirely on the public DirectDraw DDSCAPS flags.
// We deliberately do not log raw COM pointer addresses or identifiers.
fn surface_roles(caps:u32)->String {
    let mut roles=Vec::new();
    if caps&DDSCAPS_PRIMARYSURFACE!=0{roles.push("primary");}
    if caps&DDSCAPS_BACKBUFFER!=0{roles.push("backbuffer");}
    if caps&DDSCAPS_FLIP!=0{roles.push("flip-chain");}
    if caps&DDSCAPS_COMPLEX!=0{roles.push("complex");}
    if caps&presentation::ZBUFFER!=0{roles.push("z-buffer");}
    if caps&presentation::TEXTURE!=0{roles.push("texture");}
    if caps&presentation::MIPMAP!=0{roles.push("mipmap");}
    if roles.is_empty(){"other".into()}else{roles.join("+")}
}

fn describe_surface_layout(desc:*const c_void)->String{
    if desc.is_null(){return "NULL descriptor".into();}
    unsafe{
        // The first 16 bytes are common to DDSURFACEDESC and DDSURFACEDESC2:
        // dwSize, dwFlags, dwHeight and dwWidth. No pixels are inspected.
        let words=desc as *const u32;
        let size=*words;
        if size!=108 && size!=124{return format!("unsupported_descriptor_size={size}");}
        let flags=*words.add(1);
        let h=*words.add(2);
        let w=*words.add(3);
        let dims=if flags&0x6==0x6{
            format!("{w}x{h}")
        }else{"unspecified".into()};
        format!("desc_size={size} flags={flags:#x} dimensions={dims}")
    }
}

// Final preflight only: this never changes the DirectDraw cooperative
// mode, surfaces, HWND or desktop. Until an actual adapter implements
// virtualized front/back/Z surfaces, log the safe 4:3 viewport as a plan.
// The per-mode state is independent of display-resolution labels: a new
// successful SetDisplayMode invalidates *all* earlier front/back/Z evidence,
// even if it switches to the same game resolution.
fn log_render_chain_preflight(request:u32,parent:u32,child:u32) {
    let Ok(mut session)=PRESENTATION_SESSION.lock() else{return;};
    let (changed,ready)=session.observe(request,parent,child);
    let mode=session.mode;
    let epoch=session.epoch;
    let snapshot=session.chain;
    drop(session);
    if changed{
        log(&format!("CHAIN-CLASSIFY: generation={epoch} front_to_back={} back_to_depth={} texture_mipmaps={} (non-invasive)",
            snapshot.front_to_back,snapshot.back_to_depth,snapshot.mipmaps));
    }
    if !ready{return;}
    let Some(mode)=mode else{return;};
    let Some(&(desktop_width,desktop_height))=PRE_EXCLUSIVE_DESKTOP.get() else{
        log("BORDERLESS-PREFLIGHT: original desktop bounds unavailable; keeping exclusive renderer");
        return;
    };
    if let Some(plan)=presentation::preflight(mode,desktop_width,desktop_height,snapshot){
        log(&format!(
            "BORDERLESS-PREFLIGHT: generation={epoch} complete 3D front->back->Z chain; internal={}x{}x{}; monitor={}x{}; aspect_viewport=x{},y{},{}x{}; requires_virtual_primary=true; applied=false",
            mode.width,mode.height,mode.bpp,plan.desktop_width,plan.desktop_height,
            plan.viewport.x,plan.viewport.y,plan.viewport.width,plan.viewport.height));
    }
}

// The GetAttachedSurface ABI is the same for Surface2/3/4/7.
// Call through with the original DDSCAPS or DDSCAPS2 pointer unchanged.
// Do not Release the returned COM interface: ownership belongs to the game.
unsafe extern "system" fn modern_get_attached_surface(
    this:*mut c_void,caps:*const u32,out:*mut *mut c_void
)->Hr {
    let Some(addr)=orig(this,SURFACE_GET_ATTACHED) else{return DDERR_GENERIC};
    let call:GetAttachedSurfaceFn=std::mem::transmute(addr);
    let hr=call(this,caps,out);
    record_surface_usage(this,surface_usage::Method::GetAttachedSurface,hr);
    if cfg().log_enabled {
        let n=SURFACE_ATTACH_TRACE_COUNT.fetch_add(1,Ordering::Relaxed);
        if n<24 {
            let requested=if caps.is_null(){None}else{Some(*caps)};
            let parent=modern_surface_caps(this);
            let child=if hr>=0 && !out.is_null() && !(*out).is_null(){
                modern_surface_caps(*out)
            }else{None};
            log(&format!(
                "SURFACE-ATTACH: requested={} parent={} ({}) attached={} ({}) hr={hr:#x} returned={} (unchanged)",
                requested.map(|x|format!("{x:#010x}")).unwrap_or_else(||"NULL".into()),
                parent.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
                parent.map(surface_roles).unwrap_or_else(||"unknown".into()),
                child.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
                child.map(surface_roles).unwrap_or_else(||"unknown".into()),
                hr>=0 && !out.is_null() && !(*out).is_null()));
        }
        if hr>=0 && !out.is_null() && !(*out).is_null(){
            if let (Some(a),Some(b))=(modern_surface_caps(this),modern_surface_caps(*out)){
                if a&DDSCAPS_PRIMARYSURFACE!=0 && b&DDSCAPS_BACKBUFFER!=0 {
                    SURFACE_FIRST_PRIMARY_ATTACH.call_once(||log(
                        "SURFACE-CHAIN: confirmed primary front buffer -> attached backbuffer (game-provided; no modification)"));
                }
                // The requested caps pointer is game-owned, not modified.
                if !caps.is_null(){log_render_chain_preflight(*caps,a,b);}
                if !caps.is_null() && (*caps&(presentation::BACKBUFFER|presentation::ZBUFFER))!=0 {
                    probe_modern_surface_layout(*out,"GetAttachedSurface");
                }
            }
        }
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        observe_modern_surface(*out,"GetAttachedSurface");
    }
    hr
}

// GetSurfaceDesc is read-only; this wrapper captures only the first
// 16-byte descriptor prefix and delegates exactly once to the system DLL.
unsafe extern "system" fn modern_get_surface_desc(this:*mut c_void,desc:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_GET_DESC) else{return DDERR_GENERIC};
    let call:GetSurfaceDescFn=std::mem::transmute(addr);
    let hr=call(this,desc);
    if cfg().log_enabled && SURFACE_DESC_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<20 {
        let layout=if hr>=0{describe_surface_layout(desc)}else{"call failed".into()};
        let caps=modern_surface_caps(this);
        log(&format!("SURFACE-DESC: {layout} caps={} role={} hr={hr:#x} (unchanged)",
            caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into())));
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
        if !desc.is_null() && *(desc as *const u32)==124 {
            probe_modern_surface_layout(*out,"CreateSurface");
        }
        if cfg().log_enabled && SURFACE_CREATE_DETAIL_COUNT.fetch_add(1,Ordering::Relaxed)<24 {
            // Read-only DWORD prefix of a valid DDSURFACEDESC / DDSURFACEDESC2.
            // Both structures begin: dwSize, dwFlags, dwHeight, dwWidth.
            if !desc.is_null(){
                let words=desc as *const u32;
                let size=*words;
                if size==108 || size==124 {
                    let flags=*words.add(1);
                    let height=*words.add(2);
                    let width=*words.add(3);
                    let caps=modern_surface_caps(*out);
                    log(&format!("SURFACE-CREATE: desc_size={size} flags={flags:#x} width={} height={} caps={} (observed; no changes)",
                        if flags&4!=0{width.to_string()}else{"unspecified".into()},
                        if flags&2!=0{height.to_string()}else{"unspecified".into()},
                        caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
                }else{
                    log(&format!("SURFACE-CREATE: unexpected descriptor size={size} (skipping inspection)"));
                }
            }
        }
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
        log("RENDER-MODERN: first newer-surface Flip (real modern rendering path)");
        log(&format!("SURFACE-TRACE: first modern Flip surface_caps={}",
            modern_surface_caps(this).map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
        trace_game_windows("first modern Flip");
    });
    sample_render_calls("Flip",&MODERN_FLIP_EVENTS,&MODERN_FLIP_START,&MODERN_FLIP_WINDOW);
    if cfg().modern_fps_pacing {
        match modern_surface_primary(this) {
            Some(true)=>{
                FIRST_MODERN_PACED_FLIP.call_once(||log(&format!(
                    "FPS-PACING: primary modern Flip enabled at {} FPS (experimental)",cfg().fps_limit)));
                pace_frame();
            },
            Some(false)=>{
                FIRST_MODERN_NONPRIMARY_FLIP.call_once(||log(
                    "FPS-PACING: non-primary modern Flip bypassed; no throttle on non-primary surface"));
            },
            None=>{
                FIRST_MODERN_NONPRIMARY_FLIP.call_once(||log(
                    "FPS-PACING: modern surface GetCaps unavailable; no throttle applied"));
            }
        }
    }
    let call:FlipFn=std::mem::transmute(addr);
    let result=call(this,target,flags);
    // This code runs AFTER the original Flip and only counts successful
    // calls on the real primary surface. It never modifies the HRESULT.
    if result>=0 && cfg().log_enabled && modern_surface_primary(this)==Some(true) {
        if surface_usage_enabled(){
            if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
                if let Some(sample)=audit.observe_primary_flip(){
                    if SURFACE_USAGE_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                        log(&sample.summary());
                    }
                }
            }
            if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
                if let Some(report)=audit.primary_flip(){
                    if BLT_SIGNATURE_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                        log(&report);
                    }
                }
            }
        }
        if let Ok(mut group)=SCENE_FLIP_GROUP.lock(){
            let sample=group.observe_flip(
                D3D3_BEGIN_EVENTS.load(Ordering::Relaxed),
                D3D3_END_EVENTS.load(Ordering::Relaxed));
            if let Some(sample)=sample {
                // Limit to 12 correlated windows over the whole session.
                if SCENE_FLIP_GROUP_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                    let successful_flip_rate=sample
                        .successful_primary_flip_calls_per_s()
                        .map(|rate|format!("{rate:.2}"))
                        .unwrap_or_else(||"Unavailable".to_string());
                    log(&format!(
                        "SCENE-CORRELATION: generation={} successful_primary_flips={} successful_begin_scene={} successful_end_scene={} begins_per_flip={:.2} ends_per_flip={:.2} first_to_last_flip_ms={} flip_intervals={} successful_primary_flip_calls_per_s={} (same successful-Flip group; rates are COM calls, NOT display FPS)",
                        sample.generation,sample.flips,sample.successful_begins,
                        sample.successful_ends,
                        sample.successful_begins as f64/sample.flips as f64,
                        sample.successful_ends as f64/sample.flips as f64,
                        sample.first_to_last_flip_ms,sample.observed_flip_intervals,
                        successful_flip_rate));
                    let stats=sample.interflip;
                    log(&format!(
                        "SCENE-INTERFLIP: generation={} intervals={} begin_min={} begin_max={} end_min={} end_max={} begin_hist_0_to_12_and_13plus={:?} begin_end_mismatches={} counter_regressions={} (between successful primary Flips; NOT displayed FPS)",
                        sample.generation,stats.intervals,
                        if stats.intervals==0{0}else{stats.min_begins},
                        stats.max_begins,
                        if stats.intervals==0{0}else{stats.min_ends},
                        stats.max_ends,stats.histogram,
                        stats.mismatch_intervals,stats.counter_regressions));
                }
            }
        }
    }
    result
}
unsafe extern "system" fn modern_surface_blt(this:*mut c_void,dst:*const c_void,
    src:*mut c_void,src_rect:*const c_void,flags:u32,fx:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT) else{return DDERR_GENERIC};
    FIRST_MODERN_BLT.call_once(||{
        log("RENDER-MODERN: first newer-surface Blt (observed; NOT FPS-paced)");
        log(&format!("SURFACE-TRACE: first modern Blt destination_caps={}",
            modern_surface_caps(this).map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
        trace_game_windows("first modern Blt");
    });
    sample_render_calls("Blt",&MODERN_BLT_EVENTS,&MODERN_BLT_START,&MODERN_BLT_WINDOW);
    let call:BltFn=std::mem::transmute(addr);
    let hr=call(this,dst,src,src_rect,flags,fx);
    record_surface_usage(this,surface_usage::Method::Blt,hr);
    if surface_usage_enabled(){
        let role=surface_usage::classify(modern_surface_caps(this));
        if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
            audit.record(role,flags,!src.is_null(),hr);
        }
    }
    hr
}
unsafe extern "system" fn modern_surface_blt_fast(this:*mut c_void,x:u32,y:u32,
    src:*mut c_void,rect:*const c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT_FAST) else{return DDERR_GENERIC};
    FIRST_MODERN_FAST.call_once(||log("RENDER-MODERN: first newer-surface BltFast (observed; NOT FPS-paced)"));
    sample_render_calls("BltFast",&MODERN_BLTFAST_EVENTS,&MODERN_BLTFAST_START,&MODERN_BLTFAST_WINDOW);
    let call:BltFastFn=std::mem::transmute(addr);
    let hr=call(this,x,y,src,rect,flags);
    record_surface_usage(this,surface_usage::Method::BltFast,hr);
    hr
}
unsafe extern "system" fn modern_surface_unlock(this:*mut c_void,data:*const c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_UNLOCK) else{return DDERR_GENERIC};
    FIRST_MODERN_UNLOCK.call_once(||{
        log("RENDER-MODERN: first newer-surface Unlock (observed; NOT FPS-paced)");
        trace_game_windows("first modern Unlock");
    });
    sample_render_calls("Unlock",&MODERN_UNLOCK_EVENTS,&MODERN_UNLOCK_START,&MODERN_UNLOCK_WINDOW);
    let call:UnlockFn=std::mem::transmute(addr);
    let hr=call(this,data);
    record_surface_usage(this,surface_usage::Method::Unlock,hr);
    hr
}

// Separate from normal scene counters: in this *opt-in* path we
// delegate each DirectDrawSurface2/3/4/7 method exactly once, then
// observe the HRESULT. We never read or change caller buffer contents,
// retain COM pointers, or affect the game-owned object lifecycle.
fn surface_usage_enabled()->bool {
    let settings=cfg();
    settings.log_enabled && settings.surface_usage_trace
}
unsafe fn record_surface_usage(this:*mut c_void,method:surface_usage::Method,hr:Hr){
    if !surface_usage_enabled(){return;}
    let role=surface_usage::classify(modern_surface_caps(this));
    if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
        audit.record(role,method,hr);
    }
}
unsafe extern "system" fn audit_surface_add_attached_surface(this:*mut c_void,attachment:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_ADD_ATTACHED_SURFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,attachment);
    record_surface_usage(this,surface_usage::Method::AddAttachedSurface,hr);
    hr
}

unsafe extern "system" fn audit_surface_delete_attached_surface(this:*mut c_void,flags:u32,attachment:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_DELETE_ATTACHED_SURFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,u32,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,flags,attachment);
    record_surface_usage(this,surface_usage::Method::DeleteAttachedSurface,hr);
    hr
}

unsafe extern "system" fn audit_surface_get_dc(this:*mut c_void,dc:*mut *mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_GET_DC) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,dc);
    record_surface_usage(this,surface_usage::Method::GetDC,hr);
    hr
}

unsafe extern "system" fn audit_surface_is_lost(this:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_IS_LOST) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this);
    record_surface_usage(this,surface_usage::Method::IsLost,hr);
    hr
}

unsafe extern "system" fn audit_surface_lock(this:*mut c_void,rect:*const c_void,desc:*mut c_void,flags:u32,event:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_LOCK) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,u32,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,rect,desc,flags,event);
    record_surface_usage(this,surface_usage::Method::Lock,hr);
    hr
}

unsafe extern "system" fn audit_surface_release_dc(this:*mut c_void,dc:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_RELEASE_DC) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,dc);
    record_surface_usage(this,surface_usage::Method::ReleaseDC,hr);
    hr
}

unsafe extern "system" fn audit_surface_restore(this:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_RESTORE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this);
    record_surface_usage(this,surface_usage::Method::Restore,hr);
    hr
}

unsafe extern "system" fn audit_surface_set_clipper(this:*mut c_void,clipper:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_SET_CLIPPER) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,clipper);
    record_surface_usage(this,surface_usage::Method::SetClipper,hr);
    hr
}

unsafe extern "system" fn audit_surface_get_ddinterface(this:*mut c_void,out:*mut *mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_GET_DDINTERFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,out);
    record_surface_usage(this,surface_usage::Method::GetDDInterface,hr);
    hr
}
unsafe fn observe_modern_surface(surface:*mut c_void,source:&str){
    if surface.is_null() || !(cfg().log_enabled || cfg().modern_fps_pacing) {return;}
    let qi=replace_method(surface,COM_QUERY_INTERFACE,surface_query_interface as usize);
    let flip=replace_method(surface,SURFACE_FLIP,modern_surface_flip as usize);
    let blt=replace_method(surface,SURFACE_BLT,modern_surface_blt as usize);
    let fast=replace_method(surface,SURFACE_BLT_FAST,modern_surface_blt_fast as usize);
    let unlock=replace_method(surface,SURFACE_UNLOCK,modern_surface_unlock as usize);
    let attached=replace_method(surface,SURFACE_GET_ATTACHED,modern_get_attached_surface as usize);
    let desc=replace_method(surface,SURFACE_GET_DESC,modern_get_surface_desc as usize);
    if surface_usage_enabled(){
        let installed=[
            replace_method(surface,SURFACE_USAGE_ADD_ATTACHED_SURFACE,audit_surface_add_attached_surface as usize),
            replace_method(surface,SURFACE_USAGE_DELETE_ATTACHED_SURFACE,audit_surface_delete_attached_surface as usize),
            replace_method(surface,SURFACE_USAGE_GET_DC,audit_surface_get_dc as usize),
            replace_method(surface,SURFACE_USAGE_IS_LOST,audit_surface_is_lost as usize),
            replace_method(surface,SURFACE_USAGE_LOCK,audit_surface_lock as usize),
            replace_method(surface,SURFACE_USAGE_RELEASE_DC,audit_surface_release_dc as usize),
            replace_method(surface,SURFACE_USAGE_RESTORE,audit_surface_restore as usize),
            replace_method(surface,SURFACE_USAGE_SET_CLIPPER,audit_surface_set_clipper as usize),
            replace_method(surface,SURFACE_USAGE_GET_DDINTERFACE,audit_surface_get_ddinterface as usize),
        ];
        if MODERN_SURFACE_HOOK_TRACE_COUNT.load(Ordering::Relaxed)<24 {
            log(&format!("SURFACE-USAGE-HOOKS: installed={}/{} methods (opt-in; passive)",
                installed.iter().filter(|&&yes|yes).count(),installed.len()));
        }
    }
    if MODERN_SURFACE_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<24 {
        log(&format!("MODERN-SURFACE: from={source} query_interface={qi} flip={flip} blt={blt} blt_fast={fast} unlock={unlock} get_attached={attached} get_desc={desc} (observation only)"));
    }
}
unsafe fn observe_modern_ddraw(object:*mut c_void,name:&str){
    if object.is_null() || !(cfg().log_enabled || cfg().modern_fps_pacing) {return;}
    let qi=replace_method(object,COM_QUERY_INTERFACE,ddraw_query_interface as usize);
    let cooperative=replace_method(object,DD_SET_COOPERATIVE,modern_set_cooperative as usize);
    let mode=replace_method(object,DD_SET_DISPLAY_MODE,modern_set_display_mode as usize);
    let surface=replace_method(object,DD_CREATE_SURFACE,modern_create_surface as usize);
    if MODERN_DDRAW_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("MODERN-DDRAW: interface={name} query_interface={qi} cooperative={cooperative} mode={mode} surface={surface} (observation only)"));
    }
}

// Crimson Skies' IDirectDraw4::QueryInterface successfully requests
// IID_IDirect3D3. This is the legacy Direct3D3 interface, NOT Direct3D9.
fn is_d3d3_iid(iid:*const Guid)->bool {
    guid_label(iid)=="bb223240-e72b-11d0-a9b4-00aa00c0993e"
}

// The attached device's legacy Direct3D3 interface is traced only after
// the original CreateDevice succeeds. The genuine device and surfaces
// are still owned by the game: no replacement objects or AddRef/Release.
unsafe extern "system" fn d3d3_device_begin_scene(this:*mut c_void)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_BEGIN_SCENE) else{return DDERR_GENERIC};
    let original:D3D3SceneFn=std::mem::transmute(addr);
    let hr=original(this);
    if cfg().log_enabled && hr>=0 {
        FIRST_D3D3_SCENE.call_once(||log(
            "D3D3-DEVICE: first successful BeginScene; game 3D rendering active"));
        sample_render_calls("D3D3.BeginScene",&D3D3_BEGIN_EVENTS,&D3D3_BEGIN_START,&D3D3_BEGIN_WINDOW);
    }
    hr
}

unsafe extern "system" fn d3d3_device_end_scene(this:*mut c_void)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_END_SCENE) else{return DDERR_GENERIC};
    let original:D3D3SceneFn=std::mem::transmute(addr);
    let hr=original(this);
    if cfg().log_enabled && hr>=0 {
        sample_render_calls("D3D3.EndScene",&D3D3_END_EVENTS,&D3D3_END_START,&D3D3_END_WINDOW);
    }
    hr
}

unsafe extern "system" fn d3d3_device_set_render_target(
    this:*mut c_void,target:*mut c_void,flags:u32
)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_SET_RENDER_TARGET) else{return DDERR_GENERIC};
    let original:D3D3SetRenderTargetFn=std::mem::transmute(addr);
    let hr=original(this,target,flags);
    if cfg().log_enabled && D3D3_RENDER_TARGET_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16{
        let caps=if hr>=0 {modern_surface_caps(target)} else {None};
        log(&format!(
            "D3D3-DEVICE: SetRenderTarget flags={flags:#x} result={hr:#x} target_caps={} role={} (unchanged)",
            caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into())));
    }
    hr
}

unsafe fn observe_d3d3_device(device:*mut c_void){
    if device.is_null() || !cfg().log_enabled{return;}
    let begin=replace_method(device,D3D3_DEVICE_BEGIN_SCENE,d3d3_device_begin_scene as usize);
    let end=replace_method(device,D3D3_DEVICE_END_SCENE,d3d3_device_end_scene as usize);
    let target=replace_method(device,D3D3_DEVICE_SET_RENDER_TARGET,d3d3_device_set_render_target as usize);
    if D3D3_DEVICE_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!(
            "D3D3-DEVICE: hooks BeginScene={begin} EndScene={end} SetRenderTarget={target} (observation only)"));
    }
}

// In Wine d3d.h, IDirect3D3 vtable slot 8 is
// CreateDevice(REFCLSID, IDirectDrawSurface4*, IDirect3DDevice3**, IUnknown*).
// This exact Win32 x86 ABI is observed *without* changing the device.
unsafe extern "system" fn d3d3_create_device(
    this:*mut c_void,device_clsid:*const Guid,target:*mut c_void,
    out:*mut *mut c_void,outer:*mut c_void
)->Hr {
    let Some(addr)=orig(this,D3D3_CREATE_DEVICE) else{return DDERR_GENERIC};
    let call:D3D3CreateDeviceFn=std::mem::transmute(addr);
    let hr=call(this,device_clsid,target,out,outer);
    if cfg().log_enabled && hr>=0 && !out.is_null() && !(*out).is_null() {
        observe_d3d3_device(*out);
    }
    if cfg().log_enabled && D3D3_CREATE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        // If CreateDevice rejects a surface, the supplied target may not be
        // valid for post-call probing. Only inspect after success.
        let caps=if hr>=0 {modern_surface_caps(target)} else {None};
        log(&format!(
            "D3D3-TRACE: CreateDevice clsid={} render_target_caps={} role={} hr={hr:#x} object_returned={} (unchanged)",
            guid_label(device_clsid),
            caps.map(|c|format!("{c:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into()),
            hr>=0 && !out.is_null() && !(*out).is_null()));
        // Never lock the render target or modify its 3D state.
        if hr>=0 { probe_modern_surface_layout(target,"D3D3::CreateDevice target"); }
    }
    hr
}

unsafe fn observe_d3d3(object:*mut c_void) {
    if object.is_null() || !cfg().log_enabled{return;}
    let hooked=replace_method(object,D3D3_CREATE_DEVICE,d3d3_create_device as usize);
    if D3D3_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("D3D3-TRACE: IDirect3D3 CreateDevice hook_installed={hooked} (passive)"));
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
        if is_d3d3_iid(iid){observe_d3d3(*out);}
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
    fn log_identity_has_no_deprecated_project_name() {
        assert_eq!(PROJECT_NAME,"CrimsonSkiesFix");
        assert!(!SYSTEM_DDRAW_RECURSION_ERROR.contains("CrimsonFix"));
        assert!(SYSTEM_DDRAW_RECURSION_ERROR.contains(PROJECT_NAME));
    }
    #[test]
    fn real_d3d3_interface_guid_and_create_device_slot() {
        let d3d3=Guid{d1:0xbb223240,d2:0xe72b,d3:0x11d0,
            d4:[0xa9,0xb4,0x00,0xaa,0x00,0xc0,0x99,0x3e]};
        assert!(is_d3d3_iid(&d3d3));
        assert_eq!(D3D3_CREATE_DEVICE,8);
        assert_eq!(D3D3_DEVICE_BEGIN_SCENE,9);
        assert_eq!(D3D3_DEVICE_END_SCENE,10);
        assert_eq!(D3D3_DEVICE_SET_RENDER_TARGET,14);
        assert!(!is_d3d3_iid(ptr::null()));
    }
    #[test]
    fn surface_capability_roles_include_depth_and_texture_mipmaps(){
        assert_eq!(surface_roles(0x10024000),"z-buffer");
        assert_eq!(surface_roles(0x00401808),"complex+texture+mipmap");
        assert_eq!(surface_roles(0x1000601c),"backbuffer+flip-chain+complex");
    }
    #[test]
    fn surface_capabilities_identify_front_and_back_buffers(){
        assert_eq!(surface_roles(0x1000e238),"primary+flip-chain+complex");
        assert_eq!(surface_roles(DDSCAPS_BACKBUFFER|DDSCAPS_FLIP),"backbuffer+flip-chain");
        assert_eq!(surface_roles(0x1000601c),"backbuffer+flip-chain+complex");
        assert_eq!(surface_roles(0),"other");
    }
    #[test]
    fn directdraw_surface_slots_match_com_abi(){
        assert_eq!(SURFACE_GET_ATTACHED,12);
        assert_eq!(SURFACE_GET_DESC,22);
    }
    #[test]
    fn primary_caps_flag_is_not_equal_to_flip_capability(){
        assert_eq!(DDSCAPS_PRIMARYSURFACE,0x200);
        // A Flip may be issued on a front buffer, but some Flip calls
        // operate on offscreen surfaces; never pace those in this build.
        assert_eq!(DDSCAPS_PRIMARYSURFACE & 0x10,0);
    }
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
