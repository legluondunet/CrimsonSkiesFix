//! Opt-in comparison of live native XRGB8888 source vs RGB565 destination.
//!
//! Diagnostic ONLY: no writes to game pixel data, no substitute Blt,
//! no Windows APIs or owned COM references. Call immediately after a
//! successful native IDirectDrawSurface4::Blt, on that game thread.
//! Lock/Unlock of GPU resources may stall or fail under Wine; opt-in only.
//! Failure to sample is not evidence that the native conversion is correct.
#![allow(dead_code)]
use std::ptr;
use crate::com_surface4_abi::{Rect,Surface4VTable,SurfaceThis};

const DDPF_RGB:u32=0x40;
const DDLOCK_WAIT:u32=0x00000001;
const DDLOCK_READONLY:u32=0x00000010;
const SWATCHES:[(u32,u32);9]=[
    (1,1),(4,1),(7,1),(1,4),(4,4),(7,4),(1,7),(4,7),(7,7)
];

#[inline]
pub fn xrgb8888_to_rgb565(pixel:u32)->u16{
    // Explicitly ignore the unused high X/alpha byte.
    let r=(pixel>>16)&0xff;
    let g=(pixel>>8)&0xff;
    let b=pixel&0xff;
    (((r>>3)<<11)|((g>>2)<<5)|(b>>3)) as u16
}

pub fn should_probe(copy_number:usize)->bool{
    copy_number<=3 || (copy_number<=1200 && copy_number%60==0)
}

fn rect_in_bounds(rect:Rect,width:u32,height:u32)->bool{
    rect.left>=0 && rect.top>=0 &&
        rect.right>rect.left && rect.bottom>rect.top &&
        rect.right as u32<=width && rect.bottom as u32<=height
}

fn rect_or_full(rect:*mut Rect,width:u32,height:u32)->Option<Rect>{
    if width==0 || height==0 || width>8192 || height>8192{return None;}
    let entire=Rect{left:0,top:0,right:width as i32,bottom:height as i32};
    let r=if rect.is_null(){entire}else{unsafe{*rect}};
    rect_in_bounds(r,width,height).then_some(r)
}

fn sample_position(rect:Rect,frac:(u32,u32))->(u32,u32){
    let w=(rect.right-rect.left) as u32;
    let h=(rect.bottom-rect.top) as u32;
    let x=(w as u64*frac.0 as u64/8).min(w as u64-1) as u32;
    let y=(h as u64*frac.1 as u64/8).min(h as u64-1) as u32;
    (rect.left as u32+x,rect.top as u32+y)
}

fn same_extent(a:Rect,b:Rect)->bool{
    a.right-a.left==b.right-b.left && a.bottom-a.top==b.bottom-b.top
}

fn is_xrgb8888(d:&[u32;31])->bool{
    d[0]==124 && d[18]==32 && d[19]&DDPF_RGB!=0 &&
    d[21]==32 && (d[22],d[23],d[24])==(0x00ff0000,0x0000ff00,0x000000ff)
}

fn is_rgb565(d:&[u32;31])->bool{
    d[0]==124 && d[18]==32 && d[19]&DDPF_RGB!=0 &&
    d[21]==16 && (d[22],d[23],d[24])==(0xf800,0x07e0,0x001f)
}

unsafe fn vtbl(p:SurfaceThis)->&'static Surface4VTable{
    &**p.cast::<*const Surface4VTable>()
}

unsafe fn desc(p:SurfaceThis)->Result<[u32;31],String>{
    let mut d=[0u32;31];d[0]=124;
    let hr=(vtbl(p).get_surface_desc)(p,d.as_mut_ptr().cast());
    if hr<0 || d[0]!=124{
        return Err(format!("GetSurfaceDesc hr={:#010x}",hr as u32));
    }
    Ok(d)
}

/// Lock, sample 9 pixels, and ALWAYS unlock before returning.
/// Coordinates are absolute within the native 32-bit or 16-bit surface.
unsafe fn read_swatches(p:SurfaceThis,region:Rect,bytes_per_pixel:usize)
    ->Result<[u32;9],String>{
    let mut locked=[0u32;31];locked[0]=124;
    let hr=(vtbl(p).lock)(p,ptr::null_mut(),locked.as_mut_ptr().cast(),
        DDLOCK_WAIT|DDLOCK_READONLY,ptr::null_mut());
    if hr<0{return Err(format!("Lock hr={:#010x}",hr as u32));}
    let sampled=(||->Result<[u32;9],String>{
        let width=locked[3];let height=locked[2];
        if locked[0]!=124 || !rect_in_bounds(region,width,height){
            return Err("Lock invalid dimensions or descriptor".into());
        }
        let pitch=locked[4] as i32;
        if pitch<=0 || (pitch as usize)<width as usize*bytes_per_pixel{
            return Err("Lock nonpositive/short pitch".into());
        }
        let base=locked[9] as usize as *const u8;
        if base.is_null(){return Err("Lock NULL pixel pointer".into());}
        let mut pixels=[0u32;9];
        for (i,frac) in SWATCHES.iter().copied().enumerate(){
            let (x,y)=sample_position(region,frac);
            let offset=(y as usize).checked_mul(pitch as usize)
                .and_then(|row|row.checked_add(x as usize*bytes_per_pixel))
                .ok_or("Lock pixel offset overflow")?;
            let at=base.add(offset);
            pixels[i]=if bytes_per_pixel==4{
                ptr::read_unaligned(at.cast::<u32>())
            }else{
                u32::from(ptr::read_unaligned(at.cast::<u16>()))
            };
        }
        Ok(pixels)
    })();
    let unlock_hr=(vtbl(p).unlock)(p,ptr::null_mut());
    if unlock_hr<0{
        return Err(format!("Unlock hr={:#010x}",unlock_hr as u32));
    }
    sampled
}

