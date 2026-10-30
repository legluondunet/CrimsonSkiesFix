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
use std::{ffi::c_void,ptr};

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
const DDBLT_COLORFILL:u32=0x400;
const DDBLT_WAIT:u32=0x0100_0000;
const WS_OVERLAPPEDWINDOW:u32=0x00cf_0000;
const WS_VISIBLE:u32=0x1000_0000;
const SM_CXSCREEN:i32=0;
const SM_CYSCREEN:i32=1;
#[repr(C)]
#[derive(Clone,Copy)]
struct Guid {d1:u32,d2:u16,d3:u16,d4:[u8;8]}
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
    println!("{label}: HRESULT={:#010x} {}",hr as u32,if hr>=0{"OK"}else{"FAIL"});
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
        if bpp==16 && caps&DDSCAPS_ZBUFFER==0{
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
fn run()->Result<(),String>{
    if std::mem::size_of::<usize>()!=4 {
        return Err("This executable requires Windows x86 (32-bit)".into());
    }
    let before=unsafe{(GetSystemMetrics(SM_CXSCREEN),GetSystemMetrics(SM_CYSCREEN))};
    println!("WINDOWED-DDRAW-PROBE v0.1.25; standalone test; desktop={}x{}",before.0,before.1);
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
    let mut dd=ptr::null_mut();
    unsafe{check("DirectDrawCreateEx(IDirectDraw4)",
        DirectDrawCreateEx(ptr::null(),&mut dd,&IID_IDIRECTDRAW4,ptr::null_mut()))?;}
    if dd.is_null(){return Err("DirectDrawCreateEx returned NULL".into());}
    let ddraw=Com(dd);
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
    let depth=unsafe{create_surface(&ddraw,"32-bit Z-buffer",
        desc(DDSCAPS_ZBUFFER|DDSCAPS_VIDEOMEMORY,640,480,Some(32)))?};
    unsafe{
        let attach:unsafe extern "system" fn(Ptr,Ptr)->Hr=
            std::mem::transmute(slot(target.0,3));
        check("target.AddAttachedSurface(Z)",attach(target.0,depth.0))?;
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
    println!("PASS: windowed primary + clipper + RGB565 3D target + Z32 + Blt + D3D3 HAL; desktop preserved");
    // Show the owned test window briefly to confirm visible Blt. No game
    // objects, input interception, or persistence is involved.
    unsafe{Sleep(1000);}
    Ok(())
}
fn main(){
    if let Err(e)=run(){
        eprintln!("PROBE FAILED: {e}");
        std::process::exit(1);
    }
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
        assert_eq!((d[22],d[23],d[24]),(0,0,0));
        assert_eq!(d[26],0x24000);
    }
    #[test]
    fn normal_cooperative_mode_is_not_exclusive(){
        assert_eq!(DDSCL_NORMAL,8);
        assert_eq!(DDSCL_NORMAL&0x11,0);
        assert_eq!(std::mem::size_of::<Guid>(),16);
        assert_eq!(std::mem::size_of::<Rect>(),16);
    }
}
