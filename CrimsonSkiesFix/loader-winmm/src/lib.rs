//! App-local WinMM shim. No game patch/config/file IO or runtime LoadLibrary
//! runs from DllMain: only the exact-EXE PE fingerprint and DDRAW IAT redirect.
//! The game-fix runtime loads on the first intercepted DirectDraw call.
#![cfg(windows)]
#[cfg(not(all(target_arch="x86",target_env="msvc")))]
compile_error!("The WinMM loader requires i686-pc-windows-msvc");

#[path="../../winmm-prototype/src/lib.rs"]
mod winmm_proxy;
#[path="../../src/winmm_compat.rs"]
mod winmm_compat;
#[path="../../src/winmm_bootstrap.rs"]
mod winmm_bootstrap;

use std::{
    ffi::{c_char,c_void,OsString},
    os::windows::ffi::{OsStrExt,OsStringExt},
    path::PathBuf,
    ptr,
    sync::OnceLock,
};

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleExW(flags:u32,address:*const u16,module:*mut *mut c_void)->i32;
    fn GetModuleFileNameW(module:*mut c_void,path:*mut u16,size:u32)->u32;
    fn LoadLibraryW(path:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}
type Hr=i32;
const DDERR_GENERIC:Hr=0x8876_0000u32 as i32;
type CreateFn=unsafe extern "system" fn(*const c_void,*mut *mut c_void,*mut c_void)->Hr;
type EnumerateFn=unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr;
type AttachFn=unsafe extern "system" fn(usize,usize,u32)->i32;

struct Backend {create:usize,enumerate:usize}
// Never unload the verified DLL while any game-owned COM object survives.
static BACKEND:OnceLock<Option<Backend>>=OnceLock::new();

fn self_directory()->Option<PathBuf>{
    let mut module=ptr::null_mut();
    let flags=0x0000_0004u32|0x0000_0002; // FROM_ADDRESS | UNCHANGED_REFCOUNT
    let ok=unsafe{GetModuleHandleExW(flags,DirectDrawCreate as *const () as *const u16,&mut module)};
    if ok==0 || module.is_null(){return None;}
    let mut buffer=[0u16;32768];
    let count=unsafe{GetModuleFileNameW(module,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    if count==0||count>=buffer.len(){return None;}
    let path=PathBuf::from(OsString::from_wide(&buffer[..count]));
    Some(path.parent()?.to_path_buf())
}
fn runtime_path()->Option<PathBuf>{
    Some(self_directory()?.join("crimsonskiesfix").join("CrimsonSkiesFix.dll"))
}
fn load_core()->Option<Backend>{
    let path=runtime_path()?;
    // Load an exact sibling, never an unqualified DLL search-path basename.
    let wide:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
    let dll=unsafe{LoadLibraryW(wide.as_ptr())};
    if dll.is_null(){return None;}
    let resolve=|name:&[u8]|unsafe{GetProcAddress(dll,name.as_ptr().cast()) as usize};
    let init=resolve(b"CrimsonSkiesFixInstallBackend\0");
    let create=resolve(b"CrimsonSkiesFixDDrawCreate\0");
    let enumerate=resolve(b"CrimsonSkiesFixDDrawEnumerateA\0");
    if init==0 || create==0 || enumerate==0{return None;}
    let attach:AttachFn=unsafe{std::mem::transmute(init)};
    let accepted=unsafe{attach(winmm_bootstrap::original_create(),
        winmm_bootstrap::original_enumerate(),winmm_bootstrap::status())};
    if accepted!=1{return None;}
    Some(Backend{create,enumerate})
}
fn backend()->Option<&'static Backend>{
    BACKEND.get_or_init(load_core).as_ref()
}

#[allow(non_snake_case)]
pub unsafe extern "system" fn DirectDrawCreate(
    guid:*const c_void,out:*mut *mut c_void,outer:*mut c_void)->Hr {
    let Some(runtime)=backend() else {
        if !out.is_null(){unsafe{*out=ptr::null_mut()};}
        return DDERR_GENERIC;
    };
    let call:CreateFn=unsafe{std::mem::transmute(runtime.create)};
    unsafe{call(guid,out,outer)}
}

#[allow(non_snake_case)]
pub unsafe extern "system" fn DirectDrawEnumerateA(
    callback:*mut c_void,context:*mut c_void)->Hr {
    let Some(runtime)=backend() else {return DDERR_GENERIC;};
    let call:EnumerateFn=unsafe{std::mem::transmute(runtime.enumerate)};
    unsafe{call(callback,context)}
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn fixed_subfolder_is_used_instead_of_path_search(){
        let path=PathBuf::from(r"C:\Game\crimsonskies")
            .join("crimsonskiesfix").join("CrimsonSkiesFix.dll");
        assert!(path.to_string_lossy().ends_with(r"crimsonskies\crimsonskiesfix\CrimsonSkiesFix.dll"));
    }
}
