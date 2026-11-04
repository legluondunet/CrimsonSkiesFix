//! Default-enabled under Wine, strictly fingerprinted in-memory compatibility fix for the
//! original Crimson Skies ROFFILE.DLL INI parser. Never edits game files.
//!
//! The original InitIniFile allocates exactly file_size bytes and passes them
//! to a LF/NUL-delimited reader without a size. The reader advances its cursor
//! after the first NUL, so two accessible zero bytes are necessary at EOF.
//! We redirect ONLY its allocation CALL at RVA 0x1172 to the DLL's own allocator,
//! requesting size+2 and writing those two sentinel bytes before ReadFile.
//!
//! Wine-only, with explicit user opt-out. The module may be loaded dynamically, so we look for it
//! during active graphics calls; a load immediately followed by parsing with no
//! intervening graphics call will not be patched. This limitation is logged.
//! Nothing is attempted from DllMain or Windows' loader lock.
use std::{
    ffi::{c_void, OsString},
    fs, os::windows::ffi::OsStringExt, path::PathBuf, ptr,
    sync::{atomic::{AtomicUsize, Ordering}, Mutex},
};
use crate::{exe_gate, log};

const EXPECTED_SHA256: &str =
    "1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a";
const EXPECTED_SIZE: usize = 135_168;
const PE_IMAGE_SIZE: usize = 0x23000;
const CALL_RVA: usize = 0x1172;
const ALLOCATOR_RVA: usize = 0xD96C;
const ORIGINAL_CALL: [u8; 5] = [0xe8, 0xf5, 0xc7, 0x00, 0x00];
const PRECALL: [u8; 5] = [0x8b, 0x4c, 0x24, 0x14, 0x51];
const READER: [u8; 7] = [0x8a, 0x06, 0x41, 0x84, 0xc0, 0x75, 0xf4];
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

type AllocFn = unsafe extern "C" fn(size: u32) -> *mut u8;

static PATCHED_MODULE: AtomicUsize = AtomicUsize::new(0);
static REFUSED_MODULE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_ALLOCATOR: AtomicUsize = AtomicUsize::new(0);
// These lock-free counters are updated only inside the patched cdecl allocator.
// No logging, formatting, I/O, mutex, or heap allocation occurs in that path.
// Reporting happens later from the existing rendering probe.
static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_PADDED: AtomicUsize = AtomicUsize::new(0);
static ALLOC_FAILED: AtomicUsize = AtomicUsize::new(0);
static ALLOC_LAST_SIZE: AtomicUsize = AtomicUsize::new(0);
static ALLOC_REPORTED: AtomicUsize = AtomicUsize::new(0);
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(name: *const u8) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, path: *mut u16, capacity: u32) -> u32;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, len: usize, new: u32, old: *mut u32) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn FlushInstructionCache(process: *mut c_void, address: *const c_void, len: usize) -> i32;
}

fn padded_size(size: u32) -> Option<u32> {
    size.checked_add(2)
}

// Called only after allocation of n+2 has succeeded. Retain the entire
// original file payload at [0..n], including non-text contents.
unsafe fn append_eof_sentinels(buffer: *mut u8, file_size: u32) {
    ptr::write(buffer.add(file_size as usize), 0);
    ptr::write(buffer.add(file_size as usize + 1), 0);
}

fn rel32(call_at: usize, target: usize) -> Option<[u8; 4]> {
    let rel = target as i64 - (call_at as i64 + 5);
    let rel = i32::try_from(rel).ok()?;
    Some(rel.to_le_bytes())
}

// Always report the first observed call, then use sparse milestones to avoid
// producing log output for every file/line parsed while rendering.
fn should_report_calls(calls: usize, reported: usize) -> bool {
    calls > reported &&
        (reported == 0 || calls.is_power_of_two() || calls - reported >= 64)
}

fn report_allocation_stats() {
    let calls = ALLOC_CALLS.load(Ordering::Acquire);
    let reported = ALLOC_REPORTED.load(Ordering::Relaxed);
    if !should_report_calls(calls, reported) {
        return;
    }
    if ALLOC_REPORTED.compare_exchange(
        reported, calls, Ordering::AcqRel, Ordering::Relaxed
    ).is_err() {
        return;
    }
    let padded = ALLOC_PADDED.load(Ordering::Relaxed);
    let failed = ALLOC_FAILED.load(Ordering::Relaxed);
    let last_size = ALLOC_LAST_SIZE.load(Ordering::Relaxed);
    log(&format!(
        "ROFFILE-FIX: allocation calls={calls} padded_success={padded} failures={failed} last_input_size={last_size}; counted in cdecl hook, reported outside hook"
    ));
}

