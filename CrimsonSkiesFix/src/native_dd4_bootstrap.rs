//! Exact-EXE native Windows A/B: attach the proven game IDirectDraw4 vtable
//! BEFORE GOS asks it to switch display modes. The owner is published in
//! global 0x00A17104; mode setup begins in 0x0059F680 (verified PE32 EXE).
//!
//! Enabled for verified native Windows by default; configurable with
//! [Debug] NativeDD4Bootstrap=0. Never run under Wine/Proton, in DllMain,
//! or on an unidentified executable. The
//! function-entry trampoline preserves all displaced instructions and runs
//! the original game function with the same ABI/result. No source EXE edits.
//! When the adapter cannot prove a valid native windowed graph, its
//! pre-exposure fallback retains the original exclusive behavior.
use std::{ffi::c_void,ptr,sync::atomic::{AtomicUsize,Ordering}};
use crate::{diagnostics,log,modern_ddraw_hooks,native_owner_mode_trace,win32};
use native_owner_mode_trace::OwnerVersion;

const SETUP_RVA:usize=0x0019_f680;       // game VA 0x0059F680
const OWNER_RVA:usize=0x0061_7104;       // game VA 0x00A17104
const HWND_RVA:usize=0x0061_039c;        // operand of first game instruction
const PATCH_LEN:usize=6;                  // mov edx,[absolute] is exactly 6 bytes
const MEM_COMMIT:u32=0x1000;
const MEM_RESERVE:u32=0x2000;
const PAGE_EXECUTE_READWRITE:u32=0x40;
const PAGE_EXECUTE_READ:u32=0x20;
static TRAMPOLINE:AtomicUsize=AtomicUsize::new(0);
static ENTER_COUNT:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualAlloc(address:*mut c_void,size:usize,allocation:u32,protection:u32)->*mut c_void;
    fn VirtualFree(address:*mut c_void,size:usize,free_type:u32)->i32;
    fn VirtualProtect(address:*mut c_void,size:usize,protection:u32,previous:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}
fn relative_jmp(from:usize,to:usize)->[u8;5]{
    let mut result=[0xe9u8,0,0,0,0];
    result[1..5].copy_from_slice(&(to as u32).wrapping_sub((from as u32).wrapping_add(5)).to_le_bytes());
    result
}
unsafe extern "C" fn on_game_mode_setup()->u32{
    let trampoline=TRAMPOLINE.load(Ordering::Acquire);
    if trampoline==0 {return 0;}
    let count=ENTER_COUNT.fetch_add(1,Ordering::Relaxed);
    // The strict game EXE gate is required before installation. We never
    // modify the global or claim any ownership of its game-owned COM ref.
    let base=GetModuleHandleW(ptr::null()) as usize;
    if base!=0 {
        let owner=(base+OWNER_RVA) as *const *mut c_void;
        let dd4=owner.read();
        if !dd4.is_null() {
            if native_owner_mode_trace::classify(dd4)==Some(OwnerVersion::DirectDraw4){
                modern_ddraw_hooks::observe_modern_ddraw(dd4,"IDirectDraw4");
                if count<12{
                    log("NATIVE-BOOTSTRAP: game DD4 owner intercepted before SetCooperativeLevel and SetDisplayMode; windowed adapter eligible (not yet exposed)");
                }
            }else if count<12{
                log("NATIVE-BOOTSTRAP: game DD4 pointer version/identity unexpected; original game mode setup preserved");
            }
        }else if count<12{
            log("NATIVE-BOOTSTRAP: game DD4 pointer NULL; original setup preserved");
        }
    }
    // Game function is CDECL with no arguments, returns 0/1. Exactly the
    // displaced original MOV plus JMP to its next instruction is executed.
    let original:unsafe extern "C" fn()->u32=std::mem::transmute(trampoline);
    original()
}

