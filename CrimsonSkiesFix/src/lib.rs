//! Experimental x86 DirectDraw proxy for the locally unpacked Crimson Skies 1.0.2.0.
//! Place this built DLL under the name ddraw.dll beside CRIMSON.EXE.
#![cfg(windows)]
mod config;
mod build_info;
mod menu_scene;
mod gos_video_lock; // experimental, backed-up native GOS Video .rof overlay
mod menu_video_probe;
mod menu_mode_event;
mod menu_gos_event;
mod menu_ui_trace;
mod menu_ui_list_probe;
mod menu_resolution_label;
mod menu_ui_pages;
mod menu_brand;
mod win32;
// Native Windows requires the complete DDRAW export surface for hardware HAL.
#[cfg(all(target_arch = "x86", target_env = "msvc"))]
mod ddraw_forward;
mod com_vtable;
mod d3d3_hooks;
mod legacy_ddraw_hooks;
mod modern_ddraw_hooks;
mod modern_surface_hooks;
mod com_labels;
mod frame_pacing;
mod render_sampling;
mod blt_crash_trace;
mod window_exposure;
mod window_erase_guard;
mod diagnostics;
mod diagnostics_graphics;
mod native_video_imports;
mod native_getproc_trace;
mod native_cursor_trace;
mod native_dinput_mouse_trace; // opt-in Windows-only DirectInput mouse COM pass-through audit
mod native_owner_mode_trace;
mod native_dd4_bootstrap;
mod presentation;
mod scene_correlation;
mod borderless_contract;
mod borderless_flip; // shared Flip-to-Blt contract, exercised by standalone probe
mod borderless_native_blt; // shared native Blt backend and MainMenu RGB565 helper
mod com_surface4_abi;
mod color_probe;
mod color_capture; // opt-in native XRGB/RGB565 comparison capture
mod com_surface4_forward; // inactive wrapper prototype; native source unwrapping helper remains used
mod in_game_adapter; // default-on native HAL/Surface4 virtualized windowed adapter
mod exe_gate; // only our exact unwrapped v1.02 game EXE is permitted
mod roffile_terminator_fix; // default-on Wine-only, exact-hash-guarded ROFFILE EOF fix
mod native_rof_open_trace; // shared Windows/Wine authenticated ROF handle routing
mod native_rof_read_trace; // shared ROF identity validation + optional ReadFile telemetry
mod native_rof_load_trace; // shared game LoadLibrary observer for original ROFFILE.DLL
mod native_widescreen; // runtime widescreen/FOV adaptation of the verified unwrapped game EXE
mod surface_usage; // opt-in passive COM surface audit for borderless prerequisite
mod blt_signature; // opt-in exact Blt flags + source-presence audit

use config::Config;
use com_vtable::{method,orig,replace_method};
use d3d3_hooks::observe_d3d3;
use modern_ddraw_hooks::observe_modern_ddraw;
use modern_surface_hooks::{modern_surface_caps,probe_modern_surface_layout,observe_modern_surface};
use legacy_ddraw_hooks::{ddraw_query_interface,surface_query_interface,set_cooperative,set_display_mode,restore_display_mode,create_surface};
use com_labels::{guid_label,ddraw_iface,surface_iface,surface_roles,describe_surface_layout,is_d3d3_iid};
use frame_pacing::pace_frame;
use render_sampling::sample_render_calls;
use blt_crash_trace::{CRASH_BLT_LOG,NATIVE_BLT_CRASH_TRACE_ID,crash_blt_line};
#[cfg(test)]
use blt_crash_trace::{CRASH_BLT_ROTATE_CALLS,crash_blt_rotation_needed};
fn color_probe_enabled()->bool{
    // The virtual game graph can only be created AFTER cfg() initializes.
    // Never initialize configuration/filesystem while a COM Blt is in
    // progress (or from ABI mock/unit tests).
    CONFIG.get().is_some_and(|c|c.log_enabled && c.color_probe)
}
fn color_capture_enabled()->bool{
    CONFIG.get().is_some_and(|c|c.log_enabled && c.color_capture)
}
fn color_capture_path(label:&str)->Option<std::path::PathBuf>{
    win32::color_capture_path(label)
}
use std::{ffi::c_void, fs::{self,File,OpenOptions}, io::Write,
          ptr, sync::{atomic::{AtomicUsize,Ordering}, Mutex,OnceLock,Once}, time::Instant};
