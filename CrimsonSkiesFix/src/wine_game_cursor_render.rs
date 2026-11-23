//! Opt-in Wine-only suppression of the *draw call* of Crimson Skies'
//! original GameOS menu cursor (NEVER its input or menu hit-testing).
//!
//! Offline analysis of the exact SHA-256-admitted, decrypted 1.02 executable:
//!  - Constructor 0x5C3E20 constructs only one 0x5CDDD0 object at this+0x58.
//!  - This object's own vtable is 0x60AF8C (installed at 0x5CDDF8).
//!  - The vtable[1] method 0x5CDEB0 renders its bitmap via 0x5C0FF0,
//!    or falls back to the base bitmap renderer 0x5CC7B0.
//!  - Neither method manipulates the game's mouse input/hit-test coordinates.
//! A previous [CURSOR] script-lookup experiment did NOT remove the cursor.
//! The new probe hooks only this proven cursor *class's draw method* slot,
//! not global Blt/D3D/DirectDraw calls or resource loading.
//!
//! Keep Windows native completely untouched; under Wine a verified 800x600x16
//! borderless menu graph and two independent debug opts are necessary. Even
//! after installing the vtable observer, gameplay/other scenes forward every
//! call to the original function. Never modify EXE bytes or shipped assets.
use std::{ffi::c_void,sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use crate::{diagnostics,log,win32};

const EXE_BASE:usize=0x0040_0000;
const CURSOR_VTABLE:usize=0x0060_af8c;
const VTABLE_DTOR:usize=0x005c_de10;
const VTABLE_DRAW:usize=0x005c_deb0;
const VTABLE_RENDER_ALTERNATE:usize=0x005c_c7f0;
const VTABLE_POSITION:usize=0x005c_c6c0;
// Original unique in-place cursor child-object construction:
// lea ecx,[esi+0x58]; call 0x5cddd0
const CURSOR_CONSTRUCTOR_SITE:usize=0x005c_3e4c;
const CURSOR_CONSTRUCTOR_BYTES:[u8;12]=[0x8d,0x4e,0x58,0x89,0x7c,0x24,0x1c,0xe8,0x78,0x9f,0x00,0x00];
// Original cursor-specific virtual draw starts by testing a custom bitmap
// flag; either renders it or jumps to its inherited fallback draw function.
const DRAW_PREIMAGE:[u8;10]=[0x8a,0x81,0x08,0x01,0x00,0x00,0x84,0xc0,0x74,0x27];

static HOOKED:AtomicBool=AtomicBool::new(false);
static MENU_GRAPH_VERIFIED:AtomicBool=AtomicBool::new(false);
static SUPPRESSED:AtomicUsize=AtomicUsize::new(0);
static FORWARDED:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
}

type CursorDraw=unsafe extern "thiscall" fn(*mut c_void);

fn eligible(wine:bool,system_pointer:bool,sync:bool,render_flag:bool,
            bitmap_flag:bool,mode:(u32,u32,u32))->bool{
    wine && system_pointer && sync && render_flag && !bitmap_flag &&
    mode==(800,600,16)
}
fn eligible_config()->bool{
    let c=crate::cfg();
    eligible(diagnostics::is_wine_runtime(),
        c.wine_system_cursor_probe,c.wine_system_cursor_sync,
        c.wine_game_cursor_render_suppress,c.wine_menu_cursor_bitmap_probe,
        (800,600,16)) &&
        c.native_dd4_bootstrap && !c.directdraw_passthrough
}
fn may_suppress(scene_menu:bool,verified:bool,hooked:bool)->bool{
    scene_menu && verified && hooked
}

/// Called from the verified DD4 transition; disabled on leaving 800x600.
/// The VTABLE remains hooked until process exit to avoid a data race with
/// a pending virtual call, but all calls outside the active menu delegate.
pub fn set_verified_menu_mode(verified:bool){
    MENU_GRAPH_VERIFIED.store(verified,Ordering::Release);
}

/// In the authentic GameOS 800x600 menu only, skip precisely its cursor
/// bitmap draw. The original object, cursor coordinates and click handling
/// have already run and are left untouched. No Win32 cursor API calls.
unsafe extern "thiscall" fn cursor_draw_observer(this:*mut c_void){
    let menu=crate::menu_scene::main_menu_active() ||
             crate::menu_scene::preferences_active();
    if may_suppress(menu,MENU_GRAPH_VERIFIED.load(Ordering::Acquire),
                    HOOKED.load(Ordering::Acquire)) && eligible_config() {
        let n=SUPPRESSED.fetch_add(1,Ordering::Relaxed)+1;
        if crate::cfg().log_enabled && (n==1 || n%120==0){
            log(&format!(
                "WINE-GOS-CURSOR-RENDER: suppressed original cursor bitmap draw call={n}; GameOS input/hit-testing and system arrow unchanged"
            ));
        }
        return;
    }
    FORWARDED.fetch_add(1,Ordering::Relaxed);
    let original:CursorDraw=std::mem::transmute(VTABLE_DRAW);
    original(this);
}

