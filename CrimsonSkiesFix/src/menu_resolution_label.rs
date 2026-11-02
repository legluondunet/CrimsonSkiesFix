//! Replace only the original Video Preferences "640 x 480" UI string
//! with the active launch monitor dimensions. Does not alter any game
//! mode IDs, 4:3 UI surfaces, HAL selection or persistent preferences.
//! Supported exact 1.02 EXE only, installed before menu construction.
use std::{ffi::c_void,ptr};
use crate::log;

const BASE:usize=0x0040_0000;
const UI_STRING:usize=0x0062_3A1C;
const STRING_REFERENCE:usize=0x0044_4ADE;
const ORIGINAL_REFERENCE:[u8;5]=[0x68,0x1c,0x3a,0x62,0x00];
const ORIGINAL_STRING:[u8;12]=*b"640 x 480\0\0\0";
const PAGE_READWRITE:u32=0x04;

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,len:usize,new:u32,old:*mut u32)->i32;
}

fn formatted_menu_label(width:u32,height:u32)->Option<[u8;12]>{
    if width<320 || height<240{return None;}
    let s=format!("{width}x{height}");
    if s.len()>11 || !s.is_ascii(){return None;}
    let mut bytes=[0u8;12];
    bytes[..s.len()].copy_from_slice(s.as_bytes());
    Some(bytes)
}

/// The string only appears as a list item's label; it is NEVER used as
/// the logical screen size. Fail closed on any preimage mismatch.
pub unsafe fn install(width:u32,height:u32)->bool{
    if GetModuleHandleW(ptr::null()) as usize!=BASE{
        log("MENU-RESOLUTION-LABEL: unsupported image base; original text retained");
        return false;
    }
    let Some(desired)=formatted_menu_label(width,height) else{
        log("MENU-RESOLUTION-LABEL: monitor dimensions do not fit legacy UI label");
        return false;
    };
    let instruction=std::slice::from_raw_parts(STRING_REFERENCE as *const u8,5);
    if instruction!=ORIGINAL_REFERENCE{
        log("MENU-RESOLUTION-LABEL: UI option constructor signature mismatch");
        return false;
    }
    let original=std::slice::from_raw_parts(UI_STRING as *const u8,12);
    if original!=ORIGINAL_STRING{
        log("MENU-RESOLUTION-LABEL: original 640 x 480 UI label mismatch");
        return false;
    }
    let mut protect_old=0u32;
    if VirtualProtect(UI_STRING as *mut c_void,12,PAGE_READWRITE,&mut protect_old)==0{
        log("MENU-RESOLUTION-LABEL: cannot unlock original UI string");
        return false;
    }
    ptr::copy_nonoverlapping(desired.as_ptr(),UI_STRING as *mut u8,12);
    let mut ignored=0u32;
    if VirtualProtect(UI_STRING as *mut c_void,12,protect_old,&mut ignored)==0{
        // Attempt rollback without leaving the legacy menu label partially
        // modified if the page-protection restore failed.
        let mut writable=0u32;
        if VirtualProtect(UI_STRING as *mut c_void,12,PAGE_READWRITE,&mut writable)!=0{
            ptr::copy_nonoverlapping(ORIGINAL_STRING.as_ptr(),UI_STRING as *mut u8,12);
            let mut restore_old=0u32;
            let _=VirtualProtect(UI_STRING as *mut c_void,12,protect_old,&mut restore_old);
        }
        log("MENU-RESOLUTION-LABEL: page protection restore failed; fallback attempted");
        return false;
    }
    log(&format!("MENU-RESOLUTION-LABEL: original 640 x 480 UI item renamed to {width}x{height}; game mode-table unchanged"));
    true
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_original_item_string_and_reference(){
        assert_eq!(ORIGINAL_STRING,b"640 x 480\0\0\0".to_owned());
        assert_eq!(ORIGINAL_REFERENCE[0],0x68);
        assert_eq!(&ORIGINAL_REFERENCE[1..],&(UI_STRING as u32).to_le_bytes());
    }
    #[test]
    fn native_desktop_size_fits_unchanged_12_byte_menu_slot(){
        assert_eq!(&formatted_menu_label(1920,1080).unwrap()[..10],b"1920x1080\0");
        assert_eq!(&formatted_menu_label(2560,1440).unwrap()[..9],b"2560x1440");
        assert!(formatted_menu_label(100,200).is_none());
        assert!(formatted_menu_label(4294967295,4294967295).is_none());
    }
}
