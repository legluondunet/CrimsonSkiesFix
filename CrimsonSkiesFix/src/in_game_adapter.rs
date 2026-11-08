//! Experimental game-owned DirectDraw4 windowed resource adapter.
//! Exact path: verified IDirectDraw4 cooperative -> mode -> virtual Surface4 chain.
//! All native objects are created on the GAME's own DD4, not a probe DD4.
//! Never pretend a native HAL device was created: D3D3 receives native Surface4.
//!
//! This is intentionally an early in-game integration build. The game may use
//! unsupported COM methods, QI versions or window transitions. Default-on for the accepted EXE.
use std::{ffi::c_void,ptr,sync::{Mutex,OnceLock,atomic::{AtomicU8,AtomicUsize,Ordering}}};
use crate::{
    Hr,Guid,Hwnd,log,win32,
    borderless_contract::{AdapterContract,BackendEvidence,RenderTarget,SurfaceRole},
    presentation::{Mode,Chain},
    borderless_flip,
    borderless_native_blt::NativeWindowedBlt,
    com_surface4_forward,
};

const E_FAIL:Hr=0x8000_4005u32 as i32;
const E_NOTIMPL:Hr=0x8000_4001u32 as i32;
const DD_CAPS_PRIMARY:u32=0x0000_0200;
const DD_CAPS_OFFSCREEN:u32=0x0000_0040;
const DD_CAPS_3D:u32=0x0000_2000;
const DD_CAPS_VIDEOMEM:u32=0x0000_4000;
const DD_CAPS_SYSTEMMEM:u32=0x0000_0800;
const DD_CAPS_Z:u32=0x0002_0000;
const DDSD_BACKBUFFERCOUNT:u32=0x20;
const DDSD_HEIGHT:u32=0x2;
const DDSD_WIDTH:u32=0x4;
const DDSD_PIXELFORMAT:u32=0x1000;
const DDSCL_NORMAL:u32=8;
const DDSCL_EXCLUSIVE:u32=0x10;
const DDSCL_EXCLUSIVE_FULLSCREEN:u32=0x13;
// Strict native Windows borderless operation must NEVER forward the
// original game's exclusive cooperative request or real SetDisplayMode
// after an attempted virtual DD4 setup fails.
fn strict_nonexclusive_selection(
    native:bool,borderless:bool,desktop:bool,adapter:bool,
    bootstrap:bool,intro_exclusive:bool,passthrough:bool
)->bool{
    native && borderless && desktop && adapter && bootstrap &&
        !intro_exclusive && !passthrough
}
pub fn strict_native_nonexclusive()->bool{
    let c=crate::cfg();
    strict_nonexclusive_selection(
        crate::diagnostics::is_native_windows(),c.borderless,c.desktop_resolution,
        c.experimental_windowed_adapter,c.native_dd4_bootstrap,
        c.native_intro_exclusive,c.directdraw_passthrough
    )
}
fn allow_original_exclusive_fallback()->bool{
    !strict_native_nonexclusive()
}
const DDBLT_COLORFILL:u32=0x400;
const DDBLT_DEPTHFILL:u32=0x0200_0000;
const DDBLT_WAIT:u32=0x0100_0000;
const HAL:Guid=Guid{d1:0x84e63de0,d2:0x46aa,d3:0x11cf,d4:[0x81,0x6f,0,0,0xc0,0x20,0x15,0x6e]};
const IID_D3D3:Guid=Guid{d1:0xbb223240,d2:0xe72b,d3:0x11d0,d4:[0xa9,0xb4,0,0xaa,0,0xc0,0x99,0x3e]};

// All stored COM references are owned and released. Store pointer addresses
// as usize so the mutex remains Send; COM methods execute in game process.
struct Com(usize);
impl Com {
    fn ptr(&self)->*mut c_void{self.0 as *mut c_void}
    unsafe fn from_out(p:*mut c_void)->Result<Self,String>{
        if p.is_null(){Err("NULL native COM result".into())}else{Ok(Self(p as usize))}
    }
    unsafe fn add_ref(&self)->Result<(),String>{
        let f:unsafe extern "system" fn(*mut c_void)->u32=std::mem::transmute(slot(self.ptr(),1));
        if f(self.ptr())==0{Err("native AddRef failed".into())}else{Ok(())}
    }
}
impl Drop for Com {
    fn drop(&mut self){
        unsafe {
            let p=self.ptr();
            if !p.is_null(){
                let f:unsafe extern "system" fn(*mut c_void)->u32=std::mem::transmute(slot(p,2));
                let _=f(p);
            }
        }
    }
}
unsafe fn slot(object:*mut c_void,index:usize)->usize{
    *(*object.cast::<*const usize>()).add(index)
}
unsafe fn alloc(dd4:*mut c_void,mut desc:[u32;31])->Result<Com,String>{
    type Fn=unsafe extern "system" fn(*mut c_void,*mut u32,*mut *mut c_void,*mut c_void)->Hr;
    // Directly call the original DD4 method rather than recursing into our
    // CreateSurface hook when constructing the physical presentation pair.
    let Some(addr)=crate::orig(dd4,6) else{return Err("DD4 original CreateSurface missing".into());};
    let create:Fn=std::mem::transmute(addr);
    let mut p=ptr::null_mut();
    let hr=create(dd4,desc.as_mut_ptr(),&mut p,ptr::null_mut());
    if hr<0{return Err(format!("native CreateSurface HRESULT={:#010x}",hr as u32));}
    Com::from_out(p)
}
fn desc(caps:u32,w:u32,h:u32,bits:Option<u32>)->[u32;31]{
    let mut d=[0u32;31];d[0]=124;d[1]=1;d[26]=caps;
    if w!=0&&h!=0{d[1]|=2|4;d[2]=h;d[3]=w;}
    if let Some(b)=bits{
        d[1]|=DDSD_PIXELFORMAT;d[18]=32;d[19]=if caps&DD_CAPS_Z!=0{0x400}else{0x40};
        d[21]=b;
        if caps&DD_CAPS_Z!=0{d[23]=0x00ff_ffff;}
        else if b==32 {
            d[22]=0x00ff_0000;d[23]=0x0000_ff00;d[24]=0x0000_00ff;
        }
        else {d[22]=0xf800;d[23]=0x07e0;d[24]=0x001f;}
    }
    d
}
fn require(hr:Hr,stage:&str)->Result<(),String>{
    if hr<0{Err(format!("{stage}: HRESULT={:#010x}",hr as u32))}else{Ok(())}
}
unsafe fn new_clipper(dd4:*mut c_void,hwnd:Hwnd,primary:&Com)->Result<Com,String>{
    type Create=unsafe extern "system" fn(*mut c_void,u32,*mut *mut c_void,*mut c_void)->Hr;
    let create:Create=std::mem::transmute(slot(dd4,4));
    let mut out=ptr::null_mut();
    require(create(dd4,0,&mut out,ptr::null_mut()),"CreateClipper")?;
    let clip=Com::from_out(out)?;
    type SetHwnd=unsafe extern "system" fn(*mut c_void,u32,Hwnd)->Hr;
    type SetClip=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
    let set_hwnd:SetHwnd=std::mem::transmute(slot(clip.ptr(),8));
    require(set_hwnd(clip.ptr(),0,hwnd),"clipper.SetHWnd")?;
    let set_clip:SetClip=std::mem::transmute(slot(primary.ptr(),28));
    require(set_clip(primary.ptr(),clip.ptr()),"primary.SetClipper")?;
    Ok(clip)
}
unsafe fn attach_depth(back:&Com,depth:&Com)->Result<(),String>{
    type Attach=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
    let f:Attach=std::mem::transmute(slot(back.ptr(),3));
    require(f(back.ptr(),depth.ptr()),"RGB565.AddAttachedSurface(Z32)")
}
unsafe fn native_fill(target:&Com,depth:bool)->Result<(),String>{
    type Blt=unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,*const c_void,u32,*mut u32)->Hr;
    let f:Blt=std::mem::transmute(slot(target.ptr(),5));
    let mut fx=[0u32;25];fx[0]=100;fx[20]=if depth{0xffff_ffff}else{0};
    require(f(target.ptr(),ptr::null(),ptr::null_mut(),ptr::null(),
        if depth{DDBLT_DEPTHFILL}else{DDBLT_COLORFILL|DDBLT_WAIT},fx.as_mut_ptr()),
        if depth{"Z32.DepthFill"}else{"RGB565.ColorFill"})
}
unsafe fn prove_hal(dd4:*mut c_void,back:&Com)->Result<(),String>{
    type Query=unsafe extern "system" fn(*mut c_void,*const Guid,*mut *mut c_void)->Hr;
    let qi:Query=std::mem::transmute(slot(dd4,0));
    let mut result=ptr::null_mut();
    require(qi(dd4,&IID_D3D3,&mut result),"IDirect3D3 QueryInterface")?;
    let d3d=Com::from_out(result)?;
    type Create=unsafe extern "system" fn(*mut c_void,*const Guid,*mut c_void,*mut *mut c_void,*mut c_void)->Hr;
    // The game-facing D3D3 vtable may already be diagnostic-hooked.
    // A synchronous preflight while RUNTIME is locked must call the
    // ORIGINAL CreateDevice directly, never reenter the game's hooks.
    let create:Create=std::mem::transmute(
        crate::orig(d3d.ptr(),8).unwrap_or_else(||slot(d3d.ptr(),8)));
    let mut device=ptr::null_mut();
    // Keep the exact native HRESULT visible: previous Windows traces ended
    // inside this call, unable to distinguish a synchronous COM deadlock
    // from an ordinary unsupported HAL surface/driver failure.
    log("WINDOWED-DIAG: D3D3 HAL CreateDevice call ENTER (original native method)");
    let hr=create(d3d.ptr(),&HAL,back.ptr(),&mut device,ptr::null_mut());
    log(&format!("WINDOWED-DIAG: D3D3 HAL CreateDevice call RETURN hr={:#010x} device_returned={}",
        hr as u32,!device.is_null()));
    require(hr,"D3D3 HAL CreateDevice")?;
    let _device=Com::from_out(device)?;
    Ok(())
}
unsafe fn native_desc(surface:&Com)->Result<[u32;31],String>{
    type Get=unsafe extern "system" fn(*mut c_void,*mut u32)->Hr;
    let get:Get=std::mem::transmute(slot(surface.ptr(),22));
    let mut d=[0u32;31];d[0]=124;
    require(get(surface.ptr(),d.as_mut_ptr()),"native GetSurfaceDesc")?;
    Ok(d)
}
fn check_native_rgb_depth(
    back:&Com,depth:&Com,mode:Mode,movie_rgb32:bool
)->Result<(),String>{
    unsafe{
        let b=native_desc(back)?;let z=native_desc(depth)?;
        let required=if movie_rgb32 {
            (mode.width,mode.height,32,0xff0000,0xff00,0xff)
        }else{(mode.width,mode.height,16,0xf800,0x07e0,0x001f)};
        if (b[3],b[2],b[21],b[22],b[23],b[24])!=required ||
            (movie_rgb32 && b[19]&0x40==0){
            return Err(format!(
                "native {} mismatch: {}x{} bpp={} rgb_masks={:#x}/{:#x}/{:#x}",
                if movie_rgb32{"XRGB8888 movie"}else{"RGB565"},
                b[3],b[2],b[21],b[22],b[23],b[24]));
        }
        if (z[3],z[2],z[21])!=(mode.width,mode.height,32) ||
            z[19]&0x400==0 || z[23]==0 {
            return Err(format!("native Z32 mismatch: {}x{} bpp={}",z[3],z[2],z[21]));
        }
    }
    Ok(())
}
// We cannot use a 1920x1080 native desktop primary for Crimson Skies'
// initial 640x480 CPU Lock. Verify a dedicated 16-bit RGB565 native staging
// surface, including an actual CPU Lock+Unlock, before offering its pointer
// through a virtual IDirectDrawSurface4 primary.
unsafe fn verify_primary_lock_staging(staging:&Com,mode:Mode)->Result<(),String>{
    let desc=native_desc(staging)?;
    if (desc[3],desc[2],desc[21],desc[22],desc[23],desc[24])!=
        (mode.width,mode.height,16,0xf800,0x07e0,0x001f) ||
        desc[26]&DD_CAPS_OFFSCREEN==0 || desc[26]&DD_CAPS_SYSTEMMEM==0{
        return Err(format!("primary lock staging RGB565 mismatch caps={:#010x} {}x{} bpp={}",
            desc[26],desc[3],desc[2],desc[21]));
    }
    type Lock=unsafe extern "system" fn(*mut c_void,*const c_void,*mut u32,u32,*mut c_void)->Hr;
    type Unlock=unsafe extern "system" fn(*mut c_void,*const c_void)->Hr;
    let lock:Lock=std::mem::transmute(slot(staging.ptr(),25));
    let unlock:Unlock=std::mem::transmute(slot(staging.ptr(),32));
    let mut info=[0u32;31];info[0]=124;
    require(lock(staging.ptr(),ptr::null(),info.as_mut_ptr(),0,ptr::null_mut()),
        "RGB565 primary staging Lock")?;
    // Always unlock a successful native Lock before checking the returned
    // DDSURFACEDESC2. Never strand the staging surface in locked state.
    let unlock_hr=unlock(staging.ptr(),ptr::null());
    require(unlock_hr,"RGB565 primary staging Unlock")?;
    if (info[0],info[3],info[2],info[21])!=(124,mode.width,mode.height,16) ||
       info[9]==0 || info[4]==0 {
        return Err(format!("primary staging Lock incompatible: {}x{} bpp={} pitch={} pixels={}",
            info[3],info[2],info[21],info[4],if info[9]==0{"NULL"}else{"provided"}));
    }
    Ok(())
}

