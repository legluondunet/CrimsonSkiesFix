// Native Linux SafeDisc research tool for user-owned Crimson Skies 1.0.2.0 files.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Research stage: strictly read-only. No loader/DLL execution, no executable
// reconstruction, no SafeDisc driver, and no Wine requirement.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

type Result<T> = std::result::Result<T, String>;

const LOADER_SHA: &str = "675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736";
const ICD_SHA: &str = "0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b";
const DLL_SHA: &str = "dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff";
const TESTME_SHA: &str = "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75";
const LOADER_LEN: usize = 344_851;
const ICD_LEN: usize = 2_580_578;
const DLL_LEN: usize = 138_752;
const TESTME_LEN: usize = 2_580_480;
const MAX_INPUT_LEN: u64 = 16 * 1024 * 1024;
const CRPE_MARKER: &[u8] = b"CRPE\0\0";
const SPECIAL_KEY_SKIP: usize = 0x148;
const REPORTED_FIRST_WORD: u32 = 0x7496_538F;

fn read_u16(data: &[u8], at: usize) -> Result<u16> {
    let slice = data.get(at..at.checked_add(2).ok_or("u16 offset overflow")?)
        .ok_or_else(|| format!("truncated u16 at 0x{at:X}"))?;
    Ok(u16::from_le_bytes(slice.try_into().unwrap()))
}
fn read_u32(data: &[u8], at: usize) -> Result<u32> {
    let slice = data.get(at..at.checked_add(4).ok_or("u32 offset overflow")?)
        .ok_or_else(|| format!("truncated u32 at 0x{at:X}"))?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

// Embedded SHA-256 avoids native OpenSSL bindings and external Rust crates.
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut hash = [
        0x6a09e667u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,
        0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19,
    ];
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for i in 0..16 {
            words[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let a = words[i - 15];
            let b = words[i - 2];
            words[i] = words[i - 16]
                .wrapping_add(a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3))
                .wrapping_add(words[i - 7])
                .wrapping_add(b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10));
        }
        let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut h] = hash;
        for i in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(sum1).wrapping_add(choose).wrapping_add(K[i]).wrapping_add(words[i]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = sum0.wrapping_add(majority);
            h=g;g=f;f=e;e=d.wrapping_add(t1);d=c;c=b;b=a;a=t1.wrapping_add(t2);
        }
        for (previous, delta) in hash.iter_mut().zip([a,b,c,d,e,f,g,h]) {
            *previous = previous.wrapping_add(delta);
        }
    }
    hash.iter().map(|h| format!("{h:08x}")).collect()
}

#[derive(Debug, Clone)]
struct PeSection {
    name: [u8; 8],
    virtual_address: u32,
    virtual_size: u32,
    raw_offset: usize,
    raw_len: usize,
    characteristics: u32,
}
#[derive(Debug)]
struct Pe32 {
    entry_rva: u32,
    sections: Vec<PeSection>,
}
impl Pe32 {
    fn section(&self, name: &[u8]) -> Result<&PeSection> {
        self.sections.iter()
            .find(|s| s.name.split(|b| *b == 0).next() == Some(name))
            .ok_or_else(|| format!("PE section '{}' not found", String::from_utf8_lossy(name)))
    }
}

// Minimal bounded file parser. An MZ header alone does not establish a valid PE.
fn parse_pe32(data: &[u8]) -> Result<Pe32> {
    if data.len() < 0x40 || data.get(..2) != Some(b"MZ") {
        return Err("not an MZ executable".into());
    }
    let nt = read_u32(data, 0x3C)? as usize;
    if data.get(nt..nt.checked_add(4).ok_or("PE header offset overflow")?) != Some(b"PE\0\0") {
        return Err("invalid PE header".into());
    }
    if read_u16(data, nt + 4)? != 0x14C {
        return Err("only x86 PE32 supported".into());
    }
    let sections = read_u16(data, nt + 6)? as usize;
    if sections == 0 || sections > 96 {
        return Err("unreasonable PE section count".into());
    }
    let optional_len = read_u16(data, nt + 20)? as usize;
    let optional_at = nt.checked_add(24).ok_or("PE optional header overflow")?;
    if optional_len < 0x60 || data.get(optional_at..optional_at + optional_len).is_none() {
        return Err("invalid/truncated PE optional header".into());
    }
    if read_u16(data, optional_at)? != 0x10B {
        return Err("expected PE32 optional header".into());
    }
    let entry = read_u32(data, optional_at + 16)?;
    let section_at = optional_at.checked_add(optional_len).ok_or("PE sections overflow")?;
    let mut result = Vec::new();
    for i in 0..sections {
        let at = section_at.checked_add(i * 40).ok_or("PE section table overflow")?;
        let sec = data.get(at..at + 40).ok_or("truncated PE section")?;
        let name: [u8; 8] = sec[..8].try_into().unwrap();
        let virtual_size = read_u32(data, at + 8)?;
        let virtual_address = read_u32(data, at + 12)?;
        let raw_len = read_u32(data, at + 16)? as usize;
        let raw_offset = read_u32(data, at + 20)? as usize;
        let characteristics = read_u32(data, at + 36)?;
        let end = raw_offset.checked_add(raw_len).ok_or("PE section length overflow")?;
        if end > data.len() { return Err("PE section extends past EOF".into()); }
        result.push(PeSection { name, virtual_address, virtual_size, raw_offset, raw_len, characteristics });
    }
    Ok(Pe32 { entry_rva: entry, sections: result })
}

// Executable base relocation directory (IMAGE_DIRECTORY_ENTRY_BASERELOC = 5).
// Only IMAGE_REL_BASED_ABSOLUTE (0, padding) and HIGHLOW (3, x86 32-bit)
// occur in our exact verified Crimson Skies binary. Reject other types.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RelocBlock {
    page_rva: u32,
    highlow_offsets: Vec<u16>,
}

fn parse_relocation_blocks(data: &[u8]) -> Result<(Vec<RelocBlock>, usize)> {
    let mut cursor = 0usize;
    let mut blocks = Vec::new();
    let mut padding = 0usize;
    let mut previous_page: Option<u32> = None;
    // Check overlap across adjacent PE pages as well as within a block.
    let mut previous_highlow_end: Option<u64> = None;
    while cursor < data.len() {
        let page = read_u32(data, cursor)?;
        let block_len = read_u32(data, cursor + 4)? as usize;
        if page % 0x1000 != 0 || block_len < 8 || block_len % 2 != 0 {
            return Err(format!("invalid relocation block at +0x{cursor:X}"));
        }
        let next = cursor.checked_add(block_len).ok_or("relocation size overflow")?;
        if next > data.len() {
            return Err("relocation block extends past directory".into());
        }
        if previous_page.is_some_and(|prev| page <= prev) {
            return Err("relocation pages are not strictly increasing".into());
        }
        let mut highlow_offsets = Vec::new();
        for at in (cursor + 8..next).step_by(2) {
            let value = read_u16(data, at)?;
            let kind = value >> 12;
            let off = value & 0xFFF;
            match kind {
                0 => padding += 1,
                3 => {
                    // An x86 HIGHLOW relocation is permitted to start at
                    // page offsets 0xFFD..0xFFF and continue into the next
                    // page. The real Crimson Skies .text has 36 such cases.
                    // They are valid in the base relocation directory.
                    let absolute = page as u64 + off as u64;
                    if previous_highlow_end.is_some_and(|end| absolute < end) {
                        return Err("overlapping or unsorted HIGHLOW relocation, including page boundary".into());
                    }
                    previous_highlow_end = Some(absolute + 4);
                    highlow_offsets.push(off);
                }
                _ => return Err(format!("unsupported x86 relocation type {kind}")),
            }
        }
        blocks.push(RelocBlock { page_rva: page, highlow_offsets });
        previous_page = Some(page);
        cursor = next;
    }
    Ok((blocks, padding))
}

fn pe_relocation_blocks(image: &[u8], pe: &Pe32) -> Result<(Vec<RelocBlock>, usize)> {
    let nt = read_u32(image, 0x3C)? as usize;
    let optional = nt.checked_add(24).ok_or("PE optional header overflow")?;
    let optional_len = read_u16(image, nt + 20)? as usize;
    // PE32 optional header data directory index 5, after 96-byte fixed portion.
    let directory_offset = 96 + 5 * 8;
    if optional_len < directory_offset + 8 {
        return Err("PE optional header too short for relocation directory".into());
    }
    let directory_rva = read_u32(image, optional + directory_offset)?;
    let directory_len = read_u32(image, optional + directory_offset + 4)? as usize;
    if directory_rva == 0 || directory_len == 0 || directory_len > 1_048_576 {
        return Err("relocation directory missing or too large".into());
    }
    let section = pe.section(b".reloc")?;
    let relative = directory_rva.checked_sub(section.virtual_address)
        .ok_or("relocation RVA precedes .reloc section")? as usize;
    if relative.checked_add(directory_len).is_none_or(|end| end > section.raw_len) {
        return Err("relocation directory extends past .reloc raw section".into());
    }
    let start = section.raw_offset.checked_add(relative)
        .ok_or("relocation file offset overflow")?;
    let bytes = image.get(start..start + directory_len)
        .ok_or("relocation directory extends past PE file")?;
    parse_relocation_blocks(bytes)
}

// Historical unSafeDisc 1.5.5 static branch evidence:
// 0x401FCF calls 0x402257 before the checksum phase.
// 0x402505 stores IMAGE_DIRECTORY_ENTRY_BASERELOC.VirtualAddress
// (NT headers +0xA0) at [0x403B2B].
// 0x40230B..0x402366 walks PE section headers looking for this RVA
// inside a file-backed raw section. It also tests the DWORD at +0x34
// of the chosen header against the directory RVA (the following
// section header's RVA where another header exists).
// On matching it executes 0x40236C: mov dword [0x403CAC], 4.
// 0x401D48 selects the relocation-aware checksum branch for 4.
// This is *static path inference*, NOT a trace of executing SafeDisc.
#[derive(Debug, PartialEq, Eq)]
struct HistoricalRelocBranchEvidence {
    directory_rva: u32,
    directory_len: u32,
    section_name: String,
    section_rva: u32,
    next_header_guard: u32,
}