/// Arm only after authenticated EXE gate and verified 800x600x16 DD4 graph
/// plus HWND/viewport preflight. The caller has already verified mode and
/// borderless geometry. All code/data signatures checked BEFORE mutation.
pub fn install_for_verified_menu()->Result<String,String>{
    if !eligible_config() {
        return Err("Wine cursor render suppression opt-in not eligible (must disable old bitmap probe)".into());
    }
    if !MENU_GRAPH_VERIFIED.load(Ordering::Acquire){
        return Err("verified GameOS menu DD4 mode not armed".into());
    }
    if HOOKED.load(Ordering::Acquire){
        return Ok("cursor draw vtable hook already installed".into());
    }
    unsafe {
        let exe=GetModuleHandleW(std::ptr::null()) as usize;
        if exe!=EXE_BASE{
            return Err("unexpected admitted EXE image base; unchanged".into());
        }
        let constructor=std::slice::from_raw_parts(
            CURSOR_CONSTRUCTOR_SITE as *const u8,CURSOR_CONSTRUCTOR_BYTES.len()
        );
        if constructor!=CURSOR_CONSTRUCTOR_BYTES{
            return Err("unique cursor object constructor reference did not match; unchanged".into());
        }
        let preimage=std::slice::from_raw_parts(
            VTABLE_DRAW as *const u8,DRAW_PREIMAGE.len()
        );
        if preimage!=DRAW_PREIMAGE{
            return Err("original cursor draw function preimage mismatched; unchanged".into());
        }
        let vtable=CURSOR_VTABLE as *const usize;
        let expected=[VTABLE_DTOR,VTABLE_DRAW,VTABLE_RENDER_ALTERNATE,VTABLE_POSITION];
        for (i,addr) in expected.iter().enumerate(){
            if vtable.add(i).read()!=*addr{
                return Err(format!("cursor-only vtable signature failed at slot {i}; unchanged"));
            }
        }
        let draw_slot=(CURSOR_VTABLE+std::mem::size_of::<usize>()) as *mut usize;
        if !win32::patch_vtable_slot(draw_slot,cursor_draw_observer as usize){
            return Err("cursor-only draw vtable slot protection denied; unchanged".into());
        }
        if draw_slot.read()!=cursor_draw_observer as usize{
            let _=win32::patch_vtable_slot(draw_slot,VTABLE_DRAW);
            return Err("cursor draw vtable readback failed; original restored".into());
        }
        HOOKED.store(true,Ordering::Release);
    }
    Ok("exact GameOS CURSOR render vtable[1] redirected; only 800x600 Wine MainMenu/Preferences draw suppressed; gameplay and Windows original".into())
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn strict_opt_in_and_no_bitmap_probe_conflict(){
        assert!(eligible(true,true,true,true,false,(800,600,16)));
        assert!(!eligible(false,true,true,true,false,(800,600,16)));
        assert!(!eligible(true,false,true,true,false,(800,600,16)));
        assert!(!eligible(true,true,false,true,false,(800,600,16)));
        assert!(!eligible(true,true,true,false,false,(800,600,16)));
        assert!(!eligible(true,true,true,true,true,(800,600,16)));
        assert!(!eligible(true,true,true,true,false,(1920,1080,16)));
    }
    #[test]
    fn rendering_suppression_requires_authentic_menu(){
        assert!(may_suppress(true,true,true));
        assert!(!may_suppress(false,true,true));
        assert!(!may_suppress(true,false,true));
        assert!(!may_suppress(true,true,false));
    }
    #[test]
    fn original_known_menu_cursor_class_signature(){
        assert_eq!(CURSOR_VTABLE,0x60af8c);
        assert_eq!(CURSOR_CONSTRUCTOR_SITE,0x5c3e4c);
        assert_eq!(CURSOR_CONSTRUCTOR_BYTES,[0x8d,0x4e,0x58,0x89,0x7c,0x24,0x1c,0xe8,0x78,0x9f,0,0]);
        assert_eq!(VTABLE_DRAW,0x5cdeb0);
        assert_eq!(DRAW_PREIMAGE,[0x8a,0x81,0x08,0x01,0,0,0x84,0xc0,0x74,0x27]);
        let _:CursorDraw=cursor_draw_observer;
    }
}
