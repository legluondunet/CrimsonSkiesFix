//! Stage B4e: complete *forwarding* IDirectDrawSurface4 COM identity.
//!
//! Inactive in gameplay. Provides 45 ABI-exact Surface4 functions over
//! ONE real DirectDraw4 surface, with independent IUnknown identity,
//! thread-safe COM references and exactly one owned real COM reference.
//! Every non-IUnknown method forwards to the real Surface4 vtable.
//! This is NOT yet the game's virtual primary: GetAttachedSurface, Flip,
//! caps and descriptors still describe the ORIGINAL real surface. A later
//! verified topology adapter must selectively replace those semantics.
//!
//! SAFETY: the caller must pass a live real Surface4 pointer, retaining
//! one reference when constructing the façade. The source interface
//! must be the exact Win32/x86 IDirectDrawSurface4 ABI.

#![allow(dead_code)]
use std::{
    ffi::c_void,
    ptr,
    sync::{Mutex,OnceLock,TryLockError,atomic::{AtomicBool,AtomicPtr,AtomicU32,AtomicUsize,Ordering}},
    time::{Duration,Instant},
};
use crate::{
    Guid,
    com_surface4_abi::{Rect,Surface4VTable,SurfaceThis},
    borderless_contract::{AdapterContract,SurfaceRole},
    borderless_flip::{FlipBridgeError,FlipToBltBridge},
    borderless_native_blt::NativeWindowedBlt,
    presentation::Mode,
    window_exposure::{self,GameVisibility},
    window_erase_guard,
};
const S_OK:i32=0;
const E_NOINTERFACE:i32=0x8000_4002u32 as i32;
const E_POINTER:i32=0x8000_4003u32 as i32;
const E_FAIL:i32=0x8000_4005u32 as i32;
const E_NOTIMPL:i32=0x8000_4001u32 as i32;
const E_INVALIDARG:i32=0x8007_0057u32 as i32;
// Exact DirectDraw4 Win32 fill-only Blt flag subset. We deliberately do
// not emulate game texture copies, overlapping Blts or 2D primary Blts.
// Bounded diagnostic for the game's first rejected virtual COM operation.
// A flood of per-frame errors must not hide the failure that prevented init.
static VIRTUAL_DIAG_COUNT:AtomicUsize=AtomicUsize::new(0);
static GRAPH_COPY_COUNT:AtomicUsize=AtomicUsize::new(0);
static GRAPH_COPY_FAILURE_DETAILS:AtomicUsize=AtomicUsize::new(0);
static GRAPH_MOVIE_CONVERT_SUCCESS:AtomicUsize=AtomicUsize::new(0);
static GRAPH_MOVIE_CONVERT_FAILED:AtomicUsize=AtomicUsize::new(0);
static GRAPH_FLIP_COUNT:AtomicUsize=AtomicUsize::new(0);
// Native-only, opt-in RGB32 video diagnostics: enter/leave trace on the
// real DirectDraw4 surface calls. No COM/DirectShow return values altered.
// The prior samples ended after successful 120+ Flips, but sampling every
// 120 calls cannot distinguish a blocked in-flight Blt from missing input.
static NATIVE_MOVIE_RGB32_BLT_CALLS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MOVIE_RGB32_FLIP_CALLS:AtomicUsize=AtomicUsize::new(0);
// Diagnostic-only: elapsed duration from the first physically presented
// 800x600 opening image to the first decoded 320x240 movie copy.
// A successful Blt alone does not prove the owner saw a static image.
static OPENING_FIRST_PRESENT:OnceLock<Instant>=OnceLock::new();
static NATIVE_MOVIE_RGB32_BLT_STATUS_CALLS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MOVIE_RGB32_FLIP_STATUS_CALLS:AtomicUsize=AtomicUsize::new(0);
// UI/soft-cursor investigation: the graph intentionally rejects certain
// 2D methods. Count actual attempts so that unsupported cursor drawing
// can be distinguished from coordinate / OS cursor visibility problems.
static NATIVE_MENU_PRIMARY_BLT_ATTEMPTS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MENU_BLTFAST_ATTEMPTS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MENU_GETDC_ATTEMPTS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MENU_RELEASEDC_ATTEMPTS:AtomicUsize=AtomicUsize::new(0);
// Infer opt-in from the actual verified virtual graph, never from global
// configuration. This file is also compiled by the standalone x86 test
// probe binary, which intentionally has no crate::cfg/diagnostics functions.
fn native_menu_cursor_probe_enabled(this:SurfaceThis)->bool{
    unsafe {
        let Some(node)=graph_node(this) else{return false;};
        match node.role {
            SurfaceRole::Backbuffer=>node.native_movie_rgb32,
            SurfaceRole::Primary=>{
                let back=(*this.cast::<Surface4Forwarder>())
                    .attached.load(Ordering::Acquire);
                graph_node(back).is_some_and(|b|b.role==SurfaceRole::Backbuffer &&
                    b.native_movie_rgb32)
            },
            SurfaceRole::Depth=>false
        }
    }
}
fn native_menu_trace_count(count:usize)->bool{count<=12 || count%120==0}

fn movie_com_trace_selected(count:usize)->bool{
    count<=256 || count%64==0
}
fn movie_com_trace(event:&str,count:usize,result:Option<i32>,elapsed_ms:Option<u128>){
    if !movie_com_trace_selected(count){return;}
    crate::log(&format!(
        "NATIVE-MOVIE-COM: {event} count={count} thread={:?} result={} elapsed_ms={}",
        std::thread::current().id(),
        result.map(|hr|format!("{:#010x}",hr as u32)).unwrap_or_else(||"ENTER".into()),
        elapsed_ms.map(|ms|ms.to_string()).unwrap_or_else(||"pending".into())
    ));
}
fn virtual_issue(role:SurfaceRole,method:&str,hr:i32){
    if VIRTUAL_DIAG_COUNT.fetch_add(1,Ordering::Relaxed)<40 {
        crate::log(&format!("WINDOWED-GAME: virtual Surface4 {method} role={role:?} result={:#010x}",hr as u32));
    }
}
const DDBLT_COLORFILL:u32=0x0000_0400;
const DDBLT_DEPTHFILL:u32=0x0200_0000;
const DDBLT_WAIT:u32=0x0100_0000;
const DDBLT_ASYNC:u32=0x0000_0200;
// The exact original static image's RGB565->XRGB8888 native DD4 Blt returns
// DD_OK while copying only row zero, even with ASYNC removed. The verified
// fallback converts into a private XRGB8888 system-memory surface, then
// performs a same-format native Blt. No sleep, global patch or host OS test.
unsafe fn is_native_opening_image_surface(source:SurfaceThis)->bool{
    if source.is_null(){return false;}
    let mut desc=[0u32;31];desc[0]=124;
    let hr=(real_vtable(source).get_surface_desc)(source,desc.as_mut_ptr().cast());
    hr>=0 && desc[0]==124 && desc[3]==800 && desc[2]==600 &&
        desc[19]&0x40!=0 && desc[21]==16 &&
        (desc[22],desc[23],desc[24])==(0xf800,0x07e0,0x001f)
}
// The graph's front buffer is SYNTHETIC and synchronously presents by
// completing its real windowed Blt pair. GetFlipStatus MUST describe that
// exact virtual operation, not the non-flippable real desktop primary.
const DDGFS_CANFLIP:u32=0x1;
const DDGFS_ISFLIPDONE:u32=0x2;
const DDERR_WASSTILLDRAWING:i32=0x8876_021cu32 as i32;
const IID_IUNKNOWN:Guid=Guid{
    d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]
};
const IID_SURFACE4:Guid=Guid{
    d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
    d4:[0x8e,0xa6,0x00,0x60,0x97,0x97,0xea,0x5b]
};
fn same_guid(a:&Guid,b:&Guid)->bool{
    a.d1==b.d1&&a.d2==b.d2&&a.d3==b.d3&&a.d4==b.d4
}
#[derive(Clone,Copy)]
struct VirtualGraphNode {
    role:SurfaceRole, mode:Mode,
    // Exact native Windows 800x600 movie A/B only. Other graph formats
    // and Wine/Proton remain RGB565; depth remains Z32 as before.
    native_movie_rgb32:bool,
}
impl VirtualGraphNode {
    fn caps(self)->u32{match self.role{
        SurfaceRole::Primary=>0x1000_e238,
        SurfaceRole::Backbuffer=>0x1000_601c,
        SurfaceRole::Depth=>0x1002_4000,
    }}
    fn bpp(self)->u32{
        if self.role==SurfaceRole::Depth ||
           (self.role==SurfaceRole::Backbuffer && self.native_movie_rgb32) {
            32
        }else{16}
    }
    fn count(self)->u32{if self.role==SurfaceRole::Primary{1}else{0}}
}
/// Probe-only eligible native presentation session. The wrapper's
/// primary->backbuffer->depth COM strong edges own the physical resources.
/// The full three-object chain and native HWND/clipper MUST be verified
/// before this is armed. There is deliberately no active game runtime caller.
#[derive(Default)]
struct ExposureState {
    background_seen:bool,
    update_region_seen:bool,
}
impl ExposureState {
    // A one-shot restore is needed after a known foreground loss or when
    // the Win32 update region becomes pending. A continuously invalid
    // window must NOT provoke an erase+draw on every Flip (v0.1.50 flicker).
    fn sample(&mut self,snapshot:Option<GameVisibility>)->bool {
        let Some(visibility)=snapshot else{return false;};
        let appeared=visibility.pending_paint && !self.update_region_seen;
        self.update_region_seen=visibility.pending_paint;
        if !visibility.foreground{
            self.background_seen=true;
            return false;
        }
        let restore=self.background_seen || appeared;
        self.background_seen=false;
        restore
    }
}
// Passive bounded diagnostic of intervals between *successful game-facing
// virtual Surface4::Flip completions*. This measures the CPU submission
// cadence, NOT the moment a frame reaches the physical display or compositor.
// Leave completely unallocated/disabled by default. No per-frame file writes.
const FRAME_TIME_WINDOW:usize=120;
struct FrameTimeWindow{
    previous:Option<Instant>,
    intervals_us:[u32;FRAME_TIME_WINDOW],
    present_call_us:[u32;FRAME_TIME_WINDOW],
    filled:usize,
    pauses:u32,
}
impl Default for FrameTimeWindow{
    fn default()->Self{
        Self{previous:None,intervals_us:[0;FRAME_TIME_WINDOW],
            present_call_us:[0;FRAME_TIME_WINDOW],filled:0,pauses:0}
    }
}
impl FrameTimeWindow{
    fn observe(&mut self,done_at:Instant,presentation_duration:Duration,
        mode:Mode,fps:Option<u32>)->Option<String>{
        let Some(previous)=self.previous.replace(done_at) else{return None;};
        let elapsed=done_at.saturating_duration_since(previous);
        // Focus loss, loading screens and actual pauses are reported
        // separately rather than mislabeled as ordinary frame jitter.
        if elapsed>=Duration::from_millis(500){
            self.pauses=self.pauses.saturating_add(1);
            return None;
        }
        let us=elapsed.as_micros().min(u128::from(u32::MAX)) as u32;
        if us==0{return None;}
        self.intervals_us[self.filled]=us;
        self.present_call_us[self.filled]=
            presentation_duration.as_micros().min(u128::from(u32::MAX)) as u32;
        self.filled+=1;
        if self.filled<FRAME_TIME_WINDOW{return None;}
        let mut sorted=self.intervals_us;
        sorted.sort_unstable();
        let mut native_sorted=self.present_call_us;
        native_sorted.sort_unstable();
        let p50=sorted[FRAME_TIME_WINDOW/2-1];
        let p95=sorted[(FRAME_TIME_WINDOW*95+99)/100-1];
        let p99=sorted[(FRAME_TIME_WINDOW*99+99)/100-1];
        let worst=sorted[FRAME_TIME_WINDOW-1];
        let present95=native_sorted[(FRAME_TIME_WINDOW*95+99)/100-1];
        let presentmax=native_sorted[FRAME_TIME_WINDOW-1];
        let target_us=1_000_000/fps.unwrap_or(60).max(1);
        let slow2=self.intervals_us.iter()
            .filter(|&&v|v>target_us.saturating_mul(2)).count();
        let slow3=self.intervals_us.iter()
            .filter(|&&v|v>target_us.saturating_mul(3)).count();
        let sum:u64=self.intervals_us.iter().map(|&v|u64::from(v)).sum();
        let avg_us=sum/FRAME_TIME_WINDOW as u64;
        let pauses=self.pauses;
        self.filled=0;
        self.pauses=0;
        Some(format!(
            "FRAMETIME: mode={}x{}x{} samples={} target_fps={} avg_ms={:.3} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3} slow_2x={} slow_3x={} ignored_pauses={} present_call_p95_ms={:.3} present_call_max_ms={:.3} [successful virtual Flip completion spacing, NOT display/present timestamps]",
            mode.width,mode.height,mode.bpp,FRAME_TIME_WINDOW,
            fps.unwrap_or(60),avg_us as f64/1000.0,p50 as f64/1000.0,
            p95 as f64/1000.0,p99 as f64/1000.0,worst as f64/1000.0,
            slow2,slow3,pauses,present95 as f64/1000.0,
            presentmax as f64/1000.0
        ))
    }
}
struct GraphFlipSession{
    bridge:FlipToBltBridge,
    backend:NativeWindowedBlt,
    epoch:u64,
    mode:Mode,
    fps_limit:Option<u32>, // None in standalone probe and COM unit tests
    last_present:Option<Instant>, // reset whenever the mode epoch changes
    frame_time_window:Option<FrameTimeWindow>, // optional steady-state telemetry, no per-frame logging
    window:usize,
    focus_repaint:bool,
    exposure:ExposureState,
    erase_count_seen:usize,
    paint_count_seen:usize,
}

/// Limit only the *real game* virtual Primary Flip, never intermediate
/// Blts or the standalone probe. Crucial because the existing passive
/// ModernFPSPacing option cannot hook a synthetic Surface4 vtable.
fn pace_graph_flip(session:&mut GraphFlipSession){
    let Some(fps)=session.fps_limit.filter(|v|*v>0) else{return;};
    let interval=Duration::from_nanos(1_000_000_000/u64::from(fps));
    let now=Instant::now();
    if let Some(previous)=session.last_present {
        if let Some(deadline)=previous.checked_add(interval){
            if deadline>now {std::thread::sleep(deadline.duration_since(now));}
        }
    }
    session.last_present=Some(Instant::now());
}
#[repr(C)]
struct Surface4Forwarder{
    vtable:*const Surface4VTable, // COM slot 0, first DWORD
    real:SurfaceThis,             // one underlying owned COM ref
    refs:AtomicU32,               // client-owned references only
    graph:Option<VirtualGraphNode>, // None retains pure passthrough semantics
    attached:AtomicPtr<c_void>,  // one strong child edge when graph is set
    virtual_flip:Mutex<Option<GraphFlipSession>>, // explicitly armed once; default off
    // A 640x480/800x600 RGB565 native staging surface for Primary::Lock.
    // Never return the real 1920x1080 desktop primary's lock pointer to the game.
    primary_lock_surface:AtomicPtr<c_void>, // one owned COM ref if armed
    movie_rgb565_staging:AtomicPtr<c_void>, // native 800x600 system-memory RGB565, one owned COM ref
    opening_rgb32_staging:AtomicPtr<c_void>, // 800x600 system XRGB8888, only exact opening image
    retired:AtomicBool, // permanent tombstone after a resolution epoch ends
}

/// # Safety
/// \`real\` must be live, non-null, IDirectDrawSurface4, not an
/// already-wrapped forwarder. The caller must have a live COM reference
/// on entry; this function obtains its own *independent* real AddRef.
/// A returned pointer must be consumed exactly once with COM Release
/// (plus one Release for every separately acquired client reference).
pub unsafe fn new_forwarding_surface4(real:SurfaceThis)->Result<SurfaceThis,i32>{
    if real.is_null(){return Err(E_POINTER);}
    // COM owners must guarantee underlying pointer lifetime for this
    // AddRef. Afterward the wrapper holds its own strong underlying ref.
    let count=(real_vtable(real).add_ref)(real);
    if count==0{return Err(E_FAIL);}
    let obj=Box::new(Surface4Forwarder{
        vtable:&SURFACE4_FORWARD_VTABLE,
        real,refs:AtomicU32::new(1),
        graph:None,attached:AtomicPtr::new(ptr::null_mut()),
        virtual_flip:Mutex::new(None),primary_lock_surface:AtomicPtr::new(ptr::null_mut()),
        movie_rgb565_staging:AtomicPtr::new(ptr::null_mut()),
        opening_rgb32_staging:AtomicPtr::new(ptr::null_mut()),
        retired:AtomicBool::new(false),
    });
    Ok(Box::into_raw(obj).cast())
}

