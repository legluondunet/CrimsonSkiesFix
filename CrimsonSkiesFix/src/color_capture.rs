//! Opt-in, one-shot full-frame *diagnostic* captures of game-owned native
//! DirectDraw4 XRGB8888 movie sources and RGB565 backbuffer.
//! Exact images are saved as ordinary 24-bit uncompressed BMP files next
//! to the loaded proxy DLL. This module never writes to source surfaces.
//! It can stall while locking GPU resources; disabled by default.
#![allow(dead_code)]
use std::{io::Write,fs::OpenOptions,ptr};
use crate::com_surface4_abi::{SurfaceThis,Surface4VTable};

const RGB_FLAG:u32=0x40;
const LOCK_READONLY_WAIT:u32=0x11;
const MAX_PIXELS:u32=1024*768;

pub fn capture_slot(copy:usize)->bool{copy==1||copy==3}

fn rgb888_from_rgb565(px:u16)->(u8,u8,u8){
    let r=(px>>11)&31;
    let g=(px>>5)&63;
    let b=px&31;
    (((r<<3)|(r>>2)) as u8,((g<<2)|(g>>4)) as u8,
        ((b<<3)|(b>>2)) as u8)
}

/// Encode a full, complete 24-bit bottom-up BMP using BGR pixel order.
/// The pixels are supplied in **top-down** BGR order without row padding.
fn encode_bmp24(width:u32,height:u32,bgr:&[u8])->Result<Vec<u8>,String>{
    if width==0||height==0 || width.checked_mul(height).unwrap_or(u32::MAX)>MAX_PIXELS{
        return Err("invalid/oversized snapshot dimensions".into());
    }
    let pixels=(width as usize)*(height as usize);
    if bgr.len()!=pixels*3{return Err("snapshot byte count mismatch".into());}
    let row_bytes=(width as usize)*3;
    let stride=(row_bytes+3)&!3;
    let data_len=stride.checked_mul(height as usize).ok_or("BMP overflow")?;
    let file_len=54usize.checked_add(data_len).ok_or("BMP overflow")?;
    if file_len>u32::MAX as usize{return Err("BMP too large".into());}
    let mut bmp=Vec::with_capacity(file_len);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_len as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8;4]);
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes()); // bottom-up
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&24u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    bmp.extend_from_slice(&(data_len as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8;16]);
    let padding=[0u8;3];
    for y in (0..height as usize).rev(){
        let base=y*row_bytes;
        bmp.extend_from_slice(&bgr[base..base+row_bytes]);
        bmp.extend_from_slice(&padding[..stride-row_bytes]);
    }
    debug_assert_eq!(bmp.len(),file_len);
    Ok(bmp)
}

#[inline]
unsafe fn table(p:SurfaceThis)->&'static Surface4VTable{
    &**p.cast::<*const Surface4VTable>()
}

/// Copy pixel values from a locked native DirectDraw surface into an owned
/// temporary image. Never reads unvalidated GPU pointers after Unlock.
unsafe fn read_native_bgr(p:SurfaceThis,expected_bpp:u32)
    ->Result<(u32,u32,Vec<u8>),String>{
    if p.is_null(){return Err("NULL native Surface4".into());}
    let mut desc=[0u32;31];desc[0]=124;
    let hr=(table(p).get_surface_desc)(p,desc.as_mut_ptr().cast());
    if hr<0 || desc[0]!=124 {return Err(format!("GetSurfaceDesc failed {hr:#010x}"));}
    let (w,h)=(desc[3],desc[2]);
    let acceptable=(expected_bpp==32 &&
          (desc[22],desc[23],desc[24])==(0xff0000,0xff00,0xff))
       || (expected_bpp==16 &&
          (desc[22],desc[23],desc[24])==(0xf800,0x07e0,0x001f));
    if desc[18]!=32 || desc[19]&RGB_FLAG==0 || desc[21]!=expected_bpp || !acceptable ||
       w==0 || h==0 || w.checked_mul(h).unwrap_or(u32::MAX)>MAX_PIXELS {
        return Err(format!("unexpected format/dimensions {}x{} bpp={}",w,h,desc[21]));
    }
    let mut locked=[0u32;31];locked[0]=124;
    let hr=(table(p).lock)(p,ptr::null_mut(),locked.as_mut_ptr().cast(),
        LOCK_READONLY_WAIT,ptr::null_mut());
    if hr<0{return Err(format!("Lock failed {hr:#010x}"));}
    let result=(||->Result<(u32,u32,Vec<u8>),String>{
        let bpp=(expected_bpp/8) as usize;
        let pitch=locked[4] as usize;
        if locked[0]!=124 || locked[2]!=h || locked[3]!=w ||
           pitch<(w as usize)*bpp || pitch>(w as usize)*bpp+16384 ||
           locked[9]==0{
            return Err("invalid locked pitch, dimensions or pointer".into());
        }
        let base=locked[9] as usize as *const u8;
        let mut bgr=vec![0u8;(w as usize)*(h as usize)*3];
        for y in 0..h as usize{
            for x in 0..w as usize{
                let at=base.add(y*pitch+x*bpp);
                let (r,g,b)=if expected_bpp==32{
                    let px=ptr::read_unaligned(at.cast::<u32>());
                    ((px>>16) as u8,(px>>8) as u8,px as u8)
                }else{
                    rgb888_from_rgb565(ptr::read_unaligned(at.cast::<u16>()))
                };
                let out=(y*w as usize+x)*3;
                bgr[out]=b;bgr[out+1]=g;bgr[out+2]=r;
            }
        }
        Ok((w,h,bgr))
    })();
    let unlock_hr=(table(p).unlock)(p,ptr::null_mut());
    if unlock_hr<0{return Err(format!("Unlock failed {unlock_hr:#010x}"));}
    result
}