fn infer_historical_reloc_branch(
    image: &[u8], pe: &Pe32,
) -> Result<Option<HistoricalRelocBranchEvidence>> {
    let nt = read_u32(image, 0x3C)? as usize;
    let optional_at = nt.checked_add(24).ok_or("PE optional header overflow")?;
    let opt_size = read_u16(image, nt + 20)? as usize;
    const BASERELOC_OFFSET: usize = 96 + 5 * 8;
    if opt_size < BASERELOC_OFFSET + 8 {
        return Err("optional header lacks relocation data directory".into());
    }
    let reloc_rva = read_u32(image, optional_at + BASERELOC_OFFSET)?;
    let reloc_len = read_u32(image, optional_at + BASERELOC_OFFSET + 4)?;
    if reloc_rva == 0 || reloc_len == 0 {
        // Other historical fallback scans by section name; no inference
        // from this RVA-based branch can be made.
        return Ok(None);
    }
    let section_at = optional_at.checked_add(opt_size)
        .ok_or("PE section headers overflow")?;
    for (idx, section) in pe.sections.iter().enumerate() {
        if section.raw_offset == 0 || section.raw_len == 0 {
            continue;
        }
        let begin = section.virtual_address as u64;
        let end = begin + section.raw_len as u64;
        let dir = reloc_rva as u64;
        // Original 0x40232E uses <= for the high boundary. Here use
        // the stricter backed byte span (< end), as the following raw
        // data extraction must be valid too.
        if !(begin <= dir && dir < end) {
            continue;
        }
        let guard_offset = section_at
            .checked_add(idx.checked_mul(40).ok_or("section index overflow")?)
            .and_then(|n| n.checked_add(0x34))
            .ok_or("section guard offset overflow")?;
        let next_header_guard = read_u32(image, guard_offset)?;
        if next_header_guard == reloc_rva {
            // Original jumps to the next section at 0x402348.
            continue;
        }
        let rel = (reloc_rva - section.virtual_address) as u64;
        if rel + reloc_len as u64 > section.raw_len as u64 {
            // Metadata points outside this actual file-backed section.
            continue;
        }
        let section_name = String::from_utf8_lossy(
            section.name.split(|&b| b == 0).next().unwrap_or(&[])
        ).to_string();
        return Ok(Some(HistoricalRelocBranchEvidence {
            directory_rva: reloc_rva, directory_len: reloc_len,
            section_name, section_rva: section.virtual_address, next_header_guard,
        }));
    }
    Ok(None)
}

// Split one 4-KiB page into adjacent byte spans, omitting each 32-bit
// relocation. The original checksum mixer resets its weight per span, while
// its accumulation persists. Do NOT mistake this isolated primitive for
// the full SafeDisc checksum (which also depends on section selection and
// opaque dplayerx.dll transformations).
#[allow(dead_code)]
fn checksum_page_without_highlow(page: &[u8], offsets: &[u16], previous: u32) -> Result<u32> {
    if page.len() != 4096 { return Err("expected exactly one PE 4-KiB page".into()); }
    let mut sum = previous;
    let mut start = 0usize;
    for &off in offsets {
        let off = off as usize;
        if off < start || off + 4 > page.len() {
            return Err("invalid relocation offset for checksum page".into());
        }
        sum = checksum_chunk(&page[start..off], sum);
        start = off + 4;
    }
    Ok(checksum_chunk(&page[start..], sum))
}

// Validate absolute, ordered, non-overlapping HIGHLOW byte ranges without
// reading code or assuming any particular checksum algorithm.
fn relocation_windows(
    first_page_rva: u32, byte_len: usize, blocks: &[RelocBlock],
) -> Result<Vec<(usize, usize)>> {
    const PAGE: usize = 4096;
    if first_page_rva % PAGE as u32 != 0 || byte_len == 0 || byte_len % PAGE != 0 {
        return Err("expected complete, 4-KiB-aligned PE pages".into());
    }
    let span = u32::try_from(byte_len).map_err(|_| "page span does not fit PE RVA")?;
    let end_rva = first_page_rva.checked_add(span).ok_or("PE page RVA overflow")?;
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut previous_end = 0usize;
    let mut previous_block_page: Option<u32> = None;
    for block in blocks {
        if block.page_rva < first_page_rva || block.page_rva >= end_rva {
            return Err("relocation block is outside the selected page span".into());
        }
        if previous_block_page.is_some_and(|page| block.page_rva <= page) {
            return Err("relocation blocks are not in increasing page order".into());
        }
        previous_block_page = Some(block.page_rva);
        let page_offset = (block.page_rva - first_page_rva) as usize;
        for &offset in &block.highlow_offsets {
            let start = page_offset + offset as usize;
            let end = start + 4;
            if end > byte_len || (!ranges.is_empty() && start < previous_end) {
                return Err("overlapping or out-of-range relocation in page span".into());
            }
            ranges.push((start, end));
            previous_end = end;
        }
    }

    Ok(ranges)
}

// Page-oriented checksum primitive for contiguous, complete 4-KiB PE pages.
// Each page is split at HIGHLOW relocation DWORDs, including DWORDs that
// straddle the boundary into the next page. The byte mixer's weight resets
// per nonempty span; its accumulated sum carries between spans and pages.
//
// This is a SYNTHETICALLY TESTED building block, not the complete SafeDisc
// checksum: section selection, original page/chunk call order and the DLL
// modifier are still unverified. No game file is modified or decrypted.
#[allow(dead_code)]
fn checksum_pages_without_highlow(
    bytes: &[u8], first_page_rva: u32, blocks: &[RelocBlock], previous: u32,
) -> Result<u32> {
    checksum_pages_bounded_without_highlow(
        bytes, first_page_rva, blocks, bytes.len(), previous)
}

// Unlike the earlier full-raw fixture, the historical unSafeDisc 1.5.5
// section loop bounds its input to min(SizeOfRawData, VirtualSize) when
// VirtualSize is nonzero (0x401D1A..0x401D34). The last page can therefore
// be shorter than 4 KiB. Validate the COMPLETE file-backed relocation
// range first, then clip only the spans mixed into the checksum.
//
// The loop's section-type gate (0x401F01) only calls the checksum mixer
// when Characteristics & IMAGE_SCN_CNT_CODE (0x20) is set. This function
// handles a SINGLE eligible code section; the caller enforces the flag.
// It remains a candidate until full relocation traversal is verified.
fn checksum_pages_bounded_without_highlow(
    bytes: &[u8], first_page_rva: u32, blocks: &[RelocBlock],
    effective_len: usize, previous: u32,
) -> Result<u32> {
    const PAGE: usize = 4096;
    if effective_len == 0 || effective_len > bytes.len() {
        return Err("effective PE section extent is empty or exceeds raw data".into());
    }
    let ranges = relocation_windows(first_page_rva, bytes.len(), blocks)?;

    let mut checksum = previous;
    let mut relocation_index = 0usize;
    for page_index in 0..effective_len.div_ceil(PAGE) {
        let start_page = page_index * PAGE;
        let end_page = (start_page + PAGE).min(effective_len);
        while relocation_index < ranges.len() && ranges[relocation_index].1 <= start_page {
            relocation_index += 1;
        }
        let mut cursor = start_page;
        let mut index = relocation_index;
        while index < ranges.len() && ranges[index].0 < end_page {
            let skip_start = ranges[index].0.max(start_page);
            if skip_start > cursor {
                checksum = checksum_chunk(&bytes[cursor..skip_start], checksum);
            }
            cursor = ranges[index].1.min(end_page);
            index += 1;
        }
        if cursor < end_page {
            checksum = checksum_chunk(&bytes[cursor..end_page], checksum);
        }
    }
    Ok(checksum)
}

// Independently enumerate non-relocated, page-bounded fragment calls in
// the relocation-aware branch seen at historical unSafeDisc 1.5.5
// 0x401D82..0x401EFF. This walker scans absolute relocation intervals
// page-by-page and handles carry-over from the previous page separately,
// instead of using the earlier helper's range-index iteration.
//
// IMPORTANT: the original program's selection between this branch and
// its simple-page branch at 0x401D48 depends on runtime state not yet
// independently established for Crimson Skies. Equality with the
// existing relocation-aware candidate strengthens our internal model;
// it does NOT prove which original branch ran or yield the real key.
#[derive(Debug, PartialEq, Eq)]
struct RelocationWalkReport {
    checksum: u32,
    included_bytes: usize,
    excluded_bytes: usize,
    fragments: usize,
    crossings: usize,
}

fn checksum_independent_relocation_walk(
    bytes: &[u8], first_page_rva: u32, blocks: &[RelocBlock], effective_len: usize,
) -> Result<RelocationWalkReport> {
    const PAGE: usize = 4096;
    if effective_len == 0 || effective_len > bytes.len() {
        return Err("invalid effective extent for relocation walker".into());
    }
    let relocation_ranges = relocation_windows(first_page_rva, bytes.len(), blocks)?;
    let mut checksum = 0u32;
    let mut included_bytes = 0usize;
    let mut fragments = 0usize;
    let mut next_relocation = 0usize;
    let mut crossing_end = 0usize;

    // This branch's accumulator spans pages, while the weight is
    // initialized independently at the beginning of EACH call.
    for page_start in (0..effective_len).step_by(PAGE) {
        let page_limit = (page_start + PAGE).min(effective_len);
        while next_relocation < relocation_ranges.len()
            && relocation_ranges[next_relocation].1 <= page_start {
            next_relocation += 1;
        }

        // A HIGHLOW starting at offset 0xFFD..0xFFF was already
        // consumed in the previous page. Preserve its end offset
        // explicitly instead of trying to find it again in the list.
        let mut cursor = page_start.max(crossing_end.min(page_limit));

        while next_relocation < relocation_ranges.len()
            && relocation_ranges[next_relocation].0 < page_limit {
            let (begin, end) = relocation_ranges[next_relocation];
            if cursor < begin {
                checksum = checksum_chunk(&bytes[cursor..begin], checksum);
                included_bytes += begin - cursor;
                fragments += 1;
            }
            cursor = cursor.max(end.min(page_limit));
            if end > page_limit {
                crossing_end = end;
            }
            next_relocation += 1;
        }
        if cursor < page_limit {
            checksum = checksum_chunk(&bytes[cursor..page_limit], checksum);
            included_bytes += page_limit - cursor;
            fragments += 1;
        }
    }

    let excluded_bytes = relocation_ranges.iter()
        .map(|&(start, end)| effective_len.min(end).saturating_sub(start))
        .sum::<usize>();
    if included_bytes.checked_add(excluded_bytes) != Some(effective_len) {
        return Err("relocation walker does not partition the effective section".into());
    }
    let crossings = relocation_ranges.iter()
        .filter(|&&(start, end)| start < effective_len && start / PAGE != (end-1) / PAGE)
        .count();
    Ok(RelocationWalkReport {
        checksum, included_bytes, excluded_bytes, fragments, crossings,
    })
}