// Windows x86 cdecl: one 32-bit argument, caller removes it (as in the
// original CALL at ROFFILE + 0x1172). Do not change to extern "system".
#[inline(never)]
unsafe extern "C" fn allocate_padded(size: u32) -> *mut u8 {
    let Some(requested) = padded_size(size) else {
        ALLOC_LAST_SIZE.store(size as usize, Ordering::Relaxed);
        ALLOC_FAILED.fetch_add(1, Ordering::Relaxed);
        ALLOC_CALLS.fetch_add(1, Ordering::Release);
        return ptr::null_mut();
    };
    let original = ORIGINAL_ALLOCATOR.load(Ordering::Acquire);
    if original == 0 {
        ALLOC_LAST_SIZE.store(size as usize, Ordering::Relaxed);
        ALLOC_FAILED.fetch_add(1, Ordering::Relaxed);
        ALLOC_CALLS.fetch_add(1, Ordering::Release);
        return ptr::null_mut();
    }
    let allocator: AllocFn = std::mem::transmute(original);
    let result = allocator(requested);
    if !result.is_null() {
        // The game reads only the original size, leaving these bytes intact.
        append_eof_sentinels(result, size);
        ALLOC_PADDED.fetch_add(1, Ordering::Relaxed);
    } else {
        ALLOC_FAILED.fetch_add(1, Ordering::Relaxed);
    }
    ALLOC_LAST_SIZE.store(size as usize, Ordering::Relaxed);
    // Publish the completed operation only after updating its result.
    ALLOC_CALLS.fetch_add(1, Ordering::Release);
    result
}

fn loaded_path(module: *mut c_void) -> Option<PathBuf> {
    let mut wide = [0u16; 4096];
    let len = unsafe { GetModuleFileNameW(module, wide.as_mut_ptr(), wide.len() as u32) };
    if len == 0 || len as usize >= wide.len() {
        return None;
    }
    Some(PathBuf::from(OsString::from_wide(&wide[..len as usize])))
}

fn original_image_is_recognized(module: usize) -> bool {
    // A valid GetModuleHandle result means the module has mapped PE headers.
    let h = unsafe { std::slice::from_raw_parts(module as *const u8, 0x1000) };
    if h.get(..2) != Some(b"MZ") {
        return false;
    }
    let Some(pe) = h.get(0x3c..0x40) else { return false };
    let pe = u32::from_le_bytes(pe.try_into().unwrap()) as usize;
    if pe < 0x40 || pe > 0x400 || h.get(pe..pe+4) != Some(b"PE\0\0") {
        return false;
    }
    let magic = h.get(pe+24..pe+26);
    let image_size = h.get(pe+80..pe+84)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    if magic != Some(&[0x0b, 0x01][..]) || image_size != Some(PE_IMAGE_SIZE) {
        return false;
    }
    unsafe {
        let from = |rva: usize, count: usize| {
            std::slice::from_raw_parts((module + rva) as *const u8, count)
        };
        from(CALL_RVA - 5, 5) == PRECALL &&
        from(CALL_RVA, 5) == ORIGINAL_CALL &&
        from(0x22e0, 7) == READER &&
        from(ALLOCATOR_RVA, 2) == [0x6a, 0x01]
    }
}

fn is_wine() -> bool {
    unsafe {
        let ntdll = GetModuleHandleA(c"ntdll.dll".as_ptr().cast());
        !ntdll.is_null() &&
            !GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_null()
    }
}

unsafe fn restore_call(site: *mut u8) -> bool {
    let mut original_protection = 0;
    if VirtualProtect(site.cast(), 5, PAGE_EXECUTE_READWRITE, &mut original_protection) == 0 {
        return false;
    }
    ptr::copy_nonoverlapping(ORIGINAL_CALL.as_ptr(), site, 5);
    let flushed = FlushInstructionCache(GetCurrentProcess(), site.cast(), 5) != 0;
    let mut ignored = 0;
    let restored = VirtualProtect(site.cast(), 5, original_protection, &mut ignored) != 0;
    flushed && restored
}