use win32::Hwnd;

// Historical INI names are recognized only by the import logic;
// all user-facing diagnostic text uses the canonical project name.
// Kept only for the identity regression assertion, not for production.
#[cfg(test)]
const PROJECT_NAME:&str="CrimsonSkiesFix";
const SYSTEM_DDRAW_RECURSION_ERROR:&str="ERROR: loading the system DirectDraw resolved back to CrimsonSkiesFix proxy; aborting to avoid recursion";

type Hr=i32;
type CreateFn=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*mut c_void)->Hr;
type EnumerateFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type CreateExFn=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*const c_void,*mut c_void)->Hr;
type CooperativeFn=unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr;
type DisplayModeFn=unsafe extern "system" fn(*mut c_void,u32,u32,u32)->Hr;
type RestoreModeFn=unsafe extern "system" fn(*mut c_void)->Hr;
type SurfaceFn=unsafe extern "system" fn(*mut c_void,*mut c_void,*mut *mut c_void,*mut c_void)->Hr;
type FlipFn=unsafe extern "system" fn(*mut c_void,*mut c_void,u32)->Hr;
type BltFn=unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,*const c_void,u32,*mut c_void)->Hr;
type GetCapsFn=unsafe extern "system" fn(*mut c_void,*mut u32)->Hr;
type ModernDisplayModeFn=unsafe extern "system" fn(*mut c_void,u32,u32,u32,u32,u32)->Hr;
type GetDisplayModeFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type QueryInterfaceFn=unsafe extern "system" fn(*mut c_void,*const Guid,*mut *mut c_void)->Hr;
type BltFastFn=unsafe extern "system" fn(*mut c_void,u32,u32,*mut c_void,*const c_void,u32)->Hr;
type UnlockFn=unsafe extern "system" fn(*mut c_void,*const c_void)->Hr;
type GetAttachedSurfaceFn=unsafe extern "system" fn(*mut c_void,*const u32,*mut *mut c_void)->Hr;
type GetSurfaceDescFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type D3D3CreateDeviceFn=unsafe extern "system" fn(*mut c_void,*const Guid,*mut c_void,*mut *mut c_void,*mut c_void)->Hr;
type D3D3SceneFn=unsafe extern "system" fn(*mut c_void)->Hr;
type D3D3SetRenderTargetFn=unsafe extern "system" fn(*mut c_void,*mut c_void,u32)->Hr;
type D3D3GetRenderTargetFn=unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr;

#[repr(C)]
#[derive(Clone,Copy)]
struct Guid {d1:u32,d2:u16,d3:u16,d4:[u8;8]}
const S_OK:Hr=0;
const DDERR_GENERIC:Hr=0x8876_0000u32 as i32;
const DDSCL_NORMAL:u32=0x0000_0008;
const DDSCAPS_PRIMARYSURFACE:u32=0x0000_0200;
// IDirectDraw v1 slots. Surface Flip/Blt/GetCaps slots are also stable in
// later surface interface versions; QueryInterface interception is TODO.
const COM_QUERY_INTERFACE:usize=0;
const DD_CREATE_SURFACE:usize=6;
const DD_SET_COOPERATIVE:usize=20;
const DD_SET_DISPLAY_MODE:usize=21;
const DD_RESTORE_DISPLAY_MODE:usize=19; // stable IDirectDraw 1/2/4/7 COM slot
const DD_GET_DISPLAY_MODE:usize=12;
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
const D3D3_DEVICE_GET_RENDER_TARGET:usize=15;
const DDSCAPS_BACKBUFFER:u32=0x0000_0004;
const DDSCAPS_FLIP:u32=0x0000_0010;
const DDSCAPS_COMPLEX:u32=0x0000_0008;

