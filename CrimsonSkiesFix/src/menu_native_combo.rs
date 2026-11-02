//! Target the original game's actual Win32 ComboBox controls, not its
//! legacy static EXE strings or inferred mouse-hit rectangles.
//!
//! The supported v1.02 binary constructs the Video list with
//! SendMessageA(hwnd, CB_ADDSTRING, 0, LPCSTR) and CB_SETCURSEL.
//! We intercept only the authenticated EXE's SendMessageA import slot.
//! Game indices, item data and original message side effects are retained.
use std::{ffi::{c_char,c_void,CStr,CString},ptr,
    sync::atomic::{AtomicBool,AtomicU32,AtomicUsize,Ordering}};
use crate::log;

const IMAGE_BASE:usize=0x0040_0000;
const SEND_MESSAGE_IAT:usize=0x00A2_0488;
const RESOLUTION_CALL:usize=0x0044_49AE;
const RESOLUTION_CALL_BYTES:[u8;6]=[0x8b,0x2d,0x88,0x04,0xa2,0x00];
const CB_ADDSTRING:u32=0x0143;
const CB_GETCOUNT:u32=0x0146;
const CB_GETLBTEXT:u32=0x0148;
const CB_GETLBTEXTLEN:u32=0x0149;
const CB_GETCURSEL:u32=0x0147;
const CB_GETITEMDATA:u32=0x0150;
const CB_SETITEMDATA:u32=0x0151;
const CB_DELETESTRING:u32=0x0144;
const CB_INSERTSTRING:u32=0x014a;
const CB_SETCURSEL:u32=0x014e;
const GWL_STYLE:i32=-16;
const CBS_SORT:u32=0x0100;
const MAX_COMBOS:usize=80;
const MAX_ITEMS:usize=32;
const PAGE_READWRITE:u32=0x04;