unsafe fn capture_one(p:SurfaceThis,bpp:u32,label:&'static str){
    let (w,h,pixels)=match read_native_bgr(p,bpp){
        Ok(v)=>v,
        Err(e)=>{
            crate::log(&format!("COLOR-CAPTURE: {label} unavailable: {e}"));
            return;
        }
    };
    let bmp=match encode_bmp24(w,h,&pixels){
        Ok(v)=>v,
        Err(e)=>{
            crate::log(&format!("COLOR-CAPTURE: {label} encoding failed: {e}"));
            return;
        }
    };
    let Some(path)=crate::color_capture_path(label) else{
        crate::log(&format!("COLOR-CAPTURE: {label} game DLL path unavailable"));
        return;
    };
    // Create-new, not truncate: never replace old user captures or symlinks.
    match OpenOptions::new().write(true).create_new(true).open(&path)
        .and_then(|mut file|file.write_all(&bmp)){
        Ok(())=>crate::log(&format!(
            "COLOR-CAPTURE: {label} saved {}x{} BMP24 file={} (native readback, no game pixel writes)",
            w,h,path.file_name().unwrap_or_default().to_string_lossy())),
        Err(e)=>crate::log(&format!("COLOR-CAPTURE: {label} save failed: {e}")),
    }
}

/// Invoked only after successful game-native Blts for specified copy numbers.
/// Capture of #1 saves the unscaled 800x600 source; capture of #3 saves
/// BOTH the 320x240 movie source and 800x600 RGB565 destination.
pub unsafe fn capture_after_blt(src:SurfaceThis,dst:SurfaceThis,number:usize){
    match number{
        1=>capture_one(src,32,"Copy1-Source32"),
        3=>{
            capture_one(src,32,"Copy3-Source32");
            capture_one(dst,16,"Copy3-Dest565");
        }
        _=>{}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn first_movie_copies_only(){
        assert!(capture_slot(1));assert!(capture_slot(3));
        assert!(!capture_slot(2));assert!(!capture_slot(60));
    }
    #[test]
    fn bmp24_header_and_row_padding_are_correct(){
        // top-down BGR: first row blue, second row red; stride is 4 bytes.
        let buf=encode_bmp24(1,2,&[255,0,0,0,0,255]).unwrap();
        assert_eq!(&buf[..2],b"BM");
        assert_eq!(u32::from_le_bytes(buf[2..6].try_into().unwrap()),62);
        assert_eq!(u32::from_le_bytes(buf[18..22].try_into().unwrap()),1);
        assert_eq!(u32::from_le_bytes(buf[22..26].try_into().unwrap()),2);
        assert_eq!(u16::from_le_bytes(buf[28..30].try_into().unwrap()),24);
        assert_eq!(&buf[54..62],&[0,0,255,0,255,0,0,0]);
    }
    #[test]
    fn rgb565_to_rgb888_channels_are_reasonable(){
        assert_eq!(rgb888_from_rgb565(0xf800),(255,0,0));
        assert_eq!(rgb888_from_rgb565(0x07e0),(0,255,0));
        assert_eq!(rgb888_from_rgb565(0x001f),(0,0,255));
        assert_eq!(rgb888_from_rgb565(0xffff),(255,255,255));
    }
}
