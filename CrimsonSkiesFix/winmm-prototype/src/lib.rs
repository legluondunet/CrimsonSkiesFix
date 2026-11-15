//! EXPERIMENTAL Crimson Skies x86 WinMM proxy; NOT an installer component.
//! Forwards only the 14 names statically imported by the exact 1.02 game EXE.
//! No DirectDraw or game-memory modifications. Do not distribute in v1.0.
//! Ordinals 1..14 are test-only, not real system WinMM ordinals.
//! First-call native library resolution may be unsafe if a caller owns the DLL
//! loader lock. Production proxy needs a safer initialization mechanism.
#[cfg(not(all(windows, target_arch = "x86", target_env = "msvc")))]
compile_error!("WinMM prototype requires i686-pc-windows-msvc");

use std::{
    arch::global_asm,
    ffi::{c_char, c_void, OsString},
    fs::OpenOptions,
    io::Write,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, OnceLock},
};

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemDirectoryW(path: *mut u16, size: u32) -> u32;
    fn LoadLibraryW(path: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *const c_void;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, path: *mut u16, size: u32) -> u32;
    fn GetTickCount() -> u32;
    fn GetModuleHandleExW(flags: u32, address: *const u16, module: *mut *mut c_void) -> i32;
}

const EXPORT_NAMES: [&[u8]; 14] = [
    b"mixerGetLineInfoA\0",
    b"mixerOpen\0",
    b"mciGetErrorStringA\0",
    b"mixerGetLineControlsA\0",
    b"mixerClose\0",
    b"auxSetVolume\0",
    b"auxGetVolume\0",
    b"mciSendCommandA\0",
    b"auxGetNumDevs\0",
    b"auxGetDevCapsA\0",
    b"mixerSetControlDetails\0",
    b"mixerGetControlDetailsA\0",
    b"mixerGetNumDevs\0",
    b"timeGetTime\0",
];
static SYSTEM_MODULE: OnceLock<Option<usize>> = OnceLock::new();
static RESOLVED: [AtomicUsize; 14] = [const { AtomicUsize::new(0) }; 14];
static FIRST_CALL: AtomicBool = AtomicBool::new(false);

// Test-only, lock-free first-use evidence for issue #32. Recording does NOT
// resolve imports, take an OS loader lock, allocate, or write a log file.
#[cfg(feature = "winmm-ordinal-bridge")]
static GAME_DDRAW_STARTED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "winmm-ordinal-bridge")]
static FIRST_NAMED_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(feature = "winmm-ordinal-bridge")]
static FIRST_NAMED_EARLY: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "winmm-ordinal-bridge")]
static NAMED_ATTEMPT_MASK: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "winmm-ordinal-bridge")]
static NAMED_EARLY_MASK: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "winmm-ordinal-bridge")]
static NAMED_RESOLVED_MASK: AtomicU32 = AtomicU32::new(0);

// Called at entry to one of the three game-owned DirectDraw hooks, prior to
// game configuration loading. Do not infer that loader lock is released solely
// from this marker: it measures ordering, not kernel loader-lock ownership.
#[cfg(feature = "winmm-ordinal-bridge")]
pub(crate) fn mark_game_ddraw_started() {
    GAME_DDRAW_STARTED.store(true, Ordering::Release);
}

#[cfg(feature = "winmm-ordinal-bridge")]
fn mark_named_resolution_attempt(index: usize) {
    let bit = 1u32 << index; // Only indices 0..13 reach this code path.
    let early = !GAME_DDRAW_STARTED.load(Ordering::Acquire);
    NAMED_ATTEMPT_MASK.fetch_or(bit,Ordering::Relaxed);
    if early {NAMED_EARLY_MASK.fetch_or(bit,Ordering::Relaxed);}
    if FIRST_NAMED_INDEX.compare_exchange(usize::MAX,index,Ordering::AcqRel,Ordering::Relaxed).is_ok(){
        FIRST_NAMED_EARLY.store(early,Ordering::Release);
    }
}

#[cfg(feature = "winmm-ordinal-bridge")]
fn named_list_from_mask(mask: u32)->String {
    let names=EXPORT_NAMES.iter().enumerate()
        .filter(|(i,_)|mask & (1u32 << i)!=0)
        .map(|(_,name)|std::str::from_utf8(name).unwrap_or("?").trim_end_matches('\0'))
        .collect::<Vec<_>>();
    if names.is_empty(){"none".to_string()}else{names.join(",")}
}

#[cfg(feature = "winmm-ordinal-bridge")]
pub(crate) fn named_forward_snapshot()->String {
    let first=FIRST_NAMED_INDEX.load(Ordering::Acquire);
    let first_name=EXPORT_NAMES.get(first).and_then(|value|std::str::from_utf8(value).ok())
        .map(|s|s.trim_end_matches('\0')).unwrap_or("none");
    let first_early=first!=usize::MAX && FIRST_NAMED_EARLY.load(Ordering::Acquire);
    let attempted=NAMED_ATTEMPT_MASK.load(Ordering::Acquire);
    let early=NAMED_EARLY_MASK.load(Ordering::Acquire);
    let resolved=NAMED_RESOLVED_MASK.load(Ordering::Acquire);
    format!("first={first_name} first_before_ddraw={first_early} attempted_unique={} resolved_unique={} before_ddraw={}",
        attempted.count_ones(),resolved.count_ones(),named_list_from_mask(early))
}