type Hwnd=*mut c_void;
type SendMessageA=unsafe extern "system" fn(Hwnd,u32,usize,isize)->isize;
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static RESOLUTION_HWND:AtomicUsize=AtomicUsize::new(0);
static GRAPHICS_HWND:AtomicUsize=AtomicUsize::new(0);
static LABEL:AtomicUsize=AtomicUsize::new(0); // owned once, never freed in process
static RESOLUTION_REPORTED:AtomicBool=AtomicBool::new(false);
static GRAPHICS_REPORTED:AtomicBool=AtomicBool::new(false);
static IAT_CALLS:AtomicU32=AtomicU32::new(0);
static IAT_INSERTS:AtomicU32=AtomicU32::new(0);
static SCAN_COUNT:AtomicU32=AtomicU32::new(0);
static FRAME_COUNT:AtomicU32=AtomicU32::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,new:u32,old:*mut u32)->i32;
    fn GetCurrentProcessId()->u32;
}
#[link(name="user32")]
extern "system" {
    fn GetClassNameA(hwnd:Hwnd,name:*mut c_char,capacity:i32)->i32;
    fn EnableWindow(hwnd:Hwnd,enabled:i32)->i32;
    fn IsWindowEnabled(hwnd:Hwnd)->i32;
    fn IsWindow(hwnd:Hwnd)->i32;
    fn EnumWindows(callback:unsafe extern "system" fn(Hwnd,isize)->i32,lparam:isize)->i32;
    fn EnumChildWindows(parent:Hwnd,callback:unsafe extern "system" fn(Hwnd,isize)->i32,lparam:isize)->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,pid:*mut u32)->u32;
    fn GetWindowLongA(hwnd:Hwnd,index:i32)->i32;
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
/// Enumerate existing game-owned child ComboBoxes. This catches controls
/// created before the first DirectDrawCreate (and therefore before the
/// imported SendMessageA hook could observe their CB_ADDSTRING messages).
/// Identification comes from the *full native item list*, not coordinates.
struct Scan {
    pid:u32,
    visited:u32,
    combos:u32,
    graphics:u32,
    resolution:u32,
    invalid:u32,
}
fn current_original()->Option<SendMessageA>{
    let address=ORIGINAL.load(Ordering::Acquire);
    if address==0{None}else{Some(unsafe{std::mem::transmute(address)})}
}
unsafe fn read_combo_items(hwnd:Hwnd,sender:SendMessageA)->Option<Vec<(usize,String)>>{
    let count=sender(hwnd,CB_GETCOUNT,0,0);
    if count<=0 || count as usize>MAX_ITEMS{return None;}
    let mut items=Vec::new();
    for index in 0..count as usize{
        let len=sender(hwnd,CB_GETLBTEXTLEN,index,0);
        if len<0 || len>=96{return None;}
        let mut buf=[0i8;100];
        let got=sender(hwnd,CB_GETLBTEXT,index,buf.as_mut_ptr() as isize);
        if got<0 || got>=100{return None;}
        let word=CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        items.push((index,word));
    }
    Some(items)
}
fn classify_combo_items(items:&[(usize,String)])->Option<&'static str>{
    let graphics=items.iter().any(|(_,x)|x.eq_ignore_ascii_case("DirectDraw HAL")) &&
                 items.iter().any(|(_,x)|x.eq_ignore_ascii_case("Software Render"));
    if graphics{return Some("Graphics");}
    let resolutions=items.iter().filter(|(_,x)|is_resolution_option(x)).count();
    if resolutions>0 && resolutions==items.len(){return Some("Resolution");}
    None
}
unsafe fn replace_selected_resolution(hwnd:Hwnd,sender:SendMessageA,items:&[(usize,String)]) -> bool {
    if GetWindowLongA(hwnd,GWL_STYLE) as u32&CBS_SORT!=0{return false;}
    let selected=sender(hwnd,CB_GETCURSEL,0,0);
    if selected<0{return false;}
    let Some((index,old_label))=items.iter().find(|(i,_)|*i==selected as usize) else{return false;};
    if !is_resolution_option(old_label){return false;}
    let replacement=LABEL.load(Ordering::Acquire);
    if replacement==0{return false;}
    let data=sender(hwnd,CB_GETITEMDATA,*index,0);
    let deleted=sender(hwnd,CB_DELETESTRING,*index,0);
    if deleted<0{return false;}
    let inserted=sender(hwnd,CB_INSERTSTRING,*index,replacement as isize);
    if inserted<0{
        if let Ok(original)=CString::new(old_label.as_str()){
            let restored=sender(hwnd,CB_INSERTSTRING,*index,original.as_ptr() as isize);
            if restored>=0{
                let _=sender(hwnd,CB_SETITEMDATA,restored as usize,data);
                let _=sender(hwnd,CB_SETCURSEL,restored as usize,0);
            }
        }
        return false;
    }
    let _=sender(hwnd,CB_SETITEMDATA,inserted as usize,data);
    let _=sender(hwnd,CB_SETCURSEL,inserted as usize,0);
    inserted as usize==*index
}
unsafe extern "system" fn each_game_combo(hwnd:Hwnd,context:isize)->i32{
    let scan=&mut *(context as *mut Scan);
    if scan.visited>=512{return 0;}
    scan.visited+=1;
    if !is_native_combo(hwnd){return 1;}
    scan.combos+=1;
    let Some(sender)=current_original() else{return 0;};
    let Some(items)=read_combo_items(hwnd,sender) else{scan.invalid+=1;return 1;};
    match classify_combo_items(&items){
        Some("Graphics")=>{
            scan.graphics+=1;
            disable_exact_combo(hwnd,&GRAPHICS_HWND,&GRAPHICS_REPORTED,"Graphics");
        }
        Some("Resolution")=>{
            scan.resolution+=1;
            let renamed=replace_selected_resolution(hwnd,sender,&items);
            if renamed{log("MENU-NATIVE-COMBO: Resolution selected item updated using original CB messages and item data preserved");}
            disable_exact_combo(hwnd,&RESOLUTION_HWND,&RESOLUTION_REPORTED,"Resolution");
        }
        _=>{}
    }
    1
}
unsafe extern "system" fn each_game_root(hwnd:Hwnd,context:isize)->i32{
    let scan=&mut *(context as *mut Scan);
    let mut pid=0u32;
    let _=GetWindowThreadProcessId(hwnd,&mut pid);
    if pid!=scan.pid{return 1;}
    // Process-owned top-level windows may themselves be ComboBoxes,
    // and enumerating their descendants finds child controls.
    let _=each_game_combo(hwnd,context);
    let _=EnumChildWindows(hwnd,each_game_combo,context);
    1
}
pub fn scan_on_menu_frame(){
    if !crate::menu_scene::main_menu_active(){return;}
    let count=FRAME_COUNT.fetch_add(1,Ordering::Relaxed);
    if count%90!=0{return;}
    unsafe{
        if current_original().is_none(){return;}
        let mut scan=Scan{pid:GetCurrentProcessId(),visited:0,combos:0,
                          graphics:0,resolution:0,invalid:0};
        let _=EnumWindows(each_game_root,&mut scan as *mut Scan as isize);
        let attempt=SCAN_COUNT.fetch_add(1,Ordering::Relaxed);
        // Only bounded diagnostic inventory. Never print HWNDs or
        // arbitrary window texts, and never inspect a foreign process.
        if attempt<8 || (scan.graphics>0 && scan.resolution>0 && attempt==8){
            log(&format!(
                "MENU-NATIVE-COMBO: owned-window-scan={} windows={} native_combos={} recognized_graphics={} recognized_resolution={} uninspectable={} iat_calls={} iat_addstring={}",
                attempt+1,scan.visited,scan.combos,scan.graphics,scan.resolution,scan.invalid,
                IAT_CALLS.load(Ordering::Relaxed),IAT_INSERTS.load(Ordering::Relaxed)
            ));
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
    IAT_CALLS.fetch_add(1,Ordering::Relaxed);
    if message==CB_ADDSTRING {
        let count=IAT_INSERTS.fetch_add(1,Ordering::Relaxed);
        if count<8 && crate::cfg().log_enabled{
            let class=if is_native_combo(hwnd){"ComboBox"}else{"other-or-invalid"};
            log(&format!("MENU-NATIVE-COMBO: intercepted CB_ADDSTRING class={class} nonnull_text={}",lparam!=0));
        }
    }
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
    fn whole_list_classification_rejects_single_unrelated_item(){
        let gfx=vec![(0,"Software Render".into()),(1,"DirectDraw HAL".into())];
        let modes=vec![(0,"640 x 480".into())];
        let other=vec![(0,"High".into()),(1,"Low".into())];
        let mixed=vec![(0,"640 x 480".into()),(1,"High".into())];
        assert_eq!(classify_combo_items(&gfx),Some("Graphics"));
        assert_eq!(classify_combo_items(&modes),Some("Resolution"));
        assert_eq!(classify_combo_items(&other),None);
        assert_eq!(classify_combo_items(&mixed),None);
    }
    #[test]
    fn exact_original_iat_instruction(){
        assert_eq!(&RESOLUTION_CALL_BYTES[2..],&(SEND_MESSAGE_IAT as u32).to_le_bytes());
    }
}