unsafe fn install(module: usize) -> Result<(), String> {
    if !is_wine() {
        return Err("not Wine: original Windows behavior preserved".into());
    }
    let path = loaded_path(module as *mut c_void)
        .ok_or("cannot retrieve ROFFILE.DLL path")?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("cannot stat ROFFILE.DLL: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() ||
       metadata.len() != EXPECTED_SIZE as u64 {
        return Err("unsupported ROFFILE.DLL size or file type".into());
    }
    let file = fs::read(&path).map_err(|e| format!("cannot read ROFFILE.DLL: {e}"))?;
    let actual_hash = exe_gate::sha256(&file);
    if actual_hash != EXPECTED_SHA256 {
        return Err(format!("unsupported ROFFILE.DLL SHA-256={actual_hash}"));
    }
    if !original_image_is_recognized(module) {
        return Err("ROFFILE.DLL in-memory PE header or exact instruction bytes differ".into());
    }
    let site_addr = module + CALL_RVA;
    let target = allocate_padded as usize;
    let Some(displacement) = rel32(site_addr, target) else {
        return Err("ROFFILE.DLL patch target is outside x86 relative CALL range".into());
    };
    let mut patch = [0u8; 5];
    patch[0] = 0xe8;
    patch[1..].copy_from_slice(&displacement);

    let site = site_addr as *mut u8;
    let mut previous = 0u32;
    if VirtualProtect(site.cast(), patch.len(), PAGE_EXECUTE_READWRITE, &mut previous) == 0 {
        return Err("VirtualProtect could not make ROFFILE.DLL call writable".into());
    }
    // Publish the allocator before redirecting execution to our wrapper.
    ORIGINAL_ALLOCATOR.store(module + ALLOCATOR_RVA, Ordering::Release);
    ptr::copy_nonoverlapping(patch.as_ptr(), site, patch.len());
    let flushed = FlushInstructionCache(GetCurrentProcess(), site.cast(), patch.len()) != 0;
    let mut ignored = 0;
    let protected = VirtualProtect(site.cast(), patch.len(), previous, &mut ignored) != 0;
    if !flushed || !protected {
        if restore_call(site) {
            return Err("instruction-cache flush or protection restore failed; call reverted".into());
        }
        // No success claim: an inability to restore a live code site is fatal.
        // Do not continue into a potentially partially patched game process.
        log("ROFFILE-FIX: CRITICAL: unable to roll back live CALL patch; aborting");
        std::process::abort();
    }
    Ok(())
}

/// Invoked after the authorized EXE's DirectDraw hooks are running.
/// Polling is active by default, unless disabled in the user INI.
pub fn probe() {
    if !crate::cfg().roffile_terminator_fix {
        return;
    }
    let module = unsafe { GetModuleHandleA(c"roffile.dll".as_ptr().cast()) } as usize;
    if module == 0 {
        return;
    }
    if PATCHED_MODULE.load(Ordering::Acquire) == module {
        report_allocation_stats();
        return;
    }
    if REFUSED_MODULE.load(Ordering::Acquire) == module {
        return;
    }
    let Ok(_guard) = INSTALL_LOCK.try_lock() else { return };
    if PATCHED_MODULE.load(Ordering::Acquire) == module ||
       REFUSED_MODULE.load(Ordering::Acquire) == module {
        return;
    }
    match unsafe { install(module) } {
        Ok(()) => {
            PATCHED_MODULE.store(module, Ordering::Release);
            log(&format!("ROFFILE-FIX: installed in memory module={module:#010x} original_sha256={EXPECTED_SHA256} allocation=original_file_size+2; Wine only"));
        }
        Err(reason) => {
            REFUSED_MODULE.store(module, Ordering::Release);
            log(&format!("ROFFILE-FIX: NOT installed module={module:#010x}: {reason}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminators_include_an_extra_byte_for_published_cursor() {
        assert_eq!(padded_size(0), Some(2));
        assert_eq!(padded_size(0x1c), Some(0x1e));
        assert_eq!(padded_size(u32::MAX - 2), Some(u32::MAX));
        assert_eq!(padded_size(u32::MAX - 1), None);
        assert_eq!(padded_size(u32::MAX), None);
    }
    #[test]
    fn sentinels_preserve_payload_and_bound_next_cursor_read() {
        let mut buffer = [0x41u8, 0x42, 0x43, 0x44, 0xff, 0xff];
        unsafe { append_eof_sentinels(buffer.as_mut_ptr(), 4) };
        assert_eq!(buffer, [0x41, 0x42, 0x43, 0x44, 0, 0]);
        let mut empty = [0xffu8, 0xff];
        unsafe { append_eof_sentinels(empty.as_mut_ptr(), 0) };
        assert_eq!(empty, [0, 0]);
    }
    #[test]
    fn rel32_encoding_and_out_of_range_fail_closed() {
        assert_eq!(rel32(0x1000, 0x1005), Some([0, 0, 0, 0]));
        assert_eq!(rel32(0x1000, 0x1000), Some((-5i32).to_le_bytes()));
        assert!(rel32(0x0469_1172, 0x7a4a_1000).is_some());
        assert_eq!(rel32(0x1000, 0xf000_0000), None);
    }
    #[test]
    fn diagnostic_reports_first_call_and_sparse_milestones() {
        assert!(!should_report_calls(0, 0));
        assert!(should_report_calls(1, 0));
        assert!(!should_report_calls(1, 1));
        assert!(should_report_calls(2, 1));
        assert!(!should_report_calls(3, 2));
        assert!(should_report_calls(4, 2));
        assert!(!should_report_calls(6, 4));
        assert!(should_report_calls(8, 4));
        assert!(!should_report_calls(70, 64));
        assert!(should_report_calls(128, 64));
        assert!(should_report_calls(129, 64));
        assert!(!should_report_calls(129, 129));
    }
    #[test]
    fn original_call_matches_verified_allocator_rva() {
        let displacement = i32::from_le_bytes(ORIGINAL_CALL[1..].try_into().unwrap());
        assert_eq!(CALL_RVA as i64 + 5 + displacement as i64, ALLOCATOR_RVA as i64);
    }
}
