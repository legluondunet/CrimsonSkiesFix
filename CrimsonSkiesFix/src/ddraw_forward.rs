//! Windows x86 DDRAW compatibility: remaining native exports, preserving their ABIs.
//!
//! Crimson Skies' hardware HAL enumeration works on Windows 11 with a full
//! 22-export DirectDraw proxy, and fails with the former 3-export proxy.
//! Preserve the existing Rust interception only for DirectDrawCreate,
//! DirectDrawCreateEx and DirectDrawEnumerateA. The other 19 exports have
//! undocumented / heterogeneous x86 calling conventions: a C/Rust wrapper
//! with guessed parameters would corrupt the caller's stack.
//!
//! Each assembly shim preserves all general registers and the caller's stack
//! while resolving the system DLL's actual address and tail-jumping to it.
//! No graphics state or return values are modified. On a missing native
//! export, fail closed rather than jumping through NULL.
//!
//! The canonical name + ordinal mapping lives in ddraw_exports.def.
#![cfg(all(windows, target_arch = "x86", target_env = "msvc"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::arch::global_asm;

const SYSTEM_EXPORT_NAMES: [&[u8]; 22] = [
    b"AcquireDDThreadLock\0",
    b"CompleteCreateSysmemSurface\0",
    b"D3DParseUnknownCommand\0",
    b"DDGetAttachedSurfaceLcl\0",
    b"DDInternalLock\0",
    b"DDInternalUnlock\0",
    b"DSoundHelp\0",
    b"DirectDrawCreate\0",
    b"DirectDrawCreateClipper\0",
    b"DirectDrawCreateEx\0",
    b"DirectDrawEnumerateA\0",
    b"DirectDrawEnumerateExA\0",
    b"DirectDrawEnumerateExW\0",
    b"DirectDrawEnumerateW\0",
    b"DllCanUnloadNow\0",
    b"DllGetClassObject\0",
    b"GetDDSurfaceLocal\0",
    b"GetOLEThunkData\0",
    b"GetSurfaceFromDC\0",
    b"RegisterSpecialCase\0",
    b"ReleaseDDThreadLock\0",
    b"SetAppCompatData\0",
];

// Cache each native function pointer; the helper can otherwise be invoked
// thousands of times per second by internal DirectDraw synchronization APIs.
static RESOLVED: [AtomicUsize; 22] = [const { AtomicUsize::new(0) }; 22];

#[inline(never)]
extern "C" fn resolve_target(index: u32) -> usize {
    let Some((slot, name)) = RESOLVED.get(index as usize)
        .zip(SYSTEM_EXPORT_NAMES.get(index as usize))
    else { std::process::exit(126) };

    let previous = slot.load(Ordering::Acquire);
    if previous != 0 {
        return previous;
    }
    let Some(system) = crate::real_module() else {
        crate::log("DDRAW-FORWARD: system32 ddraw.dll unavailable");
        std::process::exit(126);
    };
    let target = unsafe { crate::win32::system_export(system, name) } as usize;
    if target == 0 {
        crate::log("DDRAW-FORWARD: required native DDRAW export unavailable");
        std::process::exit(126);
    }
    slot.store(target, Ordering::Release);
    target
}

// pushal/popal restore the original call's volatile and nonvolatile
// registers, except EAX, which deliberately holds the resolved destination.
// The last jmp preserves the original argument stack and return address.
// In pushal's saved register frame EAX is at ESP+28.
global_asm!(
    r#"
    .text
    .globl _crimson_ddraw_forward_0
    .p2align 4, 0x90
_crimson_ddraw_forward_0:
    pushal
    pushl $0
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_1
    .p2align 4, 0x90
_crimson_ddraw_forward_1:
    pushal
    pushl $1
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_2
    .p2align 4, 0x90
_crimson_ddraw_forward_2:
    pushal
    pushl $2
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_3
    .p2align 4, 0x90
_crimson_ddraw_forward_3:
    pushal
    pushl $3
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_4
    .p2align 4, 0x90
_crimson_ddraw_forward_4:
    pushal
    pushl $4
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_5
    .p2align 4, 0x90
_crimson_ddraw_forward_5:
    pushal
    pushl $5
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_6
    .p2align 4, 0x90
_crimson_ddraw_forward_6:
    pushal
    pushl $6
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_8
    .p2align 4, 0x90
_crimson_ddraw_forward_8:
    pushal
    pushl $8
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_11
    .p2align 4, 0x90
_crimson_ddraw_forward_11:
    pushal
    pushl $11
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_12
    .p2align 4, 0x90
_crimson_ddraw_forward_12:
    pushal
    pushl $12
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_13
    .p2align 4, 0x90
_crimson_ddraw_forward_13:
    pushal
    pushl $13
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_14
    .p2align 4, 0x90
_crimson_ddraw_forward_14:
    pushal
    pushl $14
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_15
    .p2align 4, 0x90
_crimson_ddraw_forward_15:
    pushal
    pushl $15
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_16
    .p2align 4, 0x90
_crimson_ddraw_forward_16:
    pushal
    pushl $16
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_17
    .p2align 4, 0x90
_crimson_ddraw_forward_17:
    pushal
    pushl $17
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_18
    .p2align 4, 0x90
_crimson_ddraw_forward_18:
    pushal
    pushl $18
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_19
    .p2align 4, 0x90
_crimson_ddraw_forward_19:
    pushal
    pushl $19
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_20
    .p2align 4, 0x90
_crimson_ddraw_forward_20:
    pushal
    pushl $20
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    .globl _crimson_ddraw_forward_21
    .p2align 4, 0x90
_crimson_ddraw_forward_21:
    pushal
    pushl $21
    call {resolve}
    addl $4, %esp
    movl %eax, 28(%esp)
    popal
    jmp *%eax

    "#,
    resolve = sym resolve_target,
    options(att_syntax)
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_export_names_are_complete_and_unique() {
        assert_eq!(SYSTEM_EXPORT_NAMES.len(), 22);
        let mut names = std::collections::HashSet::new();
        for name in SYSTEM_EXPORT_NAMES {
            assert_eq!(name.last(), Some(&0));
            assert!(names.insert(name));
        }
        assert_eq!(SYSTEM_EXPORT_NAMES[7], b"DirectDrawCreate\0");
        assert_eq!(SYSTEM_EXPORT_NAMES[9], b"DirectDrawCreateEx\0");
        assert_eq!(SYSTEM_EXPORT_NAMES[10], b"DirectDrawEnumerateA\0");
    }
}