fn module_path(module: *mut c_void) -> Option<PathBuf> {
    let mut chars = [0u16; 32768];
    let n = unsafe { GetModuleFileNameW(module, chars.as_mut_ptr(), chars.len() as u32) } as usize;
    (n != 0 && n < chars.len()).then(|| PathBuf::from(OsString::from_wide(&chars[..n])))
}

fn system_winmm() -> Option<*mut c_void> {
    let handle = SYSTEM_MODULE.get_or_init(|| unsafe {
        let mut path = [0u16; 32768];
        let n = GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) as usize;
        let suffix: Vec<u16> = "\\winmm.dll".encode_utf16().chain(Some(0)).collect();
        if n == 0 || n + suffix.len() > path.len() { return None; }
        path[n..n + suffix.len()].copy_from_slice(&suffix);
        let handle = LoadLibraryW(path.as_ptr());
        (!handle.is_null()).then_some(handle as usize)
    });
    handle.as_ref().copied().map(|address| address as *mut c_void)
}

fn own_module() -> Option<*mut c_void> {
    let mut own = ptr::null_mut();
    let ok = unsafe {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | UNCHANGED_REFCOUNT
        GetModuleHandleExW(0x0000_0006, resolve_target as *const () as *const u16, &mut own)
    };
    (ok != 0 && !own.is_null()).then_some(own)
}

// The integrated runtime must not touch the filesystem from an early WinMM
// export: an imported DLL may call timeGetTime while the loader lock is held.
#[cfg(feature = "winmm-bootstrap")]
fn write_first_call(_: usize, _: *mut c_void) {}

#[cfg(not(feature = "winmm-bootstrap"))]
fn write_first_call(index: usize, system: *mut c_void) {
    if FIRST_CALL.swap(true, Ordering::AcqRel) { return; }
    let Some(exe) = module_path(ptr::null_mut()) else { return; };
    let Some(base) = exe.parent() else { return; };
    let ddraw_name: Vec<u16> = "ddraw.dll".encode_utf16().chain(Some(0)).collect();
    let ddraw = unsafe { GetModuleHandleW(ddraw_name.as_ptr()) };
    let api = std::str::from_utf8(EXPORT_NAMES[index]).unwrap_or("?").trim_end_matches('\0');
    #[cfg(feature = "winmm-bootstrap")]
    let iat_status = crate::winmm_bootstrap::status();
    #[cfg(not(feature = "winmm-bootstrap"))]
    let iat_status = 0u32;
    if let Ok(mut log) = OpenOptions::new().create(true).append(true)
        .open(base.join("CrimsonSkiesFix_winmm_probe.log")) {
        let _ = writeln!(log,
            "WINMM-PROBE first_call={} tick_ms={} proxy={} native_winmm={} ddraw={} bootstrap_iat_status={}; test_only=true",
            api, unsafe { GetTickCount() },
            own_module().and_then(module_path).map(|p| p.display().to_string()).unwrap_or_default(),
            module_path(system).map(|p| p.display().to_string()).unwrap_or_default(),
            module_path(ddraw).map(|p| p.display().to_string()).unwrap_or_default(),iat_status);
    }
}

#[inline(never)]
extern "C" fn resolve_target(index: u32) -> usize {
    let Some((slot, name)) = RESOLVED.get(index as usize).zip(EXPORT_NAMES.get(index as usize))
    else { std::process::abort(); };
    let old = slot.load(Ordering::Acquire);
    if old != 0 { return old; }
    #[cfg(feature = "winmm-ordinal-bridge")]
    mark_named_resolution_attempt(index as usize);
    let Some(system) = system_winmm() else { std::process::abort(); };
    if own_module() == Some(system) { std::process::abort(); }
    let target = unsafe { GetProcAddress(system, name.as_ptr().cast()) } as usize;
    if target == 0 { std::process::abort(); }
    write_first_call(index as usize, system);
    slot.store(target, Ordering::Release);
    #[cfg(feature = "winmm-ordinal-bridge")]
    NAMED_RESOLVED_MASK.fetch_or(1u32 << index,Ordering::Release);
    target
}

// Used by the extended WinMM compatibility surface. Name-based delegation
// avoids guessing multimedia API arguments or x86 return conventions.
#[cfg(feature = "winmm-bootstrap")]
pub(crate) fn resolve_external(name:&[u8])->usize {
    let Some(system)=system_winmm() else {std::process::abort();};
    if own_module()==Some(system) {std::process::abort();}
    let target=unsafe{GetProcAddress(system,name.as_ptr().cast())} as usize;
    if target==0 {std::process::abort();}
    target
}

#[cfg(feature = "winmm-ordinal-bridge")]
use std::sync::atomic::AtomicU32;

