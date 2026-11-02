//! Target the original game's actual Win32 ComboBox controls, not its
//! legacy static EXE strings or inferred mouse-hit rectangles.
//!
//! The supported v1.02 binary constructs the Video list with
//! SendMessageA(hwnd, CB_ADDSTRING, 0, LPCSTR) and CB_SETCURSEL.
//! We intercept only the authenticated EXE's SendMessageA import slot.
//! Game indices, item data and original message side effects are retained.
use std::{ffi::{c_char,c_void,CStr,CString},ptr,
    sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use crate::log;

const IMAGE_BASE:usize=0x0040_0000;
const SEND_MESSAGE_IAT:usize=0x00A2_0488;
const RESOLUTION_CALL:usize=0x0044_49AE;
const RESOLUTION_CALL_BYTES:[u8;6]=[0x8b,0x2d,0x88,0x04,0xa2,0x00];
const CB_ADDSTRING:u32=0x0143;
const PAGE_READWRITE:u32=0x04;

type Hwnd=*mut c_void;
type SendMessageA=unsafe extern "system" fn(Hwnd,u32,usize,isize)->isize;
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static RESOLUTION_HWND:AtomicUsize=AtomicUsize::new(0);
static GRAPHICS_HWND:AtomicUsize=AtomicUsize::new(0);
static LABEL:AtomicUsize=AtomicUsize::new(0); // owned once, never freed in process
static RESOLUTION_REPORTED:AtomicBool=AtomicBool::new(false);
static GRAPHICS_REPORTED:AtomicBool=AtomicBool::new(false);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,new:u32,old:*mut u32)->i32;
}
#[link(name="user32")]
extern "system" {
    fn GetClassNameA(hwnd:Hwnd,name:*mut c_char,capacity:i32)->i32;
    fn EnableWindow(hwnd:Hwnd,enabled:i32)->i32;
    fn IsWindowEnabled(hwnd:Hwnd)->i32;
    fn IsWindow(hwnd:Hwnd)->i32;
}
fn is_resolution_option(text:&str)->bool {
    // Actual game samples: "320 x 240", "640 x 480", "800 x 600",
    // "1024 x 768". Reject all unrelated menu option strings.
    let compact=text.trim().replace(' ', "");
    let Some((w,h))=compact.split_once('x') else{return false;};
    if !w.bytes().all(|c|c.is_ascii_digit()) || !h.bytes().all(|c|c.is_ascii_digit()){
        return false;
    }
    match (w.parse::<u32>(),h.parse::<u32>()){
        (Ok(w),Ok(h))=> (320..=8192).contains(&w)&&(240..=8192).contains(&h),
        _=>false
    }
}
fn is_graphics_option(text:&str)->bool {
    let text=text.trim().to_ascii_lowercase();
    text=="software render" || text=="directdraw hal"
}
fn is_native_combo(hwnd:Hwnd)->bool {
    if hwnd.is_null(){return false;}
    unsafe{
        let mut name=[0 as c_char;64];
        let n=GetClassNameA(hwnd,name.as_mut_ptr(),name.len() as i32);
        n>0 && name[..n as usize].iter()
            .map(|&c|(c as u8).to_ascii_lowercase())
            .eq(b"combobox".iter().copied())
    }
}
fn disable_exact_combo(hwnd:Hwnd,slot:&AtomicUsize,reported:&AtomicBool,kind:&str){
    unsafe{
        if slot.load(Ordering::Acquire)!=hwnd as usize {
            slot.store(hwnd as usize,Ordering::Release);
        }
        let _=EnableWindow(hwnd,0);
        if IsWindowEnabled(hwnd)==0 && !reported.swap(true,Ordering::Relaxed) {
            log(&format!("MENU-NATIVE-COMBO: {kind} native ComboBox disabled (original selection/item-data retained)"));
        }
    }
}
/// IAT wrapper is x86 stdcall just like SendMessageA. Every message
/// not recognized as one of the two exact ComboBox types is unmodified.
unsafe extern "system" fn intercept_send_message(
    hwnd:Hwnd,message:u32,wparam:usize,lparam:isize
)->isize {
    let original_addr=ORIGINAL.load(Ordering::Acquire);
    if original_addr==0{return 0;} // installation publishes original before patch
    let original:SendMessageA=std::mem::transmute(original_addr);
    if message!=CB_ADDSTRING || lparam==0 || !is_native_combo(hwnd){
        return original(hwnd,message,wparam,lparam);
    }
    let label=CStr::from_ptr(lparam as *const c_char).to_string_lossy();
    if is_resolution_option(&label) {
        let replacement=LABEL.load(Ordering::Acquire);
        let result=if replacement!=0 {
            original(hwnd,message,wparam,replacement as isize)
        }else{
            original(hwnd,message,wparam,lparam)
        };
        if result>=0 {
            disable_exact_combo(hwnd,&RESOLUTION_HWND,&RESOLUTION_REPORTED,"Resolution");
        }
        return result;
    }
    if is_graphics_option(&label){
        let result=original(hwnd,message,wparam,lparam);
        if result>=0 {
            disable_exact_combo(hwnd,&GRAPHICS_HWND,&GRAPHICS_REPORTED,"Graphics");
        }
        return result;
    }
    original(hwnd,message,wparam,lparam)
}

