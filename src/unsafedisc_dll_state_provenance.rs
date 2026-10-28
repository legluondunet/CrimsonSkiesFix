// Original SafeDisc DLL byte-mixer state provenance (read-only).
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// The global 32-bit state at preferred VA 0x00905010 starts in
// PE .data zero-filled virtual memory, but other reachable or
// potentially reachable routines write it. The static entries
// here are NOT a proven temporal execution trace.

use super::{
    Pe32, Result, read_u32, dll_expect_raw_x86, dll_relative_call_target,
    dll_indirect_import_call, dll_import_bindings,
};

pub const STATE_PREFERRED_VA:u32=0x0090_5010;
pub const STATE_RVA:u32=0x0002_5010;

/// Two different .txt2 descriptor-initializer paths refer to the
/// SAME worker entrypoint at 0x18F81. The DLL-name caller has two
/// explicit state-reset instructions in source order around that
/// worker call. This is NOT a proof that its opaque jumps necessarily
/// execute both stores or that this is the only caller of the worker.
pub const PROCESS_WORKER_CALL:u32=0x1A1B2;
pub const DLL_WORKER_CALL:u32=0x1A39C;
pub const COMMON_WORKER_RVA:u32=0x18F81;
pub const DLL_PRE_WORKER_RESET:u32=0x1A360;
pub const DLL_POST_WORKER_RESET:u32=0x1A3C0;


#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum StateOperation {
    ConditionalAdd4f3,
    AddArgumentLow16,
    AddOutputByte,
    AddStackLocal,
    ResetToZero,
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct StateWrite {
    pub instruction_rva:u32,
    pub operation:StateOperation,
}

// Explicit, instruction-aligned writes to the state DWORD in original
// readable .txt2. They do NOT include pointer-mediated mutations via
// helper routines such as 1A77F, so this is not an exhaustive list of
// effects on the global state.
pub const VERIFIED_DIRECT_WRITES:[StateWrite;7]=[
    StateWrite{instruction_rva:0x198A0,operation:StateOperation::ConditionalAdd4f3},
    StateWrite{instruction_rva:0x19A8B,operation:StateOperation::AddArgumentLow16},
    StateWrite{instruction_rva:0x19DE1,operation:StateOperation::AddOutputByte},
    StateWrite{instruction_rva:0x19E79,operation:StateOperation::AddArgumentLow16},
    StateWrite{instruction_rva:0x19F18,operation:StateOperation::AddStackLocal},
    StateWrite{instruction_rva:0x1A360,operation:StateOperation::ResetToZero},
    StateWrite{instruction_rva:0x1A3C0,operation:StateOperation::ResetToZero},
];

/// A PE32 section is zero-filled at offset >= SizeOfRawData only up
/// to VirtualSize. This models initial image mapping, not the value
/// at a later byte-mixer call (other functions can have changed it).
pub fn virtual_zero_filled(section_rva:u32,raw_len:u32,virtual_size:u32,rva:u32)->bool {
    rva.checked_sub(section_rva).is_some_and(|delta|
        delta>=raw_len && delta<virtual_size)
}

/// x86 NEG followed by SBB EAX,EAX turns the carry flag into
/// either 0 or -1; AND 0x4f3 selects a conditional increment.
pub fn conditional_4f3_delta(original:u32)->u32 {
    if original==0 {0} else {0x4F3}
}