static CONFIG:OnceLock<Config>=OnceLock::new();
static NATIVE_IMPORT_AUDIT_ONCE:Once=Once::new();
static LOG:OnceLock<Mutex<File>>=OnceLock::new();
// Only enabled for a controlled crash reproduction. The detailed source and
// destination pointers are in a separate, bounded rotating file.
static REAL_DLL:OnceLock<Option<usize>>=OnceLock::new();
static FIRST_FLIP:Once=Once::new();
static FIRST_BLT:Once=Once::new();
static COOPERATIVE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
// Bounded, passive diagnostics of the earliest native DirectDraw calls.
// In particular, failed Create/Enumerate calls were previously silent.
static STARTUP_DDRAW_CREATE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static STARTUP_DDRAW_ENUM_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static STARTUP_LEGACY_SURFACE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
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
static MODERN_GET_DISPLAY_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_SURFACE_CREATE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_SURFACE_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MODERN_DDRAW_HOOK_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);
static FIRST_MODERN_VALID_HWND:Once=Once::new();
static FIRST_MODERN_PACED_FLIP:Once=Once::new();
static FIRST_MODERN_NONPRIMARY_FLIP:Once=Once::new();
static MODERN_FLIP_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MODERN_BLT_EVENTS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_BLT_SOURCE_UNWRAP_COUNT:AtomicUsize=AtomicUsize::new(0);
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
static GAME_DD4_VTABLE:AtomicUsize=AtomicUsize::new(0);
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
        // Existing user INIs may contain repeated [Debug] keys from
        // several Windows A/B rounds. Clean ONLY recognized duplicate
        // section/key entries, never change any effective parsed value.
        // Back up the exact old INI before a guarded atomic rewrite.
        // Leave the file entirely untouched if no duplicates are found.
        let (parsed,ini_dedup_status)=if let Some((path,source))=ini.as_ref()
            .and_then(|path|fs::read_to_string(path).ok().map(|s|(path,s))){
            let parsed=config::parse(&source);
            let status=config::deduplicate_known_ini(&source)
                .map(|(cleaned,removed)|{
                    match win32::commit_deduplicated_ini(path,&source,&cleaned){
                        Ok(backup)=>format!(
                            "CONFIG: {removed} repeated known INI keys removed without changing settings; original saved to {}",
                            backup.display()
                        ),
                        Err(err)=>format!(
                            "CONFIG: {removed} repeated known INI keys found; cleanup safely skipped: {err}"
                        )
                    }
                });
            (parsed,status)
        }else{(Config::default(),None)};
        // One debug-only A/B test for 125%-scaled native Windows: request
        // SYSTEM DPI awareness before *any* game HWND is created and before
        // the passive Win32 display inventory. Never enable by default,
        // under Wine/Proton, or during pure DirectDraw pass-through.
        let dpi_diagnostic=if parsed.native_dpi_aware && !parsed.directdraw_passthrough {
            Some(win32::enable_native_system_dpi_experiment())
        }else{None};
        if parsed.log_enabled || parsed.experimental_windowed_adapter {
            if let Some(path)=win32::log_path() {
                if let Ok(file)=OpenOptions::new().create(true).write(true).truncate(true).open(path) {
                    let _=LOG.set(Mutex::new(file));
                }
            }
            if LOG.get().is_some() {
                log(&format!("{} x86 - init; configuration={parsed:?}; DirectDraw proxy",build_info::label()));
                if let Some(ref status)=ini_dedup_status{log(status);}
                if parsed.gos_video_lock==gos_video_lock::Mode::Off &&
                   parsed.experimental_windowed_adapter &&
                   parsed.desktop_resolution && parsed.native_3d_widescreen {
                    log("GOS-VIDEO-LOCK: GosVideoLock=off explicitly in existing INI; Graphics/Resolution dropdowns are NOT greyed; use [Graphics] GosVideoLock=on to enable verified read-only GOS controls");
                }
                if let Some(ref result)=dpi_diagnostic {log(result);}
                // Only one read-only inventory, gated by the explicit opt-in.
                diagnostics::report(log);
            }
        }
        // Read once at startup, never from DllMain. This file is intentionally
        // independent of CrimsonSkiesFix.log: a fixed 20,000-call limit
        // expired BEFORE the v0.1.53 gameplay crash.
        if parsed.log_enabled && parsed.crash_blt_trace {
            if let Some(path)=win32::crash_blt_trace_path(){
                match OpenOptions::new().create(true).write(true).truncate(true).open(&path){
                    Ok(file)=>{
                        let _=CRASH_BLT_LOG.set(Mutex::new(file));
                        log("CRASH-BLT: rolling trace active: CrimsonSkiesFix-BltTrace.log, latest 2048 native Blt calls retained");
                    }
                    Err(err)=>log(&format!("CRASH-BLT: cannot create rolling trace: {err}")),
                }
            }else{
                log("CRASH-BLT: cannot resolve safe rolling trace path");
            }
        }
        parsed
    })
}

