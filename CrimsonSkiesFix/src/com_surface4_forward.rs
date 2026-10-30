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
    sync::{Mutex,atomic::{AtomicBool,AtomicPtr,AtomicU32,AtomicUsize,Ordering}},
};
use crate::{
    Guid,
    com_surface4_abi::{Rect,Surface4VTable,SurfaceThis},
    borderless_contract::{AdapterContract,SurfaceRole},
    borderless_flip::{FlipBridgeError,FlipToBltBridge},
    borderless_native_blt::NativeWindowedBlt,
    presentation::Mode,
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
fn virtual_issue(role:SurfaceRole,method:&str,hr:i32){
    if VIRTUAL_DIAG_COUNT.fetch_add(1,Ordering::Relaxed)<40 {
        crate::log(&format!("WINDOWED-GAME: virtual Surface4 {method} role={role:?} result={:#010x}",hr as u32));
    }
}
const DDBLT_COLORFILL:u32=0x0000_0400;
const DDBLT_DEPTHFILL:u32=0x0200_0000;
const DDBLT_WAIT:u32=0x0100_0000;
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
struct VirtualGraphNode {role:SurfaceRole, mode:Mode}
impl VirtualGraphNode {
    fn caps(self)->u32{match self.role{
        SurfaceRole::Primary=>0x1000_e238,
        SurfaceRole::Backbuffer=>0x1000_601c,
        SurfaceRole::Depth=>0x1002_4000,
    }}
    fn bpp(self)->u32{if self.role==SurfaceRole::Depth{32}else{16}}
    fn count(self)->u32{if self.role==SurfaceRole::Primary{1}else{0}}
}
/// Probe-only eligible native presentation session. The wrapper's
/// primary->backbuffer->depth COM strong edges own the physical resources.
/// The full three-object chain and native HWND/clipper MUST be verified
/// before this is armed. There is deliberately no active game runtime caller.
struct GraphFlipSession{
    bridge:FlipToBltBridge,
    backend:NativeWindowedBlt,
    epoch:u64,
    mode:Mode,
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
    if !matches!((mode.width,mode.height,mode.bpp),(640,480,16)|(800,600,16)){
        return Err(E_INVALIDARG);
    }
    let wrapped=new_forwarding_surface4(real)?;
    (*wrapped.cast::<Surface4Forwarder>()).graph=Some(VirtualGraphNode{role,mode});
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
    origin:(i32,i32),client:(u32,u32)
)->Result<(),i32>{
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
    let backend=NativeWindowedBlt::from_borrowed_com(
        p.real,b.real,origin,client,node.mode
    ).map_err(|_|E_INVALIDARG)?;
    let mut state=p.virtual_flip.lock().map_err(|_|E_FAIL)?;
    if state.is_some()||p.retired.load(Ordering::Acquire){return Err(E_FAIL);}
    *state=Some(GraphFlipSession{bridge,backend,epoch:generation,mode:node.mode});
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

/// Strictly allow the only verified graph-specific native Blts:
/// RGB565 Backbuffer ColorFill and Z32 DepthFill with NULL source.
/// Source-copying Blts may carry wrapped or foreign COM identities, which
/// cannot be passed directly to Wine's internal ddraw_surface implementation.
/// The windowed *primary* is never forwarded for native Blt.
unsafe extern "system" fn forward_blt(this:SurfaceThis,p1:*mut Rect,p2:SurfaceThis,p3:*mut Rect,p4:u32,p5:*mut c_void)->i32{
    if let Some(node)=graph_node(this){
        if node.role==SurfaceRole::Primary{
            virtual_issue(node.role,"Blt primary (unsupported)",E_NOTIMPL);
            return E_NOTIMPL;
        }
        let current=&*this.cast::<Surface4Forwarder>();
        if current.retired.load(Ordering::Acquire){return E_NOTIMPL;}
        if !p2.is_null() || !p3.is_null(){
            virtual_issue(node.role,"Blt source-copy (unsupported)",E_NOTIMPL);
            return E_NOTIMPL;
        }
        let requested_fill=p4&!DDBLT_WAIT;
        let expected_fill=if node.role==SurfaceRole::Depth{DDBLT_DEPTHFILL}else{DDBLT_COLORFILL};
        if requested_fill!=expected_fill{
            virtual_issue(node.role,"Blt unsupported fill flags",E_NOTIMPL);
            return E_NOTIMPL;
        }
        if p5.is_null(){return E_INVALIDARG;}
        // IDirectDrawSurface4::Blt expects a Win32-x86 100-byte DDBLTFX.
        if *(p5.cast::<u32>())!=100{return E_INVALIDARG;}
        if !p1.is_null(){
            let rect=&*p1;
            if rect.left<0 || rect.top<0 || rect.right<=rect.left ||
               rect.bottom<=rect.top || rect.right as u32>node.mode.width ||
               rect.bottom as u32>node.mode.height {
                return E_INVALIDARG;
            }
        }
        // The real RGB565/Z32 surfaces have exactly the same internal mode
        // as the graph. This deliberately does not implement texture copies.
        let real=current.real;
        return (real_vtable(real).blt)(real,p1,ptr::null_mut(),ptr::null_mut(),p4,p5);
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
    if graph_node(this).is_some(){return E_NOTIMPL;}
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
        let Ok(mut state)=p.virtual_flip.lock() else{return E_FAIL;};
        let Some(session)=state.as_mut() else{return E_NOTIMPL;};
        let GraphFlipSession{bridge,backend,epoch,mode}=session;
        return match bridge.present(*epoch,*mode,backend){
            Ok(_)=>S_OK,
            Err(FlipBridgeError::ClearBlackFailed(hr)|
                FlipBridgeError::StretchFailed(hr))=>hr,
            Err(_)=>E_FAIL, // includes latched failure; never exclusive
        };
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
    (real_vtable(real).get_blt_status)(real,p1)
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_dc)(real,p1)
}

unsafe extern "system" fn forward_get_flip_status(this:SurfaceThis,p1:u32)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_flip_status)(real,p1)
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).release_dc)(real,p1)
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
            let mut bad=Rect{left:0,top:0,right:641,bottom:480};
            assert_eq!((bv.blt)(b,&mut bad,ptr::null_mut(),
                ptr::null_mut(),DDBLT_COLORFILL,effects),E_INVALIDARG);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),0);
            assert_eq!(phys[1].blts.load(Ordering::SeqCst),1);
            assert_eq!(phys[2].blts.load(Ordering::SeqCst),1);
            arm_graph_primary_flip(p,&contract,55,(0,0),(1920,1080)).unwrap();
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
            assert_eq!(graph_primary_completed_flips(p),None);
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(960,540)),Err(E_INVALIDARG));
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(1920,1080)),Ok(()));
            assert_eq!(arm_graph_primary_flip(p,&contract,17,(0,0),(1920,1080)),Err(E_FAIL));
            assert_eq!((vt.flip)(p,z,0),E_INVALIDARG);
            assert_eq!((vt.flip)(p,b,1),E_INVALIDARG);
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),0);
            assert_eq!((vt.flip)(p,b,0),S_OK);
            assert_eq!((vt.flip)(p,ptr::null_mut(),0),S_OK);
            assert_eq!(graph_primary_completed_flips(p),Some(2));
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),4);
            assert_eq!(phys[0].flips.load(Ordering::SeqCst),0);
            assert_eq!(phys[1].flips.load(Ordering::SeqCst),0);
            assert_eq!(retire_graph_primary_flip(p),Ok(2));
            assert_eq!(graph_primary_completed_flips(p),None);
            assert_eq!((vt.flip)(p,b,0),E_NOTIMPL);
            assert_eq!((vt.flip)(p,ptr::null_mut(),0),E_NOTIMPL);
            assert_eq!(retire_graph_primary_flip(p),Err(E_FAIL));
            assert_eq!(arm_graph_primary_flip(p,&contract,18,(0,0),(1920,1080)),Err(E_FAIL));
            assert_eq!(phys[0].blts.load(Ordering::SeqCst),4); // no stale writes
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
