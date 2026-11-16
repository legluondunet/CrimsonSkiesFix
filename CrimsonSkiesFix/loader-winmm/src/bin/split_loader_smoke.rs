//! Synthetic 32-bit Windows PE export/loader smoke; not a gameplay test.
use std::{env,ffi::{c_char,c_void},os::windows::ffi::OsStrExt,path::PathBuf};
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}
fn load(p:&PathBuf)->*mut c_void{
    assert!(p.is_absolute(),"expected absolute path: {}",p.display());
    let wide:Vec<u16>=p.as_os_str().encode_wide().chain(Some(0)).collect();
    let loaded=unsafe{LoadLibraryW(wide.as_ptr())};
    assert!(!loaded.is_null(),"cannot load DLL: {}",p.display());
    loaded
}
fn require(module:*mut c_void,name:&[u8])->usize{
    let address=unsafe{GetProcAddress(module,name.as_ptr().cast())} as usize;
    assert_ne!(address,0,"missing export {}",String::from_utf8_lossy(name));
    address
}
fn main(){
    let args=env::args_os().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(),2,"syntax: split_loader_smoke CORE_DLL WINMM_DLL");
    let core=load(&PathBuf::from(&args[0]));
    let handoff=require(core,b"CrimsonSkiesFixInstallBackend\0");
    require(core,b"CrimsonSkiesFixDDrawCreate\0");
    require(core,b"CrimsonSkiesFixDDrawEnumerateA\0");
    require(core,b"CrimsonSkiesFixDDrawCreateEx\0");
    type Handoff=unsafe extern "system" fn(usize,usize,u32)->i32;
    let call:Handoff=unsafe{std::mem::transmute(handoff)};
    assert_eq!(unsafe{call(0,0,7)},0,"missing pointers must be rejected");
    let winmm=load(&PathBuf::from(&args[1]));
    require(winmm,b"timeGetTime\0");
    let status=require(winmm,b"CrimsonSkiesFixBootstrapStatus\0");
    type Status=unsafe extern "system" fn()->u32;
    let read:Status=unsafe{std::mem::transmute(status)};
    assert_eq!(unsafe{read()},1,"synthetic, non-game EXE must not be hooked");
    println!("SPLIT-WINMM-SMOKE PASS: both DLLs load x86; 4 core exports and native WinMM table present; non-game EXE safely rejected");
}
