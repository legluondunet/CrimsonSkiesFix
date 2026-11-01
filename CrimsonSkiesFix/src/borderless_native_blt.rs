//! A genuine DirectDraw4 surface Blt backend for the Flip-to-Blt plan.
//! Imported by the standalone probe and by the DLL, but NEVER activated
//! by any in-game hook yet. Not a COM surface facade or virtual primary.

#![allow(dead_code)]
use std::{ffi::c_void,ptr};
use crate::{borderless_flip::WindowedBlt,presentation::{Mode,Viewport}};
type Ptr=*mut c_void;
const BAD:i32=0x80070057u32 as i32;
const WAIT:u32=0x01000000;
const FILL:u32=0x00000400;

#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Rect{pub left:i32,pub top:i32,pub right:i32,pub bottom:i32}
type Blt=unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->i32;

pub struct NativeWindowedBlt {
    primary:Ptr,
    source_surface:Ptr,
    origin:(i32,i32),
    client:(u32,u32),
    source:Mode,
    // Borrowed DD4 offscreen RGB565 scratch owned by the current game epoch.
    // Never use it outside the matching native Surface4 graph lifetime.
    menu_scratch:Ptr,
    menu_prepared:bool,
}
impl NativeWindowedBlt {
    /// SAFETY: caller retains live IDirectDrawSurface4 COM references to
    /// primary and source for this object's entire use. The primary must
    /// be in normal mode with a working HWND clipper; source must be a real
    /// 16-bit RGB565 D3D render target. These cannot be proven from pointers.
    pub unsafe fn from_borrowed_com(
        primary:Ptr,source_surface:Ptr,origin:(i32,i32),
        client:(u32,u32),source:Mode
    )->Result<Self,i32>{
        if primary.is_null()||source_surface.is_null()||
           client.0==0||client.1==0||
           !(matches!((source.width,source.height),(640,480)|(800,600)) ||
             (source.width,source.height)==client)||
           source.bpp!=16{return Err(BAD);}
        Ok(Self{primary,source_surface,origin,client,source,menu_scratch:ptr::null_mut(),menu_prepared:false})
    }
    /// Called during graph initialization; the caller retains the strong
    /// COM ownership until the entire epoch is retired.
    pub fn set_menu_scratch(&mut self,scratch:Ptr){
        self.menu_scratch=scratch;
        self.menu_prepared=false;
    }

