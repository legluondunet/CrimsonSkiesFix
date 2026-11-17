//! Synthetic EXE host for the dependent-DllMain WinMM loader-lock regression.
//! The user game and resources are not involved. No memory injection.
use std::{env,ffi::{c_char,c_void},os::windows::ffi::OsStrExt,path::PathBuf};
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(path:*const u16)->*mut c_void;
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetLastError()->u32;
}
fn wide(s:&std::ffi::OsStr)->Vec<u16>{
    s.encode_wide().chain(Some(0)).collect()
}
fn require(module:*mut c_void,name:&[u8])->usize{
    let address=unsafe{GetProcAddress(module,name.as_ptr().cast())} as usize;
    assert_ne!(address,0,"missing export {}",String::from_utf8_lossy(name));
    address
}
fn main(){
    let args=env::args_os().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(),1,"syntax: split_dllmain_client_smoke CLIENT_DLL");
    let client=PathBuf::from(&args[0]);
    assert!(client.is_absolute(),"need absolute client DLL path");

    let winmm_name=wide(std::ffi::OsStr::new("winmm.dll"));
    let before=unsafe{GetModuleHandleW(winmm_name.as_ptr())};
    println!("WINMM-DLLMAIN-SMOKE: pre_client_winmm_loaded={}",!before.is_null());

    // Loading this DLL imports winmm.dll, then its DllMain calls the
    // imported WinMM timeGetTime while the loader lock is held.
    let client_w=wide(client.as_os_str());
    let module=unsafe{LoadLibraryW(client_w.as_ptr())};
    assert!(!module.is_null(),
        "dependent DLL failed to load in DllMain test; GetLastError={}",unsafe{GetLastError()});

    type Signal=unsafe extern "system" fn()->u32;
    let signal:Signal=unsafe{std::mem::transmute(require(
        module,b"CrimsonWinMMClientAttachSignal\0"))};
    let tick:Signal=unsafe{std::mem::transmute(require(
        module,b"CrimsonWinMMClientAttachTick\0"))};
    assert_eq!(unsafe{signal()},0xC11E_17A1,
        "client DLL_PROCESS_ATTACH did not run original WinMM call");
    let winmm=unsafe{GetModuleHandleW(winmm_name.as_ptr())};
    assert!(!winmm.is_null(),"WinMM import not loaded");
    let bootstrap=require(winmm,b"CrimsonSkiesFixBootstrapStatus\0");
    let read:Signal=unsafe{std::mem::transmute(bootstrap)};
    assert_eq!(unsafe{read()},1,"synthetic EXE was incorrectly patched");
    println!("WINMM-DLLMAIN-SMOKE PASS: first WinMM call from dependent DLL_PROCESS_ATTACH completed; tick={} synthetic_exe_rejected=true",
        unsafe{tick()});
}