// Log-free, loader-lock-safe first-resolution bitmap. A separate lifecycle
// observer reads it later, when the game's regular log may be written safely.
// No call to std::fs, logging, heap or loader operations in this observer.
#[cfg(feature = "winmm-ordinal-bridge")]
static ORDINAL_SEEN: [AtomicU32; 32] = [const { AtomicU32::new(0) }; 32];

#[cfg(feature = "winmm-ordinal-bridge")]
fn record_ordinal_resolution(number: u32) {
    let Some(word)=ORDINAL_SEEN.get((number / 32) as usize) else {return;};
    word.fetch_or(1u32 << (number % 32),Ordering::Relaxed);
}

#[cfg(feature = "winmm-ordinal-bridge")]
pub(crate) fn ordinal_resolution_snapshot()->String {
    let seen=(1u32..=1023).filter(|number|{
        let word=ORDINAL_SEEN[(number / 32) as usize].load(Ordering::Relaxed);
        (word & (1u32 << (number % 32)))!=0
    }).collect::<Vec<_>>();
    let values=if seen.is_empty(){
        "none".to_string()
    }else{
        seen.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
    };
    format!("resolved_unique={} ordinals={values}",seen.len())
}

// EXPERIMENTAL: the ordinal-only export slots generated by runtime-winmm/build.rs
// are 1:1 numeric forwarding thunks, deliberately separate from the high-numbered
// named exports. An ordinal call delegates the SAME number to the real OS/Wine
// provider, which may associate different functions with that number.
#[cfg(feature = "winmm-ordinal-bridge")]
static ORDINAL_TARGETS: [AtomicUsize; 1024] = [const { AtomicUsize::new(0) }; 1024];

#[cfg(feature = "winmm-ordinal-bridge")]
#[inline(never)]
pub(crate) extern "C" fn resolve_ordinal(number: u32) -> usize {
    let Some(slot) = ORDINAL_TARGETS.get(number as usize) else {
        std::process::abort()
    };
    if number == 0 { std::process::abort(); }
    let cached = slot.load(Ordering::Acquire);
    if cached != 0 { return cached; }
    let Some(system) = system_winmm() else { std::process::abort(); };
    if own_module() == Some(system) { std::process::abort(); }
    // MAKEINTRESOURCEA(number): low 16 bits carry an ordinal, high bits zero.
    let address = unsafe { GetProcAddress(system, number as usize as *const c_char) } as usize;
    if address == 0 { std::process::abort(); }
    slot.store(address, Ordering::Release);
    // Mark only successfully resolved numbers, after publishing their
    // actual provider address. This performs one atomic bit-set per new
    // ordinal and does not call the game's logger from arbitrary DLL loads.
    record_ordinal_resolution(number);
    address
}

// ABI-preserving x86 tail jumps. Calls retain the original arguments, stack
// and return address; EAX alone carries the resolved export address.
global_asm!(r#"
    .text

    .globl _crimson_winmm_forward_0
    .p2align 4, 0x90
_crimson_winmm_forward_0:
    pushal
    pushl $0
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_1
    .p2align 4, 0x90
_crimson_winmm_forward_1:
    pushal
    pushl $1
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_2
    .p2align 4, 0x90
_crimson_winmm_forward_2:
    pushal
    pushl $2
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_3
    .p2align 4, 0x90
_crimson_winmm_forward_3:
    pushal
    pushl $3
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_4
    .p2align 4, 0x90
_crimson_winmm_forward_4:
    pushal
    pushl $4
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_5
    .p2align 4, 0x90
_crimson_winmm_forward_5:
    pushal
    pushl $5
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_6
    .p2align 4, 0x90
_crimson_winmm_forward_6:
    pushal
    pushl $6
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_7
    .p2align 4, 0x90
_crimson_winmm_forward_7:
    pushal
    pushl $7
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_8
    .p2align 4, 0x90
_crimson_winmm_forward_8:
    pushal
    pushl $8
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_9
    .p2align 4, 0x90
_crimson_winmm_forward_9:
    pushal
    pushl $9
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_10
    .p2align 4, 0x90
_crimson_winmm_forward_10:
    pushal
    pushl $10
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_11
    .p2align 4, 0x90
_crimson_winmm_forward_11:
    pushal
    pushl $11
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_12
    .p2align 4, 0x90
_crimson_winmm_forward_12:
    pushal
    pushl $12
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax


    .globl _crimson_winmm_forward_13
    .p2align 4, 0x90
_crimson_winmm_forward_13:
    pushal
    pushl $13
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

"#, resolve = sym resolve_target, options(att_syntax));

#[cfg(all(test, feature = "winmm-ordinal-bridge"))]
mod ordinal_telemetry_tests {
    use super::{ordinal_resolution_snapshot, record_ordinal_resolution};

    #[test]
    fn first_resolution_bitmap_is_readable_without_system_dll_or_file_io() {
        // The real resolver only records after resolving the native address.
        // A synthetic successful resolution isolates the diagnostic bitset.
        record_ordinal_resolution(999);
        record_ordinal_resolution(999);
        let snapshot=ordinal_resolution_snapshot();
        assert!(snapshot.contains("999"), "missing recorded ordinal: {snapshot}");
        assert!(snapshot.contains("resolved_unique="));
    }
}