struct Epoch {
    mode:Mode,
    primary:Com,
    back:Com,
    depth:Com,
    // These three physical resources and clipper outlive their virtual COM
    // counterpart; every wrapper owns an additional native AddRef itself.
    physical_primary:Com,
    physical_lock_primary:Com, // independent logical RGB565 primary staging
    physical_back:Com, // native D3D3 RGB565 backbuffer; retained for identity checks
    physical_depth:Com,
    // Dedicated offscreen menu image, never exposed to the game. The
    // strong COM reference outlives the borrowed pointer in the Flip graph.
    menu_scratch:Option<Com>,
    movie_rgb565_scratch:Option<Com>,
    clipper:Com,
}
struct Runtime {
    dd4:usize,
    dd4_owner:Com, // strong COM parent ref for the entire experiment
    hwnd:usize,
    monitor:win32::Rect, // DPI-logical HWND/Win32 coordinates
    render_client:(u32,u32), // native physical DirectDraw pixels
    generation:u64,
    exposed:bool,
    native_preparing:bool, // in-flight native probe; release state mutex across COM calls
    native_widescreen_ready:bool,
    epoch:Option<Epoch>,
}
static RUNTIME:Mutex<Option<Runtime>>=Mutex::new(None);

// Windows-native-only startup A/B: avoid exposing synthetic RGB565/virtual
// primary surfaces to the DirectShow MPEG introduction. Do not switch a
// running virtual graph back into exclusive mode: that was previously
// demonstrated unsafe. Keep the ORIGINAL exclusive DD4 path until the
// 800x600 introduction has presented MainMenu. The first following DD4
// cooperative request is our ONE opportunity to switch before the game
// negotiates its next display mode. Once SetDisplayMode leaves 800x600,
// never switch a running native game back into a virtual graph: on native
// Win11 this late switch happened on ESC -> MainMenu and yielded a white
// screen with sound continuing. Wine/Proton and defaults are unaffected.
const INTRO_BEFORE_MOVIE:u8=0;
const INTRO_IN_MOVIE_MODE:u8=1;
const INTRO_LEFT_MOVIE_MODE:u8=2;
static NATIVE_INTRO_PHASE:AtomicU8=AtomicU8::new(INTRO_BEFORE_MOVIE);
static NATIVE_INTRO_SKIPPED:AtomicUsize=AtomicUsize::new(0);
// One-shot owner / HWND captured while the original DD4 stays EXCLUSIVE.
// Its next SetDisplayMode decides whether virtual rendering is admissible.
// We must not prematurely enter NORMAL merely because MainMenu is active.
static NATIVE_INTRO_PENDING:Mutex<Option<(usize,usize)>>=Mutex::new(None);
// Sample once BEFORE the game's first native exclusive DD4 request.
// EnumDisplaySettingsW subsequently returns the temporary 1024/800 game
// mode, which must never be confused with the initial monitor raster.
// Caching None also prevents a late unsafe retry during an exclusive mode.
static NATIVE_INTRO_LAUNCH_PHYSICAL:OnceLock<Option<(u32,u32)>>=OnceLock::new();
fn same_aspect(logical:(u32,u32),physical:(u32,u32))->bool{
    // Match the Win32/physical raster relationship using the same
    // one-pixel DPI rounding tolerance as the real DirectDraw presentation.
    // The old exact cross-product rejected otherwise valid user screens
    // such as physical 1366x768 represented by logical 1093x614.
    crate::presentation::dpi_output_matches(physical,logical)
}
fn native_intro_launch_physical()->Option<(u32,u32)>{
    *NATIVE_INTRO_LAUNCH_PHYSICAL.get_or_init(||{
        let logical=win32::capture_launch_monitor().and_then(|r|r.dimensions());
        let physical=win32::physical_primary_monitor_dimensions();
        let accepted=match (logical,physical) {
            (Some(l),Some(p)) if same_aspect(l,p)=>Some(p),
            _=>None,
        };
        log(&format!(
            "NATIVE-INTRO: pre-exclusive monitor capture logical={logical:?} physical={physical:?} accepted={accepted:?} (frozen for this process, no display changes)"
        ));
        accepted
    })
}
fn native_intro_exclusive_enabled()->bool{
    let cfg=crate::cfg();
    cfg.native_intro_exclusive && cfg.native_dd4_bootstrap &&
    cfg.experimental_windowed_adapter && cfg.native_3d_widescreen &&
    crate::diagnostics::is_native_windows()
}
fn next_intro_phase(current:u8,mode:Mode)->u8{
    match (current,(mode.width,mode.height,mode.bpp)) {
        (INTRO_BEFORE_MOVIE,(800,600,16))=>INTRO_IN_MOVIE_MODE,
        (INTRO_IN_MOVIE_MODE,(800,600,16))=>INTRO_IN_MOVIE_MODE,
        (INTRO_IN_MOVIE_MODE,_)=>INTRO_LEFT_MOVIE_MODE,
        _=>current,
    }
}
// MainMenu is loaded *while still in* the original 800x600 mode.
// Coop is called BEFORE the subsequent SetDisplayMode. Waiting until
// SetDisplayMode leaves 800x600 was too late: the next eligible coop
// occurred during ESC -> MainMenu, after native 3D gameplay, causing an
// unsafe mid-session switch to synthetic surfaces.
fn native_intro_ready(phase:u8,main_active:bool)->bool{
    phase==INTRO_IN_MOVIE_MODE && main_active
}
// A native-only intro handoff must not commit the 1920x1200 FOV/HUD
// patch when the next game-requested mode is actually 1024x768.
// Never enter a virtual graph in that case. The old unconditional patch
// at SetCooperativeLevel survived the fallback and corrupted native 3D.
fn native_intro_requested_3d(mode:Mode,target:(u32,u32))->bool{
    mode.bpp==16 && mode.width==target.0 && mode.height==target.1 &&
    mode.width>=960 && mode.height>=600
}
fn native_intro_can_prepare(mode:Mode,target:(u32,u32),graph_exposed:bool,wide_ready:bool)->bool{
    let native_3d=native_intro_requested_3d(mode,target);
    if !graph_exposed {native_3d}
    else{matches!((mode.width,mode.height,mode.bpp),(640,480,16)|(800,600,16)) ||
         (native_3d && wide_ready)}
}
fn observe_native_intro_mode(mode:Mode){
    if !native_intro_exclusive_enabled(){return;}
    let before=NATIVE_INTRO_PHASE.load(Ordering::Acquire);
    let after=next_intro_phase(before,mode);
    if after!=before && NATIVE_INTRO_PHASE.compare_exchange(
        before,after,Ordering::AcqRel,Ordering::Acquire).is_ok() {
        log(match after {
            INTRO_IN_MOVIE_MODE=>
                "NATIVE-INTRO: original 800x600 exclusive movie mode observed; virtual graph remains disabled",
            INTRO_LEFT_MOVIE_MODE=>
                "NATIVE-INTRO: game left original 800x600 mode; late virtual activation permanently disarmed for this launch",
            _=>"NATIVE-INTRO: unexpected phase transition",
        });
    }
}

// The game can repeat its *same* DDSCL_EXCLUSIVE|DDSCL_FULLSCREEN
// request after our windowed graph has already been exposed. That is an
// idempotent request from the game's perspective; do NOT forward it to
// native DirectDraw, which must remain in DDSCL_NORMAL. Any changed HWND,
// DD4 identity or flags must still fail closed.
fn identical_game_cooperative_request(
    runtime_dd4:usize,runtime_hwnd:usize,exposed:bool,has_epoch:bool,
    dd4:usize,hwnd:usize,flags:u32
)->bool{
    exposed && has_epoch && runtime_dd4==dd4 && runtime_hwnd==hwnd &&
        hwnd!=0 && flags==DDSCL_EXCLUSIVE_FULLSCREEN
}