/// Create a real COM Surface4 wrapper with graph-specific descriptor and
/// attachment behavior. All other unsupported virtual operations still
/// fail closed; this is NOT yet a complete game-facing replacement.
/// Each wrapper holds exactly one strong ref to its own physical surface.
///
/// # Safety
/// real is a live native Surface4 with an existing caller-owned reference.
/// The returned client interface must be released exactly once.
pub unsafe fn new_graph_surface4(
    real:SurfaceThis,role:SurfaceRole,mode:Mode
)->Result<SurfaceThis,i32>{
    new_graph_surface4_opt_in(real,role,mode,false)
}

/// The native 3D graph is created ONLY for a previously validated
/// monitor-exact landscape mode from the fingerprinted patched EXE.
/// Do not use this entrypoint for videos, menus or arbitrary dimensions.
pub unsafe fn new_graph_surface4_opt_in(
    real:SurfaceThis,role:SurfaceRole,mode:Mode,native_3d:bool
)->Result<SurfaceThis,i32>{
    new_graph_surface4_opt_in_movie_rgb32(real,role,mode,native_3d,false)
}
/// Native-only, strict movie surface format A/B. The *physical* RGB32
/// backbuffer remains format-correct for a direct native XRGB source Blt.
/// Game-facing logical mode stays 800x600x16; only this backbuffer reports
/// the actual 32-bit real pixel format to clients of its COM interface.
pub unsafe fn new_graph_surface4_opt_in_movie_rgb32(
    real:SurfaceThis,role:SurfaceRole,mode:Mode,native_3d:bool,
    native_movie_rgb32:bool
)->Result<SurfaceThis,i32>{
    if native_movie_rgb32 &&
       (role!=SurfaceRole::Backbuffer ||
        (mode.width,mode.height,mode.bpp)!=(800,600,16) || native_3d){
        return Err(E_INVALIDARG);
    }
    let legacy=matches!((mode.width,mode.height,mode.bpp),(640,480,16)|(800,600,16));
    let allow_native=native_3d && mode.bpp==16 &&
        mode.width>=960 && mode.height>=600 &&
        mode.width<=8192 && mode.height<=4320 &&
        (mode.width as u64)*(mode.height as u64)<=33_554_432 &&
        (mode.width as u64)*3>=(mode.height as u64)*4 &&
        (mode.width as u64)<=(mode.height as u64)*4;
    if !legacy && !allow_native{return Err(E_INVALIDARG);}
    let wrapped=new_forwarding_surface4(real)?;
    (*wrapped.cast::<Surface4Forwarder>()).graph=Some(VirtualGraphNode{
        role,mode,native_movie_rgb32
    });
    Ok(wrapped)
}

/// Associate an independently owned, mode-matched native RGB565 staging
/// surface with the *virtual* primary for its CPU Lock/Unlock operations.
/// The caller must have verified the staging surface's exact pixel format,
/// size, and functioning native Lock/Unlock BEFORE exposing any virtual COM.
/// It must not be the real desktop primary or the 3D render target.
/// All calls to the game's virtual Surface4::Lock return the staging memory,
/// never physical desktop pixels. The wrapper owns its own native AddRef.
pub unsafe fn arm_graph_primary_lock_surface(
    primary:SurfaceThis,staging:SurfaceThis
)->Result<(),i32>{
    if !is_our_forwarder(primary) || staging.is_null(){return Err(E_INVALIDARG);}
    let p=&*primary.cast::<Surface4Forwarder>();
    let Some(node)=p.graph else{return Err(E_INVALIDARG);};
    if node.role!=SurfaceRole::Primary || p.real==staging ||
       p.retired.load(Ordering::Acquire){return Err(E_INVALIDARG);}
    let vt=real_vtable(staging);
    if (vt.add_ref)(staging)==0{return Err(E_FAIL);}
    if p.primary_lock_surface.compare_exchange(
        ptr::null_mut(),staging,Ordering::AcqRel,Ordering::Acquire).is_err(){
        (vt.release)(staging);
        return Err(E_FAIL);
    }
    Ok(())
}

/// Optional native-only 320x240 XRGB8888 movie -> RGB565 staging buffer.
/// Registered BEFORE exposing the synthetic game graph, on the *backbuffer*.
/// The caller owns the original native reference; the wrapper acquires one
/// independent COM AddRef. Missing staging only means native Blt errors are
/// passed through as before. Never install on the Wine/probe path.
pub unsafe fn arm_graph_movie_conversion_staging(
    back:SurfaceThis,staging:SurfaceThis
)->Result<(),i32>{
    if !is_our_forwarder(back) || staging.is_null(){return Err(E_INVALIDARG);}
    let p=&*back.cast::<Surface4Forwarder>();
    let Some(node)=p.graph else{return Err(E_INVALIDARG)};
    if node.role!=SurfaceRole::Backbuffer ||
       (node.mode.width,node.mode.height,node.mode.bpp)!=(800,600,16) ||
       p.real==staging || p.retired.load(Ordering::Acquire){
        return Err(E_INVALIDARG);
    }
    let vt=real_vtable(staging);
    let mut d=[0u32;31];d[0]=124;
    if (vt.get_surface_desc)(staging,d.as_mut_ptr().cast())<0 ||
       (d[3],d[2],d[21],d[22],d[23],d[24])!=(800,600,16,0xf800,0x07e0,0x001f) ||
       d[26]&0x800==0{
        return Err(E_INVALIDARG);
    }
    if (vt.add_ref)(staging)==0{return Err(E_FAIL);}
    if p.movie_rgb565_staging.compare_exchange(
        ptr::null_mut(),staging,Ordering::AcqRel,Ordering::Acquire).is_err(){
        (vt.release)(staging);
        return Err(E_FAIL);
    }
    Ok(())
}

/// Arm the private XRGB8888 system-memory scratch for only the observed
/// RGB565 opening still image on an actual native XRGB8888 movie backbuffer.
/// This is format- and mode-gated, never keyed to Windows vs Wine.
pub unsafe fn arm_graph_opening_rgb32_staging(
    back:SurfaceThis,staging:SurfaceThis
)->Result<(),i32>{
    if !is_our_forwarder(back) || staging.is_null(){return Err(E_INVALIDARG);}
    let p=&*back.cast::<Surface4Forwarder>();
    let Some(node)=p.graph else{return Err(E_INVALIDARG);};
    if node.role!=SurfaceRole::Backbuffer || !node.native_movie_rgb32 ||
       (node.mode.width,node.mode.height,node.mode.bpp)!=(800,600,16) ||
       p.real==staging || p.retired.load(Ordering::Acquire){
        return Err(E_INVALIDARG);
    }
    let vt=real_vtable(staging);
    let mut d=[0u32;31];d[0]=124;
    if (vt.get_surface_desc)(staging,d.as_mut_ptr().cast())<0 ||
       (d[3],d[2],d[21],d[22],d[23],d[24])!=(800,600,32,0xff0000,0xff00,0xff) ||
       d[26]&0x800==0 || d[26]&0x40==0 {
        return Err(E_INVALIDARG);
    }
    if (vt.add_ref)(staging)==0{return Err(E_FAIL);}
    if p.opening_rgb32_staging.compare_exchange(
        ptr::null_mut(),staging,Ordering::AcqRel,Ordering::Acquire).is_err(){
        (vt.release)(staging);
        return Err(E_FAIL);
    }
    Ok(())
}

/// Add an explicit, strong and acyclic COM attachment:
/// Primary -> Backbuffer -> Depth only. The parent owns the child's
/// attachment reference until final parent Release. Independent returned
/// GetAttachedSurface client refs must always be released by the caller.
///
/// # Safety
/// parent and child must be distinct, live, graph-enabled wrappers
/// constructed by new_graph_surface4, with the same logical mode.
pub unsafe fn attach_graph_surface4(parent:SurfaceThis,child:SurfaceThis)
    ->Result<(),i32>
{
    if parent.is_null()||child.is_null()||parent==child{return Err(E_INVALIDARG);}
    if !is_our_forwarder(parent)||!is_our_forwarder(child){return Err(E_INVALIDARG);}
    let p=&*parent.cast::<Surface4Forwarder>();
    let c=&*child.cast::<Surface4Forwarder>();
    let (Some(p_role),Some(c_role))=(p.graph,c.graph) else{return Err(E_INVALIDARG);};
    let permitted=matches!((p_role.role,c_role.role),
        (SurfaceRole::Primary,SurfaceRole::Backbuffer)|
        (SurfaceRole::Backbuffer,SurfaceRole::Depth));
    if !permitted||p_role.mode!=c_role.mode{return Err(E_INVALIDARG);}
    let child_vt=&**child.cast::<*const Surface4VTable>();
    if (child_vt.add_ref)(child)==0{return Err(E_FAIL);}
    if p.attached.compare_exchange(ptr::null_mut(),child,
            Ordering::AcqRel,Ordering::Acquire).is_err(){
        (child_vt.release)(child);
        return Err(E_FAIL);
    }
    Ok(())
}
unsafe fn is_our_forwarder(candidate:SurfaceThis)->bool{
    !candidate.is_null() &&
    std::ptr::eq(*candidate.cast::<*const Surface4VTable>(),&SURFACE4_FORWARD_VTABLE)
}
unsafe fn graph_node(this:SurfaceThis)->Option<VirtualGraphNode>{
    if this.is_null(){return None;}
    (*this.cast::<Surface4Forwarder>()).graph
}
unsafe fn virtual_pixel_format(node:VirtualGraphNode,d:*mut u32){
    ptr::write_bytes(d,0,8);
    *d=32;
    *d.add(1)=if node.role==SurfaceRole::Depth{0x400}else{0x40};
    *d.add(3)=node.bpp();
    if node.role==SurfaceRole::Depth{
        *d.add(5)=0x00ff_ffff;
    }else if node.native_movie_rgb32 && node.role==SurfaceRole::Backbuffer{
        *d.add(4)=0x00ff_0000;
        *d.add(5)=0x0000_ff00;
        *d.add(6)=0x0000_00ff;
    }else{
        *d.add(4)=0xf800;
        *d.add(5)=0x07e0;
        *d.add(6)=0x001f;
    }
}

/// Enable the virtual-primary COM Flip vtable path on an already built,
/// fully verified, explicit three-object Surface4 graph. Once enabled,
/// IDirectDrawSurface4::Flip maps to *real windowed* black clear + RGB565
/// stretch. An incomplete graph, wrong mode/client size or prior arm
/// is rejected. Virtual Blt and all other incomplete operations remain
/// fail-closed. This is not installed in the game.
///
/// The contract's Phase::Exposed is a required proof gate from the caller:
/// NEVER manufacture BackendEvidence before physical DirectDraw4, Z32,
/// D3D3 HAL and clipping checks actually succeed.
///
/// # Safety
/// primary must be a live graph-enabled COM Surface4 primary with its
/// backbuffer/depth children attached. The caller ensures the underlying
/// real primary is clipped to the HWND, all physical pointers are valid,
/// and desktop origin and client dimensions describe that same HWND.
/// The graph must outlive every in-flight COM method invocation.
pub unsafe fn arm_graph_primary_flip(
    primary:SurfaceThis,contract:&AdapterContract,generation:u64,
    origin:(i32,i32),client:(u32,u32),fps_limit:Option<u32>,
    window:usize,focus_repaint:bool,menu_scratch:*mut c_void
)->Result<(),i32>{
    arm_graph_primary_flip_with_output(
        primary,contract,generation,origin,client,client,fps_limit,
        window,focus_repaint,menu_scratch)
}
/// The native Windows game can render at physical resolution while the
/// windowed DirectDraw primary accepts DPI-logical rectangle coordinates.
/// This explicit variant changes ONLY output Blt rectangles; the virtual
/// graph, mode, COM ownership and frame semantics stay physical. The
/// original helper above retains existing Wine/probe behavior.
pub unsafe fn arm_graph_primary_flip_with_output(
    primary:SurfaceThis,contract:&AdapterContract,generation:u64,
    origin:(i32,i32),client:(u32,u32),output_client:(u32,u32),
    fps_limit:Option<u32>,window:usize,focus_repaint:bool,
    menu_scratch:*mut c_void
)->Result<(),i32>{
    arm_graph_primary_flip_with_output_movie_rgb32(
        primary,contract,generation,origin,client,output_client,
        fps_limit,window,focus_repaint,menu_scratch,false)
}
/// Only for the explicit native Windows 800x600 RGB32 movie A/B.
pub unsafe fn arm_graph_primary_flip_with_output_movie_rgb32(
    primary:SurfaceThis,contract:&AdapterContract,generation:u64,
    origin:(i32,i32),client:(u32,u32),output_client:(u32,u32),
    fps_limit:Option<u32>,window:usize,focus_repaint:bool,
    menu_scratch:*mut c_void,native_movie_rgb32:bool
)->Result<(),i32>{
    arm_graph_primary_flip_with_output_movie_rgb32_composed(
        primary,contract,generation,origin,client,output_client,
        fps_limit,window,focus_repaint,menu_scratch,
        native_movie_rgb32,ptr::null_mut()
    )
}
// The native 800x600 menu scratch is validated as XRGB8888 by the epoch
// builder. It must be paired with the full-frame compositor; a scratch on
// the legacy direct presenter remains forbidden (RGB565-only history).
fn native_movie_menu_scratch_allowed(
    menu_scratch:*mut c_void,full_frame_composite:*mut c_void
)->bool{
    menu_scratch.is_null() || !full_frame_composite.is_null()
}

/// Experimental native RGB32 800x600 movie/menu path. The final surface
/// is an independently owned, full-window logical-size 32-bit offscreen
/// surface that has passed actual DD4 preflight. The epoch holds its COM
/// reference until the virtual Flip session has been retired.
/// All other entrypoints preserve their existing behavior.
pub unsafe fn arm_graph_primary_flip_with_output_movie_rgb32_composed(
    primary:SurfaceThis,contract:&AdapterContract,generation:u64,
    origin:(i32,i32),client:(u32,u32),output_client:(u32,u32),
    fps_limit:Option<u32>,window:usize,focus_repaint:bool,
    menu_scratch:*mut c_void,native_movie_rgb32:bool,
    full_frame_composite:*mut c_void
)->Result<(),i32>{
    if !full_frame_composite.is_null() && !native_movie_rgb32{
        return Err(E_INVALIDARG);
    }
    if !is_our_forwarder(primary){return Err(E_INVALIDARG);}
    let p=&*primary.cast::<Surface4Forwarder>();
    let Some(node)=p.graph else{return Err(E_INVALIDARG);};
    if node.role!=SurfaceRole::Primary||node.mode!=contract.plan.internal||
       (contract.plan.desktop_width,contract.plan.desktop_height)!=client{
        return Err(E_INVALIDARG);
    }
    let back=p.attached.load(Ordering::Acquire);
    if !is_our_forwarder(back){return Err(E_INVALIDARG);}
    let b=&*back.cast::<Surface4Forwarder>();
    let Some(b_node)=b.graph else{return Err(E_INVALIDARG);};
    if b_node.role!=SurfaceRole::Backbuffer||b_node.mode!=node.mode{
        return Err(E_INVALIDARG);
    }
    // A native 32-bit menu signature scratch is safe only with the verified
    // full-frame XRGB8888 composition path. The original RGB565-only
    // restriction must not reject this newly validated pair of surfaces.
    if native_movie_rgb32 &&
       ((node.mode.width,node.mode.height,node.mode.bpp)!=(800,600,16) ||
        !b_node.native_movie_rgb32 ||
        !native_movie_menu_scratch_allowed(menu_scratch,full_frame_composite)){
        return Err(E_INVALIDARG);
    }
    if !native_movie_rgb32 && b_node.native_movie_rgb32{
        return Err(E_INVALIDARG);
    }
    let depth=b.attached.load(Ordering::Acquire);
    if !is_our_forwarder(depth){return Err(E_INVALIDARG);}
    let z=&*depth.cast::<Surface4Forwarder>();
    let Some(z_node)=z.graph else{return Err(E_INVALIDARG);};
    if z_node.role!=SurfaceRole::Depth||z_node.mode!=node.mode{
        return Err(E_INVALIDARG);
    }
    // This fail-closed bridge rejects Phase!=Exposed and bad geometry.
    let bridge=FlipToBltBridge::from_exposed(contract,generation)
        .map_err(|_|E_FAIL)?;
    // Common Blt backend; the DD4 epoch has already verified its real
    // RGB565 or XRGB8888 surface format and graph invariants.
    let mut backend=NativeWindowedBlt::from_borrowed_com(
        p.real,b.real,origin,client,node.mode
    ).map_err(|_|E_INVALIDARG)?;
    backend.set_output_client(output_client).map_err(|_|E_INVALIDARG)?;
    if !full_frame_composite.is_null(){
        backend.enable_full_frame_composition(full_frame_composite)
            .map_err(|_|E_INVALIDARG)?;
    }
    backend.set_menu_scratch(menu_scratch);
    let mut state=p.virtual_flip.lock().map_err(|_|E_FAIL)?;
    if state.is_some()||p.retired.load(Ordering::Acquire){return Err(E_FAIL);}
    *state=Some(GraphFlipSession{bridge,backend,epoch:generation,mode:node.mode,
        fps_limit,last_present:None,
        frame_time_window:crate::frame_time_trace_enabled().then(FrameTimeWindow::default),
        window,focus_repaint,exposure:ExposureState::default(),
        erase_count_seen:0,paint_count_seen:0});
    Ok(())
}

