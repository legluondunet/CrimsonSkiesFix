//! Direct3DDevice3 draw correlation and an independent, Wine-only cursor
//! quad suppression A/B under the existing WineGosDrawTrace experiment.
//!
//! Actual 3adbe981 Wine log: 7,935 DrawPrimitive calls in 360 menu Flips.
//! Among 216 detailed samples, one 4-vertex, FVF 0x1C4, TRIANGLEFAN=6
//! follows 77 distinct GameOS cursor positions with exactly 1.5/2.5 px
//! XY hotspot offsets. No other sampled texture exhibits that signature.
//! Experimental suppression is OFF unless WineD3dCursorSuppress=1, AND all
//! original Wine pointer+sync diagnostics are enabled in verified 800x600
//! MainMenu/Preferences. This is a render-only no-op for that one quad; input,
//! buttons, FPS/presentation, OS arrow and all other draw calls remain intact.
//! Native Windows never uses this experimental hook.
use std::{ffi::c_void,sync::{Mutex,OnceLock,atomic::{AtomicUsize,Ordering}}};
use crate::{cfg,diagnostics,log};
use crate::com_vtable::{orig,replace_method};

const DRAW_PRIMITIVE:usize=28;
const DRAW_INDEXED:usize=29;
const DRAW_PRIMITIVE_VB:usize=34;
const DRAW_INDEXED_VB:usize=35;
const SET_TEXTURE:usize=38;
const MOUSE_X:usize=0x0075_ccc0;
const MOUSE_Y:usize=0x0075_ccc4;
type Hr=i32;
type Obj=*mut c_void;
type Draw=unsafe extern "system" fn(Obj,u32,u32,Obj,u32,u32)->Hr;
type DrawIndexed=unsafe extern "system" fn(Obj,u32,u32,Obj,u32,*const u16,u32,u32)->Hr;
type DrawVB=unsafe extern "system" fn(Obj,u32,Obj,u32,u32,u32)->Hr;
type DrawIndexedVB=unsafe extern "system" fn(Obj,u32,Obj,*const u16,u32,u32)->Hr;
type SetTexture=unsafe extern "system" fn(Obj,u32,Obj)->Hr;
static COUNTS:[AtomicUsize;4]=[const{AtomicUsize::new(0)};4];
static SAMPLED_NEAR:AtomicUsize=AtomicUsize::new(0);
static CURSOR_DRAW_SUPPRESSED:AtomicUsize=AtomicUsize::new(0);
static CURSOR_DRAW_LAST_TEXTURE:AtomicUsize=AtomicUsize::new(0);
static TEXTURE:AtomicUsize=AtomicUsize::new(0);
static SAMPLES:OnceLock<Mutex<Vec<String>>>=OnceLock::new();