/// Called from the real game's IDirectDraw4::SetCooperativeLevel hook.
/// The original game's normal/exclusive path is left alone if opt-in is 0.
/// Only accept a valid process-owned HWND and standard exclusive request.
pub unsafe fn cooperative(dd4:*mut c_void,hwnd:Hwnd,flags:u32,
    original:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr)->Option<Hr>{
    cooperative_with_intent(dd4,hwnd,flags,original,false)
}
// commit_pending=true is used only by display_mode after a one-shot
// (DD4,HWND) match plus the game's exact requested native raster.
unsafe fn cooperative_with_intent(dd4:*mut c_void,hwnd:Hwnd,flags:u32,
    original:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr,
    commit_pending:bool)->Option<Hr>{
    if crate::cfg().log_enabled {
        log(&format!("WINDOWED-DIAG: DD4 SetCooperativeLevel enter dd4={:p} hwnd={:p} flags={:#010x}",
            dd4,hwnd,flags));
    }
    // Do not expose any virtual surfaces while native DirectShow owns the
    // introduction. This branch is reached before any windowed runtime has
    // been created; after exposure, existing strict graph checks win.
    let native_intro=native_intro_exclusive_enabled();
    // This must happen BEFORE forwarding the very first exclusive coop call,
    // even though virtual-mode activation is intentionally still forbidden.
    if native_intro {
        let _=native_intro_launch_physical();
    }
    if native_intro && !commit_pending {
        let phase=NATIVE_INTRO_PHASE.load(Ordering::Acquire);
        let main_active=crate::menu_scene::main_menu_was_seen() &&
                        crate::menu_scene::main_menu_active();
        let no_windowed_runtime=RUNTIME.lock().ok().is_some_and(|r|r.is_none());
        if no_windowed_runtime && native_intro_ready(phase,main_active) {
            // The game's next SetDisplayMode is not known yet. Original
            // Windows 11 actually requests 1024x768, so do not briefly
            // flip native DirectDraw to DDSCL_NORMAL in anticipation.
            if flags!=DDSCL_EXCLUSIVE_FULLSCREEN || hwnd.is_null() ||
               win32::launch_monitor_or_game_window(hwnd).is_none() {
                log("NATIVE-INTRO: cannot stage DD4 handoff; original cooperative request retained");
                return None;
            }
            let hr=original(dd4,hwnd,flags);
            if hr>=0 {
                let Ok(mut pending)=NATIVE_INTRO_PENDING.lock() else{return Some(E_FAIL)};
                *pending=Some((dd4 as usize,hwnd as usize));
                log("NATIVE-INTRO: MainMenu eligible, original exclusive DD4 preserved; waiting for NEXT requested SetDisplayMode before changing cooperative level");
            }
            return Some(hr);
        }
        if no_windowed_runtime {
            let count=NATIVE_INTRO_SKIPPED.fetch_add(1,Ordering::Relaxed);
            if phase==INTRO_LEFT_MOVIE_MODE {
                if count<8{
                    log("NATIVE-INTRO: reject LATE virtual DD4 activation after native 3D mode; original exclusive path preserved (ESC/menu white-screen guard)");
                }
            }else if count<8{
                log("NATIVE-INTRO: forwarding original exclusive DD4 cooperative request; awaiting 800x600 MainMenu transition");
            }
            return None;
        }
    }
    // A second identical game-exclusive request is expected during video
    // startup, after the virtual PRIMARY and HAL device have been created.
    // Keep the native DD4 in NORMAL, and preserve the live virtual COM graph.
    // A different mode, HWND or interface is NOT silently acknowledged.
    {
        let Ok(guard)=RUNTIME.lock() else{return Some(E_FAIL)};
        if let Some(runtime)=guard.as_ref(){
            if runtime.native_preparing {
                log("WINDOWED-GAME: nested DD4 cooperative request during native HAL preflight rejected");
                return Some(E_FAIL);
            }
            if runtime.exposed {
                let same=identical_game_cooperative_request(
                    runtime.dd4,runtime.hwnd,runtime.exposed,runtime.epoch.is_some(),
                    dd4 as usize,hwnd as usize,flags);
                let Some(expected)=runtime.monitor.client_geometry() else{return Some(E_FAIL)};
                if same && win32::game_client_geometry(hwnd)==Some(expected){
                    log("WINDOWED-GAME: repeated identical game DD4 exclusive cooperative request acknowledged; native DDSCL_NORMAL and virtual graph preserved");
                    return Some(0);
                }
                log(&format!("WINDOWED-GAME: changed/invalid cooperative request after graph exposure rejected (same_request={same}, flags={flags:#x}, geometry_ok={})",
                    same && win32::game_client_geometry(hwnd)==Some(expected)));
                return Some(E_FAIL);
            }
        }
    }
    if flags&DDSCL_EXCLUSIVE_FULLSCREEN != DDSCL_EXCLUSIVE_FULLSCREEN || hwnd.is_null(){
        if strict_native_nonexclusive() && flags&DDSCL_EXCLUSIVE!=0 {
            log("NATIVE-STRICT-BORDERLESS: invalid/NULL game HWND; exclusive cooperative request REJECTED");
            return Some(E_FAIL);
        }
        // Do not allow an old active graph to silently revert to native
        // DDSCL_EXCLUSIVE or DDSCL_NORMAL through another cooperative call.
        if RUNTIME.lock().ok().and_then(|r|r.as_ref().map(|x|x.exposed)).unwrap_or(false){
            log("WINDOWED-GAME: unexpected cooperative reconfiguration while virtual graph exposed: rejected");
            return Some(E_NOTIMPL);
        }
        return None;
    }
    let Some(monitor)=win32::launch_monitor_or_game_window(hwnd) else{
        if strict_native_nonexclusive(){
            log("NATIVE-STRICT-BORDERLESS: invalid monitor/HWND; exclusive cooperative request REJECTED");
            return Some(E_FAIL);
        }
        log("WINDOWED-GAME: rejected game HWND; original exclusive path retained");
        return None;
    };
    let Some((monitor_w,monitor_h))=monitor.dimensions() else{
        if strict_native_nonexclusive(){
            log("NATIVE-STRICT-BORDERLESS: invalid monitor dimensions; exclusive cooperative request REJECTED");
            return Some(E_FAIL);
        }
        log("WINDOWED-GAME: invalid launch monitor dimensions; original path retained");
        return None;
    };
    // Native Windows 11 may show 1536x960 DPI-logical Win32 pixels while
    // DirectDraw uses the 1920x1200 physical raster. Do not change process
    // awareness: separate the game-owned window geometry from HAL pixel
    // dimensions, and fail closed for ambiguous monitors/aspect ratios.
    let render_client=if crate::diagnostics::is_native_windows(){
        // During the original native intro, the physical *current display
        // mode* becomes 1024x768/800x600. The verified launch raster,
        // not that transient exclusive mode, is the intended output.
        // Other configurations keep their established live-mode behavior.
        let current=if native_intro {
            win32::physical_primary_monitor_dimensions()
        }else{None};
        let physical=if native_intro {
            native_intro_launch_physical()
        }else{
            win32::physical_primary_monitor_dimensions()
        };
        let Some(physical)=physical else{
            if strict_native_nonexclusive(){
                log("NATIVE-STRICT-BORDERLESS: physical monitor unknown; exclusive cooperative request REJECTED");
                return Some(E_FAIL);
            }
            log("WINDOWED-GAME: native physical monitor ambiguous or uncaptured; original exclusive path retained");
            return None;
        };
        if !same_aspect((monitor_w,monitor_h),physical) {
            log(&format!(
                "WINDOWED-GAME: native logical/physical aspect mismatch logical={}x{} initial_physical={physical:?} current_exclusive={current:?}; {}",
                monitor_w,monitor_h,
                if strict_native_nonexclusive(){"strict nonexclusive: original mode request rejected"}
                else{"original exclusive path retained"}
            ));
            return if strict_native_nonexclusive(){Some(E_FAIL)}else{None};
        }
        if native_intro{
            log(&format!(
                "NATIVE-INTRO: using verified pre-exclusive physical raster={physical:?} instead of current exclusive display mode={current:?}; native windowed transition eligible"
            ));
        }
        physical
    }else{(monitor_w,monitor_h)};
    log(&format!("WINDOWED-GAME: target monitor origin={},{} logical={}x{} physical_render={}x{}",
        monitor.left,monitor.top,monitor_w,monitor_h,render_client.0,render_client.1));
    let Ok(mut guard)=RUNTIME.lock() else{
        return if strict_native_nonexclusive(){Some(E_FAIL)}else{None};
    };
    if guard.as_ref().is_some_and(|state|state.exposed){
        log("WINDOWED-GAME: repeated cooperative call while graph exposed: fail closed");
        return Some(E_FAIL);
    }
    // Prepare the actual DD4 cooperative level, but do not modify window
    // styles, suppress SetDisplayMode or expose any game surfaces yet.
    log("WINDOWED-DIAG: DD4 native SetCooperativeLevel(DDSCL_NORMAL) begin");
    let hr=original(dd4,hwnd,DDSCL_NORMAL);
    log(&format!("WINDOWED-DIAG: DD4 native SetCooperativeLevel(DDSCL_NORMAL) returned {:#010x}",hr as u32));
    if hr<0{
        log(&format!("WINDOWED-GAME: DDSCL_NORMAL rejected ({:#010x}); {}",
            hr as u32,if strict_native_nonexclusive(){
                "strict borderless: original exclusive fallback FORBIDDEN"
            }else{"original exclusive path retained"}));
        return if strict_native_nonexclusive(){Some(hr)}else{None};
    }
    // The game can Release its own DD4 interface while virtual surfaces
    // remain referenced. Hold one distinct strong DD4 COM reference.
    type AddRef=unsafe extern "system" fn(*mut c_void)->u32;
    let add_ref:AddRef=std::mem::transmute(slot(dd4,1));
    if add_ref(dd4)==0{
        log("WINDOWED-GAME: DD4 AddRef failed after DDSCL_NORMAL; cannot expose");
        return Some(E_FAIL);
    }
    // Set the patched EXE's two canonical 3D dimensions before its
    // first display-mode negotiation. These exact two fields also feed
    // the patch's x87 horizontal-FOV calculation. The game original
    // 4:3 menus/videos still request 640x480 or 800x600 unchanged.
    // Fail closed for unrecognized EXEs; never patch an arbitrary image.
    // Geometry was checked above, including monitors with negative origins.
    let native_widescreen_ready=if native_intro {
        // At SetCooperativeLevel the game has not yet supplied the new
        // mode. The observed Win11 sequence requests 1024x768 AFTER
        // this call: patching 1920x1200 here and then falling back to
        // exclusive native 1024x768 corrupts geometry, sky and HUD.
        // Defer the actual in-memory patch until a matching mode AND the
        // native graph preflight both succeed. If the game requests its
        // legacy 1024x768, do not patch at all.
        log("NATIVE-INTRO: widescreen FOV/HUD patch STAGED, not applied before requested display mode is verified");
        false
    }else if crate::cfg().native_3d_widescreen{
        // Only the original unwrapped EXE, authenticated by exe_gate,
        // can reach this path. No support for third-party widescreen EXEs.
        log("WINDOWED-DIAG: apply/reuse early native FOV/HUD patch begin");
        let result=crate::native_widescreen::apply_or_reuse_early_patch(render_client.0,render_client.1);
        log("WINDOWED-DIAG: apply/reuse early native FOV/HUD patch returned");
        log(&format!(
            "WINDOWED-WIDE: verified unwrapped EXE; native resolution+FOV+HUD {}x{} result={result:?}",
            render_client.0,render_client.1));
        matches!(result,
            crate::native_widescreen::PatchResult::Patched |
            crate::native_widescreen::PatchResult::AlreadyNative)
    }else{false};
    *guard=Some(Runtime{dd4:dd4 as usize,dd4_owner:Com(dd4 as usize),hwnd:hwnd as usize,
        monitor,render_client,generation:0,exposed:false,native_preparing:false,native_widescreen_ready,epoch:None});
    log("WINDOWED-GAME: DDSCL_NORMAL accepted on game DD4; awaiting mode and native HAL resource preflight");
    Some(hr)
}
/// Handle a real game SetDisplayMode. The callback is the original native
/// 5-argument DD4 function, so pre-exposure failures can still recover the
/// original exclusive path. No fallback is attempted after graph exposure.
pub unsafe fn display_mode(dd4:*mut c_void,w:u32,h:u32,bpp:u32,
    refresh:u32,flags:u32,
    original:unsafe extern "system" fn(*mut c_void,u32,u32,u32,u32,u32)->Hr)
    ->Option<Hr>{
    if crate::cfg().log_enabled {
        log(&format!("WINDOWED-DIAG: DD4 SetDisplayMode enter dd4={:p} requested={}x{}x{} refresh={} flags={:#x}",
            dd4,w,h,bpp,refresh,flags));
    }
    // Consume the one-shot pending owner before changing the video phase.
    // This ensures a legacy 1024x768 request sees exactly the original
    // exclusive behavior and never visits DDSCL_NORMAL even briefly.
    let requested=Mode{width:w,height:h,bpp};
    if native_intro_exclusive_enabled() {
        let pending=match NATIVE_INTRO_PENDING.lock(){
            Ok(mut guard)=>guard.take(),
            Err(_)=>return Some(E_FAIL),
        };
        if let Some((owner,game_hwnd))=pending{
            let physical=native_intro_launch_physical();
            let eligible=owner==dd4 as usize &&
                NATIVE_INTRO_PHASE.load(Ordering::Acquire)==INTRO_IN_MOVIE_MODE &&
                physical.is_some_and(|target|native_intro_requested_3d(requested,target));
            if eligible {
                type CooperativeFn=unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr;
                let Some(addr)=crate::orig(dd4,20) else{return Some(E_FAIL)};
                let native_coop:CooperativeFn=std::mem::transmute(addr);
                log("NATIVE-INTRO: exact native target mode requested; changing exclusive DD4 to NORMAL only now");
                match cooperative_with_intent(dd4,game_hwnd as Hwnd,
                        DDSCL_EXCLUSIVE_FULLSCREEN,native_coop,true) {
                    Some(hr) if hr>=0 =>{},
                    Some(hr)=>{
                        log(&format!("NATIVE-INTRO: deferred DDSCL_NORMAL failed {:#010x}; requested mode not modified",hr as u32));
                        return Some(hr);
                    },
                    None=>log("NATIVE-INTRO: deferred DD4 handoff rejected; retaining original exclusive mode"),
                }
            }else{
                log(&format!(
                    "NATIVE-INTRO: next requested {}x{}x{} differs from native target {physical:?}; NO intermediate DDSCL_NORMAL and NO FOV/HUD patch",
                    w,h,bpp
                ));
            }
        }
    }
    // Observation only while the original native movie graph owns DD4.
    // Never advance this state on Wine or without explicit opt-in.
    observe_native_intro_mode(requested);
    let Ok(mut state)=RUNTIME.lock() else{return Some(E_FAIL);};
    let Some(runtime)=state.as_mut() else{
        if strict_native_nonexclusive(){
            log("NATIVE-STRICT-BORDERLESS: DD4 SetDisplayMode without verified runtime REJECTED");
            return Some(E_FAIL);
        }
        return None;
    };
    if runtime.dd4!=dd4 as usize{
        return if strict_native_nonexclusive(){Some(E_FAIL)}else{None};
    }
    if runtime.native_preparing {
        log("WINDOWED-GAME: nested SetDisplayMode during native HAL preflight rejected");
        return Some(E_FAIL);
    }
    let mode=Mode{width:w,height:h,bpp};
    let (mon_w,mon_h)=runtime.render_client;
    let intro_pending=native_intro_exclusive_enabled();
    let native_3d=if intro_pending {
        native_intro_requested_3d(mode,runtime.render_client) &&
            (!runtime.exposed || runtime.native_widescreen_ready)
    }else{
        runtime.native_widescreen_ready && bpp==16 &&
            w==mon_w && h==mon_h && w>=960 && h>=600
    };
    let supported=if intro_pending{
        native_intro_can_prepare(mode,runtime.render_client,
                                 runtime.exposed,runtime.native_widescreen_ready)
    }else{
        matches!((w,h,bpp),(640,480,16)|(800,600,16)) || native_3d
    };
    if !supported{
        if intro_pending && !runtime.exposed {
            log(&format!(
                "NATIVE-INTRO: game requested original {}x{}x{} instead of verified native {}x{}; NO widescreen patch applied; restoring original exclusive DirectDraw",
                w,h,bpp,mon_w,mon_h
            ));
        }
        log(&format!("WINDOWED-GAME: unsupported mode {}x{}x{}, fail closed",w,h,bpp));
        if runtime.exposed{return Some(E_FAIL);}
        if !allow_original_exclusive_fallback(){
            log("NATIVE-STRICT-BORDERLESS: unsupported game mode rejected without original SetDisplayMode or DDSCL_EXCLUSIVE; physical desktop unchanged");
            return Some(E_FAIL);
        }
        // Legacy recovery is permitted only while no virtual COM interface exists.
        let coop:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr=
            match crate::orig(dd4,20){Some(addr)=>std::mem::transmute(addr),None=>return Some(E_FAIL)};
        let hr=coop(dd4,runtime.hwnd as Hwnd,DDSCL_EXCLUSIVE_FULLSCREEN);
        *state=None;
        if hr<0{return Some(hr);}
        return Some(original(dd4,w,h,bpp,refresh,flags));
    }
    // The old 800x600 menu input transform must never survive into
    // native 3D or a new mode. Re-arm only after new HAL/COM, HWND and
    // exact client geometry have all been verified.
    if crate::cfg().native_menu_mouse_remap &&
        crate::cfg().native_dd4_bootstrap &&
        crate::diagnostics::is_native_windows(){
        crate::window_erase_guard::disarm_native_menu_mouse_remap();
    }
    if let Some(old)=runtime.epoch.take(){
        if let Err(hr)=com_surface4_forward::retire_graph_primary_flip(old.primary.ptr()){
            log(&format!("WINDOWED-GAME: retiring old primary failed {:#010x}",hr as u32));
            runtime.epoch=Some(old);
            return Some(E_FAIL);
        }
        log("WINDOWED-GAME: old virtual chain retired; stale Flip/Blt disarmed");
    }
    log(&format!("WINDOWED-DIAG: prepare_epoch begin mode={}x{}x{} native_3d={}",
        w,h,bpp,native_3d));
    // Under Windows native, CreateDevice can synchronously call our OWN
    // hooked DirectDraw4::GetDisplayMode/CreateSurface/QueryInterface while
    // proving the driver's HAL. They may inspect RUNTIME, so holding the
    // non-recursive mutex here deadlocks inside the native CreateDevice call.
    // Keep the complete Runtime alive but publish an explicit preparation
    // guard, drop the mutex, and then revalidate ownership after the call.
    // Preserve the already-validated Wine/Proton call order untouched.
    let native_unlock=crate::diagnostics::is_native_windows() && crate::cfg().native_dd4_bootstrap;
    let hwnd_for_probe=runtime.hwnd as Hwnd;
    let client_for_probe=runtime.render_client;
    let output_for_probe=if native_unlock {
        let Some(logical)=runtime.monitor.dimensions() else{return Some(E_FAIL)};
        logical
    }else{client_for_probe};
    let generation_for_probe=runtime.generation+1;
    let result=if native_unlock {
        runtime.native_preparing=true;
        log("WINDOWED-DIAG: releasing adapter state mutex for reentrant native HAL preflight");
        drop(state);
        let result=prepare_epoch(dd4,hwnd_for_probe,client_for_probe,output_for_probe,mode,generation_for_probe,native_3d);
        state=match RUNTIME.lock(){Ok(guard)=>guard,Err(_)=>return Some(E_FAIL)};
        let Some(current)=state.as_mut() else {
            log("WINDOWED-GAME: runtime disappeared during native HAL preflight; fail closed");
            return Some(E_FAIL);
        };
        if current.dd4!=dd4 as usize || !current.native_preparing ||
           current.generation+1!=generation_for_probe {
            log("WINDOWED-GAME: runtime changed during native HAL preflight; fail closed");
            current.native_preparing=false;
            return Some(E_FAIL);
        }
        current.native_preparing=false;
        log("WINDOWED-DIAG: native HAL preflight returned; adapter state mutex reacquired");
        result
    }else{
        prepare_epoch(dd4,hwnd_for_probe,client_for_probe,output_for_probe,mode,generation_for_probe,native_3d)
    };
    log(&format!("WINDOWED-DIAG: prepare_epoch returned success={}",result.is_ok()));
    let Some(runtime)=state.as_mut() else{return Some(E_FAIL)};
    match result {
        Ok(next)=>{
            runtime.generation+=1;
            runtime.epoch=Some(next);
            let area=runtime.monitor;
            // Apply HWND style only after physical HAL and real Flip->Blt
            // preflight; the game now has a complete windowed COM graph.
            if let Err(err)=win32::borderless_window(runtime.hwnd as Hwnd,area){
                log(&format!("WINDOWED-GAME: borderless HWND style failed: {err}"));
                if runtime.exposed || !allow_original_exclusive_fallback(){
                    log("NATIVE-STRICT-BORDERLESS: borderless HWND style failure; refusing exclusive recovery/real SetDisplayMode");
                    return Some(E_FAIL);
                }
                let coop:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr=
                    match crate::orig(dd4,20){Some(addr)=>std::mem::transmute(addr),None=>return Some(E_FAIL)};
                let hr=coop(dd4,runtime.hwnd as Hwnd,DDSCL_EXCLUSIVE_FULLSCREEN);
                *state=None;
                if hr<0{return Some(hr);}
                return Some(original(dd4,w,h,bpp,refresh,flags));
            }
            // The host compositor may refuse/alter requested client bounds.
            // Do not start presenting to an assumed desktop-sized rectangle.
            let Some(expected)=area.client_geometry() else{return Some(E_FAIL)};
            if win32::game_client_geometry(runtime.hwnd as Hwnd)!=Some(expected){
                log(&format!("WINDOWED-GAME: HWND client geometry mismatch after borderless; expected={expected:?}; refusing to expose virtual surfaces"));
                return Some(E_FAIL);
            }
            if intro_pending && native_3d && !runtime.native_widescreen_ready {
                // Commit the native-only FOV/HUD transform strictly AFTER
                // native HAL graph preflight AND verified HWND geometry.
                // No original exclusive fallback is possible past this
                // point: the game is about to receive virtual surfaces.
                // full_patch::apply is byte-exact and rolls back failures.
                log("NATIVE-INTRO: native target mode and virtual graph verified; committing deferred FOV/HUD patch");
                let patch=crate::native_widescreen::apply_or_reuse_early_patch(
                    runtime.render_client.0,runtime.render_client.1);
                if !matches!(patch,
                    crate::native_widescreen::PatchResult::Patched |
                    crate::native_widescreen::PatchResult::AlreadyNative) {
                    log(&format!(
                        "NATIVE-INTRO: FOV/HUD commit refused result={patch:?}; virtual graph not exposed, native fallback forbidden"
                    ));
                    return Some(E_FAIL);
                }
                runtime.native_widescreen_ready=true;
                log(&format!(
                    "NATIVE-INTRO: FOV/HUD committed after verified native graph; dimensions={}x{} result={patch:?}",
                    runtime.render_client.0,runtime.render_client.1
                ));
            }
            if crate::cfg().window_erase_guard {
                // This experiment is only installed AFTER validating the
                // exact process-owned game window and normal DD4 graph.
                // A failure is logged, never hidden, but the game continues
                // with the previously validated v0.1.55 display behavior.
                match crate::window_erase_guard::install(runtime.hwnd){
                    Ok(())=>log("WINDOWED-GAME: WindowEraseGuard installed on verified game HWND (WM_ERASEBKGND only)"),
                    Err(err)=>log(&format!("WINDOWED-GAME: WindowEraseGuard unavailable: {err}")),
                }
            }
            // Windows native only, exact game-owned HWND already verified:
            // repair the game's pre-borderless ClipCursor when (and only
            // when) the OS still reports that same stale small rectangle.
            // Separate explicit A/B from WndProc mouse remapping.
            if crate::cfg().native_mouse_clip_repair &&
                crate::cfg().native_dd4_bootstrap &&
                crate::diagnostics::is_native_windows() &&
                native_movie_rgb32_enabled(mode) {
                match crate::native_cursor_trace::repair_stale_game_cursor_clip(
                    runtime.hwnd as *mut c_void
                ){
                    Ok(details)=>log(&format!(
                        "NATIVE-CURSOR-CLIP-REPAIR: applied {details}"
                    )),
                    Err(reason)=>log(&format!(
                        "NATIVE-CURSOR-CLIP-REPAIR: not applied ({reason}); OS clip unchanged"
                    )),
                }
            }
            // Experimental native Win11 RGB32 800x600 only: reinterpret
            // button/move client coordinates for the centered 4:3 source.
            // Arm *after* the exact HWND and its client rectangle have
            // been checked. MainMenu/Preferences script callback is a
            // separate required scene gate; no effect before that.
            if crate::cfg().native_menu_mouse_remap &&
                crate::cfg().native_dd4_bootstrap &&
                crate::diagnostics::is_native_windows() &&
                native_movie_rgb32_enabled(mode) {
                let output=(expected.2,expected.3);
                match crate::window_erase_guard::arm_native_menu_mouse_remap(
                    runtime.hwnd,output,runtime.render_client
                ){
                    Ok(view)=>log(&format!(
                        "NATIVE-MENU-MOUSE: opt-in armed for MainMenu/Preferences 800x600; verified HWND client={}x{} render_physical={}x{} content=({},{} {}x{}) via shared DD4 output geometry; WM_MOUSEMOVE/buttons only; script gate initially off",
                        output.0,output.1,runtime.render_client.0,runtime.render_client.1,
                        view.left,view.top,view.width,view.height)),
                    Err(error)=>log(&format!(
                        "NATIVE-MENU-MOUSE: opt-in refused: {error}; original mouse messages preserved")),
                }
            }
            log(&format!("WINDOWED-GAME: mode={}x{}x{} native HAL/Z32/virtual Surface4 ready; desktop unchanged; generation={}",
                w,h,bpp,runtime.generation));
            // Check the actual Windows physical display state after every
            // native virtual mode transition. A successful virtual Blt is
            // not proof that another module did not switch the OS display.
            if crate::cfg().native_dd4_bootstrap && crate::diagnostics::is_native_windows() {
                let observed=win32::physical_primary_monitor_dimensions();
                log(&format!(
                    "NATIVE-DESKTOP-VERIFY: virtual_mode={}x{}x{} physical_screen={observed:?} expected_screen={:?} retained={} (read-only)",
                    w,h,bpp,runtime.render_client,observed==Some(runtime.render_client)
                ));
                log(&format!(
                    "NATIVE-MOUSE-GEOMETRY: mode={}x{} Win32 HWND logical={:?} native_snapshot={} (read-only; no input remapping)",
                    w,h,win32::game_client_geometry(runtime.hwnd as Hwnd),
                    win32::native_presentation_snapshot()
                ));
            }
            Some(0)
        }
        Err(err)=>{
            log(&format!("WINDOWED-GAME: native preflight failed before exposure: {err}"));
            if runtime.exposed || !allow_original_exclusive_fallback(){
                log("NATIVE-STRICT-BORDERLESS: native graph preflight failed; no exclusive recovery or physical mode switch");
                return Some(E_FAIL);
            }
            let coop:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr=
                match crate::orig(dd4,20){Some(addr)=>std::mem::transmute(addr),None=>return Some(E_FAIL)};
            let hr=coop(dd4,runtime.hwnd as Hwnd,DDSCL_EXCLUSIVE_FULLSCREEN);
            *state=None;
            if hr<0{return Some(hr);}
            log("WINDOWED-GAME: original exclusive DD4 path restored (no virtual surface exposed)");
            Some(original(dd4,w,h,bpp,refresh,flags))
        }
    }
}
fn native_movie_rgb32_mode(mode:Mode)->bool{
    (mode.width,mode.height,mode.bpp)==(800,600,16)
}
fn native_movie_rgb32_enabled(mode:Mode)->bool{
    let c=crate::cfg();
    native_movie_rgb32_mode(mode) && c.native_movie_rgb32 &&
        c.native_dd4_bootstrap && c.experimental_windowed_adapter &&
        c.native_3d_widescreen && !c.native_intro_exclusive &&
        !c.directdraw_passthrough &&
        crate::diagnostics::is_native_windows()
}
unsafe fn prepare_epoch(dd4:*mut c_void,hwnd:Hwnd,client:(u32,u32),
    output_client:(u32,u32),mode:Mode,generation:u64,native_3d:bool)->Result<Epoch,String>{
    if client.0==0 || client.1==0 || output_client.0==0 || output_client.1==0{
        return Err("invalid render or logical output raster".into());
    }
    log(&format!("WINDOWED-DIAG: physical_render={}x{} logical_blt_output={}x{} (DPI-safe presentation)",
        client.0,client.1,output_client.0,output_client.1));
    let movie_rgb32=native_movie_rgb32_enabled(mode) && !native_3d;
    if movie_rgb32 {
        log("NATIVE-MOVIE-RGB32: opt-in physical XRGB8888 backbuffer requested for 800x600 movie; virtual primary remains RGB565 CPU staging, desktop unchanged");
    }
    log("WINDOWED-DIAG: native primary CreateSurface begin");
    let real_primary=alloc(dd4,desc(DD_CAPS_PRIMARY,0,0,None))?;
    log("WINDOWED-DIAG: native primary CreateSurface OK");
    log("WINDOWED-DIAG: native clipper.CreateClipper/SetHWnd/SetClipper begin");
    let clipper=new_clipper(dd4,hwnd,&real_primary)?;
    log("WINDOWED-DIAG: native clipper setup OK");
    log(if movie_rgb32{
        "NATIVE-MOVIE-RGB32: native XRGB8888 video-memory D3D backbuffer CreateSurface begin"
    }else{"WINDOWED-DIAG: native RGB565 video-memory D3D backbuffer CreateSurface begin"});
    let real_back=alloc(dd4,desc(DD_CAPS_OFFSCREEN|DD_CAPS_3D|DD_CAPS_VIDEOMEM,
        mode.width,mode.height,Some(if movie_rgb32{32}else{16})))?;
    log(if movie_rgb32{
        "NATIVE-MOVIE-RGB32: native XRGB8888 D3D backbuffer CreateSurface OK"
    }else{"WINDOWED-DIAG: native RGB565 D3D backbuffer CreateSurface OK"});
    // The real owner Wine HAL advertises pure 32-bit depth with 24 bits
    // of active Z (0x00ffffff); a native failure is never masked.
    log("WINDOWED-DIAG: native Z32 depth buffer CreateSurface begin");
    let real_depth=alloc(dd4,desc(DD_CAPS_Z|DD_CAPS_VIDEOMEM,
        mode.width,mode.height,Some(32)))
        .or_else(|_|alloc(dd4,desc(DD_CAPS_Z,mode.width,mode.height,Some(32))))?;
    log("WINDOWED-DIAG: native Z32 depth buffer CreateSurface OK");
    log("WINDOWED-DIAG: validate native RGB565 or movie XRGB8888 + Z32 formats begin");
    check_native_rgb_depth(&real_back,&real_depth,mode,movie_rgb32)?;
    log("WINDOWED-DIAG: native color/Z32 formats OK; attach Z32 begin");
    attach_depth(&real_back,&real_depth)?;
    log("WINDOWED-DIAG: attach Z32 OK; depth-fill begin");
    native_fill(&real_depth,true)?;
    log("WINDOWED-DIAG: depth-fill OK; color-fill begin");
    native_fill(&real_back,false)?;
    log("WINDOWED-DIAG: color-fill OK; native D3D3 HAL CreateDevice probe begin");
    prove_hal(dd4,&real_back)?;
    log("WINDOWED-DIAG: native D3D3 HAL CreateDevice probe OK");
    log("WINDOWED-DIAG: native RGB565 system-memory primary Lock staging CreateSurface begin");
    let primary_lock=alloc(dd4,desc(DD_CAPS_OFFSCREEN|DD_CAPS_SYSTEMMEM,
        mode.width,mode.height,Some(16)))?;
    log("WINDOWED-DIAG: native primary Lock staging CreateSurface OK");
    // Menu logo is composited on a PRIVATE RGB565 surface and then copied
    // to the physical primary in the same frame. The game's D3D backbuffer
    // remains untouched, even during a playing animated menu background.
    // If unavailable, graphics/gameplay continue without the version label.
    // A 16-bit menu scratch cannot transparently compose on the
    // experimental real 32-bit movie render target; leave the game frame
    // untouched during this A/B instead of risking an invalid Blt.
    if movie_rgb32 {
        log("NATIVE-MOVIE-RGB32: menu branding scratch disabled for this A/B; original game frame preserved");
    }
    let menu_scratch=if !movie_rgb32 &&
        matches!((mode.width,mode.height),(640,480)|(800,600)){
        match alloc(dd4,desc(DD_CAPS_OFFSCREEN|DD_CAPS_SYSTEMMEM,
            mode.width,mode.height,Some(16))) {
            Ok(surface)=>Some(surface),
            Err(error)=>{
                log(&format!("MENU-BRANDING: offscreen scratch unavailable: {error}"));
                None
            }
        }
    }else{None};
    // Windows native DD4 returns E_NOTIMPL for XRGB8888 320x240 ->
    // RGB565 800x600 scaled video copies. Allocate a dedicated SYSTEM
    // memory RGB565 scratch so no CPU lock touches the active D3D3 target.
    // The existing Wine/probe and default nonbootstrap paths stay untouched.
    let movie_rgb565_scratch=if !movie_rgb32 &&
        mode.width==800 && mode.height==600 &&
        crate::cfg().native_dd4_bootstrap && crate::diagnostics::is_native_windows(){
        match alloc(dd4,desc(DD_CAPS_OFFSCREEN|DD_CAPS_SYSTEMMEM,800,600,Some(16))){
            Ok(surface)=>{
                log("NATIVE-MOVIE-BLT: optional RGB565 800x600 system-memory scratch created");
                Some(surface)
            },
            Err(err)=>{
                log(&format!("NATIVE-MOVIE-BLT: optional scratch creation failed: {err}"));
                None
            }
        }
    }else{None};
    log("WINDOWED-DIAG: primary Lock/Unlock staging probe begin");
    verify_primary_lock_staging(&primary_lock,mode)?;
    log("WINDOWED-DIAG: primary Lock/Unlock staging probe OK");
    log(&format!("WINDOWED-GAME: verified logical primary {}x{} RGB565 system-memory Lock/Unlock staging (desktop primary never exposed)",
        mode.width,mode.height));
    log("WINDOWED-DIAG: native→virtual Surface4 graph construction begin");
    let primary=Com::from_out(com_surface4_forward::new_graph_surface4_opt_in(real_primary.ptr(),SurfaceRole::Primary,mode,native_3d)
        .map_err(|hr|format!("virtual primary: {:#010x}",hr as u32))?)?;
    let back=Com::from_out(com_surface4_forward::new_graph_surface4_opt_in_movie_rgb32(
        real_back.ptr(),SurfaceRole::Backbuffer,mode,native_3d,movie_rgb32)
        .map_err(|hr|format!("virtual backbuffer: {:#010x}",hr as u32))?)?;
    let depth=Com::from_out(com_surface4_forward::new_graph_surface4_opt_in(real_depth.ptr(),SurfaceRole::Depth,mode,native_3d)
        .map_err(|hr|format!("virtual depth: {:#010x}",hr as u32))?)?;
    com_surface4_forward::arm_graph_primary_lock_surface(primary.ptr(),primary_lock.ptr())
        .map_err(|hr|format!("arm primary Lock staging: {:#010x}",hr as u32))?;
    if let Some(scratch)=movie_rgb565_scratch.as_ref(){
        match com_surface4_forward::arm_graph_movie_conversion_staging(back.ptr(),scratch.ptr()){
            Ok(())=>log("NATIVE-MOVIE-BLT: scratch validated and bound to virtual RGB565 backbuffer"),
            Err(hr)=>log(&format!("NATIVE-MOVIE-BLT: scratch validation failed hr={:#010x}; original Blt failures preserved",hr as u32))
        }
    }
    com_surface4_forward::attach_graph_surface4(primary.ptr(),back.ptr())
        .map_err(|hr|format!("attach virtual BB {:#010x}",hr as u32))?;
    com_surface4_forward::attach_graph_surface4(back.ptr(),depth.ptr())
        .map_err(|hr|format!("attach virtual Z32 {:#010x}",hr as u32))?;

    let chain=Chain{front_to_back:true,back_to_depth:true,mipmaps:false};
    let mut contract=if native_3d{
        AdapterContract::prepare_native_3d(mode,client,chain)
    }else{
        AdapterContract::prepare(mode,client,chain)
    }.map_err(|e|format!("preflight: {e:?}"))?;
    if native_3d {
        log(&format!(
            "WINDOWED-WIDE: real D3D3 RGB565 color and Z32 surfaces allocated at native monitor {}x{}, viewport full, no 4:3 stretch",mode.width,mode.height));
    }
    // Before exposure, the same physical backend from the standalone
    // probe must perform the actual black-clear + 4:3 stretch pair.
    log("WINDOWED-DIAG: physical windowed Blt backend preparation begin");
    let mut backend=if movie_rgb32 {
        NativeWindowedBlt::from_borrowed_com_native_movie_rgb32(
            real_primary.ptr(),real_back.ptr(),(0,0),client,mode
        )
    }else{
        NativeWindowedBlt::from_borrowed_com(
            real_primary.ptr(),real_back.ptr(),(0,0),client,mode
        )
    }.map_err(|hr|format!("native Blt backend: {:#010x}",hr as u32))?;
    backend.set_output_client(output_client)
        .map_err(|hr|format!("logical output coordinates: {:#010x}",hr as u32))?;
    let plan=contract.plan.viewport;
    log("WINDOWED-DIAG: physical windowed two-Blt preflight begin");
    borderless_flip::execute_windowed_blt_pair(&mut backend,mode,client,plan)
        .map_err(|e|format!("native two-Blt preflight: {e:?}"))?;
    log("WINDOWED-DIAG: physical preflight OK; validate virtual graph evidence begin");
    let evidence=BackendEvidence{
        process_owned_window:true,normal_directdraw:true,
        render_target:RenderTarget::RealDirectDrawSurface4,
        render_target_caps:DD_CAPS_3D,render_target_is_rgb565:!movie_rgb32,
        real_zbuffer_32:true,primary_with_clipper:true,
        working_blt:true,complete_surface_com:true
    };
    if movie_rgb32 {
        // check_native_rgb_depth() above has already validated the
        // *exact* real XRGB8888 masks. This separate diagnostic gate does
        // not relax the standard Wine/Proton RGB565 contract.
        contract.expose_native_movie_rgb32(evidence)
    }else{contract.expose(evidence)}
        .map_err(|e|format!("graph evidence: {e:?}"))?;
    log("WINDOWED-DIAG: evidence OK; arm virtual primary Flip begin");
    com_surface4_forward::arm_graph_primary_flip_with_output_movie_rgb32(
        primary.ptr(),&contract,generation,(0,0),client,output_client,Some(crate::cfg().fps_limit),
        hwnd as usize,crate::cfg().focus_repaint,
        menu_scratch.as_ref().map(|s|s.ptr()).unwrap_or(ptr::null_mut()),
        // Native Windows 125% DPI screenshot A/B only: no global DPI,
        // pointer or WndProc changes and never activate on Wine/Proton.
        crate::cfg().native_gdi_sidebars &&
            crate::cfg().native_dd4_bootstrap &&
            crate::diagnostics::is_native_windows(),
        movie_rgb32
    ).map_err(|hr|format!("arm graph: {:#010x}",hr as u32))?;
    if movie_rgb32 {
        log("NATIVE-MOVIE-RGB32: preflight passed; virtual 800x600 movie graph uses real XRGB8888 direct Blt; no RGB565 movie-conversion scratch");
        if crate::cfg().native_movie_message_pump {
            crate::window_erase_guard::arm_native_movie_pump(hwnd as usize)
                .map_err(|reason|format!("native movie message pump arm refused: {reason}"))?;
            log("NATIVE-MOVIE-PUMP: opt-in armed for exact verified game HWND; max 4 queued messages per virtual Flip, outside presentation lock; no Wine/Proton use");
        }
    }
    Ok(Epoch{mode,primary,back,depth,physical_primary:real_primary,
        physical_lock_primary:primary_lock,physical_back:real_back,
        physical_depth:real_depth,menu_scratch,movie_rgb565_scratch,clipper})
}
#[cfg(test)]
mod native_intro_exclusive_tests {
    use super::*;