/// Compare only exact 1:1 XRGB8888->RGB565 native Blts; any scaling,
/// color-key, alpha, or different pixel masks is reported but NOT sampled.
/// If sampling succeeds, the pixel values distinguish native conversion
/// mismatch from bad input data. This intentionally cannot prove the
/// source image is the intended visual artwork.
pub unsafe fn probe_native_blt(src:SurfaceThis,dst:SurfaceThis,
    dst_rect:*mut Rect,src_rect:*mut Rect,copy_number:usize){
    if src.is_null()||dst.is_null(){return;}
    let (s,d)=match (desc(src),desc(dst)){
        (Ok(s),Ok(d))=>(s,d),
        (a,b)=>{
            crate::log(&format!("COLOR-PROBE: copy={copy_number} descriptors unavailable source={a:?} dest={b:?}"));
            return;
        }
    };
    if !is_xrgb8888(&s)||!is_rgb565(&d){
        crate::log(&format!(
            "COLOR-PROBE: copy={copy_number} skipped formats src={}bpp masks={:08x}/{:08x}/{:08x} dst={}bpp masks={:08x}/{:08x}/{:08x}",
            s[21],s[22],s[23],s[24],d[21],d[22],d[23],d[24]));
        return;
    }
    let (Some(sr),Some(dr))=(
        rect_or_full(src_rect,s[3],s[2]),rect_or_full(dst_rect,d[3],d[2])
    )else{
        crate::log(&format!("COLOR-PROBE: copy={copy_number} invalid source/destination rectangles"));
        return;
    };
    if !same_extent(sr,dr){
        crate::log(&format!(
            "COLOR-PROBE: copy={copy_number} skipped scaled Blt {}x{} -> {}x{}; sample comparison requires one-to-one",
            sr.right-sr.left,sr.bottom-sr.top,dr.right-dr.left,dr.bottom-dr.top));
        return;
    }
    let src_pixels=match read_swatches(src,sr,4){
        Ok(p)=>p,
        Err(e)=>{
            crate::log(&format!("COLOR-PROBE: copy={copy_number} source read unavailable: {e}"));
            return;
        }
    };
    let dst_pixels=match read_swatches(dst,dr,2){
        Ok(p)=>p,
        Err(e)=>{
            crate::log(&format!("COLOR-PROBE: copy={copy_number} destination read unavailable: {e}"));
            return;
        }
    };
    let mut mismatches=0usize;
    let mut samples=String::new();
    for i in 0..9{
        let expected=xrgb8888_to_rgb565(src_pixels[i]);
        let observed=dst_pixels[i] as u16;
        if expected!=observed{mismatches+=1;}
        use std::fmt::Write as _;
        let _=write!(&mut samples,
            " p{i}=xrgb{:08x}/expected{:04x}/got{:04x}",
            src_pixels[i],expected,observed);
    }
    crate::log(&format!(
        "COLOR-PROBE: copy={copy_number} match={}/9 mismatches={mismatches} src_rect={sr:?} dst_rect={dr:?} details:{samples}",
        9-mismatches));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn channel_masks_and_alpha_ignored(){
        assert_eq!(xrgb8888_to_rgb565(0x00ff0000),0xf800);
        assert_eq!(xrgb8888_to_rgb565(0x0000ff00),0x07e0);
        assert_eq!(xrgb8888_to_rgb565(0x000000ff),0x001f);
        assert_eq!(xrgb8888_to_rgb565(0xffffffff),0xffff);
        assert_eq!(xrgb8888_to_rgb565(0xff000000),0);
        assert_eq!(xrgb8888_to_rgb565(0x007f7f7f),0x7bef);
    }
    #[test]
    fn rate_is_bounded_and_includes_later_video_frames(){
        assert!(should_probe(1));assert!(should_probe(3));
        assert!(!should_probe(4));assert!(should_probe(60));
        assert!(should_probe(1200));assert!(!should_probe(1260));
    }
    #[test]
    fn sample_rectangles_respect_bounds_and_offsets(){
        let full=rect_or_full(ptr::null_mut(),800,600).unwrap();
        assert_eq!(sample_position(full,(4,4)),(400,300));
        let mut shifted=Rect{left:20,top:30,right:120,bottom:130};
        assert!(same_extent(shifted,Rect{left:200,top:40,right:300,bottom:140}));
        assert_eq!(sample_position(shifted,(4,4)),(70,80));
        assert!(rect_or_full(&mut shifted,800,600).is_some());
        shifted.right=801;
        assert!(rect_or_full(&mut shifted,800,600).is_none());
    }
    #[test]
    fn formats_require_precise_rgb_masks(){
        let mut d=[0u32;31];d[0]=124;d[18]=32;d[19]=0x40;
        d[21]=32;d[22]=0xff0000;d[23]=0xff00;d[24]=0xff;
        assert!(is_xrgb8888(&d));assert!(!is_rgb565(&d));
        d[21]=16;d[22]=0xf800;d[23]=0x07e0;d[24]=0x001f;
        assert!(is_rgb565(&d));assert!(!is_xrgb8888(&d));
    }
}
