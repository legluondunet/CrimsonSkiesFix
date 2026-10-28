// Exact-original readable .text helper A310, research-only gate.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This module never executes original x86 or accesses any process memory.

use super::{Pe32,Result,dll_expect_raw_x86,dll_relative_call_target};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum A310Dispatch {
    ReturnAxZeroNullPointer,
    ReturnAxZeroCursorPastLimit,
    ReturnAxZeroNoMemoryMap,
    /// The original function invokes BED0; its returned low AX is
    /// not modeled and must not be guessed to be nonzero.
    DelegateToBED0,
}

/// Original A310 pointer/cursor/map preconditions in precise order.
/// Cursor/end are *values loaded from nonnull DWORD pointers*.
/// Map presence is meaningful only after successful B5D0 lookup
/// by a 12-byte key (separately proven in io_backends).
pub fn a310_source_gate(
    cursor_pointer_present:bool,
    limit_pointer_present:bool,
    cursor:u32,
    limit:u32,
    memory_map_found:bool,
)->A310Dispatch {
    if !cursor_pointer_present || !limit_pointer_present {
        A310Dispatch::ReturnAxZeroNullPointer
    }else if cursor>=limit {
        A310Dispatch::ReturnAxZeroCursorPastLimit
    }else if !memory_map_found {
        A310Dispatch::ReturnAxZeroNoMemoryMap
    }else{
        A310Dispatch::DelegateToBED0
    }
}

/// Original page-cache check adjusts only ALIGNED nonzero
/// cursors back by one byte before comparing page-base
/// addresses (XOR & 0xFFFFF000), avoiding boundary ambiguity.
pub fn a310_page_boundary_probe(cursor:u32)->u32 {
    if cursor!=0 && cursor&0xFFF==0 {
        cursor-1
    }else{
        cursor
    }
}
pub fn a310_same_4k_page(a:u32,b:u32)->bool {
    ((a^b)&0xFFFF_F000)==0
}

/// Verified x86 source-only checks in original file-backed
/// .text, exactly matched only after parent SHA-256 verification.
pub fn audit_original(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,bytes) in [
        (0xA310u32,&[
            0x53,0x55,0x56,0x57,
            0x8B,0x7C,0x24,0x18,0x85,0xFF,
            0x0F,0x84,0xDF,0,0,0,
            0x8B,0x6C,0x24,0x1C,0x85,0xED,
            0x0F,0x84,0xD3,0,0,0,
            0x8B,0x07,0x8B,0x4D,0,0x3B,0xC1,
            0x0F,0x83,0xC6,0,0,0,
        ][..]),
        (0xA339,&[
            0x8B,0x4C,0x24,0x14,0x51,0xE8,0x8D,0x12,0,0,
            0x8B,0xF0,0x83,0xC4,0x04,0x85,0xF6,
        ][..]),
        (0xA350,&[
            0x8B,0x3F,0x8B,0x5D,0,
            0x85,0xFF,0x74,0x09,
            0xF7,0xC7,0xFF,0x0F,0,0,0x75,0x01,0x4F,
            0x8B,0x46,0x34,0x3B,0xC7,0x77,0x11,
            0x3B,0x5E,0x38,0x77,0x0C,
            0x8B,0xD7,0x33,0xD0,
            0xF7,0xC2,0,0xF0,0xFF,0xFF,0x74,0x69,
        ][..]),
        (0xA37A,&[
            0x66,0x83,0x3D,0xD4,0x4A,0x90,0,0,
            0x66,0x8B,0x6C,0x24,0x24,0x74,0x2A,
            0x66,0x85,0xED,0x74,0x10,
        ][..]),
        (0xA38E,&[
            0x53,0x8D,0x46,0x04,0x57,0x50,
            0xE8,0x37,0x1D,0,0,0x83,0xC4,0x0C,
            0xEB,0x0C,0x8D,0x4E,0x04,0x51,
            0xE8,0x19,0x1D,0,0,0x83,0xC4,0x04,
            0x66,0xC7,0x05,0xD4,0x4A,
        ][..]),
        (0xA3B3,&[
            0x66,0x85,0xED,0x74,0x21,
            0x8B,0x56,0x70,0x8B,0x46,0x6C,
            0x6A,0x01,0x53,0x57,0x52,
            0x8D,0x4E,0x04,0x50,0x51,
            0xE8,0xE3,0x1B,0,0,0x83,0xC4,0x18,
            0x66,0xC7,0x05,0xD4,0x4A,0x90,0,1,0,
            0x8B,0x6C,0x24,0x1C,
            0x89,0x7E,0x34,0x89,0x5E,0x38,
        ][..]),
        (0xA3E3,&[
            0x8B,0x54,0x24,0x20,0x8B,0x44,0x24,0x18,
            0x52,0x55,0x83,0xC6,0x14,0x50,0x56,
            0xE8,0xD9,0x1A,0,0,0x83,0xC4,0x10,
            0x5F,0x5E,0x5D,0x5B,0xC3,
        ][..]),
        (0xA3FF,&[0x5F,0x5E,0x5D,0x66,0x33,0xC0,0x5B,0xC3][..]),
    ] {
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
    }
    for (site,target) in [
        (0x196AEu32,0xA310u32),
        (0xA33E,0xB5D0),
        (0xA394,0xC0D0),
        (0xA3A2,0xC0C0),
        (0xA3C8,0xBFB0),
        (0xA3F2,0xBED0),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err(format!("original A310 CALL 0x{site:05X} changed"));
        }
    }
    for (from,len,delta,target) in [
        (0xA31Au32,6u32,0xDFu32,0xA3FFu32),
        (0xA326,6,0xD3,0xA3FF),
        (0xA333,6,0xC6,0xA3FF),
        (0xA38C,2,0x10,0xA39E),
        (0xA39C,2,0x0C,0xA3AA),
    ] {
        if from+len+delta!=target {
            return Err("original A310 branch displacement changed".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_null_cursor_or_limit_before_any_map_access() {
        assert_eq!(a310_source_gate(false,true,0,1,true),
            A310Dispatch::ReturnAxZeroNullPointer);
        assert_eq!(a310_source_gate(true,false,0,1,true),
            A310Dispatch::ReturnAxZeroNullPointer);
    }
    #[test]
    fn a310_unsigned_cursor_limit_checks_include_high_bit_and_wrap() {
        assert_eq!(a310_source_gate(true,true,1,1,true),
            A310Dispatch::ReturnAxZeroCursorPastLimit);
        assert_eq!(a310_source_gate(true,true,u32::MAX,1,true),
            A310Dispatch::ReturnAxZeroCursorPastLimit);
        assert_eq!(a310_source_gate(true,true,1,u32::MAX,true),
            A310Dispatch::DelegateToBED0);
    }
    #[test]
    fn mapped_descriptor_required_before_unknown_bed0_processing() {
        assert_eq!(a310_source_gate(true,true,0,10,false),
            A310Dispatch::ReturnAxZeroNoMemoryMap);
        assert_eq!(a310_source_gate(true,true,0,10,true),
            A310Dispatch::DelegateToBED0);
    }
    #[test]
    fn aligned_nonzero_page_boundary_is_decremented_only_for_probe() {
        assert_eq!(a310_page_boundary_probe(0),0);
        assert_eq!(a310_page_boundary_probe(0x1000),0xFFF);
        assert_eq!(a310_page_boundary_probe(0x2000),0x1FFF);
        assert_eq!(a310_page_boundary_probe(0x1FFF),0x1FFF);
        assert!(a310_same_4k_page(0x1000,0x1FFF));
        assert!(!a310_same_4k_page(0xFFF,0x1000));
    }
}
