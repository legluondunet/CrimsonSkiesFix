// Read-only reconstruction of a historical unSafeDisc 1.5.5
// instruction window from the PUBLIC archived compatibility variant.
//
// This is a symbolic register/provenance model, not executable SafeDisc
// code, a reimplementation of either protected DLL function, or a
// recovered game decryptor.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

/// The archived public unSafeDisc v1.5.5 CODE window 0x401F62..0x401FA7
/// contains two indirect calls separated by XOR EAX,ECX. The second
/// call is located by adding 0x63 to EDX AFTER the first call returns.
/// Its destination is therefore not guaranteed to be DLL+0x6621
/// unless the first callee preserves EDX. The x86 caller also reuses
/// the mask in ECX after the first call, and EBX/ESI/EDI in its loop.
///
/// The exact owner-original 20,480-byte release (SHA-256 gated) has
/// since passed the same read-only opcode audits in ephemeral CI on
/// 2026-10-28. This source file embeds no executable bytes beyond
/// the narrowly anchored instruction windows below.
pub const FIRST_MODIFIER_RVA:u32=0x65BE;
pub const SECOND_OFFSET_FROM_FIRST:u32=0x63;
pub const EXPECTED_SECOND_MODIFIER_RVA:u32=0x6621;

/// Registers visible when the FIRST opaque callee returns.
/// They are explicit synthetic inputs; this module never runs it.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct FirstModifierReturn {
    pub eax:u32,
    pub ecx:u32,
    pub edx:u32,
    pub ebx:u32,
    pub esi:u32,
    pub edi:u32,
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalModifierCallTrace {
    pub mask:u32,
    pub masked_first_input:u32,
    pub first_call_va:u32,
    pub second_call_va:u32,
    pub second_call_eax:u32,
    pub ecx_mask_preserved:bool,
    pub edx_target_preserved:bool,
    pub ebx_index_preserved:bool,
    pub esi_pointer_preserved:bool,
    pub edi_length_preserved:bool,
}

impl HistoricalModifierCallTrace {
    pub fn core_contract_holds(&self)->bool {
        self.ecx_mask_preserved && self.edx_target_preserved &&
        self.ebx_index_preserved && self.esi_pointer_preserved &&
        self.edi_length_preserved
    }
}

/// The historical mask is an OR of four shifted FULL-WIDTH indexes.
/// XOR instead of OR, or truncating index to u8, is incorrect.
pub fn original_index_mask(index:u32)->u32 {
    (index<<24)|(index<<16)|(index<<8)|index
}

/// Symbolically propagate the x86 state between the two CALL EDX
/// instructions in the historical 0x401F62 loop. All arithmetic
/// intentionally uses 32-bit wrapping x86 semantics.
///
/// For a first callee that preserves ECX and EDX:
///   mask = mask(index)
///   first eax = source ^ mask
///   second eax = F1(source ^ mask) ^ mask
///   second target = DLLbase + 0x6621
/// The expression does NOT hold when the opaque callee clobbers ECX
/// or EDX. No claim is made that the real protected modifiers do so.
pub fn trace_first_modifier_return(
    source:u32,index:u32,dll_base:u32,buffer_pointer:u32,
    byte_length:u32,after:FirstModifierReturn,
)->HistoricalModifierCallTrace {
    let mask=original_index_mask(index);
    let first_call_va=dll_base.wrapping_add(FIRST_MODIFIER_RVA);
    HistoricalModifierCallTrace {
        mask,
        masked_first_input:source^mask,
        first_call_va,
        second_call_va:after.edx.wrapping_add(SECOND_OFFSET_FROM_FIRST),
        second_call_eax:after.eax^after.ecx,
        ecx_mask_preserved:after.ecx==mask,
        edx_target_preserved:after.edx==first_call_va,
        ebx_index_preserved:after.ebx==index,
        esi_pointer_preserved:after.esi==buffer_pointer,
        edi_length_preserved:after.edi==byte_length,
    }
}

/// Historical 0x401F68 uses EDI >> 2 and compares against EBX.
/// The loop steps ESI by four; final 1-3 bytes are untouched by
/// THIS extra layer, not necessarily by subsequent TEA processing.
pub fn original_complete_dwords_and_tail(byte_length:u32)->(u32,u32){
    (byte_length>>2,byte_length&3)
}

/// Bounded minimal x86 branch decoder for verified historical sites.
/// Unlike constant arithmetic assertions, this reads the actual input
/// displacement bytes. It deliberately accepts Jcc rel8, JMP rel8,
/// CALL rel32 and JMP rel32, not arbitrary x86 disassembly.
pub fn historical_155_branch_target(
    code:&[u8],code_va:u32,instruction_va:u32
)->Result<u32,String> {
    let start=instruction_va.checked_sub(code_va)
        .ok_or("historical branch precedes CODE section")? as usize;
    let opcode=*code.get(start)
        .ok_or("historical branch outside CODE section")?;
    let (size,delta)=match opcode {
        0x70..=0x7F|0xEB => {
            let d=*code.get(start+1)
                .ok_or("historical rel8 branch truncated")?;
            (2i64,i64::from(d as i8))
        }
        0xE8 | 0xE9 => {
            let end=start.checked_add(5)
                .ok_or("historical rel32 branch offset overflow")?;
            let four=code.get(start+1..end)
                .ok_or("historical rel32 CALL truncated")?;
            let d=i32::from_le_bytes(
                four.try_into().map_err(|_|"historical rel32 width")?);
            (5,i64::from(d))
        }
        _ => return Err("unsupported historical control transfer".into())
    };
    let target=i64::from(instruction_va)+size+delta;
    u32::try_from(target)
        .map_err(|_|"historical control target outside 32-bit address space".into())
}

/// Site-aware read-only x86 audit for the original/archived 1.5.5
/// IMAGE_SCN_MEM_EXECUTE CODE section. The small instruction windows
/// below were independently rechecked against the public archived
/// 18,944-byte version.dll-adapted variant (Git blob 825c467b).
/// An exact owner-original SHA-verified 20,480-byte binary must
/// also pass this audit before the report describes its own bytes.
/// Input is ONLY the mapped CODE section's backed bytes.
pub fn audit_155_entry_to_modifier_loop(
    code:&[u8],image_base:u32,code_rva:u32
)->Result<(),String>{
    let code_va=image_base.checked_add(code_rva)
        .ok_or("historical executable CODE virtual address overflow")?;
    if code_va!=0x0040_1000 {
        return Err("historical 1.5.5 executable CODE base changed".into());
    }
    const WINDOWS:&[(u32,&[u8])]=&[
        (0x0040_1F43,&[
            0x81,0x3D,0x20,0x3C,0x40,0x00,0x00,0x1E,0x02,0x00,
        ]),
        // DLL-length equality -> entry trampoline, mismatch -> RET.
        (0x0040_1F4D,&[
            0x74,0x01,0xC3,0xE8,0x01,0,0,0,0xC3,
        ]),
        // 1F56 maps EBX -> ESI and EAX -> EDI, calls 1F62.
        (0x0040_1F56,&[
            0x60,0x8B,0xF3,0x8B,0xF8,
            0xE8,0x02,0,0,0,0x61,0xC3,
        ]),
        // EBX starts at 0, loop steps through INC EBX on repeats.
        // SHR EDI >>2, CMP EBX,ECX, JAE 1FA6.
        (0x0040_1F62,&[
            0x60,0x33,0xDB,0xEB,0x01,0x43,0x8B,0xCF,
            0xC1,0xE9,0x02,0x3B,0xD9,0x73,0x35,
        ]),
        // OR of four FULL-WIDTH shifted index copies.
        (0x0040_1F71,&[
            0x8B,0xCB,0xC1,0xE1,0x18,0x8B,0xD3,
            0xC1,0xE2,0x10,0x0B,0xCA,0x8B,0xC3,
            0xC1,0xE0,0x08,0x0B,0xC8,0x0B,0xCB,
        ]),
        (0x0040_1F86,&[
            0x8B,0x06,0x33,0xC1,0x8B,0x15,0x5C,0xCD,
            0x70,0x00,0x81,0xC2,0xBE,0x65,0,0,0xFF,0xD2,
        ]),
        // ECX and EDX are NOT reloaded after first CALL EDX.
        (0x0040_1F98,&[
            0x33,0xC1,0x83,0xC2,0x63,0xFF,0xD2,0x89,
            0x06,0x83,0xC6,0x04,0xEB,0xC1,0x61,0xC3,
        ]),
    ];
    for &(va,expected) in WINDOWS {
        let at=va.checked_sub(code_va)
            .ok_or("historical 1.5.5 opcode before CODE start")?
            as usize;
        let end=at.checked_add(expected.len())
            .ok_or("historical 1.5.5 instruction window overflow")?;
        if code.get(at..end)!=Some(expected) {
            return Err(format!(
                "original 1.5.5 opcode mismatch/truncation at VA 0x{va:08X}"
            ));
        }
    }
    // Independently decode six real rel8/rel32 displacements.
    // No untrusted branch target is followed or executed.
    for (source,want) in [
        (0x401F4Du32,0x401F50u32), // JE size-qualified path
        (0x401F50,0x401F56),      // CALL EBX/EAX wrapper
        (0x401F5B,0x401F62),      // CALL extra-DWORD layer
        (0x401F65,0x401F68),      // initial branch past INC EBX
        (0x401F6F,0x401FA6),      // unsigned word-count exit
        (0x401FA4,0x401F67),      // next index before comparison
    ] {
        let found=historical_155_branch_target(code,code_va,source)?;
        if found!=want {
            return Err(format!(
                "historical 1.5.5 branch at 0x{source:08X} -> 0x{found:08X}, expected 0x{want:08X}"
            ));
        }
    }
    Ok(())
}