fn real_module()->Option<*mut c_void> {
    let result=(*REAL_DLL.get_or_init(|| unsafe {win32::real_ddraw().map(|x|x as usize)}))
        .map(|x|x as *mut c_void);
    // Resolve the proxy module from our own function address, not from a
    // user-defined exported DllMain. The latter adds an unwanted 23rd PE
    // export to the canonical 22-entry native DirectDraw export table.
    let Some(own)=win32::own_module() else {
        log("DDRAW-FORWARD: unable to identify loaded proxy module");
        return None;
    };
    if result==Some(own) {
        log(SYSTEM_DDRAW_RECURSION_ERROR);
        return None;
    }
    result
}

unsafe fn symbol(name:&[u8])->*const c_void {
    // Reject unknown game executables before loading system DirectDraw or
    // installing hooks. On denial the gate logs SHA-256 and shows a dialog.
    if !exe_gate::verify_once(){return ptr::null();}
    let config=cfg();
    // Install the managed, verified GOS overlay before initializing file-ID
    // references for our shared Windows/Wine resource hook. Otherwise an
    // already-loaded ROFFILE could cache an absent pre-patch reference.
    // Pass-through and explicit opt-out never edit or intercept resources.
    if !config.directdraw_passthrough {
        static GOS_LOCK_ONCE:Once=Once::new();
        GOS_LOCK_ONCE.call_once(|| {
            gos_video_lock::apply(
                config.gos_video_lock,
                config.experimental_windowed_adapter &&
                config.native_3d_widescreen &&
                config.desktop_resolution
            );
        });
        // Observe only the authenticated game's loader imports. The same
        // code selects the already-correct archive on Wine or redirects a
        // verified original copy on Windows; no OS-specific routing branch.
        static ROF_ROUTING_EARLY_ONCE:Once=Once::new();
        ROF_ROUTING_EARLY_ONCE.call_once(|| {
            native_rof_load_trace::install();
            native_rof_open_trace::probe();
        });
    }
    if config.log_enabled && diagnostics::is_native_windows() && !config.directdraw_passthrough {
        NATIVE_IMPORT_AUDIT_ONCE.call_once(||{
            native_video_imports::report();
            // Only diagnostic Win32 IAT hook after SHA-256 admission, never
            // a system export hook and never under the Windows loader lock.
            native_getproc_trace::install();
            // Native windowed A/B only; exact EXE USER32 import provenance,
            // no cursor/input modifications or system export detours.
            native_cursor_trace::install();
            // Native-only passive mouse-device inquiry. WM_MOUSEMOVE can
            // arrive even when the original game's DirectInput cursor does
            // not advance; trace actual HRESULT, acquire and movement.
            native_dinput_mouse_trace::install();
        });
    }
    // Diagnostic-only transparent proxy: forward the three exported entry
    // points to the original system DLL, without touching game code, GOS
    // resources, COM vtables or any DirectDraw interfaces. The exact EXE
    // hash gate above still applies. No option is changed permanently.
    if config.directdraw_passthrough {
        let Some(real)=real_module() else {
            log("ERROR: system32\\ddraw.dll cannot be loaded in pass-through");
            return ptr::null();
        };
        return win32::system_export(real,name);
    }
    // The shared authenticated GOS resource patch and loader hook have
    // already been initialized above, before original script loading.
    // Freeze the screen under the cursor at the first game DirectDraw call.
    // It represents the launch screen, not the cursor's later location.
    // The actual game HWND is validated separately before it can be moved.
    if config.experimental_windowed_adapter {
        log("WINDOWED-DIAG: launch monitor capture branch entered");
        static SCREEN_ONCE:Once=Once::new();
        SCREEN_ONCE.call_once(||{
            if let Some(rect)=win32::capture_launch_monitor(){
                if let Some((w,h))=rect.dimensions(){
                    log(&format!("WINDOWED-GAME: launch monitor origin={},{} size={}x{}",
                        rect.left,rect.top,w,h));
                }
            }else{
                log("WINDOWED-GAME: launch monitor unknown; game HWND monitor will be used");
            }
        });
    }
    // Apply the FOV/HUD delta at the earliest DirectDraw export with the
    // same monitor geometry later used by the actual game's window.
    static EARLY_NATIVE_ONCE:Once=Once::new();
    // On native Windows only, the opt-in intro A/B leaves the game's initial
    // 1024/800 movie mode untouched. Patching to desktop resolution here
    // would force the native exclusive video path into an unsupported mode.
    // The original DD4 cooperative path applies FOV/HUD after MainMenu.
    let native_intro_exclusive=config.native_intro_exclusive &&
        config.native_dd4_bootstrap && config.experimental_windowed_adapter &&
        config.native_3d_widescreen && diagnostics::is_native_windows();
    if native_intro_exclusive {
        log("NATIVE-INTRO: original exclusive DirectDraw retained for startup video; early widescreen patch deferred");
    }
    if config.experimental_windowed_adapter && config.native_3d_widescreen &&
       !config.defer_early_native_patch && !native_intro_exclusive {
        EARLY_NATIVE_ONCE.call_once(||{
            log("WINDOWED-DIAG: early FOV/HUD monitor query begin");
            let logical=win32::capture_launch_monitor().and_then(|r|r.dimensions())
                .or_else(win32::sole_primary_monitor_dimensions);
            // Native Windows 11 under 125% scaling reported virtual
            // 1536x960 although the physical monitor and game HAL render
            // 1920x1200. The early EXE patch must use PHYSICAL pixels to
            // avoid "Failed to initialize video". Wine/Proton retain the
            // already-validated logical-monitored path. Never change global
            // DPI awareness, OS display mode or game files.
            let native_windows=diagnostics::is_native_windows();
            let physical=if native_windows || config.early_native_physical_mode {
                win32::physical_primary_monitor_dimensions()
            }else{None};
            let size=win32::early_patch_dimensions(
                logical,physical,native_windows,config.early_native_physical_mode
            );
            if native_windows || config.early_native_physical_mode {
                log(&format!(
                    "WINDOWED-DIAG: physical early native probe logical={logical:?} physical={physical:?} native_windows={native_windows} selected={size:?} (read-only, no DPI changes)"
                ));
            }
            if let Some((w,h))=size {
                log(&format!("WINDOWED-DIAG: early FOV/HUD patch begin target={}x{}",w,h));
                unsafe{native_widescreen::prime_native_widescreen_early(w,h);}
                log("WINDOWED-DIAG: early FOV/HUD patch returned");
            }else{
                log("WINDOWED-WIDE: early patch deferred; no eligible launch monitor dimensions");
            }
        });
    }
    if config.experimental_windowed_adapter && config.native_3d_widescreen && config.defer_early_native_patch {
        // Controlled A/B diagnosis: do not modify the EXE during early
        // DirectDraw discovery. The existing DD4 cooperative path still
        // applies the same verified in-memory patch if the game reaches it.
        // This changes no Windows DPI awareness, system mode or game asset.
        log("WINDOWED-DIAG: early FOV/HUD patch intentionally deferred to DD4 (debug-only)");
    }
    // Experimental native Windows bridge: intercept the EXACT authenticated
    // game's DD4 mode-setup function BEFORE its first exclusive request.
    // Default OFF. This is distinct from read-only owner diagnostics.
    if config.native_dd4_bootstrap {
        static DD4_BOOTSTRAP_ONCE:Once=Once::new();
        DD4_BOOTSTRAP_ONCE.call_once(||{
            let ok=native_dd4_bootstrap::install();
            log(&format!("NATIVE-BOOTSTRAP: enabled={} installed={ok}",config.native_dd4_bootstrap));
        });
    }
    // Observe the exact in-game MainMenu/Preferences script transitions
    // only after strict game executable authentication.
    if config.experimental_windowed_adapter {
        static MENU_SCENE_ONCE:Once=Once::new();
        MENU_SCENE_ONCE.call_once(||{
            log("WINDOWED-DIAG: menu_scene hook install begin");
            let installed=unsafe{menu_scene::install()};
            log(&format!("MENU-SCENE: hook_available={installed}"));
        });
    }
    let Some(real)=real_module() else {log("ERROR: system32\\ddraw.dll cannot be loaded");return ptr::null()};
    win32::system_export(real,name)
}

