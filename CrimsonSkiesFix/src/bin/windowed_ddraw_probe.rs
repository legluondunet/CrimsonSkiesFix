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


type Ptr=*mut c_void;
type Hr=i32;
const S_OK:Hr=0;
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
    fn UpdateWindow(hwnd:Ptr)->i32;
}
#[link(name="kernel32")]
extern "system" {
    fn Sleep(ms:u32);
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

/// Owns one COM reference and releases it exactly once when leaving scope.
/// No virtual surface is advertised and no borrowed COM pointer is retained.
struct Com(Ptr);
impl Com {
    fn is_null(&self)->bool{self.0.is_null()}
}
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
fn make_z32_desc(caps:u32,selected:Option<[u32;8]>)->[u32;31]{
    let mut requested=desc(caps,640,480,Some(32));
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
fn run()->Result<(),String>{
    if std::mem::size_of::<usize>()!=4 {
        return Err("This executable requires Windows x86 (32-bit)".into());
    }
    let before=unsafe{(GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN))};
    probe_line(&format!("WINDOWED-DDRAW-PROBE v0.1.29; standalone test; desktop={}x{}",before.0,before.1));
    let class=wide("STATIC");
    let title=wide("CrimsonSkiesFix windowed DirectDraw test (not the game)");
    let hwnd=unsafe {
        CreateWindowExW(0,class.as_ptr(),title.as_ptr(),
            WS_VISIBLE|WS_OVERLAPPEDWINDOW,80,80,920,720,
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
    let target=unsafe{create_surface(&ddraw,"RGB565 offscreen 3D target",
        desc(DDSCAPS_OFFSCREEN|DDSCAPS_3DDEVICE|DDSCAPS_VIDEOMEMORY,
             640,480,Some(16)))?};
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
            make_z32_desc(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,selected)) {
            Ok(surface)=>(surface,"explicit-video-memory"),
            Err(err)=>{
                probe_line(&format!(
                    "Z32-RETRY: explicit video memory rejected ({err}); retrying 32-bit depth with driver-selected memory placement"));
                let surface=create_surface(&ddraw,"32-bit Z-buffer (driver-selected memory)",
                    make_z32_desc(DDSCAPS_ZBUFFER,selected))?;
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
    let mut origin=Point::default();
    let mut client=Rect{left:0,top:0,right:0,bottom:0};
    if unsafe{GetClientRect(window.0,&mut client)}==0{return Err("GetClientRect failed".into());}
    if unsafe{ClientToScreen(window.0,&mut origin)}==0{return Err("ClientToScreen failed".into());}
    // Deliberately no stretch: direct 640x480 RGB565->desktop format Blt.
    // The clipper limits drawing to our own test window's client rectangle.
    let dst=Rect{left:origin.x+10,top:origin.y+10,
                 right:origin.x+650,bottom:origin.y+490};
    unsafe{
        let blt:unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->Hr=
            std::mem::transmute(slot(primary.0,5));
        check("primary.Blt(RGB565 target, window clipper)",
             blt(primary.0,&dst,target.0,ptr::null(),DDBLT_WAIT,ptr::null_mut()))?;
    }
    let mut device=ptr::null_mut();
    unsafe{
        let create:unsafe extern "system" fn(Ptr,*const Guid,Ptr,*mut Ptr,Ptr)->Hr=
            std::mem::transmute(slot(d3d.0,8));
        check("IDirect3D3.CreateDevice(HAL, RGB565 target)",
            create(d3d.0,&CLSID_D3D_HAL,target.0,&mut device,ptr::null_mut()))?;
    }
    if device.is_null(){return Err("CreateDevice returned NULL".into());}
    let _device=Com(device);
    let after=unsafe{(GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN))};
    if before!=after{return Err(format!("Desktop changed unexpectedly: {:?} -> {:?}",before,after));}
    probe_line("PASS: windowed primary + clipper + RGB565 3D target + Z32 + Blt + D3D3 HAL; desktop preserved");
    // Show the owned test window briefly to confirm visible Blt. No game
    // objects, input interception, or persistence is involved.
    unsafe{Sleep(1000);}
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
    probe_line("CrimsonSkiesFix standalone windowed resource probe v0.1.29");
    probe_line(&format!("Log file: {}",path.display()));
    probe_line("All DirectDraw HRESULTs and final result are saved in this log; Wine diagnostics are excluded.");
    if let Err(e)=run(){
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
