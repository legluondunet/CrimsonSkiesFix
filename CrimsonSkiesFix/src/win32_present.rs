//! Inactive Win32 GDI *output* backend for already-composited BGRA8 images.
//!
//! This function must never be called while the game is in exclusive
//! DirectDraw cooperative mode. A real windowed adapter must first
//! virtualize the primary/backbuffer/Z chain and supply stable pixels.
//! Merely drawing over an exclusive DirectDraw surface is not borderless.

#![allow(dead_code)]
use std::ffi::c_void;
use crate::win32::Hwnd;

#[derive(Debug,Copy,Clone,Eq,PartialEq)]
pub enum PresentError {
    BadGeometry,
    BufferTooSmall,
    NotGameWindow,
    WindowClientMismatch,
    NoDeviceContext,
    GdiFailure,
}

#[repr(C)]
#[derive(Debug,Copy,Clone,Eq,PartialEq)]
struct BitmapInfoHeader {
    size:u32,
    width:i32,
    height:i32,
    planes:u16,
    bits_per_pixel:u16,
    compression:u32,
    image_bytes:u32,
    x_pixels_per_meter:i32,
    y_pixels_per_meter:i32,
    colors_used:u32,
    colors_important:u32,
}

#[repr(C)]
#[derive(Default)]
struct Rect {left:i32,top:i32,right:i32,bottom:i32}

#[link(name="kernel32")]
extern "system" {
    fn GetCurrentProcessId()->u32;
}
#[link(name="user32")]
extern "system" {
    fn IsWindow(hwnd:Hwnd)->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,pid:*mut u32)->u32;
    fn GetClientRect(hwnd:Hwnd,rect:*mut Rect)->i32;
    fn GetDC(hwnd:Hwnd)->*mut c_void;
    fn ReleaseDC(hwnd:Hwnd,dc:*mut c_void)->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn SetDIBitsToDevice(
        hdc:*mut c_void,dst_x:i32,dst_y:i32,dst_width:u32,dst_height:u32,
        src_x:i32,src_y:i32,start_scan:u32,scan_lines:u32,
        pixels:*const c_void,bmi:*const c_void,color_use:u32
    )->i32;
}

fn bitmap_header(width:u32,height:u32,pixels_len:usize)
    ->Result<BitmapInfoHeader,PresentError>
{
    let w=i32::try_from(width).map_err(|_|PresentError::BadGeometry)?;
    let h=i32::try_from(height).map_err(|_|PresentError::BadGeometry)?;
    if w<=0||h<=0{return Err(PresentError::BadGeometry);}
    let expected=(width as usize).checked_mul(height as usize)
        .and_then(|n|n.checked_mul(4)).ok_or(PresentError::BadGeometry)?;
    if pixels_len<expected{return Err(PresentError::BufferTooSmall);}
    let image_bytes=u32::try_from(expected).map_err(|_|PresentError::BadGeometry)?;
    Ok(BitmapInfoHeader{
        size:40,width:w,height:-h, // negative = top-down, matching our BGRA8 compositor
        planes:1,bits_per_pixel:32,
        compression:0, // BI_RGB
        image_bytes,
        x_pixels_per_meter:0,y_pixels_per_meter:0,
        colors_used:0,colors_important:0,
    })
}

/// Copy a complete, tightly packed BGRA8 frame to the client area of
/// a process-owned game window already in normal/windowed mode.
///
/// Never performs SetWindowLong, SetWindowPos, SetDisplayMode,
/// DirectDraw Lock, COM calls, input/window focus changes or scheduling.
/// The caller must ensure no exclusive flip chain is active.
pub unsafe fn draw_frame_to_game_window(
    hwnd:Hwnd,frame:&[u8],width:u32,height:u32
)->Result<(),PresentError>{
    let header=bitmap_header(width,height,frame.len())?;
    if hwnd.is_null() || IsWindow(hwnd)==0{return Err(PresentError::NotGameWindow);}
    let mut pid=0;
    if GetWindowThreadProcessId(hwnd,&mut pid)==0 || pid!=GetCurrentProcessId(){
        return Err(PresentError::NotGameWindow);
    }
    let mut client=Rect::default();
    if GetClientRect(hwnd,&mut client)==0{return Err(PresentError::NotGameWindow);}
    if client.right.saturating_sub(client.left)!=header.width ||
       client.bottom.saturating_sub(client.top)!=-header.height {
        return Err(PresentError::WindowClientMismatch);
    }
    let hdc=GetDC(hwnd);
    if hdc.is_null(){return Err(PresentError::NoDeviceContext);}
    let result=SetDIBitsToDevice(
        hdc,0,0,width,height,0,0,0,height,
        frame.as_ptr().cast(),(&header as *const BitmapInfoHeader).cast(),0
    );
    let _=ReleaseDC(hwnd,hdc);
    if result==0 || result==-1 {Err(PresentError::GdiFailure)}else{Ok(())}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dib_header_is_40_bytes_and_top_down_32bit_bgra(){
        let h=bitmap_header(1920,1080,1920*1080*4).unwrap();
        assert_eq!(std::mem::size_of::<BitmapInfoHeader>(),40);
        assert_eq!((h.width,h.height),(1920,-1080));
        assert_eq!((h.planes,h.bits_per_pixel,h.compression),(1,32,0));
        assert_eq!(h.image_bytes,1920*1080*4);
    }
    #[test]
    fn reject_oversized_or_short_dib_before_win32_calls(){
        assert_eq!(bitmap_header(0,1080,1),Err(PresentError::BadGeometry));
        assert_eq!(bitmap_header(1920,1080,8),Err(PresentError::BufferTooSmall));
        assert_eq!(bitmap_header(u32::MAX,4,usize::MAX),Err(PresentError::BadGeometry));
    }
}
