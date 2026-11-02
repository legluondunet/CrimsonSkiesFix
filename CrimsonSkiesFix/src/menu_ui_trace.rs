//! Bounded, read-only trace of the game's OWN GOS script interface.
//!
//! Our v0.1.80 test conclusively showed no native Win32 child ComboBoxes
//! in Preferences. The exact SHA-256 admitted v1.02 EXE registers these
//! internal APIs beside gosCallback. Replace only their original function
//! *registration pointers* at first authenticated DirectDrawCreate.
//!
//! The original handlers always run with exactly the same 3 arguments
//! and return DWORD. Never interpret script argument DWORDs as pointers,
//! alter script objects, intercept user input, or disable any widget here.
use std::{ffi::c_void,ptr,
    sync::atomic::{AtomicU32,Ordering}};
use crate::log;

const IMAGE_BASE:usize=0x0040_0000;
const UI_DATA_SITE:usize=0x0040_75F4;
const UI_DATA_ORIGINAL:usize=0x0040_93A0;
const UI_CONTROL_SITE:usize=0x0040_7612;
const UI_CONTROL_ORIGINAL:usize=0x0040_4960;
const UI_DATA_BYTES:[u8;5]=[0x68,0xa0,0x93,0x40,0x00];
const UI_CONTROL_BYTES:[u8;5]=[0x68,0x60,0x49,0x40,0x00];
const PAGE_EXECUTE_READWRITE:u32=0x40;
const MAX_DATA_EVENTS:u32=96;
const MAX_CONTROL_EVENTS:u32=72;

static DATA_LOGS:AtomicU32=AtomicU32::new(0);
static CONTROL_LOGS:AtomicU32=AtomicU32::new(0);
static LAST_DATA_SIGNATURE:AtomicU32=AtomicU32::new(u32::MAX);
static LAST_CONTROL_SIGNATURE:AtomicU32=AtomicU32::new(u32::MAX);
static VIDEO_LAST_STATE:AtomicU32=AtomicU32::new(u32::MAX);
static VIDEO_STATE_LOGS:AtomicU32=AtomicU32::new(0);
const MAX_VIDEO_STATES:u32=32;

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,length:usize,new:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,site:*const c_void,length:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