/// Returns the completed native clear-and-stretch count; no COM ref is
/// created. A return of None means this is not a fully armed primary.
///
/// # Safety
/// primary is a live COM Surface4 pointer or null.
pub unsafe fn graph_primary_completed_flips(primary:SurfaceThis)->Option<u64>{
    if !is_our_forwarder(primary){return None;}
    let p=&*primary.cast::<Surface4Forwarder>();
    if p.graph?.role!=SurfaceRole::Primary{return None;}
    let guard=p.virtual_flip.lock().ok()?;
    Some(guard.as_ref()?.bridge.stats().complete_frames)
}

/// Retire a fully armed virtual primary at a resolution-generation boundary.
/// This is a permanent fail-closed tombstone: old game-facing COM references
/// may remain valid until Release, but their Flip cannot reach a physical Blt.
/// Retirement and in-flight Flip use the SAME mutex, so the old presenter
/// is dropped only after the last physical Blt call has returned. The same
/// virtual primary cannot be rearmed for a new generation.
///
/// # Safety
/// primary must be a live game-facing Surface4 COM pointer with a valid
/// client-owned reference, and callers must serialize external resource
/// lifetime according to the owning COM references.
pub unsafe fn retire_graph_primary_flip(primary:SurfaceThis)->Result<u64,i32>{
    if !is_our_forwarder(primary){return Err(E_INVALIDARG);}
    let p=&*primary.cast::<Surface4Forwarder>();
    if p.graph.map(|node|node.role)!=Some(SurfaceRole::Primary){
        return Err(E_INVALIDARG);
    }
    let mut state=p.virtual_flip.lock().map_err(|_|E_FAIL)?;
    if p.retired.load(Ordering::Acquire){return Err(E_FAIL);}
    let Some(session)=state.take() else{return Err(E_FAIL);};
    let completed=session.bridge.stats().complete_frames;
    // Propagate the retired mode generation to the entire known, acyclic
    // Primary -> Backbuffer -> Z32 graph. Old attached COM identities can
    // remain alive for Release/QI, but null-source fills must not render.
    let back=p.attached.load(Ordering::Acquire);
    if !back.is_null(){
        let b=&*back.cast::<Surface4Forwarder>();
        let depth=b.attached.load(Ordering::Acquire);
        if !depth.is_null(){
            (*depth.cast::<Surface4Forwarder>()).retired.store(true,Ordering::Release);
        }
        b.retired.store(true,Ordering::Release);
    }
    p.retired.store(true,Ordering::Release);
    drop(session); // lifetime-owning backend is no longer allowed to present
    Ok(completed)
}

/// Borrow the real DirectDraw4 surface only from our OWN Surface4 COM
/// forwarding object. Does NOT AddRef. The caller MUST retain its own
/// live forwarding COM reference until the native API operation finishes.
/// A foreign, but valid, Surface4 object must return None unchanged.
/// Wine's native Direct3D3 CreateDevice expects its internal real surface
/// implementation; do not pass this forwarding wrapper directly to Wine.
///
/// # Safety
/// candidate must be a valid, live COM interface pointer or null, with
/// a valid vtable pointer. This routine does not authenticate untrusted or
/// stale arbitrary memory addresses.
pub unsafe fn try_borrow_real_surface4(candidate:SurfaceThis)->Option<SurfaceThis>{
    if candidate.is_null(){return None;}
    let vt=*candidate.cast::<*const Surface4VTable>();
    if !std::ptr::eq(vt,&SURFACE4_FORWARD_VTABLE){return None;}
    let real=borrowed_real(candidate);
    if real.is_null(){None}else{Some(real)}
}

unsafe fn borrowed_real(this:SurfaceThis)->SurfaceThis{
    if this.is_null(){return ptr::null_mut();}
    (*this.cast::<Surface4Forwarder>()).real
}
unsafe fn real_vtable(real:SurfaceThis)->&'static Surface4VTable{
    // The caller's live COM reference protects this table's lifetime.
    &**real.cast::<*const Surface4VTable>()
}

unsafe extern "system" fn facade_query_interface(
    this:SurfaceThis,iid:*const Guid,out:*mut SurfaceThis
)->i32{
    if out.is_null(){return E_POINTER;}
    *out=ptr::null_mut();
    if this.is_null()||iid.is_null(){return E_POINTER;}
    if !same_guid(&*iid,&IID_IUNKNOWN)&&!same_guid(&*iid,&IID_SURFACE4){
        if let Some(node)=graph_node(this){
            crate::log(&format!("WINDOWED-GAME: virtual Surface4 QueryInterface unsupported role={:?} iid={:08x}-{:04x}-{:04x}",
                node.role,(*iid).d1,(*iid).d2,(*iid).d3));
        }
        return E_NOINTERFACE;
    }
    if facade_add_ref(this)==0{return E_FAIL;}
    *out=this; // same physical IUnknown and Surface4 identity
    S_OK
}
unsafe extern "system" fn facade_add_ref(this:SurfaceThis)->u32{
    if this.is_null(){return 0;}
    let refs=&(*this.cast::<Surface4Forwarder>()).refs;
    let mut old=refs.load(Ordering::Acquire);
    loop{
        if old==0||old==u32::MAX{return 0;}
        match refs.compare_exchange_weak(
            old,old+1,Ordering::AcqRel,Ordering::Acquire
        ){
            Ok(_)=>return old+1,
            Err(actual)=>old=actual,
        }
    }
}
unsafe extern "system" fn facade_release(this:SurfaceThis)->u32{
    if this.is_null(){return 0;}
    let refs=&(*this.cast::<Surface4Forwarder>()).refs;
    let mut old=refs.load(Ordering::Acquire);
    loop{
        if old==0{return 0;}
        match refs.compare_exchange_weak(
            old,old-1,Ordering::AcqRel,Ordering::Acquire
        ){
            Ok(_)=>{
                let remaining=old-1;
                if remaining==0{
                    // Last façade reference: exactly one underlying real
                    // Release, then free the outer object. All other
                    // callers must already own their own COM references.
                    std::sync::atomic::fence(Ordering::Acquire);
                    let obj=&*this.cast::<Surface4Forwarder>();
                    let child=obj.attached.swap(ptr::null_mut(),Ordering::AcqRel);
                    if !child.is_null(){
                        let child_vt=&**child.cast::<*const Surface4VTable>();
                        (child_vt.release)(child);
                    }
                    let staging=obj.primary_lock_surface.swap(ptr::null_mut(),Ordering::AcqRel);
                    if !staging.is_null(){(real_vtable(staging).release)(staging);}
                    let movie=obj.movie_rgb565_staging.swap(ptr::null_mut(),Ordering::AcqRel);
                    if !movie.is_null(){(real_vtable(movie).release)(movie);}
                    let opening=obj.opening_rgb32_staging.swap(ptr::null_mut(),Ordering::AcqRel);
                    if !opening.is_null(){(real_vtable(opening).release)(opening);}
                    let real=borrowed_real(this);
                    (real_vtable(real).release)(real);
                    drop(Box::from_raw(this.cast::<Surface4Forwarder>()));
                }
                return remaining;
            }
            Err(actual)=>old=actual,
        }
    }
}

unsafe extern "system" fn forward_add_attached_surface(this:SurfaceThis,p1:SurfaceThis)->i32{
    if let Some(node)=graph_node(this){
        // The complete real native RGB565->Z32 attachment exists before
        // game exposure. When the game repeats AddAttachedSurface(Z32) on
        // its virtual backbuffer, acknowledge ONLY our exact virtual Z32
        // child; never forward a wrapper pointer into Wine's native COM.
        if node.role!=SurfaceRole::Backbuffer || p1.is_null(){return E_NOTIMPL;}
        let owner=&*this.cast::<Surface4Forwarder>();
        if owner.retired.load(Ordering::Acquire){return E_NOTIMPL;}
        let attached=owner.attached.load(Ordering::Acquire);
        if attached.is_null()||p1!=attached{return E_NOTIMPL;}
        if graph_node(attached).map(|z|z.role)!=Some(SurfaceRole::Depth){
            return E_NOTIMPL;
        }
        return S_OK;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).add_attached_surface)(real,p1)
}

unsafe extern "system" fn forward_add_overlay_dirty_rect(this:SurfaceThis,p1:*mut Rect)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).add_overlay_dirty_rect)(real,p1)
}

/// Resolve a source supplied to a *native destination* IDirectDrawSurface4::Blt.
///
/// Native Wine ddraw uses CONTAINING_RECORD on its Surface4 argument; it must
/// never receive one of our synthetic Surface4Forwarder objects. This
/// boundary is different from forward_blt(), whose destination is synthetic.
///
/// Return (source, true) only if the candidate was our facade. A foreign
/// native COM interface passes through unchanged. A retired facade, a depth
/// buffer, or a primary without its RGB565 staging source fails closed.
/// Primary Lock/Unlock writes must be copied from staging, not from the
/// 1920x1080 physical desktop primary.
///
/// # Safety
/// The candidate must be a live, valid COM interface with a caller-held
/// reference throughout the native Blt. This cannot authenticate a freed
/// or arbitrary pointer. No AddRef or Release is performed here.
pub unsafe fn source_for_native_blt(
    candidate:SurfaceThis
)->Result<(SurfaceThis,bool),i32>{
    if candidate.is_null(){return Ok((ptr::null_mut(),false));}
    if !is_our_forwarder(candidate){return Ok((candidate,false));}
    let wrapper=&*candidate.cast::<Surface4Forwarder>();
    if wrapper.retired.load(Ordering::Acquire){return Err(E_NOTIMPL);}
    let real=match wrapper.graph {
        Some(node)=>match node.role{
            SurfaceRole::Primary=>wrapper.primary_lock_surface.load(Ordering::Acquire),
            SurfaceRole::Backbuffer=>wrapper.real,
            SurfaceRole::Depth=>return Err(E_NOTIMPL),
        },
        None=>wrapper.real,
    };
    if real.is_null(){return Err(E_NOTIMPL);}
    Ok((real,true))
}

/// Turn a game-supplied Surface4 source into a *native* Source4 pointer.
/// It is never safe to pass a synthetic COM facade into Wine's raw DD4 Blt.
/// A real non-facade Surface4 belongs to the caller and is forwarded as-is;
/// Wine itself validates its object/format. A graph child must be in the
/// same live resolution epoch; stale graph wrappers never reach Wine.
unsafe fn native_copy_source(source:SurfaceThis,target_mode:Mode)->Option<SurfaceThis>{
    if source.is_null(){return None;}
    if !is_our_forwarder(source){return Some(source);}
    let wrapper=&*source.cast::<Surface4Forwarder>();
    if wrapper.retired.load(Ordering::Acquire){return None;}
    if let Some(node)=wrapper.graph {
        if node.role!=SurfaceRole::Backbuffer || node.mode!=target_mode {
            return None;
        }
    }
    (!wrapper.real.is_null()).then_some(wrapper.real)
}

/// Read-only, bounded inspection of the native source's DDSURFACEDESC2.
/// Detects incompatible 555/565/8-bit palette/32-bit formats without
/// changing the source, pixel data or game rendering.
unsafe fn trace_native_copy_format(source:SurfaceThis,flags:u32){
    let mut words=[0u32;31];
    words[0]=124;
    let hr=(real_vtable(source).get_surface_desc)(source,words.as_mut_ptr().cast());
    if hr<0 || words[0]!=124 {
        crate::log(&format!("WINDOWED-COLOR: source GetSurfaceDesc failed hr={:#010x}",hr as u32));
        return;
    }
    crate::log(&format!(
        "WINDOWED-COLOR: backbuffer Blt source={}x{} bpp={} pf_flags={:#x} masks=({:#06x},{:#06x},{:#06x}) caps={:#x} blt_flags={:#x} (read-only)",
        words[3],words[2],words[21],words[19],words[22],words[23],words[24],words[26],flags));
}

fn rect_summary(r:*mut Rect)->String {
    if r.is_null(){return "full".into();}
    let r=unsafe{&*r};
    format!("({},{})-({},{})",r.left,r.top,r.right,r.bottom)
}
/// On an actual Windows native RGB565 Blt failure, capture the operation
/// metadata once per few samples, without touching any pixels or inventing
/// success. Some legacy Blt options can be unsupported by native drivers.
unsafe fn report_native_copy_failure(
    dest:SurfaceThis,source:SurfaceThis,
    dst_rect:*mut Rect,src_rect:*mut Rect,flags:u32,hr:i32,
    source_is_facade:bool,
){
    // The shared module is also compiled in the standalone probe binary,
    // which deliberately has no game Config. crate::log handles per-host
    // logging enablement, and a strict sample cap limits read-only probes.
    let count=GRAPH_COPY_FAILURE_DETAILS.fetch_add(1,Ordering::Relaxed)+1;
    if count>8 && count!=32 && count!=128{return;}
    let desc=|surface:SurfaceThis|->String{
        if surface.is_null(){return "NULL".into();}
        let mut words=[0u32;31];
        words[0]=124;
        let method=real_vtable(surface).get_surface_desc;
        let result=method(surface,words.as_mut_ptr().cast());
        if result<0 || words[0]!=124 {
            return format!("GetSurfaceDesc hr={:#010x}",result as u32);
        }
        format!("{}x{} bits={} flags={:#x} masks={:#x}/{:#x}/{:#x} caps={:#x}",
            words[3],words[2],words[21],words[19],
            words[22],words[23],words[24],words[26])
    };
    crate::log(&format!(
        "NATIVE-BLT-FAIL: count={count} HRESULT={:#010x} flags={:#010x}          source_virtual={} source_same_as_target={} dst_rect={} src_rect={}          source=[{}] dest=[{}] (original HRESULT unchanged)",
         hr as u32,flags,source_is_facade,source==dest,
         rect_summary(dst_rect),rect_summary(src_rect),
         desc(source),desc(dest)
    ));
}

