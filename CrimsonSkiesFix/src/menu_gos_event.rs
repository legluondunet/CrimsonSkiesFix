//! Read-only observer of the game's registered gosCallback events.
//!
//! In the exact supported 1.02 image, 0x004075D6 is a PUSH imm32 of
//! the original callback 0x00407670, used by the script registration
//! call at 0x004075E0. We replace only the registration pointer in
//! memory so the engine still calls the original callback with its
//! complete arguments and return value. No menu action is blocked here.
//! This complements the verified command-15 Resolution backend guard.
use std::{ffi::c_void,ptr,sync::atomic::{AtomicU32,Ordering}};
use crate::log;
const IMAGE_BASE:usize=0x0040_0000;
const REGISTRATION_PUSH:usize=0x0040_75D6;
const ORIGINAL_BYTES:[u8;5]=[0x68,0x70,0x76,0x40,0x00];
const ORIGINAL_CALLBACK:usize=0x0040_7670;
const SELECTED_DEVICE:usize=0x0064_B490;
const SELECTED_MODE:usize=0x0064_B48C;
const PAGE_EXECUTE_READWRITE:u32=0x40;
const MAX_EVENTS:u32=80;
static EVENTS:AtomicU32=AtomicU32::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,length:usize,protection:u32,previous:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

fn include_event(command:u32)->bool{(8..=30).contains(&command)}
fn should_trace(command:u32,n:u32,ui:bool)->bool{
    include_event(command)&&n<MAX_EVENTS&&ui
}
unsafe extern "system" fn observe_callback(
    first:usize,second:usize,third:*const u32
)->u32{
    // The game's original callback reads [third] as the GOS command ID.
    // After the original call the event structure may be invalid, so
    // inspect the ID first, without dereferencing nested object pointers.
    let command=if third.is_null(){u32::MAX}else{ptr::read_unaligned(third)};
    let record=should_trace(command,EVENTS.load(Ordering::Relaxed),
        crate::menu_video_probe::legacy_ui_active());
    let before=if record{Some((
        ptr::read_volatile(SELECTED_DEVICE as *const u32),
        ptr::read_volatile(SELECTED_MODE as *const u32)
    ))}else{None};
    let original:unsafe extern "system" fn(usize,usize,*const u32)->u32=
        std::mem::transmute(ORIGINAL_CALLBACK);
    let result=original(first,second,third);
    if let Some((device_before,mode_before))=before{
        let index=EVENTS.fetch_add(1,Ordering::Relaxed);
        if index<MAX_EVENTS && crate::cfg().log_enabled {
            let device_after=ptr::read_volatile(SELECTED_DEVICE as *const u32);
            let mode_after=ptr::read_volatile(SELECTED_MODE as *const u32);
            log(&format!(
                "MENU-GOS-EVENT: command={} result={} device={}->{} mode_index={}->{} ui_4by3=true [pass-through]",
                command,result,device_before,device_after,mode_before,mode_after
            ));
        }
    }
    result
}
pub unsafe fn install()->bool{
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE{
        log("MENU-GOS-EVENT: image base mismatch; observer disabled");
        return false;
    }
    if std::slice::from_raw_parts(REGISTRATION_PUSH as *const u8,5)!=ORIGINAL_BYTES{
        log("MENU-GOS-EVENT: original gosCallback registration byte mismatch; observer disabled");
        return false;
    }
    let mut protection=0u32;
    if VirtualProtect(REGISTRATION_PUSH as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut protection)==0{
        log("MENU-GOS-EVENT: cannot change registration page protection; disabled");
        return false;
    }
    // PUSH imm32 has the same 5-byte length; no instruction boundaries move.
    let replacement=(observe_callback as usize as u32).to_le_bytes();
    ptr::copy_nonoverlapping(replacement.as_ptr(),
        (REGISTRATION_PUSH+1) as *mut u8,4);
    let flush=FlushInstructionCache(GetCurrentProcess(),
        REGISTRATION_PUSH as *const c_void,5);
    let mut restore_previous=0u32;
    let restore=VirtualProtect(REGISTRATION_PUSH as *mut c_void,5,protection,&mut restore_previous);
    if flush==0 || restore==0 {
        let mut writable=0u32;
        if VirtualProtect(REGISTRATION_PUSH as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut writable)!=0{
            ptr::copy_nonoverlapping(ORIGINAL_BYTES.as_ptr(),
                REGISTRATION_PUSH as *mut u8,5);
            let _=FlushInstructionCache(GetCurrentProcess(),REGISTRATION_PUSH as *const c_void,5);
            let mut ignored=0u32;
            let _=VirtualProtect(REGISTRATION_PUSH as *mut c_void,5,protection,&mut ignored);
        }
        log("MENU-GOS-EVENT: registration hook failed; original bytes restored where possible");
        return false;
    }
    log("MENU-GOS-EVENT: exact callback registration observer installed; existing registrations are not retroactively changed");
    true
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_registration_preimage(){
        assert_eq!(ORIGINAL_BYTES[0],0x68);
        assert_eq!(&ORIGINAL_BYTES[1..],&(ORIGINAL_CALLBACK as u32).to_le_bytes());
    }
    #[test]
    fn bounded_video_related_menu_callbacks(){
        assert!(should_trace(14,0,true));
        assert!(should_trace(15,79,true));
        assert!(!should_trace(14,80,true));
        assert!(!should_trace(14,0,false));
        assert!(!should_trace(1,0,true));
        assert!(!should_trace(31,0,true));
    }
}
