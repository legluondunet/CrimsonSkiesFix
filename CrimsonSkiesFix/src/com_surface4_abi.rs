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
pub(crate) mod tests {
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

    pub(crate) fn fixture()->Surface4VTable {
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


    // Strictly TEST ONLY: an actual three-object x86 COM vtable graph.
    // Its unimplemented methods return E_NOTIMPL; it is deliberately
    // never compiled or exposed in a production game DLL.
    mod surface_graph_tests {
        use super::*;
        use std::{cell::RefCell,ptr};
        use crate::{
            borderless_contract::{
                AdapterContract,Attachment,BackendEvidence,Interface,
                LogicalIdentity,RenderTarget,SurfaceContract,SurfaceHandle,SurfaceRole
            },
            borderless_flip::{FlipToBltBridge,WindowedBlt},
            presentation::{Chain,Mode},
        };

        const E_POINTER:i32=0x8000_4003u32 as i32;
        const E_FAIL:i32=0x8000_4005u32 as i32;
        const IID_IUNKNOWN:Guid=Guid{
            d1:0,d2:0,d3:0,d4:[0xc0,0,0,0,0,0,0,0x46]
        };
        const IID_SURFACE4:Guid=Guid{
            d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
            d4:[0x8e,0xa6,0,0x60,0x97,0x97,0xea,0x5b]
        };
        fn same_guid(a:&Guid,b:&Guid)->bool{
            a.d1==b.d1&&a.d2==b.d2&&a.d3==b.d3&&a.d4==b.d4
        }

        #[derive(Default)]
        struct FakeBlts {
            steps:Vec<&'static str>,
            fail_stretch:Option<i32>,
        }
        impl WindowedBlt for FakeBlts {
            fn clear_full_window_black(&mut self,_w:u32,_h:u32)->Result<(),i32>{
                self.steps.push("black");
                Ok(())
            }
            fn stretch_rgb565_into_viewport(
                &mut self,_mode:Mode,_viewport:crate::presentation::Viewport
            )->Result<(),i32>{
                self.steps.push("stretch");
                self.fail_stretch.map_or(Ok(()),Err)
            }
        }
        struct GraphState {
            chain:SurfaceContract,
            bridge:FlipToBltBridge,
            backend:FakeBlts,
            epoch:u64,
            mode:Mode,
            addresses:[SurfaceThis;3],
        }
        #[repr(C)]
        struct GraphNode {
            vtable:*const Surface4VTable,
            state:*const RefCell<GraphState>,
            handle:SurfaceHandle,
        }
        fn index(role:SurfaceRole)->usize {
            match role {
                SurfaceRole::Primary=>0,
                SurfaceRole::Backbuffer=>1,
                SurfaceRole::Depth=>2,
            }
        }
        // The test Rig owns all backing storage; no COM pointer may escape
        // its lexical lifetime or be used after logical Release to zero.
        struct Rig {
            table:Box<Surface4VTable>,
            state:Box<RefCell<GraphState>>,
            nodes:[Box<GraphNode>;3],
        }
        impl Rig {
            fn new()->Self {
                let mode=Mode{width:640,height:480,bpp:16};
                let mut adapter=AdapterContract::prepare(
                    mode,(1920,1080),
                    Chain{front_to_back:true,back_to_depth:true,mipmaps:false}
                ).unwrap();
                // Fabricated evidence is permitted ONLY for a unit-test
                // graph, never as proof of a live DirectDraw backend.
                adapter.expose(BackendEvidence{
                    process_owned_window:true,normal_directdraw:true,
                    render_target:RenderTarget::RealDirectDrawSurface4,
                    render_target_caps:0x2000,render_target_is_rgb565:true,
                    real_zbuffer_32:true,primary_with_clipper:true,
                    working_blt:true,complete_surface_com:true
                }).unwrap();
                let (chain,_primary)=SurfaceContract::new(&adapter,7).unwrap();
                let bridge=FlipToBltBridge::from_exposed(&adapter,7).unwrap();
                let mut table=Box::new(fixture());
                table.query_interface=graph_query_interface;
                table.add_ref=graph_add_ref;
                table.release=graph_release;
                table.get_attached_surface=graph_get_attached_surface;
                table.get_caps=graph_get_caps;
                table.get_surface_desc=graph_get_surface_desc;
                table.flip=graph_flip;
                let state=Box::new(RefCell::new(GraphState{
                    chain,bridge,backend:FakeBlts::default(),
                    epoch:7,mode,addresses:[ptr::null_mut();3]
                }));
                let roles=[
                    SurfaceRole::Primary,SurfaceRole::Backbuffer,SurfaceRole::Depth
                ];
                let nodes=std::array::from_fn(|i|{
                    Box::new(GraphNode{
                        vtable:&*table,
                        state:&*state,
                        handle:SurfaceHandle{
                            identity:LogicalIdentity{epoch:7,role:roles[i]},
                            interface:Interface::Surface4
                        },
                    })
                });
                for (i,node) in nodes.iter().enumerate(){
                    state.borrow_mut().addresses[i]=
                        (&**node as *const GraphNode).cast_mut().cast();
                }
                Self {table,state,nodes}
            }
            fn ptr(&self,role:SurfaceRole)->SurfaceThis{
                self.state.borrow().addresses[index(role)]
            }
            fn v(&self)->&Surface4VTable{&self.table}
        }
        unsafe fn node(this:SurfaceThis)->&'static GraphNode{
            &*this.cast::<GraphNode>()
        }
        unsafe extern "system" fn graph_query_interface(
            this:SurfaceThis,iid:*const Guid,out:*mut SurfaceThis
        )->i32{
            if out.is_null(){return E_POINTER;}
            *out=ptr::null_mut();
            if this.is_null()||iid.is_null(){return E_POINTER;}
            let id=&*iid;
            let iface=if same_guid(id,&IID_IUNKNOWN){
                Interface::IUnknown
            }else if same_guid(id,&IID_SURFACE4){
                Interface::Surface4
            }else{return E_NOINTERFACE;};
            let n=node(this);
            let mut s=(*n.state).borrow_mut();
            match s.chain.query_interface(n.handle,iface){
                Ok(_)=>{*out=this;S_OK},
                Err(_)=>E_FAIL,
            }
        }
        unsafe extern "system" fn graph_add_ref(this:SurfaceThis)->u32{
            if this.is_null(){return 0;}
            let n=node(this);
            (*n.state).borrow_mut().chain.add_ref(n.handle).unwrap_or(0)
        }
        unsafe extern "system" fn graph_release(this:SurfaceThis)->u32{
            if this.is_null(){return 0;}
            let n=node(this);
            (*n.state).borrow_mut().chain.release(n.handle).unwrap_or(0)
        }
        unsafe extern "system" fn graph_get_attached_surface(
            this:SurfaceThis,caps:*mut c_void,out:*mut SurfaceThis
        )->i32{
            if out.is_null(){return E_POINTER;}
            *out=ptr::null_mut();
            if this.is_null()||caps.is_null(){return E_POINTER;}
            let request=*(caps.cast::<u32>());
            let attachment=if request==0x00000004 {
                Attachment::Backbuffer
            }else if request==0x00020000{
                Attachment::ZBuffer
            }else{return E_FAIL;};
            let n=node(this);
            let mut s=(*n.state).borrow_mut();
            match s.chain.get_attached(n.handle,attachment){
                Ok(h)=>{*out=s.addresses[index(h.identity.role)];S_OK},
                Err(_)=>E_FAIL,
            }
        }
        unsafe extern "system" fn graph_get_caps(
            this:SurfaceThis,caps:*mut c_void
        )->i32 {
            if this.is_null()||caps.is_null(){return E_POINTER;}
            let n=node(this);
            let s=(*n.state).borrow();
            let Ok(d)=s.chain.descriptor(n.handle) else{return E_FAIL;};
            let out=caps.cast::<u32>();
            ptr::write_bytes(out,0,4); // DDSCAPS2 is 16 bytes
            *out=d.caps;
            S_OK
        }
        unsafe extern "system" fn graph_get_surface_desc(
            this:SurfaceThis,desc:*mut c_void
        )->i32{
            if this.is_null()||desc.is_null(){return E_POINTER;}
            let d=desc.cast::<u32>();
            if *d!=124{return E_FAIL;}
            let n=node(this);
            let s=(*n.state).borrow();
            let Ok(info)=s.chain.descriptor(n.handle) else{return E_FAIL;};
            ptr::write_bytes(d,0,31);
            *d=124;
            *d.add(1)=0x1007 | if info.backbuffers!=0 {0x20} else {0};
            // DDSD_CAPS|HEIGHT|WIDTH|PIXELFORMAT, plus BACKBUFFERCOUNT
            // on primary only. There is no PITCH claim in this fixture.
            *d.add(2)=info.height;
            *d.add(3)=info.width;
            *d.add(18)=32;
            *d.add(19)=if n.handle.identity.role==SurfaceRole::Depth{0x400}else{0x40};
            *d.add(21)=info.bits;
            if info.bits==16{
                *d.add(22)=0xf800;*d.add(23)=0x07e0;*d.add(24)=0x001f;
            }else{
                *d.add(23)=0x00ffffff;
            }
            *d.add(26)=info.caps;
            *d.add(5)=info.backbuffers; // dwBackBufferCount, not caps2
            S_OK
        }
        unsafe extern "system" fn graph_flip(
            this:SurfaceThis,target:SurfaceThis,flags:u32
        )->i32{
            if this.is_null(){return E_POINTER;}
            let n=node(this);
            if n.handle.identity.role!=SurfaceRole::Primary || flags!=0{
                return E_FAIL;
            }
            let mut s=(*n.state).borrow_mut();
            if !target.is_null() && target!=s.addresses[1]{return E_FAIL;}
            let epoch=s.epoch;
            let mode=s.mode;
            let GraphState{bridge,backend,..}=&mut *s;
            match bridge.present(epoch,mode,backend){
                Ok(_)=>S_OK,
                Err(crate::borderless_flip::FlipBridgeError::StretchFailed(hr))=>hr,
                Err(crate::borderless_flip::FlipBridgeError::ClearBlackFailed(hr))=>hr,
                Err(_)=>E_FAIL,
            }
        }

        #[test]
        fn distinct_surface4_physicals_and_iunknown_identity_are_stable(){
            let rig=Rig::new();let v=rig.v();
            let p=rig.ptr(SurfaceRole::Primary);
            let b=rig.ptr(SurfaceRole::Backbuffer);
            assert_ne!(p,b);
            unsafe{
                let mut u=ptr::null_mut();
                assert_eq!((v.query_interface)(p,&IID_IUNKNOWN,&mut u),S_OK);
                assert_eq!(u,p);
                let mut v4=ptr::null_mut();
                assert_eq!((v.query_interface)(u,&IID_SURFACE4,&mut v4),S_OK);
                assert_eq!(v4,p);
                let unsupported=Guid{d1:0x15e65ec0,d2:0x3b9c,d3:0x11d2,
                    d4:[0xb9,0x2f,0,0x60,0x97,0x97,0xea,0x5b]};
                let mut out=p;
                assert_eq!((v.query_interface)(p,&unsupported,&mut out),E_NOINTERFACE);
                assert!(out.is_null());
                assert_eq!((v.release)(u),2);
                assert_eq!((v.release)(v4),1);
            }
        }
        #[test]
        fn real_vtable_attachment_methods_hold_distinct_backbuffer_and_depth(){
            let rig=Rig::new();let v=rig.v();
            let p=rig.ptr(SurfaceRole::Primary);
            let b=rig.ptr(SurfaceRole::Backbuffer);
            let z=rig.ptr(SurfaceRole::Depth);
            unsafe{
                let mut back=ptr::null_mut();
                let mut caps=[0x4u32,0,0,0];
                assert_eq!((v.get_attached_surface)(p,caps.as_mut_ptr().cast(),&mut back),S_OK);
                assert_eq!(back,b);
                let mut depth=ptr::null_mut();
                caps[0]=0x20000;
                assert_eq!((v.get_attached_surface)(back,caps.as_mut_ptr().cast(),&mut depth),S_OK);
                assert_eq!(depth,z);
                let mut wrong=z;
                assert_eq!((v.get_attached_surface)(p,caps.as_mut_ptr().cast(),&mut wrong),E_FAIL);
                assert!(wrong.is_null());
                let mut desc=[0u32;31];desc[0]=124;
                assert_eq!((v.get_surface_desc)(back,desc.as_mut_ptr().cast()),S_OK);
                assert_eq!((desc[2],desc[3],desc[21]),(480,640,16));
                assert_eq!(desc[26]&0x4,0x4);
                desc[0]=124;
                assert_eq!((v.get_surface_desc)(depth,desc.as_mut_ptr().cast()),S_OK);
                assert_eq!((desc[19],desc[21],desc[23]),(0x400,32,0x00ffffff));
                let mut surface_caps=[0u32;4];
                assert_eq!((v.get_caps)(back,surface_caps.as_mut_ptr().cast()),S_OK);
                assert_eq!(surface_caps[0]&0x2000,0x2000);
                desc[0]=124;
                assert_eq!((v.get_surface_desc)(p,desc.as_mut_ptr().cast()),S_OK);
                assert_eq!((desc[1]&0x20,desc[5]),(0x20,1));
                assert_eq!(desc[27],0); // DDSCAPS2.dwCaps2 must stay clear
                assert_eq!((v.release)(p),0);
                assert_eq!((v.release)(back),0);
                assert_eq!((v.release)(depth),0);
            }
        }
        #[test]
        fn vtable_flip_only_acks_after_two_blts_and_latches_fault(){
            let rig=Rig::new();let v=rig.v();
            let p=rig.ptr(SurfaceRole::Primary);
            let back=rig.ptr(SurfaceRole::Backbuffer);
            unsafe{
                assert_eq!((v.flip)(back,ptr::null_mut(),0),E_FAIL);
                assert_eq!((v.flip)(p,back,0),S_OK);
                assert_eq!((v.flip)(p,ptr::null_mut(),0),S_OK);
            }
            {
                let s=rig.state.borrow();
                assert_eq!(s.backend.steps,["black","stretch","black","stretch"]);
                assert_eq!(s.bridge.stats().complete_frames,2);
            }
            rig.state.borrow_mut().backend.fail_stretch=Some(-42);
            unsafe{
                assert_eq!((v.flip)(p,back,0),-42);
                assert_eq!((v.flip)(p,back,0),E_FAIL);
            }
            let s=rig.state.borrow();
            assert_eq!(s.bridge.stats().complete_frames,2);
            assert_eq!(s.backend.steps.len(),6);
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
