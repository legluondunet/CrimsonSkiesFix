//! Experimental game-owned DirectDraw4 windowed resource adapter.
//! Exact path: opt-in IDirectDraw4 cooperative -> mode -> virtual Surface4 chain.
//! All native objects are created on the GAME's own DD4, not a probe DD4.
//! Never pretend a native HAL device was created: D3D3 receives native Surface4.
//!
//! This is intentionally an early in-game integration build. The game may use
//! unsupported COM methods, QI versions or window transitions. Opt-in only.
use std::{ffi::c_void,ptr,sync::Mutex};
use crate::{
    Hr,Guid,Hwnd,log,win32,
    borderless_contract::{AdapterContract,BackendEvidence,RenderTarget,SurfaceRole},
    presentation::{self,Mode,Chain},
    borderless_flip::{self,WindowedBlt},
    borderless_native_blt::NativeWindowedBlt,
    com_surface4_forward,
    com_surface4_abi::{Surface4VTable,SurfaceThis},
};

const E_FAIL:Hr=0x8000_4005u32 as i32;
const E_NOTIMPL:Hr=0x8000_4001u32 as i32;
const DDERR_GENERIC:Hr=0x8876_0000u32 as i32;
const DD_CAPS_PRIMARY:u32=0x0000_0200;
const DD_CAPS_OFFSCREEN:u32=0x0000_0040;
const DD_CAPS_3D:u32=0x0000_2000;
const DD_CAPS_VIDEOMEM:u32=0x0000_4000;
const DD_CAPS_Z:u32=0x0002_0000;
const DDSD_BACKBUFFERCOUNT:u32=0x20;
const DDSD_PIXELFORMAT:u32=0x1000;
const DDSCL_NORMAL:u32=8;
const DDSCL_EXCLUSIVE_FULLSCREEN:u32=0x13;
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
    let create:Create=std::mem::transmute(slot(d3d.ptr(),8));
    let mut device=ptr::null_mut();
    require(create(d3d.ptr(),&HAL,back.ptr(),&mut device,ptr::null_mut()),"D3D3 HAL CreateDevice")?;
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
fn check_native_rgb_depth(back:&Com,depth:&Com,mode:Mode)->Result<(),String>{
    unsafe{
        let b=native_desc(back)?;let z=native_desc(depth)?;
        if (b[3],b[2],b[21],b[22],b[23],b[24])!=(mode.width,mode.height,16,0xf800,0x07e0,0x001f) {
            return Err(format!("native RGB565 mismatch: {}x{} bpp={}",b[3],b[2],b[21]));
        }
        if (z[3],z[2],z[21])!=(mode.width,mode.height,32) ||
            z[19]&0x400==0 || z[23]==0 {
            return Err(format!("native Z32 mismatch: {}x{} bpp={}",z[3],z[2],z[21]));
        }
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
    physical_back:Com,
    physical_depth:Com,
    clipper:Com,
}
struct Runtime {
    dd4:usize,
    dd4_owner:Com, // strong COM parent ref for the entire experiment
    hwnd:usize,
    monitor:win32::Rect,
    generation:u64,
    exposed:bool,
    epoch:Option<Epoch>,
}
static RUNTIME:Mutex<Option<Runtime>>=Mutex::new(None);

/// Called from the real game's IDirectDraw4::SetCooperativeLevel hook.
/// The original game's normal/exclusive path is left alone if opt-in is 0.
/// Only accept a valid process-owned HWND and standard exclusive request.
pub unsafe fn cooperative(dd4:*mut c_void,hwnd:Hwnd,flags:u32,
    original:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr)->Option<Hr>{
    if flags&DDSCL_EXCLUSIVE_FULLSCREEN != DDSCL_EXCLUSIVE_FULLSCREEN || hwnd.is_null(){
        // Do not allow an old active graph to silently revert to native
        // DDSCL_EXCLUSIVE or DDSCL_NORMAL through another cooperative call.
        if RUNTIME.lock().ok().and_then(|r|r.as_ref().map(|x|x.exposed)).unwrap_or(false){
            log("WINDOWED-GAME: unexpected cooperative reconfiguration while virtual graph exposed: rejected");
            return Some(E_NOTIMPL);
        }
        return None;
    }
    let Some(monitor)=win32::monitor_for_game_window(hwnd) else{
        log("WINDOWED-GAME: rejected game HWND; original exclusive path retained");
        return None;
    };
    if monitor.left!=0 || monitor.top!=0 || monitor.right<=0 || monitor.bottom<=0 {
        log("WINDOWED-GAME: opt-in requires origin (0,0) primary monitor, falling back before exposure");
        return None;
    }
    let Ok(mut guard)=RUNTIME.lock() else{return None;};
    if guard.as_ref().is_some_and(|state|state.exposed){
        log("WINDOWED-GAME: repeated cooperative call while graph exposed: fail closed");
        return Some(E_FAIL);
    }
    // Prepare the actual DD4 cooperative level, but do not modify window
    // styles, suppress SetDisplayMode or expose any game surfaces yet.
    let hr=original(dd4,hwnd,DDSCL_NORMAL);
    if hr<0{
        log(&format!("WINDOWED-GAME: DDSCL_NORMAL rejected ({:#010x}), original exclusive path retained",hr as u32));
        return None;
    }
    // The game can Release its own DD4 interface while virtual surfaces
    // remain referenced. Hold one distinct strong DD4 COM reference.
    type AddRef=unsafe extern "system" fn(*mut c_void)->u32;
    let add_ref:AddRef=std::mem::transmute(slot(dd4,1));
    if add_ref(dd4)==0{
        log("WINDOWED-GAME: DD4 AddRef failed after DDSCL_NORMAL; cannot expose");
        return Some(E_FAIL);
    }
    *guard=Some(Runtime{dd4:dd4 as usize,dd4_owner:Com(dd4 as usize),hwnd:hwnd as usize,
        monitor,generation:0,exposed:false,epoch:None});
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
    let Ok(mut state)=RUNTIME.lock() else{return Some(E_FAIL);};
    let Some(runtime)=state.as_mut() else{return None};
    if runtime.dd4!=dd4 as usize{return None;}
    let mode=Mode{width:w,height:h,bpp};
    if !matches!((w,h,bpp),(640,480,16)|(800,600,16)){
        log(&format!("WINDOWED-GAME: unsupported mode {}x{}x{}, fail closed",w,h,bpp));
        if runtime.exposed{return Some(E_FAIL);}
        // Recovery is permitted only while no virtual COM interface exists.
        let coop:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr=
            match crate::orig(dd4,20){Some(addr)=>std::mem::transmute(addr),None=>return Some(E_FAIL)};
        let hr=coop(dd4,runtime.hwnd as Hwnd,DDSCL_EXCLUSIVE_FULLSCREEN);
        *state=None;
        if hr<0{return Some(hr);}
        return Some(original(dd4,w,h,bpp,refresh,flags));
    }
    if let Some(mut old)=runtime.epoch.take(){
        if let Err(hr)=com_surface4_forward::retire_graph_primary_flip(old.primary.ptr()){
            log(&format!("WINDOWED-GAME: retiring old primary failed {:#010x}",hr as u32));
            runtime.epoch=Some(old);
            return Some(E_FAIL);
        }
        log("WINDOWED-GAME: old virtual chain retired; stale Flip/Blt disarmed");
    }
    let result=prepare_epoch(dd4,runtime.hwnd as Hwnd,runtime.monitor,mode,runtime.generation+1);
    match result {
        Ok(next)=>{
            runtime.generation+=1;
            runtime.epoch=Some(next);
            let area=runtime.monitor;
            // Apply HWND style only after physical HAL and real Flip->Blt
            // preflight; the game now has a complete windowed COM graph.
            if let Err(err)=win32::borderless_window(runtime.hwnd as Hwnd,area){
                log(&format!("WINDOWED-GAME: borderless HWND style failed: {err}"));
                if runtime.exposed{return Some(E_FAIL);}
                let coop:unsafe extern "system" fn(*mut c_void,Hwnd,u32)->Hr=
                    match crate::orig(dd4,20){Some(addr)=>std::mem::transmute(addr),None=>return Some(E_FAIL)};
                let hr=coop(dd4,runtime.hwnd as Hwnd,DDSCL_EXCLUSIVE_FULLSCREEN);
                *state=None;
                if hr<0{return Some(hr);}
                return Some(original(dd4,w,h,bpp,refresh,flags));
            }
            log(&format!("WINDOWED-GAME: mode={}x{}x{} native HAL/Z32/virtual Surface4 ready; desktop unchanged; generation={}",
                w,h,bpp,runtime.generation));
            Some(0)
        }
        Err(err)=>{
            log(&format!("WINDOWED-GAME: native preflight failed before exposure: {err}"));
            if runtime.exposed{
                // Strictly no mid-frame switch back to exclusive.
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
unsafe fn prepare_epoch(dd4:*mut c_void,hwnd:Hwnd,monitor:win32::Rect,
    mode:Mode,generation:u64)->Result<Epoch,String>{
    let client=(monitor.right as u32,monitor.bottom as u32);
    let real_primary=alloc(dd4,desc(DD_CAPS_PRIMARY,0,0,None))?;
    let clipper=new_clipper(dd4,hwnd,&real_primary)?;
    let real_back=alloc(dd4,desc(DD_CAPS_OFFSCREEN|DD_CAPS_3D|DD_CAPS_VIDEOMEM,
        mode.width,mode.height,Some(16)))?;
    // The real owner Wine HAL advertises pure 32-bit depth with 24 bits
    // of active Z (0x00ffffff); a native failure is never masked.
    let real_depth=alloc(dd4,desc(DD_CAPS_Z|DD_CAPS_VIDEOMEM,
        mode.width,mode.height,Some(32)))
        .or_else(|_|alloc(dd4,desc(DD_CAPS_Z,mode.width,mode.height,Some(32))))?;
    check_native_rgb_depth(&real_back,&real_depth,mode)?;
    attach_depth(&real_back,&real_depth)?;
    native_fill(&real_depth,true)?;
    native_fill(&real_back,false)?;
    prove_hal(dd4,&real_back)?;
    let primary=Com::from_out(com_surface4_forward::new_graph_surface4(real_primary.ptr(),SurfaceRole::Primary,mode)
        .map_err(|hr|format!("virtual primary: {:#010x}",hr as u32))?)?;
    let back=Com::from_out(com_surface4_forward::new_graph_surface4(real_back.ptr(),SurfaceRole::Backbuffer,mode)
        .map_err(|hr|format!("virtual backbuffer: {:#010x}",hr as u32))?)?;
    let depth=Com::from_out(com_surface4_forward::new_graph_surface4(real_depth.ptr(),SurfaceRole::Depth,mode)
        .map_err(|hr|format!("virtual depth: {:#010x}",hr as u32))?)?;
    com_surface4_forward::attach_graph_surface4(primary.ptr(),back.ptr())
        .map_err(|hr|format!("attach virtual BB {:#010x}",hr as u32))?;
    com_surface4_forward::attach_graph_surface4(back.ptr(),depth.ptr())
        .map_err(|hr|format!("attach virtual Z32 {:#010x}",hr as u32))?;

    let mut contract=AdapterContract::prepare(mode,client,Chain{
        front_to_back:true,back_to_depth:true,mipmaps:false
    }).map_err(|e|format!("preflight: {e:?}"))?;
    // Before exposure, the same physical backend from the standalone
    // probe must perform the actual black-clear + 4:3 stretch pair.
    let mut backend=NativeWindowedBlt::from_borrowed_com(
        real_primary.ptr(),real_back.ptr(),(0,0),client,mode
    ).map_err(|hr|format!("native Blt backend: {:#010x}",hr as u32))?;
    let plan=contract.plan.viewport;
    borderless_flip::execute_windowed_blt_pair(&mut backend,mode,client,plan)
        .map_err(|e|format!("native two-Blt preflight: {e:?}"))?;
    contract.expose(BackendEvidence{
        process_owned_window:true,normal_directdraw:true,
        render_target:RenderTarget::RealDirectDrawSurface4,
        render_target_caps:DD_CAPS_3D,render_target_is_rgb565:true,
        real_zbuffer_32:true,primary_with_clipper:true,
        working_blt:true,complete_surface_com:true
    }).map_err(|e|format!("graph evidence: {e:?}"))?;
    com_surface4_forward::arm_graph_primary_flip(
        primary.ptr(),&contract,generation,(0,0),client
    ).map_err(|hr|format!("arm graph: {:#010x}",hr as u32))?;
    Ok(Epoch{mode,primary,back,depth,physical_primary:real_primary,
        physical_back:real_back,physical_depth:real_depth,clipper})
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
    #[test] fn descriptors_remain_native_124_byte_surface4(){
        let p=desc(DD_CAPS_PRIMARY,0,0,None);
        assert_eq!((p[0],p[1],p[26]),(124,1,0x200));
        let z=desc(DD_CAPS_Z|DD_CAPS_VIDEOMEM,640,480,Some(32));
        assert_eq!((z[0],z[2],z[3],z[19],z[21],z[23]),(124,480,640,0x400,32,0x00ff_ffff));
        let b=desc(DD_CAPS_OFFSCREEN|DD_CAPS_3D|DD_CAPS_VIDEOMEM,800,600,Some(16));
        assert_eq!((b[2],b[3],b[21],b[22],b[23],b[24]),(600,800,16,0xf800,0x07e0,0x001f));
    }
}