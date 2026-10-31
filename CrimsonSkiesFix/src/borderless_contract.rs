//! Pure contract for a future, opt-in DirectDraw3D windowed adapter.
//!
//! This is NOT an active COM implementation. It never allocates COM objects,
//! touches a vtable, reads GPU pixels, changes a window, or intercepts Flip.
//! Identity keys model the required IUnknown equivalence; the actual COM
//! façade must later provide one physical IUnknown pointer per key.
//! The runtime must independently verify every backend resource passed as
//! evidence here. Passing booleans cannot initialize a graphics backend.

#![allow(dead_code)]

use crate::presentation::{preflight,preflight_native_3d,Chain,Mode,WindowedPresentationPlan};

// Direct3D3::CreateDevice requires DDSCAPS_3DDEVICE on its real
// IDirectDrawSurface4 render target (original DirectX 6 contract).
const DDSCAPS_3DDEVICE:u32=0x0000_2000;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Phase {Preparing,Exposed,Aborted}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Requirement {
    ProcessOwnedWindow,
    NormalDirectDrawCooperativeMode,
    RealRgb565Direct3dSurface4,
    Direct3dDeviceSurfaceCapability,
    Rgb565PixelFormat,
    Real32BitZBuffer,
    WindowedPrimaryWithClipper,
    WorkingWindowedBlt,
    CompleteSurfaceComImplementation,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RenderTarget {
    RealDirectDrawSurface4,
    CpuPixelsOnly,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct BackendEvidence {
    pub process_owned_window:bool,
    pub normal_directdraw:bool,
    pub render_target:RenderTarget,
    pub render_target_caps:u32,
    pub render_target_is_rgb565:bool,
    pub real_zbuffer_32:bool,
    pub primary_with_clipper:bool,
    pub working_blt:bool,
    pub complete_surface_com:bool,
}

impl BackendEvidence {
    fn first_missing(self)->Option<Requirement>{
        if !self.process_owned_window {return Some(Requirement::ProcessOwnedWindow);}
        if !self.normal_directdraw {return Some(Requirement::NormalDirectDrawCooperativeMode);}
        if self.render_target!=RenderTarget::RealDirectDrawSurface4 {
            return Some(Requirement::RealRgb565Direct3dSurface4);
        }
        if self.render_target_caps&DDSCAPS_3DDEVICE==0 {
            return Some(Requirement::Direct3dDeviceSurfaceCapability);
        }
        if !self.render_target_is_rgb565 {
            return Some(Requirement::Rgb565PixelFormat);
        }
        if !self.real_zbuffer_32 {return Some(Requirement::Real32BitZBuffer);}
        if !self.primary_with_clipper {return Some(Requirement::WindowedPrimaryWithClipper);}
        if !self.working_blt {return Some(Requirement::WorkingWindowedBlt);}
        if !self.complete_surface_com {return Some(Requirement::CompleteSurfaceComImplementation);}
        None
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ContractError {
    UnsupportedModeOrChain,
    Missing(Requirement),
    InvalidPhase,
    FallbackAfterExposure,
    WrongObject,
    UnsupportedInterface,
    NoAttachment,
    DeadObject,
    ReferenceOverflow,
    ReferenceUnderflow,
}

#[derive(Debug,Clone,Copy)]
pub struct AdapterContract {
    pub plan:WindowedPresentationPlan,
    pub phase:Phase,
}

impl AdapterContract {
    // The observed chain is a *preflight*, not authority to switch modes.
    pub fn prepare(mode:Mode,desktop:(u32,u32),chain:Chain)
        ->Result<Self,ContractError>
    {
        let plan=preflight(mode,desktop.0,desktop.1,chain)
            .ok_or(ContractError::UnsupportedModeOrChain)?;
        if !plan.requires_surface_virtualization {
            return Err(ContractError::UnsupportedModeOrChain);
        }
        Ok(Self {plan,phase:Phase::Preparing})
    }

    /// Only an EXE with a verified widescreen patch can opt into the
    /// full-monitor render graph. The existing prepare() remains unchanged
    /// and continues to reject native full-screen modes by default.
    pub fn prepare_native_3d(mode:Mode,desktop:(u32,u32),chain:Chain)
        ->Result<Self,ContractError>{
        let plan=preflight_native_3d(mode,desktop.0,desktop.1,chain)
            .ok_or(ContractError::UnsupportedModeOrChain)?;
        if !plan.requires_surface_virtualization {
            return Err(ContractError::UnsupportedModeOrChain);
        }
        Ok(Self {plan,phase:Phase::Preparing})
    }

    // A later real COM backend must validate the objects *before* calling
    // this method. The model does not itself create or validate real handles.
    pub fn expose(&mut self,evidence:BackendEvidence)->Result<(),ContractError>{
        if self.phase!=Phase::Preparing {return Err(ContractError::InvalidPhase);}
        if let Some(missing)=evidence.first_missing(){
            self.phase=Phase::Aborted; // original exclusive path still untouched
            return Err(ContractError::Missing(missing));
        }
        self.phase=Phase::Exposed;
        Ok(())
    }

    pub fn abandon(&mut self)->Result<(),ContractError>{
        if self.phase==Phase::Exposed {
            return Err(ContractError::FallbackAfterExposure);
        }
        self.phase=Phase::Aborted;
        Ok(())
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum SurfaceRole {Primary,Backbuffer,Depth}

impl SurfaceRole {
    const fn index(self)->usize {
        match self {Self::Primary=>0,Self::Backbuffer=>1,Self::Depth=>2}
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Interface {
    IUnknown, Surface1, Surface2, Surface3, Surface4, Surface7,
    // Unrecognized interfaces cannot silently be accepted by this contract.
    Other,
}

impl Interface {
    fn supported(self)->bool {self!=Self::Other}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct LogicalIdentity {pub epoch:u64,pub role:SurfaceRole}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SurfaceHandle {
    pub identity:LogicalIdentity,
    pub interface:Interface,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Attachment {Backbuffer,ZBuffer}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SurfaceDescriptor {
    pub width:u32,
    pub height:u32,
    pub bits:u32,
    pub caps:u32,
    pub backbuffers:u32,
}

#[derive(Clone,Copy,Debug)]
struct Node {
    client_refs:u32,
    owning_refs:u32,
    alive:bool,
}
impl Node {
    fn count(self)->u32 {self.client_refs+self.owning_refs}
}

/// A non-FFI ownership model of a single complete virtual surface chain.
/// It is not registered with Windows and cannot be passed to Direct3D.
#[derive(Debug)]
pub struct SurfaceContract {
    epoch:u64,
    mode:Mode,
    nodes:[Node;3],
}

impl SurfaceContract {
    pub fn new(contract:&AdapterContract,epoch:u64)->Result<(Self,SurfaceHandle),ContractError>{
        if contract.phase!=Phase::Exposed {return Err(ContractError::InvalidPhase);}
        let model=Self {
            epoch,
            mode:contract.plan.internal,
            // Implicit primary->backbuffer, explicit backbuffer->Z depth
            // attachment each holds the child alive, independently of
            // client-owned GetAttachedSurface/QueryInterface pointers.
            nodes:[
                Node{client_refs:1,owning_refs:0,alive:true},
                Node{client_refs:0,owning_refs:1,alive:true},
                Node{client_refs:0,owning_refs:1,alive:true},
            ],
        };
        let primary=model.handle(SurfaceRole::Primary,Interface::Surface4);
        Ok((model,primary))
    }

    fn handle(&self,role:SurfaceRole,interface:Interface)->SurfaceHandle{
        SurfaceHandle {
            identity:LogicalIdentity {epoch:self.epoch,role},
            interface,
        }
    }

    fn node(&self,handle:SurfaceHandle)->Result<usize,ContractError>{
        if handle.identity.epoch!=self.epoch {return Err(ContractError::WrongObject);}
        if !handle.interface.supported(){return Err(ContractError::UnsupportedInterface);}
        let idx=handle.identity.role.index();
        if !self.nodes[idx].alive {return Err(ContractError::DeadObject);}
        Ok(idx)
    }

    fn new_client_ref(&mut self,idx:usize)->Result<(),ContractError>{
        let node=&mut self.nodes[idx];
        if !node.alive {return Err(ContractError::DeadObject);}
        // COM's reference count is a u32. Check the *combined* count,
        // including attachment-owned refs, before incrementing.
        node.count().checked_add(1).ok_or(ContractError::ReferenceOverflow)?;
        node.client_refs=node.client_refs.checked_add(1)
            .ok_or(ContractError::ReferenceOverflow)?;
        Ok(())
    }

    // The physical IUnknown pointer and supported IID set must remain
    // identical regardless of which version first called QueryInterface.
    pub fn query_interface(&mut self,from:SurfaceHandle,to:Interface)
        ->Result<SurfaceHandle,ContractError>
    {
        let idx=self.node(from)?;
        if !to.supported(){return Err(ContractError::UnsupportedInterface);}
        self.new_client_ref(idx)?;
        Ok(self.handle(from.identity.role,to))
    }

    pub fn add_ref(&mut self,handle:SurfaceHandle)->Result<u32,ContractError>{
        let idx=self.node(handle)?;
        self.new_client_ref(idx)?;
        Ok(self.nodes[idx].count())
    }

    pub fn release(&mut self,handle:SurfaceHandle)->Result<u32,ContractError>{
        let idx=self.node(handle)?;
        let n=&mut self.nodes[idx];
        if n.client_refs==0{return Err(ContractError::ReferenceUnderflow);}
        n.client_refs-=1;
        let remaining=n.count();
        self.collect(idx);
        Ok(remaining)
    }

    fn collect(&mut self,idx:usize){
        if !self.nodes[idx].alive || self.nodes[idx].count()!=0{return;}
        self.nodes[idx].alive=false;
        // The parent-owned edge disappears only when its parent dies.
        let child=match idx {0=>Some(1),1=>Some(2),_=>None};
        if let Some(child)=child {
            debug_assert!(self.nodes[child].owning_refs>0);
            self.nodes[child].owning_refs-=1;
            self.collect(child);
        }
    }

    pub fn get_attached(&mut self,from:SurfaceHandle,request:Attachment)
        ->Result<SurfaceHandle,ContractError>
    {
        let idx=self.node(from)?;
        if from.interface==Interface::IUnknown {
            return Err(ContractError::UnsupportedInterface);
        }
        let child=match (idx,request) {
            (0,Attachment::Backbuffer)=>SurfaceRole::Backbuffer,
            (1,Attachment::ZBuffer)=>SurfaceRole::Depth,
            _=>return Err(ContractError::NoAttachment),
        };
        self.new_client_ref(child.index())?;
        // GetAttachedSurface returns an interface of the caller's
        // DirectDrawSurface version and adds a reference for the caller.
        Ok(self.handle(child,from.interface))
    }

    pub fn descriptor(&self,handle:SurfaceHandle)
        ->Result<SurfaceDescriptor,ContractError>
    {
        self.node(handle)?;
        let (caps,bits,backbuffers)=match handle.identity.role {
            SurfaceRole::Primary=>(0x1000_e238,16,1),
            SurfaceRole::Backbuffer=>(0x1000_601c,16,0),
            SurfaceRole::Depth=>(0x1002_4000,32,0),
        };
        Ok(SurfaceDescriptor {
            width:self.mode.width,height:self.mode.height,
            bits,caps,backbuffers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chain()->Chain {
        Chain {front_to_back:true,back_to_depth:true,mipmaps:false}
    }
    fn evidence()->BackendEvidence {
        BackendEvidence {
            process_owned_window:true,normal_directdraw:true,
            render_target:RenderTarget::RealDirectDrawSurface4,
            render_target_caps:0x1000_601c,
            render_target_is_rgb565:true,
            real_zbuffer_32:true,primary_with_clipper:true,
            working_blt:true,complete_surface_com:true,
        }
    }
    fn ready()->(SurfaceContract,SurfaceHandle){
        let mut contract=AdapterContract::prepare(
            Mode{width:640,height:480,bpp:16},(1920,1080),chain()).unwrap();
        assert_eq!(contract.plan.viewport.width,1440);
        assert_eq!(contract.plan.viewport.x,240);
        contract.expose(evidence()).unwrap();
        SurfaceContract::new(&contract,1).unwrap()
    }

    #[test]
    fn unsupported_3d_chain_or_mode_refuses_adapter(){
        let mode=Mode{width:640,height:480,bpp:16};
        let incomplete=Chain{front_to_back:true,back_to_depth:false,mipmaps:false};
        assert_eq!(AdapterContract::prepare(mode,(1920,1080),incomplete).unwrap_err(),
                   ContractError::UnsupportedModeOrChain);
        assert_eq!(AdapterContract::prepare(Mode{width:1920,height:1080,bpp:16},
                   (1920,1080),chain()).unwrap_err(),
                   ContractError::UnsupportedModeOrChain);
    }
    #[test]
    fn incomplete_backend_aborts_before_any_surface_is_exposed(){
        let mut c=AdapterContract::prepare(Mode{width:800,height:600,bpp:16},
                                          (1920,1080),chain()).unwrap();
        let mut e=evidence();
        e.primary_with_clipper=false;
        assert_eq!(c.expose(e),Err(ContractError::Missing(
            Requirement::WindowedPrimaryWithClipper)));
        assert_eq!(c.phase,Phase::Aborted);
        assert!(matches!(SurfaceContract::new(&c,5),Err(ContractError::InvalidPhase)));
        assert_eq!(c.expose(evidence()),Err(ContractError::InvalidPhase));
    }
    #[test]
    fn cpu_pixels_cannot_masquerade_as_real_direct3d_render_target(){
        let mut c=AdapterContract::prepare(Mode{width:640,height:480,bpp:16},
                                          (1920,1080),chain()).unwrap();
        let mut e=evidence();
        e.render_target=RenderTarget::CpuPixelsOnly;
        assert_eq!(c.expose(e),Err(ContractError::Missing(
            Requirement::RealRgb565Direct3dSurface4)));
    }
    #[test]
    fn once_exposed_never_silently_fallback_to_exclusive(){
        let mut c=AdapterContract::prepare(Mode{width:640,height:480,bpp:16},
                                          (1920,1080),chain()).unwrap();
        c.expose(evidence()).unwrap();
        assert_eq!(c.abandon(),Err(ContractError::FallbackAfterExposure));
        assert_eq!(c.phase,Phase::Exposed);
        assert_eq!(c.expose(evidence()),Err(ContractError::InvalidPhase));
    }
    #[test]
    fn query_interface_versions_preserve_logical_iunknown_identity(){
        let (mut chain,primary)=ready();
        let v1=chain.query_interface(primary,Interface::Surface1).unwrap();
        let v7=chain.query_interface(v1,Interface::Surface7).unwrap();
        let v4=chain.query_interface(v7,Interface::Surface4).unwrap();
        let u1=chain.query_interface(v1,Interface::IUnknown).unwrap();
        let u2=chain.query_interface(v7,Interface::IUnknown).unwrap();
        assert_eq!(u1.identity,u2.identity);
        assert_eq!(v1.identity,v4.identity);
        assert_eq!(chain.query_interface(v4,Interface::Other),
                   Err(ContractError::UnsupportedInterface));
        for h in [u1,u2,v4,v7,v1] {chain.release(h).unwrap();}
        assert_eq!(chain.release(primary),Ok(0));
        assert_eq!(chain.add_ref(primary),Err(ContractError::DeadObject));
    }
    #[test]
    fn attached_surface_references_outlive_parent_until_released(){
        let (mut chain,primary)=ready();
        let back=chain.get_attached(primary,Attachment::Backbuffer).unwrap();
        let depth=chain.get_attached(back,Attachment::ZBuffer).unwrap();
        assert_ne!(back.identity,primary.identity);
        assert_ne!(depth.identity,back.identity);
        assert_eq!(chain.release(primary),Ok(0));
        // The backbuffer remains alive due to its client reference.
        let back4=chain.query_interface(back,Interface::Surface4).unwrap();
        assert_eq!(chain.release(back4),Ok(1));
        // Backbuffer owns its depth attachment until the last release.
        assert_eq!(chain.release(back),Ok(0));
        assert_eq!(chain.descriptor(depth).unwrap().bits,32);
        assert_eq!(chain.release(depth),Ok(0));
        assert_eq!(chain.release(depth),Err(ContractError::DeadObject));
    }
    #[test]
    fn get_attached_adds_one_client_ref_and_rejects_wrong_edges(){
        let (mut chain,primary)=ready();
        assert_eq!(chain.get_attached(primary,Attachment::ZBuffer),
                   Err(ContractError::NoAttachment));
        let u=chain.query_interface(primary,Interface::IUnknown).unwrap();
        assert_eq!(chain.get_attached(u,Attachment::Backbuffer),
                   Err(ContractError::UnsupportedInterface));
        chain.release(u).unwrap();
        let b=chain.get_attached(primary,Attachment::Backbuffer).unwrap();
        assert_eq!(chain.descriptor(b).unwrap().caps,0x1000_601c);
        assert_eq!(chain.release(b),Ok(1)); // owning primary edge remains
        assert_eq!(chain.release(primary),Ok(0));
        assert_eq!(chain.descriptor(b),Err(ContractError::DeadObject));
    }
    #[test]
    fn descriptor_matches_observed_3d_rgb565_depth_chain(){
        let (mut chain,p)=ready();
        let b=chain.get_attached(p,Attachment::Backbuffer).unwrap();
        let z=chain.get_attached(b,Attachment::ZBuffer).unwrap();
        let pd=chain.descriptor(p).unwrap();
        let bd=chain.descriptor(b).unwrap();
        let zd=chain.descriptor(z).unwrap();
        assert_eq!((pd.width,pd.height,pd.bits,pd.backbuffers),
                   (640,480,16,1));
        assert_eq!((bd.width,bd.height,bd.bits,bd.caps),
                   (640,480,16,0x1000_601c));
        assert_eq!((zd.width,zd.height,zd.bits,zd.caps),
                   (640,480,32,0x1002_4000));
    }
    #[test]
    fn stale_epoch_pointer_cannot_reference_new_mode_objects(){
        let (mut chain,p)=ready();
        let stale=SurfaceHandle {
            identity:LogicalIdentity{epoch:3,role:SurfaceRole::Primary},
            interface:Interface::Surface4,
        };
        assert_eq!(chain.query_interface(stale,Interface::IUnknown),
                   Err(ContractError::WrongObject));
        assert_eq!(chain.release(stale),Err(ContractError::WrongObject));
        assert!(chain.descriptor(p).is_ok());
    }
    #[test]
    fn requires_true_direct3d_device_caps_and_rgb565_target_format(){
        let mode=Mode{width:640,height:480,bpp:16};
        let mut c=AdapterContract::prepare(mode,(1920,1080),chain()).unwrap();
        let mut e=evidence();
        e.render_target_caps&=!DDSCAPS_3DDEVICE;
        assert_eq!(c.expose(e),Err(ContractError::Missing(
            Requirement::Direct3dDeviceSurfaceCapability)));
        let mut c=AdapterContract::prepare(mode,(1920,1080),chain()).unwrap();
        let mut e=evidence();
        e.render_target_is_rgb565=false;
        assert_eq!(c.expose(e),Err(ContractError::Missing(
            Requirement::Rgb565PixelFormat)));
        assert_ne!(0x1000_601c&DDSCAPS_3DDEVICE,0);
    }
    #[test]
    fn overflowing_combined_client_and_attachment_refs_is_rejected(){
        let (mut c,p)=ready();
        c.nodes[SurfaceRole::Backbuffer.index()].client_refs=u32::MAX-1;
        // One attachment-owned ref already exists; no room for another.
        assert_eq!(c.get_attached(p,Attachment::Backbuffer),
                   Err(ContractError::ReferenceOverflow));
        assert_eq!(c.nodes[SurfaceRole::Backbuffer.index()].count(),u32::MAX);
    }
}
