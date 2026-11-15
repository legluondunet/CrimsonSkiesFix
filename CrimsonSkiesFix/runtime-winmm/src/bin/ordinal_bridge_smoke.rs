//! Windows x86 integration smoke: compare named and same-ordinal WinMM calls.
//! This host imports only kernel32; it does not statically load winmm.dll.
use std::{env,ffi::{c_char,c_void},path::PathBuf,ptr};

#[link(name="kernel32")]
extern "system" {
    fn GetSystemDirectoryW(buffer:*mut u16,size:u32)->u32;
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn GetModuleFileNameW(module:*mut c_void,buffer:*mut u16,size:u32)->u32;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}

fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
fn path_of(module:*mut c_void)->PathBuf{
    let mut chars=[0u16;32768];
    let count=unsafe{GetModuleFileNameW(module,chars.as_mut_ptr(),chars.len() as u32)} as usize;
    assert!(count>0&&count<chars.len(),"could not inspect WinMM loaded path");
    PathBuf::from(String::from_utf16_lossy(&chars[..count]))
}
fn canonical(s:&std::path::Path)->String{
    std::fs::canonicalize(s).unwrap_or_else(|_|s.to_owned())
        .to_string_lossy().replace('/',"\\").to_ascii_lowercase()
}

fn main(){
    let input=env::args().nth(1).expect("usage: ordinal_bridge_smoke.exe PATH_TO_CANDIDATE_WINMM");
    let candidate_path=PathBuf::from(input);
    assert!(candidate_path.is_absolute(),"candidate must be an absolute path");
    let mut chars=[0u16;32768];
    let count=unsafe{GetSystemDirectoryW(chars.as_mut_ptr(),chars.len() as u32)} as usize;
    assert!(count>0&&count<chars.len()-32,"Windows system directory unavailable");
    let system_path=PathBuf::from(String::from_utf16_lossy(&chars[..count])).join("winmm.dll");
    let sys=unsafe{LoadLibraryW(wide(&system_path.to_string_lossy()).as_ptr())};
    assert!(!sys.is_null(),"could not load system WinMM");
    assert_eq!(canonical(&path_of(sys)),canonical(&system_path),"wrong native WinMM module");
    let time_name=b"timeGetTime\0";
    let target=unsafe{GetProcAddress(sys,time_name.as_ptr().cast())};
    assert!(!target.is_null(),"system WinMM missing timeGetTime by name");
    let actual_ordinal=(1usize..=1023).find(|&n|{
        let ptr=unsafe{GetProcAddress(sys,n as *const c_char)};
        ptr==target
    }).expect("system timeGetTime ordinal not found in range 1..1023");
    // The candidate cannot be confused with the system WinMM: explicit
    // path plus full path verification, never GetModuleHandleW by basename.
    let candidate=unsafe{LoadLibraryW(wide(&candidate_path.to_string_lossy()).as_ptr())};
    assert!(!candidate.is_null(),"candidate WinMM could not be loaded");
    assert_eq!(canonical(&path_of(candidate)),canonical(&candidate_path),
               "candidate WinMM unexpectedly resolved to system WinMM");
    let named=unsafe{GetProcAddress(candidate,time_name.as_ptr().cast())};
    let by_number=unsafe{GetProcAddress(candidate,actual_ordinal as *const c_char)};
    assert!(!named.is_null() && !by_number.is_null(),"bridge lacks named or ordinal thunk");
    assert_ne!(named,by_number,"named and ordinal thunks must be isolated");
    type TimeGetTime=unsafe extern "system" fn()->u32;
    let time_by_name:TimeGetTime=unsafe{std::mem::transmute(named)};
    let time_by_ord:TimeGetTime=unsafe{std::mem::transmute(by_number)};
    let first=unsafe{time_by_name()};
    let second=unsafe{time_by_ord()};
    assert!(second.wrapping_sub(first)<5000,
       "named and ordinal timeGetTime results disagree or clock moved backwards");
    println!("WINMM-ORDINAL-BRIDGE PASS: native={} candidate={} timeGetTime=@{} time_by_name={} time_by_ordinal={}",
        system_path.display(),candidate_path.display(),actual_ordinal,first,second);
}
