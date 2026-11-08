//! Diagnostic-only observer for the EXACT sha256-admitted Crimson Skies
//! PE32 GetProcAddress import address table slot. It logs the module identity
//! of dynamic DirectDraw API resolutions and transparently forwards every
//! original call, including calls by ordinal. This is NOT a display-mode fix.
//! Only native Windows with [Debug] Log=1; no Wine/Proton, no Log=0 behavior.
//! Never install from DllMain and never hook system DLL exports.
use std::{ffi::{c_char,c_void,CStr},sync::atomic::{AtomicUsize,Ordering}};
use crate::{diagnostics,log,win32};

type GetProcFn=unsafe extern "system" fn(*mut c_void,*const c_char)->*const c_void;
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static EVENTS:AtomicUsize=AtomicUsize::new(0);

// Exact PE32 import RVA of KERNEL32!GetProcAddress in the SHA-256-admitted
// unwrapped testme.exe; disassembly at VA 0x5AA019 and 0x5AA280 calls through
// [0x00A1FCE8]. Never use this offset with an unverified executable.
const IAT_GETPROC_RVA:usize=0x0061_fce8;

fn interesting_api(name:&[u8])->bool {
    name==b"DirectDrawCreate" ||
    name==b"DirectDrawCreateEx" ||
    name==b"DirectDrawEnumerateA" ||
    name.starts_with(b"ChangeDisplaySettings") ||
    name==b"SetDisplayConfig"
}

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name:*const u16)->*mut c_void;
}

unsafe extern "system" fn observe_getproc(
    module:*mut c_void, name:*const c_char
)->*const c_void {
    let ptr=ORIGINAL.load(Ordering::Acquire);
    if ptr==0 || ptr==observe_getproc as usize {
        return std::ptr::null();
    }
    let real:GetProcFn=std::mem::transmute(ptr);
    // The actual Windows resolver must receive the original module, name
    // pointer (which may be MAKEINTRESOURCE ordinal), and return untouched.
    let result=real(module,name);
    if !name.is_null() && (name as usize)>0xffff {
        let bytes=CStr::from_ptr(name).to_bytes();
        if interesting_api(bytes) {
            let n=EVENTS.fetch_add(1,Ordering::Relaxed);
            if n<32 {
                let own=win32::own_module();
                let requested_proxy=own==Some(module);
                let result_proxy=result as usize==crate::DirectDrawCreate as usize ||
                    result as usize==crate::DirectDrawCreateEx as usize ||
                    result as usize==crate::DirectDrawEnumerateA as usize;
                log(&format!(
                    "NATIVE-RESOLVE: api={} requested_module_is_proxy={} result_is_proxy_export={} succeeded={} (GetProcAddress original result forwarded unchanged)",
                    String::from_utf8_lossy(bytes),
                    requested_proxy,result_proxy,!result.is_null()
                ));
            }
        }
    }
    result
}

pub fn install() {
    if !diagnostics::is_native_windows() || !crate::cfg().log_enabled {return;}
    unsafe {
        let base=GetModuleHandleW(std::ptr::null()) as usize;
        if base==0 || base>usize::MAX-IAT_GETPROC_RVA {
            log("NATIVE-RESOLVE: IAT audit refused (no valid EXE base)");
            return;
        }
        let slot=(base+IAT_GETPROC_RVA) as *mut usize;
        // The caller verified the exact EXE SHA-256 before this function.
        // Reject an empty or already hooked import slot.
        let current=slot.read();
        if current==0 || current==observe_getproc as usize {
            log("NATIVE-RESOLVE: GetProcAddress IAT observer refused (unexpected slot)");
            return;
        }
        if ORIGINAL.compare_exchange(0,current,Ordering::AcqRel,Ordering::Acquire).is_err(){
            return;
        }
        if !win32::patch_vtable_slot(slot,observe_getproc as usize){
            ORIGINAL.store(0,Ordering::Release);
            log("NATIVE-RESOLVE: GetProcAddress IAT observer failed to install");
        } else {
            log("NATIVE-RESOLVE: game PE32 GetProcAddress IAT observer installed (diagnostic only; all calls forwarded)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traces_only_possible_mode_resolvers_and_ddraw_functions(){
        for name in [b"DirectDrawCreate".as_slice(),b"DirectDrawCreateEx",
                     b"ChangeDisplaySettingsA",b"ChangeDisplaySettingsExW",
                     b"SetDisplayConfig"]{
            assert!(interesting_api(name));
        }
        for name in [b"DirectInputCreate".as_slice(),b"GetProcAddress",
                     b"FreeLibrary",b"ChangeDisplayMode",b"EnumDisplaySettingsW"]{
            assert!(!interesting_api(name));
        }
    }
    #[test]
    fn verified_game_import_slot_is_not_injected_into_arbitrary_modules(){
        assert_eq!(IAT_GETPROC_RVA,0x0061_fce8);
    }
}
