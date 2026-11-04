//! Standalone Windows x86 DirectDraw4/Direct3D3 windowed resource probe.
//!
//! Run outside the game directory. It NEVER changes the desktop display
//! mode, installs a proxy hook, opens the game executable, or modifies files.
//! It creates its own test window, a normal-mode DirectDraw primary/clipper,
//! a *real* 640x480 RGB565 3D-capable offscreen surface, and a 32-bit Z.
//! It exercises Blt from the test offscreen buffer to its own window and
//! HAL IDirect3D3::CreateDevice. HRESULTs report actual backend support.
//!
//! The probe is NOT a replacement IDirectDrawSurface implementation and
//! does not claim to provide Crimson Skies borderless yet.
#![cfg(windows)]

// Share the pure Flip-to-Blt operation with the game-side transaction.
// No virtual game COM surface is created or exposed by this test executable.
#[path="../presentation.rs"]
mod presentation;
#[path="../borderless_contract.rs"]
mod borderless_contract;
#[path="../borderless_flip.rs"]
mod borderless_flip;
#[path="../borderless_native_blt.rs"]
mod borderless_native_blt;
#[path="../com_surface4_abi.rs"]
mod com_surface4_abi;
#[path="../window_exposure.rs"]
mod window_exposure;
#[path="../window_erase_guard.rs"]
mod window_erase_guard;
#[path="../color_capture.rs"]
mod color_capture;
#[path="../color_probe.rs"]
mod color_probe;
#[path="../com_surface4_forward.rs"]
mod com_surface4_forward;
#[path="../com_surface4_roundtrip.rs"]
mod com_surface4_roundtrip;
use borderless_flip::WindowedBlt;

use std::{
    ffi::c_void,
    fs::{File,OpenOptions},
    io::Write,
    path::{Path,PathBuf},
    ptr,
    sync::{Mutex,OnceLock},
};

const LOG_FILE_NAME:&str="windowed_ddraw_probe.log";
static PROBE_LOG:OnceLock<Mutex<File>>=OnceLock::new();

/// The log belongs to the standalone executable's directory, not the
/// shell working directory (which may be Crimson Skies' game folder).
fn log_path_for_executable(executable:&Path)->Result<PathBuf,String>{
    let directory=executable.parent().filter(|p|!p.as_os_str().is_empty())
        .ok_or_else(||"Cannot find probe executable directory".to_owned())?;
    Ok(directory.join(LOG_FILE_NAME))
}

/// Open a fresh log for every invocation, before any DirectDraw API call.
/// Only the probe's own lines go into this file: Wine's stderr stays out.
fn init_probe_log()->Result<PathBuf,String>{
    let exe=std::env::current_exe()
        .map_err(|e|format!("Cannot locate probe executable: {e}"))?;
    let path=log_path_for_executable(&exe)?;
    let file=OpenOptions::new().create(true).write(true).truncate(true)
        .open(&path)
        .map_err(|e|format!("Cannot create {}: {e}",path.display()))?;
    PROBE_LOG.set(Mutex::new(file))
        .map_err(|_|"Probe logger was already initialized".to_string())?;
    Ok(path)
}

/// Write and flush each line immediately. An early HRESULT failure thus
/// leaves a usable report; do not depend on stdout/Wine stderr redirection.
fn probe_line(line:&str){
    if let Some(log)=PROBE_LOG.get(){
        match log.lock(){
            Ok(mut file)=>{
                if let Err(err)=writeln!(file,"{line}").and_then(|_|file.flush()){
                    eprintln!("PROBE LOG WRITE ERROR: {err}");
                }
            },
            Err(_)=>eprintln!("PROBE LOG LOCK ERROR"),
        }
    }
    println!("{line}");
}

// The shared virtual Surface4 module expects a crate-level logger.
// In the DLL this writes CrimsonSkiesFix.log; the standalone x86 probe
// deliberately records the same diagnostics only to its OWN probe log.
fn log(message:&str){
    probe_line(message);
}
// The standalone probe has no game process or MainMenu state. The
// shared virtual Surface4 module's optional menu drawing hook is inert.
mod menu_video_probe{
    pub fn observe_ui_flip(_width:u32,_height:u32){} // standalone probe has no game executable
}
mod menu_brand {
    use std::ffi::c_void;
    // Standalone DD4 surface probe has no Crimson Skies script or
    // game-owned MainMenu artwork. All branding is deliberately inert.
    pub fn should_probe_menu_frame(_width:u32,_height:u32)->bool{false}
    pub fn mark_not_drawn(){}
    pub fn note_branded_presented(){}
    pub fn report_copy_failure(_hr:i32){}
    pub fn draw_main_menu(_surface:*mut c_void,_viewport:super::borderless_native_blt::Rect)->bool{false}
}

// The standalone surface probe never locks RGB32 movie surfaces.
fn color_probe_enabled()->bool{false}
fn color_capture_enabled()->bool{false}
fn color_capture_path(_label:&str)->Option<std::path::PathBuf>{None}


