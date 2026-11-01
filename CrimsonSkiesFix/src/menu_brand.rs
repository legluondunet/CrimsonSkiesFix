//! Exact MainMenu-only build signature drawn directly onto the real
//! DirectDraw4 primary surface. GDI GetDC(HWND) is not composited with the
//! DirectDraw primary reliably under Wine, even when DrawTextW succeeds.
//! Never place the label in the 4:3 black pillarbox or during gameplay.
use std::{ffi::c_void,sync::{OnceLock,atomic::{AtomicBool,Ordering}}};
use crate::{build_info,menu_scene,log};
use crate::borderless_native_blt::Rect;

type Surface=*mut c_void;
type Hdc=*mut c_void;
type SurfaceGetDc=unsafe extern "system" fn(Surface,*mut Hdc)->i32;
type SurfaceReleaseDc=unsafe extern "system" fn(Surface,Hdc)->i32;
const GET_DC_SLOT:usize=17; // IDirectDrawSurface4::GetDC
const RELEASE_DC_SLOT:usize=26; // IDirectDrawSurface4::ReleaseDC
const TRANSPARENT:i32=1;
const DEFAULT_GUI_FONT:i32=17;
const DT_SINGLELINE:u32=0x20;
const DT_RIGHT:u32=0x2;
const DT_VCENTER:u32=0x4;
const DT_NOPREFIX:u32=0x800;
static LABEL:OnceLock<Vec<u16>>=OnceLock::new();
static REPORTED:AtomicBool=AtomicBool::new(false);
static REPORTED_ERROR:AtomicBool=AtomicBool::new(false);

#[link(name="user32")]
extern "system" {
    fn DrawTextW(dc:Hdc,text:*const u16,count:i32,rect:*mut Rect,flags:u32)->i32;
}
#[link(name="gdi32")]
extern "system" {
    fn GetStockObject(index:i32)->*mut c_void;
    fn SelectObject(dc:Hdc,object:*mut c_void)->*mut c_void;
    fn SetBkMode(dc:Hdc,mode:i32)->i32;
    fn SetTextColor(dc:Hdc,color:u32)->u32;
}
fn label_rect(view:Rect)->Option<Rect>{
    let width=view.right.checked_sub(view.left)?;
    let height=view.bottom.checked_sub(view.top)?;
    if width<320||height<240{return None;}
    let left=view.right.saturating_sub(width.min(500)).saturating_add(8);
    let right=view.right.saturating_sub(24);
    let top=view.bottom.saturating_sub(42);
    let bottom=view.bottom.saturating_sub(18);
    (left<right&&top<bottom).then_some(Rect{left,top,right,bottom})
}
fn report_error(message:&str){
    if !REPORTED_ERROR.swap(true,Ordering::Relaxed){
        log(&format!("MENU-BRANDING: primary Surface4 drawing unavailable: {message}"));
    }
}
/// Call only after a successful real primary windowed Blt. The live caller
/// owns the COM pointers and render thread. This method borrows the surface,
/// does not change DD cooperative level or alter game input/scene contents.
pub fn draw_main_menu(primary:Surface,viewport:Rect){
    if !menu_scene::main_menu_active()||primary.is_null(){return;}
    let Some(mut destination)=label_rect(viewport) else{return;};
    unsafe{
        let table=*(primary.cast::<*const usize>());
        if table.is_null(){report_error("missing COM vtable");return;}
        let get_dc:SurfaceGetDc=std::mem::transmute(*table.add(GET_DC_SLOT));
        let release_dc:SurfaceReleaseDc=std::mem::transmute(*table.add(RELEASE_DC_SLOT));
        let mut dc=std::ptr::null_mut();
        let hr=get_dc(primary,&mut dc);
        if hr<0 || dc.is_null(){
            report_error(&format!("IDirectDrawSurface4::GetDC hr={hr:#010x}"));
            return;
        }
        let font=GetStockObject(DEFAULT_GUI_FONT);
        let original=if font.is_null(){std::ptr::null_mut()}else{SelectObject(dc,font)};
        let prev_mode=SetBkMode(dc,TRANSPARENT);
        let old_color=SetTextColor(dc,0x00b0dcff); // warm light amber in COLORREF BGR
        let label=LABEL.get_or_init(||build_info::label().encode_utf16().collect());
        let count=i32::try_from(label.len()).unwrap_or(0);
        let drawn=if count>0{
            DrawTextW(dc,label.as_ptr(),count,&mut destination,
                DT_SINGLELINE|DT_RIGHT|DT_VCENTER|DT_NOPREFIX)
        }else{0};
        if old_color!=0xffff_ffff{let _=SetTextColor(dc,old_color);}
        if prev_mode!=0{let _=SetBkMode(dc,prev_mode);}
        if !original.is_null(){let _=SelectObject(dc,original);}
        let release_hr=release_dc(primary,dc);
        if drawn<=0||release_hr<0{
            report_error(&format!("DrawTextW lines={drawn} ReleaseDC hr={release_hr:#010x}"));
        }else if !REPORTED.swap(true,Ordering::Relaxed){
            log(&format!("MENU-BRANDING: DirectDraw4 primary Surface4 text submitted in active MainMenu: {}",build_info::label()));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_stays_inside_true_4_by_3_viewport(){
        for (l,t,r,b) in [(240,0,1680,1080),(-1600,0,0,900),(0,0,800,600)]{
            let viewport=Rect{left:l,top:t,right:r,bottom:b};
            let target=label_rect(viewport).unwrap();
            assert!(target.left>=viewport.left&&target.top>=viewport.top);
            assert!(target.right<=viewport.right&&target.bottom<=viewport.bottom);
        }
        assert!(label_rect(Rect{left:0,top:0,right:12,bottom:12}).is_none());
    }
    #[test]
    fn surface4_slots_are_documented_and_distinct(){
        assert_eq!(GET_DC_SLOT,17);
        assert_eq!(RELEASE_DC_SLOT,26);
    }
}
