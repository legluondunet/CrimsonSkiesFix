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
        // Test the exact native Windows export surface, not merely the three
        // game-facing hooks. An incomplete table prevents D3D HAL discovery
        // even if the 3 existing exports can be resolved and called.
        const EXPORTS: [(&[u8],usize);22] = [
            (b"AcquireDDThreadLock\0",1),
            (b"CompleteCreateSysmemSurface\0",2),
            (b"D3DParseUnknownCommand\0",3),
            (b"DDGetAttachedSurfaceLcl\0",4),
            (b"DDInternalLock\0",5),
            (b"DDInternalUnlock\0",6),
            (b"DSoundHelp\0",7),
            (b"DirectDrawCreate\0",8),
            (b"DirectDrawCreateClipper\0",9),
            (b"DirectDrawCreateEx\0",10),
            (b"DirectDrawEnumerateA\0",11),
            (b"DirectDrawEnumerateExA\0",12),
            (b"DirectDrawEnumerateExW\0",13),
            (b"DirectDrawEnumerateW\0",14),
            (b"DllCanUnloadNow\0",15),
            (b"DllGetClassObject\0",16),
            (b"GetDDSurfaceLocal\0",17),
            (b"GetOLEThunkData\0",18),
            (b"GetSurfaceFromDC\0",19),
            (b"RegisterSpecialCase\0",20),
            (b"ReleaseDDThreadLock\0",21),
            (b"SetAppCompatData\0",22),
        ];
        for (name,ordinal) in EXPORTS {
            let by_name=unsafe{GetProcAddress(module,name.as_ptr().cast())};
            let by_ordinal=unsafe{GetProcAddress(module,ordinal as *const c_char)};
            if by_name.is_null() || by_ordinal.is_null() || by_name!=by_ordinal {
                return Err(format!(
                    "missing/inconsistent DirectDraw export {} @{}",
                    String::from_utf8_lossy(&name[..name.len()-1]),ordinal));
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
    println!("DDRAW-LOAD-SMOKE: PASS; 32-bit proxy loaded and released; 22 DirectDraw names and ordinals found (game not launched)");
}

#[cfg(not(windows))]
fn main(){
    eprintln!("DDRAW-LOAD-SMOKE: this native loader test is Windows-only");
    std::process::exit(2);
}