// Exact simple-page checksum branch found in unSafeDisc 1.5.5
// 0x401D4E..0x401D77: it skips NO relocated DWORDs. Its checksum
// accumulator is carried between complete 4-KiB calls and the
// nonempty final partial-page call. The branch is selected when
// dword[0x403CAC] != 4 at 0x401D48. The runtime value of this
// global has not been established on Crimson Skies 1.0.2.0.
fn checksum_historic_simple_pages(
    bytes: &[u8], effective_len: usize, previous: u32,
) -> Result<u32> {
    const PAGE: usize = 4096;
    if effective_len == 0 || effective_len > bytes.len() {
        return Err("invalid effective extent for simple-page checksum".into());
    }
    let mut checksum = previous;
    for chunk in bytes[..effective_len].chunks(PAGE) {
        checksum = checksum_chunk(chunk, checksum);
    }
    Ok(checksum)
}

// Alternative *research* hypothesis: carry the checksum across the whole
// section, splitting ONLY around relocation DWORDs (not page boundaries).
// It is deliberately distinct from the page-local checksum hypothesis.
// Neither hypothesis includes the actual SafeDisc DLL modifier stage.
#[allow(dead_code)]
fn checksum_section_without_highlow(
    bytes: &[u8], first_page_rva: u32, blocks: &[RelocBlock], previous: u32,
) -> Result<u32> {
    let ranges = relocation_windows(first_page_rva, bytes.len(), blocks)?;
    let mut checksum = previous;
    let mut cursor = 0usize;
    for (start, end) in ranges {
        if cursor < start {
            checksum = checksum_chunk(&bytes[cursor..start], checksum);
        }
        cursor = end;
    }
    if cursor < bytes.len() {
        checksum = checksum_chunk(&bytes[cursor..], checksum);
    }
    Ok(checksum)
}

// Attribute every byte-mixer contribution to its source 4-KiB page
// under two alternative fragment boundaries. This is a diagnostic of
// proposed traversal models, NOT a recovered SafeDisc algorithm.
//
// Section-wide contributions are computed byte by byte with the original
// fragment length in the weight recurrence, even when a fragment spans
// pages. Page-local contributions restart their weight at the 4-KiB
// boundary. Both skip the exact same relocation-byte windows.
#[allow(dead_code)]
fn checksum_research_page_contributions(
    bytes: &[u8], first_page_rva: u32, blocks: &[RelocBlock],
) -> Result<(Vec<u32>, Vec<u32>)> {
    const PAGE: usize = 4096;
    let ranges = relocation_windows(first_page_rva, bytes.len(), blocks)?;
    let page_count = bytes.len() / PAGE;
    let mut page_local = vec![0u32; page_count];
    let mut section_wide = vec![0u32; page_count];

    let mut attribute_span = |start: usize, end: usize| {
        if start == end { return; }
        let length = end - start;

        // Section-wide hypothesis: initial weight and *whole-span*
        // length apply until the next relocation, even across pages.
        let mut weight = 0xFD37_9AB1u32;
        for (index, &byte) in bytes[start..end].iter().enumerate() {
            let page_index = (start + index) / PAGE;
            section_wide[page_index] = section_wide[page_index]
                .wrapping_add((byte as u32).wrapping_mul(weight));
            weight = weight
                .wrapping_mul(0xA775_3394)
                .wrapping_add((length - index - 1) as u32)
                .wrapping_add(0x3BC6_2BB2);
        }

        // Page-local hypothesis: each retained piece is a new chunk.
        let mut cursor = start;
        while cursor < end {
            let next_page = (cursor / PAGE + 1) * PAGE;
            let stop = end.min(next_page);
            let page_index = cursor / PAGE;
            page_local[page_index] = checksum_chunk(
                &bytes[cursor..stop], page_local[page_index]);
            cursor = stop;
        }
    };

    let mut cursor = 0usize;
    for (start, end) in ranges {
        attribute_span(cursor, start);
        cursor = end;
    }
    attribute_span(cursor, bytes.len());
    Ok((page_local, section_wide))
}

// Opt-in two-hypothesis comparison on the exact hash-verified protected ICD.
// These numbers are NOT recovered SafeDisc checksums or TEA key words.
fn print_checksum_research(
    image: &[u8], pe: &Pe32, section: &PeSection, blocks: &[RelocBlock],
) -> Result<()> {
    let stop = section.raw_offset.checked_add(section.raw_len)
        .ok_or(".text raw range overflow")?;
    let data = image.get(section.raw_offset..stop)
        .ok_or(".text bytes outside verified PE file")?;
    let page_local = checksum_pages_without_highlow(data, section.virtual_address, blocks, 0)?;
    let section_wide = checksum_section_without_highlow(data, section.virtual_address, blocks, 0)?;
    const CODE_SECTION: u32 = 0x20;
    if section.characteristics & CODE_SECTION == 0 {
        return Err("checksum research section must have IMAGE_SCN_CNT_CODE".into());
    }
    // Verified in historical unSafeDisc 1.5.5: a zero VirtualSize means
    // the raw section length is used; otherwise the shorter size wins.
    let effective_len = if section.virtual_size == 0 {
        section.raw_len
    } else {
        section.raw_len.min(section.virtual_size as usize)
    };
    let bounded_simple_page = checksum_historic_simple_pages(data, effective_len, 0)?;
    let bounded_page_local = checksum_pages_bounded_without_highlow(
        data, section.virtual_address, blocks, effective_len, 0)?;
    let independent_walk = checksum_independent_relocation_walk(
        data, section.virtual_address, blocks, effective_len)?;
    if independent_walk.checksum != bounded_page_local {
        return Err("two independent relocation-aware walkers disagree".into());
    }
    println!("Historical unSafeDisc 1.5.5 code-section gate and extent:");
    println!("  checksum section: .text (Characteristics & 0x20; IMAGE_SCN_CNT_CODE)");
    println!("  initial accumulator: 0 (0x401CDE), selected section size: min(raw, nonzero VirtualSize)");
    println!("  raw bytes: {}, selected bytes: {}, ignored raw tail: {}",
        section.raw_len, effective_len, section.raw_len - effective_len);
    println!("  branch at 0x401D48: if runtime [0x403CAC] == 4 then relocation-aware; otherwise simple pages.");
    if let Some(evidence) = infer_historical_reloc_branch(image, pe)? {
        println!("  static dispatch path: relocation directory RVA 0x{:08X}, size {}",
            evidence.directory_rva, evidence.directory_len);
        println!("  directory located in PE section {} (RVA 0x{:08X}); following-header guard 0x{:08X}",
            evidence.section_name, evidence.section_rva, evidence.next_header_guard);
        println!("  static expected assignment at 0x40236C: [0x403CAC] = 4 (relocation-aware).");
    } else {
        println!("  static dispatch path: INCONCLUSIVE (directory/section guards not established).");
    }
    println!("  actual historical runtime branch selector for Crimson Skies: NOT DYNAMICALLY OBSERVED");
    println!("  simple-page, no relocation exclusions: 0x{bounded_simple_page:08X}");
    println!("  relocation-aware extent-bounded:       0x{bounded_page_local:08X}");
    println!("  independent relocation-walk comparison: MATCH");
    println!("  effective non-relocated bytes: {}, skipped relocation bytes: {}",
        independent_walk.included_bytes, independent_walk.excluded_bytes);
    println!("  fragment calls: {}, page-crossing HIGHLOW DWORDs: {}",
        independent_walk.fragments, independent_walk.crossings);
    println!("  These TWO branch-specific candidate values are not verified SafeDisc checksums.");
    println!("  No real TEA key has been derived from either candidate.");
    let (local_pages, whole_pages) =
        checksum_research_page_contributions(data, section.virtual_address, blocks)?;
    let local_from_pages = local_pages.iter().fold(0u32, |sum, v| sum.wrapping_add(*v));
    let whole_from_pages = whole_pages.iter().fold(0u32, |sum, v| sum.wrapping_add(*v));
    if local_from_pages != page_local || whole_from_pages != section_wide {
        return Err("per-page attribution disagrees with existing checksum hypotheses".into());
    }
    println!("Experimental .text byte-mixer hypotheses (NOT the SafeDisc key/checksum):");
    println!("  page-local weight reset:    0x{page_local:08X}");
    println!("  section-wide weight reset:  0x{section_wide:08X}");
    println!("  initial accumulator: 0; both legacy full-raw models exclude the same HIGHLOW DWORD bytes.");
    println!("  Note: these two legacy full-raw values are NOT the actual historical section extent.");
    let differing_pages = local_pages.iter().zip(&whole_pages)
        .filter(|(a, b)| a != b).count();
    println!("  different page contributions: {differing_pages} / {}", local_pages.len());
    println!("  first differing pages (RVA | page-local | section-wide | cumulative delta):");
    let mut cumulative_local = 0u32;
    let mut cumulative_whole = 0u32;
    let mut printed = 0usize;
    for (i, (&local, &whole)) in local_pages.iter().zip(&whole_pages).enumerate() {
        cumulative_local = cumulative_local.wrapping_add(local);
        cumulative_whole = cumulative_whole.wrapping_add(whole);
        if local != whole && printed < 8 {
            let page_rva = section.virtual_address
                .checked_add((i * 4096) as u32).ok_or(".text page RVA overflow")?;
            let delta = cumulative_local.wrapping_sub(cumulative_whole);
            println!("    0x{page_rva:08X} | 0x{local:08X} | 0x{whole:08X} | 0x{delta:08X}");
            printed += 1;
        }
    }
    println!("  Per-page contributions are attributed estimates, not original SafeDisc checkpoints.");
    println!("  PE section flags, real unSafeDisc traversal and DLL modifiers remain unverified.");
    Ok(())
}