/// A fail-closed NATIVE RESEARCH ADAPTER for one complete DWORD,
/// dependent on caller-supplied SYNTHETIC output of modifier 1.
/// A successful result means register preservation was explicitly
/// established by the supplied fixture, not by loading a DLL.
///
/// No helper implementation is present. The caller supplies a
/// transparent toy second-stage function to test composition.
pub fn hypothetical_155_one_word<F>(
    source:u32,index:u32,dll_base:u32,
    cursor:u32,byte_length:u32,
    first:FirstModifierReturn,second:F,
)->Result<u32,String>
where F:FnOnce(u32)->u32 {
    if index>=byte_length>>2 {
        return Err("historical modifier index is outside full-DWORD loop".into());
    }
    let trace=trace_first_modifier_return(
        source,index,dll_base,cursor,byte_length,first);
    if !trace.core_contract_holds(){
        return Err("original 1.5.5 register-preservation contract not established".into());
    }
    if trace.second_call_va !=
       dll_base.wrapping_add(EXPECTED_SECOND_MODIFIER_RVA) {
        return Err("historical second modifier target mismatch".into());
    }
    Ok(second(trace.second_call_eax))
}

/// Static, NON-EXECUTING comparison of the two 1.5.5 DLL-startup
/// strategies. The PUBLIC 18,944-byte version.dll-adapted archive has
/// `PUSH 0` flags for LoadLibraryExA and a `JMP +0x1C` that
/// ALWAYS skips the explicit DLL entrypoint call. Earlier private
/// original-file disassembly reported flags=1 and a DS-selector
/// guarded entrypoint call. The original SHA-256-pinned 20,480-byte
/// release was independently rechecked in read-only CI on 2026-10-28.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum HistoricalAttachBranch {
    AlwaysSkipManualAttach,
    SkipWhenDsBit2Clear,
    SkipWhenDsBit2Set,
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalDllStartup {
    pub loadlibrary_flags:u8,
    pub loadlibrary_thunk_va:u32,
    pub attach_branch:HistoricalAttachBranch,
    pub manual_dll_entry_call_va:u32,
    pub skip_target_va:u32,
}

/// Decode only separately anchored, short original x86 windows.
/// The returned fields are independent of any actual Windows loader.
/// Distinguishes LoadLibraryExA(dll,0,0) plus unconditional skip
/// (archived compatibility variant), from LoadLibraryExA(dll,0,1)
/// plus a DS-selector dependent manual DLL_PROCESS_ATTACH (original
/// research note); neither route is executed in this program.
pub fn analyze_155_dll_startup(
    code:&[u8],image_base:u32,code_rva:u32,
)->Result<HistoricalDllStartup,String>{
    let code_va=image_base.checked_add(code_rva)
        .ok_or("historical CODE VA overflow")?;
    if code_va!=0x401000 {
        return Err("historical CODE virtual base is not 0x401000".into());
    }
    let at=|va:u32,n:usize|->Result<&[u8],String>{
        let i=va.checked_sub(code_va)
            .ok_or("historical startup instruction precedes CODE")?
            as usize;
        code.get(i..i.checked_add(n)
            .ok_or("historical startup instruction length overflow")?)
            .ok_or_else(||format!("truncated historical startup opcode at 0x{va:08X}"))
    };
    // MOV EAX, name-VA; PUSH imm8 flags; PUSH imm8 hFile;
    // PUSH EAX name; CALL the imported LoadLibraryExA trampoline.
    if at(0x402D50,1)?!=[0xB8] ||
       at(0x402D55,1)?!=[0x6A] ||
       at(0x402D57,2)?!=[0x6A,0] ||
       at(0x402D59,1)?!=[0x50] ||
       at(0x402D5A,1)?!=[0xE8] {
        return Err("historical DLL load argument setup changed".into());
    }
    let flags=at(0x402D56,1)?[0];
    if flags!=0 && flags!=1 {
        return Err("historical LoadLibraryExA flags are not 0 or 1".into());
    }
    let load_thunk=historical_155_branch_target(
        code,code_va,0x402D5A)?;
    // Validate cached HMODULE, PE e_lfanew/AddressOfEntryPoint
    // selection, the DS test and the original manual call block.
    for (va,pattern) in [
        (0x402D5Fu32,&[0xA3,0x5C,0xCD,0x70,0][..]),
        (0x402D6C,&[0x8B,0xD8,0x03,0x5B,0x3C][..]),
        (0x402D71,&[0x8B,0x5B,0x28,0x03,0xD8][..]),
        (0x402D76,&[0x66,0x8C,0xD9,0xF6,0xC1,0x04][..]),
        (0x402D93,&[0x6A,0,0x6A,1,0x50,0xFF,0xD3,0xC3][..]),
    ] {
        if at(va,pattern.len())?!=pattern {
            return Err(format!(
                "historical DLL startup opcode mismatch at 0x{va:08X}"
            ));
        }
    }
    let attach_branch=match at(0x402D7C,1)?[0] {
        0xEB=>HistoricalAttachBranch::AlwaysSkipManualAttach,
        0x74=>HistoricalAttachBranch::SkipWhenDsBit2Clear,
        0x75=>HistoricalAttachBranch::SkipWhenDsBit2Set,
        _=>return Err("unknown historical DS-test branch form".into()),
    };
    let skip=historical_155_branch_target(code,code_va,0x402D7C)?;
    if skip!=0x402D9A {
        return Err("historical manual DLL attach skip target changed".into());
    }
    Ok(HistoricalDllStartup{
        loadlibrary_flags:flags,
        loadlibrary_thunk_va:load_thunk,
        attach_branch,
        manual_dll_entry_call_va:0x402D98,
        skip_target_va:skip,
    })
}

/// Verified historical 1.5.5 transition from the CODE-bit CRC
/// mixing procedure to the separate DLL-size-gated extra layer.
/// The public archived compatibility-modified binary has these same
/// byte-exact windows; this inspector verifies owner-original input
/// only after the caller's original SHA-256 gate.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalCrcModifierBridge {
    pub codebit:u32,
    pub bypass_checksum_target_va:u32,
    pub after_checksum_call_target_va:u32,
    pub guarded_extra_layer_va:u32,
    pub noncode_reaches_extra_layer:bool,
}

/// Branch structure:
///   401F01 TEST DWORD [60BCD0],0x20
///   401F0B JZ   401F43        ; non-code bypasses CRC ONLY
///   401F0D PUSHAD ...         ; code CRC arithmetic
///   401F3C POPAD
///   401F3D CALL 401F43        ; code rejoins DLL-size gate
///   401F42 RET
///   401F43 CMP DWORD [403C20],0x21E00 ; special layer gate
/// This is independent of whether the higher-level caller selects
/// a given section: it models the behavior IF the caller gets here.
pub fn audit_155_codebit_crc_modifier_bridge(
    code:&[u8],image_base:u32,code_rva:u32,
)->Result<HistoricalCrcModifierBridge,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("historical CRC bridge CODE VA overflow")?;
    if base!=0x401000 {
        return Err("historical CRC bridge CODE base mismatch".into());
    }
    let exact=|va:u32,want:&[u8]|->Result<(),String>{
        let start=va.checked_sub(base)
            .ok_or("historical CRC site precedes CODE")? as usize;
        let end=start.checked_add(want.len())
            .ok_or("historical CRC instruction length overflow")?;
        if code.get(start..end)!=Some(want){
            return Err(format!(
                "historical CRC-to-modifier opcode mismatch at 0x{va:08X}"
            ));
        }
        Ok(())
    };
    exact(0x401F01,&[
        0xF7,0x05,0xD0,0xBC,0x60,0,0x20,0,0,0
    ])?;
    exact(0x401F0B,&[0x74,0x36])?;
    exact(0x401F0D,&[
        0x60,0x85,0xC0,0x7E,0x2A,0xBD,0xB1,0x9A,0x37,0xFD
    ])?;
    exact(0x401F3C,&[
        0x61,0xE8,0x01,0,0,0,0xC3
    ])?;
    exact(0x401F43,&[
        0x81,0x3D,0x20,0x3C,0x40,0,0,0x1E,0x02,0
    ])?;
    let bypass=historical_155_branch_target(code,base,0x401F0B)?;
    let after=historical_155_branch_target(code,base,0x401F3D)?;
    if bypass!=0x401F43 || after!=bypass {
        return Err("CODE and non-CODE paths do not converge at the special-layer gate".into());
    }
    Ok(HistoricalCrcModifierBridge{
        codebit:0x20,
        bypass_checksum_target_va:bypass,
        after_checksum_call_target_va:after,
        guarded_extra_layer_va:0x401F43,
        noncode_reaches_extra_layer:true,
    })
}