    #[test]
    fn requires_800x600_then_a_distinct_mode_before_arming() {
        let video=Mode{width:800,height:600,bpp:16};
        let game=Mode{width:1920,height:1200,bpp:16};
        let boot=Mode{width:1024,height:768,bpp:16};
        assert_eq!(next_intro_phase(INTRO_BEFORE_MOVIE,boot),INTRO_BEFORE_MOVIE);
        assert_eq!(next_intro_phase(INTRO_BEFORE_MOVIE,video),INTRO_IN_MOVIE_MODE);
        assert_eq!(next_intro_phase(INTRO_IN_MOVIE_MODE,video),INTRO_IN_MOVIE_MODE);
        assert_eq!(next_intro_phase(INTRO_IN_MOVIE_MODE,boot),INTRO_LEFT_MOVIE_MODE);
        assert_eq!(next_intro_phase(INTRO_LEFT_MOVIE_MODE,video),INTRO_LEFT_MOVIE_MODE);
        assert_eq!(next_intro_phase(INTRO_LEFT_MOVIE_MODE,game),INTRO_LEFT_MOVIE_MODE);
    }

    #[test]
    fn legacy_mode_must_not_trigger_pending_native_widescreen_handoff() {
        let target=(1920,1200);
        let legacy=Mode{width:1024,height:768,bpp:16};
        let menu=Mode{width:800,height:600,bpp:16};
        let native=Mode{width:1920,height:1200,bpp:16};
        assert!(!native_intro_requested_3d(legacy,target));
        assert!(!native_intro_requested_3d(menu,target));
        assert!(native_intro_requested_3d(native,target));
        assert_eq!(next_intro_phase(INTRO_IN_MOVIE_MODE,legacy),INTRO_LEFT_MOVIE_MODE);
        assert!(!native_intro_ready(INTRO_LEFT_MOVIE_MODE,true));
    }
    #[test]
    fn unpatched_native_intro_must_never_fall_back_with_widescreen_bytes() {
        let target=(1920,1200);
        let game=Mode{width:1920,height:1200,bpp:16};
        let legacy=Mode{width:1024,height:768,bpp:16};
        let menu=Mode{width:800,height:600,bpp:16};
        assert!(native_intro_requested_3d(game,target));
        assert!(!native_intro_requested_3d(legacy,target));
        assert!(!native_intro_requested_3d(menu,target));
        assert!(!native_intro_can_prepare(legacy,target,false,false));
        assert!(!native_intro_can_prepare(menu,target,false,false));
        assert!(native_intro_can_prepare(game,target,false,false));
        // Once a virtual graph is exposed, original 4:3 menu modes
        // remain legal and full native mode needs a committed wide patch.
        assert!(native_intro_can_prepare(menu,target,true,true));
        assert!(!native_intro_can_prepare(game,target,true,false));
        assert!(native_intro_can_prepare(game,target,true,true));
    }

