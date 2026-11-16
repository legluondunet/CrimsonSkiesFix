//! Windows x86 smoke test of the experimental WinMM forwarding DLL.
use std::{ffi::{c_char,c_void,OsStr},os::windows::ffi::OsStrExt,path::PathBuf};
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetModuleFileNameW(module:*mut c_void,path:*mut u16,size:u32)->u32;
    fn FreeLibrary(module:*mut c_void)->i32;
}
fn wide(s:&OsStr)->Vec<u16>{s.encode_wide().chain(Some(0)).collect()}
fn main() {
    let dll=PathBuf::from(std::env::args_os().nth(1)
        .expect("usage: winmm_loader_smoke <winmm.dll>"))
        .canonicalize().expect("experimental DLL must exist");
    let handle=unsafe{LoadLibraryW(wide(dll.as_os_str()).as_ptr())};
    assert!(!handle.is_null(),"LoadLibraryW failed");
    let mut buf=[0u16;32768];
    let n=unsafe{GetModuleFileNameW(handle,buf.as_mut_ptr(),buf.len() as u32)} as usize;
    assert!(n>0&&n<buf.len(),"cannot determine which WinMM was loaded");
    let loaded=String::from_utf16_lossy(&buf[..n]);
    let normalized=|s:&str|s.trim_start_matches(r"\\?\").to_ascii_lowercase();
    assert_eq!(normalized(&loaded),normalized(&dll.to_string_lossy()),
        "WinMM was redirected to a different file (possibly a KnownDLL)");
    for api in ["timeGetTime","mixerGetNumDevs","mciSendCommandA"] {
        let name=std::ffi::CString::new(api).unwrap();
        let ptr=unsafe{GetProcAddress(handle,name.as_ptr())};
        assert!(!ptr.is_null(),"missing WinMM export {api}");
        if api=="timeGetTime"||api=="mixerGetNumDevs" {
            let fun:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(ptr)};
            println!("WINMM-SMOKE {api}={}",unsafe{fun()});
        }
    }
    println!("WINMM-SMOKE PASS: {}",loaded);
    assert_ne!(unsafe{FreeLibrary(handle)},0);
}
