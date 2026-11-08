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
const BLACKNESS:u32=0x00000042; // Win32 GDI PatBlt raster op
#[link(name="user32")]
extern "system" {
    fn GetDC(hwnd:Ptr)->Ptr;
    fn ReleaseDC(hwnd:Ptr,dc:Ptr)->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn PatBlt(dc:Ptr,x:i32,y:i32,width:i32,height:i32,rop:u32)->i32;
}

#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Rect{pub left:i32,pub top:i32,pub right:i32,pub bottom:i32}
type Blt=unsafe extern "system" fn(Ptr,*const Rect,Ptr,*const Rect,u32,*mut u32)->i32;

pub struct NativeWindowedBlt {
    primary:Ptr,
    source_surface:Ptr,
    origin:(i32,i32),
    client:(u32,u32), // contract/render pixel extent
    output_client:(u32,u32), // Win32 DPI-logical HWND coordinates
    source:Mode,
    // Borrowed DD4 offscreen RGB565 scratch owned by the current game epoch.
    // Never use it outside the matching native Surface4 graph lifetime.
    menu_scratch:Ptr,
    menu_prepared:bool,
    // Optional *native Windows only* diagnostic. DDraw primary may report
    // successful sidebar fills while HWND compositor gutters stay white.
    // Try GDI only OUTSIDE the video viewport, after the completed RGB565
    // DDraw Blt. Never an input/focus hook, never active on Wine by default.
    gdi_sidebar_hwnd:usize,
    gdi_sidebar_reported:bool,
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
        Ok(Self{primary,source_surface,origin,client,output_client:client,source,menu_scratch:ptr::null_mut(),menu_prepared:false,
            gdi_sidebar_hwnd:0,gdi_sidebar_reported:false})
    }
    /// Native Windows movie-only A/B: the game's *logical* 800x600x16
    /// presentation bridge may borrow a separately verified real XRGB8888
    /// 800x600 backbuffer. It can Blt directly to the 32-bit native primary
    /// without CPU RGB565 conversion, matching the successful original
    /// exclusive Windows movie target format. The caller MUST verify that
    /// native format, DD4 HAL and clipper before invoking this constructor.
    /// Do not use this with menu RGB565 branding scratch or on Wine/Proton.
    pub unsafe fn from_borrowed_com_native_movie_rgb32(
        primary:Ptr,source_surface:Ptr,origin:(i32,i32),
        client:(u32,u32),source:Mode
    )->Result<Self,i32>{
        if (source.width,source.height,source.bpp)!=(800,600,16){
            return Err(BAD);
        }
        Self::from_borrowed_com(primary,source_surface,origin,client,source)
    }
    /// Windowed DirectDraw primary coordinates on native Windows are the
    /// DPI-logical Win32 client size, while the 3D render graph must remain
    /// the physical monitor raster (1920x1200 at 125% scaling). Convert
    /// only final presentation rectangles, NOT source modes/FOV/HUD/Z.
    /// The independent probe and Wine/Proton use output_client==client.
    pub fn set_output_client(&mut self,output:(u32,u32))->Result<(),i32>{
        // Shared with the WndProc's 4:3 input mapping. Win32 DPI
        // virtualization can round either dimension by one output pixel;
        // never reject an otherwise correct native monitor on that basis.
        if !crate::presentation::dpi_output_matches(self.client,output){
            return Err(BAD);
        }
        self.output_client=output;
        Ok(())
    }
    /// Only the verified game HWND may be passed. The caller gates this
    /// explicitly behind the debug option + native Windows bootstrap.
    /// A zero HWND disables it without changing the real DirectDraw path.
    pub fn enable_native_gdi_sidebars(&mut self,hwnd:usize){
        self.gdi_sidebar_hwnd=hwnd;
        self.gdi_sidebar_reported=false;
    }
    fn logical_edge(&self,edge:u32,horizontal:bool)->Result<i32,i32>{
        let (logical,physical)=if horizontal{
            (self.output_client.0,self.client.0)
        }else{(self.output_client.1,self.client.1)};
        if edge>physical{return Err(BAD);}
        let scaled=u64::from(edge)*u64::from(logical)/u64::from(physical);
        i32::try_from(scaled).map_err(|_|BAD)
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
        if self.menu_scratch.is_null(){
            crate::menu_brand::mark_not_drawn();
            return;
        }
        let dest=self.menu_scratch;
        let source=self.source_surface;
        let copied=unsafe{
            let table=*(dest.cast::<*const usize>());
            if table.is_null(){
                crate::menu_brand::mark_not_drawn();
                return;
            }
            let blt:Blt=std::mem::transmute(*table.add(5));
            blt(dest,ptr::null(),source,ptr::null(),WAIT,ptr::null_mut())
        };
        if copied<0{
            crate::menu_brand::mark_not_drawn();
            crate::menu_brand::report_copy_failure(copied);
            return;
        }
        let viewport=Rect{left:0,top:0,right:self.source.width as i32,bottom:self.source.height as i32};
        self.menu_prepared=crate::menu_brand::draw_main_menu(dest,viewport);
    }
    pub fn clear_menu_frame(&mut self){
        self.menu_prepared=false;
    }
    /// Whether the next successful present uses the separately composed
    /// MainMenu frame rather than the original unchanged game backbuffer.
    pub fn menu_frame_prepared(&self)->bool{self.menu_prepared && !self.menu_scratch.is_null()}
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
        // Both left and right edges must be mapped, not just the width,
        // so 4:3 menus stay centered after DPI virtualization and we never
        // crop the right/bottom of a physical screenshot.
        let left=self.origin.0.checked_add(self.logical_edge(view.x,true)?).ok_or(BAD)?;
        let top=self.origin.1.checked_add(self.logical_edge(view.y,false)?).ok_or(BAD)?;
        let right=self.origin.0.checked_add(
            self.logical_edge(view.x+view.width,true)?).ok_or(BAD)?;
        let bottom=self.origin.1.checked_add(
            self.logical_edge(view.y+view.height,false)?).ok_or(BAD)?;
        Ok(Rect{left,top,right,bottom})
    }
    /// Native Windows 11 may repaint the sidebars white AFTER the first
    /// full-window black clear, e.g. WM_PAINT/DPI compositor updates.
    /// Refresh ONLY the pillarbox regions, never the moving 4:3 image,
    /// before each frame when the render raster differs from HWND logical
    /// coordinates. This avoids the legacy full-screen clear flicker.
    /// Wine/probe (output_client==client) remains unchanged.
    fn maintain_native_black_sidebars(&mut self,view:Viewport)->Result<(),i32>{
        if self.output_client==self.client || view.x==0 ||
           view.x.checked_add(view.width).is_none_or(|x|x>=self.client.0){
            return Ok(());
        }
        if view.y!=0 || view.height!=self.client.1{return Err(BAD);}
        let content=self.screen_rect(view)?;
        let left=self.screen_rect(Viewport{x:0,y:0,width:self.client.0,height:self.client.1})?;
        let mut fx=[0u32;25];fx[0]=100;fx[20]=0;
        for bar in [
            Rect{left:left.left,top:left.top,right:content.left,bottom:left.bottom},
            Rect{left:content.right,top:left.top,right:left.right,bottom:left.bottom}
        ] {
            if bar.right>bar.left {
                unsafe{self.blt(&bar,ptr::null_mut(),FILL|WAIT,fx.as_mut_ptr())?;}
            }
        }
        Ok(())
    }
    /// Experimental nonfatal GDI draw after native DDraw color presentation.
    /// The existing DirectDraw black bars already reported S_OK in owner
    /// tests but stayed white on Windows 11 at 125% DPI. Use an HWND client
    /// DC (logical pixels), never the physical primary DC or a screen DC.
    /// Do not touch the central 4:3 image; GDI failure never fakes Blt S_OK.
    fn try_native_gdi_sidebars(&mut self,view:Viewport){
        if self.gdi_sidebar_hwnd==0 || self.output_client==self.client ||
           view.y!=0 || view.height!=self.client.1 || view.x==0 {
            return;
        }
        let Ok(content)=self.screen_rect(view) else{return;};
        let Ok(client_w)=i32::try_from(self.output_client.0) else{return;};
        let Ok(client_h)=i32::try_from(self.output_client.1) else{return;};
        let Some(left)=content.left.checked_sub(self.origin.0) else{return;};
        let Some(right)=content.right.checked_sub(self.origin.0) else{return;};
        if left<=0 || right<=left || right>=client_w || client_h<=0{return;}
        // Avoid acquiring a GDI DC for inactive/minimized windows during
        // Alt+Tab; only the existing DD4 Blt runs while focus is away.
        if !crate::window_exposure::sample_game_window(self.gdi_sidebar_hwnd)
            .is_some_and(|state|state.foreground) {return;}
        let (mut ok_left,mut ok_right,mut released)=(false,false,false);
        unsafe{
            let hwnd=self.gdi_sidebar_hwnd as Ptr;
            let dc=GetDC(hwnd);
            if !dc.is_null(){
                ok_left=PatBlt(dc,0,0,left,client_h,BLACKNESS)!=0;
                ok_right=PatBlt(dc,right,0,client_w-right,client_h,BLACKNESS)!=0;
                released=ReleaseDC(hwnd,dc)!=0;
            }
        }
        if !self.gdi_sidebar_reported {
            self.gdi_sidebar_reported=true;
            crate::log(&format!(
                "NATIVE-GDI-SIDEBARS: HWND client={}x{} logical bars=0..{} and {}..{} left={} right={} dc_released={} (diagnostic only)",
                client_w,client_h,left,right,client_w,ok_left,ok_right,released
            ));
        }
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
        self.maintain_native_black_sidebars(view)?;
        let source=self.source_for_present();
        unsafe{self.blt(&dest,source,WAIT,ptr::null_mut())?;}
        // Only after a successful native presentation. Do not change the
        // game's Flip result if the optional GDI experiment fails.
        self.try_native_gdi_sidebars(view);
        Ok(())
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
        let mut vt=[0usize;45];vt[5]=fake_blt as *const () as usize;
        let p=FakeSurface{vtable:ptr::null(),calls:Mutex::new(Vec::new()),fail_at};
        let s=FakeSurface{vtable:ptr::null(),calls:Mutex::new(Vec::new()),fail_at:0};
        (vt,p,s)
    }
    #[test]
    fn native_rgb32_movie_backend_is_strictly_800x600_logical_16(){
        let primary=1usize as Ptr;
        let back=2usize as Ptr;
        for (w,h,bpp,accepted) in [
            (800,600,16,true),
            (640,480,16,false),
            (1920,1200,16,false),
            (800,600,32,false)
        ]{
            let mode=Mode{width:w,height:h,bpp};
            let result=unsafe{
                NativeWindowedBlt::from_borrowed_com_native_movie_rgb32(
                    primary,back,(0,0),(1920,1200),mode)
            };
            assert_eq!(result.is_ok(),accepted);
        }
        assert!(unsafe{NativeWindowedBlt::from_borrowed_com_native_movie_rgb32(
            ptr::null_mut(),back,(0,0),(1920,1200),
            Mode{width:800,height:600,bpp:16})}.is_err());
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
    fn native_dpi_menu_maintains_black_bars_without_clearing_content(){
        let (vt,mut p,mut source)=objects(0);
        p.vtable=vt.as_ptr();source.vtable=vt.as_ptr();
        let mode=Mode{width:800,height:600,bpp:16};
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            (&mut p as *mut FakeSurface).cast(),
            (&mut source as *mut FakeSurface).cast(),
            (0,0),(1920,1200),mode
        ).unwrap()};
        assert_eq!(backend.set_output_client((1536,960)),Ok(()));
        let view=Viewport{x:160,y:0,width:1600,height:1200};
        assert_eq!(backend.stretch_rgb565_into_viewport(mode,view),Ok(()));
        assert_eq!(backend.stretch_rgb565_into_viewport(mode,view),Ok(()));
        let calls=p.calls.lock().unwrap();
        assert_eq!(calls.len(),6);
        for i in [0usize,3]{
            assert_eq!(calls[i].rect,Rect{left:0,top:0,right:128,bottom:960});
            assert_eq!(calls[i+1].rect,Rect{left:1408,top:0,right:1536,bottom:960});
            assert_eq!(calls[i+2].rect,Rect{left:128,top:0,right:1408,bottom:960});
            assert_eq!(calls[i].flags,FILL|WAIT);
            assert_eq!(calls[i+1].flags,FILL|WAIT);
            assert_eq!(calls[i+2].flags,WAIT);
            assert_eq!(calls[i].source,false);
            assert_eq!(calls[i+1].source,false);
            assert_eq!(calls[i+2].source,true);
        }
    }
    #[test]
    fn native_gdi_sidebars_are_explicitly_opt_in(){
        let (_vt,mut p,mut source)=objects(0);
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            (&mut p as *mut FakeSurface).cast(),
            (&mut source as *mut FakeSurface).cast(),
            (0,0),(1920,1200),Mode{width:800,height:600,bpp:16}
        ).unwrap()};
        assert_eq!(backend.gdi_sidebar_hwnd,0);
        backend.enable_native_gdi_sidebars(0x5678);
        assert_eq!(backend.gdi_sidebar_hwnd,0x5678);
        backend.enable_native_gdi_sidebars(0);
        assert_eq!(backend.gdi_sidebar_hwnd,0);
    }
    #[test]
    fn physical_render_rects_are_mapped_to_dpi_logical_client(){
        let (_vt,mut primary,mut source)=objects(0);
        let real_primary=(&mut primary as *mut FakeSurface).cast();
        let real_source=(&mut source as *mut FakeSurface).cast();
        let mode=Mode{width:800,height:600,bpp:16};
        let mut backend=unsafe{NativeWindowedBlt::from_borrowed_com(
            real_primary,real_source,(0,0),(1920,1200),mode
        ).unwrap()};
        assert_eq!(backend.set_output_client((1536,960)),Ok(()));
        let menu=backend.screen_rect(Viewport{x:160,y:0,width:1600,height:1200}).unwrap();
        assert_eq!(menu,Rect{left:128,top:0,right:1408,bottom:960});
        let whole=backend.screen_rect(Viewport{x:0,y:0,width:1920,height:1200}).unwrap();
        assert_eq!(whole,Rect{left:0,top:0,right:1536,bottom:960});
        assert_eq!(backend.set_output_client((1536,950)),Err(BAD));
        // Windows may round one logical client dimension at fractional DPI.
        assert_eq!(backend.set_output_client((1536,961)),Ok(()));
        // But a real aspect mismatch remains invalid.
        assert_eq!(backend.set_output_client((1536,864)),Err(BAD));
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