// Strictly bounded native 32-bit movie/video conversion. The Windows HAL
// cannot stretch+convert 320x240 XRGB8888 -> 800x600 RGB565 itself and
// returns E_NOTIMPL. The same-format system RGB565 -> video RGB565 Blt
// already succeeded in the owner's log. Do NOT touch the real destination
// by CPU Lock while a D3D3 HAL device is bound to it: convert into the
// preallocated system-memory scratch and present via ordinary native Blt.
fn xrgb_to_565(pixel:u32)->u16{
    let r=((pixel>>16)&255)*31;
    let g=((pixel>>8)&255)*63;
    let b=(pixel&255)*31;
    (((r+127)/255)<<11 | ((g+127)/255)<<5 | ((b+127)/255)) as u16
}
unsafe fn scale_xrgb320_to_rgb565800(
    src:*const u8,src_pitch:usize,dst:*mut u8,dst_pitch:usize
){
    // Exact integer nearest-neighbour scaling 320x240 -> 800x600
    // (ratio 2.5). Use rational mapping to avoid 16.16 truncation
    // incorrectly selecting pixel 159 rather than 160 at dest x=400,
    // which creates an artificial seam in the image.
    for y in 0usize..600 {
        let sy=y*240/600;
        let sr=src.add(sy*src_pitch);
        let dr=dst.add(y*dst_pitch);
        for x in 0usize..800 {
            let sx=x*320/800;
            let color=ptr::read_unaligned(sr.add(sx*4).cast::<u32>());
            ptr::write_unaligned(dr.add(x*2).cast::<u16>(),xrgb_to_565(color));
        }
    }
}
/// The same native 320x240 XRGB8888 movie Blt is called with WAIT in
/// startup, but flags=0 after MainMenu initialization. Accept both normal
/// copy modes; reject effects/async/keys. The final native scratch copy
/// always uses WAIT so success only follows completed pixels.
fn native_movie_blt_flags_supported(flags:u32)->bool{
    flags==0 || flags==DDBLT_WAIT
}
unsafe fn convert_known_native_movie_blt(
    native_dest:SurfaceThis,native_source:SurfaceThis,scratch:SurfaceThis,
    dst_rect:*mut Rect,src_rect:*mut Rect,flags:u32,fx:*mut c_void
)->Result<i32,&'static str>{
    if native_dest.is_null() || native_source.is_null() || scratch.is_null() ||
       dst_rect.is_null() || !src_rect.is_null() ||
       !native_movie_blt_flags_supported(flags) || !fx.is_null(){
        return Err("not an exact supported movie Blt");
    }
    let area=*dst_rect;
    if area!=(Rect{left:0,top:0,right:800,bottom:600}){
        return Err("movie Blt rectangle differs");
    }
    let src_vt=real_vtable(native_source);
    let dst_vt=real_vtable(native_dest);
    let scratch_vt=real_vtable(scratch);
    let mut sd=[0u32;31];sd[0]=124;
    let mut dd=[0u32;31];dd[0]=124;
    if (src_vt.get_surface_desc)(native_source,sd.as_mut_ptr().cast())<0 ||
       (dst_vt.get_surface_desc)(native_dest,dd.as_mut_ptr().cast())<0{
        return Err("native video surface GetSurfaceDesc failed");
    }
    if (sd[3],sd[2],sd[21],sd[22],sd[23],sd[24])!=(320,240,32,0xff0000,0xff00,0xff) ||
       sd[26]&0x800==0 ||
       (dd[3],dd[2],dd[21],dd[22],dd[23],dd[24])!=(800,600,16,0xf800,0x07e0,0x001f){
        return Err("video format/caps mismatch");
    }
    let mut src_lock=[0u32;31];src_lock[0]=124;
    let hr=(src_vt.lock)(native_source,ptr::null_mut(),src_lock.as_mut_ptr().cast(),
        0x11,ptr::null_mut()); // DDLOCK_WAIT|DDLOCK_READONLY
    if hr<0{return Err("video source Lock failed");}
    let mut dst_lock=[0u32;31];dst_lock[0]=124;
    let hr=(scratch_vt.lock)(scratch,ptr::null_mut(),dst_lock.as_mut_ptr().cast(),
        0x1,ptr::null_mut()); // DDLOCK_WAIT
    if hr<0{
        let _=(src_vt.unlock)(native_source,ptr::null_mut());
        return Err("movie RGB565 staging Lock failed");
    }
    let valid=(src_lock[0],src_lock[3],src_lock[2],src_lock[21])==(124,320,240,32) &&
        (dst_lock[0],dst_lock[3],dst_lock[2],dst_lock[21])==(124,800,600,16) &&
        src_lock[4]>=1280 && dst_lock[4]>=1600 &&
        src_lock[4]<=65536 && dst_lock[4]<=65536 &&
        src_lock[9]!=0 && dst_lock[9]!=0;
    if valid{
        scale_xrgb320_to_rgb565800(
            (src_lock[9] as usize) as *const u8,src_lock[4] as usize,
            (dst_lock[9] as usize) as *mut u8,dst_lock[4] as usize
        );
    }
    // Unlock both regardless of the outcome. Never pass locked video
    // memory to native Blt, or leave a failed frame half-converted.
    let dst_unlock=(scratch_vt.unlock)(scratch,ptr::null_mut());
    let src_unlock=(src_vt.unlock)(native_source,ptr::null_mut());
    if !valid || dst_unlock<0 || src_unlock<0{
        return Err("movie lock geometry/unlock failed");
    }
    let hr=(dst_vt.blt)(native_dest,dst_rect,scratch,ptr::null_mut(),
        DDBLT_WAIT,ptr::null_mut());
    if hr<0{return Err("RGB565 staging -> backbuffer native Blt failed");}
    Ok(hr)
}

/// Widen 16-bit RGB565 to native XRGB8888, keeping the original image's
/// exact pixels and dimensions. Both surfaces have been locked and pitch
/// bounds verified by the caller. Never CPU-lock the active D3D3 backbuffer.
fn rgb565_to_xrgb8888(px:u16)->u32{
    let r=((px>>11)&31) as u32;
    let g=((px>>5)&63) as u32;
    let b=(px&31) as u32;
    ((r<<3|r>>2)<<16)|((g<<2|g>>4)<<8)|(b<<3|b>>2)
}
unsafe fn convert_opening_rgb565_pixels(
    src:*const u8,src_pitch:usize,dst:*mut u8,dst_pitch:usize
){
    for y in 0..600usize{
        let src_row=src.add(y*src_pitch);
        let dst_row=dst.add(y*dst_pitch);
        for x in 0..800usize{
            let pixel=ptr::read_unaligned(src_row.add(x*2).cast::<u16>());
            ptr::write_unaligned(dst_row.add(x*4).cast::<u32>(),
                rgb565_to_xrgb8888(pixel));
        }
    }
}
unsafe fn present_converted_opening_image(
    native_dest:SurfaceThis,native_source:SurfaceThis,scratch:SurfaceThis,
    dst_rect:*mut Rect,src_rect:*mut Rect,flags:u32,fx:*mut c_void
)->Result<i32,&'static str>{
    let full=Rect{left:0,top:0,right:800,bottom:600};
    if native_dest.is_null() || native_source.is_null() || scratch.is_null() ||
       dst_rect.is_null() || src_rect.is_null() ||
       *dst_rect!=full || *src_rect!=full ||
       flags!=(DDBLT_ASYNC|DDBLT_WAIT) || !fx.is_null(){
        return Err("unexpected opening Blt geometry/flags/effects");
    }
    let src_vt=real_vtable(native_source);
    let dst_vt=real_vtable(native_dest);
    let stage_vt=real_vtable(scratch);
    let mut src_d=[0u32;31];src_d[0]=124;
    let mut dst_d=[0u32;31];dst_d[0]=124;
    if (src_vt.get_surface_desc)(native_source,src_d.as_mut_ptr().cast())<0 ||
       (dst_vt.get_surface_desc)(native_dest,dst_d.as_mut_ptr().cast())<0 {
        return Err("opening native surface descriptor failed");
    }
    if (src_d[3],src_d[2],src_d[21],src_d[22],src_d[23],src_d[24])!=
         (800,600,16,0xf800,0x07e0,0x001f) ||
       src_d[26]&0x800==0 ||
       (dst_d[3],dst_d[2],dst_d[21],dst_d[22],dst_d[23],dst_d[24])!=
         (800,600,32,0xff0000,0xff00,0xff) {
        return Err("opening native source/backbuffer format mismatch");
    }
    let mut src_l=[0u32;31];src_l[0]=124;
    if (src_vt.lock)(native_source,ptr::null_mut(),src_l.as_mut_ptr().cast(),
        0x11,ptr::null_mut())<0{
        return Err("opening RGB565 source Lock failed");
    }
    let mut stage_l=[0u32;31];stage_l[0]=124;
    if (stage_vt.lock)(scratch,ptr::null_mut(),stage_l.as_mut_ptr().cast(),
        0x1,ptr::null_mut())<0{
        let _=(src_vt.unlock)(native_source,ptr::null_mut());
        return Err("opening XRGB8888 scratch Lock failed");
    }
    let valid=(src_l[0],src_l[3],src_l[2],src_l[21])==(124,800,600,16) &&
        (stage_l[0],stage_l[3],stage_l[2],stage_l[21])==(124,800,600,32) &&
        src_l[4]>=1600 && src_l[4]<=65536 &&
        stage_l[4]>=3200 && stage_l[4]<=65536 &&
        src_l[9]!=0 && stage_l[9]!=0;
    if valid{
        convert_opening_rgb565_pixels(
            (src_l[9] as usize) as *const u8,src_l[4] as usize,
            (stage_l[9] as usize) as *mut u8,stage_l[4] as usize
        );
    }
    let stage_unlock=(stage_vt.unlock)(scratch,ptr::null_mut());
    let src_unlock=(src_vt.unlock)(native_source,ptr::null_mut());
    if !valid || stage_unlock<0 || src_unlock<0{
        return Err("opening conversion pitch/geometry/unlock failed");
    }
    // Same-format native Blt, already exercised by 320x240 XRGB8888 movies.
    // Never report a successful frame without a successful physical copy.
    let hr=(dst_vt.blt)(native_dest,dst_rect,scratch,src_rect,
        DDBLT_WAIT,ptr::null_mut());
    if hr<0{return Err("opening same-format XRGB8888 Blt failed");}
    Ok(hr)
}

/// The virtual RGB565 Backbuffer is backed by a genuine same-sized native
/// Surface4. Native texture/offscreen source copies can be forwarded safely
/// when the source is unwrapped to a real COM pointer. Depth Blt remains
/// fill-only; primary writes must never be forwarded to desktop coordinates.
unsafe extern "system" fn forward_blt(this:SurfaceThis,p1:*mut Rect,p2:SurfaceThis,p3:*mut Rect,p4:u32,p5:*mut c_void)->i32{
    if let Some(node)=graph_node(this){
        if node.role==SurfaceRole::Primary{
            if native_menu_cursor_probe_enabled(this){
                let n=NATIVE_MENU_PRIMARY_BLT_ATTEMPTS.fetch_add(1,Ordering::Relaxed)+1;
                if native_menu_trace_count(n){
                    crate::log(&format!(
                        "NATIVE-MENU-SURFACE: virtual primary Blt attempted count={n} source_null={} flags={p4:#x} result=E_NOTIMPL (unchanged)",
                        p2.is_null()
                    ));
                }
            }
            virtual_issue(node.role,"Blt primary (unsupported)",E_NOTIMPL);
            return E_NOTIMPL;
        }
        let current=&*this.cast::<Surface4Forwarder>();
        if current.retired.load(Ordering::Acquire){return E_NOTIMPL;}
        if !p1.is_null(){
            let rect=&*p1;
            if rect.left<0 || rect.top<0 || rect.right<=rect.left ||
               rect.bottom<=rect.top || rect.right as u32>node.mode.width ||
               rect.bottom as u32>node.mode.height {
                return E_INVALIDARG;
            }
        }
        if !p2.is_null(){
            // Real color source copying is needed for menu/movie UI.
            // Do not present a Z surface as a copy target or allow fill flags
            // on a source-copy operation. Any other flags are validated by DD4.
            if node.role!=SurfaceRole::Backbuffer ||
               p4&(DDBLT_COLORFILL|DDBLT_DEPTHFILL)!=0{
                virtual_issue(node.role,"Blt source-copy incompatible role/flags",E_NOTIMPL);
                return E_NOTIMPL;
            }
            let Some(real_source)=native_copy_source(p2,node.mode) else {
                virtual_issue(node.role,"Blt source is retired/foreign graph",E_NOTIMPL);
                return E_NOTIMPL;
            };
            let movie_trace=node.native_movie_rgb32 && node.role==SurfaceRole::Backbuffer;
            let movie_call=if movie_trace{
                let count=NATIVE_MOVIE_RGB32_BLT_CALLS.fetch_add(1,Ordering::Relaxed)+1;
                movie_com_trace("backbuffer.native_Blt",count,None,None);
                count
            }else{0};
            let movie_started=if movie_trace{Some(Instant::now())}else{None};
            // The original RGB565 800x600 image is complete in the source,
            // yet the driver's 16->32 native Blt (including WAIT-only) copies
            // only row zero while returning DD_OK. Convert into a separately
            // owned XRGB8888 SYSTEMMEM surface, then perform a SAME-FORMAT
            // native GPU Blt. This path requires the exact verified graph,
            // original full rectangles and original flags; never a host OS
            // identity. Other video and gameplay Blts remain untouched.
            let verified_opening=movie_trace && node.mode.width==800 &&
                node.mode.height==600 && p4==(DDBLT_ASYNC|DDBLT_WAIT) &&
                is_native_opening_image_surface(real_source) &&
                !is_our_forwarder(p2);
            let scratch=current.opening_rgb32_staging.load(Ordering::Acquire);
            let hr=if verified_opening && !scratch.is_null(){
                match present_converted_opening_image(
                    current.real,real_source,scratch,p1,p3,p4,p5
                ){
                    Ok(result)=>{
                        if movie_call<=2 {
                            crate::log(&format!(
                                "STARTUP-IMAGE-BLT: complete RGB565 800x600 converted to private XRGB8888, same-format native Blt succeeded; dst_rect={} src_rect={} original_flags={p4:#010x} final_flags={DDBLT_WAIT:#010x}",
                                rect_summary(p1),rect_summary(p3)
                            ));
                        }
                        result
                    },
                    Err(reason)=>{
                        if movie_call<=2 {
                            crate::log(&format!(
                                "STARTUP-IMAGE-BLT: conversion unavailable ({reason}); original native Blt preserved"
                            ));
                        }
                        (real_vtable(current.real).blt)(
                            current.real,p1,real_source,p3,p4,p5)
                    }
                }
            }else{
                (real_vtable(current.real).blt)(
                    current.real,p1,real_source,p3,p4,p5)
            };
            if let Some(started)=movie_started{
                movie_com_trace("backbuffer.native_Blt",movie_call,Some(hr),
                    Some(started.elapsed().as_millis()));
                if movie_call==3 {
                    crate::log(&format!(
                        "STARTUP-IMAGE: source_transition=RGB565-800x600-to-RGB32-320x240 preceding_800x600_copies=2 ms_since_first_present={:?} [observation only, not proof of visible pixels]",
                        OPENING_FIRST_PRESENT.get().map(|at|at.elapsed().as_millis())
                    ));
                }
            }
            if hr>=0 {
                let count=GRAPH_COPY_COUNT.fetch_add(1,Ordering::Relaxed)+1;
                if count<=4 {trace_native_copy_format(real_source,p4);}
                if count==1 || count%120==0{
                    crate::log(&format!("WINDOWED-GAME: native RGB565 backbuffer source Blt succeeded copies={count} flags={p4:#x}"));
                }
            }else{
                // Activate ONLY for the EXACT observed native Windows
                // unsupported XRGB8888 movie stretch and an already-owned,
                // format-checked staging surface. Wine and other modes
                // never provision this scratch and keep the old native Blt.
                let scratch=current.movie_rgb565_staging.load(Ordering::Acquire);
                if hr==E_NOTIMPL && !scratch.is_null() && !is_our_forwarder(p2) {
                    match convert_known_native_movie_blt(
                        current.real,real_source,scratch,p1,p3,p4,p5
                    ){
                        Ok(converted)=>{
                            let count=GRAPH_MOVIE_CONVERT_SUCCESS.fetch_add(1,Ordering::Relaxed)+1;
                            if count<=5 || count%120==0{
                                crate::log(&format!("NATIVE-MOVIE-BLT: converted XRGB8888 320x240 -> RGB565 800x600 via system-memory staging; original native E_NOTIMPL recovered count={count} result={converted:#x}"));
                            }
                            return converted;
                        },
                        Err(reason)=>{
                            let count=GRAPH_MOVIE_CONVERT_FAILED.fetch_add(1,Ordering::Relaxed)+1;
                            if count<=8 || count%120==0{
                                crate::log(&format!("NATIVE-MOVIE-BLT: bounded safe fallback unavailable: {reason}; original HRESULT preserved count={count}"));
                            }
                        }
                    }
                }
                report_native_copy_failure(current.real,real_source,p1,p3,p4,hr,
                    is_our_forwarder(p2));
                virtual_issue(node.role,"native backbuffer source Blt failed",hr);
            }
            return hr;
        }
        if !p3.is_null(){return E_INVALIDARG;}
        let requested_fill=p4&!DDBLT_WAIT;
        let expected_fill=if node.role==SurfaceRole::Depth{DDBLT_DEPTHFILL}else{DDBLT_COLORFILL};
        if requested_fill!=expected_fill{
            virtual_issue(node.role,"Blt unsupported fill flags",E_NOTIMPL);
            return E_NOTIMPL;
        }
        if p5.is_null(){return E_INVALIDARG;}
        if *(p5.cast::<u32>())!=100{return E_INVALIDARG;}
        return (real_vtable(current.real).blt)(current.real,p1,ptr::null_mut(),ptr::null_mut(),p4,p5);
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).blt)(real,p1,p2,p3,p4,p5)
}

unsafe extern "system" fn forward_blt_batch(this:SurfaceThis,p1:*mut c_void,p2:u32,p3:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).blt_batch)(real,p1,p2,p3)
}

unsafe extern "system" fn forward_blt_fast(this:SurfaceThis,p1:u32,p2:u32,p3:SurfaceThis,p4:*mut Rect,p5:u32)->i32{
    if let Some(node)=graph_node(this){
        if native_menu_cursor_probe_enabled(this){
            let n=NATIVE_MENU_BLTFAST_ATTEMPTS.fetch_add(1,Ordering::Relaxed)+1;
            if native_menu_trace_count(n){
                crate::log(&format!(
                    "NATIVE-MENU-SURFACE: virtual BltFast role={:?} count={n} dest=({p1},{p2}) src_null={} flags={p5:#x} result=E_NOTIMPL (unchanged)",
                    node.role,p3.is_null()
                ));
            }
        }
        return E_NOTIMPL;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).blt_fast)(real,p1,p2,p3,p4,p5)
}

unsafe extern "system" fn forward_delete_attached_surface(this:SurfaceThis,p1:u32,p2:SurfaceThis)->i32{
    if graph_node(this).is_some(){return E_NOTIMPL;}
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).delete_attached_surface)(real,p1,p2)
}

unsafe extern "system" fn forward_enum_attached_surfaces(this:SurfaceThis,p1:*mut c_void,p2:*const c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).enum_attached_surfaces)(real,p1,p2)
}

