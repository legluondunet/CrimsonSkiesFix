//! Minimal alias for ONLY the two DirectDraw imports of the approved PE32 game.
//! The launcher redirects the DDRAW.dll import name to CSFIX.dll ONLY in the
//! suspended child image. WinMM is always owned by Windows/Wine, untouched.
//! DLL_PROCESS_ATTACH does not load, inspect, hook, allocate or synchronize.
#![cfg(windows)]
#[cfg(not(all(target_arch="x86",target_env="msvc")))]
compile_error!("The DirectDraw import alias is only supported on native Windows x86");
use std::{
 ffi::{c_char,c_void,OsString},os::windows::ffi::{OsStrExt,OsStringExt},
 path::PathBuf,ptr,sync::OnceLock,
};
#[link(name="kernel32")]
extern "system" {
 fn GetModuleHandleExW(flags:u32,address:*const u16,module:*mut *mut c_void)->i32;
 fn GetModuleFileNameW(module:*mut c_void,path:*mut u16,size:u32)->u32;
 fn GetModuleHandleW(name:*const u16)->*mut c_void;
 fn LoadLibraryW(path:*const u16)->*mut c_void;
 fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}
type Hr=i32;
const DDERR_GENERIC:Hr=0x8876_0000u32 as i32;
type Create=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*mut c_void)->Hr;
type Enumerate=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type Attach=unsafe extern "system" fn(usize,usize,u32)->i32;
struct Provider{create:usize,enumerate:usize}
static PROVIDER:OnceLock<Option<Provider>>=OnceLock::new();
fn self_path()->Option<PathBuf>{
 let mut module=ptr::null_mut();
 let flags=0x0000_0004u32|0x0000_0002u32; // FROM_ADDRESS | UNCHANGED_REFCOUNT
 if unsafe{GetModuleHandleExW(flags,DirectDrawCreate as *const () as *const u16,&mut module)}==0
     || module.is_null(){return None;}
 let mut path=[0u16;32768];
 let count=unsafe{GetModuleFileNameW(module,path.as_mut_ptr(),path.len() as u32)} as usize;
 if count==0 || count>=path.len(){return None;}
 Some(PathBuf::from(OsString::from_wide(&path[..count])))
}
fn load_provider()->Option<Provider>{
 // Invoked on FIRST GAME DirectDraw function call, never from our DllMain.
 // A third-party DirectDraw wrapper, when present, retains ordinary priority.
 let mut ddraw=unsafe{GetModuleHandleW("ddraw.dll\0".encode_utf16().collect::<Vec<u16>>().as_ptr())};
 if ddraw.is_null(){
     let name:Vec<u16>="ddraw.dll\0".encode_utf16().collect();
     ddraw=unsafe{LoadLibraryW(name.as_ptr())};
 }
 if ddraw.is_null(){return None;}
 let resolve=|dll:*mut c_void,name:&[u8]|unsafe{GetProcAddress(dll,name.as_ptr().cast()) as usize};
 let create=resolve(ddraw,b"DirectDrawCreate\0");
 let enumerate=resolve(ddraw,b"DirectDrawEnumerateA\0");
 if create==0 || enumerate==0 ||
      create==DirectDrawCreate as *const () as usize ||
      enumerate==DirectDrawEnumerateA as *const () as usize{return None;}
 let core_path=self_path()?.parent()?.join("crimsonskiesfix").join("CrimsonSkiesFix.dll");
 let path=core_path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
 let core=unsafe{LoadLibraryW(path.as_ptr())};
 if core.is_null(){return None;}
 let init=resolve(core,b"CrimsonSkiesFixInstallBackend\0");
 let forwarded_create=resolve(core,b"CrimsonSkiesFixDDrawCreate\0");
 let forwarded_enum=resolve(core,b"CrimsonSkiesFixDDrawEnumerateA\0");
 if init==0||forwarded_create==0||forwarded_enum==0{return None;}
 let attach:Attach=unsafe{std::mem::transmute(init)};
 // 7 means both original DirectDraw backend functions are authenticated
 // and forwarded to the core. No game EXE IAT slot is patched in this mode.
 if unsafe{attach(create,enumerate,7)}!=1{return None;}
 Some(Provider{create:forwarded_create,enumerate:forwarded_enum})
}
#[no_mangle]
pub unsafe extern "system" fn DirectDrawCreate(
 guid:*const c_void,output:*mut *mut c_void,outer:*mut c_void)->Hr{
 let Some(provider)=PROVIDER.get_or_init(load_provider).as_ref() else{
    if !output.is_null(){unsafe{*output=ptr::null_mut()};}
    return DDERR_GENERIC;
 };
 let call:Create=unsafe{std::mem::transmute(provider.create)};
 unsafe{call(guid,output,outer)}
}
#[no_mangle]
pub unsafe extern "system" fn DirectDrawEnumerateA(
 callback:*mut c_void,context:*mut c_void)->Hr{
 let Some(provider)=PROVIDER.get_or_init(load_provider).as_ref() else{return DDERR_GENERIC;};
 let call:Enumerate=unsafe{std::mem::transmute(provider.enumerate)};
 unsafe{call(callback,context)}
}
#[no_mangle]
pub unsafe extern "system" fn DllMain(_: *mut c_void,_reason:u32,_reserved:*mut c_void)->i32{
 // No LoadLibrary, GetProcAddress, filesystem operations or process IAT edits.
 1
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn relative_core_subdirectory_is_fixed(){
    let dir=PathBuf::from(r"C:\Games\crimsonskies");
    assert_eq!(dir.join("crimsonskiesfix").join("CrimsonSkiesFix.dll").file_name().unwrap(),"CrimsonSkiesFix.dll");
 }
}
