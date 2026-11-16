//! Synthetic PE32 imports real WinMM and (originally) DDRAW by name.
#![cfg(all(windows,target_arch="x86"))]
use std::{ffi::c_void,os::windows::ffi::OsStrExt};
#[link(name="ddraw")]
extern "system" {
 fn DirectDrawCreate(_: *const c_void,_:*mut *mut c_void,_:*mut c_void)->i32;
 fn DirectDrawEnumerateA(_: *mut c_void,_:*mut c_void)->i32;
}
#[link(name="winmm")]
extern "system" {fn timeGetTime()->u32;}
#[link(name="kernel32")]
extern "system" { fn GetModuleHandleW(n:*const u16)->*mut c_void; }
fn wide(s:&str)->Vec<u16>{std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()}
fn main(){
 let alias=wide("CSFIX.dll");let native=wide("winmm.dll");
 let winmm=unsafe{GetModuleHandleW(native.as_ptr())};
 let csfix=unsafe{GetModuleHandleW(alias.as_ptr())};
 assert!(!winmm.is_null(),"ordinary native WINMM.dll was not loaded by OS");
 assert!(!csfix.is_null(),"CSFIX.dll was not loaded during initial import resolution");
 let tick=unsafe{timeGetTime()};
 let first=unsafe{DirectDrawCreate(std::ptr::null(),std::ptr::null_mut(),std::ptr::null_mut())};
 let second=unsafe{DirectDrawEnumerateA(std::ptr::null_mut(),std::ptr::null_mut())};
 assert_eq!(first,0x6A6A_1234,"DirectDrawCreate did not target alias DLL");
 assert_eq!(second,0x6A6A_1234,"DirectDrawEnumerateA did not target alias DLL");
 println!("ALIAS-IMPORT-SMOKE PASS: native WinMM import unchanged, two DirectDraw imports dispatched to unique CSFIX.dll, tick={tick}");
}
