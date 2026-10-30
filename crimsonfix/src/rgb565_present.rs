//! Software presentation of a *copy* of a legacy RGB565 frame into a BGRA8
//! desktop-sized buffer. This is independent of DirectDraw, Wine, and Win32.
//!
//! It does NOT capture the game's VRAM and is NOT called in gameplay yet.
//! It will be used by a future windowed presentation backend once a
//! DirectDraw-compatible primary/backbuffer/Z-buffer virtualization exists.

use crate::presentation::{fit_viewport,Viewport};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum BlitError {
    ZeroDimensions,
    InvalidPitch,
    ShortSource,
    ShortDestination,
    Overflow,
}

#[derive(Debug,Clone,Copy)]
pub struct Image<'a> {
    pub bytes:&'a [u8],
    pub width:u32,
    pub height:u32,
    pub pitch:usize, // Source surface pitch in bytes. This backend requires positive pitch.
}

#[derive(Debug)]
pub struct BgraOutput<'a> {
    pub bytes:&'a mut [u8],
    pub width:u32,
    pub height:u32,
    pub pitch:usize,
}

fn checked_storage(width:u32,height:u32,pitch:usize,bpp:usize)->Result<usize,BlitError>{
    if width==0||height==0{return Err(BlitError::ZeroDimensions);}
    let row=(width as usize).checked_mul(bpp).ok_or(BlitError::Overflow)?;
    if pitch<row{return Err(BlitError::InvalidPitch);}
    // Last row needs only visible pixels; no requirement that trailing row padding exists.
    (height as usize-1).checked_mul(pitch)
        .and_then(|n|n.checked_add(row)).ok_or(BlitError::Overflow)
}

#[inline]
pub fn rgb565_to_bgra8(pixel:u16)->[u8;4] {
    let r=((pixel>>11)&31) as u32;
    let g=((pixel>>5)&63) as u32;
    let b=(pixel&31) as u32;
    // Exact endpoint-preserving expansion: 0 -> 0, maximum -> 255.
    let red=((r*255+15)/31) as u8;
    let green=((g*255+31)/63) as u8;
    let blue=((b*255+15)/31) as u8;
    [blue,green,red,255]
}

