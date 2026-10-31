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
    presentation::{fit_viewport,Mode,Viewport},
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

/// Pure geometry gate shared by game-side contract and standalone probe.
fn geometry_valid(mode:Mode,desktop:(u32,u32),viewport:Viewport)->bool{
    mode.bpp==16 &&
    matches!((mode.width,mode.height),(640,480)|(800,600)) &&
    fit_viewport(mode.width,mode.height,desktop.0,desktop.1)==Some(viewport)
}

/// Perform the two windowed Blts on the supplied *real* backend.
/// This has no COM virtual-object exposure, refcount or game-visible Flip
/// acknowledgment. The game-side FlipToBltBridge wraps it with an
/// exposed-adapter requirement, mode/epoch checks and a failure latch.
pub fn execute_windowed_blt_pair<B:WindowedBlt>(
    backend:&mut B,mode:Mode,desktop:(u32,u32),viewport:Viewport
)->Result<(),FlipBridgeError>{
    if !geometry_valid(mode,desktop,viewport){
        return Err(FlipBridgeError::InvalidViewport);
    }
    backend.clear_full_window_black(desktop.0,desktop.1)
        .map_err(FlipBridgeError::ClearBlackFailed)?;
    backend.stretch_rgb565_into_viewport(mode,viewport)
        .map_err(FlipBridgeError::StretchFailed)?;
    Ok(())
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
        if !geometry_valid(
            p.internal,(p.desktop_width,p.desktop_height),p.viewport
        ) {
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
        self.present_with_refresh(generation,mode,backend,false)
    }

    /// Repaint the full game window only on explicit visibility restoration.
    /// Normal frames still stretch just the 4:3 viewport, avoiding the black
    /// flicker that occurred when the whole desktop was cleared each Flip.
    pub fn present_with_refresh<B:WindowedBlt>(&mut self,generation:u64,mode:Mode,
        backend:&mut B,visibility_refresh:bool)->Result<u64,FlipBridgeError>
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
        // Repainting the whole primary black before EVERY Flip makes
        // compositor-visible black frames possible between the clear and
        // the second Blt, perceived as intense flicker on Wine. Window
        // bounds and the pillarbox viewport are fixed for this epoch.
        // Clear once on the first frame, then update only the viewport.
        // Each new mode has its own freshly initialized bridge.
        let outcome=if self.complete_frames==0 || visibility_refresh{
            execute_windowed_blt_pair(
                backend,self.internal,self.desktop,self.viewport
            )
        }else{
            backend.stretch_rgb565_into_viewport(self.internal,self.viewport)
                .map_err(FlipBridgeError::StretchFailed)
        };
        if let Err(failure)=outcome{
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
    fn pure_backend_pair_is_testable_without_claiming_virtual_com_exposure(){
        let m=mode640();let vp=Viewport{x:240,y:0,width:1440,height:1080};
        let mut backend=FakeBlt::default();
        assert_eq!(execute_windowed_blt_pair(&mut backend,m,(1920,1080),vp),Ok(()));
        assert_eq!(backend.events,[Event::Black(1920,1080),Event::Stretch(m,vp)]);
        assert_eq!(execute_windowed_blt_pair(
            &mut backend,m,(1920,1080),Viewport{x:241,y:0,width:1440,height:1080}
        ),Err(FlipBridgeError::InvalidViewport));
        assert_eq!(backend.events.len(),2);
    }
    #[test]
    fn pure_backend_pair_propagates_partial_blt_failures(){
        let m=mode640();let vp=Viewport{x:240,y:0,width:1440,height:1080};
        let mut backend=FakeBlt{fail_black:Some(-17),..Default::default()};
        assert_eq!(execute_windowed_blt_pair(&mut backend,m,(1920,1080),vp),
                   Err(FlipBridgeError::ClearBlackFailed(-17)));
        assert_eq!(backend.events,[Event::Black(1920,1080)]);
        let mut backend=FakeBlt{fail_stretch:Some(-42),..Default::default()};
        assert_eq!(execute_windowed_blt_pair(&mut backend,m,(1920,1080),vp),
                   Err(FlipBridgeError::StretchFailed(-42)));
        assert_eq!(backend.events,[Event::Black(1920,1080),Event::Stretch(m,vp)]);
    }
    #[test]
    fn inactive_contract_refuses_any_flip_bridge(){
        let contract=prepared(mode640());
        assert!(matches!(
            FlipToBltBridge::from_exposed(&contract,1),
            Err(FlipBridgeError::BackendNotExposed)));
    }
    #[test]
    fn first_flip_clears_pillarbox_once_then_only_stretches_to_avoid_flicker(){
        let m=mode640();
        let contract=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&contract,3).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.viewport(),Viewport{x:240,y:0,width:1440,height:1080});
        for frame in 1..=120 {
            assert_eq!(bridge.present(3,m,&mut backend),Ok(frame));
        }
        assert_eq!(bridge.stats(),FlipStats{complete_frames:120,fault:None});
        assert_eq!(backend.events.len(),121); // 1 clear and 120 stretches
        assert_eq!(backend.events[0],Event::Black(1920,1080));
        assert_eq!(backend.events[1],Event::Stretch(m,bridge.viewport()));
        assert_eq!(backend.events[2],Event::Stretch(m,bridge.viewport()));
        assert_eq!(backend.events[120],Event::Stretch(m,bridge.viewport()));
        assert_eq!(backend.events.iter().filter(|x|matches!(x,Event::Black(_, _))).count(),1);
    }
    #[test]
    fn explicit_visibility_return_repaints_once_without_every_frame_clear(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,55).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(55,m,&mut backend),Ok(1));
        assert_eq!(bridge.present(55,m,&mut backend),Ok(2));
        assert_eq!(bridge.present_with_refresh(55,m,&mut backend,true),Ok(3));
        assert_eq!(bridge.present(55,m,&mut backend),Ok(4));
        assert_eq!(backend.events.len(),6);
        assert_eq!(backend.events.iter().filter(|x|matches!(x,Event::Black(_, _))).count(),2);
        assert_eq!(backend.events[3],Event::Black(1920,1080));
        assert_eq!(backend.events[4],Event::Stretch(m,bridge.viewport()));
        assert_eq!(bridge.stats(),FlipStats{complete_frames:4,fault:None});
    }
    #[test]
    fn refresh_clear_failure_does_not_acknowledge_frame(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,56).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(56,m,&mut backend),Ok(1));
        backend.fail_black=Some(-61);
        assert_eq!(bridge.present_with_refresh(56,m,&mut backend,true),
                   Err(FlipBridgeError::ClearBlackFailed(-61)));
        assert_eq!(bridge.stats().complete_frames,1);
        assert_eq!(bridge.present(56,m,&mut backend),Err(FlipBridgeError::AlreadyFaulted));
    }
    #[test]
    fn later_stretch_error_is_latched_without_black_clear_or_frame_ack(){
        let m=mode640();let c=ready(m);
        let mut bridge=FlipToBltBridge::from_exposed(&c,81).unwrap();
        let mut backend=FakeBlt::default();
        assert_eq!(bridge.present(81,m,&mut backend),Ok(1));
        backend.fail_stretch=Some(-51);
        assert_eq!(bridge.present(81,m,&mut backend),
            Err(FlipBridgeError::StretchFailed(-51)));
        assert_eq!(bridge.stats().complete_frames,1);
        assert_eq!(backend.events.len(),3);
        assert_eq!(backend.events[2],Event::Stretch(m,bridge.viewport()));
        assert_eq!(bridge.present(81,m,&mut backend),Err(FlipBridgeError::AlreadyFaulted));
        assert_eq!(backend.events.len(),3);
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