fn samples()->&'static Mutex<Vec<String>>{
    SAMPLES.get_or_init(||Mutex::new(Vec::new()))
}
fn enabled()->bool{
    let c=cfg();
    diagnostics::is_wine_runtime() && c.wine_system_cursor_probe &&
    c.wine_system_cursor_sync && c.wine_gos_draw_trace && c.log_enabled &&
    !c.wine_game_cursor_render_suppress &&
    !c.wine_menu_cursor_bitmap_probe &&
    c.native_dd4_bootstrap && !c.directdraw_passthrough
}
fn active()->bool{
    enabled() && crate::wine_gos_draw_trace::verified_menu_active()
}
// Match ONLY the bitmap-bearing TL triangle-fan quad whose first vertex
// tracks the original GOS hit-testing cursor's hotspot, as demonstrated
// at 77 distinct positions in the user's complete 3adbe981 log.
// Do not use the DX6 texture COM pointer as an identity: it changes per run,
// and the same atlas may also draw non-cursor UI regions.
fn cursor_quad_matches(
    primitive:u32,fvf:u32,vertex_count:u32,
    texture:usize,first:Option<(f32,f32)>,mouse:(i32,i32)
)->bool{
    if primitive!=6 || fvf!=0x1C4 || vertex_count!=4 || texture==0 ||
        !(0..800).contains(&mouse.0) || !(0..600).contains(&mouse.1) {
        return false;
    }
    let Some((x,y))=first else {return false;};
    if !x.is_finite() || !y.is_finite(){return false;}
    let dx=mouse.0 as f32-x;
    let dy=mouse.1 as f32-y;
    (1.0..=3.0).contains(&dx) && (1.0..=3.0).contains(&dy)
}
fn should_suppress_cursor_quad(
    wine:bool,menu_verified:bool,experimental:bool,
    primitive:u32,fvf:u32,vertex_count:u32,
    texture:usize,first:Option<(f32,f32)>,mouse:(i32,i32)
)->bool{
    wine && menu_verified && experimental &&
    cursor_quad_matches(primitive,fvf,vertex_count,texture,first,mouse)
}
fn should_record(n:usize,near:bool)->bool{
    n<=5 || (near && n<=20000) || n==64 || n==512
}
fn record(kind:usize,n:usize,primitive:u32,fvf:u32,vertex:*const c_void,vertices:u32){
    if !active(){return;}
    // Read the same live GOS cursor position used by the verified
    // DirectInput synchronization, and no additional game object memory.
    let mouse=unsafe {
        ((MOUSE_X as *const i32).read_volatile(),
         (MOUSE_Y as *const i32).read_volatile())
    };
    // Only inspect one supplied vertex if the game's documented layout
    // clearly contains screen-transformed X/Y floats. No guessed stride.
    // D3DVERTEXTYPE_TLVERTEX=3 and D3DFVF_XYZRHW=0x4.
    let tl=fvf==3 || (fvf & 0x000e)==0x0004;
    let first=if tl && !vertex.is_null() && (1..=512).contains(&vertices){
        unsafe {
            let p=vertex as *const f32;
            let x=p.read_unaligned();
            let y=p.add(1).read_unaligned();
            if x.is_finite() && y.is_finite() && x.abs()<100000.0 && y.abs()<100000.0 {
                Some((x,y))
            }else{None}
        }
    }else{None};
    let near=first.is_some_and(|(x,y)|
        (x-mouse.0 as f32).abs()<=64.0 &&
        (y-mouse.1 as f32).abs()<=64.0
    );
    if near{SAMPLED_NEAR.fetch_add(1,Ordering::Relaxed);}
    if !should_record(n,near){return;}
    if let Ok(mut v)=samples().lock(){
        if v.len()>=72{return;}
        v.push(format!(
            "WINE-D3D3-DRAW-TRACE: kind={kind} call={n} primitive={primitive} fvf=0x{fvf:X} vertices={vertices} texture_stage0=0x{:08X} first_xy={first:?} gos_mouse={mouse:?} near_mouse={near} [original draw unmodified]",
            TEXTURE.load(Ordering::Relaxed)
        ));
    }
}
unsafe extern "system" fn draw(
    this:Obj,primitive:u32,fvf:u32,vertices:Obj,count:u32,flags:u32
)->Hr{
    let menu_active=active();
    let n=if menu_active{COUNTS[0].fetch_add(1,Ordering::Relaxed)+1}else{0};
    if n>0{record(0,n,primitive,fvf,vertices,count);}
    let suppress=if menu_active && cfg().wine_d3d_cursor_suppress &&
        primitive==6 && fvf==0x1C4 && count==4 &&
        !vertices.is_null() {
        // The original renderer already expects four valid TL vertices,
        // so reading the initial x/y is safe for this exact admitted call.
        let p=vertices as *const f32;
        let first=Some((p.read_unaligned(),p.add(1).read_unaligned()));
        let mouse=((MOUSE_X as *const i32).read_volatile(),
                   (MOUSE_Y as *const i32).read_volatile());
        should_suppress_cursor_quad(
            diagnostics::is_wine_runtime(),
            crate::wine_gos_draw_trace::verified_menu_active(),
            cfg().wine_d3d_cursor_suppress,
            primitive,fvf,count,
            TEXTURE.load(Ordering::Relaxed),first,mouse
        )
    }else{false};
    if suppress{
        let texture=TEXTURE.load(Ordering::Relaxed);
        CURSOR_DRAW_LAST_TEXTURE.store(texture,Ordering::Relaxed);
        CURSOR_DRAW_SUPPRESSED.fetch_add(1,Ordering::Relaxed);
        // D3D_OK. The caller's UI/input logic already executed.
        // No Direct3D state, texture, vertex or OS cursor is changed.
        return 0;
    }
    let Some(raw)=orig(this,DRAW_PRIMITIVE) else {return -1;};
    let f:Draw=std::mem::transmute(raw);
    f(this,primitive,fvf,vertices,count,flags)
}
unsafe extern "system" fn indexed(
    this:Obj,primitive:u32,fvf:u32,vertices:Obj,count:u32,
    indices:*const u16,index_count:u32,flags:u32
)->Hr{
    let n=if active(){COUNTS[1].fetch_add(1,Ordering::Relaxed)+1}else{0};
    if n>0{record(1,n,primitive,fvf,vertices,count);}
    let Some(raw)=orig(this,DRAW_INDEXED) else{return -1;};
    let f:DrawIndexed=std::mem::transmute(raw);
    f(this,primitive,fvf,vertices,count,indices,index_count,flags)
}
unsafe extern "system" fn draw_vb(
    this:Obj,primitive:u32,vb:Obj,start:u32,count:u32,flags:u32
)->Hr{
    let n=if active(){COUNTS[2].fetch_add(1,Ordering::Relaxed)+1}else{0};
    if n>0{record(2,n,primitive,0,vb as *const c_void,count);}
    let Some(raw)=orig(this,DRAW_PRIMITIVE_VB) else{return -1;};
    let f:DrawVB=std::mem::transmute(raw);
    f(this,primitive,vb,start,count,flags)
}
unsafe extern "system" fn indexed_vb(
    this:Obj,primitive:u32,vb:Obj,indices:*const u16,count:u32,flags:u32
)->Hr{
    let n=if active(){COUNTS[3].fetch_add(1,Ordering::Relaxed)+1}else{0};
    if n>0{record(3,n,primitive,0,vb as *const c_void,count);}
    let Some(raw)=orig(this,DRAW_INDEXED_VB) else{return -1;};
    let f:DrawIndexedVB=std::mem::transmute(raw);
    f(this,primitive,vb,indices,count,flags)
}
unsafe extern "system" fn set_texture(this:Obj,stage:u32,texture:Obj)->Hr{
    let Some(raw)=orig(this,SET_TEXTURE) else{return -1;};
    let f:SetTexture=std::mem::transmute(raw);
    let hr=f(this,stage,texture);
    if hr>=0 && stage==0 && active(){
        TEXTURE.store(texture as usize,Ordering::Relaxed);
    }
    hr
}