// Inspect only the geometry of checksum candidates. This does not compute
// a checksum and makes no claims about the original tool's chunk ordering.
fn print_text_relocation_layout(
    section: &PeSection, blocks: &[RelocBlock],
) -> Result<()> {
    const PAGE: usize = 4096;
    let ranges = relocation_windows(section.virtual_address, section.raw_len, blocks)?;
    let mut spans = 0usize;
    let mut retained_bytes = 0usize;
    let mut relocation_index = 0usize;
    for page_start in (0..section.raw_len).step_by(PAGE) {
        let page_end = page_start + PAGE;
        while relocation_index < ranges.len() && ranges[relocation_index].1 <= page_start {
            relocation_index += 1;
        }
        let mut cursor = page_start;
        let mut index = relocation_index;
        while index < ranges.len() && ranges[index].0 < page_end {
            let keep_end = ranges[index].0.max(page_start);
            if cursor < keep_end {
                spans += 1;
                retained_bytes += keep_end - cursor;
            }
            cursor = ranges[index].1.min(page_end);
            index += 1;
        }
        if cursor < page_end {
            spans += 1;
            retained_bytes += page_end - cursor;
        }
    }
    let excluded_bytes = ranges.len().checked_mul(4).ok_or("excluded bytes overflow")?;
    if retained_bytes.checked_add(excluded_bytes) != Some(section.raw_len) {
        return Err("relocation coverage does not partition .text exactly".into());
    }
    let crossed_pages = ranges.iter()
        .filter(|(start, end)| start / PAGE != (end - 1) / PAGE).count();
    println!("Read-only .text relocation geometry (NOT a SafeDisc checksum):");
    println!("  section RVA: 0x{:08X}, raw offset: 0x{:X}", section.virtual_address, section.raw_offset);
    println!("  section bytes: {}, pages: {}", section.raw_len, section.raw_len / PAGE);
    println!("  excluded relocation bytes: {} ({} DWORDs)", excluded_bytes, ranges.len());
    println!("  retained bytes: {}, hypothetical nonempty page-local chunks: {}",
        retained_bytes, spans);
    println!("  relocations crossing a page boundary: {}", crossed_pages);
    println!("  No checksum, TEA key or decrypted executable has been generated.");
    Ok(())
}

// Show PE section-header properties, with base relocation DWORD *starts*
// mapped to each section's backed file bytes. The PE flags are evidence
// for research, NOT a proof of unSafeDisc's actual section-selection logic.
fn section_highlow_start_count(section: &PeSection, relocs: &[RelocBlock]) -> usize {
    let low = section.virtual_address as u64;
    let high = low + section.raw_len as u64;
    relocs.iter().map(|block| {
        block.highlow_offsets.iter().filter(|&&off| {
            let rva = block.page_rva as u64 + off as u64;
            rva >= low && rva < high
        }).count()
    }).sum()
}

fn print_pe_section_headers(pe: &Pe32, relocs: &[RelocBlock]) {
    const CODE: u32 = 0x0000_0020;
    const INITIALIZED: u32 = 0x0000_0040;
    const UNINITIALIZED: u32 = 0x0000_0080;
    const EXECUTE: u32 = 0x2000_0000;
    const READ: u32 = 0x4000_0000;
    const WRITE: u32 = 0x8000_0000;

    println!("Original ICD PE32 section headers (metadata only, NOT unSafeDisc's confirmed selection):");
    println!("  flags: C=code, I=initialized data, B=uninitialized data; X=execute, R=read, W=write");
    for sec in &pe.sections {
        let name = String::from_utf8_lossy(
            sec.name.split(|b| *b == 0).next().unwrap_or(&[]));
        let flags = sec.characteristics;
        let type_flags = [
            if flags & CODE != 0 { 'C' } else { '-' },
            if flags & INITIALIZED != 0 { 'I' } else { '-' },
            if flags & UNINITIALIZED != 0 { 'B' } else { '-' },
        ];
        let memory_flags = [
            if flags & EXECUTE != 0 { 'X' } else { '-' },
            if flags & READ != 0 { 'R' } else { '-' },
            if flags & WRITE != 0 { 'W' } else { '-' },
        ];
        let count = section_highlow_start_count(sec, relocs);
        println!(
            "  {:8} RVA 0x{:08X}, vsize {:>7}, raw +0x{:X} ({:>7} bytes), chars 0x{:08X} [{}|{}], HIGHLOW starts {}",
            name, sec.virtual_address, sec.virtual_size, sec.raw_offset, sec.raw_len,
            flags, type_flags.iter().collect::<String>(), memory_flags.iter().collect::<String>(),
            count,
        );
    }
    println!("  These are file-backed section spans; virtual-only/BSS bytes and loader-mapped memory are not inspected.");
    println!("  No section has been designated as included or excluded by the real SafeDisc checksum.");
}

fn read_original(path: &Path, expected_len: usize, expected_sha: &str) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(format!("refusing symlink/non-regular file: {}", path.display()));
    }
    if meta.len() > MAX_INPUT_LEN || meta.len() != expected_len as u64 {
        return Err(format!("unexpected size for {}: {} bytes", path.display(), meta.len()));
    }
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != expected_len || sha256(&bytes) != expected_sha {
        return Err(format!("unrecognized file or SHA-256 mismatch: {}", path.display()));
    }
    Ok(bytes)
}

fn locate_raw_key(loader: &[u8], dll_len: usize) -> Result<(usize, [u32; 4])> {
    if dll_len != DLL_LEN {
        return Err("unsupported dplayerx.dll size for SafeDisc 1.50.20".into());
    }
    let matches: Vec<_> = loader.windows(CRPE_MARKER.len())
        .enumerate()
        .filter_map(|(at, candidate)| (candidate == CRPE_MARKER).then_some(at))
        .collect();
    if matches.len() != 1 {
        return Err(format!("expected one CRPE marker, found {}", matches.len()));
    }
    // Static analysis of unSafeDisc 1.5.5: skip 0x148 for the 0x21E00-byte DLL.
    let at = matches[0] + CRPE_MARKER.len() + SPECIAL_KEY_SKIP;
    let mut words = [0u32; 4];
    for (i, value) in words.iter_mut().enumerate() {
        *value = read_u32(loader, at + i * 4)?;
    }
    Ok((at, words))
}

fn import_xor_matches(
    ciphertext: &[u8], encrypted_pe: &Pe32,
    plaintext: &[u8], decoded_pe: &Pe32, key: u32,
) -> Result<Vec<(usize, u32, u32)>> {
    let a = encrypted_pe.section(b".idata")?;
    let b = decoded_pe.section(b".idata")?;
    if a.raw_len != b.raw_len {
        return Err("encrypted and decoded .idata sizes differ".into());
    }
    let mut matches = Vec::new();
    for relative in (0..a.raw_len).step_by(4) {
        if relative + 4 > a.raw_len { break; }
        let x = read_u32(ciphertext, a.raw_offset + relative)?;
        let y = read_u32(plaintext, b.raw_offset + relative)?;
        if x != y && (x ^ y) == key {
            matches.push((a.raw_offset + relative, x, y));
        }
    }
    Ok(matches)
}

#[derive(Default, Debug)]
struct Options {
    loader: Option<PathBuf>,
    icd: Option<PathBuf>,
    dplayerx: Option<PathBuf>,
    reference: Option<PathBuf>,
    reported_key: Option<u32>,
    layout: bool,
    checksum_research: bool,
    sections: bool,
    key_research: bool,
}

fn help() {
    println!("unSafeDisc Linux research port (native Rust, initial read-only stage)\n\
Usage:\n\
  unsafedisc-linux inspect --loader CRIMSON.EXE --icd CRIMSON.ICD \\\n\
      --dplayerx dplayerx.dll [--reference TESTME.EXE] [--reported-key 0x7496538F] [--layout] [--checksum-research] [--sections] [--key-research]\n\
  unsafedisc-linux --help\n\
\n\
Scope: exact SHA-256 verified Crimson Skies 1.0.2.0 SafeDisc 1.50.20 files.\n\
Reads only; NEVER executes the protected DLL, edits inputs, or generates an EXE.\n\
Decrypt/unpack is NOT implemented yet.");
}

fn parse_hex_u32(input: &str) -> Result<u32> {
    // Explicit hex: the unSafeDisc-reported key is normally an eight-digit word.
    let raw = input.strip_prefix("0x").or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);
    if raw.is_empty() || raw.len() > 8 || !raw.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("--reported-key requires 1-8 hexadecimal digits".into());
    }
    u32::from_str_radix(raw, 16).map_err(|e| e.to_string())
}

fn parse_args() -> Result<Option<Options>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("-h" | "--help") => { help(); return Ok(None); }
        Some("inspect") => {}
        _ => { help(); return Err("expected 'inspect' (unpack is not implemented)".into()); }
    }
    let mut options = Options::default();
    while let Some(key) = args.next() {
        if key == "--help" || key == "-h" { help(); return Ok(None); }
        if key == "--layout" {
            if options.layout { return Err("repeated --layout argument".into()); }
            options.layout = true;
            continue;
        }
        if key == "--sections" {
            if options.sections { return Err("repeated --sections argument".into()); }
            options.sections = true;
            continue;
        }
        if key == "--checksum-research" {
            if options.checksum_research { return Err("repeated --checksum-research argument".into()); }
            options.checksum_research = true;
            continue;
        }
        if key == "--key-research" {
            if options.key_research { return Err("repeated --key-research argument".into()); }
            options.key_research = true;
            continue;
        }
        let value = args.next().ok_or_else(|| format!("missing value for {key}"))?;
        match key.as_str() {
            "--loader" if options.loader.is_none() => options.loader = Some(PathBuf::from(value)),
            "--icd" if options.icd.is_none() => options.icd = Some(PathBuf::from(value)),
            "--dplayerx" if options.dplayerx.is_none() => options.dplayerx = Some(PathBuf::from(value)),
            "--reference" if options.reference.is_none() => options.reference = Some(PathBuf::from(value)),
            "--reported-key" if options.reported_key.is_none() => options.reported_key = Some(parse_hex_u32(&value)?),
            _ => return Err(format!("unrecognized or repeated argument: {key}")),
        }
    }
    if options.loader.is_none() || options.icd.is_none() || options.dplayerx.is_none() {
        return Err("--loader, --icd, and --dplayerx are all required".into());
    }
    if options.key_research && options.reported_key.is_none() {
        return Err("--key-research requires an explicit --reported-key first DWORD".into());
    }
    Ok(Some(options))
}