/// Scale a tightly constrained RGB565 picture onto a BGRA8 output.
/// Both buffers are borrowed; there are NO COM allocations, global state,
/// calls to Windows, or reads beyond the source slice. The target is cleared
/// to opaque black, then the 4:3 source is nearest-neighbor scaled into
/// its integer aspect-fitting viewport. Explicit row pitches are respected.
///
/// A windowed presenter must decide when it is safe to capture the source
/// without stalling the Direct3D target. This function doesn't perform that
/// capture and must not be called while the game owns a locked surface.
pub fn compose_rgb565(input:Image<'_>,output:&mut BgraOutput<'_>)
    ->Result<Viewport,BlitError>
{
    let required_source=checked_storage(input.width,input.height,input.pitch,2)?;
    let required_target=checked_storage(output.width,output.height,output.pitch,4)?;
    if input.bytes.len()<required_source{return Err(BlitError::ShortSource);}
    if output.bytes.len()<required_target{return Err(BlitError::ShortDestination);}
    let viewport=fit_viewport(input.width,input.height,output.width,output.height)
        .ok_or(BlitError::ZeroDimensions)?;

    // Clear precisely the visible bytes, never the output's row padding.
    for row in 0..output.height as usize {
        let base=row*output.pitch;
        for px in output.bytes[base..base+output.width as usize*4].chunks_exact_mut(4) {
            px.copy_from_slice(&[0,0,0,255]);
        }
    }
    for y in 0..viewport.height as usize {
        let source_y=(y as u64*input.height as u64/viewport.height as u64) as usize;
        let src_row=source_y*input.pitch;
        let dst_row=(viewport.y as usize+y)*output.pitch;
        for x in 0..viewport.width as usize {
            let source_x=(x as u64*input.width as u64/viewport.width as u64) as usize;
            let src_offset=src_row+source_x*2;
            let pixel=u16::from_le_bytes([input.bytes[src_offset],input.bytes[src_offset+1]]);
            let dst_offset=dst_row+(viewport.x as usize+x)*4;
            output.bytes[dst_offset..dst_offset+4].copy_from_slice(&rgb565_to_bgra8(pixel));
        }
    }
    Ok(viewport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_primary_colors_are_exact() {
        assert_eq!(rgb565_to_bgra8(0xf800),[0,0,255,255]);
        assert_eq!(rgb565_to_bgra8(0x07e0),[0,255,0,255]);
        assert_eq!(rgb565_to_bgra8(0x001f),[255,0,0,255]);
        assert_eq!(rgb565_to_bgra8(0xffff),[255,255,255,255]);
        assert_eq!(rgb565_to_bgra8(0),[0,0,0,255]);
    }

    #[test]
    fn square_input_on_widescreen_is_pillarboxed_with_opaque_black() {
        let src=[0x00,0xf8, 0xe0,0x07, 0x1f,0x00, 0xff,0xff];
        let mut dst=[0u8;32*4]; // 8x4x4 BGRA
        let rect=compose_rgb565(
            Image{bytes:&src,width:2,height:2,pitch:4},
            &mut BgraOutput{bytes:&mut dst,width:8,height:4,pitch:32}).unwrap();
        assert_eq!(rect,Viewport{x:2,y:0,width:4,height:4});
        assert_eq!(&dst[0..4],[0,0,0,255]); // black pillar
        assert_eq!(&dst[8..12],[0,0,255,255]); // RGB565 red
        assert_eq!(&dst[7*4..8*4],[0,0,0,255]); // other pillar
        assert_eq!(&dst[3*32+2*4..3*32+3*4],[255,0,0,255]); // bottom left blue
        assert_eq!(&dst[3*32+5*4..3*32+6*4],[255,255,255,255]); // bottom right white
    }

    #[test]
    fn padded_source_and_target_rows_are_respected() {
        // A 2x1 input, 2 spare bytes after the visible pixels.
        let src=[0,0xf8, 0xe0,0x07, 0xde,0xad];
        let mut dst=[0xeeu8;12];
        let rect=compose_rgb565(
            Image{bytes:&src,width:2,height:1,pitch:6},
            &mut BgraOutput{bytes:&mut dst,width:2,height:1,pitch:12}).unwrap();
        assert_eq!(rect,Viewport{x:0,y:0,width:2,height:1});
        assert_eq!(&dst[0..4],[0,0,255,255]);
        assert_eq!(&dst[4..8],[0,255,0,255]);
        assert_eq!(&dst[8..12],[0xee;4]); // untouched padding
    }

    #[test]
    fn invalid_inputs_are_rejected_before_writing_any_destination() {
        let src=[0u8;4];
        let mut dst=[0x42u8;16];
        assert_eq!(compose_rgb565(
            Image{bytes:&src,width:2,height:2,pitch:2},
            &mut BgraOutput{bytes:&mut dst,width:2,height:2,pitch:8}),
            Err(BlitError::InvalidPitch));
        assert_eq!(dst,[0x42;16]);
        assert_eq!(compose_rgb565(
            Image{bytes:&src,width:2,height:2,pitch:4},
            &mut BgraOutput{bytes:&mut dst,width:2,height:2,pitch:8}),
            Err(BlitError::ShortSource));
        assert_eq!(compose_rgb565(
            Image{bytes:&src,width:1,height:1,pitch:2},
            &mut BgraOutput{bytes:&mut dst,width:3,height:2,pitch:12}),
            Err(BlitError::ShortDestination));
        assert_eq!(dst,[0x42;16]);
    }

    #[test]
    fn shrinking_keeps_input_bounds_and_output_colors() {
        let src=[0,0, 0xff,0xff, 0,0, 0xff,0xff];
        let mut dst=[0u8;4];
        let rect=compose_rgb565(
            Image{bytes:&src,width:2,height:2,pitch:4},
            &mut BgraOutput{bytes:&mut dst,width:1,height:1,pitch:4}).unwrap();
        assert_eq!(rect,Viewport{x:0,y:0,width:1,height:1});
        assert_eq!(dst,[0,0,0,255]);
    }
}
