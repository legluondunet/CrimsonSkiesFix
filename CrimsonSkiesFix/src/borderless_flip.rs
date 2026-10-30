//! Stage B4a: deterministic, unexposed Flip -> windowed-Blt transaction.
//!
//! There are NO Win32 calls, DirectDraw COM pointers, vtable hooks, or live
//! gameplay invocations in this module. It models the presentation operation
//! the real virtual-primary Flip implementation must eventually perform.
//! A passing pure unit test or the independent probe's PASS is not sufficient
//! evidence to activate this code for the game's original COM objects.

#![allow(dead_code)]

use crate::{
    borderless_contract::{AdapterContract,Phase},
    presentation::{Mode,Viewport},
};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum FlipBridgeError {
    BackendNotExposed,
    InvalidViewport,
    WrongGeneration,
    WrongInternalMode,
    FrameCounterOverflow,
    ClearBlackFailed(i32),
    StretchFailed(i32),
    AlreadyFaulted,
}

/// Backend operation order is deliberately strict. A future real COM
/// implementation must use an HWND clipper, a real DirectDraw4 primary,
/// and a genuine RGB565 IDirectDrawSurface4 (with D3D3 HAL/Z32 attached)
/// as the source. Backend is responsible for translating client-relative
/// rectangles to primary-surface screen coordinates. Neither action
/// changes SetDisplayMode, COM refcounts, or the game's original pixels.
pub trait WindowedBlt {
    fn clear_full_window_black(&mut self,desktop_width:u32,desktop_height:u32)
        ->Result<(),i32>;

    fn stretch_rgb565_into_viewport(&mut self,source:Mode,viewport:Viewport)
        ->Result<(),i32>;
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FlipStats {
    pub complete_frames:u64,
    pub fault:Option<FlipBridgeError>,
}

/// A game-visible successful Flip may be acknowledged only after BOTH Blts
/// have completed successfully. An error latches a failure: a future active
/// adapter must not silently forward a later Flip to the exclusive backend
/// after exposing a windowed primary to the game.
pub struct FlipToBltBridge {
    generation:u64,
    internal:Mode,
    desktop:(u32,u32),
    viewport:Viewport,
    complete_frames:u64,
    fault:Option<FlipBridgeError>,
}

impl FlipToBltBridge {
    /// Construct ONLY after the active adapter has verified every real
    /// backend object and exposed the *complete* virtual COM surface chain.
    /// AdapterContract itself cannot create or verify these resources.
    pub fn from_exposed(contract:&AdapterContract,generation:u64)
        ->Result<Self,FlipBridgeError>
    {
        if contract.phase!=Phase::Exposed {
            return Err(FlipBridgeError::BackendNotExposed);
        }
        let p=contract.plan;
        let vw=p.viewport.x.checked_add(p.viewport.width);
        let vh=p.viewport.y.checked_add(p.viewport.height);
        if p.viewport.width==0 || p.viewport.height==0 ||
           p.internal.bpp!=16 ||
           vw.is_none_or(|v|v>p.desktop_width) ||
           vh.is_none_or(|v|v>p.desktop_height) {
            return Err(FlipBridgeError::InvalidViewport);
        }
        Ok(Self {
            generation,internal:p.internal,
            desktop:(p.desktop_width,p.desktop_height),
            viewport:p.viewport,complete_frames:0,fault:None
        })
    }

    pub fn stats(&self)->FlipStats{
        FlipStats {complete_frames:self.complete_frames,fault:self.fault}
    }

    pub fn viewport(&self)->Viewport{self.viewport}

    /// No simulated success; a bad generation, stale mode, invalid backend
    /// call or count overflow produces an error and latches a fault. The
    /// caller cannot resume the old exclusive Flip path on the same chain.
    pub fn present<B:WindowedBlt>(&mut self,generation:u64,mode:Mode,backend:&mut B)
        ->Result<u64,FlipBridgeError>
    {
        if self.fault.is_some(){return Err(FlipBridgeError::AlreadyFaulted);}
        let failure=if generation!=self.generation {
            Some(FlipBridgeError::WrongGeneration)
        }else if mode!=self.internal {
            Some(FlipBridgeError::WrongInternalMode)
        }else{None};
        if let Some(failure)=failure{
            self.fault=Some(failure);
            return Err(failure);
        }
        let next=match self.complete_frames.checked_add(1){
            Some(value)=>value,
            None=>{
                self.fault=Some(FlipBridgeError::FrameCounterOverflow);
                return Err(FlipBridgeError::FrameCounterOverflow);
            },
        };
        if let Err(hr)=backend.clear_full_window_black(self.desktop.0,self.desktop.1){
            let failure=FlipBridgeError::ClearBlackFailed(hr);
            self.fault=Some(failure);
            return Err(failure);
        }
        if let Err(hr)=backend.stretch_rgb565_into_viewport(self.internal,self.viewport){
            let failure=FlipBridgeError::StretchFailed(hr);
            self.fault=Some(failure);
            return Err(failure);
        }
        self.complete_frames=next;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        borderless_contract::{BackendEvidence,RenderTarget},
        presentation::Chain,
    };
    #[derive(Debug,PartialEq,Eq)]
    enum Event {Black(u32,u32),Stretch(Mode,Viewport)}
    #[derive(Default)]
    struct FakeBlt {
        events:Vec<Event>,
        fail_black:Option<i32>,
        fail_stretch:Option<i32>,
    }
    impl WindowedBlt for FakeBlt {
        fn clear_full_window_black(&mut self,w:u32,h:u32)->Result<(),i32>{
            self.events.push(Event::Black(w,h));
            self.fail_black.map_or(Ok(()),Err)
        }
        fn stretch_rgb565_into_viewport(&mut self,m:Mode,v:Viewport)
            ->Result<(),i32>
        {
            self.events.push(Event::Stretch(m,v));
            self.fail_stretch.map_or(Ok(()),Err)
        }
    }
    fn prepared(mode:Mode)->AdapterContract{
        AdapterContract::prepare(
            mode,(1920,1080),
            Chain {front_to_back:true,back_to_depth:true,mipmaps:false}
        ).unwrap()
    }
    fn ready(mode:Mode)->AdapterContract {
        let mut contract=prepared(mode);
        contract.expose(BackendEvidence{
            process_owned_window:true,normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:0x0000_2000,
            render_target_is_rgb565:true,
            real_zbuffer_32:true,primary_with_clipper:true,
            working_blt:true,complete_surface_com:true
        }).unwrap();
        contract
    }
    fn mode640()->Mode{Mode{width:640,height:480,bpp:16}}
    fn mode800()->Mode{Mode{width:800,height:600,bpp:16}}