/// Upstream constructor linking the protected PE's original NT
/// header pointer to the selected-header list at [403B17].
/// The archived x86 assumes PE32's standard 0xE0 optional
/// header length: 4-byte signature + 20-byte COFF + 0xE0
/// gives 0xF8 bytes from NT header to section table.
/// Crucially, XOR ECX,ECX at 4024A5 is upstream of both
/// MOV CX,[DOS+3C] and MOV CX,[NT+6] on the audited path.
/// Therefore ECX upper 16 bits ARE ZERO on that path.
/// The e_lfanew 16-bit read is a separate historical limitation.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalSectionTableProvenance{
    pub nt_header_save_va:u32,
    pub section_count_field_offset:u32,
    pub section_count_global_va:u32,
    pub fixed_section_table_delta:u32,
    pub section_table_global_va:u32,
    pub source_count_keeps_ecx_upper_half:bool,
    pub ecx_high_word_zeroed_on_audited_path:bool,
    pub dos_e_lfanew_read_width_bits:u8,
}
pub fn audit_155_section_table_provenance(
    code:&[u8],image_base:u32,code_rva:u32,
)->Result<HistoricalSectionTableProvenance,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("historical PE section table CODE VA overflow")?;
    if base!=0x401000 {
        return Err("historical PE section table CODE base differs".into());
    }
    let exact=|va:u32,bytes:&[u8]|->Result<(),String>{
        let i=va.checked_sub(base)
            .ok_or("historical PE section table opcode before CODE")? as usize;
        let end=i.checked_add(bytes.len())
            .ok_or("historical PE section table opcode extent overflow")?;
        if code.get(i..end)!=Some(bytes){
            return Err(format!("historical PE header provenance opcode differs at 0x{va:08X}"));
        }
        Ok(())
    };
    // The code first establishes ECX=0, reads ONLY WORD e_lfanew
    // at DOS+0x3C into CX, computes the NT pointer, validates
    // PE signature, and never modifies ECX again before the
    // second MOV CX at 4024E4. This EXACT contiguous window
    // establishes the zero upper half, not a guessed caller state.
    exact(0x4024A5,&[
        0x33,0xC9,0x66,0x8B,0x4E,0x3C,0x03,0xF1,0x3B,0xF3,
        0x0F,0x87,0x49,0x04,0,0,
        0x81,0x3E,0x50,0x45,0,0,
        0x0F,0x85,0x3D,0x04,0,0,
        0x66,0x83,0x7E,0xFE,0,0x75,0x06,
        0x66,0xC7,0x46,0xFE,0x2B,0xAD,
        0xC7,0x46,0x08,0x31,0x39,0x31,0x31,
        0x89,0x35,0x13,0x3B,0x40,0,
        0x8B,0x7E,0x54,0x03,0x3D,0x0F,0x3B,0x40,0,
        0x66,0x8B,0x4E,0x06,
    ])?;
    // Save NT pointer and store WORD section count in zero-high ECX.
    exact(0x4024D5,&[0x89,0x35,0x13,0x3B,0x40,0])?;
    exact(0x4024E4,&[0x66,0x8B,0x4E,0x06])?; // MOV CX,[ESI+6]
    exact(0x4024E8,&[0x89,0x0D,0x3B,0x3B,0x40,0])?; // store full ECX
    // Fixed NT-to-section-table offset and global pointer store.
    exact(0x402514,&[
        0x81,0xC6,0xF8,0,0,0,
        0x89,0x35,0x17,0x3B,0x40,0,
    ])?;
    // The later header offset multiplication reuses FULL ECX
    // from [403B3B], so a nonzero original high half would matter.
    exact(0x402522,&[
        0xB8,0x28,0,0,0,0xF7,0x2D,0x3B,0x3B,0x40,0,
    ])?;
    Ok(HistoricalSectionTableProvenance{
        nt_header_save_va:0x403B13,
        section_count_field_offset:6,
        section_count_global_va:0x403B3B,
        fixed_section_table_delta:0xF8,
        section_table_global_va:0x403B17,
        source_count_keeps_ecx_upper_half:true,
        ecx_high_word_zeroed_on_audited_path:true,
        dos_e_lfanew_read_width_bits:16,
    })
}
/// Faithfully model the isolated x86 MOV CX,WORD PTR [ESI+6],
/// as opposed to MOVZX. This instruction preserves the high
/// 16 bits, BUT the audited preceding XOR ECX,ECX proves they
/// are zero along this normal original-image execution path.
pub fn historical_155_mov_cx_section_count(ecx_before:u32,count:u16)->u32{
    (ecx_before&0xFFFF_0000)|u32::from(count)
}
/// Safely model the proper PE32 header offset rather than blindly
/// importing the historical fixed 0xF8 for nonstandard optional
/// header lengths. A native caller must use its parsed COFF size.
pub fn native_pe32_section_table_delta(
    optional_header_size:u16
)->Result<u32,String>{
    if optional_header_size<0x60 {
        return Err("PE32 optional header is too short".into());
    }
    Ok(24+u32::from(optional_header_size))
}

/// The historical 1.5.5 section selection routine builds a
/// list of 32-bit POINTERS to PE section headers, later consumed by
/// 401D06. Its count is read from [403B3B], source section-table
/// address from [403B17], destination pointer list from [403957].
/// No historical process pointers are dereferenced by this model.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalSectionListBuilder {
    pub source_header_pointer_va:u32,
    pub section_count_va:u32,
    pub pointer_list_destination_va:u32,
    pub first_name_comparison_va:u32,
    pub skip_header_va:u32,
    pub append_pointer_va:u32,
    pub advance_header_va:u32,
    pub iteration_back_va:u32,
    pub section_header_stride:u32,
    pub pointer_list_stride:u32,
}
/// Recheck the actual archived x86 instructions at 402257..4022B2
/// including all eight 4-byte name-prefix compares, skip branches,
/// source 40-byte stride and destination 4-byte pointer append.
/// This proves the bytecode's filter, not that any particular source
/// file was processed through it.
pub fn audit_155_section_list_builder(
    code:&[u8],image_base:u32,code_rva:u32,
)->Result<HistoricalSectionListBuilder,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("historical section list CODE address overflow")?;
    if base!=0x401000 {
        return Err("historical section list CODE base mismatch".into());
    }
    let checked=|va:u32,bytes:&[u8]|->Result<(),String>{
        let i=va.checked_sub(base)
            .ok_or("section list instruction before CODE")? as usize;
        let end=i.checked_add(bytes.len())
            .ok_or("section list instruction extent overflow")?;
        if code.get(i..end)!=Some(bytes) {
            return Err(format!("historical section list opcode mismatch at 0x{va:08X}"));
        }
        Ok(())
    };
    checked(0x402257,&[
        0xA1,0x17,0x3B,0x40,0x00, // EAX = section table
        0x8B,0x0D,0x3B,0x3B,0x40,0x00, // ECX = number of sections
        0x8B,0x3D,0x57,0x39,0x40,0x00, // EDI = pointer-list destination
    ])?;
    const EXCLUDED:[[u8;4];8]=[
        *b".rsr",*b".bss",*b"BSS\0",*b".tls",
        *b".ida",*b".rda",*b".eda",*b".rel",
    ];
    for (i,prefix) in EXCLUDED.iter().enumerate() {
        let va=0x402268u32+(i as u32)*8;
        // CMP DWORD PTR [EAX],imm32; JE 4022AD
        let pattern=[0x81,0x38,prefix[0],prefix[1],
                     prefix[2],prefix[3]];
        checked(va,&pattern)?;
        let dest=historical_155_branch_target(code,base,va+6)?;
        if dest!=0x4022AD {
            return Err(format!("historical section skip JE at 0x{:08X} targets 0x{dest:08X}",va+6));
        }
    }
    checked(0x4022A8,&[
        0x89,0x07,            // MOV [EDI],EAX
        0x83,0xC7,0x04,      // ADD EDI,4
        0x83,0xC0,0x28,      // ADD EAX,40
        0xE2,0xB6,           // LOOP 402268
    ])?;
    // LOOP rel8 (E2) is outside the generic rel8 Jcc decoder:
    // explicitly verify the two's-complement delta and target.
    let loop_at=0x4022B0u32;
    let i=(loop_at-base) as usize;
    let tail=code.get(i..i+2)
        .ok_or("historical section LOOP truncated")?;
    if tail[0]!=0xE2 ||
       (i64::from(loop_at)+2+i64::from(tail[1] as i8))!=0x402268 {
        return Err("historical section LOOP does not return to first name filter".into());
    }
    Ok(HistoricalSectionListBuilder{
        source_header_pointer_va:0x403B17,
        section_count_va:0x403B3B,
        pointer_list_destination_va:0x403957,
        first_name_comparison_va:0x402268,
        skip_header_va:0x4022AD,
        append_pointer_va:0x4022A8,
        advance_header_va:0x4022AD,
        iteration_back_va:0x4022B0,
        section_header_stride:40,
        pointer_list_stride:4,
    })
}
/// Native, bounds-checked model of the historical eight-prefix filter.
/// Returns indexes of selected headers IN ORIGINAL ORDER; unlike the
/// old x86 routine it neither stores 32-bit host pointers nor relies
/// on an implicitly initialized termination cell.
pub fn historical_155_selected_section_indices(
    headers:&[u8],number_of_headers:usize,
)->Result<Vec<usize>,String>{
    const EXCLUDED:[[u8;4];8]=[
        *b".rsr",*b".bss",*b"BSS\0",*b".tls",
        *b".ida",*b".rda",*b".eda",*b".rel",
    ];
    let size=number_of_headers.checked_mul(40)
        .ok_or("section header byte length overflow")?;
    if number_of_headers>96 || headers.len()!=size {
        return Err("bounded historical section headers must be exactly 40*N bytes (N<=96)".into());
    }
    let mut selected=Vec::new();
    for (i,record) in headers.chunks_exact(40).enumerate(){
        let first:&[u8]=&record[..4];
        if !EXCLUDED.iter().any(|candidate|&candidate[..]==first){
            selected.push(i);
        }
    }
    Ok(selected)
}