/// Two different original routines add EBP+0x18 masked to 16 bits
/// to the shared state, without knowing those argument values.
pub fn increment_low16(state:u32,argument:u32)->u32 {
    state.wrapping_add(argument & 0xFFFF)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WindowsVersionPath {
    QueryFailure,
    Win32Nt,
    OtherPlatform,
}

/// Historical OSVERSIONINFOA::dwPlatformId is at struct offset
/// 0x10; value 2 is VER_PLATFORM_WIN32_NT.
pub fn get_version_path(api_success:bool,platform_id:u32)->WindowsVersionPath {
    if !api_success {WindowsVersionPath::QueryFailure}
    else if platform_id==2 {WindowsVersionPath::Win32Nt}
    else {WindowsVersionPath::OtherPlatform}
}

/// Proven original lexical sites near 19965, NOT a proven function CFG.
pub const NEAR_WORKER_SETUP_ENTRY:u32=0x19965;
pub const NEAR_SETUP_VERSION_CALL:u32=0x19A4D;
pub const NEAR_SETUP_STATE_ADD:u32=0x19A8B;

/// Conditional composition ONLY if the 1A77F Windows-version query
/// fails and the 19A8B low-16 addition later runs without intervening
/// state mutations. This does not establish actual 19965 reachability.
pub fn hypothetical_near_setup_failure_then_add(
    initial:u32,argument:u32
)->u32 {
    let (masked,_ax)=version_query_failure_state(initial);
    increment_low16(masked,argument)
}

/// Individual exact-original opcode audit of the two nearby state
/// references and the original 1974A call into 19965. Address
/// proximity does not prove a same-function reachable call chain.
pub fn audit_near_worker_setup_sites(
    dll:&[u8],pe:&Pe32
)->Result<()> {
    if !(0x1974A<NEAR_WORKER_SETUP_ENTRY &&
         NEAR_WORKER_SETUP_ENTRY<NEAR_SETUP_VERSION_CALL &&
         NEAR_SETUP_VERSION_CALL<NEAR_SETUP_STATE_ADD &&
         NEAR_SETUP_STATE_ADD<0x19C78) {
        return Err("original worker setup site order changed".into());
    }
    if dll_relative_call_target(dll,pe,0x1974A)?!=NEAR_WORKER_SETUP_ENTRY ||
       dll_relative_call_target(dll,pe,NEAR_SETUP_VERSION_CALL)?!=0x1A77F {
        return Err("original 19965 neighborhood CALL target changed".into());
    }
    dll_expect_raw_x86(dll,pe,0x19A48,&[0x68,0x10,0x50,0x90,0])?;
    dll_expect_raw_x86(dll,pe,0x19A7B,
        &[0x8B,0x45,0x18,0x25,0xFF,0xFF,0,0])?;
    dll_expect_raw_x86(dll,pe,0x19A83,
        &[0x8B,0x0D,0x10,0x50,0x90,0,0x03,0xC8])?;
    dll_expect_raw_x86(dll,pe,NEAR_SETUP_STATE_ADD,
        &[0x89,0x0D,0x10,0x50,0x90,0])?;
    Ok(())
}

/// The x86 sequence reached from the original GetVersionExA-failure
/// JE at 1A7B8 follows a flag-preserving MOV/NOP detour, a JE at
/// 1A846 and a JMP at 1A849 into 1A854. There it loads the passed
/// DWORD pointer, ANDs its value with 0xFF, stores it back and sets
/// AX=1. This is *only* a conditional failure-path effect, not the
/// state after a successful Windows version probe or at mixer entry.
pub fn version_query_failure_state(original:u32)->(u32,u16) {
    (original & 0xFF,1)
}

/// Assert independently located original code bytes and all three
/// control transfers of the 1A77F GetVersionExA failure fallback.
/// No Windows API is invoked and no DLL code is executed.
pub fn audit_version_failure_fallback(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x1A7B6u32,&[0x85,0xC0,0x0F,0x84,0x82,0x00,0x00,0x00][..]),
        (0x1A840,&[0x75,0x09,0x8B,0xC9,0x90,0x90,0x74,0x01,0x90,0xEB,0x09][..]),
        (0x1A854,&[0x8B,0x45,0x08,0x8B,0x08][..]),
        (0x1A859,&[0x81,0xE1,0xFF,0x00,0x00,0x00][..]),
        (0x1A85F,&[0x8B,0x55,0x08,0x89,0x0A][..]),
        (0x1A864,&[0x66,0xB8,0x01,0x00][..]),
        (0x1A868,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    // Numeric targets are derived directly from the signed displacements
    // that audit_version_failure_fallback checks above.
    let query_failure_jump=0x1A7B8u32 + 6 + 0x82;
    let preserved_zero_flag_jump=0x1A846u32 + 2 + 1;
    let unconditional_fallback_jump=0x1A849u32 + 2 + 9;
    if query_failure_jump!=0x1A840 ||
       preserved_zero_flag_jump!=0x1A849 ||
       unconditional_fallback_jump!=0x1A854 {
        return Err("original 1A77F failure-path branch targets changed".into());
    }
    Ok(())
}


/// The OTHER original GetVersionExA helper (1A9B6) has a different
/// failure route. From TEST EAX,EAX at 1AA35 with EAX=0, the original
/// branch network at 1AB34..1AB68 reaches AX=1/return without any
/// dereference/store through its [EBP+8] argument pointer.
/// No claim is made about a successful API query or indirect effects
/// of the actual Windows API call itself.
pub fn version_query_failure_secondary_state(original:u32)->(u32,u16) {
    (original,1)
}

/// Verify a static, conditional EAX=0 path through the original
/// obfuscated 1A9B6 routine. The ZF=1/PF=1/CF=0 flag conditions
/// follow from x86 TEST EAX,EAX on zero; all instructions along this
/// verified route either preserve those flags or set AX before return.
pub fn audit_secondary_version_failure_fallback(
    dll:&[u8],pe:&Pe32
)->Result<()> {
    for (site,bytes) in [
        (0x1AA35u32,&[0x85,0xC0,0x0F,0x84,0xF7,0x00,0x00,0x00][..]),
        (0x1AB34,&[0x7B,0x04][..]), // JNP not taken: PF=1
        (0x1AB36,&[0x8B,0xF6,0xEB,0x0E][..]),
        (0x1AB48,&[0x73,0x0B,0x74,0x07][..]), // JAE taken: CF=0
        (0x1AB53,&[0xEB,0x03,0x73,0xF3][..]),
        (0x1AB58,&[0xEB,0x07,0x8B,0xD2,0x7F,0x06][..]),
        (0x1AB5E,&[0x90,0x7E,0x03][..]), // JG false, JLE true: ZF=1
        (0x1AB61,&[0xEB,0xF7][..]),
        (0x1AB64,&[0x66,0xB8,0x01,0x00,0xEB,0x0D][..]),
        (0x1AB77,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    // This list models the original conditional branch choices on
    // GetVersionExA returning 0. It never executes or emulates the DLL.
    let branch_targets=[
        (0x1AA37u32+6+0xF7,0x1AB34),
        (0x1AB38+2+0x0E,0x1AB48),
        (0x1AB48+2+0x0B,0x1AB55),
        (0x1AB55+2-13,0x1AB4A),
        (0x1AB4A+2+7,0x1AB53),
        (0x1AB53+2+3,0x1AB58),
        (0x1AB58+2+7,0x1AB61),
        (0x1AB61+2-9,0x1AB5A),
        (0x1AB5F+2+3,0x1AB64),
        (0x1AB68+2+13,0x1AB77),
    ];
    if branch_targets.iter().any(|&(actual,expected)| actual!=expected) {
        return Err("1A9B6 conditional failure branch target changed".into());
    }
    Ok(())
}


/// Conditional destinations of 1A77F *after* GetVersionExA succeeds.
/// The helpers can modify the supplied state pointer, and their behavior
/// is NOT modeled here. A non-1/non-2 platform reaches the same 0xFF
/// masking instruction used by the failed API-query path.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum VersionSuccessDispatch {
    Win9xHelper1beee,
    WinNtHelper1ae3f,
    FallbackLowByteMask,
}
pub fn version_success_dispatch(platform_id:u32)->VersionSuccessDispatch {
    match platform_id {
        1 => VersionSuccessDispatch::Win9xHelper1beee,
        2 => VersionSuccessDispatch::WinNtHelper1ae3f,
        _ => VersionSuccessDispatch::FallbackLowByteMask,
    }
}

/// The successful-GetVersionExA path from 1A7B6 initially uses an
/// overlapping-instruction detour: 1A7BE -> 1A7D1 -> 1A7C1 -> 1A7C9
/// -> 1A7D6. TEST EAX,EAX on a nonzero BOOL sets ZF=0 and CF=0.
/// Compare against platform 1, then 2; other platforms reach 1A854.
/// On 1 and 2, these are only dispatched helper calls: their effects
/// and return values are unknown. No raw PE bytes are modified.
pub fn audit_version_success_dispatch(dll:&[u8],pe:&Pe32)->Result<()> {
    // Every anchored opcode group below was independently compared with
    // the exact-hash-verified original DLL before being recorded.
    for (site,bytes) in [
        (0x1A7B6u32,&[0x85,0xC0][..]),
        (0x1A7BE,&[0x75,0x11][..]),  // JNE -> 1A7D1
        (0x1A7D1,&[0x73,0xEE][..]),  // JAE -> 1A7C1
        (0x1A7C1,&[0x0F,0x86,0x0F,0,0,0][..]), // JBE not taken
        (0x1A7C9,&[0x77,0x0B][..]),  // JA -> 1A7D6
        (0x1A7D6,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x01][..]),
        (0x1A7DD,&[0x75,0x2C][..]), // JNE -> 1A80B
        (0x1A7DF,&[0x70,0x0B][..]), // JO not taken after equal CMP 1
        (0x1A7E1,&[0x77,0x07][..]), // JA not taken after equal CMP 1
        (0x1A7E3,&[0x87,0xF6][..]), // flag-preserving self-XCHG
        (0x1A7E5,&[0x74,0x03][..]), // JE -> 1A7EA
        (0x1A7EA,&[0xEB,0x03][..]), // JMP -> 1A7EF
        (0x1A7EF,&[0x8B,0x4D,0x08,0x51][..]), // forward same argument
        (0x1A7F8,&[0x83,0xC4,0x04][..]),
        (0x1A80B,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x02][..]),
        (0x1A812,&[0x75,0x2C][..]), // JNE -> 1A840
        (0x1A814,&[0x73,0x0B][..]), // JAE -> 1A821 after equal CMP
        (0x1A821,&[0x73,0xF3][..]), // JAE -> 1A816 after equal CMP
        (0x1A816,&[0x74,0x07][..]), // JE -> 1A81F
        (0x1A81F,&[0xEB,0x03][..]), // JMP -> 1A824
        (0x1A824,&[0x8B,0x55,0x08,0x52][..]), // forward same argument
        (0x1A82D,&[0x83,0xC4,0x04][..]),
        (0x1A840,&[0x75,0x09][..]), // JNE when CMP 2 failed
        (0x1A84B,&[0x87,0xDB][..]),
        (0x1A84D,&[0x77,0x00][..]),
        (0x1A84F,&[0x75,0xF1][..]), // JNE -> 1A842
        (0x1A842,&[0x8B,0xC9,0x90,0x90][..]),
        (0x1A846,&[0x74,0x01][..]), // JE not taken
        (0x1A849,&[0xEB,0x09][..]), // JMP -> 1A854
        (0x1A854,&[0x8B,0x45,0x08,0x8B,0x08][..]),
        (0x1A859,&[0x81,0xE1,0xFF,0,0,0][..]),
        (0x1A862,&[0x89,0x0A][..]),
        (0x1A864,&[0x66,0xB8,0x01,0x00][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for (site,target) in [
        (0x1A7F3u32,0x1BEEEu32),
        (0x1A828,0x1AE3F),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!("original 1A77F successful platform call changed at 0x{site:05X}"));
        }
    }
    // Verify the two platform helpers' REAL imported dependency:
    // each uses KERNEL32!GetModuleHandleA via IAT 1E098. This does
    // NOT infer the subsequent memory modifications made by helpers.
    let base=read_u32(dll,read_u32(dll,0x3C)? as usize+24+28)?;
    let imports=dll_import_bindings(dll,pe)?;
    if !imports.iter().any(|b|
        b.iat_rva==0x1E098 && b.symbol=="GetModuleHandleA" &&
        b.library.eq_ignore_ascii_case("KERNEL32.dll"))
    {
        return Err("original SafeDisc GetModuleHandleA import identity changed".into());
    }
    for site in [0x1AEC3u32,0x1BF52] {
        let (_,iat)=dll_indirect_import_call(dll,pe,site,base)?;
        if iat!=0x1E098 {
            return Err(format!("platform helper original IAT slot changed at 0x{site:05X}"));
        }
    }
    Ok(())
}


/// Independent platform dispatch of the second GetVersionExA caller,
/// helper 1A9B6. Only a successful API result is classified here.
/// The fallback entry 1AB34 has obfuscated flag-dependent behavior,
/// so the pointer value after that entry is deliberately unspecified.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum SecondaryVersionSuccessDispatch {
    Win9xHelper1c7f0,
    WinNtHelper1bac7,
    FallbackEntry1ab34,
}
pub fn secondary_version_success_dispatch(
    platform_id:u32
)->SecondaryVersionSuccessDispatch {
    match platform_id {
        1 => SecondaryVersionSuccessDispatch::Win9xHelper1c7f0,
        2 => SecondaryVersionSuccessDispatch::WinNtHelper1bac7,
        _ => SecondaryVersionSuccessDispatch::FallbackEntry1ab34,
    }
}

