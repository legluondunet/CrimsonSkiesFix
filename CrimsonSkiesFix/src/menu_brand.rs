//! Exact MainMenu-only build signature drawn directly onto the real
//! DirectDraw4 RGB565 scratch surface. GDI GetDC(HWND) is not composited with the
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
static ART_STATE:std::sync::atomic::AtomicU8=std::sync::atomic::AtomicU8::new(0);
static GETPIXEL_UNAVAILABLE:AtomicBool=AtomicBool::new(false);
/// A successful DrawTextW must be tracked per menu visit; the original
/// REPORTED only logged the *first* successful composition in a process.
static DRAWN_LAST_FRAME:AtomicBool=AtomicBool::new(false);
static PENDING_PRESENT_CONFIRMATION:AtomicBool=AtomicBool::new(false);
static RESUME_DRAW_COUNT:std::sync::atomic::AtomicU32=
    std::sync::atomic::AtomicU32::new(0);

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
    fn GetPixel(dc:Hdc,x:i32,y:i32)->u32;
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
/// Confirm the stable gold Campaign-button artwork instead of relying on
/// GOS command 13 (which is used by multiple nested menu transitions).
/// Positions are normalized from the exact game's native 800x600 4:3 UI,
/// derived from owner-provided MainMenu vs Video screenshots. Fail closed:
/// unknown colors / missing pixels mean no watermark. No input interception.
fn is_campaign_gold(color:u32)->bool{
    if color==0xffff_ffff{return false;} // GDI CLR_INVALID
    let r=(color&0xff) as u32;
    let g=((color>>8)&0xff) as u32;
    let b=((color>>16)&0xff) as u32;
    r>105 && g>70 && r*100>g*108 && g*100>b*128
}
fn main_menu_art_present(dc:Hdc,viewport:Rect)->bool{
    let width=viewport.right-viewport.left;
    let height=viewport.bottom-viewport.top;
    if width<640||height<480{return false;}
    // Eight interior points across the solid Campaign button, carefully
    // avoiding its caption, borders and the looping background movie.
    let samples:[(i32,i32);8]=[
        (308,296),(314,298),(320,300),(326,302),
        (332,304),(320,290),(320,310),(340,300)
    ];
    let mut gold=0u32;
    let mut invalid=0u32;
    for (x,y) in samples {
        let px=viewport.left+(x*width)/800;
        let py=viewport.top+(y*height)/600;
        let color=unsafe{GetPixel(dc,px,py)};
        if color==0xffff_ffff{invalid+=1;}
        if is_campaign_gold(color){gold+=1;}
    }
    // Some Wine GDI surface implementations support DrawTextW while
    // disallowing GetPixel. In that *specific* case retain the previous
    // known-good MainMenu gating instead of losing all branding.
    if invalid==8 {
        if !GETPIXEL_UNAVAILABLE.swap(true,Ordering::Relaxed) {
            log("MENU-BRANDING: offscreen GetPixel unsupported; falling back to original GOS dialog guard");
        }
        return crate::menu_gos_event::main_menu_unobstructed();
    }
    gold>=6
}
fn should_inspect_game_frame(width:u32,height:u32,seen_main:bool)->bool{
    matches!((width,height),(640,480)|(800,600)) && seen_main
}
/// The last script identity is not reliable on return from gameplay:
/// probe *only* genuine 4:3 UI frames after a verified MainMenu was
/// observed. A separate original-Campaign-artwork test prevents
/// drawing on Video Preferences and other non-MainMenu screens.
pub fn should_probe_menu_frame(width:u32,height:u32)->bool{
    should_inspect_game_frame(width,height,menu_scene::main_menu_was_seen())
}
/// Mark a skipped/non-main-menu frame so the next verified successful
/// composition can be logged as a distinct return-to-menu event.
pub fn mark_not_drawn(){
    DRAWN_LAST_FRAME.store(false,Ordering::Release);
    PENDING_PRESENT_CONFIRMATION.store(false,Ordering::Release);
}
/// Called only after the bridge returns a *successful* RGB565 primary
/// presentation with the prepared private scratch as source. This
/// distinguishes offscreen DrawTextW success from actual Blt success.
pub fn note_branded_presented(){
    if PENDING_PRESENT_CONFIRMATION.swap(false,Ordering::AcqRel) &&
        crate::cfg().log_enabled
    {
        log("MENU-BRANDING: signature frame actually presented by native RGB565 primary Blt");
    }
}
pub fn report_copy_failure(hr:i32){
    report_error(&format!("offscreen RGB565 source copy failed hr={hr:#010x}"));
}
fn report_error(message:&str){
    if !REPORTED_ERROR.swap(true,Ordering::Relaxed){
        log(&format!("MENU-BRANDING: offscreen RGB565 Surface4 drawing unavailable: {message}"));
    }
}
/// Draw on a dedicated offscreen RGB565 scratch surface before it is copied
/// onto the real primary. The scratch is re-filled from the unchanged game
/// backbuffer every frame, so the signature and animated background form a
/// single present operation and never alter game-owned surface contents.
pub fn draw_main_menu(scratch:Surface,viewport:Rect)->bool{
    if scratch.is_null(){mark_not_drawn();return false;}
    let Some(mut destination)=label_rect(viewport) else{return false;};
    unsafe{
        let table=*(scratch.cast::<*const usize>());
        if table.is_null(){report_error("missing COM vtable");return false;}
        let get_dc:SurfaceGetDc=std::mem::transmute(*table.add(GET_DC_SLOT));
        let release_dc:SurfaceReleaseDc=std::mem::transmute(*table.add(RELEASE_DC_SLOT));
        let mut dc=std::ptr::null_mut();
        let hr=get_dc(scratch,&mut dc);
        if hr<0 || dc.is_null(){
            report_error(&format!("offscreen IDirectDrawSurface4::GetDC hr={hr:#010x}"));
            return false;
        }
        let visible=main_menu_art_present(dc,viewport);
        let scene=if visible{2}else{1};
        let old=ART_STATE.swap(scene,Ordering::AcqRel);
        if old!=scene && crate::cfg().log_enabled{
            log(if visible{
                "MENU-BRANDING: confirmed native MainMenu Campaign artwork; watermark eligible"
            }else{
                "MENU-BRANDING: native MainMenu artwork absent; watermark hidden"
            });
        }
        if !visible {
            // Genuine Video/other dialogs do not have the gold campaign
            // button at these sampled positions. No text is drawn.
            mark_not_drawn();
            let _=release_dc(scratch,dc);
            return false;
        }
        // A previous gosCallback(13) can leave an internal "Preferences"
        // hint set after Return to Menu. Campaign artwork is stronger
        // evidence of the current visible scene. Clear only that hint.
        crate::menu_gos_event::on_main_menu_art_verified();
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
        let release_hr=release_dc(scratch,dc);
        if drawn<=0||release_hr<0{
            mark_not_drawn();
            report_error(&format!("DrawTextW lines={drawn} ReleaseDC hr={release_hr:#010x}"));
            false
        }else{
            let first=!REPORTED.swap(true,Ordering::Relaxed);
            let reentered=!DRAWN_LAST_FRAME.swap(true,Ordering::AcqRel);
            if reentered{
                PENDING_PRESENT_CONFIRMATION.store(true,Ordering::Release);
            }
            if first {
                log(&format!("MENU-BRANDING: offscreen RGB565 menu text composited before primary Blt: {}",build_info::label()));
            }else if reentered && crate::cfg().log_enabled {
                let n=RESUME_DRAW_COUNT.fetch_add(1,Ordering::Relaxed)+1;
                if n<=16 {
                    log(&format!("MENU-BRANDING: MainMenu signature COMPOSITED after return; reentry={n} script_main={} label={}",
                        menu_scene::main_menu_active(),build_info::label()));
                }
            }
            true
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn returning_to_main_menu_does_not_require_a_new_script_load(){
        assert!(should_inspect_game_frame(800,600,true));
        assert!(should_inspect_game_frame(640,480,true));
        assert!(!should_inspect_game_frame(800,600,false));
        assert!(!should_inspect_game_frame(1920,1080,true));
        assert!(!should_inspect_game_frame(320,240,true));
    }
    #[test]
    fn scene_art_classifier_distinguishes_campaign_gold_from_video_panel(){
        fn col(r:u32,g:u32,b:u32)->u32{r|(g<<8)|(b<<16)}
        for rgb in [(173,138,82),(189,138,66),(222,186,99),(167,140,79)]{
            assert!(is_campaign_gold(col(rgb.0,rgb.1,rgb.2)));
        }
        for rgb in [(54,51,38),(16,16,4),(41,1,3),(140,0,0),(66,62,36)]{
            assert!(!is_campaign_gold(col(rgb.0,rgb.1,rgb.2)));
        }
        assert!(!is_campaign_gold(0xffff_ffff));
    }
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