/// Examine only STATIC on-disk DATA bytes for the selected-section
/// pointer list used by 402262 MOV EDI,[403957]. The public modified
/// archive contains 0x403C90 in that global, and its initial list
/// slots are zero. This is a disk-state finding, not proof of runtime
/// zeroing, safe repeated calls, or the owner's original file content.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalPointerListDiskState {
    pub pointer_global_va:u32,
    pub initial_pointer_list_va:u32,
    pub capacity_words_in_backed_data:usize,
    pub leading_zero_words:usize,
}
pub fn inspect_155_pointer_list_disk_state(
    backed_data:&[u8],data_rva:u32,image_base:u32,
)->Result<HistoricalPointerListDiskState,String>{
    let data_va=image_base.checked_add(data_rva)
        .ok_or("historical DATA image-base overflow")?;
    let at=0x403957u32.checked_sub(data_va)
        .ok_or("historical pointer-list global precedes DATA")? as usize;
    let end=at.checked_add(4)
        .ok_or("pointer-list global slot overflow")?;
    let bytes=backed_data.get(at..end)
        .ok_or("historical pointer-list global not file-backed")?;
    let target=u32::from_le_bytes(bytes.try_into()
        .map_err(|_|"historical pointer-list global size mismatch")?);
    let relative=target.checked_sub(data_va)
        .ok_or("historical pointer-list points before DATA")? as usize;
    if relative%4!=0 || relative>=backed_data.len(){
        return Err("historical pointer-list is unaligned or not file-backed".into());
    }
    let capacity_words=(backed_data.len()-relative)/4;
    if capacity_words==0 {
        return Err("historical pointer-list has no complete DWORD".into());
    }
    let mut zeros=0usize;
    for word in backed_data[relative..].chunks_exact(4) {
        if word!=[0u8;4]{break;}
        zeros+=1;
    }
    Ok(HistoricalPointerListDiskState{
        pointer_global_va:0x403957,
        initial_pointer_list_va:target,
        capacity_words_in_backed_data:capacity_words,
        leading_zero_words:zeros,
    })
}

/// The archived x86 iterator has a compact PUSHAD/POPAD
/// trampoline: first entry jumps past POPAD; every completed section
/// jumps BACK to POPAD before reading the next pointer. The input to
/// this audit is CODE section bytes, never the game or executable
/// being inspected as instructions.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalSectionIterator {
    pub first_entry_va:u32,
    pub next_entry_restore_va:u32,
    pub header_stride_bytes:u32,
    pub characteristics_field_offset:u32,
    pub raw_size_field_offset:u32,
    pub virtual_size_field_offset:u32,
    pub raw_pointer_field_offset:u32,
    pub relocated_branch_va:u32,
    pub relocation_return_through_va:u32,
}

/// Confirm the control-flow and the on-disk PE section header field
/// loads in the PUBLIC archived 1.5.5 program. When invoked by the
/// SHA-gated original CLI, a differing private original fails closed.
pub fn audit_155_section_iterator(
    code:&[u8],image_base:u32,code_rva:u32
)->Result<HistoricalSectionIterator,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("original section iterator CODE VA overflow")?;
    if base!=0x401000 {
        return Err("original section iterator CODE base differs".into());
    }
    let expect=|va:u32,pattern:&[u8]|->Result<(),String>{
        let i=va.checked_sub(base)
            .ok_or("original section iterator instruction before CODE")?
            as usize;
        if code.get(i..i.checked_add(pattern.len())
            .ok_or("original section iterator instruction size overflow")?)
            !=Some(pattern) {
            return Err(format!("original 1.5.5 section iterator x86 mismatch at 0x{va:08X}"));
        }
        Ok(())
    };
    // MOV EDI,[403957]; JMP +1; POPAD (subsequent entries only);
    // MOV ESI,[EDI]; ADD EDI,4; OR ESI,ESI; JNE +2; POPAD; RET.
    expect(0x401CFD,&[
        0x8B,0x3D,0x57,0x39,0x40,0,
        0xEB,0x01,0x61,0x8B,0x37,0x83,0xC7,0x04,
        0x0B,0xF6,0x75,0x02,0x61,0xC3
    ])?;
    // MOV EAX,[ESI+0x24]; MOV [60BCD0],EAX; PUSHAD;
    // MOV EAX,[ESI+0x10]; MOV EDI,[ESI+0x0C];
    // MOV [60BCD4],EDI; CMP EAX,[ESI+0x08];
    // JBE +9; CMP DWORD [ESI+8],0; JE +3;
    // MOV EAX,[ESI+8]. Raw size is capped by nonzero VS.
    expect(0x401D11,&[
        0x8B,0x46,0x24,0xA3,0xD0,0xBC,0x60,0,0x60,
        0x8B,0x46,0x10,0x8B,0x7E,0x0C,
        0x89,0x3D,0xD4,0xBC,0x60,0,
        0x3B,0x46,0x08,0x76,0x09,0x83,0x7E,0x08,0,
        0x74,0x03,0x8B,0x46,0x08,
    ])?;
    // MOV ESI,[ESI+0x14] (PointerToRawData);
    // ADD ESI,[403B0F] (historical file-memory mapping);
    // MOV [60BCB4],ESI (current section data);
    expect(0x401D34,&[
        0x8B,0x76,0x14,0x03,0x35,0x0F,0x3B,0x40,0,
        0x89,0x35,0xB4,0xBC,0x60,0
    ])?;
    // CMP DWORD [ESI-4],4 (relocation-aware mode) / JE 401D82.
    expect(0x401D48,&[0x83,0x7E,0xFC,0x04,0x74,0x34])?;
    // The standard walker returns to 401D05 after its final chunk.
    // The relocation walker performs POPAD at 401EDD then makes a
    // near JMP to 401D79, where POP ESI and a scratch-pointer store
    // precede the same section-iterator POPAD at 401D05.
    expect(0x401D77,&[0xEB,0x8C])?;
    expect(0x401D79,&[
        0x5E,0x89,0x35,0xCC,0xBC,0x60,0,0xEB,0x83
    ])?;
    expect(0x401EDD,&[0x61,0xE9,0x96,0xFE,0xFF,0xFF])?;
    let targets=[
        (0x401D03u32,0x401D06u32), // first entry skips POPAD
        (0x401D0D,0x401D11), // nonzero section pointer
        (0x401D29,0x401D34), // raw_size<=virtual_size
        (0x401D2F,0x401D34), // virtual_size==0
        (0x401D4C,0x401D82), // relocation-aware path
        (0x401D77,0x401D05), // simple path after remainder
        (0x401D80,0x401D05), // section iteration restoration
        (0x401EDE,0x401D79), // relocation walker -> POP ESI -> POPAD
    ];
    for (src,want) in targets {
        let got=historical_155_branch_target(code,base,src)?;
        if got!=want {
            return Err(format!(
                "1.5.5 section iterator branch at 0x{src:08X} targets 0x{got:08X}, expected 0x{want:08X}"
            ));
        }
    }
    Ok(HistoricalSectionIterator{
        first_entry_va:0x401D06,
        next_entry_restore_va:0x401D05,
        header_stride_bytes:4, // POINTER table stride, NOT section-header size
        characteristics_field_offset:0x24,
        raw_size_field_offset:0x10,
        virtual_size_field_offset:0x08,
        raw_pointer_field_offset:0x14,
        relocated_branch_va:0x401D82,
        relocation_return_through_va:0x401D79,
    })
}