/// Compare original x86 instructions on the restricted nonzero-EAX
/// branch through 1AA9E and 1AB0F. TEST's CF/ZF/PF/SF flags are
/// preserved through nonmutating self-MOV/XCHG/NOP detours until the
/// first CMP of dwPlatformId, and CMP sets the subsequent flags.
/// Independently, a bounded static interpreter walked EAX inputs
/// 1, 0x80000000, 0x11223344 on platform IDs 0,1,2,3,0xFFFFFFFF.
/// No original DLL machine instructions were executed.
pub fn audit_secondary_version_success_dispatch(
    dll:&[u8],pe:&Pe32
)->Result<()> {
    for (site,bytes) in [
        (0x1AA35u32,&[0x85,0xC0][..]),
        (0x1AA37,&[0x0F,0x84,0xF7,0,0,0][..]),
        (0x1AA3D,&[0x76,0x11][..]),
        (0x1AA3F,&[0x77,0x0F][..]),
        (0x1AA50,&[0x78,0xEE][..]),
        (0x1AA52,&[0x79,0xEC][..]),
        (0x1AA40,&[0x0F,0x8B,0x0F,0,0,0][..]),
        (0x1AA46,&[0x8B,0xD2][..]),
        (0x1AA4A,&[0x7A,0x09][..]),
        (0x1AA55,&[0x76,0x07][..]),
        (0x1AA59,&[0x74,0x01][..]),
        (0x1AA5C,&[0xEB,0x0B][..]),
        (0x1AA69,&[0xEB,0x07][..]),
        (0x1AA72,&[0xEB,0xF7][..]),
        (0x1AA6B,&[0x8B,0xFF][..]),
        (0x1AA6D,&[0x73,0x06][..]),
        (0x1AA75,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x01][..]),
        (0x1AA7C,&[0x75,0x48][..]),
        (0x1AA7E,&[0xEB,0x07][..]),
        (0x1AA87,&[0xEB,0xF7][..]),
        (0x1AA80,&[0x8B,0xED][..]),
        (0x1AA82,&[0x7F,0x06][..]),
        (0x1AA85,&[0x7E,0x03][..]),
        (0x1AA8A,&[0x78,0x09][..]),
        (0x1AA8F,&[0x7F,0x09][..]),
        (0x1AA93,&[0x7E,0x05][..]),
        (0x1AA9A,&[0x8B,0x4D,0x08,0x51][..]),
        (0x1AAC6,&[0x78,0x09][..]),
        (0x1AACB,&[0x7F,0x09][..]),
        (0x1AAD6,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x02][..]),
        (0x1AADD,&[0x75,0x55][..]),
        (0x1AADF,&[0x76,0x05][..]),
        (0x1AAE6,&[0x74,0x06][..]),
        (0x1AAEE,&[0x76,0xF1][..]),
        (0x1AAE4,&[0xEB,0x0D][..]),
        (0x1AAF3,&[0x75,0x11][..]),
        (0x1AAF5,&[0x74,0x0F][..]),
        (0x1AB06,&[0x73,0xEE][..]),
        (0x1AAF6,&[0x0F,0x86,0x0F,0,0,0][..]),
        (0x1AB0B,&[0x8B,0x55,0x08,0x52][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for (site,target) in [
        (0x1AA9Eu32,0x1C7F0u32),
        (0x1AB0F,0x1BAC7),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!("1A9B6 original OS platform helper call mismatch at 0x{site:05X}"));
        }
    }
    Ok(())
}


/// x86 EFLAGS subset used by the original obfuscated 1AB34 fallback.
/// All instructions on the verified paths are Jcc/JMP/NOP, self-MOV,
/// self-XCHG, MOV AX,1, and epilogue. No argument-pointer dereference,
/// helper call or writable memory operand occurs on those paths.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FallbackFlags {
    pub zf:bool, pub cf:bool, pub pf:bool, pub sf:bool, pub of:bool,
}