    #[test]
    fn launch_physical_aspect_is_checked_without_accepting_exclusive_modes() {
        assert!(same_aspect((1536,960),(1920,1200)));
        assert!(same_aspect((1093,614),(1366,768)));
        assert!(same_aspect((1536,961),(1920,1200)));
        assert!(!same_aspect((1536,963),(1920,1200)));
        assert!(same_aspect((1920,1200),(1920,1200)));
        assert!(!same_aspect((1536,960),(1024,768)));
        assert!(!same_aspect((1536,960),(800,600)));
        assert!(!same_aspect((0,960),(1920,1200)));
        assert!(!same_aspect((1536,960),(0,1200)));
        assert!(!same_aspect((1,0),(1,0)));
    }

    #[test]
    fn only_arms_before_first_postintro_mode_transition_when_main_menu_is_active() {
        assert!(!native_intro_ready(INTRO_BEFORE_MOVIE,false));
        assert!(!native_intro_ready(INTRO_BEFORE_MOVIE,true)); // early boot cannot switch
        assert!(!native_intro_ready(INTRO_IN_MOVIE_MODE,false)); // movie still active
        assert!(native_intro_ready(INTRO_IN_MOVIE_MODE,true));  // first next coop, menu seen
        assert!(!native_intro_ready(INTRO_LEFT_MOVIE_MODE,false));
        assert!(!native_intro_ready(INTRO_LEFT_MOVIE_MODE,true)); // ESC -> menu is too late
        // Conservative mode sequence from owner logs: early 1024,
        // introduction/menu 800, gameplay 1024, ESC returns to 800.
        let intro=next_intro_phase(INTRO_BEFORE_MOVIE,Mode{width:800,height:600,bpp:16});
        assert_eq!(intro,INTRO_IN_MOVIE_MODE);
        assert!(native_intro_ready(intro,true));
        let gameplay=next_intro_phase(intro,Mode{width:1024,height:768,bpp:16});
        assert_eq!(gameplay,INTRO_LEFT_MOVIE_MODE);
        assert!(!native_intro_ready(gameplay,true)); // cannot arm on ESC
    }
}