fn inspect(options: Options) -> Result<()> {
    let loader = read_original(options.loader.as_ref().unwrap(), LOADER_LEN, LOADER_SHA)?;
    let icd = read_original(options.icd.as_ref().unwrap(), ICD_LEN, ICD_SHA)?;
    let dll = read_original(options.dplayerx.as_ref().unwrap(), DLL_LEN, DLL_SHA)?;
    let source_pe = parse_pe32(&loader)?;
    let protected_pe = parse_pe32(&icd)?;
    let (relocs, reloc_padding) = pe_relocation_blocks(&icd, &protected_pe)?;
    let text = protected_pe.section(b".text")?;
    let text_page_start = text.virtual_address;
    let text_page_count = text.raw_len / 4096;
    if text.raw_len % 4096 != 0 { return Err("unexpected unaligned .text raw size".into()); }
    let text_page_end = text_page_start.checked_add(text.raw_len as u32)
        .ok_or(".text RVA range overflow")?;
    let text_blocks = relocs.iter()
        .filter(|block| block.page_rva >= text_page_start && block.page_rva < text_page_end)
        .count();
    let text_highlow: usize = relocs.iter()
        .filter(|block| block.page_rva >= text_page_start && block.page_rva < text_page_end)
        .map(|block| block.highlow_offsets.len()).sum();
    let total_highlow: usize = relocs.iter().map(|block| block.highlow_offsets.len()).sum();
    let crossed_highlow: usize = relocs.iter()
        .filter(|block| block.page_rva >= text_page_start && block.page_rva < text_page_end)
        .map(|block| block.highlow_offsets.iter().filter(|&&off| off > 0xFFC).count())
        .sum();

    // Expected for the exact hash-verified user-owned Crimson Skies ICD,
    // not a general rule for unrelated games.
    if relocs.len() != 558 || total_highlow != 71_114 || reloc_padding != 280
        || text_page_count != 514 || text_blocks != 514 || text_highlow != 61_743
        || crossed_highlow != 36 {
        return Err("relocation layout differs from the expected Crimson Skies original".into());
    }
    if options.sections {
        print_pe_section_headers(&protected_pe, &relocs);
    }
    if options.layout {
        let text_relocs: Vec<_> = relocs.iter()
            .filter(|block| block.page_rva >= text_page_start && block.page_rva < text_page_end)
            .cloned().collect();
        print_text_relocation_layout(text, &text_relocs)?;
    }
    if options.checksum_research {
        let text_relocs: Vec<_> = relocs.iter()
            .filter(|block| block.page_rva >= text_page_start && block.page_rva < text_page_end)
            .cloned().collect();
        print_checksum_research(&icd, &protected_pe, text, &text_relocs)?;
    }
    let (raw_offset, raw) = locate_raw_key(&loader, dll.len())?;
    // The conditional research key is computed ONLY when requested;
    // all inputs have already passed their exact SHA-256 size checks.
    let tea_candidate = if options.key_research {
        let evidence = infer_historical_reloc_branch(&icd, &protected_pe)?
            .ok_or("historical relocation selector path not established; refuse key candidate")?;
        if evidence.section_name != ".reloc" {
            return Err("historical relocation selector section differs from expected .reloc".into());
        }
        let selected_len = if text.virtual_size == 0 { text.raw_len }
            else { text.raw_len.min(text.virtual_size as usize) };
        let raw_end = text.raw_offset.checked_add(text.raw_len).ok_or(".text raw overflow")?;
        let text_bytes = icd.get(text.raw_offset..raw_end)
            .ok_or("ICD .text bytes out of range")?;
        let selected_relocs: Vec<_> = relocs.iter()
            .filter(|b| b.page_rva >= text_page_start && b.page_rva < text_page_end)
            .cloned().collect();
        let checksum = checksum_pages_bounded_without_highlow(
            text_bytes, text.virtual_address, &selected_relocs, selected_len, 0)?;
        let corroboration = checksum_independent_relocation_walk(
            text_bytes, text.virtual_address, &selected_relocs, selected_len)?;
        if corroboration.checksum != checksum {
            return Err("checksum candidate walkers disagree; refuse key candidate".into());
        }
        Some((checksum, construct_tea_candidate(
            options.reported_key.unwrap(), checksum, protected_pe.entry_rva, raw)))
    } else { None };
    println!("Verified original Crimson Skies 1.0.2.0 files (SHA-256 + size).");
    println!("SafeDisc: 1.50.20; native read-only inspection; no Wine.");
    println!("Loader entry RVA: 0x{:08X}", source_pe.entry_rva);
    println!("ICD entry RVA:    0x{:08X}", protected_pe.entry_rva);
    println!("PE relocations: {} blocks, {} HIGHLOW DWORDs, {} padding entries.",
        relocs.len(), total_highlow, reloc_padding);
    println!(".text relocations: {} pages, {} HIGHLOW DWORDs.",
        text_blocks, text_highlow);
    println!(".text HIGHLOW relocations crossing a 4-KiB page: {} (valid, requires joined-page treatment).",
        crossed_highlow);
    println!("Raw key offset:  0x{raw_offset:X} (CRPE + 6 + 0x148)");
    println!("Selected raw DWORDs:");
    for (i, w) in raw.iter().enumerate() { println!("  raw[{i}] = 0x{w:08X}"); }
    println!("No full 128-bit TEA key is asserted; checksum and DLL modifier stage remain unverified.");
    let mut first_word_reference_matches: Option<usize> = None;
    if let Some(path) = options.reference {
        // A user may have patched the resolution of TESTME.EXE with our
        // separate Python tool. The original backup remains the valid
        // plaintext oracle; do not silently accept modified game code.
        let reference = read_original(&path, TESTME_LEN, TESTME_SHA)
            .map_err(|error| {
                let mut backup_name = path.as_os_str().to_os_string();
                backup_name.push(".crimson-original.bak");
                let backup = PathBuf::from(backup_name);
                format!(
                    "{error}\n                     TESTME.EXE must be the exact unmodified unSafeDisc output.\n                     If you applied the resolution patch, try its original backup:\n                     --reference '{}'\n                     Expected unmodified SHA-256: {TESTME_SHA}\n                     No input files were changed.",
                    backup.display(),
                )
            })?;
        let decoded_pe = parse_pe32(&reference)?;
        // Relocation metadata belongs to unchanged PE structure and should
        // be exactly equivalent in the original ICD and the unwrapped file.
        let decoded_relocs = pe_relocation_blocks(&reference, &decoded_pe)?;
        if decoded_relocs != (relocs, reloc_padding) {
            return Err("unwrapped reference relocation entries differ from original ICD".into());
        }
        println!("Original ICD and TESTME.EXE relocation tables match.");

        if decoded_pe.entry_rva != protected_pe.entry_rva {
            return Err("original ICD and unwrapped reference entrypoints differ".into());
        }
        println!("Verified user-supplied TESTME.EXE reference (exact known hash).");
        if let Some(key) = options.reported_key {
            let matches = import_xor_matches(&icd, &protected_pe, &reference, &decoded_pe, key)?;
            println!("Import DWORD XOR matches for 0x{key:08X}: {}", matches.len());
            first_word_reference_matches = Some(matches.len());
            for (at, cipher, plain) in matches.iter().take(16) {
                println!("  ICD offset 0x{at:X}: 0x{cipher:08X} XOR 0x{key:08X} = 0x{plain:08X}");
            }
            if matches.is_empty() { println!("WARNING: no matching import DWORDs found."); }
        } else {
            println!("Specify --reported-key 0x7496538F to check matching import DWORDs.");
        }
    } else if options.reported_key.is_some() {
        println!("To verify a reported key, also provide the local --reference TESTME.EXE.");
    }
    if let Some((checksum, key)) = tea_candidate {
        println!("Experimental SafeDisc TEA key reconstruction (NOT independently validated):");
        println!("  first word: 0x{:08X} (explicit --reported-key input)", key[0]);
        println!("  reconstructed using original ICD entry RVA 0x{:08X}, raw key, and checksum candidate 0x{checksum:08X}",
            protected_pe.entry_rva);
        if let Some(count) = first_word_reference_matches {
            println!("  reference import-XOR matches for first word: {count}");
            if count == 0 {
                return Err("first TEA word lacks even import-XOR corroboration".into());
            }
        } else {
            println!("  WARNING: first word has NO independent plaintext reference check; use --reference TESTME.EXE if available.");
        }
        for (i, word) in key.iter().enumerate() {
            println!("  provisional TEA word[{i}] = 0x{word:08X}");
        }
        println!("  This formula is supported by original x86 code, but reconstructed words have NOT been tested by game decryption.");
        println!("  SafeDisc 1.50.20 additionally requires two game-specific dplayerx.dll modifiers; no decrypted file is produced.");
    }
    println!("Unpacking is NOT implemented. Original input files have not been modified.");
    Ok(())
}

// Faithful byte-mixing inner loop from unSafeDisc 1.5.5 at 0x401F0D..0x401F36.
// EBP resets to FD379AB1 for *each* chunk; accumulated sum persists between
// calls. The caller still needs PE section filtering, relocation handling,
// and the SafeDisc 1.50.20 DLL-side modifier transformation; this is NOT the
// complete checksum used to reconstruct the TEA key.
#[allow(dead_code)]
fn checksum_chunk(data: &[u8], previous: u32) -> u32 {
    let mut weight = 0xFD37_9AB1u32;
    let mut result = previous;
    for (index, byte) in data.iter().enumerate() {
        result = result.wrapping_add((*byte as u32).wrapping_mul(weight));
        weight = weight
            .wrapping_mul(0xA775_3394)
            .wrapping_add((data.len() - index - 1) as u32)
            .wrapping_add(0x3BC6_2BB2);
    }
    result
}

// Key-material staging observed in historical unSafeDisc 1.5.5
// at 0x401CA2..0x401CDD, AFTER the checksum routine:
//   ebx = dword[0x403C50] + 0x2D8
//   eax = (dword[0x403C30] - ebx) XOR dword[0x403C40]
//   for i = 2,1,0:
//       dword[0x403C34 + i*4] =
//           (dword[0x403C44 + i*4] XOR eax XOR checksum) + ebx
// A 2001 SafeDisc 1.45+ analysis maps the source words to the raw
// loader key (at [0x403C40..]) and the scalar base to the original
// ICD PE entry RVA. The first TEA word and checksum are still merely
// candidate inputs until real title-specific decryption is checked.
// This is pure arithmetic, NOT independently validated key recovery.
#[allow(dead_code)]
fn derive_three_checksum_dependent_words(
    source_words: [u32; 3],
    source_anchor: u32,
    loader_base: u32,
    scalar_word: u32,
    checksum: u32,
) -> [u32; 3] {
    let adjusted_base = loader_base.wrapping_add(0x2D8);
    let common = source_anchor.wrapping_sub(adjusted_base) ^ scalar_word ^ checksum;
    source_words.map(|word| (word ^ common).wrapping_add(adjusted_base))
}

// ArthaXerXes (June 2001) and unSafeDisc 1.5.5 at 0x401CA2..0x401CDD
// both support this four-word SafeDisc 1.45+ construction. The first
// word is provided, NOT found by this function. The checksum and
// original PE entry RVA are independently selected by the caller.
// Correct mathematical mapping of x86 globals:
//   [0x403C30] = TEA word zero (user-provided candidate)
//   [0x403C40..0x403C4C] = raw key DWORDs
//   [0x403C50] = original ICD AddressOfEntryPoint RVA
// Never label the output a VALIDATED encryption key until plaintext
// decrypts successfully through both original title-specific layers.
fn construct_tea_candidate(
    first_word: u32, checksum: u32, entry_rva: u32, raw: [u32;4]
) -> [u32;4] {
    let remaining = derive_three_checksum_dependent_words(
        [raw[1],raw[2],raw[3]], first_word, entry_rva, raw[0], checksum);
    [first_word, remaining[0], remaining[1], remaining[2]]
}

