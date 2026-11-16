//! Synthetic DLL, NOT the game fix. Tests a custom alias by the native loader.
#![cfg(all(windows,target_arch="x86"))]
use std::ffi::c_void;
const EXPECTED:i32=0x6A6A_1234;
#[no_mangle]
pub extern "system" fn DirectDrawCreate(_: *const c_void,_:*mut *mut c_void,_:*mut c_void)->i32 { EXPECTED }
#[no_mangle]
pub extern "system" fn DirectDrawEnumerateA(_: *mut c_void,_:*mut c_void)->i32 { EXPECTED }
#[no_mangle]
pub extern "system" fn CrimsonAliasFixtureMarker()->u32{0xA11A_5001}
#[no_mangle]
pub unsafe extern "system" fn DllMain(_: *mut c_void,_:u32,_:*mut c_void)->i32{1}