fn ui_data_code(code:u32)->bool{
    // uiData has TWO opcode dispatch stages:
    // 0x7D0..0x7F6 via direct jump table at 0x40F518,
    // 0x833 via a separate special branch,
    // 0x834..0x96D via compressed table 0x40F788/0x40F5B4,
    // and 0x9C9 via a later special branch.
    // The gap 0x7F7..0x832 goes to the original default handler.
    (0x7d0..=0x7f6).contains(&code) || code==0x833 ||
      (0x834..=0x96d).contains(&code) || code==0x9c9
}
fn ui_control_code(code:u32)->bool{
    matches!(code,6|0x85d|0x85e|0x860|0x868|0x899|0x96e
      |0x9c5|0x9c6|0x9c7|0x9c8|0xc1c|0xc1d|0xc1f
      |0xc22|0xc23|0xc24)
}
fn short_arg(value:u32)->Option<u32>{
    if value<=128 || value==u32::MAX || value==u32::MAX-1{
        Some(value)
    }else{None}
}
fn result_kind(value:u32)->&'static str{
    match value{
        0=>"zero",1=>"one",2..=128=>"small-integer",_=>"other"
    }
}
fn is_menu_trace_enabled()->bool{
    let c=crate::cfg();
    c.log_enabled && c.experimental_windowed_adapter &&
    crate::menu_video_probe::legacy_ui_active() &&
    crate::menu_scene::main_menu_active() &&
    // The GOS command 13 is the Preferences transaction boundary,
    // but does not conclusively identify the Video *tab*.
    crate::menu_gos_event::preferences_transaction_active()
}
fn should_emit(code:u32,arg:u32,previous:&AtomicU32,total:&AtomicU32,max:u32)->bool{
    let low=short_arg(arg).unwrap_or(0x7ff);
    let signature=(code<<12)|low;
    if previous.swap(signature,Ordering::Relaxed)==signature{return false;}
    total.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|x|{
        if x<max{Some(x+1)}else{None}
    }).is_ok()
}
type ScriptFn=unsafe extern "system" fn(usize,usize,*const u32)->u32;
unsafe extern "system" fn ui_data_passthrough(a:usize,b:usize,command:*const u32)->u32{
    // Read only what original uiData reads unconditionally:
    // op [command+0], parameter [command+4].
    let maybe=if is_menu_trace_enabled() && !command.is_null(){
        Some((ptr::read_unaligned(command),
              ptr::read_unaligned(command.add(1))))
    }else{None};
    // Disassembly: uiData opcode 0x855 enters 0x40CCE1, reads
    // [0x648370]/[0x64836C] when arg+4 == 0, writes those *pending*
    // video indices when arg+4 != 0. Record changes without touching
    // script output pointers or any other video option.
    let video_before=if matches!(maybe,Some((0x855,_))) &&
        crate::menu_video_probe::verified_video_fields()
    {
        Some((
            ptr::read_volatile(crate::menu_video_probe::PENDING_DEVICE as *const u32),
            ptr::read_volatile(crate::menu_video_probe::PENDING_MODE as *const u32)
        ))
    }else{None};
    let original:ScriptFn=std::mem::transmute(UI_DATA_ORIGINAL);
    let result=original(a,b,command);
    if let (Some((device_before,mode_before)),Some((0x855,arg)))=(video_before,maybe){
        let device_after=ptr::read_volatile(crate::menu_video_probe::PENDING_DEVICE as *const u32);
        let mode_after=ptr::read_volatile(crate::menu_video_probe::PENDING_MODE as *const u32);
        let signature=(device_after.min(0xffff)<<16)|mode_after.min(0xffff);
        let first_state=VIDEO_LAST_STATE.swap(signature,Ordering::Relaxed)!=signature;
        if first_state || device_before!=device_after || mode_before!=mode_after {
            if VIDEO_STATE_LOGS.fetch_update(Ordering::Relaxed,Ordering::Relaxed,
                |n|if n<MAX_VIDEO_STATES{Some(n+1)}else{None}).is_ok()
            {
                let operation=if arg==0{"get"}else{"set"};
                log(&format!(
                    "MENU-GOS-UI-VIDEO: uiData opcode=0x855 operation={operation} pending_device={device_before}->{device_after} pending_mode={mode_before}->{mode_after} [original handler preserved]"
                ));
            }
        }
    }
    if let Some((code,arg))=maybe{
        if ui_data_code(code) &&
            should_emit(code,arg,&LAST_DATA_SIGNATURE,&DATA_LOGS,MAX_DATA_EVENTS)
        {
            let param=short_arg(arg).map(|x|x.to_string())
                .unwrap_or_else(||"non-scalar".to_string());
            log(&format!("MENU-GOS-UI: function=uiData opcode={code:#x} arg_class={param} result_class={} [read-only pass-through]",result_kind(result)));
        }
    }
    result
}
unsafe extern "system" fn ui_control_passthrough(a:usize,b:usize,command:*const u32)->u32{
    let maybe=if is_menu_trace_enabled() && !command.is_null(){
        Some((ptr::read_unaligned(command),
              ptr::read_unaligned(command.add(1))))
    }else{None};
    let original:ScriptFn=std::mem::transmute(UI_CONTROL_ORIGINAL);
    let result=original(a,b,command);
    if let Some((code,arg))=maybe{
        if ui_control_code(code) &&
            should_emit(code,arg,&LAST_CONTROL_SIGNATURE,&CONTROL_LOGS,MAX_CONTROL_EVENTS)
        {
            let param=short_arg(arg).map(|x|x.to_string())
                .unwrap_or_else(||"non-scalar".to_string());
            log(&format!("MENU-GOS-UI: function=uiControl opcode={code:#x} arg_class={param} result_class={} [read-only pass-through]",result_kind(result)));
        }
    }
    result
}

