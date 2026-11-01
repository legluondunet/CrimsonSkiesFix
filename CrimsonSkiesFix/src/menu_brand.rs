//! Non-interactive version label shown only while the verified EXE has
//! selected its MainMenu script. Draw after a successful windowed Flip;
//! never alter the game textures, 4:3 viewport, or keyboard/mouse routing.
use std::{ffi::c_void,sync::{OnceLock,atomic::{AtomicBool,Ordering}}};
use crate::{build_info,menu_scene,log};
type Hwnd=*mut c_void;
type Hdc=*mut c_void;
#[repr(C)] #[derive(Clone,Copy,Default)]
struct Rect {left:i32,top:i32,right:i32,bottom:i32}
const TRANSPARENT:i32=1;
const DEFAULT_GUI_FONT:i32=17;
const DT_SINGLELINE:u32=0x20;
const DT_RIGHT:u32=0x2;
const DT_VCENTER:u32=0x4;
const DT_NOPREFIX:u32=0x800;
static LABEL:OnceLock<Vec<u16>>=OnceLock::new();
static REPORTED:AtomicBool=AtomicBool::new(false);

#[link(name="user32")]
extern "system" {
    fn GetClientRect(hwnd:Hwnd,out:*mut Rect)->i32;
    fn GetDC(hwnd:Hwnd)->Hdc;
    fn ReleaseDC(hwnd:Hwnd,dc:Hdc)->i32;
    fn DrawTextW(dc:Hdc,text:*const u16,count:i32,rect:*mut Rect,flags:u32)->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn GetStockObject(index:i32)->*mut c_void;
    fn SelectObject(dc:Hdc,object:*mut c_void)->*mut c_void;
    fn SetBkMode(dc:Hdc,mode:i32)->i32;
    fn SetTextColor(dc:Hdc,color:u32)->u32;
}
fn label_rect(client:Rect)->Option<Rect>{
    let width=client.right.checked_sub(client.left)?;
    let height=client.bottom.checked_sub(client.top)?;
    if width<320||height<240{return None;}
    Some(Rect{
        left:client.right.saturating_sub(width.min(500)).saturating_add(8),
        top:client.bottom.saturating_sub(39),
        right:client.right.saturating_sub(16),
        bottom:client.bottom.saturating_sub(13)
    })
}
pub fn draw_main_menu(hwnd:Hwnd){
    if !menu_scene::main_menu_active()||hwnd.is_null(){return;}
    unsafe{
        let mut client=Rect::default();
        if GetClientRect(hwnd,&mut client)==0{return;}
        let Some(mut destination)=label_rect(client) else{return;};
        let dc=GetDC(hwnd);
        if dc.is_null(){return;}
        let font=GetStockObject(DEFAULT_GUI_FONT);
        let original=if font.is_null(){std::ptr::null_mut()}
                     else{SelectObject(dc,font)};
        let prev_mode=SetBkMode(dc,TRANSPARENT);
        let old_color=SetTextColor(dc,0x00c5e0ff); // warm pale amber, COLORREF BGR
        let label=LABEL.get_or_init(||build_info::label().encode_utf16().collect());
        let count=i32::try_from(label.len()).unwrap_or(0);
        let drawn=if count>0 {
            DrawTextW(dc,label.as_ptr(),count,&mut destination,
                DT_SINGLELINE|DT_RIGHT|DT_VCENTER|DT_NOPREFIX)
        } else{0};
        if old_color!=0xffff_ffff{let _=SetTextColor(dc,old_color);}
        if prev_mode!=0{let _=SetBkMode(dc,prev_mode);}
        if !original.is_null(){let _=SelectObject(dc,original);}
        let _=ReleaseDC(hwnd,dc);
        if drawn!=0 && !REPORTED.swap(true,Ordering::Relaxed){
            log(&format!("MENU-BRANDING: draw requested on active MainMenu: {}",build_info::label()));
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn label_stays_within_client_rect(){
        for (w,h) in [(800,600),(1920,1080),(2560,1440),(640,480)]{
            let client=Rect{left:0,top:0,right:w,bottom:h};
            let box_=label_rect(client).unwrap();
            assert!(box_.left>=0&&box_.top>=0);
            assert!(box_.right<=w&&box_.bottom<=h);
            assert!(box_.right>box_.left&&box_.bottom>box_.top);
        }
        assert!(label_rect(Rect{left:0,top:0,right:10,bottom:10}).is_none());
    }
}
