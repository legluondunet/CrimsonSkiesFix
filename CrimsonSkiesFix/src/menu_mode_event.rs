//! Read-only observer for the original game's "set video mode" gosCallback
//! dispatch. Unlike frame snapshots, logs the original event invocation
//! even when the requested legacy mode index does not actually change.
//!
//! Never block or replace user input here: this is an experimental
//! diagnostic hook, authenticated to the single supported v1.02 EXE.
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32,Ordering};
use crate::log;

const IMAGE_BASE:usize=0x0040_0000;
const CALL_SITE:usize=0x0040_7811;
const ORIGINAL_TARGET:usize=0x0041_9420;
const ORIGINAL_CALL:[u8;5]=[0xe8,0x0a,0x1c,0x01,0x00];
const SELECTED_MODE:usize=0x0064_b48c;
const MAX_LOGS:u32=48;
const PAGE_EXECUTE_READWRITE:u32=0x40;
static EVENTS:AtomicU32=AtomicU32::new(0);

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,new_protection:u32,old:*mut u32)->i32;
    fn GetCurrentProcess()->*mut c_void;
    fn FlushInstructionCache(process:*mut c_void,site:*const c_void,length:usize)->i32;
}
fn rel_call(site:usize,target:usize)->[u8;5]{
    let disp=(target as u32).wrapping_sub((site as u32).wrapping_add(5)).to_le_bytes();
    [0xe8,disp[0],disp[1],disp[2],disp[3]]
}
fn should_log(n:u32)->bool{n<MAX_LOGS}
/// The original uses a single DWORD argument and `ret 4`. It ignores
/// ECX even though the gosCallback dispatcher sets a game-object value.
unsafe extern "system" fn observe_mode_request(value:u32){
    let before=ptr::read_volatile(SELECTED_MODE as *const u32);
    let original:unsafe extern "system" fn(u32)=std::mem::transmute(ORIGINAL_TARGET);
    original(value); // retain every original side effect, including table normalization
    let after=ptr::read_volatile(SELECTED_MODE as *const u32);
    let count=EVENTS.fetch_add(1,Ordering::Relaxed);
    if should_log(count) && crate::cfg().log_enabled {
        log(&format!(
            "MENU-VIDEO-EVENT: gosCallback video-mode request={} index_before={} index_after={} changed={} [READ-ONLY; no menu input suppressed]",
            value,before,after,before!=after
        ));
    }
}

/// Called once after exact executable SHA-256 authentication, from
/// DirectDrawCreate (never from DllMain). Fail closed on any mismatch.
pub unsafe fn install()->bool{
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE{
        log("MENU-VIDEO-EVENT: unsupported executable image base; observer disabled");
        return false;
    }
    if std::slice::from_raw_parts(CALL_SITE as *const u8,5)!=ORIGINAL_CALL{
        log("MENU-VIDEO-EVENT: byte-exact mode callback preimage mismatch; observer disabled");
        return false;
    }
    let mut old_protection=0;
    if VirtualProtect(CALL_SITE as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut old_protection)==0{
        log("MENU-VIDEO-EVENT: executable page not writable; observer disabled");
        return false;
    }
    let new_bytes=rel_call(CALL_SITE,observe_mode_request as usize);
    ptr::copy_nonoverlapping(new_bytes.as_ptr(),CALL_SITE as *mut u8,5);
    let flushed=FlushInstructionCache(GetCurrentProcess(),CALL_SITE as *const c_void,5);
    let mut unused=0;
    let protected=VirtualProtect(CALL_SITE as *mut c_void,5,old_protection,&mut unused);
    if flushed==0 || protected==0 {
        let mut temporary=0;
        if VirtualProtect(CALL_SITE as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut temporary)!=0 {
            ptr::copy_nonoverlapping(ORIGINAL_CALL.as_ptr(),CALL_SITE as *mut u8,5);
            let _=FlushInstructionCache(GetCurrentProcess(),CALL_SITE as *const c_void,5);
            let mut unused_restore=0;
            let _=VirtualProtect(CALL_SITE as *mut c_void,5,old_protection,&mut unused_restore);
        }
        log("MENU-VIDEO-EVENT: mode observer installation failed; original call restored where possible");
        return false;
    }
    log("MENU-VIDEO-EVENT: exact gosCallback mode-selection call observed (pass-through, read-only)");
    true
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn original_call_is_exactly_reconstructed(){
        assert_eq!(rel_call(CALL_SITE,ORIGINAL_TARGET),ORIGINAL_CALL);
    }
    #[test]
    fn event_logging_has_strict_upper_bound(){
        assert!(should_log(0));
        assert!(should_log(MAX_LOGS-1));
        assert!(!should_log(MAX_LOGS));
    }
}
