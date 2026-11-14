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
    let Some(system) = system_winmm() else { std::process::abort(); };
    if own_module() == Some(system) { std::process::abort(); }
    let target = unsafe { GetProcAddress(system, name.as_ptr().cast()) } as usize;
    if target == 0 { std::process::abort(); }
    write_first_call(index as usize, system);
    slot.store(target, Ordering::Release);
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