// Give the game its *logical* DD4 mode, not the desktop's physical mode.
// Crimson Skies requests 640x480x16 (or 800x600x16), while our real
// fullscreen-sized HWND is windowed at the monitor's native resolution.
// Native GetDisplayMode in DDSCL_NORMAL reports the desktop and may cause a
// post-CreateDevice video-initialization check to reject the adapter.
fn logical_display_desc(words:&mut [u32;31], mode:Mode)->bool{
    if words[0]!=124 || mode.bpp!=16{return false;}
    words[1]|=DDSD_HEIGHT|DDSD_WIDTH|DDSD_PIXELFORMAT;
    words[2]=mode.height;
    words[3]=mode.width;
    words[18]=32; // DDPIXELFORMAT.dwSize
    words[19]=0x40; // DDPF_RGB
    words[20]=0;
    words[21]=16;
    words[22]=0xf800;
    words[23]=0x07e0;
    words[24]=0x001f;
    words[25]=0;
    true
}
/// Called only after native IDirectDraw4::GetDisplayMode succeeds.
pub unsafe fn rewrite_display_mode(dd4:*mut c_void,d:*mut c_void)->bool {
    if d.is_null(){return false;}
    // Keep the mode snapshot AND descriptor rewrite in the same epoch.
    // Only the diagnostic file write is moved outside RUNTIME: logging
    // must not prolong a process-wide COM resource mutex hold.
    let (mode,native_w,native_h,native_bpp)={
        let Ok(guard)=RUNTIME.lock() else{return false;};
        let Some(runtime)=guard.as_ref() else{return false;};
        if runtime.dd4!=dd4 as usize{return false;}
        let Some(epoch)=runtime.epoch.as_ref() else{return false;};
        let words=&mut *(d.cast::<[u32;31]>());
        let (native_w,native_h,native_bpp)=(words[3],words[2],words[21]);
        if !logical_display_desc(words,epoch.mode){return false;}
        (epoch.mode,native_w,native_h,native_bpp)
    };
    log(&format!("WINDOWED-GAME: DD4 GetDisplayMode virtualized: physical={native_w}x{native_h}x{native_bpp} logical={}x{}x{} (desktop untouched)",
        mode.width,mode.height,mode.bpp));
    true
}
#[cfg(test)]
mod logical_mode_tests{
    use super::*;
    #[test]
    fn get_display_mode_reports_exact_game_rgb565_without_touching_other_fields(){
        let mut native=[0u32;31];
        native[0]=124;native[1]=0x80001;native[2]=1080;native[3]=1920;
        native[18]=32;native[19]=0x40;native[21]=32;
        native[26]=0x200;native[27]=0xdeadbeef;
        assert!(logical_display_desc(&mut native,Mode{width:640,height:480,bpp:16}));
        assert_eq!((native[2],native[3],native[21]),(480,640,16));
        assert_eq!((native[22],native[23],native[24]),(0xf800,0x07e0,0x001f));
        assert_eq!((native[26],native[27]),(0x200,0xdeadbeef));
        let mut invalid=[0u32;31];invalid[0]=108;
        assert!(!logical_display_desc(&mut invalid,Mode{width:640,height:480,bpp:16}));
        assert_eq!(invalid[0],108);
    }
}

