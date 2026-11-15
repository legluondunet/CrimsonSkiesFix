//! Tests Windows PE loader resolution of an app-local winmm.dll, not explicit LoadLibrary.
use std::{ffi::{c_char,c_void},path::PathBuf};
#[link(name="winmm")]
extern "system" {
    fn timeGetTime()->u32;
    fn mixerGetNumDevs()->u32;
    fn timeBeginPeriod(period:u32)->u32;
    fn timeEndPeriod(period:u32)->u32;
}
#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetModuleFileNameW(module:*mut c_void,path:*mut u16,size:u32)->u32;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}
fn normalize(p:&str)->String{
    p.trim_start_matches(r"\\?\").replace('/',"\\").to_ascii_lowercase()
}
fn main(){
    let exe=std::env::current_exe().expect("current exe");
    let expected=exe.parent().unwrap().join("winmm.dll");
    let name:Vec<u16>="winmm.dll".encode_utf16().chain(Some(0)).collect();
    let handle=unsafe{GetModuleHandleW(name.as_ptr())};
    assert!(!handle.is_null(),"static WINMM import was not loaded");
    let mut path=[0u16;32768];
    let n=unsafe{GetModuleFileNameW(handle,path.as_mut_ptr(),path.len() as u32)} as usize;
    assert!(n>0&&n<path.len());
    let loaded=PathBuf::from(String::from_utf16_lossy(&path[..n]));
    assert_eq!(normalize(&loaded.to_string_lossy()),normalize(&expected.to_string_lossy()),
        "Windows loaded system WinMM instead of app-local WinMM");
    let probe=unsafe{GetProcAddress(handle,c"CrimsonSkiesFixBootstrapStatus".as_ptr())};
    assert!(!probe.is_null(),"bootstrap status export not found");
    let status:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(probe)};
    assert_eq!(unsafe{status()},1,
        "the synthetic host should be rejected by the exact game PE filter");
    let time=unsafe{timeGetTime()};
    let mixer=unsafe{mixerGetNumDevs()};
    let period=unsafe{timeBeginPeriod(1)};
    if period==0 {let _=unsafe{timeEndPeriod(1)};}
    println!("WINMM-APPLOCAL PASS: path={} timeGetTime={} mixerGetNumDevs={} timeBeginPeriod={}",
        loaded.display(),time,mixer,period);
}