/// Exact SizeOfRawData vs nonzero VirtualSize selection derived
/// from 401D1A..401D34. The underlying PE section-header size
/// remains 40 bytes; its pointer-list stride above is four bytes.
pub fn historical_155_extent(raw_size:u32,virtual_size:u32)->u32 {
    if virtual_size==0 {raw_size}
    else {raw_size.min(virtual_size)}
}

/// Safe native-port-only mapping of selected section bytes using
/// the original PE32 file offsets. No original executable or game
/// byte material is embedded and no actual DLL is called.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalSectionWindow{
    pub rva:u32,
    pub file_offset:u32,
    pub selected_extent:u32,
    pub characteristics:u32,
    pub checksum_eligible:bool,
}
pub fn historical_155_section_window(
    header:&[u8],file_length:usize
)->Result<HistoricalSectionWindow,String>{
    if header.len()!=40 {
        return Err("historical section header must contain exactly 40 bytes".into());
    }
    let read=|at:usize|u32::from_le_bytes(
        [header[at],header[at+1],header[at+2],header[at+3]]
    );
    let size=historical_155_extent(read(0x10),read(0x08));
    let file_offset=read(0x14);
    let end=u64::from(file_offset)+u64::from(size);
    if end>file_length as u64 {
        return Err("historical selected section extent exceeds backed file".into());
    }
    let characteristics=read(0x24);
    Ok(HistoricalSectionWindow{
        rva:read(0x0C),file_offset,selected_extent:size,
        characteristics,checksum_eligible:characteristics&0x20!=0,
    })
}

/// The ordinary full-page and final-tail walkers, as well as the
/// relocation-aware chunk walker, each CALL THE SAME helper at
/// 401F01. That helper's CODE-bit test controls the CRC only, and
/// has a separate DLL-size-gated modifier bridge at 401F43.
/// These are CALL-target facts, not dynamic execution proofs.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalCrcCallers {
    pub full_page_call_va:u32,
    pub tail_call_va:u32,
    pub relocation_call_va:u32,
    pub shared_helper_va:u32,
}
pub fn audit_155_crc_helper_callers(
    code:&[u8],image_base:u32,code_rva:u32
)->Result<HistoricalCrcCallers,String>{
    let base=image_base.checked_add(code_rva)
        .ok_or("historical CRC callers CODE VA overflow")?;
    if base!=0x401000 {
        return Err("historical CRC caller CODE virtual base differs".into());
    }
    const CALLS:&[(u32,[u8;5])]=&[
        (0x401D62,[0xE8,0x9A,0x01,0x00,0x00]),
        (0x401D72,[0xE8,0x8A,0x01,0x00,0x00]),
        (0x401ED8,[0xE8,0x24,0x00,0x00,0x00]),
    ];
    for &(va,bytes) in CALLS {
        let at=va.checked_sub(base)
            .ok_or("historical CRC CALL before CODE start")? as usize;
        if code.get(at..at.checked_add(5).ok_or("CRC CALL length overflow")?)
            !=Some(&bytes[..]) {
            return Err(format!("historical CRC CALL byte mismatch at VA 0x{va:08X}"));
        }
        let target=historical_155_branch_target(code,base,va)?;
        if target!=0x401F01 {
            return Err(format!("historical CRC CALL at VA 0x{va:08X} targets 0x{target:08X}"));
        }
    }
    Ok(HistoricalCrcCallers{
        full_page_call_va:0x401D62,
        tail_call_va:0x401D72,
        relocation_call_va:0x401ED8,
        shared_helper_va:0x401F01,
    })
}

/// The original static length gate at 401F43 is exactly 0x21E00.
/// This says the helper stage is *selected for execution* if its
/// caller reached that gate, not that title-specific callees are
/// present, decoded or callable under a modern OS.
pub fn historical_extra_layer_selected(dll_size:u32)->bool {
    dll_size==0x21E00
}