type Ptr=*mut c_void;
type Hr=i32;
const DDSCL_NORMAL:u32=0x0000_0008;
const DDSD_CAPS:u32=1;
const DDSD_HEIGHT:u32=2;
const DDSD_WIDTH:u32=4;
const DDSD_PIXELFORMAT:u32=0x1000;
const DDSCAPS_PRIMARY:u32=0x200;
const DDSCAPS_OFFSCREEN:u32=0x40;
const DDSCAPS_3DDEVICE:u32=0x2000;
const DDSCAPS_VIDEOMEMORY:u32=0x4000;
const DDSCAPS_ZBUFFER:u32=0x20000;
const DDPF_RGB:u32=0x40;
const DDPF_ZBUFFER:u32=0x400;
const DDPF_STENCILBUFFER:u32=0x4000;
const DDENUMRET_OK:Hr=1;
const DDBLT_COLORFILL:u32=0x400;
const DDBLT_DEPTHFILL:u32=0x0200_0000;
const DDBLT_WAIT:u32=0x0100_0000;
const WS_OVERLAPPEDWINDOW:u32=0x00cf_0000;
const WS_POPUP:u32=0x8000_0000;
const WS_VISIBLE:u32=0x1000_0000;
const SM_CXSCREEN:i32=0;
const SM_CYSCREEN:i32=1;
#[repr(C)]
#[derive(Clone,Copy)]
struct Guid {d1:u32,d2:u16,d3:u16,d4:[u8;8]}
// DirectDrawCreateEx MUST receive IID_IDirectDraw7 (Microsoft ddraw.h).
// Retrieve the legacy DD4 interface by QueryInterface afterwards.
const IID_IDIRECTDRAW7:Guid=Guid{
    d1:0x15e65ec0,d2:0x3b9c,d3:0x11d2,
    d4:[0xb9,0x2f,0x00,0x60,0x97,0x97,0xea,0x5b],
};
const IID_IDIRECTDRAW4:Guid=Guid{
    d1:0x9c59509a,d2:0x39bd,d3:0x11d1,
    d4:[0x8c,0x4a,0x00,0xc0,0x4f,0xd9,0x30,0xc5],
};
const IID_IDIRECT3D3:Guid=Guid{
    d1:0xbb223240,d2:0xe72b,d3:0x11d0,
    d4:[0xa9,0xb4,0x00,0xaa,0x00,0xc0,0x99,0x3e],
};
const CLSID_D3D_HAL:Guid=Guid{
    d1:0x84e63de0,d2:0x46aa,d3:0x11cf,
    d4:[0x81,0x6f,0x00,0x00,0xc0,0x20,0x15,0x6e],
};
#[repr(C)]
#[derive(Default)]
struct Point{x:i32,y:i32}
#[repr(C)]
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
struct Rect{left:i32,top:i32,right:i32,bottom:i32}
#[link(name="ddraw")]
extern "system" {
    fn DirectDrawCreateEx(adapter:*const Guid,out:*mut Ptr,iid:*const Guid,outer:Ptr)->Hr;
}
#[link(name="user32")]
extern "system" {
    fn CreateWindowExW(ex_style:u32,class:*const u16,name:*const u16,
        style:u32,x:i32,y:i32,width:i32,height:i32,
        parent:Ptr,menu:Ptr,instance:Ptr,param:Ptr)->Ptr;
    fn DestroyWindow(hwnd:Ptr)->i32;
    fn GetSystemMetrics(index:i32)->i32;
    fn ClientToScreen(hwnd:Ptr,point:*mut Point)->i32;
    fn GetClientRect(hwnd:Ptr,rect:*mut Rect)->i32;
    fn AdjustWindowRect(rect:*mut Rect,style:u32,has_menu:i32)->i32;
    fn UpdateWindow(hwnd:Ptr)->i32;
}
#[link(name="kernel32")]
extern "system" {
    fn Sleep(ms:u32);
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum ProbeMode { Windowed, Borderless, BorderlessLoop, ComForward, ComDevice, ComDeviceBridge, ComDeviceRoundtrip, ComSurfaceChain, ComVirtualFlip, ComVirtualFlip800, ComVirtualFlipTransition }
impl ProbeMode {
    fn borderless(self)->bool{matches!(self,Self::Borderless|Self::BorderlessLoop|Self::ComForward|Self::ComDevice|Self::ComDeviceBridge|Self::ComDeviceRoundtrip|Self::ComSurfaceChain|Self::ComVirtualFlip|Self::ComVirtualFlip800|Self::ComVirtualFlipTransition)}
    fn virtual_flip(self)->bool{matches!(self,Self::ComVirtualFlip|Self::ComVirtualFlip800|Self::ComVirtualFlipTransition)}
    fn source_mode(self)->presentation::Mode{
        let (width,height)=if self==Self::ComVirtualFlip800{(800,600)}else{(640,480)};
        presentation::Mode{width,height,bpp:16}
    }
}

/// Three independently owned graphics epochs reuse one HWND, DD4 and clipper.
/// This intentionally does not imitate the game's SetDisplayMode/COM hooks.
fn probe_source_modes(mode:ProbeMode)->Vec<presentation::Mode>{
    if mode==ProbeMode::ComVirtualFlipTransition{
        vec![
            presentation::Mode{width:800,height:600,bpp:16},
            presentation::Mode{width:640,height:480,bpp:16},
            presentation::Mode{width:800,height:600,bpp:16},
        ]
    }else{vec![mode.source_mode()]}
}

fn parse_mode(args:&[String])->Result<ProbeMode,String>{
    match args {
        [] => Ok(ProbeMode::Windowed),
        [arg] if arg=="--borderless" => Ok(ProbeMode::Borderless),
        [arg] if arg=="--borderless-loop" => Ok(ProbeMode::BorderlessLoop),
        [arg] if arg=="--com-forward" => Ok(ProbeMode::ComForward),
        [arg] if arg=="--com-device" => Ok(ProbeMode::ComDevice),
        [arg] if arg=="--com-device-bridge" => Ok(ProbeMode::ComDeviceBridge),
        [arg] if arg=="--com-device-roundtrip" => Ok(ProbeMode::ComDeviceRoundtrip),
        [arg] if arg=="--com-surface-chain" => Ok(ProbeMode::ComSurfaceChain),
        [arg] if arg=="--com-virtual-flip" => Ok(ProbeMode::ComVirtualFlip),
        [arg] if arg=="--com-virtual-flip-800" => Ok(ProbeMode::ComVirtualFlip800),
        [arg] if arg=="--com-virtual-flip-transition" => Ok(ProbeMode::ComVirtualFlipTransition),
        _ => Err("Usage: windowed_ddraw_probe.exe [--borderless|--borderless-loop|--com-forward|--com-device|--com-device-bridge|--com-device-roundtrip|--com-surface-chain|--com-virtual-flip|--com-virtual-flip-800|--com-virtual-flip-transition]".to_owned())
    }
}

/// Primary desktop is anchored at (0,0). A temporary WS_POPUP window
/// fills only this monitor and does NOT modify the monitor resolution.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
struct TestWindowPlan{
    style:u32,x:i32,y:i32,width:i32,height:i32,
    client_width:u32,client_height:u32
}
fn plan_test_window(mode:ProbeMode,desktop_w:i32,desktop_h:i32)
    ->Result<TestWindowPlan,String>{
    if desktop_w<=0||desktop_h<=0{
        return Err("Desktop dimensions unavailable".into());
    }
    match mode{
        ProbeMode::Windowed => Ok(TestWindowPlan {
            style:WS_VISIBLE|WS_OVERLAPPEDWINDOW,
            x:80,y:80,width:960,height:540,
            client_width:960,client_height:540,
        }),
        ProbeMode::Borderless|ProbeMode::BorderlessLoop|ProbeMode::ComForward|ProbeMode::ComDevice|ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip|ProbeMode::ComSurfaceChain|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition=>Ok(TestWindowPlan{
            style:WS_VISIBLE|WS_POPUP,
            x:0,y:0,width:desktop_w,height:desktop_h,
            client_width:desktop_w as u32,client_height:desktop_h as u32,
        }),
    }
}

/// Integer aspect fit; source must not be stretched out of proportion.
fn letterbox(src_w:u32,src_h:u32,dst_w:u32,dst_h:u32)->Option<Rect>{
    if src_w==0||src_h==0||dst_w==0||dst_h==0{return None;}
    let sw=u64::from(src_w);let sh=u64::from(src_h);
    let dw=u64::from(dst_w);let dh=u64::from(dst_h);
    let (w,h)=if dw*sh <= dh*sw {(dw,(dw*sh/sw).max(1))}
              else {((dh*sw/sh).max(1),dh)};
    let x=(dw-w)/2;let y=(dh-h)/2;
    Some(Rect{
        left:i32::try_from(x).ok()?,top:i32::try_from(y).ok()?,
        right:i32::try_from(x+w).ok()?,bottom:i32::try_from(y+h).ok()?
    })
}

/// Owns one COM reference and releases it exactly once when leaving scope.
/// No virtual surface is advertised and no borrowed COM pointer is retained.
struct Com(Ptr);
impl Drop for Com {
    fn drop(&mut self){
        if !self.0.is_null(){
            unsafe{
                let release:unsafe extern "system" fn(Ptr)->u32=std::mem::transmute(slot(self.0,2));
                let _=release(self.0);
            }
            self.0=ptr::null_mut();
        }
    }
}
struct Window(Ptr);
impl Drop for Window{
    fn drop(&mut self){
        if !self.0.is_null(){unsafe{let _=DestroyWindow(self.0);}}
    }
}
/// Read a vtable slot from a live COM object, never from an arbitrary address.
unsafe fn slot(object:Ptr,index:usize)->usize{
    let vtable=*(object.cast::<*const usize>());
    *vtable.add(index)
}
fn check(label:&str,hr:Hr)->Result<(),String>{
    probe_line(&format!("{label}: HRESULT={:#010x} {}",hr as u32,if hr>=0{"OK"}else{"FAIL"}));
    if hr<0{Err(format!("{label} returned {:#010x}",hr as u32))}else{Ok(())}
}
/// Windows x86 DDSURFACEDESC2: 31 DWORDs, with DDPIXELFORMAT at 18,
/// and DDSCAPS2 at 26. No 64-bit process may use this packed layout.
fn desc(caps:u32,width:u32,height:u32,bits:Option<u32>)->[u32;31]{
    let mut d=[0u32;31];
    d[0]=124;
    d[1]=DDSD_CAPS;
    d[26]=caps;
    if width>0 && height>0{
        d[1]|=DDSD_HEIGHT|DDSD_WIDTH;
        d[2]=height;d[3]=width;
    }
    if let Some(bpp)=bits {
        d[1]|=DDSD_PIXELFORMAT;
        d[18]=32;
        d[19]=if caps&DDSCAPS_ZBUFFER!=0{DDPF_ZBUFFER}else{DDPF_RGB};
        d[21]=bpp;
        if caps&DDSCAPS_ZBUFFER!=0 && bpp==32 {
            // Win32 DDPIXELFORMAT starts at DDSURFACEDESC2 DWORD 18.
            // dwZBitMask aliases dwGBitMask at pixel-format DWORD 5,
            // i.e. DDSURFACEDESC2 DWORD 23, NOT dwRBitMask at 22.
            // The 0.1.28 probe incorrectly wrote 0xffffffff to 22.
            d[23]=0xffff_ffff;
        }else if bpp==16 && caps&DDSCAPS_ZBUFFER==0{
            d[22]=0xf800;d[23]=0x07e0;d[24]=0x001f;
        }
    }
    d
}
unsafe fn create_surface(ddraw:&Com,name:&str,mut description:[u32;31])
    ->Result<Com,String>
{
    let f:unsafe extern "system" fn(Ptr,*mut u32,*mut Ptr,Ptr)->Hr=
        std::mem::transmute(slot(ddraw.0,6));
    let mut out=ptr::null_mut();
    check(name,f(ddraw.0,description.as_mut_ptr(),&mut out,ptr::null_mut()))?;
    if out.is_null(){return Err(format!("{name} returned a NULL COM pointer"));}
    Ok(Com(out))
}

/// IDirect3D3::EnumZBufferFormats ABI callback: DDPIXELFORMAT
/// has exactly 8 DWORDs on Windows x86; the depth mask is DWORD 5.
/// Never retain the callback's borrowed pointer after enumeration.
unsafe extern "system" fn capture_z_format(format:*const u32,context:Ptr)->Hr {
    if format.is_null() || context.is_null(){return DDENUMRET_OK;}
    if *format!=32{return DDENUMRET_OK;}
    let formats=&mut *(context as *mut Vec<[u32;8]>);
    if formats.len()>=32{return DDENUMRET_OK;}
    let mut words=[0u32;8];
    ptr::copy_nonoverlapping(format,words.as_mut_ptr(),8);
    formats.push(words);
    DDENUMRET_OK
}
/// Use the actual Direct3D3 HAL formats to avoid guessing a Z layout.
/// Enumeration is advisory: failed/empty enumeration is logged and the
/// corrected strict Z32 descriptor is still tried, without Z16 fallback.
unsafe fn enumerate_z_formats(d3d:&Com)->Vec<[u32;8]> {
    type Callback=unsafe extern "system" fn(*const u32,Ptr)->Hr;
    let enumerate:unsafe extern "system" fn(Ptr,*const Guid,Callback,Ptr)->Hr=
        std::mem::transmute(slot(d3d.0,10)); // IDirect3D3 EnumZBufferFormats
    let mut formats=Vec::<[u32;8]>::new();
    let hr=enumerate(d3d.0,&CLSID_D3D_HAL,capture_z_format,
                     (&mut formats as *mut Vec<[u32;8]>).cast());
    let _=check("IDirect3D3.EnumZBufferFormats(HAL)",hr);
    for (i,p) in formats.iter().enumerate(){
        probe_line(&format!(
            "Z-FORMAT[{i}]: flags={:#010x} bits={} stencil_bits={} zmask={:#010x} stencil_mask={:#010x}",
            p[1],p[3],p[4],p[5],p[6]));
    }
    probe_line(&format!(
        "Z-FORMAT-SUMMARY: enumerated={} pure_32bit={}",
        formats.len(),formats.iter().filter(|p|pure_z32(p)).count()));
    formats
}
fn pure_z32(p:&[u32;8])->bool{
    p[0]==32 && p[1]&DDPF_ZBUFFER!=0 &&
    p[1]&DDPF_STENCILBUFFER==0 && p[3]==32 && p[5]!=0
}
fn select_pure_z32(formats:&[[u32;8]])->Option<[u32;8]>{
    formats.iter().copied().filter(pure_z32)
        .min_by_key(|p|if p[5]==0xffff_ffff {0}else{1})
}
fn make_z32_desc_for(caps:u32,selected:Option<[u32;8]>,width:u32,height:u32)->[u32;31]{
    let mut requested=desc(caps,width,height,Some(32));
    if let Some(format)=selected{
        requested[18..26].copy_from_slice(&format);
    }
    requested
}

/// Query the real surface's returned Z format. This is the actual
/// DirectDraw result, not a copy of our requested descriptor.
unsafe fn verify_z_format(depth:&Com)->Result<(),String>{
    let get_desc:unsafe extern "system" fn(Ptr,*mut u32)->Hr=
        std::mem::transmute(slot(depth.0,22));
    let mut actual=[0u32;31];actual[0]=124;
    check("Z32.GetSurfaceDesc",get_desc(depth.0,actual.as_mut_ptr()))?;
    probe_line(&format!(
        "Z32-RESULT: flags={:#010x} caps={:#010x} pixel_flags={:#010x} bits={} stencil_bits={} zmask={:#010x}",
        actual[1],actual[26],actual[19],actual[21],actual[22],actual[23]));
    if actual[1]&DDSD_PIXELFORMAT==0 || actual[18]!=32 ||
       actual[19]&DDPF_ZBUFFER==0 || actual[21]!=32 || actual[23]==0 {
        return Err(format!(
            "Actual Z-buffer format is not a usable 32-bit depth pixel format (flags={:#x}, bits={}, mask={:#x})",
            actual[19],actual[21],actual[23]));
    }
    Ok(())
}
/// A real DirectDraw4 backend borrowing the tested primary and RGB565
/// offscreen COM objects. It is NOT an exposed game surface interface.
/// Probe-only log wrapper for shared real DirectDraw4 backend.
/// References to the original COM owners keep their lifetimes explicit.
struct LoggedNativeBlt<'a>{
    inner:borderless_native_blt::NativeWindowedBlt,
    _primary:&'a Com,
    _source:&'a Com,
    verbose:bool,
}
impl LoggedNativeBlt<'_>{
    fn report(&self,step:&str,result:Result<(),i32>)->Result<(),i32>{
        if self.verbose || result.is_err(){
            let hr=result.err().unwrap_or(0);
            probe_line(&format!("{step}: HRESULT={:#010x} {}",
                hr as u32,if hr>=0{"OK"}else{"FAIL"}));
        }
        result
    }
}
impl WindowedBlt for LoggedNativeBlt<'_>{
    fn clear_full_window_black(&mut self,w:u32,h:u32)->Result<(),i32>{
        let result=self.inner.clear_full_window_black(w,h);
        self.report("primary.Blt(COLORFILL black 16:9 window with clipper)",result)
    }
    fn stretch_rgb565_into_viewport(
        &mut self,mode:presentation::Mode,view:presentation::Viewport
    )->Result<(),i32>{
        let result=self.inner.stretch_rgb565_into_viewport(mode,view);
        self.report(&format!("primary.Blt(STRETCH RGB565 {}x{} to 4:3 centered viewport)",mode.width,mode.height),result)
    }
}