unsafe fn patch_registration(site:usize,expected:[u8;5],new_callback:usize)->bool {
    if std::slice::from_raw_parts(site as *const u8,5)!=expected {
        log(&format!("MENU-GOS-UI: original registration preimage mismatch at RVA {:#x}; skipped",site-IMAGE_BASE));
        return false;
    }
    let mut old=0u32;
    if VirtualProtect(site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut old)==0{
        log("MENU-GOS-UI: cannot change script registration memory protection");
        return false;
    }
    // Original instruction is PUSH imm32. Write ONLY the 4-byte operand.
    let target=(new_callback as u32).to_le_bytes();
    ptr::copy_nonoverlapping(target.as_ptr(),(site+1) as *mut u8,4);
    let flushed=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5);
    let mut unused=0u32;
    let restored=VirtualProtect(site as *mut c_void,5,old,&mut unused);
    if flushed==0 || restored==0 {
        let mut temporary=0u32;
        if VirtualProtect(site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut temporary)!=0 {
            ptr::copy_nonoverlapping(expected.as_ptr(),site as *mut u8,5);
            let _=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5);
            let mut ignored=0u32;
            let _=VirtualProtect(site as *mut c_void,5,old,&mut ignored);
        }
        log("MENU-GOS-UI: script registration patch failure; original bytes restored where possible");
        return false;
    }
    true
}

/// Call at most once, after executable SHA-256 verification and before
/// GOS script functions register. Logging is diagnostic-only.
pub unsafe fn install()->(bool,bool){
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE {
        log("MENU-GOS-UI: image base mismatch; observers disabled");
        return (false,false);
    }
    if !crate::cfg().log_enabled {return (false,false);}
    let data=patch_registration(UI_DATA_SITE,UI_DATA_BYTES,ui_data_passthrough as usize);
    let control=patch_registration(UI_CONTROL_SITE,UI_CONTROL_BYTES,ui_control_passthrough as usize);
    log(&format!("MENU-GOS-UI: uiData_hook={data} uiControl_hook={control}; strict 3-argument pass-through, only during GOS Preferences transaction"));
    (data,control)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_expected_script_opcodes_traced(){
        assert!(ui_data_code(0x7d0));
        assert!(!ui_data_code(0x7f7));
        assert!(ui_data_code(0x833));
        assert!(ui_data_code(0x834));
        assert!(ui_data_code(0x85d));
        assert!(ui_data_code(0x96d));
        assert!(ui_data_code(0x9c9));
        assert!(!ui_data_code(0xc24));
        assert!(ui_data_code(0x855));
        assert!(ui_control_code(0x85d));
        assert!(ui_control_code(0xc24));
        assert!(!ui_control_code(0x7d1));
    }
    #[test]
    fn exact_registration_push_bytes(){
        assert_eq!(UI_DATA_BYTES[0],0x68);
        assert_eq!(&UI_DATA_BYTES[1..],&(UI_DATA_ORIGINAL as u32).to_le_bytes());
        assert_eq!(UI_CONTROL_BYTES[0],0x68);
        assert_eq!(&UI_CONTROL_BYTES[1..],&(UI_CONTROL_ORIGINAL as u32).to_le_bytes());
    }
    #[test]
    fn logging_deduplicates_and_limits(){
        let last=AtomicU32::new(u32::MAX);
        let count=AtomicU32::new(0);
        assert!(should_emit(0x7d0,1,&last,&count,2));
        assert!(!should_emit(0x7d0,1,&last,&count,2));
        assert!(should_emit(0x7d0,2,&last,&count,2));
        assert!(!should_emit(0x7d0,3,&last,&count,2));
    }
    #[test]
    fn no_pointer_like_values_logged_as_numeric_parameters(){
        assert_eq!(short_arg(2),Some(2));
        assert_eq!(short_arg(u32::MAX),Some(u32::MAX));
        assert_eq!(short_arg(0x648370),None);
    }
}