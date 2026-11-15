//! Experimental camera/world/HUD screen-space geometry aspect A/B.
//!
//! Evidence from 15 Nov 2026 native DD4 WinMM under GE-Proton 11-12:
//! 1920x1080 D3D3 DrawPrimitive(quad) and DrawIndexedPrimitive(mesh) BOTH
//! have FVF=0x000001c4 (XYZRHW | DIFFUSE | SPECULAR | TEX1), already
//! transformed into screen pixels by the original game software renderer.
//! IDirect3DDevice3::GetTransform(PROJECTION)=identity is therefore
//! NOT the camera's effective 3D projection matrix.
//!
//! A/B actual 1920x1200 screenshots vs centered 1600x1200 demonstrated
//! that BOTH gauges and the red aircraft seem vertically compressed by
//! approximately (4/3)/(1920/1200) = 5/6 in the wider mode.
//!
//! This opt-in, *gameplay-only* experiment scales ONLY the Y component of
//! previously projected vertices around the vertical screen center:
//!     y' = H/2 + (y-H/2) * [3W/(4H)]
//! It uses an independent temporary vertex buffer PER drawing call; the
//! game's vertex allocation and x/z/rhw/texture/color are untouched.
//! Does not change any on-disk game code, display resolution, camera FOV
//! variables or COM object ABI. It is not a proved graphics correction.
//! The effective camera vertical field of view may change; this is
//! precisely what must be evaluated visually.
//
//! Guard rails: default OFF; strict FVF 0x1c4; only known DrawPrimitive
//! triangle fan (6), or DrawIndexedPrimitive triangle list (4);
//! current native-3D *gameplay* epoch must match the authenticated
//! widescreen patch resolution; menu/movie 800x600 and true 4:3 are
//! excluded; known old HUD probes mutually exclusive. Malformed or huge
//! vertex arrays fall back unchanged. Works on Wine/Proton and Windows.
use std::{
    ffi::c_void,
    mem::{size_of,MaybeUninit},
    ptr,
    sync::atomic::{AtomicUsize,Ordering},
};

const EXPECTED_FVF:u32=0x0000_01c4;
const FVF_VERTEX_STRIDE:usize=32; // X,Y,Z,RHW; DWORD colors x2; TEX1 (u,v)
const MAX_VERTICES:u32=2048;
const MAX_BUFFER_BYTES:usize=MAX_VERTICES as usize*FVF_VERTEX_STRIDE;
const MEM_COMMIT:u32=0x1000;
const PAGE_GUARD:u32=0x100;
const TRACE_FIRST:usize=6;
static PATCHED_PRIMITIVES:AtomicUsize=AtomicUsize::new(0);
static PATCHED_INDEXED:AtomicUsize=AtomicUsize::new(0);
static REJECTS:AtomicUsize=AtomicUsize::new(0);

#[repr(C)]
struct MemoryBasicInformation{
    base_address:*mut c_void,
    allocation_base:*mut c_void,
    allocation_protect:u32,
    region_size:usize,
    state:u32,
    protect:u32,
    kind:u32,
}

#[link(name="kernel32")]
extern "system" {
    fn VirtualQuery(address:*const c_void,buffer:*mut MemoryBasicInformation,bytes:usize)->usize;
}

fn factor_for_mode(w:u32,h:u32)->Option<f32>{
    if w<960 || h<600 || w>8192 || h>4320 ||
       (w as u64)*(h as u64)>33_554_432 ||
       (w as u64)*3<=(h as u64)*4 || (w as u64)>4*(h as u64){
        return None;
    }
    let f=3.0*w as f32/(4.0*h as f32);
    (f.is_finite() && (1.0001..=3.0).contains(&f)).then_some(f)
}

fn y_at_screen_center(y:f32,h:u32,factor:f32)->Option<f32>{
    if !y.is_finite() || y.abs()>1_000_000.0 || !factor.is_finite(){
        return None;
    }
    let center=h as f32*0.5;
    let value=center+(y-center)*factor;
    (value.is_finite() && value.abs()<=2_000_000.0).then_some(value)
}

fn eligible(fvf:u32,primitive:u32,indexed:bool)->bool{
    fvf==EXPECTED_FVF &&
    ((indexed && primitive==4) || (!indexed && primitive==6))
}

unsafe fn readable_region(ptr:*const c_void,size:usize)->bool{
    if ptr.is_null() || size==0 || size>MAX_BUFFER_BYTES{return false;}
    let start=ptr as usize;
    let Some(end)=start.checked_add(size)else{return false;};
    let mut info=MaybeUninit::<MemoryBasicInformation>::zeroed();
    if VirtualQuery(ptr,info.as_mut_ptr(),size_of::<MemoryBasicInformation>())
       !=size_of::<MemoryBasicInformation>(){return false;}
    let info=info.assume_init();
    if info.state!=MEM_COMMIT || info.protect&PAGE_GUARD!=0{return false;}
    // Only the Win32 pages in which memory is actually readable.
    if !matches!(info.protect&0xff,0x02|0x04|0x08|0x20|0x40|0x80){
        return false;
    }
    let base=info.base_address as usize;
    let Some(region_end)=base.checked_add(info.region_size)else{return false;};
    base<=start && end<=region_end
}