// Confirmed from 2026-10-31 full-frame readback: the game generated
// 800 pixels of 16-bit RGB565 in each scanline, yet the real unmodified
// DDSCL_NORMAL/Wine CreateSurface with no explicit DDPIXELFORMAT built a
// 32-bit system-memory surface. The 800px source therefore occupied
// exactly 400 32-bit pixels, with magenta/green colors when interpreted
// as RGB32. Decoding low 16 bits of each DWORD restores the artwork.
//
// Only the *observed* user-game legacy 800x600 source is eligible.
// Offscreen video frames 320x240 (and textures, depth, render targets,
// other caps/descriptor flags) retain their original native formats.
// Do NOT mutate the game's original desc, and never change the physical
// desktop display mode. This remains default-off pending live test.
const EXACT_LEGACY_IMAGE_FLAGS:u32=0x0011_0007; // CAPS|HEIGHT|WIDTH|CKSRCBLT|TEXTURESTAGE
const EXACT_LEGACY_IMAGE_CAPS:u32=DD_CAPS_OFFSCREEN|DD_CAPS_SYSTEMMEM;

fn legacy_rgb565_descriptor(words:&[u32;31],mode:Mode)->Option<[u32;31]>{
    if mode.bpp!=16 || (mode.width,mode.height)!=(800,600){
        return None;
    }
    if words[0]!=124 || words[1]!=EXACT_LEGACY_IMAGE_FLAGS ||
       words[26]!=EXACT_LEGACY_IMAGE_CAPS ||
       words[2]!=mode.height || words[3]!=mode.width{
        return None;
    }
    // No game-requested pixel format is present (DDSD_PIXELFORMAT unset).
    // Insert exact native 16-bit RGB565 masks, preserving color-key flags
    // and all caller-requested caps and texture-stage data.
    let mut d=*words;
    d[1]|=DDSD_PIXELFORMAT;
    d[18..26].fill(0);
    d[18]=32;   // DDPIXELFORMAT.dwSize
    d[19]=0x40; // DDPF_RGB
    d[21]=16;
    d[22]=0xf800;
    d[23]=0x07e0;
    d[24]=0x001f;
    Some(d)
}

/// Return a modified COPY of the game descriptor for only the matching
/// native DD4 offscreen system-memory image source. Never call Wine here:
/// the existing original DD4 CreateSurface must create and own the object.
pub unsafe fn legacy_offscreen_rgb565(dd4:*mut c_void,desc:*mut c_void,
    outer:*mut c_void)->Option<[u32;31]>{
    if dd4.is_null() || desc.is_null() || !outer.is_null(){return None;}
    let words=desc.cast::<u32>();
    if *words!=124{return None;}
    let Ok(state)=RUNTIME.lock() else{return None};
    let runtime=state.as_ref()?;
    if runtime.dd4!=dd4 as usize || !runtime.exposed{return None;}
    let epoch=runtime.epoch.as_ref()?;
    let input=&*(words as *const [u32;31]);
    legacy_rgb565_descriptor(input,epoch.mode)
}

#[cfg(test)]
mod legacy_source_tests{
    use super::*;
    fn source_desc(width:u32,height:u32)->[u32;31]{
        let mut d=[0u32;31];
        d[0]=124;
        d[1]=EXACT_LEGACY_IMAGE_FLAGS;
        d[2]=height;d[3]=width;
        d[26]=EXACT_LEGACY_IMAGE_CAPS;
        // The game did not request explicit PF; preserve existing payload.
        d[27]=0x12345678;
        d
    }
    #[test]
    fn exact_game_rgb565_16bit_image_source_is_only_one_selected(){
        for (w,h) in [(800,600)]{
            let d=source_desc(w,h);
            let changed=legacy_rgb565_descriptor(&d,Mode{width:w,height:h,bpp:16}).unwrap();
            assert_eq!(d[1],EXACT_LEGACY_IMAGE_FLAGS); // no mutation
            assert_eq!(changed[1],EXACT_LEGACY_IMAGE_FLAGS|DDSD_PIXELFORMAT);
            assert_eq!((changed[18],changed[19],changed[21]),(32,0x40,16));
            assert_eq!((changed[22],changed[23],changed[24],changed[25]),
                       (0xf800,0x07e0,0x001f,0));
            assert_eq!((changed[2],changed[3],changed[26],changed[27]),
                       (h,w,EXACT_LEGACY_IMAGE_CAPS,0x12345678));
        }
    }
    #[test]
    fn refuses_other_formats_video_and_unsupported_surface_descriptors(){
        let mode=Mode{width:800,height:600,bpp:16};
        assert!(legacy_rgb565_descriptor(&source_desc(320,240),mode).is_none());
        assert!(legacy_rgb565_descriptor(&source_desc(640,480),Mode{width:640,height:480,bpp:16}).is_none());
        assert!(legacy_rgb565_descriptor(&source_desc(800,600),
            Mode{width:800,height:600,bpp:32}).is_none());
        let baseline=source_desc(800,600);
        for (slot,value) in [(0,108),(1,EXACT_LEGACY_IMAGE_FLAGS|DDSD_PIXELFORMAT),
                             (1,EXACT_LEGACY_IMAGE_FLAGS&!0x10000),
                             (26,DD_CAPS_OFFSCREEN|DD_CAPS_VIDEOMEM),
                             (26,DD_CAPS_SYSTEMMEM)]{
            let mut other=baseline;other[slot]=value;
            assert!(legacy_rgb565_descriptor(&other,mode).is_none(),
                    "bad descriptor unexpectedly matched slot {slot}");
        }
    }
}