/// The gate's immediately reached wrapper 401F56 saves all general
/// registers (PUSHAD), copies EBX=buffer to ESI and EAX=length to
/// EDI, then CALLs the inner two-modifier loop at 401F62.
/// A conditional symbolic bridge only: no original code is run.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct HistoricalWrapperArgs {
    pub buffer_pointer_esi:u32,
    pub byte_length_edi:u32,
    pub full_dwords:u32,
    pub untouched_tail_bytes:u32,
}
pub fn conditional_155_wrapper_args(
    caller_ebx:u32,caller_eax:u32,dll_size:u32,
)->Option<HistoricalWrapperArgs>{
    if !historical_extra_layer_selected(dll_size){return None;}
    let (full_dwords,untouched_tail_bytes)=
        original_complete_dwords_and_tail(caller_eax);
    Some(HistoricalWrapperArgs {
        buffer_pointer_esi:caller_ebx,
        byte_length_edi:caller_eax,
        full_dwords,untouched_tail_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preserved(
        source:u32,index:u32,base:u32,pointer:u32,length:u32,
        first_output:u32
    )->HistoricalModifierCallTrace {
        trace_first_modifier_return(source,index,base,pointer,length,
            FirstModifierReturn{
                eax:first_output,ecx:original_index_mask(index),
                edx:base.wrapping_add(FIRST_MODIFIER_RVA),
                ebx:index,esi:pointer,edi:length,
            })
    }

    #[test]
    fn short_and_near_x86_branch_decoder_handles_sign_and_bounds() {
        let code=[
            0x74u8,0x03,        // 1000 -> 1005
            0xEB,0xFC,          // 1002 -> 1000 (negative)
            0xE8,0xFB,0xFF,0xFF,0xFF, // 1004 -> 1004
            0x73,0xFD,         // 1009 -> 1008
        ];
        assert_eq!(historical_155_branch_target(
            &code,0x1000,0x1000).unwrap(),0x1005);
        assert_eq!(historical_155_branch_target(
            &code,0x1000,0x1002).unwrap(),0x1000);
        assert_eq!(historical_155_branch_target(
            &code,0x1000,0x1004).unwrap(),0x1004);
        assert_eq!(historical_155_branch_target(
            &code,0x1000,0x1009).unwrap(),0x1008);
        // E9 rel32 must decode negative, not just E8 CALL rel32.
        assert_eq!(historical_155_branch_target(
            &[0xE9,0xFB,0xFF,0xFF,0xFF],0x3000,0x3000
        ).unwrap(),0x3000);
        assert!(historical_155_branch_target(
            &code[..1],0x1000,0x1000).is_err());
        assert!(historical_155_branch_target(
            &code[..6],0x1000,0x1004).is_err());
        assert!(historical_155_branch_target(
            &[0x90],0x1000,0x1000).is_err());
        assert!(historical_155_branch_target(
            &code,0x1000,0x0FFF).is_err());
        assert!(historical_155_branch_target(
            &[0xEB,0x7F],u32::MAX-1,u32::MAX-1).is_err());
    }

    #[test]
    fn archived_loader_strategy_is_not_original_manual_attach() {
        let mut code=vec![0u8;0x2000];
        let mut put=|va:u32,bytes:&[u8]|{
            let i=(va-0x401000) as usize;
            code[i..i+bytes.len()].copy_from_slice(bytes);
        };
        put(0x402D50,&[0xB8,0x41,0x37,0x40,0,0x6A,0,
                      0x6A,0,0x50,0xE8,0xAD,0x00,0,0]);
        put(0x402D5F,&[0xA3,0x5C,0xCD,0x70,0]);
        put(0x402D6C,&[0x8B,0xD8,0x03,0x5B,0x3C,0x8B,
                      0x5B,0x28,0x03,0xD8]);
        put(0x402D76,&[0x66,0x8C,0xD9,0xF6,0xC1,0x04,
                      0xEB,0x1C]);
        put(0x402D93,&[0x6A,0,0x6A,1,0x50,0xFF,0xD3,0xC3]);
        let archived=analyze_155_dll_startup(
            &code,0x400000,0x1000).unwrap();
        assert_eq!(archived,HistoricalDllStartup{
            loadlibrary_flags:0,loadlibrary_thunk_va:0x402E0C,
            attach_branch:HistoricalAttachBranch::AlwaysSkipManualAttach,
            manual_dll_entry_call_va:0x402D98,skip_target_va:0x402D9A,
        });
        // Synthetic original-style flags=1 and conditional guard.
        code[(0x402D56-0x401000) as usize]=1;
        code[(0x402D7C-0x401000) as usize]=0x74;
        let original_style=analyze_155_dll_startup(
            &code,0x400000,0x1000).unwrap();
        assert_eq!(original_style.loadlibrary_flags,1);
        assert_eq!(original_style.attach_branch,
                   HistoricalAttachBranch::SkipWhenDsBit2Clear);
        code[(0x402D7C-0x401000) as usize]=0x75;
        assert_eq!(analyze_155_dll_startup(
            &code,0x400000,0x1000).unwrap().attach_branch,
            HistoricalAttachBranch::SkipWhenDsBit2Set);
        // Fail closed for unsupported and mutated control flows.
        code[(0x402D7C-0x401000) as usize]=0x90;
        assert!(analyze_155_dll_startup(&code,0x400000,0x1000).is_err());
        code[(0x402D7C-0x401000) as usize]=0xEB;
        code[(0x402D7D-0x401000) as usize]=0x1B;
        assert!(analyze_155_dll_startup(&code,0x400000,0x1000).is_err());
        code[(0x402D7D-0x401000) as usize]=0x1C;
        code[(0x402D56-0x401000) as usize]=2;
        assert!(analyze_155_dll_startup(&code,0x400000,0x1000).is_err());
    }

    #[test]
    fn startup_decoder_rejects_truncation_and_wrong_pe_base() {
        assert!(analyze_155_dll_startup(
            &[0u8;4],0x400000,0x1000).is_err());
        assert!(analyze_155_dll_startup(
            &[0u8;0x2000],0x400001,0x1000).is_err());
        assert!(analyze_155_dll_startup(
            &[0u8;0x2000],u32::MAX,0x1000).is_err());
    }

    #[test]
    fn archived_original_pe_header_provenance_preserves_ecx_high_word(){
        let mut code=vec![0u8;0x2000];
        let windows:&[(u32,&[u8])]=&[
            (0x4024A5,&[
                0x33,0xC9,0x66,0x8B,0x4E,0x3C,0x03,0xF1,0x3B,0xF3,
                0x0F,0x87,0x49,0x04,0,0,
                0x81,0x3E,0x50,0x45,0,0,
                0x0F,0x85,0x3D,0x04,0,0,
                0x66,0x83,0x7E,0xFE,0,0x75,0x06,
                0x66,0xC7,0x46,0xFE,0x2B,0xAD,
                0xC7,0x46,0x08,0x31,0x39,0x31,0x31,
                0x89,0x35,0x13,0x3B,0x40,0,
                0x8B,0x7E,0x54,0x03,0x3D,0x0F,0x3B,0x40,0,
                0x66,0x8B,0x4E,0x06,
            ]),
            (0x4024D5,&[0x89,0x35,0x13,0x3B,0x40,0]),
            (0x4024E4,&[0x66,0x8B,0x4E,0x06]),
            (0x4024E8,&[0x89,0x0D,0x3B,0x3B,0x40,0]),
            (0x402514,&[0x81,0xC6,0xF8,0,0,0,
                0x89,0x35,0x17,0x3B,0x40,0]),
            (0x402522,&[0xB8,0x28,0,0,0,0xF7,0x2D,0x3B,0x3B,0x40,0]),
        ];
        for &(va,b) in windows{
            let i=(va-0x401000) as usize;
            code[i..i+b.len()].copy_from_slice(b);
        }
        assert_eq!(audit_155_section_table_provenance(
            &code,0x400000,0x1000).unwrap(),
            HistoricalSectionTableProvenance{
                nt_header_save_va:0x403B13,
                section_count_field_offset:6,
                section_count_global_va:0x403B3B,
                fixed_section_table_delta:0xF8,
                section_table_global_va:0x403B17,
                source_count_keeps_ecx_upper_half:true,
                ecx_high_word_zeroed_on_audited_path:true,
                dos_e_lfanew_read_width_bits:16,
            });
        assert_eq!(historical_155_mov_cx_section_count(0,6),6);
        // On the verified straight-line path, XOR ECX,ECX
        // makes the isolated high-word preservation harmless.
        assert_eq!(historical_155_mov_cx_section_count(0,7),7);
        assert_eq!(historical_155_mov_cx_section_count(0xBEEF_0001,7),
                   0xBEEF_0007);
        assert_eq!(native_pe32_section_table_delta(0xE0).unwrap(),0xF8);
        assert_eq!(native_pe32_section_table_delta(0xF0).unwrap(),0x108);
        assert!(native_pe32_section_table_delta(0x5F).is_err());
        for &(va,_) in windows {
            let i=(va-0x401000) as usize;
            code[i]^=1;
            assert!(audit_155_section_table_provenance(
                &code,0x400000,0x1000).is_err());
            code[i]^=1;
        }
        assert!(audit_155_section_table_provenance(
            &code[..0x300],0x400000,0x1000).is_err());
        assert!(audit_155_section_table_provenance(
            &code,0x400001,0x1000).is_err());
    }

    #[test]
    fn historical_pointer_list_builder_matches_eight_exact_name_prefixes(){
        let mut code=vec![0u8;0x2000];
        let windows:&[(u32,&[u8])]=&[
            (0x402257,&[0xA1,0x17,0x3B,0x40,0,
                0x8B,0x0D,0x3B,0x3B,0x40,0,
                0x8B,0x3D,0x57,0x39,0x40,0]),
            (0x402268,&[0x81,0x38,0x2E,0x72,0x73,0x72,0x74,0x3D]),
            (0x402270,&[0x81,0x38,0x2E,0x62,0x73,0x73,0x74,0x35]),
            (0x402278,&[0x81,0x38,0x42,0x53,0x53,0x00,0x74,0x2D]),
            (0x402280,&[0x81,0x38,0x2E,0x74,0x6C,0x73,0x74,0x25]),
            (0x402288,&[0x81,0x38,0x2E,0x69,0x64,0x61,0x74,0x1D]),
            (0x402290,&[0x81,0x38,0x2E,0x72,0x64,0x61,0x74,0x15]),
            (0x402298,&[0x81,0x38,0x2E,0x65,0x64,0x61,0x74,0x0D]),
            (0x4022A0,&[0x81,0x38,0x2E,0x72,0x65,0x6C,0x74,0x05]),
            (0x4022A8,&[0x89,0x07,0x83,0xC7,0x04,0x83,0xC0,0x28,0xE2,0xB6]),
        ];
        for &(va,bytes) in windows{
            let i=(va-0x401000) as usize;
            code[i..i+bytes.len()].copy_from_slice(bytes);
        }
        let result=audit_155_section_list_builder(&code,0x400000,0x1000).unwrap();
        assert_eq!(result.section_header_stride,40);
        assert_eq!(result.pointer_list_stride,4);
        assert_eq!(result.source_header_pointer_va,0x403B17);
        assert_eq!(result.section_count_va,0x403B3B);
        assert_eq!(result.pointer_list_destination_va,0x403957);
        for &(va,_) in windows{
            let i=(va-0x401000) as usize;
            code[i]^=1;
            assert!(audit_155_section_list_builder(&code,0x400000,0x1000).is_err());
            code[i]^=1;
        }
        // A modified skip displacement must fail even if the opcode holds.
        let i=(0x40226F-0x401000) as usize;
        code[i]^=1;
        assert!(audit_155_section_list_builder(&code,0x400000,0x1000).is_err());
        code[i]^=1;
        let i=(0x4022B1-0x401000) as usize;
        code[i]^=1;
        assert!(audit_155_section_list_builder(&code,0x400000,0x1000).is_err());
        assert!(audit_155_section_list_builder(&code[..0x500],0x400000,0x1000).is_err());
        assert!(audit_155_section_list_builder(&code,0x400001,0x1000).is_err());
    }

    #[test]
    fn public_style_pointer_list_backing_can_have_zero_terminators(){
        // Synthetic DATA section 0x403000..0x403D00 with an in-file
        // pointer global and allocated zeroed list beginning 403C90.
        let mut data=vec![0u8;0xD00];
        let global=(0x403957-0x403000) as usize;
        data[global..global+4].copy_from_slice(&0x403C90u32.to_le_bytes());
        let first=inspect_155_pointer_list_disk_state(
            &data,0x3000,0x400000).unwrap();
        assert_eq!(first.pointer_global_va,0x403957);
        assert_eq!(first.initial_pointer_list_va,0x403C90);
        assert_eq!(first.capacity_words_in_backed_data,28);
        assert_eq!(first.leading_zero_words,28);
        // No guaranteed runtime terminator: a stale entry on disk
        // demonstrably ends the initial run of zeros.
        let slot=0xC90+7*4;
        data[slot..slot+4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(inspect_155_pointer_list_disk_state(
            &data,0x3000,0x400000).unwrap().leading_zero_words,7);
        data[global..global+4].copy_from_slice(&0x403C91u32.to_le_bytes());
        assert!(inspect_155_pointer_list_disk_state(
            &data,0x3000,0x400000).is_err());
        data[global..global+4].copy_from_slice(&0x404000u32.to_le_bytes());
        assert!(inspect_155_pointer_list_disk_state(
            &data,0x3000,0x400000).is_err());
        assert!(inspect_155_pointer_list_disk_state(
            &data[..0x200],0x3000,0x400000).is_err());
        assert!(inspect_155_pointer_list_disk_state(
            &data,0x3000,u32::MAX).is_err());
    }

    #[test]
    fn historical_bounded_section_filter_keeps_order_and_four_byte_aliases(){
        let names=[
            b".text\0\0\0",b".rsrc\0\0\0",b".data\0\0\0",
            b".idata\0\0",b".reloc\0\0",b".bss\0\0\0\0",
            b"ABCDE123",b"BSS\0NAME",b".rdata\0\0",
            b".rsrtest",b".edata\0\0",b".tls\0\0\0\0",
        ];
        let mut records=vec![0u8;40*names.len()];
        for (i,name) in names.iter().enumerate(){
            records[i*40..i*40+8].copy_from_slice(&name[..]);
        }
        assert_eq!(
            historical_155_selected_section_indices(&records,names.len()).unwrap(),
            vec![0,2,6]);
        // The filter is exactly four bytes, not an 8-byte name match:
        // \".rsrtest\" and \".rsrc\" both compare equal to \".rsr\".
        assert!(historical_155_selected_section_indices(&records[..39],1).is_err());
        assert!(historical_155_selected_section_indices(&records,97).is_err());
        assert!(historical_155_selected_section_indices(&[],0).unwrap().is_empty());
    }

    #[test]
    fn historical_pushed_state_section_iterator_matches_archived_opcodes(){
        let mut code=vec![0u8;0x2000];
        let windows:&[(u32,&[u8])]=&[
            (0x401CFD,&[0x8B,0x3D,0x57,0x39,0x40,0,
                0xEB,0x01,0x61,0x8B,0x37,0x83,0xC7,0x04,
                0x0B,0xF6,0x75,0x02,0x61,0xC3]),
            (0x401D11,&[0x8B,0x46,0x24,0xA3,0xD0,0xBC,0x60,0,0x60,
                0x8B,0x46,0x10,0x8B,0x7E,0x0C,0x89,0x3D,
                0xD4,0xBC,0x60,0,0x3B,0x46,0x08,0x76,0x09,
                0x83,0x7E,0x08,0,0x74,0x03,0x8B,0x46,0x08]),
            (0x401D34,&[0x8B,0x76,0x14,0x03,0x35,0x0F,0x3B,0x40,0,
                0x89,0x35,0xB4,0xBC,0x60,0]),
            (0x401D48,&[0x83,0x7E,0xFC,0x04,0x74,0x34]),
            (0x401D77,&[0xEB,0x8C]),
            (0x401D79,&[0x5E,0x89,0x35,0xCC,0xBC,0x60,0,0xEB,0x83]),
            (0x401EDD,&[0x61,0xE9,0x96,0xFE,0xFF,0xFF]),
        ];
        for &(va,bytes) in windows{
            let i=(va-0x401000) as usize;
            code[i..i+bytes.len()].copy_from_slice(bytes);
        }
        let audit=audit_155_section_iterator(
            &code,0x400000,0x1000).unwrap();
        assert_eq!(audit.first_entry_va,0x401D06);
        assert_eq!(audit.next_entry_restore_va,0x401D05);
        assert_eq!(audit.header_stride_bytes,4);
        assert_eq!(audit.relocation_return_through_va,0x401D79);
        assert_eq!(audit.raw_size_field_offset,0x10);
        assert_eq!(audit.virtual_size_field_offset,0x08);
        assert_eq!(audit.characteristics_field_offset,0x24);
        for &(va,_) in windows {
            let i=(va-0x401000) as usize;
            code[i]^=0x80;
            assert!(audit_155_section_iterator(
                &code,0x400000,0x1000).is_err());
            code[i]^=0x80;
        }
        let branch=(0x401D4D-0x401000) as usize;
        code[branch]=0x35;
        assert!(audit_155_section_iterator(
            &code,0x400000,0x1000).is_err());
        assert!(audit_155_section_iterator(
            &code[..0x100],0x400000,0x1000).is_err());
    }

    #[test]
    fn historical_section_window_caps_raw_data_and_preserves_data(){
        let mut header=[0u8;40];
        let set=|buf:&mut [u8;40],at:usize,n:u32|{
            buf[at..at+4].copy_from_slice(&n.to_le_bytes());
        };
        set(&mut header,0x08,0x123);
        set(&mut header,0x0C,0x1000);
        set(&mut header,0x10,0x200);
        set(&mut header,0x14,0x200);
        set(&mut header,0x24,0x60000020);
        let w=historical_155_section_window(&header,0x500).unwrap();
        assert_eq!(w.selected_extent,0x123);
        assert_eq!(w.rva,0x1000);
        assert_eq!(w.file_offset,0x200);
        assert!(w.checksum_eligible);
        // .data can be selected even with no CODE bit: only CRC differs.
        set(&mut header,0x08,0x300);
        set(&mut header,0x24,0xC0000040);
        assert_eq!(historical_155_section_window(&header,0x500)
                       .unwrap().selected_extent,0x200);
        assert!(!historical_155_section_window(&header,0x500)
                        .unwrap().checksum_eligible);
        set(&mut header,0x08,0);
        assert_eq!(historical_155_extent(0x200,0),0x200);
        assert!(historical_155_section_window(&header,0x350).is_err());
        assert!(historical_155_section_window(&header[..39],0x500).is_err());
        set(&mut header,0x14,u32::MAX);
        assert!(historical_155_section_window(&header,0x500).is_err());
    }

    #[test]
    fn historical_three_chunk_paths_call_same_crc_and_modifier_helper(){
        let mut code=vec![0u8;0x2000];
        let cases:&[(u32,[u8;5])]=&[
            (0x401D62,[0xE8,0x9A,0x01,0,0]),
            (0x401D72,[0xE8,0x8A,0x01,0,0]),
            (0x401ED8,[0xE8,0x24,0,0,0]),
        ];
        for &(va,bytes) in cases{
            let i=(va-0x401000) as usize;
            code[i..i+5].copy_from_slice(&bytes);
        }
        assert_eq!(
            audit_155_crc_helper_callers(&code,0x400000,0x1000).unwrap(),
            HistoricalCrcCallers{
                full_page_call_va:0x401D62,
                tail_call_va:0x401D72,
                relocation_call_va:0x401ED8,
                shared_helper_va:0x401F01,
            });
        for &(va,_) in cases{
            let i=(va-0x401000) as usize;
            code[i+1]^=1;
            assert!(audit_155_crc_helper_callers(
                &code,0x400000,0x1000).is_err());
            code[i+1]^=1;
        }
        assert!(audit_155_crc_helper_callers(
            &code[..0x500],0x400000,0x1000).is_err());
        assert!(audit_155_crc_helper_callers(
            &code,0x400001,0x1000).is_err());
    }

    #[test]
    fn historical_checksum_codebit_skips_crc_not_extra_layer() {
        let mut code=vec![0u8;0x2000];
        let patterns:&[(u32,&[u8])]=&[
            (0x401F01,&[0xF7,0x05,0xD0,0xBC,0x60,0,0x20,0,0,0]),
            (0x401F0B,&[0x74,0x36]),
            (0x401F0D,&[0x60,0x85,0xC0,0x7E,0x2A,0xBD,0xB1,0x9A,0x37,0xFD]),
            (0x401F3C,&[0x61,0xE8,0x01,0,0,0,0xC3]),
            (0x401F43,&[0x81,0x3D,0x20,0x3C,0x40,0,0,0x1E,0x02,0]),
        ];
        for &(va,bytes) in patterns {
            let i=(va-0x401000) as usize;
            code[i..i+bytes.len()].copy_from_slice(bytes);
        }
        let good=audit_155_codebit_crc_modifier_bridge(
            &code,0x400000,0x1000).unwrap();
        assert_eq!(good,HistoricalCrcModifierBridge{
            codebit:0x20,
            bypass_checksum_target_va:0x401F43,
            after_checksum_call_target_va:0x401F43,
            guarded_extra_layer_va:0x401F43,
            noncode_reaches_extra_layer:true,
        });
        for &(va,_) in patterns {
            let i=(va-0x401000) as usize;
            code[i]^=1;
            assert!(audit_155_codebit_crc_modifier_bridge(
                &code,0x400000,0x1000).is_err());
            code[i]^=1;
        }
        let skip=(0x401F0C-0x401000) as usize;
        code[skip]=0x35;
        assert!(audit_155_codebit_crc_modifier_bridge(
            &code,0x400000,0x1000).is_err());
        code[skip]=0x36;
        let call=(0x401F3E-0x401000) as usize;
        code[call]=0x02;
        assert!(audit_155_codebit_crc_modifier_bridge(
            &code,0x400000,0x1000).is_err());
        assert!(audit_155_codebit_crc_modifier_bridge(
            &code[..0x400],0x400000,0x1000).is_err());
        assert!(audit_155_codebit_crc_modifier_bridge(
            &code,0x400000,0x1001).is_err());
    }

    #[test]
    fn historical_length_guard_and_wrapper_register_mapping() {
        for size in [0u32,0x21DFF,0x21E01,u32::MAX] {
            assert!(!historical_extra_layer_selected(size));
            assert!(conditional_155_wrapper_args(3,9,size).is_none());
        }
        let args=conditional_155_wrapper_args(
            0x0123_4567,0x1003,0x21E00).unwrap();
        assert_eq!(args,HistoricalWrapperArgs{
            buffer_pointer_esi:0x0123_4567,
            byte_length_edi:0x1003,
            full_dwords:0x400,
            untouched_tail_bytes:3,
        });
        assert_eq!(conditional_155_wrapper_args(
            0x8000_0000,3,0x21E00).unwrap().full_dwords,0);
    }

    #[test]
    fn exact_historical_opcode_audit_fails_closed_on_mutation() {
        // Small, 0x2000-byte pure SYNTHETIC CODE fixture; never
        // copies the executable or any original game data.
        let mut fixture=vec![0u8;0x2000];
        let windows:&[(u32,&[u8])]=&[
            (0x401F43,&[0x81,0x3D,0x20,0x3C,0x40,0,0,0x1E,0x02,0]),
            (0x401F4D,&[0x74,0x01,0xC3,0xE8,0x01,0,0,0,0xC3]),
            (0x401F56,&[0x60,0x8B,0xF3,0x8B,0xF8,0xE8,0x02,0,0,0,0x61,0xC3]),
            (0x401F62,&[0x60,0x33,0xDB,0xEB,0x01,0x43,0x8B,0xCF,
                        0xC1,0xE9,0x02,0x3B,0xD9,0x73,0x35]),
            (0x401F71,&[0x8B,0xCB,0xC1,0xE1,0x18,0x8B,0xD3,
                        0xC1,0xE2,0x10,0x0B,0xCA,0x8B,0xC3,
                        0xC1,0xE0,0x08,0x0B,0xC8,0x0B,0xCB]),
            (0x401F86,&[0x8B,0x06,0x33,0xC1,0x8B,0x15,0x5C,0xCD,
                        0x70,0,0x81,0xC2,0xBE,0x65,0,0,0xFF,0xD2]),
            (0x401F98,&[0x33,0xC1,0x83,0xC2,0x63,0xFF,0xD2,0x89,
                        0x06,0x83,0xC6,0x04,0xEB,0xC1,0x61,0xC3]),
        ];
        for &(va,bytes) in windows {
            let at=(va-0x401000) as usize;
            fixture[at..at+bytes.len()].copy_from_slice(bytes);
        }
        assert!(audit_155_entry_to_modifier_loop(
            &fixture,0x400000,0x1000).is_ok());
        assert!(audit_155_entry_to_modifier_loop(
            &fixture,0x400001,0x1000).is_err());
        for &(va,_) in windows {
            let at=(va-0x401000) as usize;
            fixture[at]^=0x80;
            assert!(audit_155_entry_to_modifier_loop(
                &fixture,0x400000,0x1000).is_err());
            fixture[at]^=0x80;
        }
        assert!(audit_155_entry_to_modifier_loop(
            &fixture[..0x900],0x400000,0x1000).is_err());
    }

    #[test]
    fn recovered_two_indirect_calls_require_preserved_edx() {
        let r=preserved(0xDEAD_BEEF,257,0x006E_0000,0x3000,0x100,0x1234_5678);
        assert!(r.core_contract_holds());
        assert_eq!(r.mask,0x0101_0101);
        assert_eq!(r.masked_first_input,0xDFAC_BFEE);
        assert_eq!(r.first_call_va,0x006E_65BE);
        assert_eq!(r.second_call_va,0x006E_6621);
        assert_eq!(r.second_call_eax,0x1335_5779);
        assert_eq!(FIRST_MODIFIER_RVA+SECOND_OFFSET_FROM_FIRST,
                   EXPECTED_SECOND_MODIFIER_RVA);
    }

    #[test]
    fn edx_and_ecx_are_x86_caller_saved_not_implicitly_reloaded() {
        let base=0x006E_0000u32;
        let index=1u32;
        let first=base+FIRST_MODIFIER_RVA;
        let changed=trace_first_modifier_return(
            0, index,base,0x1000,16,
            FirstModifierReturn{
                eax:0x1234,ecx:0xABCD,edx:first.wrapping_add(1),
                ebx:index,esi:0x1000,edi:16,
            });
        assert!(!changed.core_contract_holds());
        assert!(!changed.ecx_mask_preserved);
        assert!(!changed.edx_target_preserved);
        assert_eq!(changed.second_call_eax,0xB9F9);
        assert_eq!(changed.second_call_va,base+0x6622);
    }

    #[test]
    fn caller_reuses_index_pointer_and_length_after_call() {
        let good=preserved(3,2,0x10000,0x9000,0x200,9);
        assert!(good.core_contract_holds());
        let bad=trace_first_modifier_return(3,2,0x10000,0x9000,0x200,
            FirstModifierReturn{
                eax:9,ecx:original_index_mask(2),
                edx:0x165BE,ebx:3,esi:0x9004,edi:0x201,
            });
        assert!(bad.ecx_mask_preserved && bad.edx_target_preserved);
        assert!(!bad.ebx_index_preserved);
        assert!(!bad.esi_pointer_preserved);
        assert!(!bad.edi_length_preserved);
        assert!(!bad.core_contract_holds());
    }

    #[test]
    fn original_or_mask_retains_full_width_index_aliases() {
        assert_eq!(original_index_mask(0),0);
        assert_eq!(original_index_mask(1),0x0101_0101);
        assert_eq!(original_index_mask(256),0x0101_0100);
        assert_eq!(original_index_mask(257),original_index_mask(1));
        assert_eq!(original_index_mask(1023),0xFFFF_FFFF);
        // Test against the independent module's historical OR mask.
        for i in 0..=1023 {
            assert_eq!(original_index_mask(i),
                       crate::unsafedisc_indexed_affine::index_mask(i));
        }
    }

    #[test]
    fn synthetic_first_second_modifiers_compose_only_under_register_contract() {
        let source=0x4AF5_1807u32;
        let index=259u32;
        let mask=original_index_mask(index);
        let base=0x006E_0000u32;
        let cursor=0x0000_3000u32;
        let first_function=|x:u32|x.rotate_left(7).wrapping_add(0xABCD_1234);
        let second_function=|x:u32|x.rotate_right(9)^0x1234_ABCD;
        let f1=first_function(source^mask);
        let good=FirstModifierReturn{
            eax:f1,ecx:mask,
            edx:base+FIRST_MODIFIER_RVA,
            ebx:index,esi:cursor,edi:1040,
        };
        let actual=hypothetical_155_one_word(
            source,index,base,cursor,1040,good,second_function).unwrap();
        let expected=second_function(first_function(source^mask)^mask);
        assert_eq!(actual,expected);
        let mut bad=good;bad.ecx^=0x100;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,1040,bad,second_function).is_err());
        bad=good;bad.edx=0;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,1040,bad,second_function).is_err());
        bad=good;bad.esi=cursor+4;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,1040,bad,second_function).is_err());
        // Three bytes cannot enter the original whole-DWORD loop.
        assert!(hypothetical_155_one_word(
            source,0,base,cursor,3,good,second_function).is_err());
        assert!(hypothetical_155_one_word(
            source,260,base,cursor,1040,good,second_function).is_err());
    }

    #[test]
    fn original_extra_layer_truncates_only_full_dword_count() {
        for length in 0u32..=4096 {
            let (count,tail)=original_complete_dwords_and_tail(length);
            assert_eq!(count,length/4);
            assert_eq!(tail,length%4);
            assert_eq!(count*4+tail,length);
            assert!(tail<4);
        }
        assert_eq!(original_complete_dwords_and_tail(u32::MAX),
                   (0x3FFF_FFFF,3));
    }

    #[test]
    fn register_additions_wrap_to_original_x86_word_width() {
        let r=preserved(0,u32::MAX,0xFFFF_FFFF,0x8000_0000,4,0xDEAD_BEEF);
        assert_eq!(r.first_call_va,0x65BD);
        assert_eq!(r.second_call_va,0x6620);
        assert!(r.core_contract_holds());
    }
}
