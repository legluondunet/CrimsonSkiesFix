//! Windows x86 CI smoke check: load the *built* proxy and inspect exports.
//! Never calls any DirectDrawCreate hook, game patch, window or real D3D device.
//! A successful result tests PE loading / DllMain under a real Windows loader,
//! NOT native Windows 11 gameplay compatibility.
// Generic Windows-specific diagnostics must not break non-Windows Cargo builds.
#[cfg(windows)]
use std::{ffi::{c_char,c_void,OsStr},os::windows::ffi::OsStrExt,path::PathBuf};

#[cfg(windows)]
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(filename:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn FreeLibrary(module:*mut c_void)->i32;
}

#[cfg(windows)]
fn run()->Result<(),String>{
    let argument=std::env::args_os().nth(1)
        .ok_or("usage: ddraw_loader_smoke.exe <absolute-or-relative-ddraw.dll>")?;
    let path=PathBuf::from(argument).canonicalize()
        .map_err(|e|format!("cannot resolve proxy path: {e}"))?;
    if path.file_name().and_then(|s|s.to_str())!=Some("ddraw.dll"){
        return Err(format!("expected the built proxy named ddraw.dll, got {}",path.display()));
    }
    let wide:Vec<u16>=OsStr::new(&path).encode_wide().chain(Some(0)).collect();
    let module=unsafe{LoadLibraryW(wide.as_ptr())};
    if module.is_null(){
        return Err(format!("Windows loader rejected 32-bit proxy {}",path.display()));
    }
    let result=(||{
        for symbol in [
            b"DirectDrawCreate\0".as_slice(),
            b"DirectDrawCreateEx\0".as_slice(),
            b"DirectDrawEnumerateA\0".as_slice(),
        ] {
            let ptr=unsafe{GetProcAddress(module,symbol.as_ptr().cast())};
            if ptr.is_null(){
                return Err(format!("missing expected DirectDraw export {}",
                    String::from_utf8_lossy(&symbol[..symbol.len()-1])));
            }
        }
        Ok(())
    })();
    // Always pair one successful LoadLibraryW with exactly one FreeLibrary.
    let released=unsafe{FreeLibrary(module)};
    if released==0{return Err("FreeLibrary failed after loading proxy".to_owned());}
    result
}

#[cfg(windows)]
fn main(){
    if let Err(e)=run(){
        eprintln!("DDRAW-LOAD-SMOKE: ERROR: {e}");
        std::process::exit(1);
    }
    println!("DDRAW-LOAD-SMOKE: PASS; 32-bit proxy loaded and released; 3 DirectDraw exports found (game not launched)");
}

#[cfg(not(windows))]
fn main(){
    eprintln!("DDRAW-LOAD-SMOKE: this native loader test is Windows-only");
    std::process::exit(2);
}