    /// A menu frame is copied and labeled entirely offscreen, then promoted
    /// by the existing single Blt. The copy never touches the game's own
    /// 3D/backbuffer surface and cannot outlive a Flip session.
    pub fn prepare_menu_frame(&mut self){
        self.menu_prepared=false;
        if self.menu_scratch.is_null(){return;}
        let dest=self.menu_scratch;
        let source=self.source_surface;
        let copied=unsafe{
            let table=*(dest.cast::<*const usize>());
            if table.is_null(){return;}
            let blt:Blt=std::mem::transmute(*table.add(5));
            blt(dest,ptr::null(),source,ptr::null(),WAIT,ptr::null_mut())
        };
        if copied<0{
            crate::menu_brand::report_copy_failure(copied);
            return;
        }
        let viewport=Rect{left:0,top:0,right:self.source.width as i32,bottom:self.source.height as i32};
        self.menu_prepared=crate::menu_brand::draw_main_menu(dest,viewport);
    }
    pub fn clear_menu_frame(&mut self){
        self.menu_prepared=false;
    }
    fn source_for_present(&self)->Ptr{
        if self.menu_prepared && !self.menu_scratch.is_null(){
            self.menu_scratch
        }else{
            self.source_surface
        }
    }
    /// The real windowed primary is owned by the live Flip session.
    /// The menu brander borrows this pointer only after successful Blt,
    /// never keeps or releases the COM interface.
    pub fn primary_for_overlay(&self)->Ptr{self.primary}
    /// Keep the label inside the aspect-correct 4:3 content, not in
    /// the physical desktop pillarbox or a guessed HWND rectangle.
    pub fn overlay_viewport(&self)->Option<Rect>{
        let fit=crate::presentation::fit_viewport(
            self.source.width,self.source.height,self.client.0,self.client.1)?;
        self.screen_rect(fit).ok()
    }
    fn screen_rect(&self,view:Viewport)->Result<Rect,i32>{
        if view.width==0||view.height==0||
           view.x.checked_add(view.width).is_none_or(|x|x>self.client.0)||
           view.y.checked_add(view.height).is_none_or(|y|y>self.client.1){
            return Err(BAD);
        }
        let left=i32::try_from(i64::from(self.origin.0)+i64::from(view.x))
            .map_err(|_|BAD)?;
        let top=i32::try_from(i64::from(self.origin.1)+i64::from(view.y))
            .map_err(|_|BAD)?;
        let right=i32::try_from(i64::from(self.origin.0)+i64::from(view.x)+i64::from(view.width))
            .map_err(|_|BAD)?;
        let bottom=i32::try_from(i64::from(self.origin.1)+i64::from(view.y)+i64::from(view.height))
            .map_err(|_|BAD)?;
        Ok(Rect{left,top,right,bottom})
    }
    unsafe fn blt(&self,rect:&Rect,source:Ptr,flags:u32,fx:*mut u32)->Result<(),i32>{
        let vtable=*(self.primary.cast::<*const usize>());
        let call:Blt=std::mem::transmute(*vtable.add(5));
        let hr=call(self.primary,rect,source,ptr::null(),flags,fx);
        if hr<0{Err(hr)}else{Ok(())}
    }
}
impl WindowedBlt for NativeWindowedBlt {
    fn clear_full_window_black(&mut self,w:u32,h:u32)->Result<(),i32>{
        if (w,h)!=self.client{return Err(BAD);}
        let dest=self.screen_rect(Viewport{x:0,y:0,width:w,height:h})?;
        let mut fx=[0u32;25];fx[0]=100;fx[20]=0;
        unsafe{self.blt(&dest,ptr::null_mut(),FILL|WAIT,fx.as_mut_ptr())}
    }
    fn stretch_rgb565_into_viewport(&mut self,mode:Mode,view:Viewport)->Result<(),i32>{
        if mode!=self.source{return Err(BAD);}
        let dest=self.screen_rect(view)?;
        let source=self.source_for_present();
        unsafe{self.blt(&dest,source,WAIT,ptr::null_mut())}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::borderless_flip::{execute_windowed_blt_pair,FlipBridgeError};
    use std::sync::Mutex;
    #[derive(Clone,Copy,Debug,PartialEq,Eq)]
    struct Call{rect:Rect,source:bool,flags:u32,fx_size:u32,fill:u32}
    #[repr(C)]
    struct FakeSurface{
        vtable:*const usize,
        calls:Mutex<Vec<Call>>,
        fail_at:usize,
    }
    unsafe extern "system" fn fake_blt(
        this:Ptr,rect:*const Rect,source:Ptr,_src:*const Rect,
        flags:u32,fx:*mut u32
    )->i32{
        let s=&*(this.cast::<FakeSurface>());
        let mut log=s.calls.lock().unwrap();
        log.push(Call{rect:*rect,source:!source.is_null(),flags,
                      fx_size:if fx.is_null(){0}else{*fx},
                      fill:if fx.is_null(){0}else{*fx.add(20)}});
        if s.fail_at==log.len(){-17}else{0}
    }
    fn mode()->Mode{Mode{width:640,height:480,bpp:16}}
    fn objects(fail_at:usize)->([usize;45],FakeSurface,FakeSurface){
        let mut vt=[0usize;45];vt[5]=fake_blt as usize;
        let p=FakeSurface{vtable:ptr::null(),calls:Mutex::new(Vec::new()),fail_at};
        let s=FakeSurface{vtable:ptr::null(),calls:Mutex::new(Vec::new()),fail_at:0};
        (vt,p,s)
    }
    #[test]
    fn original_frame_is_presented_unless_menu_composition_succeeded(){
        let (_vt,mut primary,mut original)=objects(0);
        let mut scratch=FakeSurface{
            vtable:ptr::null(),calls:Mutex::new(Vec::new()),fail_at:0
        };
        let original_ptr=(&mut original as *mut FakeSurface).cast();
        let scratch_ptr=(&mut scratch as *mut FakeSurface).cast();
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            (&mut primary as *mut FakeSurface).cast(),original_ptr,
            (0,0),(1920,1080),mode()
        ).unwrap()};
        assert_eq!(backend.source_for_present(),original_ptr);
        backend.set_menu_scratch(scratch_ptr);
        assert_eq!(backend.source_for_present(),original_ptr);
        backend.menu_prepared=true;
        assert_eq!(backend.source_for_present(),scratch_ptr);
        backend.clear_menu_frame();
        assert_eq!(backend.source_for_present(),original_ptr);
        backend.set_menu_scratch(ptr::null_mut());
        backend.menu_prepared=true; // even inconsistent flags fail closed
        assert_eq!(backend.source_for_present(),original_ptr);
    }
    #[test]
    fn real_surface4_slot5_dispatch_and_flags(){
        let (vt,mut p,mut s)=objects(0);
        p.vtable=vt.as_ptr();s.vtable=vt.as_ptr();
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            (&mut p as *mut FakeSurface).cast(),(&mut s as *mut FakeSurface).cast(),
            (0,0),(1920,1080),mode()).unwrap()};
        assert_eq!(execute_windowed_blt_pair(&mut backend,mode(),(1920,1080),
            Viewport{x:240,y:0,width:1440,height:1080}),Ok(()));
        let calls=p.calls.lock().unwrap();
        assert_eq!(*calls,[
            Call{rect:Rect{left:0,top:0,right:1920,bottom:1080},
                 source:false,flags:WAIT|FILL,fx_size:100,fill:0},
            Call{rect:Rect{left:240,top:0,right:1680,bottom:1080},
                 source:true,flags:WAIT,fx_size:0,fill:0}
        ]);
    }
    #[test]
    fn origin_translation_and_hresult_failure(){
        let (vt,mut p,mut s)=objects(2);
        p.vtable=vt.as_ptr();s.vtable=vt.as_ptr();
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            (&mut p as *mut FakeSurface).cast(),(&mut s as *mut FakeSurface).cast(),
            (84,110),(960,540),mode()).unwrap()};
        assert_eq!(execute_windowed_blt_pair(&mut backend,mode(),(960,540),
            Viewport{x:120,y:0,width:720,height:540}),
            Err(FlipBridgeError::StretchFailed(-17)));
        let calls=p.calls.lock().unwrap();
        assert_eq!(calls.len(),2);
        assert_eq!(calls[0].rect,Rect{left:84,top:110,right:1044,bottom:650});
        assert_eq!(calls[1].rect,Rect{left:204,top:110,right:924,bottom:650});
    }
    #[test]
    fn invalid_dimensions_rejected_without_vtable_call(){
        let (vt,mut p,mut s)=objects(0);
        p.vtable=vt.as_ptr();s.vtable=vt.as_ptr();
        let primary=(&mut p as *mut FakeSurface).cast();
        let source=(&mut s as *mut FakeSurface).cast();
        assert!(unsafe{NativeWindowedBlt::from_borrowed_com(
            ptr::null_mut(),source,(0,0),(960,540),mode())}.is_err());
        let mut b=unsafe{NativeWindowedBlt::from_borrowed_com(
            primary,source,(0,0),(960,540),mode()).unwrap()};
        assert_eq!(b.clear_full_window_black(1920,1080),Err(BAD));
        assert_eq!(b.stretch_rgb565_into_viewport(mode(),
            Viewport{x:900,y:0,width:300,height:540}),Err(BAD));
        assert!(p.calls.lock().unwrap().is_empty());
    }
}