/// Exercise our REAL 45-slot Surface4 forwarder with Wine COM objects.
/// The game never receives the forwarder, and CreateDevice is supplied
/// the original real target. This is not game COM-chain virtualization.
unsafe fn test_real_surface4_forwarder(ddraw:&Com,target:&Com)->Result<(),String>{
    use com_surface4_abi::Surface4VTable;
    probe_line("COM-FORWARD-START: native real IDirectDrawSurface4, no game object exposure");

    let forward=com_surface4_forward::new_forwarding_surface4(target.0)
        .map_err(|hr|format!("COM-FORWARD: construction failed {:#010x}",hr as u32))?;
    if forward.is_null(){return Err("COM-FORWARD: NULL facade".into());}
    let wrapped=Com(forward);
    if wrapped.0==target.0{return Err("COM-FORWARD: wrapper aliases original".into());}
    let vt=&**(wrapped.0.cast::<*const Surface4VTable>());
    let iid_unknown=Guid{d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]};
    let iid_surface4=Guid{d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
                          d4:[0x8e,0xa6,0,0x60,0x97,0x97,0xea,0x5b]};
    let mut out=ptr::null_mut();
    check("COM-FORWARD.QueryInterface(IUnknown)",
        (vt.query_interface)(wrapped.0,&iid_unknown,&mut out))?;
    if out!=wrapped.0{return Err("COM-FORWARD: IUnknown identity differs".into());}
    let qi_unknown=Com(out);

    out=ptr::null_mut();
    check("COM-FORWARD.QueryInterface(IDirectDrawSurface4)",
        (vt.query_interface)(qi_unknown.0,&iid_surface4,&mut out))?;
    if out!=wrapped.0{return Err("COM-FORWARD: Surface4 identity differs".into());}
    let qi_surface=Com(out);
    let refcount=(vt.add_ref)(wrapped.0);
    if refcount!=4{return Err(format!("COM-FORWARD: AddRef expected 4, got {refcount}"));}
    if (vt.release)(wrapped.0)!=3{return Err("COM-FORWARD: Release after AddRef mismatched".into());}
    drop(qi_surface);
    drop(qi_unknown);
    probe_line("COM-FORWARD-IUNKNOWN: stable Surface4 identity, QI and AddRef/Release OK");

    out=wrapped.0; // invalid QI must clear this pointer.
    let hr=(vt.query_interface)(wrapped.0,&IID_IDIRECTDRAW7,&mut out);
    if hr!=0x8000_4002u32 as i32 || !out.is_null(){
        return Err(format!("COM-FORWARD: unsupported IID returned {:#010x} ptr_null={}",
            hr as u32,out.is_null()));
    }
    probe_line("COM-FORWARD-UNSUPPORTED-IID: E_NOINTERFACE, output NULL OK");

    let mut caps=[0u32;4];
    check("COM-FORWARD.GetCaps",(vt.get_caps)(wrapped.0,caps.as_mut_ptr().cast()))?;
    if caps[0]&DDSCAPS_3DDEVICE==0 || caps[0]&DDSCAPS_OFFSCREEN==0{
        return Err(format!("COM-FORWARD: expected real D3D offscreen caps, got {:#010x}",caps[0]));
    }
    let mut d=[0u32;31];d[0]=124;
    check("COM-FORWARD.GetSurfaceDesc",(vt.get_surface_desc)(wrapped.0,d.as_mut_ptr().cast()))?;
    if d[2]!=480||d[3]!=640||d[19]&DDPF_RGB==0||d[21]!=16||
       (d[22],d[23],d[24])!=(0xf800,0x07e0,0x001f){
        return Err(format!("COM-FORWARD: invalid returned RGB565 {}x{} bpp={} masks={:#x}/{:#x}/{:#x}",
            d[3],d[2],d[21],d[22],d[23],d[24]));
    }
    probe_line(&format!("COM-FORWARD-SURFACE: {}x{} RGB565 caps={:#010x}",
        d[3],d[2],caps[0]));

    // This returns a *physical* attached Z32 COM pointer (unwrapped).
    // The caller must Release this borrowed attachment result once.
    let mut zcaps=[DDSCAPS_ZBUFFER,0,0,0];
    let mut z=ptr::null_mut();
    check("COM-FORWARD.GetAttachedSurface(Z32, physical)",
        (vt.get_attached_surface)(wrapped.0,zcaps.as_mut_ptr().cast(),&mut z))?;
    if z.is_null()||z==wrapped.0{return Err("COM-FORWARD: invalid raw depth attachment".into());}
    let raw_z=Com(z);
    let mut zd=[0u32;31];zd[0]=124;
    let get_z:unsafe extern "system" fn(Ptr,*mut u32)->Hr=
        std::mem::transmute(slot(raw_z.0,22));
    check("COM-FORWARD.RawDepth.GetSurfaceDesc",get_z(raw_z.0,zd.as_mut_ptr()))?;
    if zd[19]&DDPF_ZBUFFER==0 || zd[21]!=32 || zd[23]!=0x00ff_ffff{
        return Err(format!("COM-FORWARD: wrong attached depth: flags={:#x} bits={} mask={:#x}",
            zd[19],zd[21],zd[23]));
    }
    probe_line("COM-FORWARD-ATTACHMENT: raw real Z32 with 24 active Z bits; NOT VIRTUAL");
    drop(raw_z);

    let mut fx=[0u32;25];fx[0]=100;fx[20]=0x07e0;
    check("COM-FORWARD.Blt(RGB565 green fill)",(vt.blt)(
        wrapped.0,ptr::null_mut(),ptr::null_mut(),ptr::null_mut(),
        DDBLT_COLORFILL|DDBLT_WAIT,fx.as_mut_ptr().cast()))?;
    drop(wrapped); // real target remains alive in the calling probe.
    probe_line("COM-FORWARD-RELEASE: wrapper released, original 3D target still owned");

    // Independent lifetime test: wrapper owns its real AddRef even when
    // its original Com owner is dropped before final facade Release.
    let owned=create_surface(ddraw,"COM-FORWARD isolated RGB565",
        desc(DDSCAPS_3DDEVICE|DDSCAPS_OFFSCREEN|DDSCAPS_VIDEOMEMORY,
             640,480,Some(16)))?;
    let retained=com_surface4_forward::new_forwarding_surface4(owned.0)
        .map_err(|hr|format!("COM-FORWARD isolated wrapper failed: {:#010x}",hr as u32))?;
    let wrapper_only=Com(retained);
    drop(owned);
    let wvt=&**(wrapper_only.0.cast::<*const Surface4VTable>());
    let mut alive=[0u32;31];alive[0]=124;
    check("COM-FORWARD.GetSurfaceDesc(after original Release)",
        (wvt.get_surface_desc)(wrapper_only.0,alive.as_mut_ptr().cast()))?;
    if (alive[3],alive[2],alive[21])!=(640,480,16){
        return Err("COM-FORWARD: live wrapper lost its original surface".into());
    }
    drop(wrapper_only); // releases the final owned real COM reference.
    probe_line("COM-FORWARD-LIFETIME: original owner released first, wrapper still valid");
    probe_line("COM-FORWARD-RESULT: PASS (native COM identity, caps, RGB565, Z32, Blt, ownership)");
    Ok(())
}


/// Stage B4j: test three real native Surface4 objects against three
/// distinct virtual COM Surface4 wrappers, then call their ACTUAL
/// GetAttachedSurface, GetSurfaceDesc, QI and fail-closed Flip vtable slots.
/// This intentionally does NOT activate a game Flip presenter or pass a
/// virtual primary to Wine's device creation.
unsafe fn test_real_virtual_surface_chain(
    primary:&Com,backbuffer:&Com,depth:&Com,mode:presentation::Mode
)->Result<(Com,Com,Com),String>{
    use borderless_contract::SurfaceRole;
    use com_surface4_abi::Surface4VTable;
    probe_line("COM-CHAIN-START: native primary/RGB565/Z32 -> separate virtual Surface4 COM objects");
    let create=|real:Ptr,role:SurfaceRole,label:&str|->Result<Com,String>{
        let raw=com_surface4_forward::new_graph_surface4(real,role,mode)
            .map_err(|hr|format!("{label}: graph wrapper failed {:#010x}",hr as u32))?;
        if raw.is_null() || raw==real {
            return Err(format!("{label}: invalid/aliased wrapper pointer"));
        }
        Ok(Com(raw))
    };
    let gp=create(primary.0,SurfaceRole::Primary,"COM-CHAIN.PRIMARY")?;
    let gb=create(backbuffer.0,SurfaceRole::Backbuffer,"COM-CHAIN.BACKBUFFER")?;
    let gz=create(depth.0,SurfaceRole::Depth,"COM-CHAIN.DEPTH")?;
    if gp.0==gb.0 || gb.0==gz.0 || gp.0==gz.0{
        return Err("COM-CHAIN: virtual identity collision".into());
    }
    com_surface4_forward::attach_graph_surface4(gp.0,gb.0)
        .map_err(|hr|format!("COM-CHAIN.Primary->Backbuffer failed {:#010x}",hr as u32))?;
    com_surface4_forward::attach_graph_surface4(gb.0,gz.0)
        .map_err(|hr|format!("COM-CHAIN.Backbuffer->Z failed {:#010x}",hr as u32))?;
    probe_line("COM-CHAIN-ATTACH: Primary->Backbuffer->Z32 strong COM edges established");
    for (name,com,expected_bits,expected_caps) in [
        ("primary",&gp,16u32,0x1000_e238u32),
        ("backbuffer",&gb,16u32,0x1000_601cu32),
        ("depth",&gz,32u32,0x1002_4000u32),
    ]{
        let v=&**com.0.cast::<*const Surface4VTable>();
        let mut caps=[0u32;4];
        check(&format!("COM-CHAIN.{name}.GetCaps"),
            (v.get_caps)(com.0,caps.as_mut_ptr().cast()))?;
        if caps[0]!=expected_caps{
            return Err(format!("COM-CHAIN.{name} bad caps={:#010x}",caps[0]));
        }
        let mut d=[0u32;31];d[0]=124;
        check(&format!("COM-CHAIN.{name}.GetSurfaceDesc"),
            (v.get_surface_desc)(com.0,d.as_mut_ptr().cast()))?;
        if (d[2],d[3],d[21],d[26])!=(mode.height,mode.width,expected_bits,expected_caps){
            return Err(format!("COM-CHAIN.{name} wrong virtual descriptor"));
        }
        let mut pf=[0u32;8];pf[0]=32;
        check(&format!("COM-CHAIN.{name}.GetPixelFormat"),
            (v.get_pixel_format)(com.0,pf.as_mut_ptr().cast()))?;
        if pf[3]!=expected_bits{
            return Err(format!("COM-CHAIN.{name} wrong virtual pixel format"));
        }
        if name=="depth" && pf[5]!=0x00ff_ffff{
            return Err("COM-CHAIN: virtual Z32 mask differs from real HAL Z32".into());
        }
    }
    let primary_vt=&**gp.0.cast::<*const Surface4VTable>();
    let back_vt=&**gb.0.cast::<*const Surface4VTable>();
    let z_vt=&**gz.0.cast::<*const Surface4VTable>();
    let mut caps=[0x4u32,0,0,0];
    let mut p=ptr::null_mut();
    check("COM-CHAIN.Primary.GetAttachedSurface(BACKBUFFER)",
        (primary_vt.get_attached_surface)(gp.0,caps.as_mut_ptr().cast(),&mut p))?;
    if p!=gb.0{return Err("COM-CHAIN: primary did not return same backbuffer COM pointer".into());}
    let obtained_back=Com(p);
    caps[0]=0x20000;
    p=ptr::null_mut();
    check("COM-CHAIN.Backbuffer.GetAttachedSurface(ZBUFFER)",
        (back_vt.get_attached_surface)(obtained_back.0,caps.as_mut_ptr().cast(),&mut p))?;
    if p!=gz.0{return Err("COM-CHAIN: backbuffer did not return same depth COM pointer".into());}
    let obtained_depth=Com(p);
    let mut incorrect=gp.0;
    check_expected_failure(
        "COM-CHAIN.Primary.GetAttachedSurface(ZBUFFER) rejected",
        (primary_vt.get_attached_surface)(gp.0,caps.as_mut_ptr().cast(),&mut incorrect))?;
    if !incorrect.is_null(){return Err("COM-CHAIN: rejected attachment did not clear output".into());}
    let unknown=Guid{d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]};
    let mut qi=ptr::null_mut();
    check("COM-CHAIN.Depth.QueryInterface(IUnknown)",
        (z_vt.query_interface)(obtained_depth.0,&unknown,&mut qi))?;
    if qi!=gz.0{return Err("COM-CHAIN: depth IUnknown identity mismatch".into());}
    drop(Com(qi));
    drop(obtained_depth);
    drop(obtained_back);
    probe_line("COM-CHAIN-IUNKNOWN: stable distinct Primary/Backbuffer/Z32 COM identities");

    // Primary virtual Flip and Blt MUST NOT be forwarded to the actual
    // normal-mode DirectDraw primary before a production presenter exists.
    let flip=(primary_vt.flip)(gp.0,gb.0,0);
    if flip!=0x8000_4001u32 as i32{
        return Err(format!("COM-CHAIN: unsafe virtual Flip returned {:#010x}",flip as u32));
    }
    let blt=(primary_vt.blt)(gp.0,ptr::null_mut(),gb.0,
                           ptr::null_mut(),0,ptr::null_mut());
    if blt!=0x8000_4001u32 as i32{
        return Err(format!("COM-CHAIN: unsafe virtual Blt returned {:#010x}",blt as u32));
    }
    probe_line("COM-CHAIN-GUARD: virtual Flip and Blt fail closed; no native exclusive Flip");
    if com_surface4_forward::try_borrow_real_surface4(gb.0)!=Some(backbuffer.0){
        return Err("COM-CHAIN: backbuffer cannot unwrap its native RGB565".into());
    }
    probe_line("COM-CHAIN-UNWRAP: game-facing virtual backbuffer resolves native RGB565 for D3D3 only");
    probe_line("COM-CHAIN-RESULT: PASS (real three-surface COM ABI, attachments, refcounts, virtual descriptors)");
    Ok((gp,gb,gz))
}
fn check_expected_failure(label:&str,hr:Hr)->Result<(),String>{
    probe_line(&format!("{label}: HRESULT={:#010x} {}",
        hr as u32,if hr<0{"expected-failure"}else{"UNEXPECTED-SUCCESS"}));
    if hr>=0{Err(format!("{label}: expected failure, got {:#010x}",hr as u32))}
    else{Ok(())}
}

