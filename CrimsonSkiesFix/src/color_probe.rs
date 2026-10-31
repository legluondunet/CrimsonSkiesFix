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
const DDBLT_WAIT:u32=0x01000000;
const DDBLT_ASYNC:u32=0x00000200;
const SWATCHES:[(u32,u32);9]=[
    (1,1),(4,1),(7,1),(1,4),(4,4),(7,4),(1,7),(4,7),(7,7)
];

#[inline]
pub fn xrgb8888_to_rgb565(pixel:u32)->u16{
    // Explicitly ignore the unused high X/alpha byte.
    let r=(pixel>>16)&0xff;
    let g=(pixel>>8)&0xff;
    let b=pixel&0xff;
    // Wine's observed XRGB8888 -> RGB565 normalized-channel conversion
    // rounds to the nearest representable 5/6-bit value. Earlier test
    // code truncated high bits, causing false mismatches in scaled movie
    // samples. 45 of 45 uploaded samples match these rounded values.
    let r5=(r*31+127)/255;
    let g6=(g*63+127)/255;
    let b5=(b*31+127)/255;
    ((r5<<11)|(g6<<5)|b5) as u16
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

/// Wine 11.0 wined3d/surface.c::surface_cpu_blt uses a 16.16 fixed-point
/// nearest-neighbor source step if software scaling is invoked, including
/// when LINEAR is requested but unsupported in the software blitter.
/// This is a DIAGNOSTIC EXPECTATION, not proof that a particular Blt
/// used the CPU path; mismatches are hypotheses, not automatic bugs.
fn cpu_nearest_source_axis(dest_offset:u32,src_extent:u32,dst_extent:u32)->u32{
    let step=(u64::from(src_extent)<<16)/u64::from(dst_extent);
    ((u64::from(dest_offset)*step)>>16)
        .min(u64::from(src_extent)-1) as u32
}

fn matched_sample_positions(src:Rect,dst:Rect)
    ->([(u32,u32);9],[(u32,u32);9]){
    let sw=(src.right-src.left) as u32;
    let sh=(src.bottom-src.top) as u32;
    let dw=(dst.right-dst.left) as u32;
    let dh=(dst.bottom-dst.top) as u32;
    let dst_points=SWATCHES.map(|frac|sample_position(dst,frac));
    let src_points=dst_points.map(|(dx,dy)|{
        (src.left as u32+cpu_nearest_source_axis(dx-dst.left as u32,sw,dw),
         src.top as u32+cpu_nearest_source_axis(dy-dst.top as u32,sh,dh))
    });
    (src_points,dst_points)
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
unsafe fn read_swatches(p:SurfaceThis,region:Rect,
    coordinates:&[(u32,u32);9],bytes_per_pixel:usize)
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
        for (i,(x,y)) in coordinates.iter().copied().enumerate(){
            if x<region.left as u32 || x>=region.right as u32 ||
               y<region.top as u32 || y>=region.bottom as u32{
                return Err("sample outside requested region".into());
            }
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

/// Compare native XRGB8888->RGB565 1:1 and scaled Blts. Scaling uses
/// Wine 11.0 CPU nearest-neighbor fixed-point coordinates as a candidate
/// model, NOT a verified assertion about Wine's chosen Blt backend.
/// Skip keys/alpha/effects: those cannot be compared by plain conversion.
/// This cannot establish whether the original movie pixels are correct.
pub unsafe fn probe_native_blt(src:SurfaceThis,dst:SurfaceThis,
    dst_rect:*mut Rect,src_rect:*mut Rect,flags:u32,copy_number:usize){
    if src.is_null()||dst.is_null(){return;}
    if flags&!(DDBLT_WAIT|DDBLT_ASYNC)!=0{
        crate::log(&format!(
            "COLOR-PROBE: copy={copy_number} skipped non-copy Blt effects flags={flags:#010x}"));
        return;
    }
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
    let mode=if same_extent(sr,dr){"one-to-one"}else{"scaled-cpu-nearest-candidate"};
    let (src_points,dst_points)=matched_sample_positions(sr,dr);
    let src_pixels=match read_swatches(src,sr,&src_points,4){
        Ok(p)=>p,
        Err(e)=>{
            crate::log(&format!("COLOR-PROBE: copy={copy_number} source read unavailable: {e}"));
            return;
        }
    };
    let dst_pixels=match read_swatches(dst,dr,&dst_points,2){
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
            " p{i}=src({},{})_xrgb{:08x}/dst({},{})_expected{:04x}/got{:04x}",
            src_points[i].0,src_points[i].1,src_pixels[i],
            dst_points[i].0,dst_points[i].1,expected,observed);
    }
    crate::log(&format!(
        "COLOR-PROBE: copy={copy_number} mode={mode} match={}/9 mismatches={mismatches} src_rect={sr:?} dst_rect={dr:?} details:{samples}",
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
        // Exact real-game samples from 2026-10-31 v0.1.59 log:
        // previous truncation falsely reported these as corrupt.
        assert_eq!(xrgb8888_to_rgb565(0x003d1705),0x38c1);
        assert_eq!(xrgb8888_to_rgb565(0x002c0d06),0x2861);
        assert_eq!(xrgb8888_to_rgb565(0x00070000),0x0800);
        assert_eq!(xrgb8888_to_rgb565(0x00270705),0x2841);
        assert_eq!(xrgb8888_to_rgb565(0x000d0004),0x1000);
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
    fn wine_cpu_scaled_sampling_matches_16_16_step_without_rounding_up(){
        assert_eq!(cpu_nearest_source_axis(0,320,800),0);
        assert_eq!(cpu_nearest_source_axis(100,320,800),39);
        assert_eq!(cpu_nearest_source_axis(400,320,800),159);
        assert_eq!(cpu_nearest_source_axis(799,320,800),319);
        assert_eq!(cpu_nearest_source_axis(75,240,600),29);
        assert_eq!(cpu_nearest_source_axis(400,800,800),400);
    }
    #[test]
    fn scaled_sample_coordinates_apply_both_rectangle_origins(){
        let sr=Rect{left:12,top:30,right:332,bottom:270};
        let dr=Rect{left:50,top:20,right:850,bottom:620};
        let (sp,dp)=matched_sample_positions(sr,dr);
        assert_eq!(dp[0],(150,95));
        assert_eq!(sp[0],(51,59)); // 12+39, 30+29
        assert_eq!(dp[4],(450,320));
        assert_eq!(sp[4],(171,149)); // 12+159, 30+119
        for (x,y) in sp{
            assert!(x>=12&&x<332&&y>=30&&y<270);
        }
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
