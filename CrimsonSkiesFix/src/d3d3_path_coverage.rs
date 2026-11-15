//! Experimental analysis-only Direct3D3 draw path coverage.
//!
//! The failed PROJECTED-Y43 A/B (15 November 2026) corrected the circular
//! gauges but detached aircraft submeshes and the two compass layers.
//! A single screen-space stretch of only DrawPrimitive/DrawIndexedPrimitive
//! cannot be promoted as a general fix while other drawing paths may exist.
//!
//! DX6 IDirect3DDevice3 vtable slots 32..35 are respectively
//! DrawPrimitiveStrided, DrawIndexedPrimitiveStrided, DrawPrimitiveVB,
//! DrawIndexedPrimitiveVB. These hooks FORWARD all arguments unchanged and
//! observe bounded counts only; no vertex memory is touched.
//!
//! The two previously hooked non-strided calls report one line for EACH
//! newly seen primitive type and whether the FVF is the exact currently
//! patched 0x1C4. This catches rare uncovered types hidden by sampling.
use super::*;
use std::sync::atomic::{AtomicU32,AtomicUsize,Ordering};

const D3D3_DRAW_PRIMITIVE_STRIDED:usize=32;
const D3D3_DRAW_INDEXED_PRIMITIVE_STRIDED:usize=33;
const D3D3_DRAW_PRIMITIVE_VB:usize=34;
const D3D3_DRAW_INDEXED_PRIMITIVE_VB:usize=35;
const PATCHED_FVF:u32=0x000001C4;
static PLAIN_PATTERNS:AtomicU32=AtomicU32::new(0);
static STRIDED_PRIMITIVES:AtomicUsize=AtomicUsize::new(0);
static STRIDED_INDEXED:AtomicUsize=AtomicUsize::new(0);
static VERTEX_BUFFER_PRIMITIVES:AtomicUsize=AtomicUsize::new(0);
static VERTEX_BUFFER_INDEXED:AtomicUsize=AtomicUsize::new(0);

pub(super) fn observe_plain(indexed:bool,primitive:u32,fvf:u32,vertices:u32){
    // 8 bins of primitive types * 2 FVF categories * 2 draw methods
    // fit in a 32-bit atomic. Log one line per first-observed category.
    let category=(
        (if indexed {2usize}else{0usize}) +
        (if fvf==PATCHED_FVF{0usize}else{1usize})
    )*8+(primitive.min(7) as usize);
    let bit=1u32<<category;
    if PLAIN_PATTERNS.fetch_or(bit,Ordering::Relaxed)&bit==0{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: first_observed={} primitive_type={} fvf={fvf:#010x} \
             exact_y43_fvf={} y43_old_eligible={} vertex_count={vertices} \
             observation_only=true",
            if indexed{"DrawIndexedPrimitive"}else{"DrawPrimitive"},
            primitive,
            fvf==PATCHED_FVF,
            fvf==PATCHED_FVF && (if indexed {primitive==4}else{primitive==6})
        ));
    }
}

unsafe extern "system" fn primitive_strided(
    this:*mut c_void,primitive:u32,fvf:u32,data:*const c_void,
    vertices:u32,flags:u32
)->Hr{
    let Some(p)=orig(this,D3D3_DRAW_PRIMITIVE_STRIDED)else{return DDERR_GENERIC};
    type Fn=unsafe extern "system" fn(
        *mut c_void,u32,u32,*const c_void,u32,u32)->Hr;
    let f:Fn=std::mem::transmute(p);
    let hr=f(this,primitive,fvf,data,vertices,flags);
    let n=STRIDED_PRIMITIVES.fetch_add(1,Ordering::Relaxed)+1;
    if n<=3||n%32768==0{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: method=DrawPrimitiveStrided call={n} primitive_type={primitive} fvf={fvf:#010x} vertex_count={vertices} hr={hr:#010x} forwarded_unchanged=true"
        ));
    }
    hr
}

unsafe extern "system" fn indexed_strided(
    this:*mut c_void,primitive:u32,fvf:u32,data:*const c_void,
    vertices:u32,indices:*const u16,index_count:u32,flags:u32
)->Hr{
    let Some(p)=orig(this,D3D3_DRAW_INDEXED_PRIMITIVE_STRIDED)else{return DDERR_GENERIC};
    type Fn=unsafe extern "system" fn(
        *mut c_void,u32,u32,*const c_void,u32,*const u16,u32,u32)->Hr;
    let f:Fn=std::mem::transmute(p);
    let hr=f(this,primitive,fvf,data,vertices,indices,index_count,flags);
    let n=STRIDED_INDEXED.fetch_add(1,Ordering::Relaxed)+1;
    if n<=3||n%32768==0{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: method=DrawIndexedPrimitiveStrided call={n} primitive_type={primitive} fvf={fvf:#010x} vertex_count={vertices} index_count={index_count} hr={hr:#010x} forwarded_unchanged=true"
        ));
    }
    hr
}