pub fn install()->bool {
    if !crate::cfg().native_dd4_bootstrap ||
       !crate::cfg().experimental_windowed_adapter ||
       !crate::cfg().native_3d_widescreen ||
       crate::cfg().native_dpi_aware ||
       !diagnostics::is_native_windows() ||
       crate::cfg().directdraw_passthrough {return false;}
    if TRAMPOLINE.load(Ordering::Acquire)!=0{return true;}
    // Only one physical output: the adapter must never choose a guessed
    // monitor on multi-screen configurations.
    if win32::physical_primary_monitor_dimensions().is_none(){
        log("NATIVE-BOOTSTRAP: physical one-monitor mode unavailable; refusing game code hook");
        return false;
    }
    unsafe{
        let base=GetModuleHandleW(ptr::null()) as usize;
        if base==0 || base.checked_add(OWNER_RVA).is_none(){
            log("NATIVE-BOOTSTRAP: game base unavailable");
            return false;
        }
        let entry=(base+SETUP_RVA) as *mut u8;
        let mut expected=[0x8b,0x15,0,0,0,0];
        expected[2..6].copy_from_slice(&((base+HWND_RVA) as u32).to_le_bytes());
        if std::slice::from_raw_parts(entry,PATCH_LEN)!=expected {
            log("NATIVE-BOOTSTRAP: exact game mode-setup bytes differ; no patch installed");
            return false;
        }
        let trampoline=VirtualAlloc(ptr::null_mut(),16,MEM_COMMIT|MEM_RESERVE,PAGE_EXECUTE_READWRITE) as *mut u8;
        if trampoline.is_null(){
            log("NATIVE-BOOTSTRAP: trampoline allocation failed; game untouched");
            return false;
        }
        ptr::copy_nonoverlapping(entry,trampoline,PATCH_LEN);
        let jmp_back=relative_jmp(trampoline as usize+PATCH_LEN,entry as usize+PATCH_LEN);
        ptr::copy_nonoverlapping(jmp_back.as_ptr(),trampoline.add(PATCH_LEN),5);
        let mut old_trampoline_protection=0u32;
        if FlushInstructionCache(GetCurrentProcess(),trampoline.cast(),11)==0 ||
           VirtualProtect(trampoline.cast(),16,PAGE_EXECUTE_READ,
               &mut old_trampoline_protection)==0 {
            VirtualFree(trampoline.cast(),0,0x8000);
            log("NATIVE-BOOTSTRAP: trampoline final RX protection/flush failed; game untouched");
            return false;
        }
        let mut old=0u32;
        if VirtualProtect(entry.cast(),PATCH_LEN,PAGE_EXECUTE_READWRITE,&mut old)==0{
            VirtualFree(trampoline.cast(),0,0x8000);
            log("NATIVE-BOOTSTRAP: code page protection refused; game untouched");
            return false;
        }
        // Publish the complete callable trampoline BEFORE replacing the
        // entrypoint so no call can observe an uninitialized target.
        TRAMPOLINE.store(trampoline as usize,Ordering::Release);
        let jump=relative_jmp(entry as usize,on_game_mode_setup as usize);
        ptr::copy_nonoverlapping(jump.as_ptr(),entry,5);
        entry.add(5).write(0x90); // preserve 6-byte instruction boundary
        let flushed=FlushInstructionCache(GetCurrentProcess(),entry.cast(),PATCH_LEN)!=0;
        if !flushed {
            ptr::copy_nonoverlapping(expected.as_ptr(),entry,PATCH_LEN);
            let _=FlushInstructionCache(GetCurrentProcess(),entry.cast(),PATCH_LEN);
        }
        let mut discard=0u32;
        let restored=VirtualProtect(entry.cast(),PATCH_LEN,old,&mut discard)!=0;
        if !flushed {
            // The trampoline is no longer referenced after restoration.
            TRAMPOLINE.store(0,Ordering::Release);
            // Keep allocated RWX trampoline in failure path rather than
            // risking a concurrent invocation/use-after-free.
            log("NATIVE-BOOTSTRAP: code cache flush failed; original instruction restored");
            return false;
        }
        if !restored {
            log("NATIVE-BOOTSTRAP: warning: code protection restoration failed; hook active");
        }
        log("NATIVE-BOOTSTRAP: exact game entrypoint hooked before first DD4 mode request (experimental native-only)");
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_jumps_are_exact_five_bytes(){
        assert_eq!(relative_jmp(0x0040_1000,0x0040_2000),[0xe9,0xfb,0x0f,0,0]);
        assert_eq!(relative_jmp(0x0040_2000,0x0040_1000),[0xe9,0xfb,0xef,0xff,0xff]);
    }
    #[test]
    fn bootstrap_matches_exact_verified_x86_function_entry(){
        assert_eq!(SETUP_RVA,0x19f680);
        assert_eq!(OWNER_RVA,0x617104);
        assert_eq!(HWND_RVA,0x61039c);
        assert_eq!(PATCH_LEN,6);
    }
}
