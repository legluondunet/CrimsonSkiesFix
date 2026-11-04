//! Native Win32 x86 IUnknown vtable proof of concept (Stage B3a).
//!
//! Entirely inactive during game execution: no export, DirectDraw COM
//! interception, or runtime constructor invokes this module. These are
//! real ABI-compatible IUnknown objects, but deliberately NOT usable
//! IDirectDrawSurface objects. Never expose them as a game surface!
//!
//! Only IID_IUnknown is supported. Advertising Surface1/2/3/4/7 before
//! implementing their complete ABI would be unsafe and misleading.
//! Aggregation, marshalling, Direct3D targets, attachment methods and
//! presentation remain future stages.

#![allow(dead_code)]

use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicU32,Ordering},
};

use crate::{Guid,borderless_contract::SurfaceRole};

const S_OK:i32=0;
const E_NOINTERFACE:i32=0x8000_4002u32 as i32;
const E_POINTER:i32=0x8000_4003u32 as i32;
const E_FAIL:i32=0x8000_4005u32 as i32;
const IID_IUNKNOWN:Guid=Guid {
    d1:0, d2:0, d3:0,
    d4:[0xc0,0x00,0x00,0x00,0x00,0x00,0x00,0x46],
};

/// Win32/x86 COM ABI: this pointer + REFIID + void** for QueryInterface;
/// these are the first three slots of *every* COM interface vtable.
#[repr(C)]
struct IUnknownVTable {
    query_interface:unsafe extern "system" fn(*mut c_void,*const Guid,*mut *mut c_void)->i32,
    add_ref:unsafe extern "system" fn(*mut c_void)->u32,
    release:unsafe extern "system" fn(*mut c_void)->u32,
}

/// A single physical COM identity with a fixed IUnknown vtable.
/// The vtable pointer MUST occupy offset zero. No exposed surface
/// methods exist, and no object is installed in the game runtime.
#[repr(C)]
struct UnknownObject {
    vtable:*const IUnknownVTable,
    references:AtomicU32,
    role:SurfaceRole,
    generation:u64,
    #[cfg(test)]
    drop_counter:Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

#[cfg(test)]
impl Drop for UnknownObject {
    fn drop(&mut self) {
        if let Some(counter)=self.drop_counter.as_ref(){
            counter.fetch_add(1,Ordering::SeqCst);
        }
    }
}

static IUNKNOWN_VTABLE:IUnknownVTable=IUnknownVTable {
    query_interface:unknown_query_interface,
    add_ref:unknown_add_ref,
    release:unknown_release,
};

/// Create one standalone IUnknown pointer with one caller-owned COM ref.
/// The caller must release this pointer exactly once. This constructor
/// does not imply that any DirectDraw API supports this object.
///
/// The raw pointer must never be used after the final Release, and must
/// not be sent to a game or to Direct3D as a surface interface.
fn new_iunknown(role:SurfaceRole,generation:u64)->*mut c_void {
    let instance=Box::new(UnknownObject{
        vtable:&IUNKNOWN_VTABLE,
        references:AtomicU32::new(1),
        role,
        generation,
        #[cfg(test)]
        drop_counter:None,
    });
    Box::into_raw(instance).cast()
}

/// Failure-safe increment; COM AddRef has no HRESULT channel, so a
/// count of zero means a corrupt/dead object or exhausted u32 count.
/// A compliant caller holds a live reference, making exhaustion
/// unreachable in ordinary game usage.
fn increment_reference(object:&UnknownObject)->Option<u32> {
    let mut old=object.references.load(Ordering::Acquire);
    loop {
        if old==0 || old==u32::MAX{return None;}
        match object.references.compare_exchange_weak(
            old,old+1,Ordering::AcqRel,Ordering::Acquire
        ){
            Ok(_)=>return Some(old+1),
            Err(actual)=>old=actual,
        }
    }
}

/// COM QueryInterface must return the same *physical* IUnknown pointer
/// for the same object and AddRef it. The set of accepted IIDs is static.
/// The caller must supply a valid, live COM 'this' pointer.
unsafe extern "system" fn unknown_query_interface(
    this:*mut c_void,iid:*const Guid,out:*mut *mut c_void
)->i32 {
    if out.is_null(){return E_POINTER;}
    *out=ptr::null_mut();
    if this.is_null() || iid.is_null(){return E_POINTER;}
    let id=&*iid;
    if id.d1!=IID_IUNKNOWN.d1 ||
       id.d2!=IID_IUNKNOWN.d2 ||
       id.d3!=IID_IUNKNOWN.d3 ||
       id.d4!=IID_IUNKNOWN.d4
    {
        return E_NOINTERFACE;
    }
    let object=&*this.cast::<UnknownObject>();
    if increment_reference(object).is_none(){return E_FAIL;}
    *out=this;
    S_OK
}

unsafe extern "system" fn unknown_add_ref(this:*mut c_void)->u32 {
    if this.is_null(){return 0;}
    let object=&*this.cast::<UnknownObject>();
    increment_reference(object).unwrap_or(0)
}

unsafe extern "system" fn unknown_release(this:*mut c_void)->u32 {
    if this.is_null(){return 0;}
    let object=&*this.cast::<UnknownObject>();
    let mut old=object.references.load(Ordering::Acquire);
    loop {
        if old==0{return 0;}
        match object.references.compare_exchange_weak(
            old,old-1,Ordering::AcqRel,Ordering::Acquire
        ){
            Ok(_)=>{
                let remaining=old-1;
                if remaining==0 {
                    // No further access to this or object is allowed.
                    // Other threads must already have their own refs.
                    std::sync::atomic::fence(Ordering::Acquire);
                    drop(Box::from_raw(this.cast::<UnknownObject>()));
                }
                return remaining;
            }
            Err(actual)=>old=actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize,Ordering as TestOrdering},
    };

    unsafe fn vtable(pointer:*mut c_void)->&'static IUnknownVTable {
        // Valid only while the test owns a live reference.
        &**(pointer.cast::<*const IUnknownVTable>())
    }

