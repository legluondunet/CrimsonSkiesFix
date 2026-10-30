//! Exact 45-method Win32 x86 IDirectDrawSurface4 COM ABI (Stage B3b).
//! Source: Wine include/ddraw.h, INTERFACE IDirectDrawSurface4.
//! https://github.com/wine-mirror/wine/blob/master/include/ddraw.h
//!
//! ONLY test-only mock objects use this table. This module is not
//! connected to the game, its DirectDraw hooks, or any real 3D surfaces.
//! DirectDrawSurface1/2/3/7 have other ABI contracts; never reuse
//! a Surface4 vtable to advertise any of those interface versions.

#![allow(dead_code)]
use std::ffi::c_void;
use crate::Guid;
pub type SurfaceThis=*mut c_void;
const S_OK:i32=0;
const E_NOINTERFACE:i32=0x8000_4002u32 as i32;
const E_NOTIMPL:i32=0x8000_4001u32 as i32;
#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Rect {pub left:i32,pub top:i32,pub right:i32,pub bottom:i32}

/// Raw pointer parameter types mirror DirectDraw header signatures.
/// Opaque descriptors, FX, callbacks and caps are never dereferenced.
/// Every vtable entry occupies one function pointer on Win32 x86.
#[repr(C)]
pub struct Surface4VTable {
    pub query_interface: unsafe extern "system" fn(SurfaceThis, *const Guid, *mut *mut c_void)->i32,
    pub add_ref: unsafe extern "system" fn(SurfaceThis)->u32,
    pub release: unsafe extern "system" fn(SurfaceThis)->u32,
    pub add_attached_surface: unsafe extern "system" fn(SurfaceThis, SurfaceThis)->i32,
    pub add_overlay_dirty_rect: unsafe extern "system" fn(SurfaceThis, *mut Rect)->i32,
    pub blt: unsafe extern "system" fn(SurfaceThis, *mut Rect, SurfaceThis, *mut Rect, u32, *mut c_void)->i32,
    pub blt_batch: unsafe extern "system" fn(SurfaceThis, *mut c_void, u32, u32)->i32,
    pub blt_fast: unsafe extern "system" fn(SurfaceThis, u32, u32, SurfaceThis, *mut Rect, u32)->i32,
    pub delete_attached_surface: unsafe extern "system" fn(SurfaceThis, u32, SurfaceThis)->i32,
    pub enum_attached_surfaces: unsafe extern "system" fn(SurfaceThis, *mut c_void, *const c_void)->i32,
    pub enum_overlay_z_orders: unsafe extern "system" fn(SurfaceThis, u32, *mut c_void, *const c_void)->i32,
    pub flip: unsafe extern "system" fn(SurfaceThis, SurfaceThis, u32)->i32,
    pub get_attached_surface: unsafe extern "system" fn(SurfaceThis, *mut c_void, *mut SurfaceThis)->i32,
    pub get_blt_status: unsafe extern "system" fn(SurfaceThis, u32)->i32,
    pub get_caps: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub get_clipper: unsafe extern "system" fn(SurfaceThis, *mut *mut c_void)->i32,
    pub get_color_key: unsafe extern "system" fn(SurfaceThis, u32, *mut c_void)->i32,
    pub get_dc: unsafe extern "system" fn(SurfaceThis, *mut *mut c_void)->i32,
    pub get_flip_status: unsafe extern "system" fn(SurfaceThis, u32)->i32,
    pub get_overlay_position: unsafe extern "system" fn(SurfaceThis, *mut i32, *mut i32)->i32,
    pub get_palette: unsafe extern "system" fn(SurfaceThis, *mut *mut c_void)->i32,
    pub get_pixel_format: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub get_surface_desc: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub initialize: unsafe extern "system" fn(SurfaceThis, *mut c_void, *mut c_void)->i32,
    pub is_lost: unsafe extern "system" fn(SurfaceThis)->i32,
    pub lock: unsafe extern "system" fn(SurfaceThis, *mut Rect, *mut c_void, u32, *mut c_void)->i32,
    pub release_dc: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub restore: unsafe extern "system" fn(SurfaceThis)->i32,
    pub set_clipper: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub set_color_key: unsafe extern "system" fn(SurfaceThis, u32, *mut c_void)->i32,
    pub set_overlay_position: unsafe extern "system" fn(SurfaceThis, i32, i32)->i32,
    pub set_palette: unsafe extern "system" fn(SurfaceThis, *mut c_void)->i32,
    pub unlock: unsafe extern "system" fn(SurfaceThis, *mut Rect)->i32,
    pub update_overlay: unsafe extern "system" fn(SurfaceThis, *mut Rect, SurfaceThis, *mut Rect, u32, *mut c_void)->i32,
    pub update_overlay_display: unsafe extern "system" fn(SurfaceThis, u32)->i32,
    pub update_overlay_z_order: unsafe extern "system" fn(SurfaceThis, u32, SurfaceThis)->i32,
    pub get_dd_interface: unsafe extern "system" fn(SurfaceThis, *mut *mut c_void)->i32,
    pub page_lock: unsafe extern "system" fn(SurfaceThis, u32)->i32,
    pub page_unlock: unsafe extern "system" fn(SurfaceThis, u32)->i32,
    pub set_surface_desc: unsafe extern "system" fn(SurfaceThis, *mut c_void, u32)->i32,
    pub set_private_data: unsafe extern "system" fn(SurfaceThis, *const Guid, *mut c_void, u32, u32)->i32,
    pub get_private_data: unsafe extern "system" fn(SurfaceThis, *const Guid, *mut c_void, *mut u32)->i32,
    pub free_private_data: unsafe extern "system" fn(SurfaceThis, *const Guid)->i32,
    pub get_uniqueness_value: unsafe extern "system" fn(SurfaceThis, *mut u32)->i32,
    pub change_uniqueness_value: unsafe extern "system" fn(SurfaceThis)->i32,
}

