//! Experimental Wine-only GameOS menu CURSOR bitmap lookup A/B.
//!
//! Offline analysis of the user's exact SHA256-admitted decrypted 1.02 game
//! EXE shows a single script section lookup for the ASCII key "CURSOR" at
//! RVA 0x001C4986 (push absolute pointer RVA 0x0023FE9C).
//! A subsequent lookup of "BITMAP" loads the candidate menu pointer art.
//! We have NOT yet confirmed that this is the visible software cursor:
//! this reversible, disabled-by-default test distinguishes that hypothesis.
//!
//! Keep all game input, cursor coordinates, hit testing, D3D/DirectDraw
//! surfaces and OS cursor untouched. Change one byte in the game's writable
//! .data lookup key at the menu-mode transition, BEFORE the script loads;
//! restore it on the next mode transition. No game assets are shipped.
use std::sync::atomic::{AtomicBool,Ordering};
use crate::{diagnostics,log};

const CURSOR_SECTION_KEY_RVA:usize=0x0023_fe9c;
const CURSOR_SECTION_REFERENCE_RVA:usize=0x001c_4986;
// Exact 1.02 x86 instruction: push offset "CURSOR".
const PUSH_IMM32:u8=0x68;
const KEY_ORIGINAL:&[u8;7]=b"CURSOR\0";
const KEY_HIDDEN_FIRST:u8=b'X';
static APPLIED:AtomicBool=AtomicBool::new(false);
static VERIFIED_MENU_MODE:AtomicBool=AtomicBool::new(false);

/// Only the already-validated 800x600x16 DD4 graph can authorize a
/// subsequent original MainMenu/Preferences script lookup interception.
pub fn set_verified_menu_mode(verified:bool){
    VERIFIED_MENU_MODE.store(verified,Ordering::Release);
    if !verified{restore_if_armed();}
}

/// The menu dispatcher calls this before forwarding the authentic script
/// loader. Suppressing lookup outside an authentic script risks unrelated UI.
pub fn before_original_script(menu_or_preferences:bool){
    if !menu_or_preferences {
        restore_if_armed();
        return;
    }
    if !VERIFIED_MENU_MODE.load(Ordering::Acquire){return;}
    match arm_for_verified_menu((800,600,16)){
        Ok(details)=>log(&format!("WINE-GOS-CURSOR-BITMAP-PROBE: {details}")),
        Err(reason)=>{
            if crate::cfg().wine_menu_cursor_bitmap_probe {
                log(&format!("WINE-GOS-CURSOR-BITMAP-PROBE: not applied ({reason})"));
            }
        }
    }
}

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleA(name:*const u8)->*mut std::ffi::c_void;
}

fn eligible(wine:bool,probe:bool,sync:bool,bitmap:bool,mode:(u32,u32,u32))->bool{
    wine && probe && sync && bitmap && mode==(800,600,16)
}

/// This toggle is ONLY an A/B test: section lookup bypass != proven renderer
/// suppression, so a user visual report is essential before any default-on.
pub fn arm_for_verified_menu(mode:(u32,u32,u32))->Result<String,String>{
    let c=crate::cfg();
    if !eligible(
        diagnostics::is_wine_runtime(),
        c.wine_system_cursor_probe,c.wine_system_cursor_sync,
        c.wine_menu_cursor_bitmap_probe,mode
    ) || !c.native_dd4_bootstrap || c.directdraw_passthrough {
        return Err("only explicit Wine system-cursor+sync+bitmap probe in 800x600x16 is admitted".into());
    }
    if !VERIFIED_MENU_MODE.load(Ordering::Acquire){
        return Err("verified 800x600 DD4 game window mode not established".into());
    }
    if APPLIED.load(Ordering::Acquire) {
        return Ok("already armed; no additional memory write".into());
    }
    unsafe {
        let base=GetModuleHandleA(std::ptr::null()) as usize;
        if base==0 {return Err("no game EXE base address".into());}
        let key_address=base.checked_add(CURSOR_SECTION_KEY_RVA)
            .ok_or("invalid GameOS CURSOR key RVA")?;
        let callsite=base.checked_add(CURSOR_SECTION_REFERENCE_RVA)
            .ok_or("invalid GameOS CURSOR reference RVA")?;
        // Confirm the original single section lookup, NOT an arbitrary game
        // buffer. A relocated EXE must still reference the relocated key.
        let expected_pointer=u32::try_from(key_address)
            .map_err(|_|"game executable is not 32-bit")?;
        let instruction=std::slice::from_raw_parts(callsite as *const u8,5);
        if instruction[0]!=PUSH_IMM32 ||
            instruction[1..]!=expected_pointer.to_le_bytes() {
            return Err("original CURSOR section push reference does not match; unchanged".into());
        }
        let key=std::slice::from_raw_parts(key_address as *const u8,7);
        if key!=KEY_ORIGINAL{
            return Err("original CURSOR key bytes do not match; unchanged".into());
        }
        // Exactly ONE byte of the verified writable PE .data section. Keep
        // the same string size and terminator. Never mutate render/code.
        std::ptr::write_volatile(key_address as *mut u8,KEY_HIDDEN_FIRST);
        if std::ptr::read_volatile(key_address as *const u8)!=KEY_HIDDEN_FIRST{
            std::ptr::write_volatile(key_address as *mut u8,b'C');
            return Err("GameOS CURSOR lookup key write did not verify".into());
        }
        APPLIED.store(true,Ordering::Release);
    }
    Ok("exact GameOS menu CURSOR section lookup bypassed (CURSOR -> XURSOR); input/OS pointer unchanged; visual verification required".into())
}

/// Return exact original lookup key on leaving the verified menu mode.
pub fn restore_if_armed(){
    if !APPLIED.swap(false,Ordering::AcqRel){return;}
    unsafe {
        let base=GetModuleHandleA(std::ptr::null()) as usize;
        let Some(addr)=base.checked_add(CURSOR_SECTION_KEY_RVA) else {
            log("WINE-GOS-CURSOR-BITMAP-PROBE: could not restore key (address overflow)");
            return;
        };
        // Restore only our own sentinel; another modification is not ours.
        if std::ptr::read_volatile(addr as *const u8)==KEY_HIDDEN_FIRST {
            std::ptr::write_volatile(addr as *mut u8,b'C');
            log("WINE-GOS-CURSOR-BITMAP-PROBE: original CURSOR key restored");
        }else{
            log("WINE-GOS-CURSOR-BITMAP-PROBE: key was modified by another owner; no restore write");
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn bitmap_lookup_probe_is_opt_in_and_menu_only(){
        let m=(800,600,16);
        assert!(eligible(true,true,true,true,m));
        assert!(!eligible(false,true,true,true,m));
        assert!(!eligible(true,false,true,true,m));
        assert!(!eligible(true,true,false,true,m));
        assert!(!eligible(true,true,true,false,m));
        assert!(!eligible(true,true,true,true,(1920,1080,16)));
        assert!(!eligible(true,true,true,true,(800,600,32)));
    }
    #[test]
    fn known_exact_gameos_key_and_x86_reference(){
        assert_eq!(KEY_ORIGINAL,b"CURSOR\0");
        assert_eq!(CURSOR_SECTION_KEY_RVA,0x23fe9c);
        assert_eq!(CURSOR_SECTION_REFERENCE_RVA,0x1c4986);
        assert_ne!(KEY_HIDDEN_FIRST,KEY_ORIGINAL[0]);
        assert_eq!(PUSH_IMM32,0x68);
    }
}