/// Called once after the supported EXE SHA gate; before the original menu
/// creates its native Win32 ComboBoxes. Do not enable on unknown binaries.
pub unsafe fn install(width:u32,height:u32)->bool {
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE{
        log("MENU-NATIVE-COMBO: executable image base mismatch");
        return false;
    }
    let preimage=std::slice::from_raw_parts(RESOLUTION_CALL as *const u8,
        RESOLUTION_CALL_BYTES.len());
    if preimage!=RESOLUTION_CALL_BYTES {
        log("MENU-NATIVE-COMBO: original SendMessageA import instruction mismatch");
        return false;
    }
    let label=format!("{width}x{height}");
    if !(320..=8192).contains(&width) || !(240..=8192).contains(&height) || label.len()>11{
        log("MENU-NATIVE-COMBO: monitor resolution unavailable/out of menu bounds");
        return false;
    }
    let module=GetModuleHandleA(c"USER32.dll".as_ptr());
    if module.is_null(){log("MENU-NATIVE-COMBO: USER32 unavailable");return false;}
    let address=GetProcAddress(module,c"SendMessageA".as_ptr()) as usize;
    if address==0{log("MENU-NATIVE-COMBO: SendMessageA unavailable");return false;}
    let site=SEND_MESSAGE_IAT as *mut usize;
    let original=ptr::read_volatile(site);
    if original!=address{
        log("MENU-NATIVE-COMBO: original SendMessageA import differs; native hook disabled");
        return false;
    }
    let Ok(label)=CString::new(label) else{return false;};
    // The label must outlive the import hook. Leaked once per process.
    let stable_label=label.into_raw() as usize;
    ORIGINAL.store(original,Ordering::Release);
    LABEL.store(stable_label,Ordering::Release);
    let mut old=0u32;
    if VirtualProtect(site.cast(),4,PAGE_READWRITE,&mut old)==0{
        log("MENU-NATIVE-COMBO: import pointer page protection failed");
        return false;
    }
    ptr::write_volatile(site,intercept_send_message as usize);
    let mut unused=0u32;
    if VirtualProtect(site.cast(),4,old,&mut unused)==0{
        let mut writable=0u32;
        if VirtualProtect(site.cast(),4,PAGE_READWRITE,&mut writable)!=0{
            ptr::write_volatile(site,original);
            let mut restored=0u32;
            let _=VirtualProtect(site.cast(),4,old,&mut restored);
        }
        log("MENU-NATIVE-COMBO: failed to restore import protection; original import restored where possible");
        return false;
    }
    log(&format!("MENU-NATIVE-COMBO: original SendMessageA import intercepted; target display={width}x{height}; awaiting actual Graphics/Resolution controls"));
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_resolution_values_are_classified(){
        for x in ["640 x 480","320 x 240","800 x 600","1024 x 768","1920x1080"]{
            assert!(is_resolution_option(x),"{x}");
        }
        for x in ["High","Medium","DirectDraw HAL","Software Render","1920x","0 x 0","640 x -480","640x480h"] {
            assert!(!is_resolution_option(x),"{x}");
        }
    }
    #[test]
    fn only_observed_graphics_labels_are_classified(){
        assert!(is_graphics_option("Software Render"));
        assert!(is_graphics_option("DirectDraw HAL"));
        assert!(!is_graphics_option("High"));
        assert!(!is_graphics_option("Shadows"));
    }
    #[test]
    fn exact_original_iat_instruction(){
        assert_eq!(&RESOLUTION_CALL_BYTES[2..],&(SEND_MESSAGE_IAT as u32).to_le_bytes());
    }
}