pub const SURFACE4_SLOTS:[&str;45]=[
    "query_interface", // 0
    "add_ref", // 1
    "release", // 2
    "add_attached_surface", // 3
    "add_overlay_dirty_rect", // 4
    "blt", // 5
    "blt_batch", // 6
    "blt_fast", // 7
    "delete_attached_surface", // 8
    "enum_attached_surfaces", // 9
    "enum_overlay_z_orders", // 10
    "flip", // 11
    "get_attached_surface", // 12
    "get_blt_status", // 13
    "get_caps", // 14
    "get_clipper", // 15
    "get_color_key", // 16
    "get_dc", // 17
    "get_flip_status", // 18
    "get_overlay_position", // 19
    "get_palette", // 20
    "get_pixel_format", // 21
    "get_surface_desc", // 22
    "initialize", // 23
    "is_lost", // 24
    "lock", // 25
    "release_dc", // 26
    "restore", // 27
    "set_clipper", // 28
    "set_color_key", // 29
    "set_overlay_position", // 30
    "set_palette", // 31
    "unlock", // 32
    "update_overlay", // 33
    "update_overlay_display", // 34
    "update_overlay_z_order", // 35
    "get_dd_interface", // 36
    "page_lock", // 37
    "page_unlock", // 38
    "set_surface_desc", // 39
    "set_private_data", // 40
    "get_private_data", // 41
    "free_private_data", // 42
    "get_uniqueness_value", // 43
    "change_uniqueness_value", // 44
];

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use std::{mem::{size_of,align_of,offset_of},ptr};
    unsafe extern "system" fn mock_query_interface(_this:SurfaceThis, iid:*const Guid, out:*mut *mut c_void)->i32{ if !out.is_null(){*out=ptr::null_mut();} E_NOINTERFACE }
    unsafe extern "system" fn mock_add_ref(_this:SurfaceThis)->u32{ 1 }
    unsafe extern "system" fn mock_release(_this:SurfaceThis)->u32{ 1 }
    unsafe extern "system" fn mock_add_attached_surface(_this:SurfaceThis, surface:SurfaceThis)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_add_overlay_dirty_rect(_this:SurfaceThis, rect:*mut Rect)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_blt(_this:SurfaceThis, dst:*mut Rect, source:SurfaceThis, src:*mut Rect, flags:u32, effects:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_blt_batch(_this:SurfaceThis, batch:*mut c_void, count:u32, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_blt_fast(_this:SurfaceThis, x:u32, y:u32, source:SurfaceThis, src:*mut Rect, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_delete_attached_surface(_this:SurfaceThis, flags:u32, surface:SurfaceThis)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_enum_attached_surfaces(_this:SurfaceThis, context:*mut c_void, callback:*const c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_enum_overlay_z_orders(_this:SurfaceThis, flags:u32, context:*mut c_void, callback:*const c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_flip(_this:SurfaceThis, target:SurfaceThis, flags:u32)->i32{ flags as i32 }
    unsafe extern "system" fn mock_get_attached_surface(_this:SurfaceThis, caps:*mut c_void, out:*mut SurfaceThis)->i32{ if !out.is_null(){*out=0x1234usize as SurfaceThis;} S_OK }
    unsafe extern "system" fn mock_get_blt_status(_this:SurfaceThis, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_caps(_this:SurfaceThis, caps:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_clipper(_this:SurfaceThis, clipper:*mut *mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_color_key(_this:SurfaceThis, flags:u32, key:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_dc(_this:SurfaceThis, dc:*mut *mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_flip_status(_this:SurfaceThis, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_overlay_position(_this:SurfaceThis, x:*mut i32, y:*mut i32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_palette(_this:SurfaceThis, palette:*mut *mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_pixel_format(_this:SurfaceThis, format:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_surface_desc(_this:SurfaceThis, desc:*mut c_void)->i32{ if !desc.is_null(){*desc.cast::<u32>()=124;} S_OK }
    unsafe extern "system" fn mock_initialize(_this:SurfaceThis, ddraw:*mut c_void, desc:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_is_lost(_this:SurfaceThis)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_lock(_this:SurfaceThis, rect:*mut Rect, desc:*mut c_void, flags:u32, event:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_release_dc(_this:SurfaceThis, dc:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_restore(_this:SurfaceThis)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_clipper(_this:SurfaceThis, clipper:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_color_key(_this:SurfaceThis, flags:u32, key:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_overlay_position(_this:SurfaceThis, x:i32, y:i32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_palette(_this:SurfaceThis, palette:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_unlock(_this:SurfaceThis, rect:*mut Rect)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_update_overlay(_this:SurfaceThis, src:*mut Rect, target:SurfaceThis, dst:*mut Rect, flags:u32, effects:*mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_update_overlay_display(_this:SurfaceThis, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_update_overlay_z_order(_this:SurfaceThis, flags:u32, reference_surface:SurfaceThis)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_dd_interface(_this:SurfaceThis, ddraw:*mut *mut c_void)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_page_lock(_this:SurfaceThis, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_page_unlock(_this:SurfaceThis, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_surface_desc(_this:SurfaceThis, desc:*mut c_void, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_set_private_data(_this:SurfaceThis, tag:*const Guid, data:*mut c_void, size:u32, flags:u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_private_data(_this:SurfaceThis, tag:*const Guid, data:*mut c_void, size:*mut u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_free_private_data(_this:SurfaceThis, tag:*const Guid)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_get_uniqueness_value(_this:SurfaceThis, value:*mut u32)->i32{ E_NOTIMPL }
    unsafe extern "system" fn mock_change_uniqueness_value(_this:SurfaceThis)->i32{ E_NOTIMPL }

    fn fixture()->Surface4VTable {
        Surface4VTable {
            query_interface:mock_query_interface,
            add_ref:mock_add_ref,
            release:mock_release,
            add_attached_surface:mock_add_attached_surface,
            add_overlay_dirty_rect:mock_add_overlay_dirty_rect,
            blt:mock_blt,
            blt_batch:mock_blt_batch,
            blt_fast:mock_blt_fast,
            delete_attached_surface:mock_delete_attached_surface,
            enum_attached_surfaces:mock_enum_attached_surfaces,
            enum_overlay_z_orders:mock_enum_overlay_z_orders,
            flip:mock_flip,
            get_attached_surface:mock_get_attached_surface,
            get_blt_status:mock_get_blt_status,
            get_caps:mock_get_caps,
            get_clipper:mock_get_clipper,
            get_color_key:mock_get_color_key,
            get_dc:mock_get_dc,
            get_flip_status:mock_get_flip_status,
            get_overlay_position:mock_get_overlay_position,
            get_palette:mock_get_palette,
            get_pixel_format:mock_get_pixel_format,
            get_surface_desc:mock_get_surface_desc,
            initialize:mock_initialize,
            is_lost:mock_is_lost,
            lock:mock_lock,
            release_dc:mock_release_dc,
            restore:mock_restore,
            set_clipper:mock_set_clipper,
            set_color_key:mock_set_color_key,
            set_overlay_position:mock_set_overlay_position,
            set_palette:mock_set_palette,
            unlock:mock_unlock,
            update_overlay:mock_update_overlay,
            update_overlay_display:mock_update_overlay_display,
            update_overlay_z_order:mock_update_overlay_z_order,
            get_dd_interface:mock_get_dd_interface,
            page_lock:mock_page_lock,
            page_unlock:mock_page_unlock,
            set_surface_desc:mock_set_surface_desc,
            set_private_data:mock_set_private_data,
            get_private_data:mock_get_private_data,
            free_private_data:mock_free_private_data,
            get_uniqueness_value:mock_get_uniqueness_value,
            change_uniqueness_value:mock_change_uniqueness_value,
        }
    }

    #[test]
    fn all_45_slots_match_win32_x86_offsets(){
        assert_eq!(size_of::<usize>(),4);
        assert_eq!(size_of::<Guid>(),16);
        assert_eq!(size_of::<Rect>(),16);
        assert_eq!(align_of::<Surface4VTable>(),4);
        assert_eq!(size_of::<Surface4VTable>(),180);
        assert_eq!(offset_of!(Surface4VTable, query_interface), 0*4);
        assert_eq!(offset_of!(Surface4VTable, add_ref), 1*4);
        assert_eq!(offset_of!(Surface4VTable, release), 2*4);
        assert_eq!(offset_of!(Surface4VTable, add_attached_surface), 3*4);
        assert_eq!(offset_of!(Surface4VTable, add_overlay_dirty_rect), 4*4);
        assert_eq!(offset_of!(Surface4VTable, blt), 5*4);
        assert_eq!(offset_of!(Surface4VTable, blt_batch), 6*4);
        assert_eq!(offset_of!(Surface4VTable, blt_fast), 7*4);
        assert_eq!(offset_of!(Surface4VTable, delete_attached_surface), 8*4);
        assert_eq!(offset_of!(Surface4VTable, enum_attached_surfaces), 9*4);
        assert_eq!(offset_of!(Surface4VTable, enum_overlay_z_orders), 10*4);
        assert_eq!(offset_of!(Surface4VTable, flip), 11*4);
        assert_eq!(offset_of!(Surface4VTable, get_attached_surface), 12*4);
        assert_eq!(offset_of!(Surface4VTable, get_blt_status), 13*4);
        assert_eq!(offset_of!(Surface4VTable, get_caps), 14*4);
        assert_eq!(offset_of!(Surface4VTable, get_clipper), 15*4);
        assert_eq!(offset_of!(Surface4VTable, get_color_key), 16*4);
        assert_eq!(offset_of!(Surface4VTable, get_dc), 17*4);
        assert_eq!(offset_of!(Surface4VTable, get_flip_status), 18*4);
        assert_eq!(offset_of!(Surface4VTable, get_overlay_position), 19*4);
        assert_eq!(offset_of!(Surface4VTable, get_palette), 20*4);
        assert_eq!(offset_of!(Surface4VTable, get_pixel_format), 21*4);
        assert_eq!(offset_of!(Surface4VTable, get_surface_desc), 22*4);
        assert_eq!(offset_of!(Surface4VTable, initialize), 23*4);
        assert_eq!(offset_of!(Surface4VTable, is_lost), 24*4);
        assert_eq!(offset_of!(Surface4VTable, lock), 25*4);
        assert_eq!(offset_of!(Surface4VTable, release_dc), 26*4);
        assert_eq!(offset_of!(Surface4VTable, restore), 27*4);
        assert_eq!(offset_of!(Surface4VTable, set_clipper), 28*4);
        assert_eq!(offset_of!(Surface4VTable, set_color_key), 29*4);
        assert_eq!(offset_of!(Surface4VTable, set_overlay_position), 30*4);
        assert_eq!(offset_of!(Surface4VTable, set_palette), 31*4);
        assert_eq!(offset_of!(Surface4VTable, unlock), 32*4);
        assert_eq!(offset_of!(Surface4VTable, update_overlay), 33*4);
        assert_eq!(offset_of!(Surface4VTable, update_overlay_display), 34*4);
        assert_eq!(offset_of!(Surface4VTable, update_overlay_z_order), 35*4);
        assert_eq!(offset_of!(Surface4VTable, get_dd_interface), 36*4);
        assert_eq!(offset_of!(Surface4VTable, page_lock), 37*4);
        assert_eq!(offset_of!(Surface4VTable, page_unlock), 38*4);
        assert_eq!(offset_of!(Surface4VTable, set_surface_desc), 39*4);
        assert_eq!(offset_of!(Surface4VTable, set_private_data), 40*4);
        assert_eq!(offset_of!(Surface4VTable, get_private_data), 41*4);
        assert_eq!(offset_of!(Surface4VTable, free_private_data), 42*4);
        assert_eq!(offset_of!(Surface4VTable, get_uniqueness_value), 43*4);
        assert_eq!(offset_of!(Surface4VTable, change_uniqueness_value), 44*4);
    }

    #[test]
    fn known_game_slots_match_authoritative_surface4_order(){
        assert_eq!(SURFACE4_SLOTS[0],"query_interface");
        assert_eq!(SURFACE4_SLOTS[5],"blt");
        assert_eq!(SURFACE4_SLOTS[7],"blt_fast");
        assert_eq!(SURFACE4_SLOTS[11],"flip");
        assert_eq!(SURFACE4_SLOTS[12],"get_attached_surface");
        assert_eq!(SURFACE4_SLOTS[14],"get_caps");
        assert_eq!(SURFACE4_SLOTS[22],"get_surface_desc");
        assert_eq!(SURFACE4_SLOTS[25],"lock");
        assert_eq!(SURFACE4_SLOTS[32],"unlock");
        assert_eq!(SURFACE4_SLOTS[44],"change_uniqueness_value");
    }

    #[repr(C)]
    struct Mock {vtable:*const Surface4VTable}
    #[test]
    fn typed_vtable_dispatch_preserves_argument_order(){
        let table=fixture();
        let object=Mock{vtable:&table};
        let this=(&object as *const Mock).cast_mut().cast::<c_void>();
        unsafe {
            let v=&*object.vtable;
            assert_eq!((v.flip)(this,ptr::null_mut(),0x70),0x70);
            let mut attached=ptr::null_mut();
            assert_eq!((v.get_attached_surface)(this,ptr::null_mut(),&mut attached),S_OK);
            assert_eq!(attached as usize,0x1234);
            let mut descriptor=[0u32;31];
            assert_eq!((v.get_surface_desc)(this,descriptor.as_mut_ptr().cast()),S_OK);
            assert_eq!(descriptor[0],124);
            assert_eq!((v.unlock)(this,ptr::null_mut()),E_NOTIMPL);
        }
    }

    #[test]
    fn test_mock_cannot_advertise_an_incomplete_surface4_interface(){
        let table=fixture();
        let mock=Mock{vtable:&table};
        let this=(&mock as *const Mock).cast_mut().cast::<c_void>();
        let iid=Guid{d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
                     d4:[0x8e,0xa6,0x00,0x60,0x97,0x97,0xea,0x5b]};
        let mut out=this;
        unsafe {
            assert_eq!((table.query_interface)(this,&iid,&mut out),E_NOINTERFACE);
            assert!(out.is_null());
        }
    }
}
