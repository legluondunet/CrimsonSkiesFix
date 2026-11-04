//! Stage B4i: inactive COM identity round-trip across native D3D3.
//!
//! Wine's IDirect3D3::CreateDevice must receive its native Surface4
//! implementation, not our independent COM wrapper. The game-facing
//! GetRenderTarget result, however, must map back to the SAME wrapper
//! identity and carry its own AddRef. This isolated one-target map
//! models exactly that boundary. A full surface-chain registry and
//! primary/Flip interception have NOT been implemented.
//!
//! Owns one AddRef to the wrapper; wrapper owns the real COM surface.
//! Physical GetRenderTarget results still require caller's own Release.

#![allow(dead_code)]
use std::ptr;
use crate::{
    com_surface4_abi::{Surface4VTable,SurfaceThis},
    com_surface4_forward::try_borrow_real_surface4,
};
const E_POINTER:i32=0x8000_4003u32 as i32;
const E_INVALIDARG:i32=0x8007_0057u32 as i32;
const E_FAIL:i32=0x8000_4005u32 as i32;

pub struct Surface4RoundTrip {
    wrapped:SurfaceThis,
    native:SurfaceThis,
}
impl Surface4RoundTrip {
    /// # Safety
    /// The wrapper argument must be a live COM Surface4 pointer owned by
    /// the caller and created by new_forwarding_surface4. The caller keeps
    /// that reference alive until this function finishes. Unknown native
    /// or unrelated COM pointers are not accepted as a registered wrapper.
    pub unsafe fn new(wrapped:SurfaceThis)->Result<Self,i32>{
        if wrapped.is_null(){return Err(E_POINTER);}
        let native=try_borrow_real_surface4(wrapped).ok_or(E_INVALIDARG)?;
        if native.is_null() || native==wrapped{return Err(E_INVALIDARG);}
        let table=&**wrapped.cast::<*const Surface4VTable>();
        if (table.add_ref)(wrapped)==0{return Err(E_FAIL);}
        Ok(Self{wrapped,native})
    }

    /// Native COM interface to provide only to IDirect3D3::CreateDevice.
    /// Borrowed: do not Release and do not use after dropping the map.
    pub fn native_target(&self)->SurfaceThis{self.native}

    /// Convert an already-acquired native result from GetRenderTarget to
    /// the game-facing wrapper. The returned virtual interface owns
    /// exactly one new AddRef. The original native GetRenderTarget pointer
    /// is *still owned by its caller*, and must be Release'd separately.
    ///
    /// # Safety
    /// native_result must be a live COM Surface4 pointer if non-null.
    /// Caller must retain its returned native reference while mapping.
    pub unsafe fn map_native_result(&self,native_result:SurfaceThis)
        ->Result<SurfaceThis,i32>{
        if native_result.is_null(){return Err(E_POINTER);}
        if native_result!=self.native{return Err(E_INVALIDARG);}
        let table=&**self.wrapped.cast::<*const Surface4VTable>();
        if (table.add_ref)(self.wrapped)==0{return Err(E_FAIL);}
        Ok(self.wrapped)
    }

    pub fn matches_wrapper(&self,wrapped:SurfaceThis)->bool{
        !wrapped.is_null() && wrapped==self.wrapped
    }
}
impl Drop for Surface4RoundTrip {
    fn drop(&mut self){
        unsafe {
            if !self.wrapped.is_null(){
                let table=&**self.wrapped.cast::<*const Surface4VTable>();
                (table.release)(self.wrapped);
                self.wrapped=ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::atomic::{AtomicU32,Ordering};
    use crate::{Guid,com_surface4_forward::new_forwarding_surface4};
    #[repr(C)]
    struct Real{
        vtable:*const Surface4VTable,
        refs:AtomicU32,
    }
    unsafe extern "system" fn add_ref(this:SurfaceThis)->u32{
        (&*this.cast::<Real>()).refs.fetch_add(1,Ordering::SeqCst)+1
    }
    unsafe extern "system" fn release(this:SurfaceThis)->u32{
        (&*this.cast::<Real>()).refs.fetch_sub(1,Ordering::SeqCst)-1
    }
    fn fixture()->Surface4VTable{
        let mut t=crate::com_surface4_abi::tests::fixture();
        t.add_ref=add_ref;t.release=release;t
    }
    const UNKNOWN:Guid=Guid{d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]};
    #[test]
    fn native_device_boundary_roundtrips_to_same_wrapper_identity(){
        let t=fixture();
        let mut real=Real{vtable:&t,refs:AtomicU32::new(1)};
        let native=(&mut real as *mut Real).cast::<c_void>();
        unsafe{
            let wrapped=new_forwarding_surface4(native).unwrap();
            assert_eq!(real.refs.load(Ordering::SeqCst),2);
            let table=&**wrapped.cast::<*const Surface4VTable>();
            let map=Surface4RoundTrip::new(wrapped).unwrap();
            assert_eq!(map.native_target(),native);
            assert!(map.matches_wrapper(wrapped));
            assert!(!map.matches_wrapper(native));
            // Caller releases its original wrapper before D3D3 reports
            // a target. The map's own AddRef must still protect it.
            assert_eq!((table.release)(wrapped),1);
            let returned=map.map_native_result(native).unwrap();
            assert_eq!(returned,wrapped);
            let mut unknown=ptr::null_mut();
            assert_eq!((table.query_interface)(returned,&UNKNOWN,&mut unknown),0);
            assert_eq!(unknown,wrapped);
            assert_eq!((table.release)(unknown),2);
            assert_eq!((table.release)(returned),1);
            assert_eq!(real.refs.load(Ordering::SeqCst),2);
            drop(map);
        }
        assert_eq!(real.refs.load(Ordering::SeqCst),1);
    }
    #[test]
    fn rejects_foreign_native_and_null_without_creating_client_refs(){
        let t=fixture();
        let mut a=Real{vtable:&t,refs:AtomicU32::new(1)};
        let mut b=Real{vtable:&t,refs:AtomicU32::new(1)};
        let native=(&mut a as *mut Real).cast::<c_void>();
        let foreign=(&mut b as *mut Real).cast::<c_void>();
        unsafe{
            assert!(matches!(Surface4RoundTrip::new(ptr::null_mut()),Err(E_POINTER)));
            assert!(matches!(Surface4RoundTrip::new(native),Err(E_INVALIDARG)));
            let wrapped=new_forwarding_surface4(native).unwrap();
            let map=Surface4RoundTrip::new(wrapped).unwrap();
            assert!(matches!(map.map_native_result(ptr::null_mut()),Err(E_POINTER)));
            assert!(matches!(map.map_native_result(foreign),Err(E_INVALIDARG)));
            assert_eq!(a.refs.load(Ordering::SeqCst),2);
            let table=&**wrapped.cast::<*const Surface4VTable>();
            assert_eq!((table.release)(wrapped),1);
            drop(map);
        }
        assert_eq!(a.refs.load(Ordering::SeqCst),1);
        assert_eq!(b.refs.load(Ordering::SeqCst),1);
    }
}