// Exact EAX call ordering observed in unSafeDisc 1.5.5 at 0x401F62:
// for each DWORD, XOR by index-derived mask, call 0x65BE, XOR same mask,
// call 0x6621, write transformed DWORD. Neither real protected DLL routine
// is currently available as a native callback. Keep the adapter private
// and test exclusively with synthetic closures, never a fabricated decoder.
#[allow(dead_code)]
fn pre_tea_dwords_with_modifiers<F, G>(
    data: &mut [u8], mut first_modifier: F, mut second_modifier: G,
) where F: FnMut(u32) -> u32, G: FnMut(u32) -> u32 {
    for (index, bytes) in data.chunks_exact_mut(4).enumerate() {
        let index = index as u32;
        // The real x86 code ORs FOUR shifts of the entire DWORD index.
        // Do not truncate to (index & 0xFF) or assume simple byte repetition:
        // e.g. index 0x100 produces mask 0x01010100.
        let mask = (index << 24) | (index << 16) | (index << 8) | index;
        let input = u32::from_le_bytes(bytes.try_into().unwrap());
        let output = second_modifier(first_modifier(input ^ mask) ^ mask);
        bytes.copy_from_slice(&output.to_le_bytes());
    }
}

// SafeDisc 1.50.20 second-layer page adapter: index mask must restart
// for EACH 4-KiB page. The last page may be short, and incomplete DWORD
// bytes are deliberately untouched. No real DLL callbacks exist yet.
#[allow(dead_code)]
fn pre_tea_section_pages_with_modifiers<F, G>(
    data: &mut [u8], mut first: F, mut second: G,
) where F: FnMut(u32) -> u32, G: FnMut(u32) -> u32 {
    for page in data.chunks_mut(4096) {
        pre_tea_dwords_with_modifiers(page, &mut first, &mut second);
    }
}

// Standard TEA reference routine, tested with synthetic vectors.
// NOT a full SafeDisc 1.50.20 decoder: its input needs the DLL's extra layer undone.
#[cfg(test)]
fn tea_decrypt(mut v0: u32, mut v1: u32, k: [u32; 4]) -> (u32, u32) {
    let mut sum = 0xC6EF_3720u32;
    for _ in 0..32 {
        v1 = v1.wrapping_sub(
            (v0.wrapping_shl(4).wrapping_add(k[2]))
                ^ (v0.wrapping_add(sum))
                ^ ((v0 >> 5).wrapping_add(k[3])),
        );
        v0 = v0.wrapping_sub(
            (v1.wrapping_shl(4).wrapping_add(k[0]))
                ^ (v1.wrapping_add(sum))
                ^ ((v1 >> 5).wrapping_add(k[1])),
        );
        sum = sum.wrapping_sub(0x9E37_79B9);
    }
    (v0, v1)
}

