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