/// Source-level, conditional control-flow model only. Not execution of
/// original x86. The bounded walk fails closed on unrecognized sites
/// or nonconverging branches. The separate original-byte audit anchors
/// all of the sites participating in this model.
pub fn secondary_fallback_steps(flags:FallbackFlags)->Result<usize> {
    let mut pc=0x1AB34u32;
    let mut set_ax=false;
    for steps in 1..=40 {
        pc=match pc {
            0x1AB34 => if !flags.pf {0x1AB3A} else {0x1AB36},
            0x1AB36 => 0x1AB38, // MOV ESI,ESI
            0x1AB38 => 0x1AB48, // JMP +0E
            0x1AB3A => 0x1AB3B, // NOP
            0x1AB3B => 0x1AB3C, // NOP
            0x1AB3C => if !flags.cf && !flags.zf {0x1AB43} else {0x1AB3E},
            0x1AB3E => 0x1AB40, // XCHG ESI,ESI
            0x1AB40 => if flags.zf {0x1AB43} else {0x1AB42},
            0x1AB42 => 0x1AB43, // NOP
            0x1AB43 => if !flags.pf {0x1AB36} else {
                return Err("1AB43 PF changed without a flag-writing instruction".into())
            },
            0x1AB48 => if !flags.cf {0x1AB55} else {0x1AB4A},
            0x1AB4A => if flags.zf {0x1AB53} else {0x1AB4C},
            0x1AB4C => 0x1AB4E, // XCHG EBX,EBX
            0x1AB4E => if !flags.cf && !flags.zf {0x1AB53} else {0x1AB50},
            0x1AB50 => 0x1AB52, // XCHG EBP,EBP
            0x1AB52 => 0x1AB53, // NOP
            0x1AB53 => 0x1AB58, // JMP +03
            0x1AB55 => if !flags.cf {0x1AB4A} else {
                return Err("1AB55 reached with unexpected carry flag".into())
            },
            0x1AB58 => 0x1AB61, // JMP +07
            0x1AB61 => 0x1AB5A, // JMP -09
            0x1AB5A => 0x1AB5C, // MOV EDX,EDX
            0x1AB5C => if !flags.zf && flags.sf==flags.of {0x1AB64} else {0x1AB5E},
            0x1AB5E => 0x1AB5F, // NOP
            0x1AB5F => if flags.zf || flags.sf!=flags.of {0x1AB64} else {0x1AB61},
            0x1AB64 => {set_ax=true;0x1AB68}, // MOV AX,1
            0x1AB68 => 0x1AB77, // JMP +0D
            0x1AB77 if set_ax => return Ok(steps),
            _ => return Err(format!("unknown original 1AB34 branch at RVA 0x{pc:05X}")),
        };
    }
    Err("original 1AB34 fallback did not converge within 40 steps".into())
}

/// Assert every backed instruction group that can be reached by the
/// complete five-flag branch-model on the SHA-pinned retail DLL.
/// The error checks are intentionally strict and read-only.
pub fn audit_secondary_fallback_all_flags(dll:&[u8],pe:&Pe32)->Result<()> {
    for (site,bytes) in [
        (0x1AB34u32,&[0x7B,0x04][..]),
        (0x1AB36,&[0x8B,0xF6][..]),
        (0x1AB38,&[0xEB,0x0E][..]),
        (0x1AB3A,&[0x90,0x90][..]),
        (0x1AB3C,&[0x77,0x05][..]),
        (0x1AB3E,&[0x87,0xF6][..]),
        (0x1AB40,&[0x74,0x01][..]),
        (0x1AB42,&[0x90][..]),
        (0x1AB43,&[0x7B,0xF1][..]),
        (0x1AB48,&[0x73,0x0B][..]),
        (0x1AB4A,&[0x74,0x07][..]),
        (0x1AB4C,&[0x87,0xDB][..]),
        (0x1AB4E,&[0x77,0x03][..]),
        (0x1AB50,&[0x87,0xED][..]),
        (0x1AB52,&[0x90][..]),
        (0x1AB53,&[0xEB,0x03][..]),
        (0x1AB55,&[0x73,0xF3][..]),
        (0x1AB58,&[0xEB,0x07][..]),
        (0x1AB61,&[0xEB,0xF7][..]),
        (0x1AB5A,&[0x8B,0xD2][..]),
        (0x1AB5C,&[0x7F,0x06][..]),
        (0x1AB5E,&[0x90][..]),
        (0x1AB5F,&[0x7E,0x03][..]),
        (0x1AB64,&[0x66,0xB8,0x01,0x00][..]),
        (0x1AB68,&[0xEB,0x0D][..]),
        (0x1AB77,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,site,bytes)?;
    }
    for mask in 0..32u32 {
        let flags=FallbackFlags {
            zf:mask&1!=0,cf:mask&2!=0,pf:mask&4!=0,
            sf:mask&8!=0,of:mask&16!=0,
        };
        secondary_fallback_steps(flags)?;
    }
    Ok(())
}


/// Within the original 1C7F0 Windows-9x-platform helper's file-backed
/// code span, the instructions at 1D2A4..1D2B4 read DWORD *[EBP+8],
/// ADD 0x34F modulo 2^32, then store it through the *same pointer*.
/// This is only the conditional LOCAL transition if execution enters
/// at 1D28F with a valid, writable pointer. The full 1C7F0 path and
/// its final state are NOT recovered.
pub fn conditional_win9x_34f_step(old:u32)->u32 {
    old.wrapping_add(0x34F)
}

/// Two original complementary source pairs surround the state store:
/// 1D28F JNE / 1D297 JE (before ADD), 1D2B4 JNE /
/// 1D2B6 JE (after ADD). The intervening self-MOV, ordinary MOV
/// and memory store do not change ZF; only ADD updates flags.
pub fn win9x_34f_conditional_trace(entry_zf:bool,add_result_zf:bool)->Vec<u32> {
    let mut path=vec![0x1D28F];
    if entry_zf {
        path.extend_from_slice(&[0x1D295,0x1D297]);
    }
    path.extend_from_slice(&[0x1D2A4,0x1D2A7,0x1D2A9,0x1D2AF,0x1D2B2,0x1D2B4]);
    if add_result_zf {path.push(0x1D2B6);}
    path.push(0x1D2C7);
    path
}


/// CMP [EBP+8],0 at 1D26A guards the conditional Win9x block.
/// Zero pointers take the original 1D2FC MOV AX,1 + JMP epilogue.
/// Nonzero pointers fall into a flag-preserving obfuscated join
/// which reaches the ADD 0x34F block without changing the pointer.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Win9xPointerGate {NullPointerReturn,Add34f}