fn enabled_for_gameplay()->Option<((u32,u32),f32)>{
    let c=crate::cfg();
    if !c.experimental_projected_y43_probe || !c.log_enabled ||
       c.force_4_by_3 || !c.native_3d_widescreen ||
       c.experimental_hud_fov43_probe ||
       c.experimental_hud_entity_y43_probe ||
       c.experimental_hud_canvas43_probe {
        return None;
    }
    let mode=crate::in_game_adapter::active_verified_widescreen_gameplay_raster()?;
    let factor=factor_for_mode(mode.0,mode.1)?;
    Some((mode,factor))
}

/// Return None to forward caller data unchanged. The scratch buffer MUST
/// remain live until original IDirect3DDevice3::Draw* returns.
pub(super) unsafe fn shadow_y_corrected(
    primitive:u32,fvf:u32,vertices:*const c_void,count:u32,indexed:bool
)->Option<Vec<u8>>{
    if !eligible(fvf,primitive,indexed) || count<3 || count>MAX_VERTICES{
        return None;
    }
    let (mode,factor)=enabled_for_gameplay()?;
    let size=(count as usize).checked_mul(FVF_VERTEX_STRIDE)?;
    if !readable_region(vertices,size){return None;}
    let mut shadow=Vec::new();
    if shadow.try_reserve_exact(size).is_err(){return None;}
    shadow.resize(size,0);
    ptr::copy_nonoverlapping(vertices.cast::<u8>(),shadow.as_mut_ptr(),size);
    let mut first:Option<(f32,f32)>=None;
    for i in 0..count as usize {
        let yptr=shadow.as_mut_ptr().add(i*FVF_VERTEX_STRIDE+4).cast::<f32>();
        let source=ptr::read_unaligned(yptr);
        let Some(target)=y_at_screen_center(source,mode.1,factor)else{
            let n=REJECTS.fetch_add(1,Ordering::Relaxed)+1;
            if n<=3 {
                crate::log("PROJECTED-Y43: rejected nonfinite/out-of-bounds projected Y; original draw forwarded");
            }
            return None;
        };
        ptr::write_unaligned(yptr,target);
        if first.is_none(){first=Some((source,target));}
    }
    let number=if indexed{
        PATCHED_INDEXED.fetch_add(1,Ordering::Relaxed)+1
    }else{
        PATCHED_PRIMITIVES.fetch_add(1,Ordering::Relaxed)+1
    };
    if number<=TRACE_FIRST || number%8192==0 {
        let (before,after)=first.unwrap_or((0.0,0.0));
        crate::log(&format!(
            "PROJECTED-Y43: enabled=1 draw={} call={} mode={}x{} factor={factor:.6} \
             fvf={fvf:#010x} vertices={count} sample_y={before:.3} corrected_y={after:.3} \
             x_z_rhw_color_uv_unchanged=true source_vertices_unchanged=true experiment_only=true",
            if indexed{"indexed-triangle-list"}else{"quad-fan"},
            number,mode.0,mode.1
        ));
    }
    Some(shadow)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn projected_geometry_preserves_4by3_metric_for_both_world_and_hud(){
        let f=factor_for_mode(1920,1200).unwrap();
        assert!((f-1.2).abs()<0.0001);
        assert_eq!(y_at_screen_center(600.0,1200,f),Some(600.0));
        assert!((y_at_screen_center(700.0,1200,f).unwrap()-720.0).abs()<0.001);
        assert!((y_at_screen_center(500.0,1200,f).unwrap()-480.0).abs()<0.001);
        let wide=factor_for_mode(1920,1080).unwrap();
        assert!((wide-4.0/3.0).abs()<0.0001);
        assert!((y_at_screen_center(640.0,1080,wide).unwrap()-673.3333).abs()<0.001);
        assert_eq!(factor_for_mode(1600,1200),None);
        assert_eq!(factor_for_mode(800,600),None);
        assert_eq!(factor_for_mode(0,1080),None);
        assert!(y_at_screen_center(f32::NAN,1080,wide).is_none());
    }
    #[test]
    fn only_exact_pretransformed_fvf_and_expected_primitive_are_eligible(){
        assert!(eligible(0x1c4,6,false));
        assert!(eligible(0x1c4,4,true));
        for kind in [0x1c2,0x104,0x1e4,0x1c0]{
            assert!(!eligible(kind,6,false));
            assert!(!eligible(kind,4,true));
        }
        assert!(!eligible(0x1c4,6,true));
        assert!(!eligible(0x1c4,4,false));
        assert_eq!(FVF_VERTEX_STRIDE,32);
    }
    #[test]
    fn win32_memory_basic_info_structure_matches_pe32_runtime(){
        #[cfg(target_pointer_width="32")]
        assert_eq!(size_of::<MemoryBasicInformation>(),28);
    }
}