unsafe extern "system" fn primitive_vb(
    this:*mut c_void,primitive:u32,buffer:*mut c_void,
    start:u32,vertices:u32,flags:u32
)->Hr{
    let Some(p)=orig(this,D3D3_DRAW_PRIMITIVE_VB)else{return DDERR_GENERIC};
    type Fn=unsafe extern "system" fn(*mut c_void,u32,*mut c_void,u32,u32,u32)->Hr;
    let f:Fn=std::mem::transmute(p);
    let hr=f(this,primitive,buffer,start,vertices,flags);
    let n=VERTEX_BUFFER_PRIMITIVES.fetch_add(1,Ordering::Relaxed)+1;
    if n<=3||n%32768==0{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: method=DrawPrimitiveVB call={n} primitive_type={primitive} start={start} vertex_count={vertices} hr={hr:#010x} forwarded_unchanged=true"
        ));
    }
    hr
}

unsafe extern "system" fn indexed_vb(
    this:*mut c_void,primitive:u32,buffer:*mut c_void,
    indices:*const u16,index_count:u32,flags:u32
)->Hr{
    let Some(p)=orig(this,D3D3_DRAW_INDEXED_PRIMITIVE_VB)else{return DDERR_GENERIC};
    type Fn=unsafe extern "system" fn(*mut c_void,u32,*mut c_void,*const u16,u32,u32)->Hr;
    let f:Fn=std::mem::transmute(p);
    let hr=f(this,primitive,buffer,indices,index_count,flags);
    let n=VERTEX_BUFFER_INDEXED.fetch_add(1,Ordering::Relaxed)+1;
    if n<=3||n%32768==0{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: method=DrawIndexedPrimitiveVB call={n} primitive_type={primitive} index_count={index_count} hr={hr:#010x} forwarded_unchanged=true"
        ));
    }
    hr
}

pub(super) unsafe fn install(device:*mut c_void){
    let methods=[
        (D3D3_DRAW_PRIMITIVE_STRIDED,primitive_strided as *const () as usize),
        (D3D3_DRAW_INDEXED_PRIMITIVE_STRIDED,indexed_strided as *const () as usize),
        (D3D3_DRAW_PRIMITIVE_VB,primitive_vb as *const () as usize),
        (D3D3_DRAW_INDEXED_PRIMITIVE_VB,indexed_vb as *const () as usize),
    ];
    let mut installed=0;
    for (slot,callback) in methods{
        if replace_method(device,slot,callback){installed+=1;}
    }
    if cfg().log_enabled{
        crate::log(&format!(
            "D3D3-PATH-COVERAGE: installed_extra_pass_through={installed}/4 slots=32..35 no_vertex_or_render_changes=true"
        ));
    }
}

pub(super) fn periodic_summary(sample:usize){
    if sample!=16 && sample!=480 && sample%1920!=0{return;}
    crate::log(&format!(
        "D3D3-PATH-COVERAGE: snapshot={sample} \
         primitive_pattern_mask={:#010x} strided={} indexed_strided={} vb={} indexed_vb={} \
         all_four_extra_paths_pass_through=true",
        PLAIN_PATTERNS.load(Ordering::Relaxed),
        STRIDED_PRIMITIVES.load(Ordering::Relaxed),
        STRIDED_INDEXED.load(Ordering::Relaxed),
        VERTEX_BUFFER_PRIMITIVES.load(Ordering::Relaxed),
        VERTEX_BUFFER_INDEXED.load(Ordering::Relaxed)
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn device3_dx6_additional_draw_method_slots(){
        assert_eq!(D3D3_DRAW_PRIMITIVE_STRIDED,32);
        assert_eq!(D3D3_DRAW_INDEXED_PRIMITIVE_STRIDED,33);
        assert_eq!(D3D3_DRAW_PRIMITIVE_VB,34);
        assert_eq!(D3D3_DRAW_INDEXED_PRIMITIVE_VB,35);
        let _:unsafe extern "system" fn(*mut c_void,u32,u32,*const c_void,u32,u32)->Hr=primitive_strided;
        let _:unsafe extern "system" fn(*mut c_void,u32,u32,*const c_void,u32,*const u16,u32,u32)->Hr=indexed_strided;
        let _:unsafe extern "system" fn(*mut c_void,u32,*mut c_void,u32,u32,u32)->Hr=primitive_vb;
        let _:unsafe extern "system" fn(*mut c_void,u32,*mut c_void,*const u16,u32,u32)->Hr=indexed_vb;
    }
}