    unsafe fn with_counter(
        role:SurfaceRole,generation:u64,counter:Arc<AtomicUsize>
    )->*mut c_void {
        let pointer=new_iunknown(role,generation);
        (*pointer.cast::<UnknownObject>()).drop_counter=Some(counter);
        pointer
    }

    #[test]
    fn iunknown_has_exact_three_function_pointer_abi_and_zero_offset(){
        assert_eq!(std::mem::size_of::<usize>(),4);
        assert_eq!(std::mem::size_of::<Guid>(),16);
        assert_eq!(std::mem::size_of::<IUnknownVTable>(),3*std::mem::size_of::<usize>());
        assert_eq!(std::mem::offset_of!(UnknownObject,vtable),0);
        unsafe {
            let object=new_iunknown(SurfaceRole::Primary,7);
            assert_eq!((*object.cast::<UnknownObject>()).generation,7);
            assert_eq!((*object.cast::<UnknownObject>()).role,SurfaceRole::Primary);
            let vt=vtable(object);
            assert_eq!(vt.query_interface as usize,unknown_query_interface as usize);
            assert_eq!(vt.add_ref as usize,unknown_add_ref as usize);
            assert_eq!(vt.release as usize,unknown_release as usize);
            assert_eq!((vt.release)(object),0);
        }
    }

    #[test]
    fn querying_iunknown_returns_same_address_and_increments_reference(){
        unsafe {
            let original=new_iunknown(SurfaceRole::Backbuffer,12);
            let vt=vtable(original);
            let mut first=ptr::null_mut();
            assert_eq!((vt.query_interface)(original,&IID_IUNKNOWN,&mut first),S_OK);
            assert_eq!(first,original);
            let mut second=ptr::null_mut();
            assert_eq!((vt.query_interface)(first,&IID_IUNKNOWN,&mut second),S_OK);
            assert_eq!(second,original);
            assert_eq!((vt.release)(second),2);
            assert_eq!((vt.release)(first),1);
            assert_eq!((vt.release)(original),0);
        }
    }

    #[test]
    fn unsupported_directdraw_interface_is_never_advertised(){
        let iid_surface4=Guid {
            d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
            d4:[0x8e,0xa6,0x00,0x60,0x97,0x97,0xea,0x5b],
        };
        unsafe {
            let p=new_iunknown(SurfaceRole::Depth,3);
            let vt=vtable(p);
            let mut out=p;
            assert_eq!((vt.query_interface)(p,&iid_surface4,&mut out),E_NOINTERFACE);
            assert!(out.is_null());
            assert_eq!((vt.release)(p),0);
        }
    }

    #[test]
    fn query_interface_checks_null_out_and_iid_without_changing_refs(){
        unsafe {
            let p=new_iunknown(SurfaceRole::Primary,1);
            let vt=vtable(p);
            assert_eq!((vt.query_interface)(p,&IID_IUNKNOWN,ptr::null_mut()),E_POINTER);
            let mut out=p;
            assert_eq!((vt.query_interface)(p,ptr::null(),&mut out),E_POINTER);
            assert!(out.is_null());
            out=p;
            assert_eq!((vt.query_interface)(ptr::null_mut(),&IID_IUNKNOWN,&mut out),E_POINTER);
            assert!(out.is_null());
            assert_eq!((vt.release)(p),0);
        }
    }

    #[test]
    fn multiple_physical_objects_have_distinct_iunknown_identity(){
        unsafe {
            let p=new_iunknown(SurfaceRole::Primary,1);
            let b=new_iunknown(SurfaceRole::Backbuffer,1);
            let mut up=ptr::null_mut();
            let mut ub=ptr::null_mut();
            assert_eq!((vtable(p).query_interface)(p,&IID_IUNKNOWN,&mut up),S_OK);
            assert_eq!((vtable(b).query_interface)(b,&IID_IUNKNOWN,&mut ub),S_OK);
            assert_ne!(up,ub);
            assert_eq!((vtable(up).release)(up),1);
            assert_eq!((vtable(ub).release)(ub),1);
            assert_eq!((vtable(p).release)(p),0);
            assert_eq!((vtable(b).release)(b),0);
        }
    }

    #[test]
    fn final_release_destroys_object_once_and_nonfinal_release_keeps_it(){
        let count=Arc::new(AtomicUsize::new(0));
        unsafe {
            let p=with_counter(SurfaceRole::Backbuffer,9,count.clone());
            let vt=vtable(p);
            assert_eq!((vt.add_ref)(p),2);
            assert_eq!((vt.release)(p),1);
            assert_eq!(count.load(TestOrdering::SeqCst),0);
            assert_eq!((vt.release)(p),0);
        }
        assert_eq!(count.load(TestOrdering::SeqCst),1);
    }

    #[test]
    fn reference_overflow_fails_closed_without_wrapping(){
        unsafe {
            let p=new_iunknown(SurfaceRole::Primary,10);
            let vt=vtable(p);
            let object=&*p.cast::<UnknownObject>();
            object.references.store(u32::MAX,Ordering::Release);
            assert_eq!((vt.add_ref)(p),0);
            let mut out=p;
            assert_eq!((vt.query_interface)(p,&IID_IUNKNOWN,&mut out),E_FAIL);
            assert!(out.is_null());
            assert_eq!(object.references.load(Ordering::Acquire),u32::MAX);
            object.references.store(1,Ordering::Release);
            assert_eq!((vt.release)(p),0);
        }
    }
}