    #[test]
    fn inactive_contract_refuses_any_flip_bridge(){
        let contract=prepared(mode640());
        assert!(matches!(
            FlipToBltBridge::from_exposed(&contract,1),
            Err(FlipBridgeError::BackendNotExposed)));
    }
    #[test]
    fn flip_is_two_real_operations_before_success_is_reported(){
        let m=mode640();
        let contract=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&contract,3).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.viewport(),Viewport{x:240,y:0,width:1440,height:1080});
        for frame in 1..=120 {
            assert_eq!(bridge.present(3,m,&mut backend),Ok(frame));
        }
        assert_eq!(bridge.stats(),FlipStats{complete_frames:120,fault:None});
        assert_eq!(backend.events.len(),240);
        assert_eq!(backend.events[0],Event::Black(1920,1080));
        assert_eq!(backend.events[1],Event::Stretch(m,bridge.viewport()));
        assert_eq!(backend.events[238],Event::Black(1920,1080));
        assert_eq!(backend.events[239],Event::Stretch(m,bridge.viewport()));
    }
    #[test]
    fn eight_hundred_by_six_hundred_remains_fully_pillarboxed(){
        let m=mode800();
        let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,42).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(42,m,&mut backend),Ok(1));
        assert_eq!(bridge.viewport(),Viewport{x:240,y:0,width:1440,height:1080});
        assert_eq!(backend.events,[Event::Black(1920,1080),
            Event::Stretch(m,Viewport{x:240,y:0,width:1440,height:1080})]);
    }
    #[test]
    fn failed_black_clear_never_stretches_or_acknowledges_flip(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,8).unwrap();
        let mut backend=FakeBlt{fail_black:Some(-17),..Default::default()};
        assert_eq!(bridge.present(8,m,&mut backend),Err(FlipBridgeError::ClearBlackFailed(-17)));
        assert_eq!(backend.events,[Event::Black(1920,1080)]);
        assert_eq!(bridge.stats().complete_frames,0);
        assert_eq!(bridge.present(8,m,&mut backend),Err(FlipBridgeError::AlreadyFaulted));
        assert_eq!(backend.events.len(),1);
    }
    #[test]
    fn failed_stretch_never_acknowledges_flip_or_retries_exclusive(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,8).unwrap();
        let mut backend=FakeBlt{fail_stretch:Some(-42),..Default::default()};
        assert_eq!(bridge.present(8,m,&mut backend),Err(FlipBridgeError::StretchFailed(-42)));
        assert_eq!(backend.events.len(),2);
        assert_eq!(bridge.stats().complete_frames,0);
        assert_eq!(bridge.present(8,m,&mut backend),Err(FlipBridgeError::AlreadyFaulted));
        assert_eq!(backend.events.len(),2);
    }
    #[test]
    fn stale_epoch_is_rejected_before_any_blt(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,10).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(9,m,&mut backend),Err(FlipBridgeError::WrongGeneration));
        assert!(backend.events.is_empty());
        assert_eq!(bridge.stats().fault,Some(FlipBridgeError::WrongGeneration));
    }
    #[test]
    fn changing_internal_resolution_requires_a_new_chain(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,10).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(10,mode800(),&mut backend),Err(FlipBridgeError::WrongInternalMode));
        assert!(backend.events.is_empty());
        assert_eq!(bridge.stats().fault,Some(FlipBridgeError::WrongInternalMode));
    }
    #[test]
    fn counter_overflow_does_not_touch_backend(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,10).unwrap();
        bridge.complete_frames=u64::MAX;
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(10,m,&mut backend),Err(FlipBridgeError::FrameCounterOverflow));
        assert!(backend.events.is_empty());
        assert_eq!(bridge.stats().complete_frames,u64::MAX);
    }
}