fn main() {
    let outcome = parse_args().and_then(|o| match o {
        Some(args) => inspect(args),
        None => Ok(()),
    });
    if let Err(e) = outcome {
        eprintln!("ERROR: {e}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_256_vectors() {
        assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn reference_tea_decryption_vector_zero_key() {
        // Published textbook 32-round TEA zero-key test case.
        assert_eq!(tea_decrypt(0x41EA_3A0A, 0x94BA_A940, [0; 4]), (0, 0));
    }

    #[test]
    fn unsafedisc_checksum_inner_loop_synthetic_vectors() {
        // Constants and order come from static x86 disassembly, and these
        // fixed expected values were calculated independently in Python.
        assert_eq!(checksum_chunk(b"", 0), 0);
        assert_eq!(checksum_chunk(&[0x01], 0), 0xFD37_9AB1);
        assert_eq!(checksum_chunk(&[1, 2, 3, 4], 0), 0x518B_EBC7);
        assert_eq!(checksum_chunk(b"abc", 0), 0xB91C_7E3A);
        assert_eq!(checksum_chunk(&[0xFF; 8], 0), 0xD544_3EF5);
        // Reset the weight at each separate call while retaining checksum.
        let part = checksum_chunk(&[1, 2], 0);
        assert_eq!(part, 0xB87B_54BF);
        assert_eq!(checksum_chunk(&[3, 4], part), 0x26A9_98EE);
        assert_ne!(checksum_chunk(&[3, 4], part), checksum_chunk(&[1, 2, 3, 4], 0));
        let page: Vec<u8> = (0..4096).map(|i| (i * 7 % 256) as u8).collect();
        assert_eq!(checksum_chunk(&page, 0), 0xA8B4_2184);
    }

    #[test]
    fn pretea_dword_modifier_order_and_full_width_index_mask() {
        let mut synthetic = vec![0u8; 258 * 4 + 2];
        synthetic[258 * 4..].copy_from_slice(&[0xAA, 0xBB]);
        // Identity modifiers cancel the XOR mask, regardless of index.
        let mut unchanged = synthetic.clone();
        pre_tea_dwords_with_modifiers(&mut unchanged, |x| x, |x| x);
        assert_eq!(unchanged, synthetic);
        // Deliberately noncommuting synthetic modifiers expose stage order,
        // XOR placement and the high-index (0x100) mask semantics.
        pre_tea_dwords_with_modifiers(
            &mut synthetic,
            |x| x.wrapping_add(1),
            |x| x ^ 0x1122_3344,
        );
        let read = |index: usize| {
            u32::from_le_bytes(synthetic[index * 4..index * 4 + 4].try_into().unwrap())
        };
        assert_eq!(read(0), 0x1122_3345);
        assert_eq!(read(1), 0x1122_3347);
        assert_eq!(read(255), 0xEEDD_CCBB);
        assert_eq!(read(256), 0x1122_3345);
        assert_eq!(read(257), 0x1122_3347);
        assert_eq!(&synthetic[258 * 4..], &[0xAA, 0xBB]);
    }

    #[test]
    fn extract_correct_special_raw_words_from_synthetic_loader() {
        let mut loader = vec![0u8; 0x400];
        loader[0x40..0x46].copy_from_slice(CRPE_MARKER);
        let words = [0xE51D_C3D7u32, 0x63CA_5EAC, 0x4493_379C, 0x08B4_2B8C];
        let at = 0x40 + 6 + SPECIAL_KEY_SKIP;
        for (i, word) in words.iter().enumerate() {
            loader[at + i * 4..at + (i + 1) * 4].copy_from_slice(&word.to_le_bytes());
        }
        let (observed_offset, actual) = locate_raw_key(&loader, DLL_LEN).unwrap();
        assert_eq!(observed_offset, at);
        assert_eq!(actual, words);
        assert!(locate_raw_key(&loader, 1000).is_err());
    }

    #[test]
    fn reject_duplicate_marker_and_short_loader() {
        let mut loader = vec![0u8; 0x400];
        loader[10..16].copy_from_slice(CRPE_MARKER);
        loader[100..106].copy_from_slice(CRPE_MARKER);
        assert!(locate_raw_key(&loader, DLL_LEN).is_err());
        loader[100..106].fill(0);
        loader.truncate(0x100);
        assert!(locate_raw_key(&loader, DLL_LEN).is_err());
    }

    fn synthetic_pe() -> Vec<u8> {
        let mut b = vec![0u8; 0x600];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&0x14Cu16.to_le_bytes());
        b[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        b[0x94..0x96].copy_from_slice(&0xE0u16.to_le_bytes());
        b[0x98..0x9A].copy_from_slice(&0x10Bu16.to_le_bytes());
        b[0xA8..0xAC].copy_from_slice(&0x1000u32.to_le_bytes());
        let section = 0x178;
        b[section..section + 6].copy_from_slice(b".idata");
        b[section + 16..section + 20].copy_from_slice(&0x100u32.to_le_bytes());
        b[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
        b
    }

    #[test]
    fn pe_parse_and_import_dword_matching() {
        let mut encrypted = synthetic_pe();
        let mut decoded = synthetic_pe();
        assert_eq!(parse_pe32(&encrypted).unwrap().entry_rva, 0x1000);
        let plain = 0x0062_1298u32;
        encrypted[0x220..0x224].copy_from_slice(&(plain ^ REPORTED_FIRST_WORD).to_le_bytes());
        decoded[0x220..0x224].copy_from_slice(&plain.to_le_bytes());
        let ep = parse_pe32(&encrypted).unwrap();
        let dp = parse_pe32(&decoded).unwrap();
        let matches = import_xor_matches(&encrypted, &ep, &decoded, &dp, REPORTED_FIRST_WORD).unwrap();
        assert_eq!(matches, vec![(0x220, plain ^ REPORTED_FIRST_WORD, plain)]);
        assert!(parse_pe32(b"junk").is_err());
    }

    #[test]
    fn parse_and_use_synthetic_highlow_relocations() {
        // Two relocations at 2 and 9, while no relocations overlap.
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x1000u32.to_le_bytes());
        raw.extend_from_slice(&14u32.to_le_bytes());
        for entry in [0x3002u16, 0x3009, 0x0000] {
            raw.extend_from_slice(&entry.to_le_bytes());
        }
        let (blocks, padding) = parse_relocation_blocks(&raw).unwrap();
        assert_eq!(padding, 1);
        assert_eq!(blocks, vec![RelocBlock { page_rva: 0x1000, highlow_offsets: vec![2, 9] }]);
        let mut page = [0u8; 4096];
        for (i, byte) in page[..16].iter_mut().enumerate() { *byte = i as u8; }
        let mut expected = 0;
        expected = checksum_chunk(&page[0..2], expected);
        expected = checksum_chunk(&page[6..9], expected);
        expected = checksum_chunk(&page[13..], expected);
        assert_eq!(checksum_page_without_highlow(&page, &[2, 9], 0).unwrap(), expected);
        assert!(checksum_page_without_highlow(&page, &[9, 2], 0).is_err());
        assert!(checksum_page_without_highlow(&page[..16], &[2], 0).is_err());
    }

    #[test]
    fn reject_invalid_relocation_blocks() {
        assert!(parse_relocation_blocks(&[0u8; 7]).is_err());
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x1000u32.to_le_bytes());
        raw.extend_from_slice(&10u32.to_le_bytes());
        raw.extend_from_slice(&0x3FFFu16.to_le_bytes());
        // A HIGHLOW at 0xFFF can cross a 4-KiB boundary, and is valid
        // in the relocation table even if a single-page checksum helper
        // cannot process it in isolation.
        let (blocks, _) = parse_relocation_blocks(&raw).unwrap();
        assert_eq!(blocks[0].highlow_offsets, vec![0xFFF]);
        assert!(checksum_page_without_highlow(&[0u8; 4096], &[0xFFF], 0).is_err());
        raw[8..10].copy_from_slice(&0x7008u16.to_le_bytes());
        assert!(parse_relocation_blocks(&raw).is_err());
        raw[8..10].copy_from_slice(&0x3008u16.to_le_bytes());
        assert!(parse_relocation_blocks(&raw).is_ok());
        raw.extend_from_slice(&0x1000u32.to_le_bytes());
        raw.extend_from_slice(&8u32.to_le_bytes());
        assert!(parse_relocation_blocks(&raw).is_err());
    }

    #[test]
    fn joined_pages_skip_highlow_dwords_crossing_each_page_edge() {
        let data: Vec<u8> = (0..8192).map(|i| (i % 251) as u8).collect();
        let initial = 0x1234_5678;
        for offset in [0xFFDu16, 0xFFE, 0xFFF] {
            let blocks = [RelocBlock {
                page_rva: 0x1000, highlow_offsets: vec![offset],
            }];
            // Independent two-span reference: all four relocated bytes are
            // omitted, even when one to three land in the second page.
            let expected = checksum_chunk(&data[offset as usize + 4..],
                checksum_chunk(&data[..offset as usize], initial));
            assert_eq!(checksum_pages_without_highlow(&data, 0x1000, &blocks, initial).unwrap(), expected);
            assert!(checksum_pages_without_highlow(&data[..4096], 0x1000, &blocks, initial).is_err());
        }

        let blocks = [
            RelocBlock { page_rva: 0x1000, highlow_offsets: vec![0xFFE] },
            RelocBlock { page_rva: 0x2000, highlow_offsets: vec![0x020] },
        ];
        let mut expected = checksum_chunk(&data[..4094], initial);
        expected = checksum_chunk(&data[4098..4128], expected);
        expected = checksum_chunk(&data[4132..], expected);
        assert_eq!(checksum_pages_without_highlow(&data, 0x1000, &blocks, initial).unwrap(), expected);
        // All page chunks have independent initial weights but a carried sum.
        let plain = [RelocBlock { page_rva: 0x1000, highlow_offsets: vec![2, 9] }];
        assert_eq!(
            checksum_pages_without_highlow(&data[..4096], 0x1000, &plain, initial).unwrap(),
            checksum_page_without_highlow(&data[..4096], &[2, 9], initial).unwrap(),
        );
        assert!(checksum_pages_without_highlow(&data[..4096], 0x1001, &[], initial).is_err());
        assert!(checksum_pages_without_highlow(&data[..4096], 0x1000,
            &[RelocBlock { page_rva: 0x2000, highlow_offsets: vec![] }], initial).is_err());
    }

    #[test]
    fn reject_inter_page_relocation_overlap_but_allow_adjacency() {
        let directory = |next_offset: u16| {
            let mut data = Vec::new();
            for (page, offset) in [(0x1000u32, 0xFFFu16), (0x2000, next_offset)] {
                data.extend_from_slice(&page.to_le_bytes());
                data.extend_from_slice(&10u32.to_le_bytes());
                data.extend_from_slice(&(0x3000u16 | offset).to_le_bytes());
            }
            data
        };
        for overlapping in [0, 1, 2] {
            assert!(parse_relocation_blocks(&directory(overlapping)).is_err());
        }
        let (blocks, padding) = parse_relocation_blocks(&directory(3)).unwrap();
        assert_eq!(padding, 0);
        assert_eq!(blocks.len(), 2);
        let sample = vec![0xAAu8; 8192];
        assert!(checksum_pages_without_highlow(&sample[..4096], 0x1000, &blocks[..1], 0).is_err());
        assert!(checksum_pages_without_highlow(&sample, 0x1000, &blocks, 0).is_ok());
    }

    #[test]
    fn joined_page_relocation_windows_preserve_byte_partition() {
        let mut data: Vec<u8> = (0..12288).map(|i| (i % 251) as u8).collect();
        let blocks = [
            RelocBlock { page_rva: 0x3000, highlow_offsets: vec![3, 0xFFF] },
            RelocBlock { page_rva: 0x4000, highlow_offsets: vec![10] },
            RelocBlock { page_rva: 0x5000, highlow_offsets: vec![0xFFC] },
        ];
        let windows = relocation_windows(0x3000, data.len(), &blocks).unwrap();
        assert_eq!(windows, vec![(3, 7), (4095, 4099), (4106, 4110), (12284, 12288)]);
        let original = checksum_pages_without_highlow(&data, 0x3000, &blocks, 123).unwrap();
        for &(start, end) in &windows {
            for byte in &mut data[start..end] { *byte ^= 0xFF; }
        }
        assert_eq!(checksum_pages_without_highlow(&data, 0x3000, &blocks, 123).unwrap(), original);
        // Confirm that the exclusions account for every byte exactly once.
        let excluded = windows.len() * 4;
        assert_eq!(data.len() - excluded, 12272);
        assert!(relocation_windows(0x3000, 4096, &blocks).is_err());
        assert!(relocation_windows(0x3000, 8192, &blocks).is_err());
        assert!(relocation_windows(0x3000, data.len() - 1, &blocks).is_err());
    }

    #[test]
    fn pretea_modifier_mask_restarts_on_each_page() {
        let mut bytes = vec![0u8; 4096 * 2 + 7];
        bytes[4096 * 2 + 4..].copy_from_slice(&[0x10, 0x20, 0x30]);
        // The identity first modifier returns its input so the second one
        // observes the XOR mask canceled out. Capture the first input to
        // distinguish word 0 from word 1024 and word 1025.
        let mut captured = Vec::new();
        pre_tea_section_pages_with_modifiers(
            &mut bytes,
            |word| { captured.push(word); word },
            |word| word,
        );
        assert_eq!(captured.len(), 2049);
        assert_eq!(captured[0], 0);
        assert_eq!(captured[1], 0x0101_0101);
        assert_eq!(captured[256], 0x0101_0100);
        assert_eq!(captured[1023], 0xFFFF_FFFF);
        assert_eq!(captured[1024], 0); // second page, first DWORD
        assert_eq!(captured[2048], 0); // partial third page
        assert_eq!(&bytes[4096 * 2 + 4..], &[0x10, 0x20, 0x30]);
        assert!(bytes.iter().take(4096 * 2 + 4).all(|b| *b == 0));
    }

    #[test]
    fn checksum_research_distinguishes_page_and_section_models() {
        // Independently calculated Python expected vectors: three 4-KiB
        // pages, one straddling HIGHLOW and another in the next page.
        let mut bytes: Vec<u8> = (0..12288).map(|i| ((i * 7 + 3) % 256) as u8).collect();
        let blocks = [
            RelocBlock { page_rva: 0x1000, highlow_offsets: vec![0xFFE] },
            RelocBlock { page_rva: 0x2000, highlow_offsets: vec![0x021] },
        ];
        let local = checksum_pages_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap();
        let whole = checksum_section_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap();
        assert_eq!(local, 0x6D73_525C);
        assert_eq!(whole, 0x9C2B_674B);
        assert_ne!(local, whole);
        for &offset in &[0xFFEusize, 0xFFF, 0x1000, 0x1001, 0x1021, 0x1024] {
            bytes[offset] ^= 0xA5;
        }
        assert_eq!(checksum_pages_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap(), local);
        assert_eq!(checksum_section_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap(), whole);
        bytes[0x2500] ^= 1;
        assert_ne!(checksum_pages_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap(), local);
        assert_ne!(checksum_section_without_highlow(&bytes, 0x1000, &blocks, 0).unwrap(), whole);
    }

    #[test]
    fn research_per_page_attribution_matches_both_models() {
        let mut bytes: Vec<u8> = (0..12288).map(|i| ((i * 7 + 3) % 256) as u8).collect();
        let blocks = [
            RelocBlock { page_rva: 0x1000, highlow_offsets: vec![0xFFE] },
            RelocBlock { page_rva: 0x2000, highlow_offsets: vec![0x021] },
        ];
        let (a, b) = checksum_research_page_contributions(&bytes, 0x1000, &blocks).unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(b.len(), 3);
        assert_eq!(a.iter().fold(0u32, |s, v| s.wrapping_add(*v)), 0x6D73_525C);
        assert_eq!(b.iter().fold(0u32, |s, v| s.wrapping_add(*v)), 0x9C2B_674B);
        assert!(a.iter().zip(&b).any(|(left, right)| left != right));

        // The same excluded bytes must have zero contribution in both models.
        for &index in &[0xFFEusize, 0xFFF, 0x1000, 0x1001, 0x1021, 0x1024] {
            bytes[index] ^= 0xA5;
        }
        assert_eq!(checksum_research_page_contributions(&bytes, 0x1000, &blocks).unwrap(), (a, b));

        // No-relocation model also attributes the full section by page.
        let plain = vec![1u8; 8192];
        let (a, b) = checksum_research_page_contributions(&plain, 0x1000, &[]).unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 2);
        assert_eq!(a.iter().fold(0u32, |s, v| s.wrapping_add(*v)),
            checksum_pages_without_highlow(&plain, 0x1000, &[], 0).unwrap());
        assert_eq!(b.iter().fold(0u32, |s, v| s.wrapping_add(*v)),
            checksum_section_without_highlow(&plain, 0x1000, &[], 0).unwrap());
    }

    #[test]
    fn pe_section_metadata_flags_and_highlow_start_count() {
        // Header offsets are prescribed by PE/COFF: VirtualSize +8,
        // Characteristics +36, independent of the base relocation directory.
        let mut image = synthetic_pe();
        let section = 0x178usize;
        image[section + 8..section + 12].copy_from_slice(&0xE0u32.to_le_bytes());
        image[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[section + 36..section + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        let metadata = pe.section(b".idata").unwrap();
        assert_eq!(metadata.virtual_size, 0xE0);
        assert_eq!(metadata.virtual_address, 0x1000);
        assert_eq!(metadata.characteristics, 0x6000_0020);
        assert_eq!(metadata.raw_len, 0x100);

        let relocs = vec![
            RelocBlock { page_rva: 0x1000, highlow_offsets: vec![0x0, 0x0FE, 0x100, 0xFFD] },
            RelocBlock { page_rva: 0x2000, highlow_offsets: vec![0x0] },
        ];
        // The relocation at +0xFE starts in raw .idata, although its
        // 4-byte DWORD spans beyond this section's file-backed RVA span.
        // The start at +0x100 is outside the section and must not count.
        assert_eq!(section_highlow_start_count(metadata, &relocs), 2);
    }

    #[test]
    fn historical_bounded_checksum_clips_raw_section_tail() {
        let bytes: Vec<u8> = (0..8192).map(|n| ((n * 11 + 7) % 256) as u8).collect();
        let blocks = [
            RelocBlock { page_rva: 0x3000, highlow_offsets: vec![0xFFD] },
            RelocBlock { page_rva: 0x4000, highlow_offsets: vec![0x001, 0x010, 0xF00] },
        ];
        // The first relocation crosses a page; the second page has a
        // relocation that straddles a deliberately short virtual limit.
        let effective = 4096 + 18;
        let mut altered = bytes.clone();
        let base = checksum_pages_bounded_without_highlow(
            &bytes, 0x3000, &blocks, effective, 0).unwrap();
        let full_raw = checksum_pages_without_highlow(&bytes, 0x3000, &blocks, 0).unwrap();
        assert_ne!(base, full_raw);
        for byte in &mut altered[effective..] { *byte ^= 0xFF; }
        assert_eq!(base, checksum_pages_bounded_without_highlow(
            &altered, 0x3000, &blocks, effective, 0).unwrap());
        // Only excluded relocation bytes in the effective portion are
        // immaterial, including the 4 KiB page-straddling DWORD.
        for &at in &[4093usize,4094,4095,4096,4097, 4112,4113] {
            altered[at] ^= 0xA5;
        }
        assert_eq!(base, checksum_pages_bounded_without_highlow(
            &altered, 0x3000, &blocks, effective, 0).unwrap());
        altered[4105] ^= 0x7F;
        assert_ne!(base, checksum_pages_bounded_without_highlow(
            &altered, 0x3000, &blocks, effective, 0).unwrap());
        assert!(checksum_pages_bounded_without_highlow(
            &bytes, 0x3000, &blocks, 0, 0).is_err());
        assert!(checksum_pages_bounded_without_highlow(
            &bytes, 0x3000, &blocks, bytes.len()+1, 0).is_err());
        assert_eq!(full_raw, checksum_pages_bounded_without_highlow(
            &bytes, 0x3000, &blocks, bytes.len(), 0).unwrap());
    }

    #[test]
    fn independent_relocation_walker_matches_page_candidate_on_boundaries() {
        let bytes: Vec<u8> = (0..12288).map(|i| ((i * 23 + 1) % 256) as u8).collect();
        for crossing in [0xFFDu16, 0xFFE, 0xFFF] {
            let block_a = RelocBlock { page_rva: 0x2000,
                highlow_offsets: vec![0x007, crossing] };
            let block_b = RelocBlock { page_rva: 0x3000,
                highlow_offsets: vec![0x010, 0xC00] };
            let blocks = [block_a, block_b];
            // Include partial last page and an exclusion carrying
            // across the boundary of pages 0 and 1.
            for effective in [4093usize, 4096, 4097, 4110, 8192, 10003, bytes.len()] {
                let old = checksum_pages_bounded_without_highlow(
                    &bytes, 0x2000, &blocks, effective, 0).unwrap();
                let result = checksum_independent_relocation_walk(
                    &bytes, 0x2000, &blocks, effective).unwrap();
                assert_eq!(result.checksum, old, "crossing={crossing:x}, extent={effective}");
                assert_eq!(result.included_bytes + result.excluded_bytes, effective);
                assert_eq!(result.crossings, usize::from(
                    (crossing as usize) < effective && (crossing as usize + 4) > 4096));
            }
        }

        // Missing relocation block: entirely retained first page.
        let data = vec![0xA5u8; 8192];
        let block = RelocBlock { page_rva: 0x3000, highlow_offsets: vec![5, 17] };
        let result = checksum_independent_relocation_walk(
            &data, 0x2000, &[block.clone()], 5000).unwrap();
        assert_eq!(result.checksum,
            checksum_pages_bounded_without_highlow(&data, 0x2000, &[block], 5000, 0).unwrap());
        assert_eq!(result.included_bytes + result.excluded_bytes, 5000);
        assert_eq!(result.fragments, 4);
        assert!(checksum_independent_relocation_walk(&data, 0x2000, &[], 0).is_err());
        assert!(checksum_independent_relocation_walk(&data, 0x2000, &[], 8193).is_err());
    }

    #[test]
    fn historic_three_word_checksum_stage_matches_instruction_formula() {
        let sources = [0x1122_3344, 0x5566_7788, 0x99AA_BBCC];
        let derived = derive_three_checksum_dependent_words(
            sources, 0x1234_5678, 0x0002_8000, 0x0F0E_0D0C, 0x1B09_633B);
        assert_eq!(derived, [0x1717_11AB, 0x5353_4CF7, 0x9F9E_8933]);

        // The historical x86 arithmetic is modulo 2^32, including the
        // address adjustment and final addition.
        let overflowing = derive_three_checksum_dependent_words(
            [u32::MAX, 0, 0x1234_5678],
            0, u32::MAX, 0x1234_5678, 0x55AA_55AA);
        let base = u32::MAX.wrapping_add(0x2D8);
        let mix = 0u32.wrapping_sub(base) ^ 0x1234_5678 ^ 0x55AA_55AA;
        assert_eq!(overflowing[0], (u32::MAX ^ mix).wrapping_add(base));
        assert_eq!(overflowing[1], mix.wrapping_add(base));
        assert_eq!(overflowing[2], (0x1234_5678 ^ mix).wrapping_add(base));
    }

    #[test]
    fn simple_page_checksum_follows_historic_page_and_remainder_branch() {
        let mut bytes: Vec<u8> = (0..8192).map(|i| ((13*i + 5) % 251) as u8).collect();
        let effective = 4096+17usize;
        let first = checksum_chunk(&bytes[..4096], 0);
        let expected = checksum_chunk(&bytes[4096..effective], first);
        assert_eq!(checksum_historic_simple_pages(&bytes, effective, 0).unwrap(), expected);

        let entire = checksum_chunk(&bytes[4096..8192],
            checksum_chunk(&bytes[..4096], 0));
        assert_eq!(checksum_historic_simple_pages(&bytes, 8192, 0).unwrap(), entire);

        let old = checksum_historic_simple_pages(&bytes, effective, 0).unwrap();
        for b in bytes.iter_mut().skip(effective) { *b ^= 0xA5; }
        assert_eq!(old, checksum_historic_simple_pages(&bytes, effective, 0).unwrap());
        // Unlike the relocation-aware model, the simple branch also
        // mixes in the bytes at HIGHLOW relocation sites.
        bytes[4101] ^= 0xFF;
        assert_ne!(old, checksum_historic_simple_pages(&bytes, effective, 0).unwrap());
        assert!(checksum_historic_simple_pages(&bytes, 0, 0).is_err());
        assert!(checksum_historic_simple_pages(&bytes, bytes.len()+1, 0).is_err());
    }

    #[test]
    fn original_dispatch_comparison_is_to_four() {
        // Bytes around 0x401D43 in the historical PE:
        // mov esi, 0x403CB0 ; cmp dword [esi-4], 4 ; je 0x401D82.
        // Using synthetic selector values must retain both branches,
        // without claiming which value the original Crimson run had.
        fn branch_uses_relocs(selector: u32) -> bool { selector == 4 }
        assert!(branch_uses_relocs(4));
        for x in [0,1,3,5,u32::MAX] {
            assert!(!branch_uses_relocs(x));
        }
    }

    #[test]
    fn historical_branch_inference_checks_actual_pe_header_guards() {
        let mut image = synthetic_pe();
        let section_at = 0x178usize;
        let optional = 0x98usize;
        let reloc_directory = optional + 96 + 5 * 8;
        image[section_at + 12..section_at + 16].copy_from_slice(&0x3000u32.to_le_bytes());
        image[reloc_directory..reloc_directory + 4]
            .copy_from_slice(&0x3020u32.to_le_bytes());
        image[reloc_directory + 4..reloc_directory + 8]
            .copy_from_slice(&16u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        let found = infer_historical_reloc_branch(&image, &pe).unwrap().unwrap();
        assert_eq!(found.directory_rva, 0x3020);
        assert_eq!(found.directory_len, 16);
        assert_eq!(found.section_name, ".idata");
        assert_eq!(found.section_rva, 0x3000);
        assert_eq!(found.next_header_guard, 0);

        // Original branch checks the +0x34 guard DWORD of the
        // relocation-containing section header before setting 4.
        image[section_at+0x34..section_at+0x38]
            .copy_from_slice(&0x3020u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert!(infer_historical_reloc_branch(&image, &pe).unwrap().is_none());
        image[section_at+0x34..section_at+0x38].fill(0);

        // An unbacked section, missing directory or directory past raw
        // section data may not be treated as proven static dispatch.
        image[section_at+20..section_at+24].fill(0);
        let pe = parse_pe32(&image).unwrap();
        assert!(infer_historical_reloc_branch(&image, &pe).unwrap().is_none());
        image[section_at+20..section_at+24]
            .copy_from_slice(&0x200u32.to_le_bytes());
        image[reloc_directory..reloc_directory+4].fill(0);
        let pe = parse_pe32(&image).unwrap();
        assert!(infer_historical_reloc_branch(&image, &pe).unwrap().is_none());
        image[reloc_directory..reloc_directory+4]
            .copy_from_slice(&0x30FFu32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert!(infer_historical_reloc_branch(&image, &pe).unwrap().is_none());
    }

    #[test]
    fn tea_key_formula_matches_independently_calculated_synthetic_vector() {
        let raw = [0x1020_3040,0xA0B0_C0D0,0xFEDC_BA98,0x7654_3210];
        let result = construct_tea_candidate(0x1234_5678,0x89AB_CDEF,0x0010_4000,raw);
        assert_eq!(result,[0x1234_5678,0x2B2F_71B7,0x7583_976F,0xFE0C_1EF7]);
        let changed = construct_tea_candidate(0x1234_5678,0x89AB_CDEE,0x0010_4000,raw);
        assert_eq!(result[0],changed[0]);
        assert!(result[1..].iter().zip(changed[1..].iter()).all(|(a,b)| a != b));
    }

    #[test]
    fn reported_key_hex_syntax() {
        assert_eq!(parse_hex_u32("7496538f").unwrap(), REPORTED_FIRST_WORD);
        assert_eq!(parse_hex_u32("0x7496538F").unwrap(), REPORTED_FIRST_WORD);
        assert!(parse_hex_u32("0x100000000").is_err());
        assert!(parse_hex_u32("not_a_key").is_err());
    }
}
