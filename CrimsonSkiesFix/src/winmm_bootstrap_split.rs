//! Split-runtime side of the WinMM bridge. The app-local winmm.dll owns
//! DllMain and exact-game IAT hooks. This module is initialized ONLY after
//! loader lock, on the first intercepted DirectDraw call.
use core::ffi::c_void;
use std::sync::atomic::{AtomicU32,AtomicUsize,Ordering};

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn LoadLibraryW(name:*const u16)->*mut c_void;
}
const DDRAW:[u16;10]=[100,100,114,97,119,46,100,108,108,0];
static ORIGINAL_CREATE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_ENUMERATE:AtomicUsize=AtomicUsize::new(0);
static IAT_STATUS:AtomicU32=AtomicU32::new(0);

pub(crate) fn original_create()->usize{ORIGINAL_CREATE.load(Ordering::Acquire)}
pub(crate) fn original_enumerate()->usize{ORIGINAL_ENUMERATE.load(Ordering::Acquire)}
pub(crate) fn status()->u32{IAT_STATUS.load(Ordering::Acquire)}

#[no_mangle]
pub extern "system" fn CrimsonSkiesFixInstallBackend(
    create:usize,enumerate:usize,bootstrap_status:u32)->i32{
    // Reject all other modes; the loader has already structurally
    // authenticated CRIMSON.EXE and stored both original DDRAW addresses.
    if bootstrap_status!=7 || create<0x10000 || enumerate<0x10000{return 0;}
    if ORIGINAL_CREATE.compare_exchange(0,create,Ordering::AcqRel,Ordering::Acquire).is_err()
        && original_create()!=create{return 0;}
    if ORIGINAL_ENUMERATE.compare_exchange(0,enumerate,Ordering::AcqRel,Ordering::Acquire).is_err()
        && original_enumerate()!=enumerate{return 0;}
    IAT_STATUS.store(bootstrap_status,Ordering::Release);
    1
}

pub(crate) unsafe fn loaded_ddraw()->Option<*mut c_void>{
    let mut h=unsafe{GetModuleHandleW(DDRAW.as_ptr())};
    if h.is_null(){h=unsafe{LoadLibraryW(DDRAW.as_ptr())};}
    (!h.is_null()).then_some(h)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn invalid_original_imports_are_rejected_without_state_changes(){
        assert_eq!(CrimsonSkiesFixInstallBackend(0,0,7),0);
        assert_eq!(CrimsonSkiesFixInstallBackend(0x70001234,0x70005678,4),0);
        assert_eq!(status(),0);
    }
}
