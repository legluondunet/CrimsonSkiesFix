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
    sync::atomic::{AtomicU32,Ordering},
};
use crate::{
    Guid,
    com_surface4_abi::{Rect,Surface4VTable,SurfaceThis},
};
const S_OK:i32=0;
const E_NOINTERFACE:i32=0x8000_4002u32 as i32;
const E_POINTER:i32=0x8000_4003u32 as i32;
const E_FAIL:i32=0x8000_4005u32 as i32;
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
#[repr(C)]
struct Surface4Forwarder{
    vtable:*const Surface4VTable, // COM slot 0, first DWORD
    real:SurfaceThis,             // one underlying owned COM ref
    refs:AtomicU32,               // client-owned references only
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
    });
    Ok(Box::into_raw(obj).cast())
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).add_attached_surface)(real,p1)
}

unsafe extern "system" fn forward_add_overlay_dirty_rect(this:SurfaceThis,p1:*mut Rect)->i32{
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).add_overlay_dirty_rect)(real,p1)
}

unsafe extern "system" fn forward_blt(this:SurfaceThis,p1:*mut Rect,p2:SurfaceThis,p3:*mut Rect,p4:u32,p5:*mut c_void)->i32{
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).blt_fast)(real,p1,p2,p3,p4,p5)
}

unsafe extern "system" fn forward_delete_attached_surface(this:SurfaceThis,p1:u32,p2:SurfaceThis)->i32{
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).flip)(real,p1,p2)
}

unsafe extern "system" fn forward_get_attached_surface(this:SurfaceThis,p1:*mut c_void,p2:*mut SurfaceThis)->i32{
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).get_pixel_format)(real,p1)
}

unsafe extern "system" fn forward_get_surface_desc(this:SurfaceThis,p1:*mut c_void)->i32{
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).lock)(real,p1,p2,p3,p4)
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
    let real=borrowed_real(this);
    if real.is_null(){return E_POINTER;}
    (real_vtable(real).unlock)(real,p1)
}

unsafe extern "system" fn forward_update_overlay(this:SurfaceThis,p1:*mut Rect,p2:SurfaceThis,p3:*mut Rect,p4:u32,p5:*mut c_void)->i32{
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