pub fn win9x_pointer_gate(pointer:u32)->Win9xPointerGate {
    if pointer==0 {Win9xPointerGate::NullPointerReturn}
    else {Win9xPointerGate::Add34f}
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Win9xGuardFlags {
    pub cf:bool, pub zf:bool, pub pf:bool, pub sf:bool, pub of:bool,
}

/// Source-level conditional trace, beginning only at 1D274 (after
/// a nonzero pointer succeeded the guard at 1D26A). The original
/// instructions between branches are Jcc, NOP, register self-XCHG
/// or register self-MOV and do not mutate these EFLAGS. The output
/// is a vector of original instruction-boundary RVAs, not a real
/// runtime execution trace.
pub fn win9x_nonnull_gate_trace(f:Win9xGuardFlags)->Result<Vec<u32>> {
    let mut pc=0x1D274u32;
    let mut trace=Vec::new();
    for _ in 0..28 {
        trace.push(pc);
        if pc==0x1D2A4 {return Ok(trace);}
        pc=match pc {
            0x1D274 => if !f.pf {0x1D287} else {0x1D276},
            0x1D276 => if f.pf {0x1D287} else {
                return Err("1D274 JNP/JP flags inconsistent".into())
            },
            0x1D287 => if f.of {0x1D277} else {0x1D289},
            0x1D289 => if !f.of {0x1D277} else {
                return Err("1D287 JO/JNO flags inconsistent".into())
            },
            0x1D277 => if f.sf!=f.of {0x1D28C} else {0x1D27D},
            0x1D27D => 0x1D27E, // NOP
            0x1D27E => 0x1D27F, // NOP
            0x1D27F => if !f.cf && !f.zf {0x1D28C} else {0x1D281},
            0x1D281 => if f.sf==f.of {0x1D28C} else {
                return Err("1D277 JL/JGE flags inconsistent".into())
            },
            0x1D28C => if f.sf!=f.of {0x1D29F} else {0x1D28E},
            0x1D28E => if f.sf==f.of {0x1D29F} else {
                return Err("1D28C JL/JGE flags inconsistent".into())
            },
            0x1D29F => if !f.zf && f.sf==f.of {0x1D28F} else {0x1D2A1},
            0x1D2A1 => if f.zf || f.sf!=f.of {0x1D28F} else {
                return Err("1D29F JG/JLE flags inconsistent".into())
            },
            0x1D28F => if !f.zf {0x1D2A4} else {0x1D295},
            0x1D295 => 0x1D297, // MOV EDX,EDX, EFLAGS unchanged
            0x1D297 => if f.zf {0x1D2A4} else {
                return Err("1D28F JNE/JE flags inconsistent".into())
            },
            _ => return Err(format!(
                "unrecognized original 1D274 guard instruction 0x{pc:05X}"
            )),
        }
    }
    Err("1D274 guard trace did not converge in 28 steps".into())
}

/// Verify the exact original pointer guard, obfuscated path, and null
/// return. This does NOT establish that 1C7F0 reaches 1D26A from
/// its public entry.
pub fn audit_win9x_pointer_guard_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (at,bytes) in [
        (0x1D26Au32,&[0x83,0x7D,0x08,0x00][..]), // CMP [EBP+8],0
        (0x1D26E,&[0x0F,0x84,0x88,0,0,0][..]), // JE 1D2FC
        (0x1D274,&[0x7B,0x11,0x7A,0x0F][..]), // JNP/JP
        (0x1D277,&[0x0F,0x8C,0x0F,0,0,0][..]), // JL
        (0x1D27D,&[0x90,0x90][..]), // NOP NOP
        (0x1D27F,&[0x77,0x0B,0x7D,0x09][..]), // JA/JGE
        (0x1D287,&[0x70,0xEE,0x71,0xEC][..]), // JO/JNO
        (0x1D28C,&[0x7C,0x11,0x7D,0x0F][..]), // JL/JGE
        (0x1D29F,&[0x7F,0xEE,0x7E,0xEC][..]), // JG/JLE
        (0x1D2FC,&[0x66,0xB8,0x01,0x00,0xE9,0x23,0x01,0,0][..]),
        (0x1D428,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }
    for (start,length,displacement,expected) in [
        (0x1D26Ei64,6i64,0x88i64,0x1D2FCi64),
        (0x1D274,2,0x11,0x1D287),
        (0x1D276,2,0x0F,0x1D287),
        (0x1D287,2,-18,0x1D277),
        (0x1D289,2,-20,0x1D277),
        (0x1D277,6,0x0F,0x1D28C),
        (0x1D27F,2,0x0B,0x1D28C),
        (0x1D281,2,0x09,0x1D28C),
        (0x1D28C,2,0x11,0x1D29F),
        (0x1D28E,2,0x0F,0x1D29F),
        (0x1D29F,2,-18,0x1D28F),
        (0x1D2A1,2,-20,0x1D28F),
        (0x1D300,5,0x123,0x1D428),
    ] {
        if start+length+displacement!=expected {
            return Err("original Win9x guard Jcc/JMP destination changed".into());
        }
    }
    Ok(())
}

/// Byte-anchored original-disassembly audit, without loading the DLL
/// or inferring reachability of the conditional 1D28F block.
pub fn audit_conditional_win9x_state_add(dll:&[u8],pe:&Pe32)->Result<()> {
    audit_win9x_pointer_guard_original(dll,pe)?;
    if dll_relative_call_target(dll,pe,0x1AA9E)?!=0x1C7F0 {
        return Err("original 1A9B6 Windows-9x helper call changed".into());
    }
    for (rva,bytes) in [
        (0x1AA9Au32,&[0x8B,0x4D,0x08,0x51][..]), // forward passed DWORD pointer
        (0x1D28F,&[0x0F,0x85,0x0F,0,0,0][..]), // JNE 1D2A4
        (0x1D295,&[0x8B,0xD2,0x74,0x0B][..]), // self-MOV then JE
        (0x1D2A4,&[
            0x8B,0x4D,0x08,         // MOV ECX,[EBP+8]
            0x8B,0x11,              // MOV EDX,[ECX]
            0x81,0xC2,0x4F,0x03,0,0,// ADD EDX,0x34F
            0x8B,0x45,0x08,         // MOV EAX,[EBP+8]
            0x89,0x10,              // MOV [EAX],EDX
        ][..]),
        (0x1D2B4,&[0x75,0x11,0x74,0x0F][..]), // JNE / JE same target
    ] {
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
    }
    for (source,length,delta,target) in [
        (0x1D28Fu32,6u32,0x0Fu32,0x1D2A4u32),
        (0x1D297,2,0x0B,0x1D2A4),
        (0x1D2B4,2,0x11,0x1D2C7),
        (0x1D2B6,2,0x0F,0x1D2C7),
    ] {
        if source+length+delta!=target {
            return Err("original 1D28F state-add branch changed".into());
        }
    }
    Ok(())
}

/// Audit only original file-backed instruction sites and PE geometry.
/// The caller enforces the canonical dplayerx.dll SHA-256 first.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    audit_near_worker_setup_sites(dll,pe)?;
    audit_conditional_win9x_state_add(dll,pe)?;
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    if base!=0x008E_0000 || base+STATE_RVA!=STATE_PREFERRED_VA {
        return Err("original DLL state RVA/image base changed".into());
    }
    let data=pe.section(b".data")?;
    if data.virtual_address!=0x20000 || data.raw_len!=0x3E00 ||
       data.virtual_size!=0x6C80 ||
       !virtual_zero_filled(data.virtual_address,data.raw_len as u32,
                            data.virtual_size,STATE_RVA) {
        return Err("original DLL byte mixer state not in expected PE .data zero-fill".into());
    }

    // The exact original state writer instruction spans:
    // conditional 0x4F3, two low16 argument additions,
    // the per-output-byte add, an extra local add and two resets.
    for (at,bytes) in [
        (0x1988Fu32,&[0xF7,0xD8,0x1B,0xC0,0x25,0xF3,0x04,0x00,0x00][..]),
        (0x19898,&[0x8B,0x15,0x10,0x50,0x90,0x00,0x03,0xD0][..]),
        (0x198A0,&[0x89,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19A7B,&[0x8B,0x45,0x18,0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x19A83,&[0x8B,0x0D,0x10,0x50,0x90,0x00,0x03,0xC8][..]),
        (0x19A8B,&[0x89,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19DE1,&[0x89,0x15,0x10,0x50,0x90,0x00][..]),
        (0x19E69,&[0x8B,0x45,0x18,0x25,0xFF,0xFF,0x00,0x00][..]),
        (0x19E71,&[0x8B,0x0D,0x10,0x50,0x90,0x00,0x03,0xC8][..]),
        (0x19E79,&[0x89,0x0D,0x10,0x50,0x90,0x00][..]),
        (0x19F10,&[0xA1,0x10,0x50,0x90,0x00,0x03,0x45,0xFC][..]),
        (0x19F18,&[0xA3,0x10,0x50,0x90,0x00][..]),
        (0x1A360,&[0xC7,0x05,0x10,0x50,0x90,0x00,0,0,0,0][..]),
        (0x1A3C0,&[0xC7,0x05,0x10,0x50,0x90,0x00,0,0,0,0][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }

    // The two initializer paths call the same worker, and the
    // DLL-specific path contains a state-reset / worker / state-reset
    // ordering in its original file-backed code. Record only
    // instruction/call-site provenance; opaque control-flow
    // reachability is not yet established.
    for site in [PROCESS_WORKER_CALL,DLL_WORKER_CALL] {
        if dll_relative_call_target(dll,pe,site)?!=COMMON_WORKER_RVA {
            return Err(format!("shared 18F81 worker E8 target changed at 0x{site:05X}"));
        }
        dll_expect_raw_x86(dll,pe,site+5,&[0x83,0xC4,0x18])?;
    }
    dll_expect_raw_x86(dll,pe,0x1A38B,&[0x8B,0x55,0xF4,0x89,0x11])?;
    dll_expect_raw_x86(dll,pe,0x1A390,&[0x8B,0x45,0xF8,0x89,0x41,0x04])?;
    dll_expect_raw_x86(dll,pe,0x1A396,&[0x8B,0x55,0xFC,0x89,0x51,0x08])?;
    dll_expect_raw_x86(dll,pe,0x1A3CA,&[0x8D,0x45,0xF4,0x50])?;
    if dll_relative_call_target(dll,pe,0x1A3CE)?!=0xA200 {
        return Err("post-worker A200 module cleanup CALL changed".into());
    }

    // Three site-specific direct calls pass POINTERS to the same
    // global state: two into 1A77F, and an earlier call into 1A9B6.
    // Both helpers contain real GetVersionExA calls. Any pointer-
    // mediated effects on state are outside the seven direct writes.
    for (push_at,call_at,target) in [
        (0x1987Du32,0x19882u32,0x1A9B6u32),
        (0x19A48,0x19A4D,0x1A77F),
        (0x19E2B,0x19E30,0x1A77F),
    ] {
        dll_expect_raw_x86(dll,pe,push_at,&[0x68,0x10,0x50,0x90,0x00])?;
        if dll_relative_call_target(dll,pe,call_at)?!=target {
            return Err(format!("original state-pointer helper call mismatch at 0x{call_at:05X}"));
        }
    }

    // 1A77F builds OSVERSIONINFOA (148 bytes), calls the *actual*
    // original KERNEL32!GetVersionExA import, and branches on
    // OSVERSIONINFOA::dwPlatformId at +0x10. Platform ID 2
    // dispatches down the Win32 NT branch; other IDs down another.
    for (at,bytes) in [
        (0x1A9EAu32,&[0xC7,0x85,0x6C,0xFF,0xFF,0xFF,0x94,0,0,0][..]),
        (0x1AA28,&[0x8D,0x85,0x6C,0xFF,0xFF,0xFF,0x50][..]),
        (0x1AA2F,&[0xFF,0x15,0x58,0xE0,0x8F,0x00][..]),
        (0x1A79Fu32,&[0xC7,0x85,0x6C,0xFF,0xFF,0xFF,0x94,0,0,0][..]),
        (0x1A7A9,&[0x8D,0x85,0x6C,0xFF,0xFF,0xFF,0x50][..]),
        (0x1A7B0,&[0xFF,0x15,0x58,0xE0,0x8F,0x00][..]),
        (0x1A7B6,&[0x85,0xC0,0x0F,0x84,0x82,0,0,0][..]),
        (0x1A80B,&[0x83,0xBD,0x7C,0xFF,0xFF,0xFF,0x02,0x75,0x2C][..]),
        (0x1A824,&[0x8B,0x55,0x08,0x52][..]),
        (0x1A7EF,&[0x8B,0x4D,0x08,0x51][..]),
    ] {
        dll_expect_raw_x86(dll,pe,at,bytes)?;
    }
    for (at,target) in [
        (0x1A828u32,0x1AE3Fu32),
        (0x1A7F3,0x1BEEE),
    ] {
        if dll_relative_call_target(dll,pe,at)?!=target {
            return Err(format!("1A77F platform-specific helper call mismatch at 0x{at:05X}"));
        }
    }
    let (_,iat_slot)=dll_indirect_import_call(dll,pe,0x1A7B0,base)?;
    let (_,other_slot)=dll_indirect_import_call(dll,pe,0x1AA2F,base)?;
    if iat_slot!=0x1E058 || other_slot!=iat_slot {
        return Err("original 1A77F/1A9B6 platform-detection import slots changed".into());
    }
    audit_version_failure_fallback(dll,pe)?;
    audit_version_success_dispatch(dll,pe)?;
    audit_secondary_version_failure_fallback(dll,pe)?;
    audit_secondary_version_success_dispatch(dll,pe)?;
    audit_secondary_fallback_all_flags(dll,pe)?;
    let imports=dll_import_bindings(dll,pe)?;
    if !imports.iter().any(|imp|imp.iat_rva==iat_slot &&
        imp.symbol=="GetVersionExA" &&
        imp.library.eq_ignore_ascii_case("KERNEL32.dll")) {
        return Err("original 1A77F GetVersionExA symbol verification failed".into());
    }
    Ok(())
}


// A PE HIGHLOW relocation on a DWORD equal to STATE_PREFERRED_VA is
// stronger evidence of an absolute state reference than a raw byte-pattern
// search. It still does not establish instruction boundaries, reachability,
// or indirect aliases derived from the pointer at runtime.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum StateRefKind {
    KnownRead,
    KnownWrite,
    KnownAddressPass,
    Unclassified,
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct StateRelocationRef {
    pub operand_rva:u32,
    pub section:String,
    pub kind:StateRefKind,
}

/// Classify only byte operands whose surrounding original instruction
/// bytes are independently anchored by audit_original or
/// unsafedisc_dll_byte_mix::audit_original.
pub fn classify_state_operand(operand_rva:u32)->StateRefKind {
    match operand_rva {
        0x1989A | 0x19A85 | 0x19D50 | 0x19D64 | 0x19D8D |
        0x19DA9 | 0x19DDB | 0x19E73 | 0x19F11 =>
            StateRefKind::KnownRead,
        0x198A2 | 0x19A8D | 0x19DE3 | 0x19E7B | 0x19F19 |
        0x1A362 | 0x1A3C2 => StateRefKind::KnownWrite,
        0x1987E | 0x19A49 | 0x19E2C =>
            StateRefKind::KnownAddressPass,
        _ => StateRefKind::Unclassified,
    }
}

/// Operates on an arbitrary SYNTHETIC file-backed section. Only relocation
/// operand sites are considered, and DWORDs may straddle 4-KiB pages.
/// They may not straddle the end of the supplied file-backed section.
pub fn scan_state_reloc_section(
    section_bytes:&[u8],section_rva:u32,relocation_sites:&[u32],
)->Result<Vec<u32>> {
    let mut hits=Vec::new();
    for &operand_rva in relocation_sites {
        let Some(local)=operand_rva.checked_sub(section_rva) else {continue};
        let local=usize::try_from(local)
            .map_err(|_|"state reference RVA is too large")?;
        let Some(bytes)=section_bytes.get(local..local.saturating_add(4)) else {continue};
        if u32::from_le_bytes(bytes.try_into().unwrap())==STATE_PREFERRED_VA {
            hits.push(operand_rva);
        }
    }
    hits.sort_unstable();
    hits.dedup();
    Ok(hits)
}

/// Run the above test on the *already exact-hash-verified* retail DLL.
/// All sections are examined, including non-executable pointer tables.
/// A ref is a statically relocated absolute DWORD, not necessarily code.
pub fn scan_original_state_relocations(
    dll:&[u8],pe:&Pe32,
)->Result<Vec<StateRelocationRef>> {
    let (blocks,_)=super::pe_relocation_blocks(dll,pe)?;
    let mut relocation_sites=Vec::new();
    for block in &blocks {
        for &offset in &block.highlow_offsets {
            relocation_sites.push(block.page_rva.checked_add(u32::from(offset))
                .ok_or("state relocation RVA overflow")?);
        }
    }
    relocation_sites.sort_unstable();
    relocation_sites.dedup();
    let mut refs=Vec::new();
    for section in &pe.sections {
        let backed=section.raw_len.min(if section.virtual_size==0 {
            section.raw_len
        } else {
            section.virtual_size as usize
        });
        if backed==0 {continue}
        let end=section.raw_offset.checked_add(backed)
            .ok_or("state scan section offset overflow")?;
        let section_bytes=dll.get(section.raw_offset..end)
            .ok_or("state scan section extends beyond verified DLL")?;
        let name=String::from_utf8_lossy(
            section.name.split(|&b|b==0).next().unwrap_or(&[])
        ).to_string();
        for operand_rva in scan_state_reloc_section(
            section_bytes,section.virtual_address,&relocation_sites
        )? {
            refs.push(StateRelocationRef {
                operand_rva,
                section:name.clone(),
                kind:classify_state_operand(operand_rva),
            });
        }
    }
    refs.sort_unstable_by_key(|r|r.operand_rva);
    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn potential_19965_neighborhood_preserves_source_order_not_reachability() {
        assert!(NEAR_WORKER_SETUP_ENTRY<NEAR_SETUP_VERSION_CALL);
        assert!(NEAR_SETUP_VERSION_CALL<NEAR_SETUP_STATE_ADD);
        assert_eq!(NEAR_SETUP_STATE_ADD,0x19A8B);
        // No assertion that these addresses form a reachable CFG path.
    }

    #[test]
    fn conditional_failed_version_then_low16_add_is_not_universal_state() {
        for (initial,arg,expected) in [
            (0u32,0u32,0u32),
            (0x1234_5678,0x100,0x178),
            (0xFFFF_FFFF,1,0x100),
            (0xDEAD_BEEF,0xABCD_1122,0x1211),
        ] {
            assert_eq!(hypothetical_near_setup_failure_then_add(
                initial,arg),expected);
        }
        assert_ne!(hypothetical_near_setup_failure_then_add(
            0x1234_5678,0x100),increment_low16(0x1234_5678,0x100));
    }

    #[test]
    fn relocation_state_scan_ignores_nonrelocated_decoys_and_is_deterministic() {
        let mut bytes=vec![0x90u8;0x1030];
        let va=STATE_PREFERRED_VA.to_le_bytes();
        bytes[5..9].copy_from_slice(&va);       // literal decoy, not relocated
        bytes[0xFFE..0x1002].copy_from_slice(&va); // across a 4KiB page
        // 0x8030 points exactly beyond the file-backed section.
        let relocs=[0x7FFEu32,0x7006,0x7FFE,0x7020,0x8030,0x6FFF];
        assert_eq!(scan_state_reloc_section(&bytes,0x7000,&relocs).unwrap(),
                   vec![0x7FFE]);
    }

    #[test]
    fn relocated_state_pointer_must_match_exact_preferred_va() {
        let mut section=vec![0u8;32];
        section[7..11].copy_from_slice(&STATE_PREFERRED_VA.to_le_bytes());
        assert_eq!(scan_state_reloc_section(&section,0x18000,&[0x18007]).unwrap(),
                   vec![0x18007]);
        section[7]+=1;
        assert!(scan_state_reloc_section(&section,0x18000,&[0x18007]).unwrap().is_empty());
    }

    #[test]
    fn all_nineteen_known_state_operands_have_distinct_classes() {
        let reads=[0x1989A,0x19A85,0x19D50,0x19D64,0x19D8D,
                   0x19DA9,0x19DDB,0x19E73,0x19F11];
        let writes=[0x198A2,0x19A8D,0x19DE3,0x19E7B,0x19F19,
                    0x1A362,0x1A3C2];
        let passed=[0x1987E,0x19A49,0x19E2C];
        let mut all=std::collections::HashSet::new();
        for operand in reads {
            assert_eq!(classify_state_operand(operand),StateRefKind::KnownRead);
            assert!(all.insert(operand));
        }
        for operand in writes {
            assert_eq!(classify_state_operand(operand),StateRefKind::KnownWrite);
            assert!(all.insert(operand));
        }
        for operand in passed {
            assert_eq!(classify_state_operand(operand),StateRefKind::KnownAddressPass);
            assert!(all.insert(operand));
        }
        assert_eq!(all.len(),19);
        assert_eq!(classify_state_operand(0x1A9B6),StateRefKind::Unclassified);
    }

    #[test]
    fn failed_os_version_probe_keeps_only_original_low_byte() {
        for (before,after) in [
            (0u32,0u32), (1,1), (0x1234_5678,0x78),
            (0xDEAD_BEEF,0xEF), (0xFFFF_FF00,0),
            (u32::MAX,0xFF),
        ] {
            assert_eq!(version_query_failure_state(before),(after,1));
        }
    }

    #[test]
    fn successful_version_probe_routes_three_platform_classes() {
        assert_eq!(version_success_dispatch(1),
                   VersionSuccessDispatch::Win9xHelper1beee);
        assert_eq!(version_success_dispatch(2),
                   VersionSuccessDispatch::WinNtHelper1ae3f);
        for other in [0,3,4,0xFFFF_FFFF] {
            assert_eq!(version_success_dispatch(other),
                       VersionSuccessDispatch::FallbackLowByteMask);
        }
    }

    #[test]
    fn nonstandard_platform_fallback_is_distinct_from_helper_effects() {
        assert_eq!(version_success_dispatch(0),
                   VersionSuccessDispatch::FallbackLowByteMask);
        assert_eq!(version_query_failure_state(0x1234_5678).0,0x78);
        // The model does not guess any output for platform=1 or 2:
        // each executes a different, opaque original helper.
        assert_ne!(version_success_dispatch(1),version_success_dispatch(2));
    }

    #[test]
    fn all_thirty_two_fallback_flag_combinations_return_ax_one() {
        let mut min_steps=usize::MAX;
        let mut max_steps=0usize;
        for mask in 0..32u32 {
            let flags=FallbackFlags {
                zf:mask&1!=0,cf:mask&2!=0,pf:mask&4!=0,
                sf:mask&8!=0,of:mask&16!=0,
            };
            let steps=secondary_fallback_steps(flags).unwrap();
            min_steps=min_steps.min(steps);
            max_steps=max_steps.max(steps);
        }
        // Both routes converge without a state-pointer memory access.
        assert_eq!((min_steps,max_steps),(15,26));
    }

    #[test]
    fn second_successful_version_probe_uses_distinct_helpers() {
        assert_eq!(secondary_version_success_dispatch(1),
                   SecondaryVersionSuccessDispatch::Win9xHelper1c7f0);
        assert_eq!(secondary_version_success_dispatch(2),
                   SecondaryVersionSuccessDispatch::WinNtHelper1bac7);
        for other in [0,3,4,0xFFFF_FFFF] {
            assert_eq!(secondary_version_success_dispatch(other),
                       SecondaryVersionSuccessDispatch::FallbackEntry1ab34);
        }
        // Do not conflate the fallback entry with the low-byte mask
        // of the DIFFERENT 1A77F helper.
        assert_ne!(secondary_version_success_dispatch(0),
                   SecondaryVersionSuccessDispatch::WinNtHelper1bac7);
    }

    #[test]
    fn secondary_os_probe_failure_retains_original_state() {
        for before in [0,1,0x1234_5678,0xFFFF_FF00,u32::MAX] {
            assert_eq!(version_query_failure_secondary_state(before),(before,1));
        }
        assert_ne!(version_query_failure_secondary_state(0x1234_5678),
                   version_query_failure_state(0x1234_5678));
    }

    #[test]
    fn fallback_state_transition_is_idempotent_but_not_a_reset() {
        for before in [0,0x0100,0xFF,0xFEDC_BA98,u32::MAX] {
            let (after,result)=version_query_failure_state(before);
            assert_eq!(result,1);
            assert_eq!(version_query_failure_state(after).0,after);
            assert_eq!(after & !0xFF,0);
        }
        assert_ne!(version_query_failure_state(0x1234_5678).0,0);
    }

    #[test]
    fn win9x_pointer_guard_exits_on_null_and_selects_local_add_for_nonnull() {
        assert_eq!(win9x_pointer_gate(0),Win9xPointerGate::NullPointerReturn);
        for value in [1,0x80,0x8000_0000,u32::MAX] {
            assert_eq!(win9x_pointer_gate(value),Win9xPointerGate::Add34f);
        }
    }

    #[test]
    fn all_32_x86_flag_patterns_join_conditional_win9x_add_from_1d274() {
        let mut distinct=std::collections::HashSet::new();
        let mut min=usize::MAX;
        let mut max=0usize;
        for bits in 0u8..32 {
            let flags=Win9xGuardFlags {
                cf:bits&1!=0,zf:bits&2!=0,pf:bits&4!=0,
                sf:bits&8!=0,of:bits&16!=0,
            };
            let trace=win9x_nonnull_gate_trace(flags).unwrap();
            assert_eq!(trace.first(),Some(&0x1D274));
            assert_eq!(trace.last(),Some(&0x1D2A4));
            assert!(!trace.contains(&0x1D2FC));
            distinct.insert(trace.clone());
            min=min.min(trace.len());
            max=max.max(trace.len());
        }
        assert_eq!(distinct.len(),20);
        assert_eq!((min,max),(8,17));
    }

    #[test]
    fn conditional_win9x_add34f_is_wrapping_and_retains_pointer_semantics() {
        for (before,after) in [
            (0u32,0x34Fu32),(1,0x350),(0xFFFF_FF00,0x24F),
            (0xFFFF_FCB1,0),(u32::MAX,0x34E),
            (0x1234_5678,0x1234_59C7),
        ] {
            assert_eq!(conditional_win9x_34f_step(before),after);
        }
    }

    #[test]
    fn conditional_original_add34f_branches_rejoin_for_both_zf_values() {
        for entry_zf in [false,true] {
            for result_zf in [false,true] {
                let trace=win9x_34f_conditional_trace(entry_zf,result_zf);
                assert_eq!(trace.first(),Some(&0x1D28F));
                assert_eq!(trace.last(),Some(&0x1D2C7));
                assert_eq!(trace.iter().filter(|&&pc|pc==0x1D2A9).count(),1);
                assert_eq!(trace.iter().filter(|&&pc|pc==0x1D2B2).count(),1);
                assert_eq!(trace.contains(&0x1D295),entry_zf);
                assert_eq!(trace.contains(&0x1D2B6),result_zf);
            }
        }
    }

    #[test]
    fn original_state_is_in_unbacked_virtual_data() {
        assert!(virtual_zero_filled(0x20000,0x3E00,0x6C80,STATE_RVA));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x23DFF));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x26C80));
        assert!(!virtual_zero_filled(0x20000,0x3E00,0x6C80,0x1FFFF));
    }

    #[test]
    fn conditional_4f3_x86_carry_gate_is_binary() {
        assert_eq!(conditional_4f3_delta(0),0);
        for x in [1u32,0x100,0x8000_0000,u32::MAX]{
            assert_eq!(conditional_4f3_delta(x),0x4F3);
        }
    }

    #[test]
    fn two_low16_additions_ignore_high_argument_bits_and_wrap() {
        assert_eq!(increment_low16(42,0xABCD_1001),42+0x1001);
        assert_eq!(increment_low16(42,0x0000_1001),42+0x1001);
        assert_eq!(increment_low16(u32::MAX,1),0);
    }

    #[test]
    fn exact_state_write_sites_and_categories_are_unique() {
        let sites=VERIFIED_DIRECT_WRITES.map(|w|w.instruction_rva);
        let set:std::collections::HashSet<_>=sites.into_iter().collect();
        assert_eq!(set.len(),7);
        assert_eq!(VERIFIED_DIRECT_WRITES.iter()
                   .filter(|w|w.operation==StateOperation::ResetToZero).count(),2);
        assert_eq!(VERIFIED_DIRECT_WRITES.iter()
                   .filter(|w|w.operation==StateOperation::AddArgumentLow16).count(),2);
        assert!(VERIFIED_DIRECT_WRITES.windows(2).all(|p|
            p[0].instruction_rva<p[1].instruction_rva));
    }

    #[test]
    fn three_address_passing_calls_feed_two_os_version_helpers() {
        let calls=[(0x1987Du32,0x19882u32,0x1A9B6u32),
                   (0x19A48,0x19A4D,0x1A77F),
                   (0x19E2B,0x19E30,0x1A77F)];
        assert_eq!(calls.iter().filter(|&&(_,_,target)|target==0x1A77F).count(),2);
        assert_eq!(calls.iter().filter(|&&(_,_,target)|target==0x1A9B6).count(),1);
        assert!(calls.iter().all(|&(push,call,_)|push+5==call));
    }

    #[test]
    fn two_separate_origins_share_one_worker_entrypoint() {
        assert_ne!(PROCESS_WORKER_CALL,DLL_WORKER_CALL);
        assert_eq!(COMMON_WORKER_RVA,0x18F81);
        assert!(PROCESS_WORKER_CALL<DLL_WORKER_CALL);
    }

    #[test]
    fn dll_reset_worker_reset_have_static_address_order_only() {
        assert!(DLL_PRE_WORKER_RESET<DLL_WORKER_CALL);
        assert!(DLL_WORKER_CALL<DLL_POST_WORKER_RESET);
        // Source-order adjacency does NOT imply execution of either
        // store in the presence of opaque x86 branches.
    }

    #[test]
    fn nt_branch_is_distinguished_from_os_version_query_failure() {
        assert_eq!(get_version_path(false,2),WindowsVersionPath::QueryFailure);
        assert_eq!(get_version_path(true,2),WindowsVersionPath::Win32Nt);
        for platform in [0,1,3,4,u32::MAX] {
            assert_eq!(get_version_path(true,platform),WindowsVersionPath::OtherPlatform);
        }
    }

    #[test]
    fn a77f_version_structure_has_correct_platform_field_offset() {
        // OSVERSIONINFOA: dwOSVersionInfoSize at -0x94, dwPlatformId
        // at -0x84, a relative distance of 16 bytes.
        assert_eq!(0x94u32-0x84,0x10);
        assert_eq!(0x94,148);
    }
}
