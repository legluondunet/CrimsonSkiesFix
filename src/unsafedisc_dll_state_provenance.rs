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

/// Audit only original file-backed instruction sites and PE geometry.
/// The caller enforces the canonical dplayerx.dll SHA-256 first.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
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
    fn relocation_state_scan_ignores_nonrelocated_decoys_and_is_deterministic() {
        let mut bytes=vec![0x90u8;0x1030];
        let va=STATE_PREFERRED_VA.to_le_bytes();
        bytes[5..9].copy_from_slice(&va);       // literal decoy, not relocated
        bytes[0xFFE..0x1002].copy_from_slice(&va); // across a 4KiB page
        // 0x8030 points exactly beyond the file-backed section.
        let relocs=[0x7FFEu32,0x7005,0x7FFE,0x7020,0x8030,0x6FFF];
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