unsafe extern "system" fn forward_enum_overlay_z_orders(this:SurfaceThis,p1:u32,p2:*mut c_void,p3:*const c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).enum_overlay_z_orders)(real,p1,p2,p3)
}

unsafe extern "system" fn forward_flip(this:SurfaceThis,p1:SurfaceThis,p2:u32)->i32{
    if let Some(node)=graph_node(this){
        if node.role!=SurfaceRole::Primary||p2!=0{return E_INVALIDARG;}
        let p=&*this.cast::<Surface4Forwarder>();
        let back=p.attached.load(Ordering::Acquire);
        // Accept only a NULL target or this primary's own virtual backbuffer.
        if back.is_null(){return E_NOTIMPL;} // incomplete graph is unarmed
        if !p1.is_null()&&p1!=back{return E_INVALIDARG;}
        let movie=graph_node(back).is_some_and(|n|n.native_movie_rgb32);
        let movie_call=if movie{
            let count=NATIVE_MOVIE_RGB32_FLIP_CALLS.fetch_add(1,Ordering::Relaxed)+1;
            movie_com_trace("primary.virtual_Flip",count,None,None);
            count
        }else{0};
        let movie_started=if movie{Some(Instant::now())}else{None};
        let Ok(mut state)=p.virtual_flip.lock() else{return E_FAIL;};
        let Some(session)=state.as_mut() else{return E_NOTIMPL;};
        // A successful windowed Flip is the sole frame-present boundary.
        // Do not rely on the passive modern Flip hook: these synthetic COM
        // surfaces never call through that native interface.
        // Sample atomic guard counters from the rendering thread; the
        // optional WndProc callback itself never logs, paints, calls DD4
        // or takes our Flip mutex. Suppressed WM_ERASEBKGND is evidence
        // that Win32 requested an erase, NOT proof it caused the white
        // screen. Log only the first events and each 120th thereafter.
        if let Some(counts)=window_erase_guard::snapshot(session.window){
            let total=counts.erased;
            if total>session.erase_count_seen{
                session.erase_count_seen=total;
                if total<=12 || total%120==0{
                    crate::log(&format!(
                        "WINDOWED-GAME: WindowEraseGuard suppressed WM_ERASEBKGND total={total} wm_paint_total={}",counts.painted));
                }
            }
            if counts.painted>session.paint_count_seen{
                session.paint_count_seen=counts.painted;
                if counts.painted<=12 || counts.painted%120==0{
                    crate::log(&format!(
                        "WINDOWED-GAME: WindowEraseGuard observed WM_PAINT total={} suppressed_erase_total={total}",
                        counts.painted));
                }
            }
        }
        // Observe foreground status only for FocusRepaint. Do not
        // erase paint regions or reenter COM from a window callback.
        let repaint=if session.focus_repaint{
            session.exposure.sample(
                window_exposure::sample_game_window(session.window))
        }else{false};
        // Native Windows RGB32 movie A/B only: did the game window's
        // WndProc actually receive keyboard messages during the videos?
        // Do not consume/dispatch input here: doing so under the virtual
        // Flip lock risks COM/UI reentrancy and changes sample scheduling.
        if movie && (movie_call==1 || movie_call%120==0){
            match window_erase_guard::sample_input_state(session.window){
                Some(input)=>{
                    let c=input.counts;
                    crate::log(&format!(
                        "NATIVE-MOVIE-INPUT: movie_flip={movie_call} foreground={} window_thread={} flip_thread={} same_thread={} new_key_or_mouse_input={} wndproc_keydown={} wndproc_keyup={} wndproc_char={} wndproc_focus_changes={} wndproc_paint={} wndproc_mouse_moves={} wndproc_mouse_buttons={} last_mouse_client={:?} menu_mouse_remapped={} last_menu_mouse_logical={:?} system_cursor_probe_events={} system_cursor_probe_restores={} os_cursor_visible={:?} os_cursor_screen={:?} primary_blt_rejected={} bltfast_rejected={} surface_getdc={} surface_releasedc={}",
                        input.game_is_foreground,input.window_thread,
                        input.current_thread,
                        input.window_thread==input.current_thread,
                        input.queue_has_new_input,c.keydown,c.keyup,
                        c.character,c.focus_changes,c.painted,
                        c.mouse_moves,c.mouse_buttons,input.cursor_client_last_move,
                        c.menu_mouse_mapped,input.menu_mouse_logical_last_move,
                        c.system_cursor_probe_events,c.system_cursor_probe_restores,
                        input.cursor_visible,input.cursor_screen,
                        NATIVE_MENU_PRIMARY_BLT_ATTEMPTS.load(Ordering::Relaxed),
                        NATIVE_MENU_BLTFAST_ATTEMPTS.load(Ordering::Relaxed),
                        NATIVE_MENU_GETDC_ATTEMPTS.load(Ordering::Relaxed),
                        NATIVE_MENU_RELEASEDC_ATTEMPTS.load(Ordering::Relaxed)
                    ));
                },
                None=>crate::log(&format!(
                    "NATIVE-MOVIE-INPUT: movie_flip={movie_call} window_probe=unavailable"
                ))
            }
        }
        pace_graph_flip(session);
        // Passive, bounded preferences diagnostics only; the original
        // dropdowns and video settings are not modified.
        crate::menu_video_probe::observe_ui_flip(session.mode.width,session.mode.height);
        let pump_window=session.window;
        let timing_enabled=session.frame_time_window.is_some();
        let GraphFlipSession{bridge,backend,epoch,mode,..}=session;
        // The original script dispatcher reports only four CALL sites.
        // Returning from gameplay can restore the genuine MainMenu
        // artwork without reissuing one of those script-loader calls.
        // Probe only an authenticated 4:3 menu frame *after* MainMenu
        // was observed once. The offscreen Campaign-art test decides
        // whether text is actually drawn; all other frames stay unedited.
        if crate::menu_brand::should_probe_menu_frame(mode.width,mode.height){
            backend.prepare_menu_frame();
        }else{
            backend.clear_menu_frame();
            crate::menu_brand::mark_not_drawn();
        }
        let composed=backend.menu_frame_prepared();
        let timing_started=timing_enabled.then(Instant::now);
        let presentation=bridge.present_with_refresh(*epoch,*mode,backend,repaint);
        let timing_finished=timing_started.map(|_|Instant::now());
        if let Some(started)=movie_started{
            let hr=match &presentation{
                Ok(_)=>S_OK,
                Err(FlipBridgeError::ClearBlackFailed(hr)|
                    FlipBridgeError::StretchFailed(hr))=>*hr,
                Err(_)=>E_FAIL,
            };
            movie_com_trace("primary.virtual_Flip",movie_call,Some(hr),
                Some(started.elapsed().as_millis()));
        }
        let flip_result=match presentation{
            Ok(frame)=>{
                if movie && movie_call==1 {
                    let _=OPENING_FIRST_PRESENT.set(Instant::now());
                    crate::log("STARTUP-IMAGE: first 800x600 movie/menu frame submitted to real primary [pixel content unverified]");
                }
                if composed{
                    crate::menu_brand::note_branded_presented();
                }
                if repaint{
                    crate::log(&format!(
                        "WINDOWED-GAME: FocusRepaint one-shot full repaint succeeded epoch={epoch} frame={frame} (foreground return or new Win32 update region)"));
                }
                let count=GRAPH_FLIP_COUNT.fetch_add(1,Ordering::Relaxed)+1;
                if count==1 || count%120==0{
                    crate::log(&format!("WINDOWED-GAME: virtual Primary Flip presented frame={frame} global_flips={count}"));
                }
                S_OK
            },
            Err(FlipBridgeError::ClearBlackFailed(hr)|
                FlipBridgeError::StretchFailed(hr))=>{
                virtual_issue(SurfaceRole::Primary,"Flip native presentation failed",hr);
                hr
            },
            Err(failure)=>{
                virtual_issue(SurfaceRole::Primary,&format!("Flip bridge failed: {failure:?}"),E_FAIL);
                E_FAIL // includes latched failure; never exclusive
            },
        };
        let frame_timing=if flip_result==S_OK {
            match (timing_started,timing_finished,session.frame_time_window.as_mut()){
                (Some(start),Some(end),Some(window))=>
                    window.observe(end,end.saturating_duration_since(start),
                        node.mode,session.fps_limit),
                _=>None,
            }
        }else{None};
        // The OS window procedure can re-enter game code and COM.
        // ABSOLUTELY release the entire virtual Flip mutex before attempting
        // to dispatch even one queued Win32 message. Without the explicit
        // native-only, off-by-default arm this does nothing (including Wine).
        drop(state);
        // Emit at most one summary per 120 successful Flips and outside
        // the graph mutex. No synchronous logging in each sampled frame.
        if let Some(summary)=frame_timing{crate::log(&summary);}
        if movie {
            if let Some((n,calls,total))=
                window_erase_guard::pump_native_movie_window(
                    // Read the verified HWND from the session before
                    // releasing state: no borrowed session references here.
                    pump_window
                )
            {
                if calls==1 || calls%120==0 || (n>0 && total<=12){
                    crate::log(&format!(
                        "NATIVE-MOVIE-PUMP: after Flip count={calls} dispatched_this_flip={n} total_dispatched={total} hwnd=verified owner_thread=true max_per_flip=4"
                    ));
                }
            }
        }
        return flip_result;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).flip)(real,p1,p2)
}

unsafe extern "system" fn forward_get_attached_surface(this:SurfaceThis,p1:*mut c_void,p2:*mut SurfaceThis)->i32{
    if let Some(node)=graph_node(this){
        if p2.is_null(){return E_POINTER;}
        *p2=ptr::null_mut();
        if p1.is_null(){return E_POINTER;}
        let requested=*(p1.cast::<u32>());
        let expected=match node.role {
            SurfaceRole::Primary=>0x4,       // DDSCAPS_BACKBUFFER
            SurfaceRole::Backbuffer=>0x20000, // DDSCAPS_ZBUFFER
            SurfaceRole::Depth=>return E_INVALIDARG,
        };
        if requested!=expected{
            virtual_issue(node.role,"GetAttachedSurface unexpected caps",E_INVALIDARG);
            return E_INVALIDARG;
        }
        let parent=&*this.cast::<Surface4Forwarder>();
        let child=parent.attached.load(Ordering::Acquire);
        if child.is_null(){return E_FAIL;}
        let v=&**child.cast::<*const Surface4VTable>();
        if (v.add_ref)(child)==0{return E_FAIL;}
        *p2=child;
        return S_OK;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_attached_surface)(real,p1,p2)
}

unsafe extern "system" fn forward_get_blt_status(this:SurfaceThis,p1:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    let movie=graph_node(this).is_some_and(|n|n.native_movie_rgb32);
    let count=if movie{
        let n=NATIVE_MOVIE_RGB32_BLT_STATUS_CALLS.fetch_add(1,Ordering::Relaxed)+1;
        movie_com_trace("backbuffer.GetBltStatus",n,None,None);
        n
    }else{0};
    let start=if movie{Some(Instant::now())}else{None};
    let hr=(real_vtable(real).get_blt_status)(real,p1);
    if let Some(start)=start{
        movie_com_trace("backbuffer.GetBltStatus",count,Some(hr),
            Some(start.elapsed().as_millis()));
    }
    hr
}

unsafe extern "system" fn forward_get_caps(this:SurfaceThis,p1:*mut c_void)->i32{
    if let Some(node)=graph_node(this){
        if p1.is_null(){return E_POINTER;}
        let out=p1.cast::<u32>();
        ptr::write_bytes(out,0,4);
        *out=node.caps();
        return S_OK;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_caps)(real,p1)
}

unsafe extern "system" fn forward_get_clipper(this:SurfaceThis,p1:*mut *mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_clipper)(real,p1)
}

unsafe extern "system" fn forward_get_color_key(this:SurfaceThis,p1:u32,p2:*mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_color_key)(real,p1,p2)
}

unsafe extern "system" fn forward_get_dc(this:SurfaceThis,p1:*mut *mut c_void)->i32{
    let node=graph_node(this);
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    let hr=(real_vtable(real).get_dc)(real,p1);
    if let Some(node)=node{
        if native_menu_cursor_probe_enabled(this){
            let n=NATIVE_MENU_GETDC_ATTEMPTS.fetch_add(1,Ordering::Relaxed)+1;
            if native_menu_trace_count(n){
                crate::log(&format!(
                    "NATIVE-MENU-SURFACE: GetDC forwarded to physical surface role={:?} count={n} result={:#010x} (unchanged)",
                    node.role,hr as u32
                ));
            }
        }
    }
    hr
}

/// The native primary lives in DDSCL_NORMAL and does NOT have a physical
/// flipping chain. The game's advertised virtual PRIMARY does: Flip returns
/// only after both native Blts complete. Query only the virtual session,
/// never the unrelated real primary, so status cannot misreport NOTFLIPPABLE.
fn virtual_primary_flip_status(
    p:&Surface4Forwarder,flags:u32
)->i32{
    if flags!=DDGFS_CANFLIP && flags!=DDGFS_ISFLIPDONE{
        return E_INVALIDARG;
    }
    if p.retired.load(Ordering::Acquire){return E_FAIL;}
    // Never block in GetFlipStatus while an in-flight Flip holds this
    // same mutex. The busy HRESULT reflects a genuine incomplete
    // synchronous Blt pair, never a fabricated DirectShow completion.
    match p.virtual_flip.try_lock(){
        Ok(state)=>match state.as_ref(){
            Some(session) if session.bridge.stats().fault.is_none()=>S_OK,
            _=>E_FAIL,
        },
        Err(TryLockError::WouldBlock)=>DDERR_WASSTILLDRAWING,
        Err(TryLockError::Poisoned(_))=>E_FAIL,
    }
}
unsafe extern "system" fn forward_get_flip_status(this:SurfaceThis,p1:u32)->i32{
    if let Some(node)=graph_node(this){
        if node.role==SurfaceRole::Primary{
            return virtual_primary_flip_status(&*this.cast::<Surface4Forwarder>(),p1);
        }
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    let movie=graph_node(this).is_some_and(|n|n.native_movie_rgb32);
    let count=if movie{
        let n=NATIVE_MOVIE_RGB32_FLIP_STATUS_CALLS.fetch_add(1,Ordering::Relaxed)+1;
        movie_com_trace("backbuffer.GetFlipStatus",n,None,None);
        n
    }else{0};
    let start=if movie{Some(Instant::now())}else{None};
    let hr=(real_vtable(real).get_flip_status)(real,p1);
    if let Some(start)=start{
        movie_com_trace("backbuffer.GetFlipStatus",count,Some(hr),
            Some(start.elapsed().as_millis()));
    }
    hr
}

unsafe extern "system" fn forward_get_overlay_position(this:SurfaceThis,p1:*mut i32,p2:*mut i32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_overlay_position)(real,p1,p2)
}

unsafe extern "system" fn forward_get_palette(this:SurfaceThis,p1:*mut *mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_palette)(real,p1)
}

unsafe extern "system" fn forward_get_pixel_format(this:SurfaceThis,p1:*mut c_void)->i32{
    if let Some(node)=graph_node(this){
        if p1.is_null(){return E_POINTER;}
        let out=p1.cast::<u32>();
        if *out!=32{return E_INVALIDARG;}
        virtual_pixel_format(node,out);
        return S_OK;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_pixel_format)(real,p1)
}

unsafe extern "system" fn forward_get_surface_desc(this:SurfaceThis,p1:*mut c_void)->i32{
    if let Some(node)=graph_node(this){
        if p1.is_null(){return E_POINTER;}
        let out=p1.cast::<u32>();
        if *out!=124{return E_INVALIDARG;}
        ptr::write_bytes(out,0,31);
        *out=124;
        *out.add(1)=0x1007|if node.count()!=0{0x20}else{0};
        *out.add(2)=node.mode.height;
        *out.add(3)=node.mode.width;
        *out.add(5)=node.count();
        virtual_pixel_format(node,out.add(18));
        *out.add(26)=node.caps();
        return S_OK;
    }
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_surface_desc)(real,p1)
}

unsafe extern "system" fn forward_initialize(this:SurfaceThis,p1:*mut c_void,p2:*mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).initialize)(real,p1,p2)
}

unsafe extern "system" fn forward_is_lost(this:SurfaceThis)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).is_lost)(real)
}

