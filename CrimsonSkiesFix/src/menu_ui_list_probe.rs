//! Precise read-only examination of original GOS uiData list labels.
//! 0x845 enumerates resolution modes and produces "W x H" via
//! formatter resource 0x206. 0x847 enumerates a *different* 5-choice
//! localized list (resource base 0x72); widget ownership unknown.
//! 0x843 walks an unrelated 360-byte record table (widget unknown).
//! 0x844 enumerates the game's Graphics-device labels from the pending
//! video-settings structure, a 97-byte string array paired with the
//! pending Graphics-device count. This ownership is established by
//! direct original-code enumeration and the owner's 0.1.87 Wine log.
//! The visual Video objects are HVA (Graphics) and IVA (Resolution).
//! Disabled-state semantics were verified in the owner's original GOS
//! CTL.SCRIPT and SHAREDITEMS.SCRIPT; this module remains read-only.
//! Observe the original counts and indices only, without editing UI.
//! Strict preimage checks; never write to game mode indices or text.
use std::{ptr,sync::atomic::{AtomicU32,Ordering}};
use crate::log;
const MAX:u32=32;
const COUNT:usize=0x0064_b498;
const MODES:usize=0x0064_b49c;
const DEVICE_RECORD_COUNT:usize=0x0064_ab3c;
const GRAPHICS_LIST_COUNT:usize=0x0064_8374;
static MODE_CALLS:AtomicU32=AtomicU32::new(0);
static FIVE_CALLS:AtomicU32=AtomicU32::new(0);
static DEVICE_CALLS:AtomicU32=AtomicU32::new(0);
static GRAPHICS_CALLS:AtomicU32=AtomicU32::new(0);

pub fn handles(code:u32)->bool{matches!(code,0x843|0x844|0x845|0x847)}
fn is_count_request(index:u32)->bool{index==u32::MAX}

fn mode_dimensions(word:u32)->(u32,u32){(word&0xffff,word>>16)}
fn safe_count(count:u32)->bool{(1..=64).contains(&count)}
fn verified(code:u32)->bool{
    unsafe{
        match code{
            0x843=>
                std::slice::from_raw_parts(0x0040_ce17 as *const u8,6)==[0x8b,0x03,0x3b,0xc2,0x75,0x17] &&
                std::slice::from_raw_parts(0x0040_ce1d as *const u8,6)==[0x8b,0x15,0x3c,0xab,0x64,0x00] &&
                std::slice::from_raw_parts(0x0040_ce34 as *const u8,6)==[0x8b,0x0d,0x2c,0xab,0x64,0x00],
            0x844=>
                std::slice::from_raw_parts(0x0040_cf58 as *const u8,6)==[0x8b,0x03,0x3b,0xc2,0x75,0x16] &&
                std::slice::from_raw_parts(0x0040_cf5e as *const u8,5)==[0xa1,0x74,0x83,0x64,0x00] &&
                std::slice::from_raw_parts(0x0040_cf7a as *const u8,7)==[0x8d,0x94,0x01,0x9c,0x83,0x64,0x00],
            0x845=>
                std::slice::from_raw_parts(0x0040_cef7 as *const u8,6)==[0x8b,0x03,0x3b,0xc2,0x75,0x17] &&
                std::slice::from_raw_parts(0x0040_cf14 as *const u8,7)==[0x8b,0x04,0x85,0x9c,0xb4,0x64,0x00] &&
                std::slice::from_raw_parts(0x0040_cf2b as *const u8,5)==[0x68,0x06,0x02,0x00,0x00],
            0x847=>
                std::slice::from_raw_parts(0x0040_cfe4 as *const u8,6)==[0x8b,0x03,0x3b,0xc2,0x75,0x15] &&
                std::slice::from_raw_parts(0x0040_cfed as *const u8,6)==[0xc7,0x03,0x05,0x00,0x00,0x00] &&
                std::slice::from_raw_parts(0x0040_cfff as *const u8,5)==[0x6a,0x01,0x83,0xc0,0x72],
            _=>false
        }
    }
}
/// Take the index before the original callback writes its count/result.
/// This pointer is exactly the same one the original handler dereferences.
pub unsafe fn before(code:u32,descriptor:u32)->Option<u32>{
    if descriptor==0 || !verified(code){return None;}
    Some(ptr::read_volatile(descriptor as usize as *const u32))
}
pub unsafe fn after(code:u32,descriptor:u32,index_before:u32){
    if !verified(code){return;}
    let count=match code{
        0x843=>&DEVICE_CALLS,0x844=>&GRAPHICS_CALLS,
        0x845=>&MODE_CALLS,_=>&FIVE_CALLS
    };
    if count.fetch_add(1,Ordering::Relaxed)>=MAX{return;}
    let after=ptr::read_volatile(descriptor as usize as *const u32);
    if matches!(code,0x843|0x844){
        // Both handlers use -1 to retrieve their list count and an
        // entry index to request game-owned text. Read only the count
        // DWORD whose use was byte-verified above; NEVER dereference
        // the device/entry table or the GOS output pointers.
        let count_site=if code==0x843{DEVICE_RECORD_COUNT}else{GRAPHICS_LIST_COUNT};
        let identity=if code==0x844{"graphics-devices"}else{"unidentified-device-records"};
        let original_count=ptr::read_volatile(count_site as *const u32);
        let action=if is_count_request(index_before){"count"}else{"label"};
        if original_count<=64 {
            log(&format!(
                "MENU-GOS-UI-LIST: opcode={code:#x} action={action} index_in={index_before} index_out={after} original_list_count={original_count} list={identity} [read-only; GOS dropdown disabled state managed separately]"
            ));
        }else{
            log(&format!(
                "MENU-GOS-UI-LIST: opcode={code:#x} action={action} original_list_count=out-of-range list={identity} [read-only; skipped]"
            ));
        }
    }else if code==0x845{
        let num=ptr::read_volatile(COUNT as *const u32);
        if !safe_count(num){return;}
        let mode=if index_before<num{
            let word=ptr::read_volatile((MODES+index_before as usize*4) as *const u32);
            let (width,height)=mode_dimensions(word);
            if (320..=8192).contains(&width) && (240..=8192).contains(&height){
                format!("{width}x{height}")
            }else{"invalid".to_owned()}
        }else{"not-an-entry".to_owned()};
        let action=if index_before==u32::MAX{"count"}else{"label"};
        log(&format!(
            "MENU-GOS-UI-LIST: opcode=0x845 action={action} index_in={index_before} index_out={after} original_mode_count={num} original_mode_label={mode} [read-only]"
        ));
    }else{
        let action=if index_before==u32::MAX{"count"}else{"label"};
        let resource=if index_before<5{
            format!("0x{:x}",0x72+index_before)
        }else{"not-an-entry".to_owned()};
        log(&format!(
            "MENU-GOS-UI-LIST: opcode=0x847 action={action} index_in={index_before} index_out={after} localized_id={resource} [five-item list; widget unknown; read-only]"
        ));
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn packed_resolution_format_is_not_a_mode_index(){
        assert_eq!(mode_dimensions((1080<<16)|1920),(1920,1080));
        assert_eq!(mode_dimensions((480<<16)|640),(640,480));
        assert!(safe_count(1));
        assert!(safe_count(64));
        assert!(!safe_count(0));
        assert!(!safe_count(65));
        assert!(handles(0x843));
        assert!(handles(0x844));
        assert!(handles(0x845));
        assert!(handles(0x847));
        assert!(!handles(0x842));
        assert!(is_count_request(u32::MAX));
        assert!(!is_count_request(0));
    }
}