fn run(mode:ProbeMode)->Result<(),String>{
    if std::mem::size_of::<usize>()!=4 {
        return Err("This executable requires Windows x86 (32-bit)".into());
    }
    let sources=probe_source_modes(mode);
    let before=unsafe{(GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN))};
    probe_line(&format!("WINDOWED-DDRAW-PROBE v0.1.46; standalone test; desktop={}x{}",before.0,before.1));
    // Default behaviour remains the owner-verified 960x540 test.
    // Only --borderless creates a desktop-sized WS_POPUP test window,
    // with no display-mode change, topmost setting or game HWND hooks.
    let plan=plan_test_window(mode,before.0,before.1)?;
    let mut outer=Rect{left:0,top:0,right:plan.width,bottom:plan.height};
    if mode==ProbeMode::Windowed &&
       unsafe{AdjustWindowRect(&mut outer,plan.style,0)}==0 {
        return Err("AdjustWindowRect failed".into());
    }
    let mode_label=match mode {ProbeMode::Windowed=>"windowed-default",
        ProbeMode::Borderless=>"borderless-opt-in",
        ProbeMode::BorderlessLoop=>"borderless-loop-opt-in",
        ProbeMode::ComForward=>"com-forward-borderless-opt-in",
        ProbeMode::ComDevice=>"com-device-borderless-opt-in",
        ProbeMode::ComDeviceBridge=>"com-device-bridge-borderless-opt-in",
        ProbeMode::ComDeviceRoundtrip=>"com-device-roundtrip-borderless-opt-in",
        ProbeMode::ComSurfaceChain=>"com-surface-chain-borderless-opt-in",
        ProbeMode::ComVirtualFlip=>"com-virtual-flip-borderless-opt-in",
        ProbeMode::ComVirtualFlip800=>"com-virtual-flip-800-borderless-opt-in",
        ProbeMode::ComVirtualFlipTransition=>"com-virtual-flip-transition-borderless-opt-in"};
    probe_line(&format!(
        "WINDOW-MODE: {mode_label} style={:#010x} requested_client={}x{} at x={},y={}",
        plan.style,plan.client_width,plan.client_height,plan.x,plan.y));
    let class=wide("STATIC");
    let title=wide("CrimsonSkiesFix temporary DirectDraw presentation probe (not game)");
    let hwnd=unsafe {
        CreateWindowExW(0,class.as_ptr(),title.as_ptr(),plan.style,
            plan.x,plan.y,outer.right-outer.left,outer.bottom-outer.top,
            ptr::null_mut(),ptr::null_mut(),ptr::null_mut(),ptr::null_mut())
    };
    if hwnd.is_null(){return Err("CreateWindowExW failed".into());}
    let window=Window(hwnd);
    unsafe{let _=UpdateWindow(hwnd);}
    // DirectDrawCreateEx accepts ONLY IID_IDirectDraw7, even when the
    // rest of this probe needs the older DirectDraw4 COM interface.
    // The previous probe incorrectly passed IID_IDirectDraw4 here,
    // which Wine rejects with E_INVALIDARG (0x80070057).
    let mut dd7=ptr::null_mut();
    unsafe{
        check("DirectDrawCreateEx(IDirectDraw7)",
            DirectDrawCreateEx(ptr::null(),&mut dd7,&IID_IDIRECTDRAW7,ptr::null_mut()))?;
    }
    if dd7.is_null(){return Err("DirectDrawCreateEx returned NULL IDirectDraw7".into());}
    let directdraw7=Com(dd7);
    let mut dd4=ptr::null_mut();
    unsafe {
        let query:unsafe extern "system" fn(Ptr,*const Guid,*mut Ptr)->Hr=
            std::mem::transmute(slot(directdraw7.0,0));
        check("IDirectDraw7.QueryInterface(IDirectDraw4)",
            query(directdraw7.0,&IID_IDIRECTDRAW4,&mut dd4))?;
    }
    if dd4.is_null(){return Err("QueryInterface returned NULL IDirectDraw4".into());}
    let ddraw=Com(dd4);
    // IDirectDraw4 owns its own COM reference. The initial IDirectDraw7
    // pointer is no longer used; release it exactly once before testing.
    drop(directdraw7);
    unsafe{
        let cooperative:unsafe extern "system" fn(Ptr,Ptr,u32)->Hr=
            std::mem::transmute(slot(ddraw.0,20));
        check("DDSCL_NORMAL (never exclusive)",cooperative(ddraw.0,hwnd,DDSCL_NORMAL))?;
    }
    let primary=unsafe{create_surface(&ddraw,"windowed primary",
        desc(DDSCAPS_PRIMARY,0,0,None))?};
    let mut cl=ptr::null_mut();
    unsafe{
        let create_clipper:unsafe extern "system" fn(Ptr,u32,*mut Ptr,Ptr)->Hr=
            std::mem::transmute(slot(ddraw.0,4));
        check("CreateClipper",create_clipper(ddraw.0,0,&mut cl,ptr::null_mut()))?;
    }
    if cl.is_null(){return Err("CreateClipper returned NULL".into());}
    let clipper=Com(cl);
    unsafe{
        let set_hwnd:unsafe extern "system" fn(Ptr,u32,Ptr)->Hr=
            std::mem::transmute(slot(clipper.0,8));
        check("clipper.SetHWnd",set_hwnd(clipper.0,0,window.0))?;
        let set_clipper:unsafe extern "system" fn(Ptr,Ptr)->Hr=
            std::mem::transmute(slot(primary.0,28));
        check("primary.SetClipper",set_clipper(primary.0,clipper.0))?;
    }
    if mode==ProbeMode::ComVirtualFlipTransition{
        probe_line("COM-TRANSITION-START: one HWND+DirectDraw4+clipper, three isolated 3D surface epochs (800x600 -> 640x480 -> 800x600)");
    }
    // An intentionally retained client AddRef models a game still holding
    // the previous mode's primary. Retirement must make every stale Flip
    // fail closed even while that COM identity remains live.
    let mut retired_previous:Option<(usize,Com)>=None;
    for (epoch_index,source) in sources.iter().copied().enumerate(){
        if mode==ProbeMode::ComVirtualFlipTransition{
            probe_line(&format!("COM-TRANSITION-EPOCH-START: epoch={}/{} source={}x{} bpp={} (recreate target/Z32/COM graph/D3D3 device)",
                epoch_index+1,sources.len(),source.width,source.height,source.bpp));
        }
        // The braces force final COM Release for the previous physical and
        // virtual device/surfaces before the next epoch allocates anything.
        {
    let target=unsafe{create_surface(&ddraw,"RGB565 offscreen 3D target",
        desc(DDSCAPS_OFFSCREEN|DDSCAPS_3DDEVICE|DDSCAPS_VIDEOMEMORY,
             source.width,source.height,Some(16)))?};
    // A separate query of the same DD4 object. Do not substitute the
    // standalone probe target into the game; CreateDevice must succeed here.
    let mut d3d=ptr::null_mut();
    unsafe{
        let qi:unsafe extern "system" fn(Ptr,*const Guid,*mut Ptr)->Hr=
            std::mem::transmute(slot(ddraw.0,0));
        check("IDirectDraw4.QueryInterface(IDirect3D3)",
            qi(ddraw.0,&IID_IDIRECT3D3,&mut d3d))?;
    }
    if d3d.is_null(){return Err("IDirect3D3 QueryInterface returned NULL".into());}
    let d3d=Com(d3d);
    // Query actual HAL Z formats before selecting a depth surface;
    // keep the probe strict: no 16-bit fallback and no fake success.
    let offered=unsafe{enumerate_z_formats(&d3d)};
    let selected=select_pure_z32(&offered);
    match selected{
        Some(p)=>probe_line(&format!(
            "Z32-SELECTED: enumerated HAL format flags={:#010x} bits={} stencil_bits={} zmask={:#010x}",
            p[1],p[3],p[4],p[5])),
        None=>probe_line("Z32-SELECTED: no pure 32-bit format enumerated; trying corrected 0xffffffff mask as diagnostic"),
    }
    // Try the game's preferred video-memory placement first. If that
    // exact placement is rejected, probe a second *32-bit* allocation
    // without forcing memory location; NEVER silently degrade to Z16.
    let (depth,depth_placement)=unsafe {
        match create_surface(&ddraw,"32-bit Z-buffer (explicit video memory)",
            make_z32_desc_for(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,selected,source.width,source.height)) {
            Ok(surface)=>(surface,"explicit-video-memory"),
            Err(err)=>{
                probe_line(&format!(
                    "Z32-RETRY: explicit video memory rejected ({err}); retrying 32-bit depth with driver-selected memory placement"));
                let surface=create_surface(&ddraw,"32-bit Z-buffer (driver-selected memory)",
                    make_z32_desc_for(DDSCAPS_ZBUFFER,selected,source.width,source.height))?;
                (surface,"driver-selected-memory")
            },
        }
    };
    probe_line(&format!("Z32-PLACEMENT: {depth_placement}"));
    unsafe{verify_z_format(&depth)?;}
    unsafe{
        let attach:unsafe extern "system" fn(Ptr,Ptr)->Hr=
            std::mem::transmute(slot(target.0,3));
        check("target.AddAttachedSurface(Z)",attach(target.0,depth.0))?;
    }
    // Real game uses DDBLT_DEPTHFILL with NULL source. Exercise the
    // actual depth fill and report its HRESULT before 3D device setup.
    // In Win32 DDBLTFX, dwFillDepth occupies DWORD index 20.
    let mut depth_fill=[0u32;25];depth_fill[0]=100;depth_fill[20]=0xffff_ffff;
    unsafe{
        let blt:unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->Hr=
            std::mem::transmute(slot(depth.0,5));
        check("Z32.Blt(DDBLT_DEPTHFILL)",blt(depth.0,ptr::null(),ptr::null_mut(),
            ptr::null(),DDBLT_DEPTHFILL,depth_fill.as_mut_ptr()))?;
    }
    // 100-byte DDBLTFX in Win32 x86; dwFillColor is DWORD index 20.
    let mut fill=[0u32;25];fill[0]=100;fill[20]=0x3ac7;
    unsafe{
        let blt:unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->Hr=
            std::mem::transmute(slot(target.0,5));
        check("RGB565 offscreen ColorFill",blt(target.0,ptr::null(),ptr::null_mut(),
                ptr::null(),DDBLT_COLORFILL|DDBLT_WAIT,fill.as_mut_ptr()))?;
    }
    // The old --com-device experiment directly supplied our wrapper to
    // Wine and the owner received DDERR_INVALIDCAPS (0x88760064).
    // Wine d3d3_CreateDevice uses unsafe_impl_from_IDirectDrawSurface4 to
    // obtain its *own internal* ddraw_surface, not a generic COM proxy.
    // New opt-in --com-device-bridge explicitly borrows the physical
    // Surface4 at the D3D boundary while keeping wrapper alive.
    let surface_chain=if matches!(mode,ProbeMode::ComSurfaceChain|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition){
        Some(unsafe{test_real_virtual_surface_chain(&primary,&target,&depth,source)?})
    }else{None};
    if mode==ProbeMode::ComVirtualFlipTransition{
        use com_surface4_abi::Surface4VTable;
        let (gp,gb,gz)=surface_chain.as_ref().unwrap();
        unsafe{
            let pvt=&**gp.0.cast::<*const Surface4VTable>();
            let bvt=&**gb.0.cast::<*const Surface4VTable>();
            let zvt=&**gz.0.cast::<*const Surface4VTable>();
            check("COM-GRAPH-BLT.backbuffer.ColorFill(NULL source)",
                (bvt.blt)(gb.0,ptr::null_mut(),ptr::null_mut(),
                    ptr::null_mut(),DDBLT_COLORFILL|DDBLT_WAIT,fill.as_mut_ptr().cast()))?;
            check("COM-GRAPH-BLT.depth.DepthFill(NULL source)",
                (zvt.blt)(gz.0,ptr::null_mut(),ptr::null_mut(),
                    ptr::null_mut(),DDBLT_DEPTHFILL,depth_fill.as_mut_ptr().cast()))?;
            let bad=(pvt.blt)(gp.0,ptr::null_mut(),ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,fill.as_mut_ptr().cast());
            if bad!=0x8000_4001u32 as i32{
                return Err(format!("COM-GRAPH-BLT: unsafe primary Blt returned {:#010x}",bad as u32));
            }
            let copy=(bvt.blt)(gb.0,ptr::null_mut(),gz.0,ptr::null_mut(),0,ptr::null_mut());
            if copy!=0x8000_4001u32 as i32{
                return Err(format!("COM-GRAPH-BLT: unsupported source-copy returned {:#010x}",copy as u32));
            }
        }
        probe_line(&format!("COM-GRAPH-BLT: epoch={}/{} RGB565 ColorFill and Z32 DepthFill routed to physical Surface4; primary/source-copy rejected",epoch_index+1,sources.len()));
    }
    if let Some((previous_epoch,old))=retired_previous.take(){
        // The next mode's physical RGB565/Z32 and virtual graph now exist.
        // A still-live OLD virtual primary must never present onto either.
        let vt=unsafe{&**old.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let stale=unsafe{(vt.flip)(old.0,ptr::null_mut(),0)};
        if stale!=0x8000_4001u32 as i32{
            return Err(format!("COM-TRANSITION-STALE: old epoch {} Flip returned {:#010x}, expected E_NOTIMPL",
                previous_epoch,stale as u32));
        }
        let mut old_back=ptr::null_mut();
        let mut back_caps=[4u32,0,0,0];
        check("COM-GRAPH-BLT.old-primary.GetAttachedSurface(backbuffer)",
            unsafe{(vt.get_attached_surface)(old.0,back_caps.as_mut_ptr().cast(),&mut old_back)})?;
        if old_back.is_null(){return Err("COM-GRAPH-BLT: old backbuffer NULL".into());}
        let old_back=Com(old_back);
        let bvt=unsafe{&**old_back.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let mut old_depth=ptr::null_mut();
        let mut depth_caps=[DDSCAPS_ZBUFFER,0,0,0];
        check("COM-GRAPH-BLT.old-backbuffer.GetAttachedSurface(depth)",
            unsafe{(bvt.get_attached_surface)(old_back.0,depth_caps.as_mut_ptr().cast(),&mut old_depth)})?;
        if old_depth.is_null(){return Err("COM-GRAPH-BLT: old depth NULL".into());}
        let old_depth=Com(old_depth);
        let zvt=unsafe{&**old_depth.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let stale_blt=unsafe{(bvt.blt)(old_back.0,ptr::null_mut(),ptr::null_mut(),
            ptr::null_mut(),DDBLT_COLORFILL,fill.as_mut_ptr().cast())};
        let stale_depth=unsafe{(zvt.blt)(old_depth.0,ptr::null_mut(),ptr::null_mut(),
            ptr::null_mut(),DDBLT_DEPTHFILL,depth_fill.as_mut_ptr().cast())};
        if stale_blt!=0x8000_4001u32 as i32 || stale_depth!=0x8000_4001u32 as i32{
            return Err(format!("COM-GRAPH-BLT: old epoch {} stale backbuffer={:#010x} depth={:#010x} expected E_NOTIMPL",
                previous_epoch,stale_blt as u32,stale_depth as u32));
        }
        drop(old_depth);
        drop(old_back);
        probe_line(&format!("COM-TRANSITION-STALE: old epoch {} primary Flip and backbuffer/Z32 null-source Blts all E_NOTIMPL after new epoch {} graph initialization",
            previous_epoch,epoch_index+1));
        drop(old); // last deliberate stale AddRef, after verifying fail closed
    }
    let needs_wrapper=matches!(mode,ProbeMode::ComDevice|ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip);
    let wrapped_device_target=if needs_wrapper{
        probe_line(if matches!(mode,ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip) {
            "COM-DEVICE-BRIDGE-START: wrapper retained; native Direct3D3 target unwrapped"
        }else{
            "COM-DEVICE-START: IDirect3D3.CreateDevice(HAL, forwarded Surface4)"
        });
        let facade=unsafe{com_surface4_forward::new_forwarding_surface4(target.0)}
            .map_err(|hr|format!("COM-DEVICE: Surface4 wrapper creation failed {:#010x}",hr as u32))?;
        if facade.is_null(){return Err("COM-DEVICE: wrapper NULL".into());}
        let wrapped=Com(facade);
        if wrapped.0==target.0{return Err("COM-DEVICE: wrapper aliases physical target".into());}
        probe_line("COM-DEVICE-WRAPPER: native 45-slot Surface4 with separate IUnknown identity created");
        Some(wrapped)
    }else{None};
    let roundtrip=if mode==ProbeMode::ComDeviceRoundtrip {
        let wrapped=wrapped_device_target.as_ref()
            .ok_or_else(||"COM-ROUNDTRIP: missing wrapper".to_owned())?;
        let map=unsafe{com_surface4_roundtrip::Surface4RoundTrip::new(wrapped.0)}
            .map_err(|hr|format!("COM-ROUNDTRIP: mapping constructor failed {:#010x}",hr as u32))?;
        probe_line("COM-ROUNDTRIP-REGISTER: one owned wrapper reference and native target registered");
        Some(map)
    }else{None};
    let device_target=if let Some((_,back,_))=surface_chain.as_ref(){
        unsafe{com_surface4_forward::try_borrow_real_surface4(back.0)}
            .ok_or_else(||"COM-CHAIN: native backbuffer missing".to_owned())?
    }else if matches!(mode,ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip){
        let wrapped=wrapped_device_target.as_ref()
            .ok_or_else(||"COM-DEVICE-BRIDGE: no live wrapper".to_owned())?;
        let native=if let Some(map)=roundtrip.as_ref(){
            map.native_target()
        }else{
            unsafe{com_surface4_forward::try_borrow_real_surface4(wrapped.0)}
                .ok_or_else(||"COM-DEVICE-BRIDGE: cannot resolve our own wrapper".to_owned())?
        };
        if native!=target.0 || native==wrapped.0 {
            return Err("COM-DEVICE-BRIDGE: physical pointer mismatch".into());
        }
        let raw_check=unsafe{
            com_surface4_forward::try_borrow_real_surface4(target.0)
        };
        if raw_check.is_some(){
            return Err("COM-DEVICE-BRIDGE: a foreign native surface was mistaken for wrapper".into());
        }
        probe_line("COM-DEVICE-BRIDGE-UNWRAP: verified original RGB565 physical Surface4 (borrowed, not owned)");
        native
    }else{
        wrapped_device_target.as_ref().map_or(target.0,|w|w.0)
    };
    let mut device=ptr::null_mut();
    unsafe{
        let create:unsafe extern "system" fn(Ptr,*const Guid,Ptr,*mut Ptr,Ptr)->Hr=
            std::mem::transmute(slot(d3d.0,8));
        let label=match mode{
            ProbeMode::ComDevice=>"COM-DEVICE.CreateDevice(HAL, forwarded RGB565 Surface4)",
            ProbeMode::ComDeviceBridge=>"COM-DEVICE-BRIDGE.CreateDevice(HAL, ORIGINAL RGB565 Surface4)",
            ProbeMode::ComDeviceRoundtrip=>"COM-ROUNDTRIP.CreateDevice(HAL, original physical RGB565 Surface4)",
            ProbeMode::ComSurfaceChain=>"COM-CHAIN.CreateDevice(HAL, physical graph Backbuffer)",
            ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition=>"COM-VFLIP.CreateDevice(HAL, physical virtual-backbuffer target)",
            _=>"IDirect3D3.CreateDevice(HAL, RGB565 target)"
        };
        check(label,create(d3d.0,&CLSID_D3D_HAL,device_target,&mut device,ptr::null_mut()))?;
    }
    if device.is_null(){return Err("CreateDevice returned NULL".into());}
    let device=Com(device);
    if mode==ProbeMode::ComDevice{
        probe_line("COM-DEVICE-CREATED: real HAL device accepted Surface4 forwarding object");
    }else if matches!(mode,ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip){
        probe_line("COM-DEVICE-BRIDGE-CREATED: native HAL device accepted borrowed underlying Surface4");
        unsafe{
            let get:unsafe extern "system" fn(Ptr,*mut Ptr)->Hr=
                std::mem::transmute(slot(device.0,15)); // IDirect3DDevice3::GetRenderTarget
            let mut returned=ptr::null_mut();
            check("COM-DEVICE-BRIDGE.GetRenderTarget",get(device.0,&mut returned))?;
            if returned.is_null(){return Err("COM-DEVICE-BRIDGE: GetRenderTarget returned NULL".into());}
            let original=Com(returned);
            if original.0!=target.0 || original.0==wrapped_device_target.as_ref().unwrap().0 {
                return Err(format!(
                    "COM-DEVICE-BRIDGE: non-native render target returned (same_physical={},same_wrapper={})",
                    original.0==target.0,original.0==wrapped_device_target.as_ref().unwrap().0));
            }
            probe_line("COM-DEVICE-BRIDGE-RENDER-TARGET: native physical Surface4 returned, NOT wrapper");
            if let Some(map)=roundtrip.as_ref(){
                let mapped=map.map_native_result(original.0)
                    .map_err(|hr|format!("COM-ROUNDTRIP: native result not registered {:#010x}",hr as u32))?;
                let virtual_result=Com(mapped);
                let wrapped=wrapped_device_target.as_ref().unwrap();
                if !map.matches_wrapper(virtual_result.0) || virtual_result.0!=wrapped.0 {
                    return Err("COM-ROUNDTRIP: wrong game-facing Surface4 identity".into());
                }
                let mut unknown=ptr::null_mut();
                let qi:unsafe extern "system" fn(Ptr,*const Guid,*mut Ptr)->Hr=
                    std::mem::transmute(slot(virtual_result.0,0));
                let iid=Guid{d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]};
                check("COM-ROUNDTRIP.QueryInterface(IUnknown)",qi(virtual_result.0,&iid,&mut unknown))?;
                if unknown!=wrapped.0{return Err("COM-ROUNDTRIP: IUnknown identity mismatch".into());}
                let alias=Com(unknown);
                drop(alias);
                drop(virtual_result);
                probe_line("COM-ROUNDTRIP-RESULT: native GetRenderTarget remapped to original game-facing Surface4 identity");
            }
        }
    }
    if matches!(mode,ProbeMode::ComSurfaceChain|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition){
        let (_,ref back,_)=surface_chain.as_ref().unwrap();
        unsafe{
            let get:unsafe extern "system" fn(Ptr,*mut Ptr)->Hr=
                std::mem::transmute(slot(device.0,15));
            let mut raw=ptr::null_mut();
            check("COM-CHAIN.GetRenderTarget(native)",get(device.0,&mut raw))?;
            if raw.is_null(){return Err("COM-CHAIN: native D3D3 target NULL".into());}
            let physical=Com(raw);
            if physical.0!=target.0{return Err("COM-CHAIN: different native HAL target".into());}
            let map=com_surface4_roundtrip::Surface4RoundTrip::new(back.0)
                .map_err(|hr|format!("COM-CHAIN: roundtrip map failed {:#010x}",hr as u32))?;
            let restored=map.map_native_result(physical.0)
                .map_err(|hr|format!("COM-CHAIN: native target mapping failed {:#010x}",hr as u32))?;
            let restored=Com(restored);
            if restored.0!=back.0{return Err("COM-CHAIN: wrong virtual backbuffer returned".into());}
            probe_line("COM-CHAIN-RENDERTARGET: native HAL surface restored to exact virtual backbuffer COM identity");
        }
    }
    if mode==ProbeMode::ComForward{
        unsafe{test_real_surface4_forwarder(&ddraw,&target)?;}
    }
    // In-game Crimson Skies repeatedly uses BeginScene/EndScene on the
    // same Direct3D3 device. Exercise the real HAL scene API in normal mode.
    unsafe{
        let begin:unsafe extern "system" fn(Ptr)->Hr=
            std::mem::transmute(slot(device.0,9));
        let end:unsafe extern "system" fn(Ptr)->Hr=
            std::mem::transmute(slot(device.0,10));
        check("IDirect3DDevice3.BeginScene",begin(device.0))?;
        check("IDirect3DDevice3.EndScene",end(device.0))?;
    }
    if mode==ProbeMode::ComDevice{
        probe_line("COM-DEVICE-SCENE: BeginScene and EndScene on real HAL device succeeded");
    }else if matches!(mode,ProbeMode::ComDeviceBridge|ProbeMode::ComDeviceRoundtrip){
        probe_line("COM-DEVICE-BRIDGE-SCENE: native HAL BeginScene/EndScene succeeded");
    }

    // Visual proof in the optional borderless mode only: draw three
    // solid RGB565 stripes into the genuine 3D-capable offscreen surface.
    // The previous all-solid-color default test remains unchanged.
    if mode==ProbeMode::Borderless {
        let stripes=[
            ("red",Rect{left:0,top:0,right:213,bottom:480},0xf800u32),
            ("green",Rect{left:213,top:0,right:426,bottom:480},0x07e0u32),
            ("blue",Rect{left:426,top:0,right:640,bottom:480},0x001fu32),
        ];
        unsafe {
            let blt:unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->Hr=
                std::mem::transmute(slot(target.0,5));
            for (color,rect,rgb565) in &stripes {
                let mut fx=[0u32;25]; fx[0]=100;fx[20]=*rgb565;
                check(&format!("RGB565-TEST-PATTERN: {color}"),
                    blt(target.0,rect,ptr::null_mut(),ptr::null(),
                        DDBLT_COLORFILL|DDBLT_WAIT,fx.as_mut_ptr()))?;
            }
        }
    }
    let mut origin=Point::default();
    let mut client=Rect{left:0,top:0,right:0,bottom:0};
    if unsafe{GetClientRect(window.0,&mut client)}==0{return Err("GetClientRect failed".into());}
    if unsafe{ClientToScreen(window.0,&mut origin)}==0{return Err("ClientToScreen failed".into());}
    let client_w=u32::try_from(client.right-client.left).map_err(|_|"Bad client width")?;
    let client_h=u32::try_from(client.bottom-client.top).map_err(|_|"Bad client height")?;
    let viewport=letterbox(source.width,source.height,client_w,client_h)
        .ok_or_else(||"Invalid 4:3-in-16:9 geometry".to_owned())?;
    probe_line(&format!(
        "WINDOW-CLIENT: {}x{} expected={}x{} origin_screen=x{},y{}",
        client_w,client_h,plan.client_width,plan.client_height,origin.x,origin.y));
    if client_w!=plan.client_width || client_h!=plan.client_height{
        return Err(format!("Actual client {}x{} differs from requested {}x{}",
            client_w,client_h,plan.client_width,plan.client_height));
    }
    if mode.borderless() && (origin.x!=0||origin.y!=0){
        return Err(format!("Borderless test window was repositioned by host WM: client origin={},{}",
            origin.x,origin.y));
    }
    probe_line(&format!(
        "ASPECT-VIEWPORT: source={}x{} window={}x{} viewport=x{},y{},{}x{} letterbox=4:3-in-16:9",
        source.width,source.height,client_w,client_h,viewport.left,viewport.top,
        viewport.right-viewport.left,viewport.bottom-viewport.top));
    // The primary is in desktop coordinates, not window-client coordinates.
    // First paint the *entire client* black, then stretch the 640x480
    // RGB565 surface into the centered 4:3 viewport. The clipper restricts
    // drawing to our own window, leaving the rest of the desktop untouched.
    // Exercise exactly the same two-Blt transaction as the planned
    // fail-closed game Flip bridge, but here with *actual* Win32 COM
    // primary and 640x480 RGB565 offscreen surfaces.
    let view=presentation::Viewport{
        x:viewport.left as u32,y:viewport.top as u32,
        width:(viewport.right-viewport.left) as u32,
        height:(viewport.bottom-viewport.top) as u32,
    };
    // The shared backend is compiled into both binaries. The probe
    // retains the actual COM owners and adds only its local log wrapper.
    let physical=unsafe{borderless_native_blt::NativeWindowedBlt::from_borrowed_com(
        primary.0,target.0,(origin.x,origin.y),(client_w,client_h),source
    )}.map_err(|hr|format!("Native DirectDraw4 backend rejected parameters: {:#010x}",hr as u32))?;
    let mut backend=LoggedNativeBlt{
        inner:physical,_primary:&primary,_source:&target,verbose:true,
    };
    if mode.virtual_flip(){
        use borderless_contract::{AdapterContract,BackendEvidence,RenderTarget};
        let (gp,gb,gz)=surface_chain.as_ref()
            .ok_or_else(||"COM-VFLIP: no attached Surface4 graph".to_owned())?;
        let vt=unsafe{&**gp.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let prearm=unsafe{(vt.flip)(gp.0,gb.0,0)};
        if prearm!=0x8000_4001u32 as i32{
            return Err(format!("COM-VFLIP: pre-arm Flip must fail closed, returned {:#010x}",
                prearm as u32));
        }
        probe_line("COM-VFLIP-PREARM: virtual primary Flip rejected safely before activation");
        // Physical proof precedes exposing the *standalone* graph contract.
        // No game COM interface is exposed by this program.
        borderless_flip::execute_windowed_blt_pair(
            &mut backend,source,(client_w,client_h),view
        ).map_err(|e|format!("COM-VFLIP: physical presentation preflight failed: {e:?}"))?;
        probe_line("COM-VFLIP-PREFLIGHT: actual clipped black clear + native RGB565 stretch OK");
        let mut contract=AdapterContract::prepare(
            source,(client_w,client_h),
            presentation::Chain{
                front_to_back:true,back_to_depth:true,mipmaps:false
            }
        ).map_err(|e|format!("COM-VFLIP: invalid graph plan: {e:?}"))?;
        contract.expose(BackendEvidence{
            process_owned_window:true,
            normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:DDSCAPS_3DDEVICE,
            render_target_is_rgb565:true,
            real_zbuffer_32:true,
            primary_with_clipper:true,
            working_blt:true,
            complete_surface_com:true,
        }).map_err(|e|format!("COM-VFLIP: adapter refused physical evidence: {e:?}"))?;
        let generation=1042u64 + epoch_index as u64;
        unsafe{
            com_surface4_forward::arm_graph_primary_flip(
                gp.0,&contract,generation,(origin.x,origin.y),(client_w,client_h),None,0,false,std::ptr::null_mut()
            ).map_err(|hr|format!("COM-VFLIP: arm failed {:#010x}",hr as u32))?;
        }
        if unsafe{com_surface4_forward::graph_primary_completed_flips(gp.0)}!=Some(0){
            return Err("COM-VFLIP: armed graph has incorrect initial frame count".into());
        }
        let invalid_target=unsafe{(vt.flip)(gp.0,gz.0,0)};
        let invalid_flags=unsafe{(vt.flip)(gp.0,gb.0,1)};
        if invalid_target!=0x8007_0057u32 as i32||
           invalid_flags!=0x8007_0057u32 as i32 ||
           unsafe{com_surface4_forward::graph_primary_completed_flips(gp.0)}!=Some(0){
            return Err("COM-VFLIP: invalid target/flags touched presentation".into());
        }
        probe_line("COM-VFLIP-ARMED: complete COM graph, physical HAL and presenter verified");
        probe_line("COM-VFLIP-GUARD: wrong target/flags rejected before any physical frame");
    }
    let frames=if matches!(mode,ProbeMode::BorderlessLoop|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition){120u32}else{1u32};
    probe_line(&format!(
        "FLIP-BLT-SEQUENCE: requested_frames={frames} backend={}",
        if mode.virtual_flip(){"virtual-Surface4::Flip -> real DirectDraw4 Blt"}
        else{"real-DirectDraw4"}
    ));
    let start=std::time::Instant::now();
    let mut completed=0u32;
    for n in 0..frames{
        if matches!(mode,ProbeMode::BorderlessLoop|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition){
            // Animate physical RGB565 colors; these are NOT game frames.
            // Keep logs bounded even for 120 consecutive actual Blts.
            let color=[0xf800u32,0x07e0u32,0x001fu32][n as usize%3];
            let mut fx=[0u32;25];fx[0]=100;fx[20]=color;
            unsafe{
                let blt:unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->Hr=
                    std::mem::transmute(slot(target.0,5));
                let hr=blt(target.0,ptr::null(),ptr::null_mut(),ptr::null(),
                           DDBLT_COLORFILL|DDBLT_WAIT,fx.as_mut_ptr());
                if hr<0{return Err(format!(
                    "FLIP-BLT frame {} RGB565 fill failed: {:#010x}",n+1,hr as u32));}
                let begin:unsafe extern "system" fn(Ptr)->Hr=
                    std::mem::transmute(slot(device.0,9));
                let end:unsafe extern "system" fn(Ptr)->Hr=
                    std::mem::transmute(slot(device.0,10));
                let hr=begin(device.0);
                if hr<0{return Err(format!(
                    "FLIP-BLT frame {} BeginScene failed: {:#010x}",n+1,hr as u32));}
                let hr=end(device.0);
                if hr<0{return Err(format!(
                    "FLIP-BLT frame {} EndScene failed: {:#010x}",n+1,hr as u32));}
            }
        }
        if mode.virtual_flip(){
            let (gp,gb,_)=surface_chain.as_ref().unwrap();
            unsafe{
                let flip=(&**gp.0.cast::<*const com_surface4_abi::Surface4VTable>()).flip;
                let hr=flip(gp.0,gb.0,0);
                if hr<0{
                    return Err(format!("COM-VFLIP frame {}/{} virtual Flip failed: {:#010x}",
                        n+1,frames,hr as u32));
                }
                if n==0{
                    probe_line("COM-VFLIP.Flip(virtual Primary, virtual Backbuffer): HRESULT=0x00000000 OK");
                }
                let count=com_surface4_forward::graph_primary_completed_flips(gp.0);
                if count!=Some(u64::from(n)+1){
                    return Err(format!("COM-VFLIP: acknowledged frame counter invalid: {count:?}"));
                }
            }
        }else{
            backend.verbose=n==0;
            borderless_flip::execute_windowed_blt_pair(
                &mut backend,source,(client_w,client_h),view
            ).map_err(|e|format!("FLIP-BLT frame {}/{} failed: {e:?}",n+1,frames))?;
        }
        completed+=1;
        if matches!(mode,ProbeMode::BorderlessLoop|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition) && completed%30==0{
            probe_line(&format!(
                "{}: successful_frames={completed}/{frames}",
                if mode.virtual_flip(){"COM-VFLIP-PROGRESS"}else{"FLIP-BLT-PROGRESS"}
            ));
        }
        if matches!(mode,ProbeMode::BorderlessLoop|ProbeMode::ComVirtualFlip|ProbeMode::ComVirtualFlip800|ProbeMode::ComVirtualFlipTransition){
            unsafe{Sleep(12);} // visual demo, NEVER measured gameplay FPS
        }
    }
    probe_line(&format!(
        "{}: requested={frames} successful={completed} elapsed_ms={} (NOT gameplay FPS)",
        if mode.virtual_flip(){"COM-VFLIP-RESULT"}else{"FLIP-BLT-RESULT"},
        start.elapsed().as_millis()));
    // A windowed primary Blt success means the DirectDraw backend accepted
    // the operation, but does not independently measure displayed pixels.
    let after=unsafe{(GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN))};
    if before!=after{return Err(format!("Desktop changed unexpectedly: {:?} -> {:?}",before,after));}
    if mode==ProbeMode::ComDevice{
        probe_line("COM-DEVICE-RESULT: PASS (forwarded Surface4 accepted by Direct3D3 HAL, D3D scene, Blt)");
    }else if mode==ProbeMode::ComDeviceBridge{
        probe_line("COM-DEVICE-BRIDGE-RESULT: PASS (wrapper identity retained, D3D3 receives native target, scene, Blt)");
    }else if mode==ProbeMode::ComDeviceRoundtrip{
        probe_line("COM-ROUNDTRIP-FINAL: PASS (wrapper -> native D3D3 -> wrapper identity, scene, 4:3 Blt)");
    }else if mode==ProbeMode::ComSurfaceChain{
        probe_line("COM-CHAIN-FINAL: PASS (virtual Primary->Backbuffer->Z32, physical HAL D3D3, 4:3 Blt)");
    }else if mode.virtual_flip(){
        probe_line("COM-VFLIP-FINAL: PASS (120 real native presentations through virtual Surface4::Flip)");
    }
    if mode==ProbeMode::ComVirtualFlipTransition{
        let (gp,gb,gz)=surface_chain.as_ref().unwrap();
        let vt=unsafe{&**gp.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let retired=unsafe{com_surface4_forward::retire_graph_primary_flip(gp.0)}
            .map_err(|hr|format!("COM-TRANSITION-RETIRE: epoch={} failed {:#010x}",epoch_index+1,hr as u32))?;
        if retired!=u64::from(frames) || unsafe{com_surface4_forward::graph_primary_completed_flips(gp.0)}.is_some(){
            return Err(format!("COM-TRANSITION-RETIRE: epoch={} incorrect final frame count/armed state",epoch_index+1));
        }
        let stale=unsafe{(vt.flip)(gp.0,gb.0,0)};
        if stale!=0x8000_4001u32 as i32{
            return Err(format!("COM-TRANSITION-RETIRE: epoch={} stale Flip unexpectedly succeeded {:#010x}",epoch_index+1,stale as u32));
        }
        let bvt=unsafe{&**gb.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let zvt=unsafe{&**gz.0.cast::<*const com_surface4_abi::Surface4VTable>()};
        let stale_back=unsafe{(bvt.blt)(gb.0,ptr::null_mut(),ptr::null_mut(),
            ptr::null_mut(),DDBLT_COLORFILL,fill.as_mut_ptr().cast())};
        let stale_z=unsafe{(zvt.blt)(gz.0,ptr::null_mut(),ptr::null_mut(),
            ptr::null_mut(),DDBLT_DEPTHFILL,depth_fill.as_mut_ptr().cast())};
        if stale_back!=0x8000_4001u32 as i32 || stale_z!=0x8000_4001u32 as i32{
            return Err(format!("COM-TRANSITION-RETIRE: epoch={} old backbuffer/Z32 Blt not rejected ({:#010x}/{:#010x})",
                epoch_index+1,stale_back as u32,stale_z as u32));
        }
        let refcount=unsafe{(vt.add_ref)(gp.0)};
        if refcount<2{return Err("COM-TRANSITION-RETIRE: failed to hold stale COM reference".into());}
        retired_previous=Some((epoch_index+1,Com(gp.0)));
        probe_line(&format!("COM-TRANSITION-RETIRE: epoch={}/{} final_frames={} presenter permanently disarmed; stale Flip and backbuffer/Z32 Blts rejected; one COM reference retained for next epoch",
            epoch_index+1,sources.len(),retired));
    }
        } // Release this epoch\x27s normal COM owners; one retained stale AddRef is intentional.
        if mode==ProbeMode::ComVirtualFlipTransition{
            probe_line(&format!("COM-TRANSITION-EPOCH-END: epoch={}/{} source={}x{} finished; normal COM owners dropped; retired stale primary retained until checked",
                epoch_index+1,sources.len(),source.width,source.height));
        }
    }
    if mode==ProbeMode::ComVirtualFlipTransition{
        if let Some((last_epoch,last))=retired_previous.take(){
            let vt=unsafe{&**last.0.cast::<*const com_surface4_abi::Surface4VTable>()};
            let hr=unsafe{(vt.flip)(last.0,ptr::null_mut(),0)};
            if hr!=0x8000_4001u32 as i32{
                return Err(format!("COM-TRANSITION-LAST-STALE: epoch={} unexpected Flip {:#010x}",last_epoch,hr as u32));
            }
            drop(last);
            probe_line(&format!("COM-TRANSITION-LAST-STALE: final epoch {} old primary rejects Flip and final retained reference is released",last_epoch));
        }
        probe_line("COM-TRANSITION-FINAL: PASS (3 full 120-Flip epochs; 800x600 -> 640x480 -> 800x600; retired COM Flip identities never rearm; desktop preserved)");
    }
    probe_line(&format!(
        "PASS: mode={mode_label}; real windowed primary+clipper, RGB565+Z32, D3D3 HAL scenes, centered 4:3 Blt; desktop preserved"));
    // A temporary test window, deliberately self-closing. Give the owner
    // slightly longer to see the RGB565 stripes in borderless mode.
    unsafe{Sleep(if mode==ProbeMode::Borderless {3000}else{1000});}
    Ok(())
}
fn main(){
    let path=match init_probe_log(){
        Ok(path)=>path,
        Err(e)=>{
            eprintln!("PROBE LOG INITIALIZATION FAILED: {e}");
            std::process::exit(2);
        }
    };
    probe_line("CrimsonSkiesFix standalone windowed resource probe v0.1.46");
    probe_line(&format!("Log file: {}",path.display()));
    probe_line("All DirectDraw HRESULTs and final result are saved in this log; Wine diagnostics are excluded.");
    let args:Vec<String>=std::env::args().skip(1).collect();
    let mode=match parse_mode(&args){
        Ok(mode)=>mode,
        Err(e)=>{
            probe_line(&format!("PROBE FAILED: {e}"));
            std::process::exit(2);
        }
    };
    if let Err(e)=run(mode){
        probe_line(&format!("PROBE FAILED: {e}"));
        std::process::exit(1);
    }
    probe_line("PROBE COMPLETED: PASS");
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn desc2_has_exact_win32_x86_layout(){
        assert_eq!(std::mem::size_of::<usize>(),4);
        let d=desc(DDSCAPS_3DDEVICE|DDSCAPS_OFFSCREEN|DDSCAPS_VIDEOMEMORY,640,480,Some(16));
        assert_eq!(d.len()*4,124);
        assert_eq!(d[0],124);
        assert_eq!(d[1],DDSD_CAPS|DDSD_HEIGHT|DDSD_WIDTH|DDSD_PIXELFORMAT);
        assert_eq!((d[2],d[3]),(480,640));
        assert_eq!(d[19],DDPF_RGB);
        assert_eq!(d[21],16);
        assert_eq!((d[22],d[23],d[24]),(0xf800,0x07e0,0x001f));
        assert_eq!(d[26],0x6040);
    }
    #[test]
    fn zbuffer_descriptor_is_independent_of_rgb_masks(){
        let d=desc(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,640,480,Some(32));
        assert_eq!(d[19],DDPF_ZBUFFER);
        assert_eq!(d[21],32);
        assert_eq!((d[22],d[23],d[24]),(0,0xffff_ffff,0));
        assert_eq!(d[26],0x24000);
    }
    #[test]
    fn log_is_always_next_to_exe_not_current_working_directory(){
        let exe=Path::new(r"C:\Test-Probe\windowed_ddraw_probe.exe");
        let log=log_path_for_executable(exe).unwrap();
        assert_eq!(log.parent(),exe.parent());
        assert_eq!(log.file_name().unwrap(),"windowed_ddraw_probe.log");
        assert!(log_path_for_executable(Path::new("")).is_err());
    }
    #[test]
    fn depth_format_mask_matches_wine_32bit_reference(){
        let forced=desc(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,640,480,Some(32));
        let auto=desc(DDSCAPS_ZBUFFER,640,480,Some(32));
        for d in [forced,auto] {
            assert_eq!(d[0],124);
            assert_eq!(d[1],DDSD_CAPS|DDSD_HEIGHT|DDSD_WIDTH|DDSD_PIXELFORMAT);
            assert_eq!(d[19],DDPF_ZBUFFER);
            assert_eq!(d[21],32);
            assert_eq!(d[22],0);
            assert_eq!(d[23],0xffff_ffff);
        }
        assert_eq!(forced[26],DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY);
        assert_eq!(auto[26],DDSCAPS_ZBUFFER);
        assert_eq!(DDBLT_DEPTHFILL,0x0200_0000);
    }
    #[test]
    fn ddpixelformat_depth_bit_mask_is_at_word_23_not_22(){
        let d=desc(DDSCAPS_ZBUFFER,640,480,Some(32));
        assert_eq!(d[18],32);
        assert_eq!(d[19],DDPF_ZBUFFER);
        assert_eq!(d[21],32);
        assert_eq!(d[22],0); // dwStencilBitDepth / dwRBitMask
        assert_eq!(d[23],0xffff_ffff); // dwZBitMask / dwGBitMask
        assert_eq!(d[24],0); // dwStencilBitMask
    }
    #[test]
    fn enumeration_preference_preserves_exact_driver_pixel_format(){
        let mut z24stencil=[0u32;8];
        z24stencil[0]=32;
        z24stencil[1]=DDPF_ZBUFFER|DDPF_STENCILBUFFER;
        z24stencil[3]=32;z24stencil[4]=8;z24stencil[5]=0x00ffffff;
        let mut pure=z24stencil;
        pure[1]=DDPF_ZBUFFER;pure[4]=0;pure[5]=0xffff_ffff;
        assert!(!pure_z32(&z24stencil));
        assert!(pure_z32(&pure));
        let selected=select_pure_z32(&[z24stencil,pure]);
        assert_eq!(selected,Some(pure));
        let d=make_z32_desc(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,selected);
        assert_eq!(&d[18..26],&pure);
        assert_eq!(d[22],0);
        assert_eq!(d[23],0xffff_ffff);
        assert_eq!(d[26],DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY);
        assert_eq!(select_pure_z32(&[z24stencil]),None);
        assert_eq!(make_z32_desc(DDSCAPS_ZBUFFER,None)[23],0xffff_ffff);
        let z800=make_z32_desc_for(DDSCAPS_ZBUFFER,Some(pure),800,600);
        assert_eq!((z800[3],z800[2]),(800,600));
        assert_eq!(&z800[18..26],&pure);
    }
    #[test]
    fn integer_pillarbox_for_4_3_sources_in_16_9_window(){
        assert_eq!(letterbox(640,480,960,540),
            Some(Rect{left:120,top:0,right:840,bottom:540}));
        assert_eq!(letterbox(800,600,1920,1080),
            Some(Rect{left:240,top:0,right:1680,bottom:1080}));
        assert_eq!(letterbox(640,480,1440,1080),
            Some(Rect{left:0,top:0,right:1440,bottom:1080}));
    }
    #[test]
    fn invalid_or_extreme_geometry_is_rejected(){
        assert!(letterbox(0,480,960,540).is_none());
        assert!(letterbox(640,0,960,540).is_none());
        assert!(letterbox(640,480,0,540).is_none());
        assert!(letterbox(640,480,960,0).is_none());
        assert!(letterbox(640,480,u32::MAX,u32::MAX).is_none());
    }
    #[test]
    fn standalone_borderless_mode_is_opt_in_only(){
        assert_eq!(parse_mode(&[]),Ok(ProbeMode::Windowed));
        assert_eq!(parse_mode(&["--borderless".to_owned()]),Ok(ProbeMode::Borderless));
        assert_eq!(parse_mode(&["--borderless-loop".to_owned()]),Ok(ProbeMode::BorderlessLoop));
        assert_eq!(parse_mode(&["--com-forward".to_owned()]),Ok(ProbeMode::ComForward));
        assert_eq!(parse_mode(&["--com-device".to_owned()]),Ok(ProbeMode::ComDevice));
        assert_eq!(parse_mode(&["--com-device-bridge".to_owned()]),Ok(ProbeMode::ComDeviceBridge));
        assert_eq!(parse_mode(&["--com-device-roundtrip".to_owned()]),Ok(ProbeMode::ComDeviceRoundtrip));
        assert_eq!(parse_mode(&["--com-surface-chain".to_owned()]),Ok(ProbeMode::ComSurfaceChain));
        assert_eq!(parse_mode(&["--com-virtual-flip".to_owned()]),Ok(ProbeMode::ComVirtualFlip));
        assert_eq!(parse_mode(&["--com-virtual-flip-800".to_owned()]),Ok(ProbeMode::ComVirtualFlip800));
        assert_eq!(parse_mode(&["--com-virtual-flip-transition".to_owned()]),Ok(ProbeMode::ComVirtualFlipTransition));
        assert_eq!((ProbeMode::ComVirtualFlip.source_mode().width,ProbeMode::ComVirtualFlip.source_mode().height),(640,480));
        assert_eq!((ProbeMode::ComVirtualFlip800.source_mode().width,ProbeMode::ComVirtualFlip800.source_mode().height),(800,600));
        let transition=probe_source_modes(ProbeMode::ComVirtualFlipTransition);
        assert_eq!(transition.len(),3);
        assert_eq!(transition.iter().map(|m|(m.width,m.height,m.bpp)).collect::<Vec<_>>(),
            vec![(800,600,16),(640,480,16),(800,600,16)]);
        assert_eq!(probe_source_modes(ProbeMode::ComVirtualFlip).len(),1);
        assert!(parse_mode(&["--fullscreen".to_owned()]).is_err());
        assert!(parse_mode(&["--borderless".to_owned(),"x".to_owned()]).is_err());
    }
    #[test]
    fn fullscreen_test_window_is_popup_without_exclusive_or_topmost(){
        let plan=plan_test_window(ProbeMode::Borderless,1920,1080).unwrap();
        assert_eq!((plan.x,plan.y,plan.width,plan.height),(0,0,1920,1080));
        assert_eq!((plan.client_width,plan.client_height),(1920,1080));
        assert_eq!(plan.style,WS_VISIBLE|WS_POPUP);
        assert_eq!(plan.style&WS_OVERLAPPEDWINDOW,0);
        assert!(plan_test_window(ProbeMode::Borderless,0,1080).is_err());
        let loop_plan=plan_test_window(ProbeMode::BorderlessLoop,1920,1080).unwrap();
        assert_eq!(loop_plan,plan);
        assert!(ProbeMode::BorderlessLoop.borderless());
        let com_plan=plan_test_window(ProbeMode::ComForward,1920,1080).unwrap();
        assert_eq!(com_plan,plan);
        assert!(ProbeMode::ComForward.borderless());
        let device_plan=plan_test_window(ProbeMode::ComDevice,1920,1080).unwrap();
        assert_eq!(device_plan,plan);
        assert!(ProbeMode::ComDevice.borderless());
        let bridge_plan=plan_test_window(ProbeMode::ComDeviceBridge,1920,1080).unwrap();
        assert_eq!(bridge_plan,plan);
        assert!(ProbeMode::ComDeviceBridge.borderless());
        let roundtrip_plan=plan_test_window(ProbeMode::ComDeviceRoundtrip,1920,1080).unwrap();
        assert_eq!(roundtrip_plan,plan);
        assert_eq!(plan_test_window(ProbeMode::ComSurfaceChain,1920,1080).unwrap(),plan);
        assert_eq!(plan_test_window(ProbeMode::ComVirtualFlip,1920,1080).unwrap(),plan);
        assert_eq!(plan_test_window(ProbeMode::ComVirtualFlip800,1920,1080).unwrap(),plan);
        assert_eq!(plan_test_window(ProbeMode::ComVirtualFlipTransition,1920,1080).unwrap(),plan);
        assert!(ProbeMode::ComVirtualFlipTransition.virtual_flip());
        let normal=plan_test_window(ProbeMode::Windowed,1920,1080).unwrap();
        assert_eq!((normal.client_width,normal.client_height),(960,540));
        assert_eq!(normal.style,WS_VISIBLE|WS_OVERLAPPEDWINDOW);
    }
    #[test]
    fn directdrawcreateex_requires_v7_then_legacy_v4_query(){
        assert_eq!(IID_IDIRECTDRAW7.d1,0x15e65ec0);
        assert_eq!(IID_IDIRECTDRAW7.d2,0x3b9c);
        assert_eq!(IID_IDIRECTDRAW7.d3,0x11d2);
        assert_eq!(IID_IDIRECTDRAW7.d4,[0xb9,0x2f,0x00,0x60,0x97,0x97,0xea,0x5b]);
        assert_ne!(IID_IDIRECTDRAW7.d1,IID_IDIRECTDRAW4.d1);
        assert_eq!(IID_IDIRECTDRAW4.d1,0x9c59509a);
    }
    #[test]
    fn normal_cooperative_mode_is_not_exclusive(){
        assert_eq!(DDSCL_NORMAL,8);
        assert_eq!(DDSCL_NORMAL&0x11,0);
        assert_eq!(std::mem::size_of::<Guid>(),16);
        assert_eq!(std::mem::size_of::<Rect>(),16);
    }
}
