// Read-only, synthetic-only bounds model for unSafeDisc 1.5.5's
// historical PE section-header pointer-list builder and iterator.
//
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unsafedisc_historical_155::historical_155_selected_section_indices;

/// At 0x4022B0 the instruction is LOOP rel8, not a zero-checked
/// for-loop. Entry into that loop with ECX=0 executes 2^32 bodies,
/// because the first decrement wraps to 0xFFFF_FFFF. This function
/// models only the loop count, not the intervening memory operations.
pub fn historical_155_loop_iterations(initial_ecx: u32) -> u64 {
    if initial_ecx == 0 {
        1u64 << 32
    } else {
        u64::from(initial_ecx)
    }
}

/// A SAFE synthetic preview, never a Windows process-pointer writer.
/// The historical builder at 0x4022A8 stores each selected section
/// header's 32-bit address and advances EDI by four. The AUDITED LOOP
/// does not write a final NULL; subsequent code is not excluded.
/// The historical iterator at 0x401D06 reads pointers
/// until it encounters zero. A previously zero-filled next slot is
/// therefore a precondition for the isolated loop-only model, not
/// necessarily for the complete historical program. Later writes,
/// runtime initialization and intervening changes remain unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPointerListPreview {
    pub selected_indices: Vec<usize>,
    pub selected_header_vas: Vec<u32>,
    pub resulting_pointer_words: Vec<u32>,
    pub terminating_slot: usize,
}

/// Bounded native-port-only model for invented or caller-owned
/// section records and an explicit snapshot of destination slots.
/// It must NOT silently invent a NULL terminator absent from the
/// audited loop. This does not assert the rest of the function lacks one.
/// Successful validation is not proof of real runtime conditions.
pub fn hypothetical_155_pointer_list_preview(
    section_headers: &[u8],
    section_count: usize,
    section_table_va: u32,
    initial_destination_words: &[u32],
) -> Result<HistoricalPointerListPreview, String> {
    if section_count == 0 {
        return Err("historical x86 LOOP with section count zero wraps for 2^32 iterations".into());
    }
    let selected_indices =
        historical_155_selected_section_indices(section_headers, section_count)?;
    if selected_indices.len() >= initial_destination_words.len() {
        return Err("historical pointer list lacks a complete terminator DWORD slot".into());
    }
    let terminating_slot = selected_indices.len();
    if initial_destination_words[terminating_slot] != 0 {
        return Err("audited historical list loop does not overwrite a stale nonzero terminator".into());
    }
    let mut result = initial_destination_words.to_vec();
    let mut selected_header_vas = Vec::with_capacity(selected_indices.len());
    for (slot, &index) in selected_indices.iter().enumerate() {
        let offset = u32::try_from(index)
            .map_err(|_| "section header index exceeds 32-bit width")?
            .checked_mul(40)
            .ok_or("historical section header address multiplication overflow")?;
        let header_va = section_table_va.checked_add(offset)
            .ok_or("historical 32-bit section header pointer overflow")?;
        if header_va == 0 {
            return Err("historical null section header pointer would end iterator prematurely".into());
        }
        result[slot] = header_va;
        selected_header_vas.push(header_va);
    }
    Ok(HistoricalPointerListPreview {
        selected_indices,
        selected_header_vas,
        resulting_pointer_words: result,
        terminating_slot,
    })
}

/// How many selected entries fit within an unchanged initial run
/// of zero DWORDs while retaining one zero slot for iteration.
/// On-disk snapshot only; it does not establish a runtime capacity.
pub fn hypothetical_zero_terminated_capacity(leading_zero_words: usize) -> usize {
    leading_zero_words.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(names: &[[u8; 8]]) -> Vec<u8> {
        let mut records = vec![0u8; names.len() * 40];
        for (i, name) in names.iter().enumerate() {
            records[i*40..i*40+8].copy_from_slice(name);
        }
        records
    }

    #[test]
    fn zero_count_is_not_a_zero_iteration_loop() {
        assert_eq!(historical_155_loop_iterations(0), 4_294_967_296);
        assert_eq!(historical_155_loop_iterations(1), 1);
        assert_eq!(historical_155_loop_iterations(96), 96);
        assert_eq!(historical_155_loop_iterations(u32::MAX), 4_294_967_295);
        assert!(hypothetical_155_pointer_list_preview(&[], 0, 0x401000, &[0]).is_err());
    }

    #[test]
    fn selected_pointers_keep_header_order_and_existing_zero_sentinel() {
        let records = headers(&[
            *b".text\0\0\0",
            *b".rsrc\0\0\0",
            *b".data\0\0\0",
            *b".idata\0\0",
            *b".rdata\0\0",
        ]);
        let preview =
            hypothetical_155_pointer_list_preview(&records, 5, 0x501000, &[0, 0, 0, 0])
            .unwrap();
        assert_eq!(preview.selected_indices, vec![0, 2]);
        assert_eq!(preview.selected_header_vas, vec![0x501000, 0x501050]);
        assert_eq!(preview.resulting_pointer_words, vec![0x501000, 0x501050, 0, 0]);
        assert_eq!(preview.terminating_slot, 2);
    }

    #[test]
    fn no_selected_sections_still_require_zero_first_slot() {
        let records = headers(&[*b".reloc\0\0", *b".rsrc\0\0\0"]);
        let preview =
            hypothetical_155_pointer_list_preview(&records, 2, 0x501000, &[0, 0])
            .unwrap();
        assert!(preview.selected_indices.is_empty());
        assert_eq!(preview.terminating_slot, 0);
        assert_eq!(preview.resulting_pointer_words, vec![0, 0]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0xDEAD_BEEF, 0]
        ).is_err());
    }

    #[test]
    fn stale_sentinel_or_missing_capacity_fails_closed() {
        let records = headers(&[*b".text\0\0\0", *b".data\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0, 0]
        ).is_err());
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, 0x501000, &[0, 0, 0x1234]
        ).is_err());
        assert!(hypothetical_155_pointer_list_preview(
            &records[..39], 2, 0x501000, &[0, 0, 0]
        ).is_err());
        assert_eq!(hypothetical_zero_terminated_capacity(0), 0);
        assert_eq!(hypothetical_zero_terminated_capacity(1), 0);
        assert_eq!(hypothetical_zero_terminated_capacity(28), 27);
    }

    #[test]
    fn checked_32bit_pointers_refuse_overflow_or_null() {
        let records = headers(&[*b".rdata\0\0", *b".text\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &records, 2, u32::MAX - 39, &[0, 0]
        ).is_err());
        let first = headers(&[*b".text\0\0\0"]);
        assert!(hypothetical_155_pointer_list_preview(
            &first, 1, 0, &[0, 0]
        ).is_err());
        let preview = hypothetical_155_pointer_list_preview(
            &first, 1, u32::MAX, &[0, 0]
        ).unwrap();
        assert_eq!(preview.selected_header_vas, vec![u32::MAX]);
    }
}