unsafe extern "system" fn forward_lock(this:SurfaceThis,p1:*mut Rect,p2:*mut c_void,p3:u32,p4:*mut c_void)->i32{
    let node=graph_node(this);
    if let Some(n)=node {
        if (*this.cast::<Surface4Forwarder>()).retired.load(Ordering::Acquire){
            virtual_issue(n.role,"Lock retired surface",E_NOTIMPL);
            return E_NOTIMPL;
        }
    }
    let real=if node.map(|n|n.role)==Some(SurfaceRole::Primary){
        (*this.cast::<Surface4Forwarder>()).primary_lock_surface.load(Ordering::Acquire)
    }else{borrowed_real(this)};
    if real.is_null(){
        if let Some(n)=node{virtual_issue(n.role,"Lock no verified native target",E_NOTIMPL);}
        return E_NOTIMPL;
    }
    let hr=(real_vtable(real).lock)(real,p1,p2,p3,p4);
    if hr<0 {if let Some(n)=node{virtual_issue(n.role,"native Lock failed",hr);}}
    else if let Some(n)=node{
        if n.role==SurfaceRole::Primary{
            // Native staging is logically the *primary*. Preserve its
            // returned pitch and pixel pointer, but expose the game's
            // expected surface capabilities, never the offscreen caps.
            if !p2.is_null() && *(p2.cast::<u32>())==124{
                *p2.cast::<u32>().add(26)=n.caps();
            }
            if VIRTUAL_DIAG_COUNT.fetch_add(1,Ordering::Relaxed)<40{
                crate::log(&format!("WINDOWED-GAME: virtual Surface4 Primary Lock via mode-matched RGB565 staging surface {}x{} result=0x0",
                    n.mode.width,n.mode.height));
            }
        }
    }
    hr
}

unsafe extern "system" fn forward_release_dc(this:SurfaceThis,p1:*mut c_void)->i32{
    let node=graph_node(this);
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    let hr=(real_vtable(real).release_dc)(real,p1);
    if let Some(node)=node{
        if native_menu_cursor_probe_enabled(this){
            let n=NATIVE_MENU_RELEASEDC_ATTEMPTS.fetch_add(1,Ordering::Relaxed)+1;
            if native_menu_trace_count(n){
                crate::log(&format!(
                    "NATIVE-MENU-SURFACE: ReleaseDC forwarded to physical surface role={:?} count={n} result={:#010x} (unchanged)",
                    node.role,hr as u32
                ));
            }
        }
    }
    hr
}

unsafe extern "system" fn forward_restore(this:SurfaceThis)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).restore)(real)
}

unsafe extern "system" fn forward_set_clipper(this:SurfaceThis,p1:*mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_clipper)(real,p1)
}

unsafe extern "system" fn forward_set_color_key(this:SurfaceThis,p1:u32,p2:*mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_color_key)(real,p1,p2)
}

unsafe extern "system" fn forward_set_overlay_position(this:SurfaceThis,p1:i32,p2:i32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_overlay_position)(real,p1,p2)
}

unsafe extern "system" fn forward_set_palette(this:SurfaceThis,p1:*mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_palette)(real,p1)
}

unsafe extern "system" fn forward_unlock(this:SurfaceThis,p1:*mut Rect)->i32{
    let node=graph_node(this);
    let real=if node.map(|n|n.role)==Some(SurfaceRole::Primary){
        (*this.cast::<Surface4Forwarder>()).primary_lock_surface.load(Ordering::Acquire)
    }else{borrowed_real(this)};
    if real.is_null(){return E_NOTIMPL;}
    let hr=(real_vtable(real).unlock)(real,p1);
    if hr<0 {if let Some(n)=node{virtual_issue(n.role,"native Unlock failed",hr);}}
    hr
}

unsafe extern "system" fn forward_update_overlay(this:SurfaceThis,p1:*mut Rect,p2:SurfaceThis,p3:*mut Rect,p4:u32,p5:*mut c_void)->i32{
    if graph_node(this).is_some(){return E_NOTIMPL;}
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).update_overlay)(real,p1,p2,p3,p4,p5)
}

unsafe extern "system" fn forward_update_overlay_display(this:SurfaceThis,p1:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).update_overlay_display)(real,p1)
}

unsafe extern "system" fn forward_update_overlay_z_order(this:SurfaceThis,p1:u32,p2:SurfaceThis)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).update_overlay_z_order)(real,p1,p2)
}

unsafe extern "system" fn forward_get_dd_interface(this:SurfaceThis,p1:*mut *mut c_void)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_dd_interface)(real,p1)
}

unsafe extern "system" fn forward_page_lock(this:SurfaceThis,p1:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).page_lock)(real,p1)
}

unsafe extern "system" fn forward_page_unlock(this:SurfaceThis,p1:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).page_unlock)(real,p1)
}

unsafe extern "system" fn forward_set_surface_desc(this:SurfaceThis,p1:*mut c_void,p2:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_surface_desc)(real,p1,p2)
}

unsafe extern "system" fn forward_set_private_data(this:SurfaceThis,p1:*const Guid,p2:*mut c_void,p3:u32,p4:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).set_private_data)(real,p1,p2,p3,p4)
}

unsafe extern "system" fn forward_get_private_data(this:SurfaceThis,p1:*const Guid,p2:*mut c_void,p3:*mut u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_private_data)(real,p1,p2,p3)
}

unsafe extern "system" fn forward_free_private_data(this:SurfaceThis,p1:*const Guid)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).free_private_data)(real,p1)
}

unsafe extern "system" fn forward_get_uniqueness_value(this:SurfaceThis,p1:*mut u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_uniqueness_value)(real,p1)
}

unsafe extern "system" fn forward_change_uniqueness_value(this:SurfaceThis)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).change_uniqueness_value)(real)
}