pub unsafe fn install_for_verified_game_device(device:Obj){
    if !enabled() || device.is_null(){return;}
    let hooks=[
        (DRAW_PRIMITIVE,draw as usize),
        (DRAW_INDEXED,indexed as usize),
        (DRAW_PRIMITIVE_VB,draw_vb as usize),
        (DRAW_INDEXED_VB,indexed_vb as usize),
        (SET_TEXTURE,set_texture as usize),
    ];
    let results:Vec<bool>=hooks.iter().map(|&(slot,hook)|
        replace_method(device,slot,hook)).collect();
    log(&format!(
        "WINE-D3D3-DRAW-TRACE: device created; indexed_com_slots=[28,29,34,35,38]; hooks={results:?}; original COM methods always forwarded"
    ));
}
/// Called only once per 120 successful 800x600 menu Flips after the graph
/// lock is dropped; logging is never performed from inside a D3D draw.
pub fn report_sample(){
    if !active(){return;}
    let counters:Vec<usize>=COUNTS.iter().map(|x|x.load(Ordering::Relaxed)).collect();
    let near=SAMPLED_NEAR.load(Ordering::Relaxed);
    log(&format!(
        "WINE-D3D3-DRAW-TRACE-STATUS: primitive={} indexed={} primitive_vb={} indexed_vb={} close_to_mouse_samples={} cursor_quads_suppressed={} last_cursor_texture=0x{:08X} [cumulative; all nonmatching draws forwarded]",
        counters[0],counters[1],counters[2],counters[3],near,
        CURSOR_DRAW_SUPPRESSED.load(Ordering::Relaxed),
        CURSOR_DRAW_LAST_TEXTURE.load(Ordering::Relaxed)
    ));
    if let Ok(mut v)=samples().lock(){
        for line in v.drain(..){log(&line);}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dx6_idirect3ddevice3_slots_and_exact_win32_abi(){
        assert_eq!([DRAW_PRIMITIVE,DRAW_INDEXED,DRAW_PRIMITIVE_VB,DRAW_INDEXED_VB,SET_TEXTURE],[28,29,34,35,38]);
        let _:Draw=draw;
        let _:DrawIndexed=indexed;
        let _:DrawVB=draw_vb;
        let _:DrawIndexedVB=indexed_vb;
        let _:SetTexture=set_texture;
    }
    #[test]
    fn exact_cursor_alignment_filters_other_menu_quads(){
        let p=Some((553.5,386.5));
        assert!(cursor_quad_matches(6,0x1C4,4,0x1234,p,(556,389)));
        assert!(cursor_quad_matches(6,0x1C4,4,0x1234,Some((553.5,385.5)),(555,388)));
        // A background tile uses the same texture atlas; coordinate
        // coupling, not texture identity, is the essential safeguard.
        assert!(!cursor_quad_matches(6,0x1C4,4,0x1234,Some((645.5,140.5)),(556,389)));
        assert!(!cursor_quad_matches(6,0x1C4,4,0x1234,Some((389.5,140.5)),(556,389)));
        assert!(!cursor_quad_matches(6,0x1C4,4,0,Some((553.5,386.5)),(556,389)));
        assert!(!cursor_quad_matches(5,0x1C4,4,0x1234,p,(556,389)));
        assert!(!cursor_quad_matches(6,0x1C4,6,0x1234,p,(556,389)));
        assert!(!cursor_quad_matches(6,0x1C0,4,0x1234,p,(556,389)));
        assert!(!cursor_quad_matches(6,0x1C4,4,0x1234,None,(556,389)));
        assert!(!cursor_quad_matches(6,0x1C4,4,0x1234,p,(799,600)));
        assert!(!should_suppress_cursor_quad(false,true,true,6,0x1C4,4,0x1234,p,(556,389)));
        assert!(!should_suppress_cursor_quad(true,false,true,6,0x1C4,4,0x1234,p,(556,389)));
        assert!(!should_suppress_cursor_quad(true,true,false,6,0x1C4,4,0x1234,p,(556,389)));
        assert!(should_suppress_cursor_quad(true,true,true,6,0x1C4,4,0x1234,p,(556,389)));
    }
    #[test]
    fn bounded_samples(){
        assert!(should_record(1,false));
        assert!(!should_record(8,false));
        assert!(should_record(512,false));
        assert!(should_record(1024,true));
        assert!(!should_record(20001,true));
    }
}
