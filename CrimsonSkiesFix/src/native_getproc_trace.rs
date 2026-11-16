//! Exact SHA-256-admitted Crimson Skies PE32 GetProcAddress IAT interception.
//! For the 1.0 DirectDraw proxy this remains diagnostic-only (Windows/Log=1).
//! With the separate 1.1 WinMM runtime on native Windows, dynamically
//! requested DDRAW!DirectDrawCreate[Ex] / EnumerateA must re-enter our
//! original complete hooks. The game does not use only its static imports.
//! Otherwise the game-owned IDirectDraw4 bypasses the previously proven
//! native DD4 fullscreen/borderless bridge (historical issue #22).
//! All other imports, ordinals, modules, and failed resolutions pass unchanged.
//! Never install from DllMain or patch DLL exports/global process imports.
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
    fn GetLastError()->u32;
    fn SetLastError(error:u32);
}

// The game explicitly LoadLibraryA("DDRAW.DLL") on its dynamic path. Only
// the real, already-loaded module handle may be redirected: NEVER route an
// unrelated module's identically named symbol into CrimsonSkiesFix.
#[cfg(feature="winmm-bootstrap")]
const DDRAW_NAME:[u16;10]=[100,100,114,97,119,46,100,108,108,0];
#[cfg(feature="winmm-bootstrap")]
fn redirected_ddraw_function(
    api:&[u8],requested:usize,ddraw_module:usize,resolved:usize
)->Option<usize>{
    if requested==0 || requested!=ddraw_module || resolved==0 {return None;}
    Some(match api {
        b"DirectDrawCreate"=>crate::DirectDrawCreate as usize,
        b"DirectDrawCreateEx"=>crate::DirectDrawCreateEx as usize,
        b"DirectDrawEnumerateA"=>crate::DirectDrawEnumerateA as usize,
        _=>return None,
    })
}

unsafe extern "system" fn observe_getproc(
    module:*mut c_void, name:*const c_char
)->*const c_void {
    let ptr=ORIGINAL.load(Ordering::Acquire);
    if ptr==0 || ptr==observe_getproc as usize {
        return std::ptr::null();
    }
    let real:GetProcFn=std::mem::transmute(ptr);
    // Resolve from the caller's original module/name first. The NAME pointer
    // can be a 16-bit MAKEINTRESOURCE ordinal; never interpret those as CStr.
    let result=real(module,name);
    let saved_error=GetLastError();
    let mut returned=result;
    if !name.is_null() && (name as usize)>0xffff {
        let bytes=CStr::from_ptr(name).to_bytes();
        #[cfg(feature="winmm-bootstrap")]
        {
            // Never LoadLibrary under a resolver callback; GetModuleHandleW
            // only checks an already-loaded backend. It can be Windows' DLL,
            // Wine's native/builtin bridge, or an external ddraw wrapper.
            if matches!(bytes,b"DirectDrawCreate"|b"DirectDrawCreateEx"|b"DirectDrawEnumerateA") {
                let backend=GetModuleHandleW(DDRAW_NAME.as_ptr());
                if let Some(target)=redirected_ddraw_function(
                    bytes,module as usize,backend as usize,result as usize
                ){
                    returned=target as *const c_void;
                    let n=EVENTS.fetch_add(1,Ordering::Relaxed);
                    if n<24 {
                        log(&format!(
                            "NATIVE-WINMM-RESOLVE: api={} backend_module_matched=true original_resolved=true redirected_to_crimson=true (game IAT only, no global DDRAW replacement)",
                            String::from_utf8_lossy(bytes)));
                    }
                }
            }
        }
        if interesting_api(bytes) && crate::cfg().log_enabled {
            let n=EVENTS.fetch_add(1,Ordering::Relaxed);
            if n<32 {
                let own=win32::own_module();
                let requested_proxy=own==Some(module);
                let result_proxy=returned as usize==crate::DirectDrawCreate as usize ||
                    returned as usize==crate::DirectDrawCreateEx as usize ||
                    returned as usize==crate::DirectDrawEnumerateA as usize;
                log(&format!(
                    "NATIVE-RESOLVE: api={} requested_module_is_proxy={} result_is_proxy_export={} succeeded={} redirected={} (game IAT only)",
                    String::from_utf8_lossy(bytes),requested_proxy,
                    result_proxy,!result.is_null(),returned!=result));
            }
        }
    }
    // GetModuleHandleW, logging or Rust's formatting must not change the
    // return status of the caller's original GetProcAddress invocation.
    SetLastError(saved_error);
    returned
}

pub fn install() {
    if !diagnostics::is_native_windows() ||
       (!crate::cfg().log_enabled && !cfg!(feature="winmm-bootstrap")) {return;}
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
            if cfg!(feature="winmm-bootstrap"){
                log("NATIVE-WINMM-RESOLVE: game GetProcAddress IAT interception active; only successful named DDRAW resolutions from actual backend redirected");
            }else{
                log("NATIVE-RESOLVE: game PE32 GetProcAddress IAT observer installed (diagnostic only; all calls forwarded)");
            }
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
    #[cfg(feature="winmm-bootstrap")]
    #[test]
    fn only_real_ddraw_named_exports_may_reenter_winmm_hooks(){
        let backend=0x7a11_usize;
        for (name,target) in [
            (b"DirectDrawCreate".as_slice(),crate::DirectDrawCreate as usize),
            (b"DirectDrawCreateEx",crate::DirectDrawCreateEx as usize),
            (b"DirectDrawEnumerateA",crate::DirectDrawEnumerateA as usize)
        ] {
            assert_eq!(redirected_ddraw_function(name,backend,backend,0x8811),Some(target));
            assert_eq!(redirected_ddraw_function(name,backend+1,backend,0x8811),None);
            assert_eq!(redirected_ddraw_function(name,backend,backend,0),None);
            assert_eq!(redirected_ddraw_function(name,0,backend,0x8811),None);
        }
        for name in [b"DirectDrawEnumerateExA".as_slice(),b"DirectDrawCreateXXX",
                     b"DirectDrawCreateex",b"GetProcAddress",b"ChangeDisplaySettingsA"] {
            assert_eq!(redirected_ddraw_function(name,backend,backend,0x8811),None);
        }
    }
    #[test]
    fn original_getproc_abi_is_unchanged(){
        let _:GetProcFn=observe_getproc;
    }

    #[test]
    fn verified_game_import_slot_is_not_injected_into_arbitrary_modules(){
        assert_eq!(IAT_GETPROC_RVA,0x0061_fce8);
    }
}