/// Called before the native DD4 CreateSurface. Only replace the exact
/// one-backbuffer primary (or explicit matching Z32) on the game DD4.
pub unsafe fn create_surface(dd4:*mut c_void,d:*mut c_void,out:*mut *mut c_void,
    outer:*mut c_void)->Option<Hr>{
    if d.is_null()||out.is_null()||!outer.is_null(){return None;}
    let words=d.cast::<u32>();
    if *words!=124{return None;}
    let caps=*words.add(26);
    let is_primary=caps&DD_CAPS_PRIMARY!=0;
    let is_z=caps&DD_CAPS_Z!=0;
    if !is_primary && !is_z{return None;}
    let Ok(mut state)=RUNTIME.lock() else{return Some(E_FAIL)};
    let Some(runtime)=state.as_mut() else{return None};
    if runtime.dd4!=dd4 as usize{return None;}
    // Native HAL validation may internally create PRIMARY or Z surfaces.
    // These are driver-owned requests, not the game's post-negotiation
    // request for a virtual surface. Forward them unchanged while the
    // adapter's real HAL graph is still being constructed.
    if runtime.native_preparing && !runtime.exposed{return None;}
    // COM output ownership is transferred only on S_OK. On any refusal,
    // never accidentally return the caller's previous surface pointer.
    *out=ptr::null_mut();
    let Some(epoch)=runtime.epoch.as_ref() else{return Some(E_FAIL)};
    let requested=if is_primary{
        let flags=*words.add(1);
        let count=if flags&DDSD_BACKBUFFERCOUNT!=0{*words.add(5)}else{0};
        if count!=1 || caps&DD_CAPS_Z!=0{
            log("WINDOWED-GAME: unexpected primary surface caps or backbuffer count, failing closed");
            return Some(E_NOTIMPL);
        }
        &epoch.primary
    }else{
        let flags=*words.add(1);
        // A game-issued Z creation can request a 32-bit depth attachment,
        // but a different-sized or explicit Z16 is incompatible.
        if flags&6==6 && (*words.add(2),*words.add(3))!=(epoch.mode.height,epoch.mode.width){
            log("WINDOWED-GAME: unexpected Z dimensions, failing closed");
            return Some(E_NOTIMPL);
        }
        if flags&DDSD_PIXELFORMAT!=0 && *words.add(21)!=32{
            log("WINDOWED-GAME: unsupported Z pixel format; only native Z32 allowed");
            return Some(E_NOTIMPL);
        }
        &epoch.depth
    };
    if let Err(err)=requested.add_ref(){
        log(&format!("WINDOWED-GAME: game surface COM AddRef failed: {err}"));
        return Some(E_FAIL);
    }
    *out=requested.ptr();
    runtime.exposed=true;
    log(if is_primary{
        "WINDOWED-GAME: game now owns verified virtual Surface4 PRIMARY and one attached RGB565 backbuffer"
    }else{
        "WINDOWED-GAME: game now owns verified virtual Surface4 Z32"
    });
    Some(0)
}
/// Other DD2/DD7 SetDisplayMode and cooperative calls cannot silently switch
/// this live graph back to exclusive after the game has seen virtual objects.
pub fn must_block_other_mode_interface()->bool{
    strict_native_nonexclusive() ||
        RUNTIME.lock().ok().and_then(|r|r.as_ref().map(|x|x.exposed)).unwrap_or(false)
}
pub fn must_block_other_cooperative_interface(flags:u32)->bool{
    // Normal cooperation remains allowed before the virtual graph; any
    // *exclusive* request from another DirectDraw interface is forbidden.
    (strict_native_nonexclusive() &&
     flags&DDSCL_EXCLUSIVE!=0) ||
    RUNTIME.lock().ok().and_then(|r|r.as_ref().map(|x|x.exposed)).unwrap_or(false)
}
/// Only valid while the game holds a live virtual backbuffer reference.
/// The native HAL device must always receive the actual RGB565 surface.
pub unsafe fn unwrap_target(candidate:*mut c_void)->Option<*mut c_void>{
    com_surface4_forward::try_borrow_real_surface4(candidate)
}
/// Return an AddRef'd GAME-facing backbuffer when Wine returns the native
/// backing surface for the currently owned D3DDevice3 render target.
pub unsafe fn remap_render_target(native:*mut c_void)->Option<*mut c_void>{
    let Ok(state)=RUNTIME.lock() else{return None};
    let epoch=state.as_ref()?.epoch.as_ref()?;
    if epoch.physical_back.ptr()!=native{return None;}
    epoch.back.add_ref().ok()?;
    Some(epoch.back.ptr())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn strict_native_borderless_never_allows_original_exclusive_fallback(){
        assert!(strict_nonexclusive_selection(true,true,true,true,true,false,false));
        for case in [
            (false,true,true,true,true,false,false),
            (true,false,true,true,true,false,false),
            (true,true,false,true,true,false,false),
            (true,true,true,false,true,false,false),
            (true,true,true,true,false,false,false),
            (true,true,true,true,true,true,false),
            (true,true,true,true,true,false,true)
        ]{
            assert!(!strict_nonexclusive_selection(case.0,case.1,case.2,
                case.3,case.4,case.5,case.6));
        }
    }
    #[test] fn only_identical_exposed_game_cooperative_request_is_idempotent(){
        let original_dd4=0x1234usize;
        let hwnd=0x4321usize;
        let accept=|dd4,window,flags,exposed,epoch|
            identical_game_cooperative_request(original_dd4,hwnd,exposed,epoch,
                dd4,window,flags);
        assert!(accept(original_dd4,hwnd,0x13,true,true));
        assert!(!accept(original_dd4,hwnd,0x13,false,true));
        assert!(!accept(original_dd4,hwnd,0x13,true,false));
        assert!(!accept(original_dd4+1,hwnd,0x13,true,true));
        assert!(!accept(original_dd4,hwnd+1,0x13,true,true));
        assert!(!accept(original_dd4,0,0x13,true,true));
        assert!(!accept(original_dd4,hwnd,0x8,true,true));
        assert!(!accept(original_dd4,hwnd,0x33,true,true));
        assert!(!accept(original_dd4,hwnd,0,true,true));
    }
    #[test]
    fn native_movie_rgb32_mode_is_strictly_800x600x16(){
        for (w,h,bpp,accepted) in [
            (800,600,16,true), (640,480,16,false),
            (1024,768,16,false), (1920,1200,16,false),
            (800,600,32,false)
        ]{
            assert_eq!(native_movie_rgb32_mode(Mode{width:w,height:h,bpp}),accepted);
        }
    }
    #[test] fn descriptors_remain_native_124_byte_surface4(){
        let p=desc(DD_CAPS_PRIMARY,0,0,None);
        assert_eq!((p[0],p[1],p[26]),(124,1,0x200));
        let z=desc(DD_CAPS_Z|DD_CAPS_VIDEOMEM,640,480,Some(32));
        assert_eq!((z[0],z[2],z[3],z[19],z[21],z[23]),(124,480,640,0x400,32,0x00ff_ffff));
        let b=desc(DD_CAPS_OFFSCREEN|DD_CAPS_3D|DD_CAPS_VIDEOMEM,800,600,Some(16));
        assert_eq!((b[2],b[3],b[21],b[22],b[23],b[24]),(600,800,16,0xf800,0x07e0,0x001f));
        let movie=desc(DD_CAPS_OFFSCREEN|DD_CAPS_3D|DD_CAPS_VIDEOMEM,800,600,Some(32));
        assert_eq!((movie[2],movie[3],movie[19],movie[21],movie[22],movie[23],movie[24]),
            (600,800,0x40,32,0xff0000,0xff00,0xff));
    }
}
#[cfg(test)]
mod native_com_ownership_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32,Ordering};

    #[repr(C)]
    struct NativeMock {
        vtable:*const usize,
        refs:AtomicU32,
        add_refs:AtomicU32,
        releases:AtomicU32,
    }
    unsafe extern "system" fn add_ref(this:*mut c_void)->u32 {
        let object=&*this.cast::<NativeMock>();
        object.add_refs.fetch_add(1,Ordering::SeqCst);
        object.refs.fetch_add(1,Ordering::SeqCst)+1
    }
    unsafe extern "system" fn release(this:*mut c_void)->u32 {
        let object=&*this.cast::<NativeMock>();
        object.releases.fetch_add(1,Ordering::SeqCst);
        object.refs.fetch_sub(1,Ordering::SeqCst)-1
    }

    #[test]
    fn native_com_owner_releases_exactly_one_borrowed_output_ref() {
        let slots=[0usize,add_ref as *const () as usize,release as *const () as usize];
        let mut obj=NativeMock{
            vtable:slots.as_ptr(),refs:AtomicU32::new(1),
            add_refs:AtomicU32::new(0),releases:AtomicU32::new(0),
        };
        {
            let owner=unsafe {Com::from_out((&mut obj as *mut NativeMock).cast()).unwrap()};
            assert_eq!(owner.ptr(),(&mut obj as *mut NativeMock).cast());
            assert_eq!(obj.refs.load(Ordering::SeqCst),1);
        }
        assert_eq!(obj.refs.load(Ordering::SeqCst),0);
        assert_eq!(obj.add_refs.load(Ordering::SeqCst),0);
        assert_eq!(obj.releases.load(Ordering::SeqCst),1);
        assert!(unsafe {Com::from_out(ptr::null_mut())}.is_err());
    }

    #[test]
    fn game_facing_addref_outlives_adapters_own_strong_reference() {
        let slots=[0usize,add_ref as *const () as usize,release as *const () as usize];
        let mut obj=NativeMock{
            vtable:slots.as_ptr(),refs:AtomicU32::new(1),
            add_refs:AtomicU32::new(0),releases:AtomicU32::new(0),
        };
        let raw=(&mut obj as *mut NativeMock).cast();
        let owner=unsafe{Com::from_out(raw).unwrap()};
        unsafe{owner.add_ref().unwrap();}
        assert_eq!(obj.refs.load(Ordering::SeqCst),2);
        drop(owner);
        assert_eq!(obj.refs.load(Ordering::SeqCst),1);
        assert_eq!(obj.releases.load(Ordering::SeqCst),1);
        assert_eq!(unsafe{release(raw)},0);
        assert_eq!(obj.refs.load(Ordering::SeqCst),0);
        assert_eq!(obj.add_refs.load(Ordering::SeqCst),1);
        assert_eq!(obj.releases.load(Ordering::SeqCst),2);
    }
}