static SURFACE4_FORWARD_VTABLE:Surface4VTable=Surface4VTable{
    query_interface:facade_query_interface,
    add_ref:facade_add_ref,
    release:facade_release,
    add_attached_surface:forward_add_attached_surface,
    add_overlay_dirty_rect:forward_add_overlay_dirty_rect,
    blt:forward_blt,
    blt_batch:forward_blt_batch,
    blt_fast:forward_blt_fast,
    delete_attached_surface:forward_delete_attached_surface,
    enum_attached_surfaces:forward_enum_attached_surfaces,
    enum_overlay_z_orders:forward_enum_overlay_z_orders,
    flip:forward_flip,
    get_attached_surface:forward_get_attached_surface,
    get_blt_status:forward_get_blt_status,
    get_caps:forward_get_caps,
    get_clipper:forward_get_clipper,
    get_color_key:forward_get_color_key,
    get_dc:forward_get_dc,
    get_flip_status:forward_get_flip_status,
    get_overlay_position:forward_get_overlay_position,
    get_palette:forward_get_palette,
    get_pixel_format:forward_get_pixel_format,
    get_surface_desc:forward_get_surface_desc,
    initialize:forward_initialize,
    is_lost:forward_is_lost,
    lock:forward_lock,
    release_dc:forward_release_dc,
    restore:forward_restore,
    set_clipper:forward_set_clipper,
    set_color_key:forward_set_color_key,
    set_overlay_position:forward_set_overlay_position,
    set_palette:forward_set_palette,
    unlock:forward_unlock,
    update_overlay:forward_update_overlay,
    update_overlay_display:forward_update_overlay_display,
    update_overlay_z_order:forward_update_overlay_z_order,
    get_dd_interface:forward_get_dd_interface,
    page_lock:forward_page_lock,
    page_unlock:forward_page_unlock,
    set_surface_desc:forward_set_surface_desc,
    set_private_data:forward_set_private_data,
    get_private_data:forward_get_private_data,
    free_private_data:forward_free_private_data,
    get_uniqueness_value:forward_get_uniqueness_value,
    change_uniqueness_value:forward_change_uniqueness_value,
};

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::atomic::{AtomicU32,Ordering};

    #[test]
    fn native_movie_com_trace_is_bounded_but_preserves_progress_checkpoints(){
        assert!(movie_com_trace_selected(1));
        assert!(movie_com_trace_selected(120));
        assert!(movie_com_trace_selected(256));
        assert!(!movie_com_trace_selected(257));
        assert!(movie_com_trace_selected(320));
        assert!(!movie_com_trace_selected(321));
    }

    #[test]
    fn native_rgb32_menu_scratch_requires_full_frame_composition(){
        let scratch=std::ptr::dangling_mut::<c_void>();
        let composite=2usize as *mut c_void;
        assert!(native_movie_menu_scratch_allowed(ptr::null_mut(),ptr::null_mut()));
        assert!(native_movie_menu_scratch_allowed(ptr::null_mut(),composite));
        assert!(native_movie_menu_scratch_allowed(scratch,composite));
        assert!(!native_movie_menu_scratch_allowed(scratch,ptr::null_mut()));
    }

    #[test]
    fn native_movie_rgb32_format_is_real_xrgb_on_backbuffer_only(){
        let mode=Mode{width:800,height:600,bpp:16};
        for role in [SurfaceRole::Primary,SurfaceRole::Backbuffer,SurfaceRole::Depth]{
            let node=VirtualGraphNode{
                role,mode,native_movie_rgb32:role==SurfaceRole::Backbuffer
            };
            let mut fmt=[0u32;8];
            unsafe{virtual_pixel_format(node,fmt.as_mut_ptr())};
            assert_eq!(fmt[0],32);
            assert_eq!(fmt[3],if role==SurfaceRole::Primary{16}else{32});
            if role==SurfaceRole::Backbuffer{
                assert_eq!((fmt[4],fmt[5],fmt[6]),(0xff0000,0xff00,0xff));
            }else if role==SurfaceRole::Primary{
                assert_eq!((fmt[4],fmt[5],fmt[6]),(0xf800,0x07e0,0x001f));
            }else{
                assert_eq!(fmt[1],0x400);
            }
        }
    }

    #[repr(C)]
    struct RealMock{
        vtable:*const Surface4VTable,
        refs:AtomicU32,
        blts:AtomicU32,
        flips:AtomicU32,
    }
    unsafe extern "system" fn real_add_ref(this:SurfaceThis)->u32{
        (&*this.cast::<RealMock>()).refs.fetch_add(1,Ordering::SeqCst)+1
    }
    unsafe extern "system" fn real_release(this:SurfaceThis)->u32{
        (&*this.cast::<RealMock>()).refs.fetch_sub(1,Ordering::SeqCst)-1
    }
    unsafe extern "system" fn real_blt(
        this:SurfaceThis,_dst:*mut Rect,_source:SurfaceThis,_src:*mut Rect,
        _flags:u32,_effects:*mut c_void
    )->i32{
        (&*this.cast::<RealMock>()).blts.fetch_add(1,Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn real_flip(
        this:SurfaceThis,_target:SurfaceThis,_flags:u32
    )->i32{
        (&*this.cast::<RealMock>()).flips.fetch_add(1,Ordering::SeqCst);
        0
    }

    fn mock_table()->Surface4VTable{
        let mut vt=crate::com_surface4_abi::tests::fixture();
        vt.add_ref=real_add_ref;
        vt.release=real_release;
        vt.blt=real_blt;
        vt.flip=real_flip;
        vt
    }
    fn real_mock<'a>(table:&'a Surface4VTable)->RealMock{
        RealMock{vtable:table,refs:AtomicU32::new(1),
                 blts:AtomicU32::new(0),flips:AtomicU32::new(0)}
    }
    #[test]
    fn native_destination_blt_never_forwards_a_synthetic_graph_source(){
        let table=mock_table();
        let mut primary_real=real_mock(&table);
        let mut staging_real=real_mock(&table);
        let mut back_real=real_mock(&table);
        let mut depth_real=real_mock(&table);
        let primary_ptr=(&mut primary_real as *mut RealMock).cast();
        let staging_ptr=(&mut staging_real as *mut RealMock).cast();
        let back_ptr=(&mut back_real as *mut RealMock).cast();
        let depth_ptr=(&mut depth_real as *mut RealMock).cast();
        let mode=Mode{width:640,height:480,bpp:16};
        unsafe{
            assert_eq!(source_for_native_blt(ptr::null_mut()),Ok((ptr::null_mut(),false)));
            assert_eq!(source_for_native_blt(back_ptr),Ok((back_ptr,false)));
            let primary=new_graph_surface4(primary_ptr,SurfaceRole::Primary,mode).unwrap();
            let back=new_graph_surface4(back_ptr,SurfaceRole::Backbuffer,mode).unwrap();
            let depth=new_graph_surface4(depth_ptr,SurfaceRole::Depth,mode).unwrap();
            // Prior to staging initialization a virtual primary is not a
            // valid native Blt source.
            assert_eq!(source_for_native_blt(primary),Err(E_NOTIMPL));
            arm_graph_primary_lock_surface(primary,staging_ptr).unwrap();
            assert_eq!(source_for_native_blt(primary),Ok((staging_ptr,true)));
            assert_eq!(source_for_native_blt(back),Ok((back_ptr,true)));
            assert_eq!(source_for_native_blt(depth),Err(E_NOTIMPL));
            (*back.cast::<Surface4Forwarder>()).retired.store(true,Ordering::Release);
            assert_eq!(source_for_native_blt(back),Err(E_NOTIMPL));
            for wrapper in [primary,back,depth]{
                assert_eq!(((*wrapper.cast::<Surface4Forwarder>()).vtable.as_ref().unwrap().release)(wrapper),0);
            }
            assert_eq!(primary_real.refs.load(Ordering::SeqCst),1);
            assert_eq!(staging_real.refs.load(Ordering::SeqCst),1);
            assert_eq!(back_real.refs.load(Ordering::SeqCst),1);
            assert_eq!(depth_real.refs.load(Ordering::SeqCst),1);
        }
    }
    #[test]
    fn focus_recovery_is_one_shot_and_repaint_region_edge_triggered(){
        let mut state=ExposureState::default();
        let focused=Some(GameVisibility{foreground:true,pending_paint:false});
        let unfocused=Some(GameVisibility{foreground:false,pending_paint:false});
        let pending=Some(GameVisibility{foreground:true,pending_paint:true});
        assert!(!state.sample(None));
        assert!(!state.sample(focused));
        assert!(!state.sample(unfocused));
        assert!(state.sample(focused)); // return from another foreground app
        assert!(!state.sample(focused));
        assert!(state.sample(pending)); // new Win32 paint region
        assert!(!state.sample(pending)); // no per-frame black erase
        assert!(!state.sample(focused));
        assert!(state.sample(pending)); // a distinct invalidation
    }
    #[test]
    fn no_focus_repaint_when_foreground_query_fails(){
        let mut state=ExposureState::default();
        assert!(!state.sample(Some(GameVisibility{foreground:false,pending_paint:true})));
        assert!(!state.sample(None));
        assert!(state.sample(Some(GameVisibility{foreground:true,pending_paint:false})));
    }
    #[test]
    fn all_45_slots_present_and_forwarding_identity_is_consistent(){
        assert_eq!(std::mem::size_of::<Surface4VTable>(),45*4);
        assert_eq!(std::mem::size_of::<usize>(),4);
        let table=mock_table();
        let mut real=real_mock(&table);
        let ptr=(&mut real as *mut RealMock).cast();
        unsafe{
            let facade=new_forwarding_surface4(ptr).unwrap();
            assert_ne!(facade,ptr);
            assert_eq!(real.refs.load(Ordering::SeqCst),2);
            let vt=&**facade.cast::<*const Surface4VTable>();
            let mut unknown=ptr::null_mut();
            let mut surface=ptr::null_mut();
            assert_eq!((vt.query_interface)(facade,&IID_IUNKNOWN,&mut unknown),S_OK);
            assert_eq!((vt.query_interface)(unknown,&IID_SURFACE4,&mut surface),S_OK);
            assert_eq!(unknown,facade);
            assert_eq!(surface,facade);
            let unsupported=Guid{d1:0x15e65ec0,d2:0x3b9c,d3:0x11d2,
               d4:[0xb9,0x2f,0,0x60,0x97,0x97,0xea,0x5b]};
            let mut out=facade;
            assert_eq!((vt.query_interface)(facade,&unsupported,&mut out),E_NOINTERFACE);
            assert!(out.is_null());
            assert_eq!((vt.release)(surface),2);
            assert_eq!((vt.release)(unknown),1);
            assert_eq!((vt.release)(facade),0);
        }
        // The wrapper kept precisely one underlying strong reference
        // and released it exactly once at facade refcount zero.
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
    }
    #[test]
    fn virtual_primary_lock_and_unlock_use_dedicated_rgb565_staging_not_desktop(){
        unsafe extern "system" fn mock_lock(
            this:SurfaceThis,_rect:*mut Rect,desc:*mut c_void,_flags:u32,_event:*mut c_void
        )->i32{
            let real=&*this.cast::<RealMock>();
            real.blts.fetch_add(1,Ordering::SeqCst);
            if !desc.is_null(){
                let d=desc.cast::<u32>();
                if *d!=124{return E_INVALIDARG;}
                *d.add(2)=480;*d.add(3)=640;*d.add(4)=1280;
                *d.add(9)=0x1234;*d.add(21)=16;
                *d.add(26)=0x840; // real OFFSCREEN+SYSTEMMEM caps
            }
            S_OK
        }
        unsafe extern "system" fn mock_unlock(this:SurfaceThis,_rect:*mut Rect)->i32{
            (&*this.cast::<RealMock>()).flips.fetch_add(1,Ordering::SeqCst);
            S_OK
        }
        let mut vt=mock_table();
        vt.lock=mock_lock;
        vt.unlock=mock_unlock;
        let mut desktop=real_mock(&vt);
        let mut staging=real_mock(&vt);
        let desktop_ptr=(&mut desktop as *mut RealMock).cast::<c_void>();
        let staging_ptr=(&mut staging as *mut RealMock).cast::<c_void>();
        unsafe{
            let primary=new_graph_surface4(desktop_ptr,SurfaceRole::Primary,
                Mode{width:640,height:480,bpp:16}).unwrap();
            let iface=&**primary.cast::<*const Surface4VTable>();
            let mut desc=[0u32;31];desc[0]=124;
            assert_eq!((iface.lock)(primary,ptr::null_mut(),desc.as_mut_ptr().cast(),
                0,ptr::null_mut()),E_NOTIMPL);
            assert_eq!(desktop.blts.load(Ordering::SeqCst),0);
            assert_eq!(arm_graph_primary_lock_surface(primary,ptr::null_mut()),Err(E_INVALIDARG));
            assert_eq!(arm_graph_primary_lock_surface(primary,desktop_ptr),Err(E_INVALIDARG));
            assert_eq!(arm_graph_primary_lock_surface(primary,staging_ptr),Ok(()));
            assert_eq!(arm_graph_primary_lock_surface(primary,staging_ptr),Err(E_FAIL));
            assert_eq!(staging.refs.load(Ordering::SeqCst),2); // one owned ref
            assert_eq!((iface.lock)(primary,ptr::null_mut(),desc.as_mut_ptr().cast(),
                0,ptr::null_mut()),S_OK);
            assert_eq!((desc[2],desc[3],desc[4],desc[9],desc[21]),
                (480,640,1280,0x1234,16));
            assert_eq!(desc[26],0x1000_e238); // virtual PRIMARY
            assert_eq!(staging.blts.load(Ordering::SeqCst),1);
            assert_eq!(desktop.blts.load(Ordering::SeqCst),0);
            assert_eq!((iface.unlock)(primary,ptr::null_mut()),S_OK);
            assert_eq!(staging.flips.load(Ordering::SeqCst),1);
            assert_eq!(desktop.flips.load(Ordering::SeqCst),0);
            assert_eq!((iface.release)(primary),0);
        }
        assert_eq!(desktop.refs.load(Ordering::SeqCst),1);
        assert_eq!(staging.refs.load(Ordering::SeqCst),1);
    }

    #[test]
    fn native_movie_fallback_accepts_both_observed_wait_variants(){
        assert!(native_movie_blt_flags_supported(0));
        assert!(native_movie_blt_flags_supported(DDBLT_WAIT));
        for flags in [1,2,0x200,0x400,0x0100_0200] {
            assert!(!native_movie_blt_flags_supported(flags));
        }
    }
    #[test]
    fn xrgb8888_to_rgb565_native_fallback_color_and_nearest_scaling(){
        assert_eq!(xrgb_to_565(0x00ff0000),0xf800);
        assert_eq!(xrgb_to_565(0x0000ff00),0x07e0);
        assert_eq!(xrgb_to_565(0x000000ff),0x001f);
        assert_eq!(xrgb_to_565(0x00ffffff),0xffff);
        let mut source=vec![0u8;320*240*4];
        for y in 0..240usize{
            for x in 0..320usize{
                let pixel=if x<160{0x00ff0000u32}else{0x0000ff00u32};
                source[(y*320+x)*4..(y*320+x+1)*4].copy_from_slice(&pixel.to_le_bytes());
            }
        }
        let mut out=vec![0u8;800*600*2];
        unsafe{scale_xrgb320_to_rgb565800(source.as_ptr(),1280,out.as_mut_ptr(),1600);}
        let at=|x:usize,y:usize|{
            u16::from_le_bytes(out[(y*800+x)*2..(y*800+x+1)*2].try_into().unwrap())
        };
        assert_eq!(at(0,0),0xf800);
        assert_eq!(at(399,599),0xf800);
        assert_eq!(at(400,0),0x07e0);
        assert_eq!(at(799,599),0x07e0);
    }
    #[test]
    fn opening_rgb565_to_xrgb8888_is_exact_and_pitch_safe(){
        assert_eq!(rgb565_to_xrgb8888(0xf800),0x00ff0000);
        assert_eq!(rgb565_to_xrgb8888(0x07e0),0x0000ff00);
        assert_eq!(rgb565_to_xrgb8888(0x001f),0x000000ff);
        assert_eq!(rgb565_to_xrgb8888(0xffff),0x00ffffff);
        let mut src=vec![0u8;600*1604];
        let mut dst=vec![0x5au8;600*3208];
        for y in 0..600usize{
            for x in 0..800usize{
                let px=if (x+y)%3==0{0xf800u16}else if (x+y)%3==1{
                    0x07e0
                }else{0x001f};
                let at=y*1604+x*2;
                src[at..at+2].copy_from_slice(&px.to_le_bytes());
            }
        }
        unsafe{convert_opening_rgb565_pixels(
            src.as_ptr(),1604,dst.as_mut_ptr(),3208
        );}
        for (x,y,color) in [(0usize,0usize,0xff0000u32),
             (1,0,0x00ff00),(2,0,0x0000ff),
             (0,599,0x0000ff),(799,599,0xff0000)] {
            let at=y*3208+x*4;
            assert_eq!(u32::from_le_bytes(dst[at..at+4].try_into().unwrap()),
                color,"pixel {x},{y}");
        }
        // No writes into row padding.
        assert!(dst.chunks_exact(3208).all(|row|
            row[3200..].iter().all(|b|*b==0x5a)));
    }
    #[test]
    fn native_blt_failure_rectangle_summary_is_bounded(){
        let mut rect=Rect{left:2,top:3,right:11,bottom:13};
        assert_eq!(rect_summary(ptr::null_mut()),"full");
        assert_eq!(rect_summary(&mut rect),"(2,3)-(11,13)");
    }

    #[test]
    fn game_backbuffer_native_texture_blt_is_forwarded_but_stale_or_wrong_graph_is_not(){
        let vt=mock_table();
        let mut primary_native=real_mock(&vt);
        let mut back_native=real_mock(&vt);
        let mut texture_native=real_mock(&vt);
        let primary_raw=(&mut primary_native as *mut RealMock).cast::<c_void>();
        let back_raw=(&mut back_native as *mut RealMock).cast::<c_void>();
        let texture_raw=(&mut texture_native as *mut RealMock).cast::<c_void>();
        let mode=Mode{width:640,height:480,bpp:16};
        unsafe{
            let back=new_graph_surface4(back_raw,SurfaceRole::Backbuffer,mode).unwrap();
            let primary=new_graph_surface4(primary_raw,SurfaceRole::Primary,mode).unwrap();
            let bv=&**back.cast::<*const Surface4VTable>();
            let rect=Rect{left:0,top:0,right:100,bottom:100};
            assert_eq!((bv.blt)(back,&rect as *const _ as *mut _,
                texture_raw,ptr::null_mut(),DDBLT_WAIT,ptr::null_mut()),S_OK);
            assert_eq!(back_native.blts.load(Ordering::SeqCst),1);
            // The wrong physical desktop primary cannot become a color source.
            assert_eq!((bv.blt)(back,ptr::null_mut(),primary,ptr::null_mut(),
                DDBLT_WAIT,ptr::null_mut()),E_NOTIMPL);
            assert_eq!((bv.blt)(back,ptr::null_mut(),texture_raw,ptr::null_mut(),
                DDBLT_COLORFILL,ptr::null_mut()),E_NOTIMPL);
            assert_eq!((bv.blt)(back,ptr::null_mut(),texture_raw,ptr::null_mut(),
                DDBLT_WAIT,ptr::null_mut()),S_OK);
            assert_eq!(back_native.blts.load(Ordering::SeqCst),2);
            // Passing a wrapped, same-epoch backbuffer must unwrap to a REAL COM surface.
            assert_eq!(native_copy_source(back,mode),Some(back_raw));
            assert_eq!(native_copy_source(back,Mode{width:800,height:600,bpp:16}),None);
            assert_eq!((bv.release)(back),0);
            assert_eq!(((&**primary.cast::<*const Surface4VTable>()).release)(primary),0);
        }
        assert_eq!(back_native.refs.load(Ordering::SeqCst),1);
        assert_eq!(primary_native.refs.load(Ordering::SeqCst),1);
        assert_eq!(texture_native.refs.load(Ordering::SeqCst),1);
    }
    #[test]
    fn every_forwarded_backend_call_uses_the_actual_real_surface(){
        let table=mock_table();
        let mut real=real_mock(&table);
        let ptr=(&mut real as *mut RealMock).cast();
        unsafe{
            let facade=new_forwarding_surface4(ptr).unwrap();
            let vt=&**facade.cast::<*const Surface4VTable>();
            assert_eq!((vt.blt)(facade,ptr::null_mut(),ptr::null_mut(),
                                ptr::null_mut(),0,ptr::null_mut()),S_OK);
            assert_eq!((vt.flip)(facade,ptr::null_mut(),0),S_OK);
            assert_eq!((vt.unlock)(facade,ptr::null_mut()),0x80004001u32 as i32);
            let mut out=ptr::null_mut();
            // Current passthrough GetAttachedSurface returns the *real*
            // attached pointer, NOT a virtual child. Never expose this
            // incomplete chain to the game.
            assert_eq!((vt.get_attached_surface)(facade,ptr::null_mut(),&mut out),S_OK);
            assert_eq!(out as usize,0x1234);
            assert_eq!((vt.release)(facade),0);
        }
        assert_eq!(real.blts.load(Ordering::SeqCst),1);
        assert_eq!(real.flips.load(Ordering::SeqCst),1);
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
    }
    #[test]
    fn three_real_com_wrappers_form_acyclic_client_visible_attachment_graph(){
        let table=mock_table();
        let mut physical=[
            real_mock(&table),real_mock(&table),real_mock(&table)
        ];
        let native=std::array::from_fn::<_,3,_>(|i|
            (&mut physical[i] as *mut RealMock).cast::<c_void>());
        let m=Mode{width:640,height:480,bpp:16};
        unsafe{
            let p=new_graph_surface4(native[0],SurfaceRole::Primary,m).unwrap();
            let b=new_graph_surface4(native[1],SurfaceRole::Backbuffer,m).unwrap();
            let z=new_graph_surface4(native[2],SurfaceRole::Depth,m).unwrap();
            attach_graph_surface4(p,b).unwrap();
            attach_graph_surface4(b,z).unwrap();
            // Graph edges own independent child references after callers
            // release their original backbuffer and depth pointers.
            let pv=&**p.cast::<*const Surface4VTable>();
            let bv=&**b.cast::<*const Surface4VTable>();
            let zv=&**z.cast::<*const Surface4VTable>();
            assert_eq!((zv.release)(z),1);
            assert_eq!((bv.release)(b),1);
            let mut back=ptr::null_mut();
            let mut back_cap=[4u32,0,0,0];
            assert_eq!((pv.get_attached_surface)(p,back_cap.as_mut_ptr().cast(),&mut back),S_OK);
            assert_eq!(back,b);
            let mut depth=ptr::null_mut();
            let mut depth_cap=[0x20000u32,0,0,0];
            assert_eq!((bv.get_attached_surface)(back,depth_cap.as_mut_ptr().cast(),&mut depth),S_OK);
            assert_eq!(depth,z);
            let mut unknown=ptr::null_mut();
            assert_eq!((zv.query_interface)(depth,&IID_IUNKNOWN,&mut unknown),S_OK);
            assert_eq!(unknown,z);
            assert_eq!((zv.release)(unknown),2);
            for (ptr,expected_caps,expected_bpp) in [
                (p,0x1000_e238u32,16u32),
                (back,0x1000_601cu32,16u32),
                (depth,0x1002_4000u32,32u32)]{
                let v=&**ptr.cast::<*const Surface4VTable>();
                let mut caps=[0u32;4];
                assert_eq!((v.get_caps)(ptr,caps.as_mut_ptr().cast()),S_OK);
                assert_eq!(caps[0],expected_caps);
                let mut d=[0u32;31];d[0]=124;
                assert_eq!((v.get_surface_desc)(ptr,d.as_mut_ptr().cast()),S_OK);
                assert_eq!((d[2],d[3],d[21],d[26]),(480,640,expected_bpp,expected_caps));
                let mut fmt=[0u32;8];fmt[0]=32;
                assert_eq!((v.get_pixel_format)(ptr,fmt.as_mut_ptr().cast()),S_OK);
                assert_eq!(fmt[3],expected_bpp);
                if expected_bpp==32{assert_eq!(fmt[5],0x00ff_ffff);}
            }
            assert_eq!((bv.add_attached_surface)(back,z),S_OK); // already native-attached
            assert_eq!((pv.add_attached_surface)(p,z),E_NOTIMPL);
            assert_eq!((bv.add_attached_surface)(back,p),E_NOTIMPL);
            assert_eq!((zv.release)(depth),1);
            assert_eq!((bv.release)(back),1);
            // Cascades ownership edges Primary->Backbuffer->Depth.
            assert_eq!((pv.release)(p),0);
        }
        for phys in &physical{
            assert_eq!(phys.refs.load(Ordering::SeqCst),1);
            assert_eq!(phys.flips.load(Ordering::SeqCst),0);
            assert_eq!(phys.blts.load(Ordering::SeqCst),0);
        }
    }
    #[test]
    fn graph_only_allows_physical_null_source_fills_and_retires_stale_depth_and_backbuffer(){
        use crate::{borderless_contract::{BackendEvidence,RenderTarget},presentation::Chain};
        let table=mock_table();
        let mut phys=[real_mock(&table),real_mock(&table),real_mock(&table)];
        let raw=std::array::from_fn::<_,3,_>(|i|
            (&mut phys[i] as *mut RealMock).cast::<c_void>());
        let m=Mode{width:640,height:480,bpp:16};
        let mut contract=AdapterContract::prepare(m,(1920,1080),
            Chain{front_to_back:true,back_to_depth:true,mipmaps:false}).unwrap();
        contract.expose(BackendEvidence{
            process_owned_window:true,normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:0x2000,render_target_is_rgb565:true,
            real_zbuffer_32:true,primary_with_clipper:true,
            working_blt:true,complete_surface_com:true,
        }).unwrap();
        unsafe{
            let p=new_graph_surface4(raw[0],SurfaceRole::Primary,m).unwrap();
            let b=new_graph_surface4(raw[1],SurfaceRole::Backbuffer,m).unwrap();
            let z=new_graph_surface4(raw[2],SurfaceRole::Depth,m).unwrap();
            attach_graph_surface4(p,b).unwrap();
            attach_graph_surface4(b,z).unwrap();
            let pv=&**p.cast::<*const Surface4VTable>();
            let bv=&**b.cast::<*const Surface4VTable>();
            let zv=&**z.cast::<*const Surface4VTable>();
            let mut fx=[0u32;25];fx[0]=100;fx[20]=0x07e0;
            let effects=fx.as_mut_ptr().cast();
            assert_eq!((pv.blt)(p,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_NOTIMPL);
            assert_eq!((bv.blt)(b,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL|DDBLT_WAIT,effects),S_OK);
            assert_eq!((zv.blt)(z,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_DEPTHFILL,effects),S_OK);
            assert_eq!((bv.blt)(b,ptr::null_mut(),z,
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_NOTIMPL);
            assert_eq!((bv.blt)(b,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_DEPTHFILL,effects),E_NOTIMPL);
            assert_eq!((zv.blt)(z,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_NOTIMPL);
            assert_eq!((bv.blt)(b,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,ptr::null_mut()),E_INVALIDARG);
            fx[0]=0;
            assert_eq!((bv.blt)(b,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_INVALIDARG);
            fx[0]=100;
            // The invalid rectangle, not the earlier zero-valued effects length,
            // must be what causes the following E_INVALIDARG result.
            assert_eq!(fx[0],100);
            let mut bad=Rect{left:0,top:0,right:641,bottom:480};
            assert_eq!((bv.blt)(b,&mut bad,ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_INVALIDARG);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),0);
            assert_eq!(phys[1].blts.load(Ordering::SeqCst),1);
            assert_eq!(phys[2].blts.load(Ordering::SeqCst),1);
            arm_graph_primary_flip(p,&contract,55,(0,0),(1920,1080),None,0,false,ptr::null_mut()).unwrap();
            assert_eq!(retire_graph_primary_flip(p),Ok(0));
            assert_eq!((bv.blt)(b,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_NOTIMPL);
            assert_eq!((zv.blt)(z,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_DEPTHFILL,effects),E_NOTIMPL);
            assert_eq!(phys[1].blts.load(Ordering::SeqCst),1);
            assert_eq!(phys[2].blts.load(Ordering::SeqCst),1);
            assert_eq!((pv.release)(p),0);
            assert_eq!((bv.release)(b),0);
            assert_eq!((zv.release)(z),0);
        }
        for phys in &phys{assert_eq!(phys.refs.load(Ordering::SeqCst),1);}
    }

    #[test]
    fn armed_virtual_flip_uses_actual_blt_slot_and_never_native_flip(){
        use crate::{
            borderless_contract::{BackendEvidence,RenderTarget},
            presentation::Chain,
        };
        let table=mock_table();
        let mut phys=[real_mock(&table),real_mock(&table),real_mock(&table)];
        let raw=std::array::from_fn::<_,3,_>(|i|
            (&mut phys[i] as *mut RealMock).cast::<c_void>());
        let mode=Mode{width:640,height:480,bpp:16};
        let mut contract=AdapterContract::prepare(
            mode,(1920,1080),
            Chain{front_to_back:true,back_to_depth:true,mipmaps:false}
        ).unwrap();
        // In this UNIT TEST ONLY the physical mock evidence is synthetic.
        // The standalone live test must first pass real HRESULT checks.
        contract.expose(BackendEvidence{
            process_owned_window:true,normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:0x2000,render_target_is_rgb565:true,
            real_zbuffer_32:true,primary_with_clipper:true,
            working_blt:true,complete_surface_com:true,
        }).unwrap();
        unsafe{
            let p=new_graph_surface4(raw[0],SurfaceRole::Primary,mode).unwrap();
            let b=new_graph_surface4(raw[1],SurfaceRole::Backbuffer,mode).unwrap();
            let z=new_graph_surface4(raw[2],SurfaceRole::Depth,mode).unwrap();
            attach_graph_surface4(p,b).unwrap();
            attach_graph_surface4(b,z).unwrap();
            let vt=&**p.cast::<*const Surface4VTable>();
            assert_eq!((vt.flip)(p,b,0),E_NOTIMPL);
            assert_eq!((vt.get_flip_status)(p,DDGFS_CANFLIP),E_FAIL);
            assert_eq!((vt.get_flip_status)(p,DDGFS_ISFLIPDONE),E_FAIL);
            assert_eq!(graph_primary_completed_flips(p),None);
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(960,540),None,0,false,ptr::null_mut()),Err(E_INVALIDARG));
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(1920,1080),None,0,false,ptr::null_mut()),Ok(()));
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(1920,1080),None,0,false,ptr::null_mut()),Err(E_FAIL));
            assert_eq!((vt.get_flip_status)(p,DDGFS_CANFLIP),S_OK);
            assert_eq!((vt.get_flip_status)(p,DDGFS_ISFLIPDONE),S_OK);
            assert_eq!((vt.get_flip_status)(p,0),E_INVALIDARG);
            assert_eq!((vt.get_flip_status)(p,3),E_INVALIDARG);
            assert_eq!((vt.flip)(p,z,0),E_INVALIDARG);
            assert_eq!((vt.flip)(p,b,1),E_INVALIDARG);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),0);
            assert_eq!((vt.flip)(p,b,0),S_OK);
            assert_eq!((vt.flip)(p,ptr::null_mut(),0),S_OK);
            assert_eq!(graph_primary_completed_flips(p),Some(2));
            assert_eq!((vt.get_flip_status)(p,DDGFS_ISFLIPDONE),S_OK);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),3);
            assert_eq!(phys[0].flips.load(Ordering::SeqCst),0);
            assert_eq!(phys[1].flips.load(Ordering::SeqCst),0);
            assert_eq!(retire_graph_primary_flip(p),Ok(2));
            assert_eq!((vt.get_flip_status)(p,DDGFS_CANFLIP),E_FAIL);
            assert_eq!((vt.get_flip_status)(p,DDGFS_ISFLIPDONE),E_FAIL);
            assert_eq!(graph_primary_completed_flips(p),None);
            assert_eq!((vt.flip)(p,b,0),E_NOTIMPL);
            assert_eq!((vt.flip)(p,ptr::null_mut(),0),E_NOTIMPL);
            assert_eq!(retire_graph_primary_flip(p),Err(E_FAIL));
            assert_eq!(arm_graph_primary_flip(p,&contract,18,(0,0),(1920,1080),None,0,false,ptr::null_mut()),Err(E_FAIL));
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),3); // no stale writes
            assert_eq!((vt.release)(p),0); // cascades strong child refs
            assert_eq!((vt.release)(b),0);
            assert_eq!((vt.release)(z),0);
        }
        for phys in &phys{
            assert_eq!(phys.refs.load(Ordering::SeqCst),1);
            assert_eq!(phys.flips.load(Ordering::SeqCst),0);
        }
    }

    #[test]
    fn retirement_waits_for_inflight_flip_and_disarms_old_primary() {
        // This is a synthetic native COM graph, not a real game or GPU.
        // The first native Blt deliberately runs while retirement starts.
        // Both paths must share the per-primary virtual_flip mutex.
        use crate::{
            borderless_contract::{BackendEvidence,RenderTarget},
            presentation::Chain,
        };
        use std::{sync::Arc,thread,time::{Duration,Instant}};
        // Handshake the simulated native Blt instead of trusting a sleep:
        // retirement is explicitly attempted while this callback is blocked.
        static BLT_ENTERED:AtomicBool=AtomicBool::new(false);
        static BLT_CAN_RETURN:AtomicBool=AtomicBool::new(false);
        BLT_ENTERED.store(false,Ordering::SeqCst);
        BLT_CAN_RETURN.store(false,Ordering::SeqCst);
        unsafe extern "system" fn blocking_native_blt(
            this:SurfaceThis,_dst:*mut Rect,_source:SurfaceThis,_src:*mut Rect,
            _flags:u32,_effects:*mut c_void
        )->i32{
            let real=&*this.cast::<RealMock>();
            if real.blts.fetch_add(1,Ordering::SeqCst)==0{
                BLT_ENTERED.store(true,Ordering::SeqCst);
                let deadline=Instant::now()+Duration::from_secs(5);
                while !BLT_CAN_RETURN.load(Ordering::SeqCst) && Instant::now()<deadline{
                    thread::yield_now();
                }
                if !BLT_CAN_RETURN.load(Ordering::SeqCst){return E_FAIL;}
            }
            S_OK
        }
        let mut table=mock_table();
        table.blt=blocking_native_blt;
        let mut phys=[real_mock(&table),real_mock(&table),real_mock(&table)];
        let raw=std::array::from_fn::<_,3,_>(|i|
            (&mut phys[i] as *mut RealMock).cast::<c_void>());
        let mode=Mode{width:640,height:480,bpp:16};
        let mut contract=AdapterContract::prepare(
            mode,(1920,1080),Chain{front_to_back:true,back_to_depth:true,mipmaps:false}
        ).unwrap();
        contract.expose(BackendEvidence{
            process_owned_window:true,normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:0x2000,render_target_is_rgb565:true,
            real_zbuffer_32:true,primary_with_clipper:true,
            working_blt:true,complete_surface_com:true,
        }).unwrap();
        unsafe {
            let primary=new_graph_surface4(raw[0],SurfaceRole::Primary,mode).unwrap();
            let back=new_graph_surface4(raw[1],SurfaceRole::Backbuffer,mode).unwrap();
            let depth=new_graph_surface4(raw[2],SurfaceRole::Depth,mode).unwrap();
            attach_graph_surface4(primary,back).unwrap();
            attach_graph_surface4(back,depth).unwrap();
            arm_graph_primary_flip(primary,&contract,88,(0,0),(1920,1080),
                None,0,false,ptr::null_mut()).unwrap();
            let vt=&**primary.cast::<*const Surface4VTable>();
            // Worker owns its own COM reference throughout its in-flight Flip.
            assert_eq!((vt.add_ref)(primary),2);
            let primary_addr=primary as usize;
            let back_addr=back as usize;
            let worker=thread::spawn(move ||{
                let p=primary_addr as SurfaceThis;
                let b=back_addr as SurfaceThis;
                let v=&**p.cast::<*const Surface4VTable>();
                let hr=(v.flip)(p,b,0);
                let remaining=(v.release)(p);
                (hr,remaining)
            });
            let deadline=Instant::now()+Duration::from_secs(5);
            while !BLT_ENTERED.load(Ordering::SeqCst) &&
                  Instant::now()<deadline {
                thread::yield_now();
            }
            // Never enter the retirement lock before the first native Blt.
            if !BLT_ENTERED.load(Ordering::SeqCst){
                BLT_CAN_RETURN.store(true,Ordering::SeqCst);
                let _=worker.join();
                panic!("synthetic native Flip did not reach its first Blt");
            }
            // The virtual flip is in the real Blt callback: ISFLIPDONE
            // must describe it as pending, without taking the held mutex
            // or trusting the unrelated native primary GetFlipStatus.
            assert_eq!((vt.get_flip_status)(primary,DDGFS_ISFLIPDONE),
                DDERR_WASSTILLDRAWING);
            assert_eq!((vt.get_flip_status)(primary,DDGFS_CANFLIP),
                DDERR_WASSTILLDRAWING);
            let retirement_started=Arc::new(AtomicBool::new(false));
            let retirement_finished=Arc::new(AtomicBool::new(false));
            let started=Arc::clone(&retirement_started);
            let finished=Arc::clone(&retirement_finished);
            let retired_primary=primary as usize;
            let retiring=thread::spawn(move ||{
                started.store(true,Ordering::SeqCst);
                let result=retire_graph_primary_flip(retired_primary as SurfaceThis);
                finished.store(true,Ordering::SeqCst);
                result
            });
            while !retirement_started.load(Ordering::SeqCst) && Instant::now()<deadline{
                thread::yield_now();
            }
            // Let the retirement worker attempt the lock, while the native
            // callback is held by our explicit gate; never strand threads
            // even if the assertion below detects an unexpected regression.
            thread::sleep(Duration::from_millis(25));
            let finished_while_native_blt_blocked=retirement_finished.load(Ordering::SeqCst);
            BLT_CAN_RETURN.store(true,Ordering::SeqCst);
            let retired=retiring.join().unwrap();
            let flipped=worker.join().unwrap();
            assert!(!finished_while_native_blt_blocked,
                "old generation retired while a native Blt remained blocked");
            assert_eq!(retired,Ok(1));
            assert_eq!(flipped,(S_OK,1));
            let blts_after_retirement=phys[0].blts.load(Ordering::SeqCst);
            assert!(blts_after_retirement>0);
            assert_eq!((vt.flip)(primary,back,0),E_NOTIMPL);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),blts_after_retirement);
            assert_eq!(retire_graph_primary_flip(primary),Err(E_FAIL));
            assert_eq!((vt.release)(primary),0);
            assert_eq!(((&**back.cast::<*const Surface4VTable>()).release)(back),0);
            assert_eq!(((&**depth.cast::<*const Surface4VTable>()).release)(depth),0);
        }
        for item in &phys {assert_eq!(item.refs.load(Ordering::SeqCst),1);}
    }

    #[test]
    fn virtual_graph_rejects_native_flip_blt_invalid_attachment_and_mutation(){
        let table=mock_table();
        let mut real=real_mock(&table);
        let native=(&mut real as *mut RealMock).cast::<c_void>();
        unsafe{
            assert_eq!(new_graph_surface4(native,SurfaceRole::Primary,
                Mode{width:1920,height:1080,bpp:16}),Err(E_INVALIDARG));
            let p=new_graph_surface4(native,SurfaceRole::Primary,
                Mode{width:640,height:480,bpp:16}).unwrap();
            let vt=&**p.cast::<*const Surface4VTable>();
            assert_eq!((vt.flip)(p,ptr::null_mut(),0),E_NOTIMPL);
            assert_eq!((vt.blt)(p,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),0,ptr::null_mut()),E_NOTIMPL);
            assert_eq!((vt.lock)(p,ptr::null_mut(),ptr::null_mut(),
                0,ptr::null_mut()),E_NOTIMPL);
            assert_eq!((vt.add_attached_surface)(p,ptr::null_mut()),E_NOTIMPL);
            let mut out=p;
            let mut wrong_caps=[0x20000u32,0,0,0];
            assert_eq!((vt.get_attached_surface)(p,wrong_caps.as_mut_ptr().cast(),
                &mut out),E_INVALIDARG);
            assert!(out.is_null());
            let mut right_caps=[4u32,0,0,0];
            assert_eq!((vt.get_attached_surface)(p,right_caps.as_mut_ptr().cast(),
                &mut out),E_FAIL);
            assert!(out.is_null());
            let mut fmt=[0u32;8];fmt[0]=16;
            assert_eq!((vt.get_pixel_format)(p,fmt.as_mut_ptr().cast()),E_INVALIDARG);
            assert_eq!((vt.release)(p),0);
        }
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
        assert_eq!(real.flips.load(Ordering::SeqCst),0);
        assert_eq!(real.blts.load(Ordering::SeqCst),0);
    }

    #[test]
    fn native_d3d_boundary_can_borrow_only_our_own_live_wrapper(){
        let table=mock_table();
        let mut real=real_mock(&table);
        let physical=(&mut real as *mut RealMock).cast();
        unsafe{
            assert_eq!(try_borrow_real_surface4(ptr::null_mut()),None);
            assert_eq!(try_borrow_real_surface4(physical),None);
            let wrapper=new_forwarding_surface4(physical).unwrap();
            assert_ne!(wrapper,physical);
            assert_eq!(try_borrow_real_surface4(wrapper),Some(physical));
            let vt=&**wrapper.cast::<*const Surface4VTable>();
            let mut alias=ptr::null_mut();
            assert_eq!((vt.query_interface)(wrapper,&IID_IUNKNOWN,&mut alias),S_OK);
            assert_eq!(try_borrow_real_surface4(alias),Some(physical));
            assert_eq!((vt.release)(alias),1);
            assert_eq!((vt.release)(wrapper),0);
        }
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
    }
    #[test]
    fn new_forwarder_rejects_null_without_touching_real_com(){
        assert_eq!(unsafe{new_forwarding_surface4(ptr::null_mut())},Err(E_POINTER));
    }
    #[test]
    fn query_interface_handles_null_arguments_without_new_refs(){
        let table=mock_table();
        let mut real=real_mock(&table);
        unsafe{
            let facade=new_forwarding_surface4((&mut real as *mut RealMock).cast()).unwrap();
            let vt=&**facade.cast::<*const Surface4VTable>();
            assert_eq!((vt.query_interface)(facade,&IID_SURFACE4,ptr::null_mut()),E_POINTER);
            let mut out=facade;
            assert_eq!((vt.query_interface)(facade,ptr::null(),&mut out),E_POINTER);
            assert!(out.is_null());
            assert_eq!((vt.release)(facade),0);
        }
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
    }
}

