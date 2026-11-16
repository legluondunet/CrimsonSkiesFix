//! Deliberately difficult loader-lock smoke fixture: a *dependent* DLL
//! calls the app-local WinMM import during its own DLL_PROCESS_ATTACH.
 //! The game's executable and its data are NEVER used here.
#![cfg(windows)]
use std::{ffi::c_void,sync::atomic::{AtomicU32,Ordering}};

#[link(name="winmm")]
extern "system" { fn timeGetTime()->u32; }

const SEEN: u32=0xC11E_17A1;
static CALLED:AtomicU32=AtomicU32::new(0);
static TICK:AtomicU32=AtomicU32::new(0);

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _module:*mut c_void,reason:u32,_reserved:*mut c_void)->i32 {
    if reason==1 {  // DLL_PROCESS_ATTACH; the loader lock is held.
        let tick=unsafe{timeGetTime()};
        TICK.store(tick,Ordering::Release);
        CALLED.store(SEEN,Ordering::Release);
    }
    1
}

#[no_mangle]
pub extern "system" fn CrimsonWinMMClientAttachSignal()->u32 {
    CALLED.load(Ordering::Acquire)
}
#[no_mangle]
pub extern "system" fn CrimsonWinMMClientAttachTick()->u32 {
    TICK.load(Ordering::Acquire)
}
