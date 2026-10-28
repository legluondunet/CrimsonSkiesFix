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
/// The owner-supplied, unmodified 20,480-byte release was examined in
/// earlier research and has the same core address layout, but it is
/// NOT present for fresh byte comparison in this module.
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
/// displacement bytes. It deliberately accepts only Jcc rel8/JMP rel8
/// or CALL rel32 rather than attempting general disassembly.
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
        0xE8 => {
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
            ebx:index,esi:cursor,edi:48,
        };
        let actual=hypothetical_155_one_word(
            source,index,base,cursor,48,good,second_function).unwrap();
        let expected=second_function(first_function(source^mask)^mask);
        assert_eq!(actual,expected);
        let mut bad=good;bad.ecx^=0x100;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,48,bad,second_function).is_err());
        bad=good;bad.edx=0;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,48,bad,second_function).is_err());
        bad=good;bad.esi=cursor+4;
        assert!(hypothetical_155_one_word(
            source,index,base,cursor,48,bad,second_function).is_err());
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
