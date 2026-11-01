//! Exact single-EXE script transition observer. This is deliberately
//! limited to four verified CALL rel32 instructions in the game's menu
//! dispatcher. It never intercepts general memory, UI input, or COM.
use std::{ffi::{c_char,c_void},ptr,sync::atomic::{AtomicU8,Ordering}};
use crate::log;

const BASE:usize=0x400000;
const SCRIPT_LOADER:usize=0x406d30;
const MAIN_MENU_PTR:usize=0x61eb98;
const PREFERENCES_PTR:usize=0x61eb20;
const OTHER:u8=0;
const MAIN:u8=1;
const PREFERENCES:u8=2;
static CURRENT:AtomicU8=AtomicU8::new(OTHER);
const PAGE_EXECUTE_READWRITE:u32=0x40;

const SITES:[(usize,[u8;5]);4]=[
    (0x27df,[0xe8,0x4c,0x45,0,0]),
    (0x28bf,[0xe8,0x6c,0x44,0,0]),
    (0x2914,[0xe8,0x17,0x44,0,0]),
    (0x2954,[0xe8,0xd7,0x43,0,0]),
];

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,protect:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

fn scene_from_pointer(ptr:usize)->u8 {
    match ptr {MAIN_MENU_PTR=>MAIN,PREFERENCES_PTR=>PREFERENCES,_=>OTHER}
}
pub fn main_menu_active()->bool{CURRENT.load(Ordering::Acquire)==MAIN}
pub fn preferences_active()->bool{CURRENT.load(Ordering::Acquire)==PREFERENCES}

unsafe extern "C" fn observe_script(name:*const c_char,capacity:u32,flags:u32){
    let scene=scene_from_pointer(name as usize);
    let old=CURRENT.swap(scene,Ordering::AcqRel);
    if old!=scene {
        log(match scene {
            MAIN=>"MENU-SCENE: entered MainMenu; branding enabled",
            PREFERENCES=>"MENU-SCENE: entered Preferences; branding hidden",
            _=>"MENU-SCENE: other script; branding hidden"
        });
    }
    let original:unsafe extern "C" fn(*const c_char,u32,u32)=
        std::mem::transmute(SCRIPT_LOADER);
    original(name,capacity,flags);
}

fn rel_call(site:usize,target:usize)->[u8;5]{
    let displacement=(target as u32).wrapping_sub((site as u32).wrapping_add(5));
    let bytes=displacement.to_le_bytes();
    [0xe8,bytes[0],bytes[1],bytes[2],bytes[3]]
}

/// Never install without the exact authenticated image and all four
/// byte-exact preimages; no mutation of the on-disk EXE.
pub unsafe fn install()->bool {
    let module=GetModuleHandleW(ptr::null());
    if module as usize!=BASE {log("MENU-SCENE: unsupported image base");return false;}
    let site_bytes:Vec<(usize,[u8;5])>=SITES.iter().map(|&(rva,expected)|{
        (rva,expected)
    }).collect();
    for &(rva,expected) in &site_bytes{
        let image=std::slice::from_raw_parts((BASE+rva) as *const u8,5);
        if image!=expected {
            log(&format!("MENU-SCENE: expected CALL missing at RVA={rva:#x}; no hooks installed"));
            return false;
        }
    }
    // All four CALL sites fall in the same executable .text page.
    let page=(BASE+SITES[0].0)&!0xfff;
    let mut previous=0u32;
    if VirtualProtect(page as *mut c_void,0x1000,PAGE_EXECUTE_READWRITE,&mut previous)==0{
        log("MENU-SCENE: page protection denied");return false;
    }
    for &(rva,_) in &SITES {
        let instruction=rel_call(BASE+rva,observe_script as usize);
        ptr::copy_nonoverlapping(instruction.as_ptr(),(BASE+rva) as *mut u8,5);
    }
    let flush=FlushInstructionCache(GetCurrentProcess(),page as *const c_void,0x1000);
    let mut ignored=0u32;
    let restore=VirtualProtect(page as *mut c_void,0x1000,previous,&mut ignored);
    if flush==0||restore==0{
        let mut permit=0u32;
        if VirtualProtect(page as *mut c_void,0x1000,PAGE_EXECUTE_READWRITE,&mut permit)!=0 {
            for &(rva,expected) in &SITES{
                ptr::copy_nonoverlapping(expected.as_ptr(),(BASE+rva) as *mut u8,5);
            }
            let _=FlushInstructionCache(GetCurrentProcess(),page as *const c_void,0x1000);
            let mut unused=0u32;
            let _=VirtualProtect(page as *mut c_void,0x1000,previous,&mut unused);
        }
        CURRENT.store(OTHER,Ordering::Release);
        log("MENU-SCENE: installation failed; restoring original calls");
        return false;
    }
    log("MENU-SCENE: exact menu dispatcher hooks installed");
    true
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn script_name_identity_is_exact(){
        assert_eq!(scene_from_pointer(MAIN_MENU_PTR),MAIN);
        assert_eq!(scene_from_pointer(PREFERENCES_PTR),PREFERENCES);
        assert_eq!(scene_from_pointer(0),OTHER);
        assert_eq!(scene_from_pointer(MAIN_MENU_PTR+1),OTHER);
    }
    #[test]
    fn all_four_original_calls_target_one_script_loader(){
        for &(rva,old) in &SITES{
            assert_eq!(old[0],0xe8);
            let target=BASE+rva+5+
                i32::from_le_bytes(old[1..5].try_into().unwrap()) as usize;
            assert_eq!(target,SCRIPT_LOADER);
            let patched=rel_call(BASE+rva,SCRIPT_LOADER);
            assert_eq!(patched,old);
        }
    }
    #[test]
    fn sites_fit_in_one_page_and_are_disjoint(){
        assert!(SITES.iter().all(|(rva,_)|(rva&!0xfff)==0x2000));
        assert!(SITES.windows(2).all(|pair|pair[0].0+5<=pair[1].0));
    }
}