fn trace_game_windows(stage:&str) {
    if CONFIG.get().is_some_and(|cfg|cfg.log_enabled) &&
       WINDOW_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<6 {
        log(&format!("WIN-TRACE: at {stage}: {}",win32::game_windows_snapshot()));
    }
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawCreate(guid:*const c_void,out:*mut *mut c_void,outer:*mut c_void)->Hr {
    // Must run before the default-enabled startup selector's in-memory patch.
    if !exe_gate::verify_once(){
        if !out.is_null(){*out=ptr::null_mut();}
        return DDERR_GENERIC;
    }
    if cfg().directdraw_passthrough {
        let function=symbol(b"DirectDrawCreate\0");
        if function.is_null(){return DDERR_GENERIC;}
        let real:CreateFn=std::mem::transmute(function);
        let hr=real(guid,out,outer);
        log(&format!(
            "PASSTHROUGH: DirectDrawCreate original hr={:#010x} object_returned={}; no COM hooks",
            hr as u32,hr>=0 && !out.is_null() && !(*out).is_null()));
        return hr;
    }
    // Install the verified GOS callback even if Log=0: the command-14
    // Graphics guard and menu-visibility state must not depend on logging.
    static GOS_MENU_EVENT_ONCE:Once=Once::new();
    GOS_MENU_EVENT_ONCE.call_once(||{
        if cfg().experimental_windowed_adapter && cfg().native_3d_widescreen && cfg().desktop_resolution {
            log("WINDOWED-DIAG: menu_gos_event hook install begin");
            let active=unsafe{menu_gos_event::install()};
            log(&format!("MENU-GOS-EVENT: hook_available={active}"));
        }
    });
    // Override ONLY the original GOS 0x845 resolution-text formatter.
    // This is independent of debug logging and never mutates the mode table.
    static GOS_RESOLUTION_LABEL_ONCE:Once=Once::new();
    GOS_RESOLUTION_LABEL_ONCE.call_once(||{
        if cfg().experimental_windowed_adapter && cfg().native_3d_widescreen && cfg().desktop_resolution {
            log("WINDOWED-DIAG: menu_resolution_label hook install begin");
            let installed=unsafe{menu_resolution_label::install()};
            log(&format!("MENU-RESOLUTION-LABEL: hook_available={installed}"));
        }
    });
    // Passive GOS UI tracing is off with Log=0; unlike the verified
    // backend guards, it never changes native GUI behavior.
    static GOS_UI_TRACE_ONCE:Once=Once::new();
    GOS_UI_TRACE_ONCE.call_once(||{
        if cfg().experimental_windowed_adapter && cfg().log_enabled {
            log("WINDOWED-DIAG: menu_ui_trace hook install begin");
            let (data,control)=unsafe{menu_ui_trace::install()};
            log(&format!("MENU-GOS-UI: data_registered={data} control_registered={control}"));
        }
    });
    // Separate exact GOS page/localization hooks: diagnostic only.
    // Capture which original script resources are requested while
    // Video Preferences is active; no strings or UI state modified.
    static GOS_UI_PAGES_ONCE:Once=Once::new();
    GOS_UI_PAGES_ONCE.call_once(||{
        if cfg().experimental_windowed_adapter && cfg().log_enabled {
            log("WINDOWED-DIAG: menu_ui_pages hook install begin");
            let (pages,localize)=unsafe{menu_ui_pages::install()};
            log(&format!("MENU-GOS-PAGE: page_registered={pages} localize_registered={localize}"));
        }
    });
    // The command-15 mode guard is separately constrained to the
    // 4:3 menu and the three native-widescreen option flags.
    static VIDEO_EVENT_ONCE:Once=Once::new();
    VIDEO_EVENT_ONCE.call_once(||{
        if cfg().experimental_windowed_adapter && cfg().native_3d_widescreen && cfg().desktop_resolution {
            log("WINDOWED-DIAG: menu_mode_event hook install begin");
            let installed=unsafe{menu_mode_event::install()};
            log(&format!("MENU-VIDEO-EVENT: hook_available={installed}"));
        }
    });
    // Install before DirectDraw enumeration: the game can show the
    // AskForVDev device chooser before a game HWND or DD4 exists.
    static VIDEO_SELECTOR_ONCE:Once=Once::new();
    VIDEO_SELECTOR_ONCE.call_once(||{
        if cfg().native_3d_widescreen {
            let applied=unsafe{native_widescreen::suppress_video_selector()};
            log(&format!("STARTUP-VIDEO: AskForVDev bypass={} (HAL selection unchanged)",applied));
        }
    });
    let function=symbol(b"DirectDrawCreate\0");
    let trace=cfg().log_enabled &&
        STARTUP_DDRAW_CREATE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<32;
    if function.is_null(){
        if trace {log("STARTUP-DDRAW: DirectDrawCreate system export unavailable");}
        return DDERR_GENERIC;
    }
    let real:CreateFn=std::mem::transmute(function);
    let hr=real(guid,out,outer);
    if trace {
        log(&format!(
            "STARTUP-DDRAW: DirectDrawCreate guid={} outer={} hr={:#010x} object_returned={}",
            if guid.is_null(){"NULL"}else{"provided"},
            if outer.is_null(){"NULL"}else{"provided"},
            hr as u32,hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        let dd=*out;
        let qi=replace_method(dd,COM_QUERY_INTERFACE,ddraw_query_interface as *const () as usize);
        let a=replace_method(dd,DD_SET_COOPERATIVE,set_cooperative as *const () as usize);
        let b=replace_method(dd,DD_SET_DISPLAY_MODE,set_display_mode as *const () as usize);
        let restore=if cfg().log_enabled && diagnostics::is_native_windows(){
            replace_method(dd,DD_RESTORE_DISPLAY_MODE,restore_display_mode as *const () as usize)
        }else{false};
        let c=replace_method(dd,DD_CREATE_SURFACE,create_surface as *const () as usize);
        log(&format!("DirectDrawCreate interception: query_interface={qi}, cooperative={a}, mode={b}, restore_mode={restore}, surface={c}"));
    }
    hr
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawEnumerateA(callback:*mut c_void,context:*mut c_void)->Hr {
    let function=symbol(b"DirectDrawEnumerateA\0");
    let trace=cfg().log_enabled &&
        STARTUP_DDRAW_ENUM_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<32;
    if function.is_null(){
        if trace {log("STARTUP-DDRAW: DirectDrawEnumerateA system export unavailable");}
        return DDERR_GENERIC;
    }
    let real:EnumerateFn=std::mem::transmute(function);
    let hr=real(callback,context);
    if trace {
        log(&format!(
            "STARTUP-DDRAW: DirectDrawEnumerateA callback={} context={} hr={:#010x}",
            if callback.is_null(){"NULL"}else{"provided"},
            if context.is_null(){"NULL"}else{"provided"},
            hr as u32));
    }
    hr
}

#[no_mangle]
pub unsafe extern "system" fn DirectDrawCreateEx(guid:*const c_void,out:*mut *mut c_void,iid:*const c_void,outer:*mut c_void)->Hr {
    let function=symbol(b"DirectDrawCreateEx\0");
    if function.is_null(){return DDERR_GENERIC;}
    let real:CreateExFn=std::mem::transmute(function);
    // Diagnostic-only until newer DirectDraw COM interfaces are understood.
    let hr=real(guid,out,iid,outer);
    if cfg().directdraw_passthrough {
        log(&format!(
            "PASSTHROUGH: DirectDrawCreateEx original hr={:#010x} object_returned={}; no COM hooks",
            hr as u32,hr>=0 && !out.is_null() && !(*out).is_null()));
        return hr;
    }
    log(&format!("COM-TRACE: DirectDrawCreateEx iid={} result={hr:#x} object_returned={}",
        guid_label(iid.cast()),hr>=0 && !out.is_null() && !(*out).is_null()));
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        if let Some(version)=ddraw_iface(iid.cast()){observe_modern_ddraw(*out,version);}
    }
    hr
}

#[cfg(test)]
mod trace_tests {
    use super::*;
    #[test]
    fn crash_blt_rolling_trace_keeps_logging_after_twenty_thousand_calls(){
        assert!(!crash_blt_rotation_needed(1));
        assert!(!crash_blt_rotation_needed(2048));
        assert!(crash_blt_rotation_needed(2049));
        assert!(crash_blt_rotation_needed(4097));
        assert!(!crash_blt_rotation_needed(20_000));
        assert!(!crash_blt_rotation_needed(0));
        assert_eq!(CRASH_BLT_ROTATE_CALLS,2048);
    }
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
        assert_eq!(DD_GET_DISPLAY_MODE,12);
        assert_eq!(D3D3_DEVICE_BEGIN_SCENE,9);
        assert_eq!(D3D3_DEVICE_END_SCENE,10);
        assert_eq!(D3D3_DEVICE_SET_RENDER_TARGET,14);
        assert_eq!(D3D3_DEVICE_GET_RENDER_TARGET,15);
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