#[cfg(test)]
mod frame_time_telemetry_tests{
    use super::*;
    #[test]
    fn regular_60fps_virtual_flips_show_no_hitch(){
        let mut t=FrameTimeWindow::default();
        let base=Instant::now();
        assert!(t.observe(base,Duration::from_millis(1),
            Mode{width:1920,height:1080,bpp:16},Some(60)).is_none());
        let mut result=None;
        for n in 1..=120u64{
            result=t.observe(base+Duration::from_micros(n*16_667),
                Duration::from_millis(1),
                Mode{width:1920,height:1080,bpp:16},Some(60));
        }
        let report=result.unwrap();
        assert!(report.contains("samples=120"));
        assert!(report.contains("slow_2x=0 slow_3x=0"));
        assert!(report.contains("mode=1920x1080x16"));
        assert!(report.contains("p95_ms=16.667"));
    }
    #[test]
    fn one_50ms_hitch_counts_only_at_window_summary(){
        let mut t=FrameTimeWindow::default();
        let mode=Mode{width:1920,height:1080,bpp:16};
        let mut timestamp=Instant::now();
        assert!(t.observe(timestamp,Duration::from_millis(1),mode,Some(60)).is_none());
        let mut report=None;
        for n in 1..=120{
            timestamp+=if n==45{Duration::from_millis(50)}
                else{Duration::from_micros(16_667)};
            report=t.observe(timestamp,Duration::from_millis(1),mode,Some(60));
        }
        let report=report.unwrap();
        assert!(report.contains("slow_2x=1 slow_3x=1"));
        assert!(report.contains("max_ms=50.000"));
    }
    #[test]
    fn focus_pause_is_not_counted_as_frame_stutter(){
        let mut t=FrameTimeWindow::default();
        let mode=Mode{width:800,height:600,bpp:16};
        let base=Instant::now();
        t.observe(base,Duration::ZERO,mode,Some(60));
        assert!(t.observe(base+Duration::from_secs(5),
            Duration::ZERO,mode,Some(60)).is_none());
        let mut r=None;
        for n in 1..=120{
            r=t.observe(base+Duration::from_secs(5)+
                Duration::from_micros(n*16_667),
                Duration::ZERO,mode,Some(60));
        }
        let r=r.unwrap();
        assert!(r.contains("ignored_pauses=1"));
        assert!(r.contains("slow_2x=0 slow_3x=0"));
    }
}
