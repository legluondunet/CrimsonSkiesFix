//! Original Crimson Skies UI artwork as a smooth Wine HCURSOR (A/B).
//!
//! The user's verified 1.02 game draws the gold pointer using one
//! D3D3 TL triangle-fan quad matching the GOS cursor hotspot. Read the
//! CURRENT GAME TEXTURE, not screenshots, a replacement .cur file or an
//! asset bundled with the fix. QI IDirect3DTexture2 -> Surface4 is a
//! documented DirectDraw 6 surface interface identity (Wine implements it).
//!
//! If capture/format/alpha/Win32 cursor creation cannot be verified, keep
//! the *original decorated D3D draw visible*. The standard Windows arrow
//! is explicitly NOT an acceptable fallback for this mode. The original
//! quad is skipped only AFTER the same artwork is installed as an HCURSOR.
//! Windows native and gameplay are never affected by this experiment.
use std::{ffi::c_void,ptr,sync::{Mutex,OnceLock,atomic::{AtomicBool,AtomicUsize,Ordering}}};
use crate::{Guid,cfg,diagnostics,log,window_erase_guard};

type Obj=*mut c_void;
type Hr=i32;
const IID_SURFACE4:Guid=Guid{
    d1:0x0b2b8630,d2:0xad35,d3:0x11d0,
    d4:[0x8e,0xa6,0x00,0x60,0x97,0x97,0xea,0x5b]
};
const DDPF_RGB:u32=0x40;
const DDCKEY_SRCBLT:u32=0x8;
const DDLOCK_WAIT:u32=0x1;
const DDLOCK_READONLY:u32=0x10;
const BI_RGB:u32=0;
const DIB_RGB_COLORS:u32=0;
#[repr(C)]
struct BitmapInfoHeader{
    size:u32,width:i32,height:i32,planes:u16,bits:u16,
    compression:u32,image_size:u32,xppm:i32,yppm:i32,used:u32,important:u32
}
#[repr(C)]
struct IconInfo{
    is_icon:i32,hot_x:u32,hot_y:u32,mask:Obj,color:Obj
}
#[derive(Clone,Copy)]
struct Vertex{x:f32,y:f32,u:f32,v:f32}
struct Candidate{
    texture:usize,vertices:[Vertex;4],mouse:(i32,i32)
}
static CAPTURE:OnceLock<Mutex<Option<Candidate>>>=OnceLock::new();
static HAVE_SAMPLE:AtomicBool=AtomicBool::new(false);
static INSTALLED:AtomicUsize=AtomicUsize::new(0);
static MENU_VERIFIED:AtomicBool=AtomicBool::new(false);
static REPORTED:AtomicBool=AtomicBool::new(false);
fn pending()->&'static Mutex<Option<Candidate>>{
    CAPTURE.get_or_init(||Mutex::new(None))
}
#[link(name="user32")]
extern "system"{
    fn CreateIconIndirect(info:*const IconInfo)->Obj;
}
#[link(name="gdi32")]
extern "system"{
    fn CreateDIBSection(dc:Obj,info:*const BitmapInfoHeader,usage:u32,
        bits:*mut Obj,section:Obj,offset:u32)->Obj;
    fn CreateBitmap(width:i32,height:i32,planes:u32,bits:u32,data:*const c_void)->Obj;
    fn DeleteObject(object:Obj)->i32;
}
unsafe fn method(obj:Obj,slot:usize)->usize{
    let vt=*(obj as *const *const usize);
    *vt.add(slot)
}
unsafe fn com_addref(obj:Obj){
    let f:unsafe extern "system" fn(Obj)->u32=std::mem::transmute(method(obj,1));
    f(obj);
}
unsafe fn com_release(obj:Obj){
    let f:unsafe extern "system" fn(Obj)->u32=std::mem::transmute(method(obj,2));
    f(obj);
}
fn enabled()->bool{
    let c=cfg();
    diagnostics::is_wine_runtime() && c.wine_decorated_system_cursor &&
    c.wine_system_cursor_probe && c.wine_system_cursor_sync &&
    c.wine_gos_draw_trace && c.log_enabled &&
    !c.wine_game_cursor_render_suppress &&
    !c.wine_d3d_cursor_suppress &&
    !c.wine_menu_cursor_bitmap_probe &&
    c.native_dd4_bootstrap && !c.directdraw_passthrough
}
pub fn set_verified_menu_mode(yes:bool){
    MENU_VERIFIED.store(yes,Ordering::Release);
}
/// A cursor only belongs to an authenticated MainMenu or Preferences
/// script; being in the DD4 800x600 graphics mode is NOT sufficient.
fn cursor_menu_evidence(script_active:bool,campaign_presented:bool)->bool{
    script_active || campaign_presented
}
fn script_allows_decorated_cursor()->bool{
    cursor_menu_evidence(
        crate::menu_scene::main_menu_active() ||
        crate::menu_scene::preferences_active(),
        window_erase_guard::wine_campaign_menu_presented()
    )
}
/// An authenticated pixel sample from the REAL 4:3 menu was successfully
/// presented in the actual game HWND. Unlike script callbacks, this also
/// fires on post-gameplay return, as recorded by MENU-BRANDING reentry=...
/// No activation is possible before the original golden icon was captured.
pub fn on_verified_campaign_presented(){
    if !enabled() || !MENU_VERIFIED.load(Ordering::Acquire){return;}
    if window_erase_guard::note_verified_wine_campaign_presented(){
        log("WINE-DECORATED-CURSOR: presented Campaign artwork authenticated menu return even without MainMenu script reload");
        on_menu_script_change(true);
    }
}
fn can_rearm(verified:bool,script_active:bool,icon_ready:bool)->bool{
    verified && script_active && icon_ready
}
/// The script change and the WndProc owner are the SAME authenticated UI
/// thread. If we already captured the original artwork, show it immediately
/// on re-entry, without waiting another 120 completed menu Flips. Never
/// create a new icon or inspect graphics in the script loader itself.
pub fn on_menu_script_change(active:bool){
    if !active || !enabled() {return;}
    let icon=INSTALLED.load(Ordering::Acquire);
    if !can_rearm(MENU_VERIFIED.load(Ordering::Acquire),active,icon!=0) ||
       window_erase_guard::decorated_menu_cursor_active() {return;}
    match window_erase_guard::activate_decorated_menu_cursor(icon){
        Ok(())=>{
            REPORTED.store(false,Ordering::Release);
            log("WINE-DECORATED-CURSOR: rearmed existing original-art HCURSOR immediately on verified menu script entry");
        },
        Err(reason)=>{
            // Focus can legitimately be elsewhere while the menu script
            // loads. Reattempt on a later successfully presented Flip.
            if !REPORTED.swap(true,Ordering::AcqRel){
                log(&format!("WINE-DECORATED-CURSOR: scene rearm deferred ({reason})"));
            }
        },
    }
}
pub fn original_art_hcursor_ready()->bool{
    enabled() && MENU_VERIFIED.load(Ordering::Acquire) &&
    INSTALLED.load(Ordering::Acquire)!=0 &&
    window_erase_guard::decorated_menu_cursor_active()
}
fn sprite_match(vertices:&[Vertex;4],mouse:(i32,i32))->bool{
    let left=vertices.iter().map(|v|v.x).fold(f32::INFINITY,f32::min);
    let top=vertices.iter().map(|v|v.y).fold(f32::INFINITY,f32::min);
    let right=vertices.iter().map(|v|v.x).fold(f32::NEG_INFINITY,f32::max);
    let bottom=vertices.iter().map(|v|v.y).fold(f32::NEG_INFINITY,f32::max);
    let dx=mouse.0 as f32-left;
    let dy=mouse.1 as f32-top;
    (1.0..=3.0).contains(&dx) && (1.0..=3.0).contains(&dy) &&
    (8.0..=96.0).contains(&(right-left)) &&
    (8.0..=96.0).contains(&(bottom-top)) &&
    vertices.iter().all(|v|
        v.x.is_finite() && v.y.is_finite() &&
        v.u.is_finite() && v.v.is_finite() &&
        (-0.01..=1.01).contains(&v.u) &&
        (-0.01..=1.01).contains(&v.v)
    )
}
/// Called from an authenticated D3D3 DrawPrimitive call only. Retain
/// exactly ONE COM reference and four vertex values; NEVER lock GPU
/// resources or call GDI from inside the graphics submission.
pub unsafe fn observe_draw(texture:usize,vertices:Obj,mouse:(i32,i32)){
    // A verified DD4 800x600 graph may also be a movie or a transition.
    // Never capture an arbitrary sprite until the exact owner game script
    // identifies MainMenu or Preferences. The admitted game's successful
    // original-art capture occurs after MainMenu script entry.
    if !enabled() || !MENU_VERIFIED.load(Ordering::Acquire) ||
       !script_allows_decorated_cursor() ||
       HAVE_SAMPLE.load(Ordering::Acquire) || texture==0 ||
       vertices.is_null(){return;}
    let p=vertices as *const f32;
    let mut v=[Vertex{x:0.,y:0.,u:0.,v:0.};4];
    for (i,slot) in v.iter_mut().enumerate(){
        let base=p.add(i*8);
        *slot=Vertex{
            x:base.read_unaligned(),y:base.add(1).read_unaligned(),
            u:base.add(6).read_unaligned(),v:base.add(7).read_unaligned()
        };
    }
    if !sprite_match(&v,mouse){return;}
    if let Ok(mut slot)=pending().try_lock(){
        if slot.is_none() && !HAVE_SAMPLE.load(Ordering::Acquire){
            com_addref(texture as Obj);
            *slot=Some(Candidate{texture,vertices:v,mouse});
            HAVE_SAMPLE.store(true,Ordering::Release);
        }
    }
}
unsafe fn surface4_from_texture(texture:Obj)->Result<Obj,String>{
    let query:unsafe extern "system" fn(Obj,*const Guid,*mut Obj)->Hr=
        std::mem::transmute(method(texture,0));
    let mut surface:Obj=ptr::null_mut();
    let hr=query(texture,&IID_SURFACE4,&mut surface);
    if hr<0 || surface.is_null(){
        return Err(format!("texture QI(IDirectDrawSurface4) failed hr=0x{:08X}",hr as u32));
    }
    Ok(surface)
}
unsafe fn get_surface_desc(surface:Obj)->Result<[u32;31],String>{
    let f:unsafe extern "system" fn(Obj,*mut u32)->Hr=
        std::mem::transmute(method(surface,22));
    let mut desc=[0u32;31];desc[0]=124;
    let hr=f(surface,desc.as_mut_ptr());
    if hr<0 || desc[0]!=124{
        return Err(format!("texture GetSurfaceDesc failed hr=0x{:08X}",hr as u32));
    }
    Ok(desc)
}
fn extract_channel(pixel:u32,mask:u32)->u8{
    if mask==0{return 0;}
    let shift=mask.trailing_zeros();
    let max=mask>>shift;
    if max==0{return 0;}
    ((((pixel&mask)>>shift) as u64*255+max as u64/2)/max as u64) as u8
}
fn sample_position(a:f32,b:f32,t:f32,limit:u32)->usize{
    ((a+(b-a)*t).clamp(0.0,0.999999)*limit as f32) as usize
}
fn decode_pixels(
    src:*const u8,pitch:usize,w:u32,h:u32,desc:&[u32;31],
    key:Option<(u32,u32)>,uv:(f32,f32,f32,f32),
    out_w:usize,out_h:usize
)->Result<(Vec<u32>,usize,usize),String>{
    if src.is_null() || out_w<8 || out_h<8 || out_w>256 || out_h>256{
        return Err("invalid output pixel buffer bounds".into());
    }
    let bpp=desc[21];let masks=(desc[22],desc[23],desc[24],desc[25]);
    if !(bpp==16 || bpp==32) || desc[19]&DDPF_RGB==0 ||
       masks.0==0 || masks.1==0 || masks.2==0 ||
       (masks.0|masks.1|masks.2|masks.3).count_ones()>bpp {
        return Err(format!(
            "unsupported true-color cursor texture bpp={bpp} flags=0x{:X} masks={masks:?}",desc[19]
        ));
    }
    if masks.3==0 && key.is_none(){
        return Err("texture has neither alpha pixels nor source color key: preserving original GameOS sprite".into());
    }
    let bytes=(bpp/8) as usize;
    if pitch<(w as usize)*bytes || w==0 || h==0 || w>4096 || h>4096{
        return Err("texture pitch/dimensions not readable".into());
    }
    let (u0,v0,u1,v1)=uv;
    let mut rgba=vec![0u32;out_w*out_h];
    let (mut transparent,mut visible)=(0usize,0usize);
    for y in 0..out_h{
        let ty=sample_position(v0,v1,(y as f32+0.5)/out_h as f32,h);
        let row=unsafe{src.add(ty*pitch)};
        for x in 0..out_w{
            let tx=sample_position(u0,u1,(x as f32+0.5)/out_w as f32,w);
            let ptr=unsafe{row.add(tx*bytes)};
            let pixel=unsafe {
                if bpp==16 {ptr::read_unaligned(ptr as *const u16) as u32}
                else{ptr::read_unaligned(ptr as *const u32)}
            };
            let a=if masks.3!=0 {extract_channel(pixel,masks.3)}
                else if key.is_some_and(|(lo,hi)| pixel>=lo && pixel<=hi){0}
                else{255};
            let red=extract_channel(pixel,masks.0) as u32;
            let green=extract_channel(pixel,masks.1) as u32;
            let blue=extract_channel(pixel,masks.2) as u32;
            let aa=a as u32;
            // GDI top-down 32bpp cursor: premultiplied BGRA pixel.
            let r=(red*aa+127)/255;
            let g=(green*aa+127)/255;
            let b=(blue*aa+127)/255;
            rgba[y*out_w+x]=(aa<<24)|(r<<16)|(g<<8)|b;
            if a==0{transparent+=1;}
            if a>=64{visible+=1;}
        }
    }
    // Prevent a blank cursor, a solid rectangle or an arbitrary atlas tile
    // from hiding the decorated original. Either one fails closed.
    if transparent<out_w*out_h/16 || visible<out_w*out_h/16{
        return Err(format!("cursor alpha validation failed visible={visible} transparent={transparent}"));
    }
    Ok((rgba,visible,transparent))
}
unsafe fn cursor_from_pixels(pixels:&[u32],w:i32,h:i32,hot:(u32,u32))
    ->Result<usize,String>{
    if w<=0 || h<=0 || hot.0>=w as u32 || hot.1>=h as u32 ||
        pixels.len()!=w as usize*h as usize {
        return Err("invalid original cursor dimensions or hotspot".into());
    }
    let bmi=BitmapInfoHeader{
        size:40,width:w,height:-h,planes:1,bits:32,compression:BI_RGB,
        image_size:0,xppm:0,yppm:0,used:0,important:0
    };
    let mut mapped:Obj=ptr::null_mut();
    let color=CreateDIBSection(ptr::null_mut(),&bmi,DIB_RGB_COLORS,
        &mut mapped,ptr::null_mut(),0);
    if color.is_null() || mapped.is_null(){
        return Err("CreateDIBSection failed for game cursor".into());
    }
    ptr::copy_nonoverlapping(pixels.as_ptr(),mapped as *mut u32,pixels.len());
    // Mask pixels are all zero: Win32 alpha channel owns transparency.
    let mask_pixels=vec![0u8;(((w as usize+15)/16)*2)*h as usize];
    let mask=CreateBitmap(w,h,1,1,mask_pixels.as_ptr().cast());
    if mask.is_null(){DeleteObject(color);return Err("cursor AND mask allocation failed".into());}
    let icon=CreateIconIndirect(&IconInfo{
        is_icon:0,hot_x:hot.0,hot_y:hot.1,mask,color
    });
    DeleteObject(mask);DeleteObject(color);
    if icon.is_null(){
        return Err("CreateIconIndirect failed for authenticated game cursor".into());
    }
    Ok(icon as usize)
}
unsafe fn convert_game_texture(sample:&Candidate)->Result<(usize,String),String>{
    let surface=surface4_from_texture(sample.texture as Obj)?;
    let result=(||{
        let desc=get_surface_desc(surface)?;
        let (w,h)=(desc[3],desc[2]);
        let (view_w,view_h)=window_erase_guard::decorated_menu_viewport()
            .ok_or("verified 4:3 game window viewport unavailable")?;
        let x_min=sample.vertices.iter().map(|v|v.x).fold(f32::INFINITY,f32::min);
        let x_max=sample.vertices.iter().map(|v|v.x).fold(f32::NEG_INFINITY,f32::max);
        let y_min=sample.vertices.iter().map(|v|v.y).fold(f32::INFINITY,f32::min);
        let y_max=sample.vertices.iter().map(|v|v.y).fold(f32::NEG_INFINITY,f32::max);
        // UV sampling follows the on-screen original quad orientation.
        let left=sample.vertices.iter().min_by(|a,b|a.x.total_cmp(&b.x)).unwrap();
        let right=sample.vertices.iter().max_by(|a,b|a.x.total_cmp(&b.x)).unwrap();
        let top=sample.vertices.iter().min_by(|a,b|a.y.total_cmp(&b.y)).unwrap();
        let bottom=sample.vertices.iter().max_by(|a,b|a.y.total_cmp(&b.y)).unwrap();
        let uv=(left.u,top.v,right.u,bottom.v);
        if (uv.2-uv.0).abs()<0.005 || (uv.3-uv.1).abs()<0.005 {
            return Err(format!("unexpected degenerate cursor UV={uv:?}"));
        }
        let sx=view_w as f32/800.0;
        let sy=view_h as f32/600.0;
        let out_w=((x_max-x_min)*sx).round().clamp(8.,256.) as usize;
        let out_h=((y_max-y_min)*sy).round().clamp(8.,256.) as usize;
        let hotspot=(
            ((sample.mouse.0 as f32-x_min)*sx).round().max(0.) as u32,
            ((sample.mouse.1 as f32-y_min)*sy).round().max(0.) as u32
        );
        if hotspot.0>=out_w as u32 || hotspot.1>=out_h as u32{
            return Err(format!("game cursor hotspot outside sprite: {hotspot:?}/{out_w}x{out_h}"));
        }
        // Read the texture after the completed menu Flip, not from the
        // D3D draw callback while the GPU holds scene resources.
        let mut locked=[0u32;31];locked[0]=124;
        let lock:unsafe extern "system" fn(Obj,Obj,*mut u32,u32,Obj)->Hr=
            std::mem::transmute(method(surface,25));
        let unlock:unsafe extern "system" fn(Obj,Obj)->Hr=
            std::mem::transmute(method(surface,32));
        let mut hr=lock(surface,ptr::null_mut(),locked.as_mut_ptr(),
            DDLOCK_WAIT|DDLOCK_READONLY,ptr::null_mut());
        if hr<0{
            locked=[0;31];locked[0]=124;
            hr=lock(surface,ptr::null_mut(),locked.as_mut_ptr(),
                DDLOCK_WAIT,ptr::null_mut());
        }
        if hr<0 {
            return Err(format!(
                "game cursor texture Lock failed hr=0x{:08X}, {}x{} bpp={} flags=0x{:X} uv={uv:?}",
                hr as u32,w,h,desc[21],desc[19]
            ));
        }
        let color_key={
            let get:unsafe extern "system" fn(Obj,u32,*mut u32)->Hr=
                std::mem::transmute(method(surface,16));
            let mut keys=[0u32;2];
            if get(surface,DDCKEY_SRCBLT,keys.as_mut_ptr())>=0{
                Some((keys[0],keys[1]))
            }else{None}
        };
        let pitch=locked[4] as i32;
        let pixels=if pitch>0{
            decode_pixels(locked[9] as usize as *const u8,pitch as usize,
                w,h,&desc,color_key,uv,out_w,out_h)
        }else{Err(format!("negative/zero texture pitch {pitch} unsupported"))};
        let _=unlock(surface,ptr::null_mut());
        let (pixels,visible,transparent)=pixels?;
        let icon=cursor_from_pixels(&pixels,out_w as i32,out_h as i32,hotspot)?;
        Ok((icon,format!(
            "original texture QI/Lock OK {}x{} bpp={} flags=0x{:X} masks=({:X},{:X},{:X},{:X}) key={color_key:?} uv={uv:?} logical=({:.1}x{:.1}) native={}x{} hotspot={hotspot:?} opaque_pixels={visible} transparent_pixels={transparent}",
            w,h,desc[21],desc[19],desc[22],desc[23],desc[24],desc[25],
            x_max-x_min,y_max-y_min,out_w,out_h
        )))
    })();
    com_release(surface);
    result
}
/// Runs after a completed virtual menu Flip, on the verified game UI
/// thread; attempts at most once per process. Failure leaves original
/// texture drawing and the game's normal cursor visibility unchanged.
pub fn try_install(){
    if !enabled() || !MENU_VERIFIED.load(Ordering::Acquire) ||
       !script_allows_decorated_cursor(){return;}
    let icon=INSTALLED.load(Ordering::Acquire);
    if icon!=0 {
        // Re-arm on a real return to MainMenu/Preferences, never in
        // arbitrary 800x600 movies or other scripts. The script observer
        // attempts immediately; this is a later focus-return fallback.
        if !window_erase_guard::decorated_menu_cursor_active(){
            match window_erase_guard::activate_decorated_menu_cursor(icon){
                Ok(())=>{REPORTED.store(false,Ordering::Release);},
                Err(e)=>{
                    if !REPORTED.swap(true,Ordering::AcqRel){
                        log(&format!("WINE-DECORATED-CURSOR: rearm deferred ({e})"));
                    }
                },
            }
        }
        return;
    }
    let sample=pending().lock().ok().and_then(|mut g|g.take());
    let Some(sample)=sample else{return;};
    let result=unsafe{convert_game_texture(&sample)};
    unsafe{com_release(sample.texture as Obj);}
    match result{
        Ok((icon,details))=>{
            match window_erase_guard::activate_decorated_menu_cursor(icon){
                Ok(())=>{
                    INSTALLED.store(icon,Ordering::Release);
                    log(&format!(
                        "WINE-DECORATED-CURSOR: ACTIVE using ONLY the user's original cursor texture as HCURSOR; {details}; original GOS D3D quad now suppressed"
                    ));
                }
                Err(e)=>{
                    log(&format!("WINE-DECORATED-CURSOR: refused to activate ({e}); original game cursor remains, no generic arrow"));
                    // The unreferenced HCURSOR is deliberately retained until
                    // process exit: ShowCursor/WM_SETCURSOR may still race.
                }
            }
        }
        Err(e)=>log(&format!(
            "WINE-DECORATED-CURSOR: capture unavailable ({e}); original game pointer retained; no generic arrow or suppression"
        ))
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn script_rearm_is_scoped_to_original_menu_and_existing_icon(){
        assert!(cursor_menu_evidence(true,false));
        assert!(cursor_menu_evidence(false,true));
        assert!(!cursor_menu_evidence(false,false));
        assert!(can_rearm(true,true,true));
        assert!(!can_rearm(true,false,true));
        assert!(!can_rearm(false,true,true));
        assert!(!can_rearm(true,true,false));
    }
    #[test]
    fn original_cursor_geometry_and_pixel_decoding(){
        let v=[
            Vertex{x:553.5,y:386.5,u:0.,v:0.},
            Vertex{x:585.5,y:386.5,u:1.,v:0.},
            Vertex{x:553.5,y:418.5,u:0.,v:1.},
            Vertex{x:585.5,y:418.5,u:1.,v:1.},
        ];
        assert!(sprite_match(&v,(556,389)));
        assert!(!sprite_match(&v,(700,389)));
        assert_eq!(extract_channel(0xf800,0xf800),255);
        assert_eq!(extract_channel(0x07e0,0x07e0),255);
        assert_eq!(extract_channel(0x0000,0xf800),0);
        assert_eq!(sample_position(0.25,0.75,0.5,128),64);
    }
    #[test]
    fn win32_cursor_bitmap_headers(){
        assert_eq!(std::mem::size_of::<BitmapInfoHeader>(),40);
        assert_eq!(std::mem::size_of::<IconInfo>(),20);
        assert_eq!(std::mem::size_of::<Guid>(),16);
    }
}
