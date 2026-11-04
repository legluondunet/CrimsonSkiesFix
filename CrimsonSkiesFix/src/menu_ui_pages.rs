//! Bounded, diagnostic-only observers for the original GOS page-loader
//! and localization APIs. Neither wrapper interprets object pointers,
//! changes the game's script data nor modifies rendered text.
//!
//! Exact original 1.02 registration addresses and handler pointers
//! verified against the unique accepted game EXE SHA-256.
use std::{ffi::c_void,ptr,sync::atomic::{AtomicU32,Ordering}};
use crate::log;

const IMAGE_BASE:usize=0x0040_0000;
const PAGE_SITE:usize=0x0040_7630;
const PAGE_ORIGINAL:usize=0x0040_3680;
const PAGE_BYTES:[u8;5]=[0x68,0x80,0x36,0x40,0x00];
const LOCALIZE_SITE:usize=0x0040_764e;
const LOCALIZE_ORIGINAL:usize=0x0040_4630;
const LOCALIZE_BYTES:[u8;5]=[0x68,0x30,0x46,0x40,0x00];
const PAGE_EXECUTE_READWRITE:u32=0x40;
const MAX_PAGES:u32=24;
const MAX_LOCALIZE:u32=80;
static PAGE_LOGS:AtomicU32=AtomicU32::new(0);
static LABEL_LOGS:AtomicU32=AtomicU32::new(0);
static LAST_LABEL_ID:AtomicU32=AtomicU32::new(u32::MAX);

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(site:*mut c_void,size:usize,protection:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(proc:*mut c_void,site:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}
type ScriptFn=unsafe extern "system" fn(usize,usize,*const u32)->u32;

fn in_menu_context()->bool{
    crate::cfg().log_enabled &&
    crate::menu_video_probe::legacy_ui_active() &&
    crate::menu_scene::main_menu_was_seen()
}
fn is_small_resource_id(id:u32)->bool{(1..=0xffff).contains(&id)}
unsafe extern "system" fn page_passthrough(a:usize,b:usize,args:*const u32)->u32{
    let record=in_menu_context() &&
        PAGE_LOGS.load(Ordering::Relaxed)<MAX_PAGES;
    let original:ScriptFn=std::mem::transmute(PAGE_ORIGINAL);
    let result=original(a,b,args);
    if record && PAGE_LOGS.try_update(Ordering::Relaxed,Ordering::Relaxed,
        |n|if n<MAX_PAGES{Some(n+1)}else{None}).is_ok()
    {
        log(&format!(
            "MENU-GOS-PAGE: gosPageInit invoked after MainMenu; preferences_transaction={} callback_result_class={} [pass-through]",
            crate::menu_gos_event::preferences_transaction_active(),
            if result==1{"success"}else if result==0{"zero"}else{"other"}
        ));
    }
    result
}
unsafe extern "system" fn localize_passthrough(a:usize,b:usize,args:*const u32)->u32{
    // Original gosLocalize reads [args+0] as resource ID, then writes a
    // localized text through [args+4]. Never inspect that output pointer.
    let id=if !args.is_null() && in_menu_context() &&
       crate::menu_gos_event::preferences_transaction_active() {
        Some(ptr::read_unaligned(args))
    }else{None};
    let original:ScriptFn=std::mem::transmute(LOCALIZE_ORIGINAL);
    let result=original(a,b,args);
    if let Some(id)=id {
        if is_small_resource_id(id) &&
            LAST_LABEL_ID.swap(id,Ordering::Relaxed)!=id &&
            LABEL_LOGS.try_update(Ordering::Relaxed,Ordering::Relaxed,
                |n|if n<MAX_LOCALIZE{Some(n+1)}else{None}).is_ok()
        {
            log(&format!(
                "MENU-GOS-LOCALIZE: localized resource_id={} (0x{:x}) [original text/output unchanged]",
                id,id
            ));
        }
    }
    result
}
unsafe fn replace_registration(site:usize,original:[u8;5],replacement:usize)->bool{
    if std::slice::from_raw_parts(site as *const u8,5)!=original{
        log("MENU-GOS-PAGE: exact script registration mismatch; skipped");
        return false;
    }
    let mut previous=0u32;
    if VirtualProtect(site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut previous)==0{
        log("MENU-GOS-PAGE: script registration memory not writable");
        return false;
    }
    let operand=(replacement as u32).to_le_bytes();
    ptr::copy_nonoverlapping(operand.as_ptr(),(site+1) as *mut u8,4);
    let flushed=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5);
    let mut unused=0u32;
    let restored=VirtualProtect(site as *mut c_void,5,previous,&mut unused);
    if flushed==0 || restored==0 {
        let mut writable=0u32;
        if VirtualProtect(site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut writable)!=0{
            ptr::copy_nonoverlapping(original.as_ptr(),site as *mut u8,5);
            let _=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5);
            let mut ignored=0u32;
            let _=VirtualProtect(site as *mut c_void,5,previous,&mut ignored);
        }
        log("MENU-GOS-PAGE: registration failed; original bytes restored where possible");
        return false;
    }
    true
}
/// Instrument only with explicit logging enabled and a verified exact EXE.
/// All 3-parameter original calls and return values are forwarded.
pub unsafe fn install()->(bool,bool){
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE || !crate::cfg().log_enabled{
        return (false,false);
    }
    let page=replace_registration(PAGE_SITE,PAGE_BYTES,page_passthrough as *const () as usize);
    let localize=replace_registration(LOCALIZE_SITE,LOCALIZE_BYTES,localize_passthrough as *const () as usize);
    log(&format!(
        "MENU-GOS-PAGE: page_hook={page} localize_hook={localize}; read-only callbacks, no UI modifications"
    ));
    (page,localize)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn registration_preimages_match_original_1_02(){
        assert_eq!(PAGE_BYTES[0],0x68);
        assert_eq!(&PAGE_BYTES[1..],&(PAGE_ORIGINAL as u32).to_le_bytes());
        assert_eq!(LOCALIZE_BYTES[0],0x68);
        assert_eq!(&LOCALIZE_BYTES[1..],&(LOCALIZE_ORIGINAL as u32).to_le_bytes());
        assert_ne!(PAGE_SITE,LOCALIZE_SITE);
    }
    #[test]
    fn only_plausible_localized_resource_ids_are_logged(){
        assert!(!is_small_resource_id(0));
        assert!(is_small_resource_id(0x2723));
        assert!(is_small_resource_id(0xffff));
        assert!(!is_small_resource_id(0x00648370));
    }
}