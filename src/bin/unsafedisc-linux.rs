// Native Linux SafeDisc research tool for user-owned Crimson Skies 1.0.2.0 files.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Research stage: strictly read-only. No loader/DLL execution, no executable
// reconstruction, no SafeDisc driver, and no Wine requirement.

#[path = "../unsafedisc_strings.rs"]
mod unsafedisc_strings;

#[path = "../unsafedisc_algebra.rs"]
mod unsafedisc_algebra;

#[path = "../unsafedisc_indexed_affine.rs"]
mod unsafedisc_indexed_affine;

#[path = "../unsafedisc_historical_155.rs"]
mod unsafedisc_historical_155;

#[path = "../unsafedisc_modifier_snapshot.rs"]
mod unsafedisc_modifier_snapshot;

#[path = "../unsafedisc_candidate_forward.rs"]
mod unsafedisc_candidate_forward;
#[path = "../unsafedisc_historical_155_list_plan.rs"]
mod unsafedisc_historical_155_list_plan;
#[path = "../unsafedisc_historical_155_reloc_builder.rs"]
mod unsafedisc_historical_155_reloc_builder;
#[path = "../unsafedisc_historical_155_reloc_consumer.rs"]
mod unsafedisc_historical_155_reloc_consumer;
#[path = "../unsafedisc_historical_155_reloc_hazards.rs"]
mod unsafedisc_historical_155_reloc_hazards;
#[path = "../unsafedisc_historical_155_page_fragments.rs"]
mod unsafedisc_historical_155_page_fragments;

#[path = "../unsafedisc_second_stage.rs"]
mod unsafedisc_second_stage;

#[path = "../unsafedisc_first_stage.rs"]
mod unsafedisc_first_stage;

#[path = "../unsafedisc_dll_bootstrap.rs"]
mod unsafedisc_dll_bootstrap;

#[path = "../unsafedisc_dll_category_dispatch.rs"]
mod unsafedisc_dll_category_dispatch;

#[path = "../unsafedisc_dll_ranges.rs"]
mod unsafedisc_dll_ranges;

#[path = "../unsafedisc_dll_memory_io.rs"]
mod unsafedisc_dll_memory_io;

#[path = "../unsafedisc_dll_write_pipeline.rs"]
mod unsafedisc_dll_write_pipeline;

#[path = "../unsafedisc_dll_byte_mix.rs"]
mod unsafedisc_dll_byte_mix;
#[path = "../unsafedisc_dll_byte_mix_solver.rs"]
mod unsafedisc_dll_byte_mix_solver;

#[path = "../unsafedisc_dll_state_provenance.rs"]
mod unsafedisc_dll_state_provenance;

#[path = "../unsafedisc_dll_keyboard_probe.rs"]
mod unsafedisc_dll_keyboard_probe;

#[path = "../unsafedisc_dll_worker_finalizer.rs"]
mod unsafedisc_dll_worker_finalizer;

#[path = "../unsafedisc_dll_worker_entry.rs"]
mod unsafedisc_dll_worker_entry;

#[path = "../unsafedisc_dll_io_backends.rs"]
mod unsafedisc_dll_io_backends;

#[path = "../unsafedisc_dll_pe_sections.rs"]
mod unsafedisc_dll_pe_sections;

#[path = "../unsafedisc_dll_a310.rs"]
mod unsafedisc_dll_a310;

#[path = "../unsafedisc_dll_bed0.rs"]
mod unsafedisc_dll_bed0;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

type Result<T> = std::result::Result<T, String>;

// Exact, unmodified 20,480-byte historical user-supplied release.
// The publicly archived version.dll variant is a DIFFERENT 18,944-byte
// file and MUST NOT pass this hash gate.
const HISTORICAL_155_SHA: &str = "e484c9aadf8b3b5bf0db58b26cb9ecfcaba0fd971fb1bb66216f558f0f1eaa9f";
const HISTORICAL_155_LEN: usize = 20_480;
const LOADER_SHA: &str = "675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736";
const ICD_SHA: &str = "0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b";
const DLL_SHA: &str = "dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff";
const TESTME_SHA: &str = "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75";
const LOADER_LEN: usize = 344_851;
const ICD_LEN: usize = 2_580_578;
const DLL_LEN: usize = 138_752;
const TESTME_LEN: usize = 2_580_480;
// Exact successful Windows 98 passive memory snapshot. Research only.
// Never commit or redistribute the captured proprietary memory image.
const SNAPSHOT_LEN: usize = unsafedisc_modifier_snapshot::SNAPSHOT_SIZE;
const SNAPSHOT_SHA: &str = unsafedisc_modifier_snapshot::VERIFIED_SNAPSHOT_SHA256;
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

// Map an executable RVA into the backed file bytes of an x86 PE32
// section. This never loads a DLL, applies relocations or executes code.
// The virtual-only suffix (VirtualSize > SizeOfRawData) is deliberately
// rejected; a DLL loader might otherwise synthesize unbacked zero data.
fn executable_rva_to_file_offset(pe: &Pe32, rva: u32, min_bytes: usize) -> Result<usize> {
    const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
    if min_bytes == 0 || min_bytes > 256 {
        return Err("invalid bounded executable-byte request".into());
    }
    for section in &pe.sections {
        if section.characteristics & IMAGE_SCN_MEM_EXECUTE == 0 {
            continue;
        }
        let Some(relative) = rva.checked_sub(section.virtual_address) else { continue; };
        let relative = relative as usize;
        let Some(end_relative) = relative.checked_add(min_bytes) else { continue; };
        // Use both VirtualSize (unless zero) and backed SizeOfRawData.
        let backed_len = if section.virtual_size == 0 {
            section.raw_len
        } else {
            section.raw_len.min(section.virtual_size as usize)
        };
        if end_relative > backed_len { continue; }
        return section.raw_offset.checked_add(relative)
            .ok_or("executable RVA file offset overflow".into());
    }
    Err(format!("executable RVA 0x{rva:08X} not backed by requested bytes"))
}

// General backed-file RVA mapper for metadata. In particular, TLS
// directory and callback tables may be in readable NON-executable PE
// sections; use a separate resolver from executable_rva_to_file_offset.
fn dll_backed_rva_offset(pe: &Pe32, rva: u32, len: usize) -> Result<usize> {
    if len == 0 || len > 256 {
        return Err("invalid metadata RVA byte count".into());
    }
    for sec in &pe.sections {
        let Some(rel) = rva.checked_sub(sec.virtual_address) else { continue; };
        let rel = rel as usize;
        let backed_len = if sec.virtual_size == 0 { sec.raw_len }
            else { sec.raw_len.min(sec.virtual_size as usize) };
        if rel.checked_add(len).is_some_and(|end| end <= backed_len) {
            return sec.raw_offset.checked_add(rel)
                .ok_or("DLL RVA metadata file offset overflow".into());
        }
    }
    Err(format!("DLL metadata RVA 0x{rva:08X} not fully file-backed"))
}

// IMAGE_DIRECTORY_ENTRY_TLS = 9, IMAGE_TLS_DIRECTORY32 is 24 bytes.
// IMAGE_TLS_DIRECTORY32.AddressOfCallBacks (offset +12) contains a
// *virtual address*, not an RVA. TLS callbacks run BEFORE DllMain.
// Only report callbacks backed by verified original bytes; never call.
#[derive(Debug, PartialEq, Eq)]
struct DllTlsMetadata {
    directory_rva: u32,
    directory_size: u32,
    callbacks_va: u32,
    callback_rvas: Vec<u32>,
}
fn dll_tls_metadata(dll: &[u8], pe: &Pe32) -> Result<Option<DllTlsMetadata>> {
    let nt = read_u32(dll, 0x3C)? as usize;
    let opt = nt.checked_add(24).ok_or("DLL PE optional header overflow")?;
    let opt_size = read_u16(dll, nt + 20)? as usize;
    let dir_offset = 96 + 9 * 8;
    if opt_size < dir_offset + 8 {
        return Err("DLL optional header lacks complete TLS directory slot".into());
    }
    let directory_rva = read_u32(dll, opt + dir_offset)?;
    let directory_size = read_u32(dll, opt + dir_offset + 4)?;
    if directory_rva == 0 && directory_size == 0 {
        return Ok(None);
    }
    if directory_rva == 0 || directory_size < 24 {
        return Err("malformed DLL TLS directory RVA or size".into());
    }
    let image_base = read_u32(dll, opt + 28)?;
    let directory_off = dll_backed_rva_offset(pe, directory_rva, 24)?;
    let callbacks_va = read_u32(dll, directory_off + 12)?;
    let mut callback_rvas = Vec::new();
    if callbacks_va != 0 {
        let callbacks_rva = callbacks_va.checked_sub(image_base)
            .ok_or("DLL TLS callbacks VA below preferred image base")?;
        for idx in 0..8usize {
            let byte_offset = (idx * 4) as u32;
            let elem_rva = callbacks_rva.checked_add(byte_offset)
                .ok_or("TLS callbacks RVA overflow")?;
            let elem_off = dll_backed_rva_offset(pe, elem_rva, 4)?;
            let callback_va = read_u32(dll, elem_off)?;
            if callback_va == 0 {
                return Ok(Some(DllTlsMetadata {
                    directory_rva, directory_size, callbacks_va, callback_rvas,
                }));
            }
            callback_rvas.push(callback_va.checked_sub(image_base)
                .ok_or("TLS callback VA below preferred image base")?);
        }
        return Err("DLL TLS callback list has no terminator within bounded eight entries".into());
    }
    Ok(Some(DllTlsMetadata {
        directory_rva, directory_size, callbacks_va, callback_rvas,
    }))
}

// Print executable code windows by explicit RVA, rather than assuming
// the DLL entry is the only path executed by the Windows PE loader.
fn print_dll_code_window(dll: &[u8], pe: &Pe32, title: &str, rva: u32, len: usize) -> Result<()> {
    let offset = executable_rva_to_file_offset(pe, rva, len)?;
    let code = dll.get(offset..offset + len)
        .ok_or("requested startup code outside original DLL file")?;
    println!("  {title}: RVA 0x{rva:08X}, raw file +0x{offset:06X}, {len}-byte static excerpt");
    for (line, chunk) in code.chunks(16).enumerate() {
        let at_rva = rva.checked_add((line * 16) as u32)
            .ok_or("startup code label RVA overflow")?;
        let hex = chunk.iter().map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>().join(" ");
        println!("    0x{at_rva:08X}: {hex}");
    }
    Ok(())
}

// Distinguish PE virtual-zero-fill from an RVA outside the mapped DLL.
// When VirtualSize > SizeOfRawData, the loader zero-initializes the
// remainder of the section's memory span; it cannot be read from disk.
fn dll_zero_filled_section<'a>(
    pe: &'a Pe32, rva: u32, len: usize,
) -> Option<&'a PeSection> {
    if len == 0 { return None; }
    pe.sections.iter().find(|sec| {
        if sec.virtual_size as usize <= sec.raw_len { return false; }
        let Some(rel) = rva.checked_sub(sec.virtual_address) else { return false; };
        let rel = rel as usize;
        rel >= sec.raw_len
            && rel.checked_add(len).is_some_and(|end| end <= sec.virtual_size as usize)
    })
}

// Decode ONLY a known x86 direct relative CALL (E8 rel32) at a
// confirmed instruction boundary, then validate its target inside a
// file-backed executable section. Not a generic x86 disassembler.
fn dll_relative_call_target(dll: &[u8], pe: &Pe32, call_rva: u32) -> Result<u32> {
    let offset = executable_rva_to_file_offset(pe, call_rva, 5)?;
    if dll.get(offset).copied() != Some(0xE8) {
        return Err(format!("expected E8 rel32 at DLL RVA 0x{call_rva:08X}"));
    }
    let relative = read_u32(dll, offset + 1)? as i32 as i64;
    let after = i64::from(call_rva) + 5;
    let target = u32::try_from(after + relative)
        .map_err(|_| "relative DLL call points outside 32-bit RVA space")?;
    executable_rva_to_file_offset(pe, target, 64)?;
    Ok(target)
}

// PE base relocations offer a more grounded way to identify on-disk
// absolute pointers to .txt than scanning for arbitrary byte patterns.
// Inspect both executable-only and all file-backed source sections:
// relocation-backed pointer tables may live in .data/.rdata, even when
// the code consuming those tables is in another executable section.
// Relocation-backed DWORDs are still just pointers (possibly data),
// NOT proof the containing bytes decode to reachable x86 instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DllCodeReference {
    source_rva: u32,
    source_section: String,
    target_rva: u32,
}
fn dll_code_reloc_refs_to_section(
    dll: &[u8], pe: &Pe32, blocks: &[RelocBlock],
    preferred_base: u32, target: &PeSection, include_data_sections: bool,
) -> Result<(usize, Vec<DllCodeReference>)> {
    const MAX_REFERENCES: usize = 100_000;
    let mut source_relocations = 0usize;
    let mut references = Vec::new();
    let target_end = target.virtual_address as u64 +
        u64::from(target.virtual_size.max(target.raw_len as u32));
    for block in blocks {
        for &offset in &block.highlow_offsets {
            let source_rva = block.page_rva.checked_add(u32::from(offset))
                .ok_or("DLL relocation source RVA overflow")?;
            let Some(source) = pe.sections.iter().find(|section| {
                (include_data_sections || section.characteristics & 0x2000_0000 != 0)
                && section.name.as_slice() != target.name.as_slice()
                && source_rva.checked_sub(section.virtual_address).is_some_and(|rel| {
                    (rel as usize).checked_add(4).is_some_and(|end| {
                        end <= section.raw_len.min(if section.virtual_size == 0 {
                            section.raw_len
                        } else { section.virtual_size as usize })
                    })
                })
            }) else { continue; };
            source_relocations += 1;
            let at = dll_backed_rva_offset(pe, source_rva, 4)?;
            let pointer_va = read_u32(dll, at)?;
            let Some(pointer_rva) = pointer_va.checked_sub(preferred_base) else { continue; };
            if u64::from(pointer_rva) < u64::from(target.virtual_address) ||
               u64::from(pointer_rva) >= target_end {
                continue;
            }
            let name = String::from_utf8_lossy(
                source.name.split(|&b| b==0).next().unwrap_or(&[])).to_string();
            references.push(DllCodeReference {
                source_rva, source_section:name, target_rva:pointer_rva,
            });
            if references.len() > MAX_REFERENCES {
                return Err("unexpectedly many protected-section xrefs".into());
            }
        }
    }
    Ok((source_relocations, references))
}

fn print_dll_protected_xrefs(dll: &[u8]) -> Result<()> {
    let pe = parse_pe32(dll)?;
    let nt = read_u32(dll,0x3C)? as usize;
    let image_base = read_u32(dll,nt+24+28)?;
    let protected = pe.section(b".txt")?;
    if protected.characteristics & 0x2000_0000 == 0 {
        return Err("protected .txt section is not executable".into());
    }
    let (blocks, padding) = pe_relocation_blocks(dll,&pe)?;
    let total_highlow:usize = blocks.iter().map(|block| block.highlow_offsets.len()).sum();
    let (executable_source_count, executable_refs) =
        dll_code_reloc_refs_to_section(dll,&pe,&blocks,image_base,protected,false)?;
    let (all_backed_source_count, references) =
        dll_code_reloc_refs_to_section(dll,&pe,&blocks,image_base,protected,true)?;
    println!("SafeDisc 1.50.20 DLL relocation-backed protected .txt reference research:");
    println!("  PE preferred image base 0x{image_base:08X}; all input bytes from exact verified original dplayerx.dll.");
    println!("  Protected target: .txt RVA 0x{:08X}, raw bytes {}, virtual bytes {}.",
        protected.virtual_address,protected.raw_len,protected.virtual_size);
    println!("  DLL base relocations: {} pages, {} HIGHLOW, {} ABSOLUTE padding.",
        blocks.len(),total_highlow,padding);
    println!("  Backed HIGHLOW words in other executable sections: {executable_source_count}");
    println!("  Backed HIGHLOW words in all other sections (code + data): {all_backed_source_count}");
    println!("  Relocation-backed references into .txt from executable sections: {}",executable_refs.len());
    println!("  Relocation-backed references into .txt from ALL backed non-.txt sections: {}",references.len());
    for name in [".text",".txt2",".rdata",".data",".reloc"] {
        let sub:Vec<_>=references.iter().filter(|r|r.source_section==name).collect();
        println!("  Origin section {name}: {} matching pointers.",sub.len());
        for r in sub.iter().take(24) {
            let distance_a = r.target_rva.abs_diff(0x65BEu32);
            let distance_b = r.target_rva.abs_diff(0x6621u32);
            println!("    DWORD source RVA 0x{:08X} -> .txt RVA 0x{:08X}{}",
                r.source_rva,r.target_rva,
                if distance_a <= 0x100 || distance_b <= 0x100 { " (near known modifier RVA)" } else { "" });
        }
        if sub.len() > 24 {
            println!("    ... {} additional relocation-backed references omitted.",sub.len()-24);
        }
    }
    for (name,rva) in [("first EAX modifier",0x65BEu32),
                       ("second EAX modifier",0x6621u32)] {
        let callers:Vec<_> = references.iter().filter(|r|r.target_rva == rva).collect();
        println!("  Exact relocation-backed reference to {name} RVA 0x{rva:08X}: {}.",callers.len());
        for r in callers.iter().take(16) {
            println!("    From {} source RVA 0x{:08X}",r.source_section,r.source_rva);
        }
    }
    println!("  Only PE relocation-backed DWORD references count: not a full cross-reference graph.");
    println!("  They can be data pointers; indirect control flow and code that constructs addresses at runtime are not covered.");
    println!("  No protected section is decrypted; no file is written or DLL loaded.");
    Ok(())
}

// This is a deliberately limited *literal-byte* scan: unlike PE
// HIGHLOW relocations, raw 32-bit constants cannot establish pointer
// semantics. Scan both LE RVA and preferred-base VA encodings, but
// exclude protected .txt bytes and every 4-byte window overlapping
// any HIGHLOW DWORD (already handled by --xref-research).
// Overlapping x86 instructions can also produce apparent constants.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DllLiteralHit {
    source_rva: u32,
    value: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct DllLiteralHits {
    rvas: Vec<DllLiteralHit>,
    absolute_vas: Vec<DllLiteralHit>,
    special: Vec<DllLiteralHit>,
}
fn dll_unrelocated_literals(
    dll: &[u8], section: &PeSection,
    relocations: &[RelocBlock], preferred_base: u32,
    target: &PeSection,
) -> Result<DllLiteralHits> {
    const CODE_SIZE: usize = 4;
    let backed = section.raw_len.min(if section.virtual_size == 0 {
        section.raw_len
    } else { section.virtual_size as usize });
    let end = section.raw_offset.checked_add(backed)
        .ok_or("literal source raw range overflow")?;
    let bytes = dll.get(section.raw_offset..end)
        .ok_or("literal source raw section not within verified DLL")?;
    let mut relocated = vec![false; backed];
    for block in relocations {
        for &offset in &block.highlow_offsets {
            let source_rva = block.page_rva.checked_add(offset as u32)
                .ok_or("literal relocation RVA overflow")?;
            let Some(local) = source_rva.checked_sub(section.virtual_address) else { continue; };
            let local = local as usize;
            if local < backed {
                let until = local.saturating_add(CODE_SIZE).min(backed);
                relocated[local..until].fill(true);
            }
        }
    }
    let target_end = target.virtual_address.checked_add(target.virtual_size)
        .ok_or("protected target RVA overflow")?;
    let va_start = preferred_base.checked_add(target.virtual_address)
        .ok_or("protected target VA overflow")?;
    let va_end = preferred_base.checked_add(target_end)
        .ok_or("protected target VA extent overflow")?;
    let candidates = [
        target.virtual_address,
        target_end,
        target.virtual_size,
        target.raw_len as u32,
        0x65BE,
        0x6621,
        preferred_base.checked_add(0x65BE).ok_or("modifier VA overflow")?,
        preferred_base.checked_add(0x6621).ok_or("modifier VA overflow")?,
    ];
    let mut hits = DllLiteralHits {
        rvas: Vec::new(),absolute_vas:Vec::new(),special:Vec::new(),
    };
    for i in 0..backed.saturating_sub(3) {
        if relocated[i..i+CODE_SIZE].contains(&true) { continue; }
        let value = u32::from_le_bytes(bytes[i..i+4].try_into().unwrap());
        let location = section.virtual_address.checked_add(i as u32)
            .ok_or("literal source RVA overflow")?;
        let hit = DllLiteralHit { source_rva:location,value };
        if target.virtual_address <= value && value < target_end {
            hits.rvas.push(hit.clone());
        }
        if va_start <= value && value < va_end {
            hits.absolute_vas.push(hit.clone());
        }
        if candidates.contains(&value) {
            hits.special.push(hit);
        }
    }
    Ok(hits)
}

// Print a small *byte context* around a verified exact literal.
// This keeps the follow-up output focused on the few constants that
// might plausibly encode a section boundary, allocation size or RVA.
// The four-byte value is verified against the user's hash-verified
// original before any surrounding bytes are printed.
fn print_dll_exact_literal_context(
    dll: &[u8], pe: &Pe32, source_rva: u32, expected: u32,
    interpretation: &str,
) -> Result<()> {
    const BEFORE: u32 = 16;
    const AFTER: u32 = 28;
    let source_offset = dll_backed_rva_offset(pe, source_rva, 4)?;
    let observed = read_u32(dll, source_offset)?;
    if observed != expected {
        return Err(format!(
            "original DLL literal at RVA 0x{source_rva:08X} changed: expected 0x{expected:08X}, got 0x{observed:08X}"
        ));
    }
    let source = pe.sections.iter().find(|sec| {
        source_rva.checked_sub(sec.virtual_address).is_some_and(|rel| {
            let len = sec.raw_len.min(if sec.virtual_size == 0 {
                sec.raw_len
            } else { sec.virtual_size as usize });
            (rel as usize).checked_add(4).is_some_and(|end| end <= len)
        })
    }).ok_or("literal context source has no PE-backed section")?;
    let raw_len = source.raw_len.min(if source.virtual_size == 0 {
        source.raw_len
    } else { source.virtual_size as usize });
    let raw_start = source_rva - source.virtual_address;
    let window_start = raw_start.saturating_sub(BEFORE);
    let window_end = (raw_start as usize)
        .checked_add(4+AFTER as usize)
        .ok_or("literal context window overflow")?
        .min(raw_len);
    let start = source.raw_offset.checked_add(window_start as usize)
        .ok_or("literal context raw-offset overflow")?;
    let end = source.raw_offset.checked_add(window_end)
        .ok_or("literal context end overflow")?;
    let around = dll.get(start..end)
        .ok_or("literal context outside original DLL file")?;
    println!("  Focus RVA 0x{source_rva:08X} = 0x{observed:08X} ({interpretation}; not a decoded instruction)");
    for (i, chunk) in around.chunks(16).enumerate() {
        let rva = source.virtual_address
            .checked_add(window_start)
            .and_then(|value| value.checked_add((i * 16) as u32))
            .ok_or("literal context display RVA overflow")?;
        let value = chunk.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
        println!("    0x{rva:08X}: {value}");
    }
    Ok(())
}

// This independent reference comparison is a *validation oracle*
// for future genuine SafeDisc decryption, NOT a decryption step.
// Only the exact known-hash unwrapped original may be used as the
// plaintext reference; no file is reconstructed or written.
// Exact per-site import-index unrotation found in the *original*
// historical unSafeDisc 1.5.5 x86 code (0x401869..0x4018D9).
//
// After identifying FF 15 and locating the old IAT table, the tool
// computes shift=(instruction_OPERAND_site_RVA - .text_RVA - 2
//                   + reported_first_word) modulo imported_count.
// For the user's SHA-256 verified original Crimson Skies 1.0.2.0,
// the two affected IAT tables are KERNEL32 (74 entries) and
// USER32 (62 entries). The historical algorithm rotates the entry
// index backwards by this shift. This is why no global old-VA -> new-VA
// dictionary is possible. All values below are PE virtual ADDRESSES,
// not file offsets or arbitrary guesses for unrelated executables.
fn historical_155_unrotate_import_va(
    site_rva: u32, original_va: u32, first_word: u32, text_rva: u32,
) -> Option<u32> {
    const IATS: [(u32,u32);2]=[
        (0x00A1_FC7C,74),  // KERNEL32.dll, exact verified Crimson image
        (0x00A2_039C,62),  // USER32.dll, exact verified Crimson image
    ];
    let local=site_rva.checked_sub(text_rva)?.checked_sub(2)?;
    for (first,count) in IATS {
        let last=first.checked_add(count.checked_mul(4)?)?;
        if original_va>=first && original_va<last &&
           (original_va-first)%4==0 {
            let index=(original_va-first)/4;
            let shift=local.wrapping_add(first_word)%count;
            let restored=(index+count-shift)%count;
            return first.checked_add(restored.checked_mul(4)?);
        }
    }
    None
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ReferenceTextAudit {
    relocation_words: usize,
    identical_relocation_words: usize,
    relocated_bytes: usize,
    identical_relocated_bytes: usize,
    plain_bytes: usize,
    identical_plain_bytes: usize,
    import_pointer_mismatches: usize,
    historically_restored_import_pointers: usize,
    unresolved_import_pointers: usize,
    ambiguous_import_va_sources: usize,
    first_page_plain_equal: usize,
    first_page_plain_total: usize,
}
fn audit_reference_text_bytes(
    cipher: &[u8], plain: &[u8],
    text_rva: u32,
    reloc_blocks: &[RelocBlock],
    preferred_base: u32,
    idata: &PeSection,
) -> Result<ReferenceTextAudit> {
    if cipher.len()!=plain.len() {
        return Err("protected and unwrapped .text lengths differ".into());
    }
    let mut out=ReferenceTextAudit::default();
    let mut flagged=vec![false;cipher.len()];
    let mut import_sources:std::collections::HashMap<u32,std::collections::HashSet<u32>>=
        std::collections::HashMap::new();
    let import_start=preferred_base.checked_add(idata.virtual_address)
        .ok_or("IAT section address overflow")?;
    let import_end=import_start.checked_add(idata.virtual_size.max(idata.raw_len as u32))
        .ok_or("IAT section upper bound overflow")?;
    for block in reloc_blocks {
        for &rel_off in &block.highlow_offsets {
            let source_rva=block.page_rva.checked_add(u32::from(rel_off))
                .ok_or("reference relocation RVA overflow")?;
            let Some(local) = source_rva.checked_sub(text_rva) else { continue };
            let local=local as usize;
            if local>=cipher.len() { continue; }
            let Some(end)=local.checked_add(4) else { return Err("reference relocation DWORD overflow".into()) };
            if end>cipher.len() { return Err("reference .text HIGHLOW DWORD crosses .text boundary".into()) }
            if flagged[local..end].contains(&true) {
                return Err("overlapping .text HIGHLOW references in original image".into());
            }
            flagged[local..end].fill(true);
            out.relocation_words+=1;
            let x=u32::from_le_bytes(cipher[local..end].try_into().unwrap());
            let y=u32::from_le_bytes(plain[local..end].try_into().unwrap());
            if x==y {out.identical_relocation_words+=1}
            else if x%4==0 && y%4==0 &&
                import_start<=x && x<import_end &&
                import_start<=y && y<import_end {
                out.import_pointer_mismatches+=1;
                let restored=historical_155_unrotate_import_va(
                    source_rva,x,REPORTED_FIRST_WORD,text_rva);
                if restored==Some(y) {
                    out.historically_restored_import_pointers+=1;
                } else {
                    out.unresolved_import_pointers+=1;
                }
                import_sources.entry(x).or_default().insert(y);
            }
        }
    }
    out.ambiguous_import_va_sources=import_sources.values().filter(|targets|targets.len()>1).count();
    for (i, (&x,&y)) in cipher.iter().zip(plain).enumerate() {
        if flagged[i] {
            out.relocated_bytes+=1;
            if x==y {out.identical_relocated_bytes+=1}
        } else {
            out.plain_bytes+=1;
            if x==y {out.identical_plain_bytes+=1}
            if i<4096 {
                out.first_page_plain_total+=1;
                if x==y {out.first_page_plain_equal+=1}
            }
        }
    }
    Ok(out)
}
fn print_reference_audit(
    cipher: &[u8], plain: &[u8],
    cipher_pe: &Pe32, plain_pe: &Pe32,
    relocs: &[RelocBlock],
) -> Result<()> {
    let original_text=cipher_pe.section(b".text")?;
    let plain_text=plain_pe.section(b".text")?;
    if original_text.virtual_address != plain_text.virtual_address ||
       original_text.raw_offset != plain_text.raw_offset ||
       original_text.raw_len != plain_text.raw_len ||
       original_text.virtual_size != plain_text.virtual_size {
        return Err("protected and reference .text PE layout differs".into());
    }
    let idata=cipher_pe.section(b".idata")?;
    let nt=read_u32(cipher,0x3C)? as usize;
    let base=read_u32(cipher,nt+24+28)?;
    let orig_end=original_text.raw_offset.checked_add(original_text.raw_len)
        .ok_or("protected .text slice overflow")?;
    let ref_end=plain_text.raw_offset.checked_add(plain_text.raw_len)
        .ok_or("reference .text slice overflow")?;
    let original_bytes=cipher.get(original_text.raw_offset..orig_end)
        .ok_or("protected original .text outside file")?;
    let reference_bytes=plain.get(plain_text.raw_offset..ref_end)
        .ok_or("unwrapped reference .text outside file")?;
    let audit=audit_reference_text_bytes(
        original_bytes,reference_bytes,original_text.virtual_address,
        relocs,base,idata)?;
    println!("Exact original ICD/TESTME.EXE relocation-aware plaintext audit (NO decryption):");
    println!("  Original .text HIGHLOW DWORDs: {}",audit.relocation_words);
    println!("  Exactly identical HIGHLOW DWORDs: {}",audit.identical_relocation_words);
    println!("  Relocation-covered byte matches: {} / {}",
        audit.identical_relocated_bytes,audit.relocated_bytes);
    println!("  Other .text byte matches: {} / {}",
        audit.identical_plain_bytes,audit.plain_bytes);
    println!("  First 4096-byte .text page, non-HIGHLOW matches: {} / {}",
        audit.first_page_plain_equal,audit.first_page_plain_total);
    println!("  Differing HIGHLOW words pointing into .idata on both sides: {}",
        audit.import_pointer_mismatches);
    println!("  Protected IAT VAs that map to multiple reference IAT VAs: {}",
        audit.ambiguous_import_va_sources);
    println!("  Historic per-site 1.5.5 index unrotation matches against exact reference: {} / {}",
        audit.historically_restored_import_pointers,audit.import_pointer_mismatches);
    println!("  Import-address mismatches not explained by original algorithm: {}",
        audit.unresolved_import_pointers);
    if audit.relocation_words == 61_743
        && audit.identical_relocation_words == 61_498
        && (audit.import_pointer_mismatches != 245
            || audit.historically_restored_import_pointers != 245
            || audit.unresolved_import_pointers != 0) {
        return Err("historical 1.5.5 import rotation does not reproduce all 245 verified Crimson IAT references".into());
    }
    println!("  Treat IAT replacement as site-specific; NEVER infer a global VA map.");
    println!("  These results validate the reference relationship, not a recovered cipher or TEA key.");
    Ok(())
}

fn print_dll_literal_research(dll: &[u8]) -> Result<()> {
    const MAX_LIST: usize = 16;
    let pe = parse_pe32(dll)?;
    let nt = read_u32(dll,0x3C)? as usize;
    let base = read_u32(dll,nt+24+28)?;
    let target = pe.section(b".txt")?;
    let (blocks, _) = pe_relocation_blocks(dll,&pe)?;
    let mut total_rva = 0;
    let mut total_va = 0;
    let mut total_special = 0;
    let mut modifier_rva_hits = 0usize;
    let mut modifier_va_hits = 0usize;
    let mut raw_section_size_hits = 0usize;
    println!("SafeDisc DLL protected .txt literal research (NON-relocated DWORD candidates):");
    println!("  Base 0x{base:08X}; protected section RVA 0x{:08X}..0x{:08X} (end excluded)",
        target.virtual_address,target.virtual_address.checked_add(target.virtual_size)
            .ok_or("protected section virtual end overflow")?);
    println!("  Exclude HIGHLOW-relocated DWORD overlaps; these were analyzed separately by --xref-research.");
    for section in &pe.sections {
        if section.name == target.name || section.name.starts_with(b".reloc") {
            continue;
        }
        let name=String::from_utf8_lossy(section.name.split(|&b| b==0).next().unwrap_or(&[]));
        let hits=dll_unrelocated_literals(dll,section,&blocks,base,target)?;
        total_rva+=hits.rvas.len();total_va+=hits.absolute_vas.len();total_special+=hits.special.len();
        modifier_rva_hits += hits.special.iter().filter(|h| h.value == 0x65BE || h.value == 0x6621).count();
        modifier_va_hits += hits.special.iter().filter(|h| h.value == base+0x65BE || h.value == base+0x6621).count();
        raw_section_size_hits += hits.special.iter().filter(|h| h.value == target.raw_len as u32).count();
        println!("  Section {name}: raw LE RVA-range {}, preferred-base VA-range {}, exact section/modifier constants {}.",
            hits.rvas.len(),hits.absolute_vas.len(),hits.special.len());
        for (kind, values) in [
            ("RVA-range", &hits.rvas),
            ("VA-range", &hits.absolute_vas),
            ("exact marker", &hits.special),
        ] {
            let limit = if kind == "exact marker" { 64 } else { MAX_LIST };
            for item in values.iter().take(limit) {
                println!("    {kind} source RVA 0x{:08X}: 0x{:08X}",
                    item.source_rva,item.value);
            }
            if values.len()>limit {
                println!("    {kind}: {} further unclassified matches omitted.",values.len()-limit);
            }
        }
    }
    println!("  Total NON-relocated 4-byte window matches: RVA-range {total_rva}, VA-range {total_va}, exact special markers {total_special}.");
    println!("  Explicit exact modifier RVA hits (0x65BE, 0x6621): {modifier_rva_hits}; modifier preferred VA hits: {modifier_va_hits}.");
    println!("  Exact original .txt raw-size marker (0x{:X}): {raw_section_size_hits} hits.",target.raw_len);
    println!("  Focused original on-disk byte contexts (literal starts at its RVA, not necessarily an instruction boundary):");
    // Values from the user-observed report of the exact original DLL.
    // 0x1000 also appears as HeapCreate initial size at RVA 0x116E3;
    // 0x7800 equals the original .txt SizeOfRawData, 30720 bytes.
    // Neither value is, by itself, a pointer to protected code.
    for (rva, expected, why) in [
        (0x9410u32, 0x1000u32, ".text 4 KiB marker"),
        (0x9427, 0x1000, ".text 4 KiB marker"),
        (0x116E3, 0x1000, "HeapCreate initial heap-size control"),
        (0x1921A, 0x1000, ".txt2 4 KiB marker"),
        (0x19230, 0x1000, ".txt2 4 KiB marker"),
        (0x19287, 0x1000, ".txt2 4 KiB marker"),
        (0x192A1, 0x1000, ".txt2 4 KiB marker"),
        (0x22FFF, 0x7800, ".data value equals protected .txt raw size"),
        (0x230CF, 0x7800, ".data value equals protected .txt raw size"),
        (0x23148, 0x1000, ".data 4 KiB marker"),
    ] {
        print_dll_exact_literal_context(dll,&pe,rva,expected,why)?;
    }
    println!("  None of these exact constants proves a decoded .txt function or runtime reference.");
    println!("  LE constants are not established absolute pointers or reachable x86 operands.");
    println!("  A zero count cannot exclude computed addresses, relative branches, or decoded runtime code.");
    println!("  Original DLL is only read; no protected code is executed or written.");
    Ok(())
}

// Parse the original DLL's normal IMAGE_IMPORT_DESCRIPTOR metadata.
// This lets us NAME the imported APIs referenced by indirect CALLs
// in the readable .text startup code, without assuming that every
// internal function is specific to the SafeDisc protection.
//
// PE32 import descriptor entries: OriginalFirstThunk at +0,
// DLL Name RVA at +12, FirstThunk (IAT) at +16. Lookup DWORD:
// bit31 = ordinal, otherwise RVA of IMAGE_IMPORT_BY_NAME,
// whose ASCII function name begins at +2 after the 16-bit hint.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DllImportBinding {
    iat_rva: u32,
    library: String,
    symbol: String,
}
fn dll_pe_import_string(bytes: &[u8], pe: &Pe32, rva: u32) -> Result<String> {
    const MAX_CHARS: usize = 127;
    let mut value = String::new();
    for i in 0..=MAX_CHARS {
        let at_rva = rva.checked_add(i as u32).ok_or("DLL import string RVA overflow")?;
        let at = dll_backed_rva_offset(pe, at_rva, 1)?;
        let byte = *bytes.get(at).ok_or("DLL import string outside input file")?;
        if byte == 0 {
            if value.is_empty() { return Err("empty DLL import string".into()); }
            return Ok(value);
        }
        if !byte.is_ascii_graphic() {
            return Err("non-ASCII-printable DLL import string".into());
        }
        value.push(char::from(byte));
    }
    Err("DLL import string exceeds 127-byte maximum".into())
}
fn dll_import_bindings(bytes: &[u8], pe: &Pe32) -> Result<Vec<DllImportBinding>> {
    let nt = read_u32(bytes, 0x3C)? as usize;
    let opt = nt.checked_add(24).ok_or("DLL optional header overflow")?;
    let opt_len = read_u16(bytes, nt + 20)? as usize;
    const IMPORT_DIR: usize = 96 + 1 * 8;
    if opt_len < IMPORT_DIR + 8 {
        return Err("DLL optional header too small for import directory".into());
    }
    let directory_rva = read_u32(bytes, opt + IMPORT_DIR)?;
    let directory_size = read_u32(bytes, opt + IMPORT_DIR + 4)?;
    if directory_rva == 0 && directory_size == 0 { return Ok(Vec::new()); }
    if directory_rva == 0 || directory_size < 40 || directory_size > 65536 {
        return Err("invalid DLL import directory RVA/size".into());
    }
    const MAX_DESCRIPTORS: usize = 128;
    const MAX_THUNKS: usize = 2048;
    let mut bindings = Vec::new();
    let mut found_terminator = false;
    for index in 0..MAX_DESCRIPTORS {
        let delta = index.checked_mul(20).ok_or("import descriptor offset overflow")?;
        if delta + 20 > directory_size as usize {
            return Err("DLL import directory terminator is outside declared size".into());
        }
        let at_rva = directory_rva.checked_add(delta as u32)
            .ok_or("DLL import descriptor RVA overflow")?;
        let at = dll_backed_rva_offset(pe, at_rva, 20)?;
        let original_thunk = read_u32(bytes, at)?;
        let name_rva = read_u32(bytes, at + 12)?;
        let first_thunk = read_u32(bytes, at + 16)?;
        if original_thunk == 0 && name_rva == 0 && first_thunk == 0 {
            found_terminator = true;
            break;
        }
        if name_rva == 0 || first_thunk == 0 {
            return Err("DLL import descriptor missing DLL name or FirstThunk".into());
        }
        let library = dll_pe_import_string(bytes, pe, name_rva)?;
        let lookup_rva = if original_thunk != 0 { original_thunk } else { first_thunk };
        let mut terminated = false;
        for n in 0..MAX_THUNKS {
            let inc = n.checked_mul(4).ok_or("import thunk overflow")? as u32;
            let lookup_entry_rva = lookup_rva.checked_add(inc)
                .ok_or("DLL original thunk RVA overflow")?;
            let iat_rva = first_thunk.checked_add(inc)
                .ok_or("DLL IAT thunk RVA overflow")?;
            let lookup_at = dll_backed_rva_offset(pe, lookup_entry_rva, 4)?;
            // Validate the target IAT slot exists on disk too. It is
            // used for address-bound functions at runtime, never called.
            dll_backed_rva_offset(pe, iat_rva, 4)?;
            let entry = read_u32(bytes, lookup_at)?;
            if entry == 0 {
                terminated = true;
                break;
            }
            let symbol = if entry & 0x8000_0000 != 0 {
                format!("#{}", entry & 0xFFFF)
            } else {
                let name_rva = entry.checked_add(2)
                    .ok_or("DLL import by-name RVA overflow")?;
                dll_pe_import_string(bytes, pe, name_rva)?
            };
            bindings.push(DllImportBinding {
                iat_rva, library: library.clone(), symbol,
            });
        }
        if !terminated { return Err("DLL import thunk list exceeds 2048 entries".into()); }
    }
    if !found_terminator {
        return Err("DLL import descriptor list lacks bounded terminator".into());
    }
    Ok(bindings)
}

// The original 0x9770 DLL_PROCESS_ATTACH handler has two branch
// instructions whose destinations converge. Validate their actual
// x86 opcode+rel32 bytes instead of trusting previously guessed RVAs.
// Only the two specifically identified forms (0F 84 and E9) are
// recognized; this is not a generic disassembler/control-flow graph.
fn dll_known_near_branch_target(
    dll: &[u8], pe: &Pe32, rva: u32, conditional: bool,
) -> Result<u32> {
    let len: usize = if conditional { 6 } else { 5 };
    let off = executable_rva_to_file_offset(pe, rva, len)?;
    if conditional {
        if dll.get(off..off+2) != Some(&b"\x0F\x84"[..]) {
            return Err(format!("expected 0F84 near JZ at 0x{rva:08X}"));
        }
    } else if dll.get(off).copied() != Some(0xE9) {
        return Err(format!("expected E9 near JMP at 0x{rva:08X}"));
    }
    let displacement_at = off + if conditional { 2 } else { 1 };
    let signed = read_u32(dll, displacement_at)? as i32 as i64;
    let dest = i64::from(rva) + len as i64 + signed;
    let dest = u32::try_from(dest)
        .map_err(|_| "near branch target outside x86 RVA range")?;
    executable_rva_to_file_offset(pe, dest, 32)?;
    Ok(dest)
}

// The user's verified original DllMain process-attach excerpt at RVA
// 0x9770..0x986F contains overlapping x86 instructions: e.g. reading
// from 0x9850 yields "2B FF ; 15 ...", but 0x9851 decodes a
// perfectly formed "FF 15 04 E0 8F 00" indirect CALL. NEVER assume
// these candidate CALL sites are dynamically reached from DllMain.
// Restrict this research flag to known on-disk opcodes and IAT metadata.
fn dll_indirect_import_call(
    dll: &[u8], pe: &Pe32, call_rva: u32, base: u32,
) -> Result<(u32, u32)> {
    let at = executable_rva_to_file_offset(pe, call_rva, 6)?;
    if dll.get(at..at+2) != Some(&b"\xFF\x15"[..]) {
        return Err(format!("expected FF15 indirect CALL at DLL RVA 0x{call_rva:08X}"));
    }
    let absolute_va = read_u32(dll, at+2)?;
    let iat_rva = absolute_va.checked_sub(base)
        .ok_or("indirect call VA below DLL preferred image base")?;
    Ok((absolute_va, iat_rva))
}

// Static x86 path for DLL_PROCESS_ATTACH=1. The CMP at 0x9787
// gives ZF=PF=1, CF=SF=OF=0. Only flag-preserving Jcc/JMP/NOPs
// occur before the first reachable call. Overlapping unvisited
// instructions are deliberately ignored. No DLL is executed.
fn dll_attach_first_call_path(dll:&[u8],pe:&Pe32)->Result<Vec<u32>>{
    let mut pc=0x9787u32;
    let mut trace=Vec::new();
    let mut seen=std::collections::HashSet::new();
    let (zf,cf,pf,sf,of)=(true,false,true,false,false);
    for _ in 0..32 {
        if !seen.insert(pc) {return Err("attach branch cycle".into())}
        trace.push(pc);
        let at=executable_rva_to_file_offset(pe,pc,1)?;
        let op=dll[at];
        if pc==0x9787 {
            if dll.get(at..at+4)!=Some(&[0x83,0x7D,0xFC,0x01][..]){
                return Err("unexpected original attach reason compare".into())
            }
            pc+=4;
        } else if op==0x90 {pc+=1;}
        else if op==0xE8 {
            if pc==0x97BE && dll_relative_call_target(dll,pe,pc)?==0xD62C {
                return Ok(trace);
            }
            return Err("unexpected first reachable attach call".into());
        } else if op==0x0F {
            let start=executable_rva_to_file_offset(pe,pc,6)?;
            if dll[start+1]!=0x8C {return Err("unsupported near attach Jcc".into())}
            let rel=i32::from_le_bytes(dll[start+2..start+6].try_into().unwrap());
            pc=if sf!=of {u32::try_from(i64::from(pc)+6+i64::from(rel))
                .map_err(|_|"near attach branch overflow")?} else {pc+6};
        } else if matches!(op,0xEB|0x70|0x71|0x74|0x76|0x77|0x7A|0x7B|0x7D){
            let start=executable_rva_to_file_offset(pe,pc,2)?;
            let taken=match op {
                0xEB=>true,0x70=>of,0x71=>!of,
                0x74=>zf,0x76=>cf||zf,0x77=>!cf&&!zf,
                0x7A=>pf,0x7B=>!pf,0x7D=>sf==of,_=>unreachable!(),
            };
            pc=if taken {u32::try_from(i64::from(pc)+2+i64::from(dll[start+1] as i8))
                .map_err(|_|"short attach branch overflow")?} else {pc+2};
        } else {return Err(format!("unsupported reached attach opcode 0x{op:02X} at 0x{pc:08X}"))}
        if !(0x9787..=0x97BE).contains(&pc) {
            return Err(format!("attach path escapes bound at 0x{pc:08X}"));
        }
    }
    Err("attach path exceeded 32 bounded instructions".into())
}

// Process-attach's first proved callee (0xD62C) begins with
// push ebp; mov ebp,esp; sub esp,8; push ebx; push esi; push edi.
// On a valid stack with enough room, SUB ESP,8 has ZF=CF=0.
// PF, SF and OF are enumerated, not assumed; each of the eight
// paths below must converge to the same first rel32 CALL.
//
// This is a conditional STATIC proof, not an executable emulator,
// real stack observation, or evidence of a .txt decoder.
fn dll_d62c_first_call_paths(dll:&[u8],pe:&Pe32)->Result<Vec<Vec<u32>>>{
    let start=executable_rva_to_file_offset(pe,0xD62C,9)?;
    if dll.get(start..start+9)!=Some(&[
        0x55,0x8B,0xEC,0x83,0xEC,0x08,0x53,0x56,0x57
    ][..]){
        return Err("original D62C stack prologue unexpectedly changed".into());
    }
    let mut all=Vec::new();
    for pf in [false,true]{
        for sf in [false,true]{
            for of in [false,true]{
                let (zf,cf)=(false,false);
                let mut pc=0xD635u32;
                let mut seen=std::collections::HashSet::new();
                let mut path=Vec::new();
                for _ in 0..32{
                    if !seen.insert(pc){return Err("D62C opaque predicate loop".into())}
                    path.push(pc);
                    let offset=executable_rva_to_file_offset(pe,pc,1)?;
                    let opcode=dll[offset];
                    if opcode==0xE8 {
                        if pc!=0xD664 || dll_relative_call_target(dll,pe,pc)?!=0xD7BE{
                            return Err("first reached D62C call differs from original".into());
                        }
                        break;
                    }
                    if opcode==0x90 || (opcode==0x50 && pc==0xD663){
                        pc+=1;
                    } else if opcode==0x87 {
                        let at=executable_rva_to_file_offset(pe,pc,2)?;
                        if dll[at+1]!=0xC0{return Err("unsupported reached D62C XCHG".into())}
                        pc+=2;
                    } else if opcode==0x0F{
                        let at=executable_rva_to_file_offset(pe,pc,6)?;
                        if dll[at+1]!=0x8C{return Err("unsupported D62C near Jcc".into())}
                        let rel=i32::from_le_bytes(dll[at+2..at+6].try_into().unwrap());
                        pc=if sf!=of {
                            u32::try_from(i64::from(pc)+6+i64::from(rel))
                                .map_err(|_|"D62C near Jcc overflow")?
                        }else{pc+6};
                    } else if matches!(opcode,0xEB|0x70|0x71|0x74|0x76|0x77|0x7A|0x7B|0x7D){
                        let at=executable_rva_to_file_offset(pe,pc,2)?;
                        let taken=match opcode{
                            0xEB=>true,0x70=>of,0x71=>!of,
                            0x74=>zf,0x76=>cf||zf,0x77=>!cf&&!zf,
                            0x7A=>pf,0x7B=>!pf,0x7D=>sf==of,_=>unreachable!()
                        };
                        pc=if taken{
                            u32::try_from(i64::from(pc)+2+i64::from(dll[at+1] as i8))
                                .map_err(|_|"D62C short Jcc overflow")?
                        }else{pc+2};
                    } else {
                        return Err(format!("unrecognized D62C opcode 0x{opcode:02X} at RVA 0x{pc:08X}"))
                    }
                    if !(0xD635..=0xD664).contains(&pc){
                        return Err(format!("D62C branch escaped bounded code at RVA 0x{pc:08X}"));
                    }
                }
                if path.last().copied()!=Some(0xD664){
                    return Err("D62C pre-call 32-instruction limit reached".into());
                }
                all.push(path);
            }
        }
    }
    Ok(all)
}

// 0xD7BE is a single-byte RET (0xC3), not an independent
// SafeDisc decoding routine. The CALL at D664 returns to D669.
// POP EAX / MOV word [ebp-8],1 / Jcc preserve EFLAGS. The valid
// stack assumption after SUB ESP,8 gives CF=ZF=0. Independently
// enumerate all PF/SF/OF states to prove that the next actual
// CALL is RVA D68C -> C5C0. Only EXACT reached opcode bytes
// are accepted, never scan overlapping dead x86 bytes as code.
fn dll_d62c_after_ret_paths(dll:&[u8],pe:&Pe32)->Result<Vec<Vec<u32>>>{
    fn expect(dll:&[u8],pe:&Pe32,pc:u32,code:&[u8])->Result<()>{
        let file=executable_rva_to_file_offset(pe,pc,code.len())?;
        if dll.get(file..file+code.len()) != Some(code) {
            return Err(format!("D62C post-RET bytes differ at RVA 0x{pc:08X}"));
        }
        Ok(())
    }
    // Before interpreting the indirect next-call path, verify that
    // the previously reached CALL target really does just RET.
    expect(dll,pe,0xD7BE,&[0xC3])?;
    if dll_relative_call_target(dll,pe,0xD664)?!=0xD7BE{
        return Err("D62C first E8 no longer invokes RET trampoline".into());
    }
    let mut outcomes=Vec::new();
    for pf in [false,true]{
        for sf in [false,true]{
            for _of in [false,true]{
                let mut pc=0xD669u32;
                let mut trace=Vec::new();
                for _ in 0..16{
                    trace.push(pc);
                    pc=match pc{
                        0xD669 => {expect(dll,pe,pc,&[0x58])?;0xD66A},
                        0xD66A => {expect(dll,pe,pc,&[0x77,0x02])?;0xD66E},
                        0xD66E => {expect(dll,pe,pc,&[0x66,0xC7,0x45,0xF8,0x01,0x00])?;0xD674},
                        0xD674 => {expect(dll,pe,pc,&[0x76,0x11])?;0xD676},
                        0xD676 => {expect(dll,pe,pc,&[0x77,0x0F])?;0xD687},
                        0xD687 => {expect(dll,pe,pc,&[0x78,0xEE])?;
                            if sf{0xD677}else{0xD689}},
                        0xD689 => {expect(dll,pe,pc,&[0x79,0xEC])?;0xD677},
                        0xD677 => {expect(dll,pe,pc,&[0x0F,0x8B,0x0F,0x00,0x00,0x00])?;
                            if !pf{0xD68C}else{0xD67D}},
                        0xD67D => {expect(dll,pe,pc,&[0x7A,0x0D])?;0xD68C},
                        0xD68C => {
                            if dll_relative_call_target(dll,pe,pc)?!=0xC5C0{
                                return Err("D62C second meaningful E8 does not reach C5C0".into());
                            }
                            break;
                        }
                        _ => return Err(format!("D62C post-RET path escaped at RVA 0x{pc:08X}")),
                    };
                }
                if trace.last().copied()!=Some(0xD68C){
                    return Err("D62C post-RET path failed to converge within 16 instructions".into());
                }
                outcomes.push(trace);
            }
        }
    }
    Ok(outcomes)
}

// Third protected attach tier: C5C0 uses the same kind of
// overlapping opaque predicates. After a conventional valid
// SUB ESP,8 (ZF=CF=0), enumerate all four SF/OF combinations;
// parity does not affect reached code. The first actual CALL
// is 0xC5DE -> 0xE25C. This neither executes nor decrypts.
fn dll_c5c0_first_call_paths(dll:&[u8],pe:&Pe32)->Result<Vec<Vec<u32>>>{
    fn check(dll:&[u8],pe:&Pe32,rva:u32,expected:&[u8])->Result<()>{
        let file=executable_rva_to_file_offset(pe,rva,expected.len())?;
        if dll.get(file..file+expected.len())!=Some(expected){
            return Err(format!("C5C0 unexpected opcode at RVA 0x{rva:08X}"));
        }
        Ok(())
    }
    check(dll,pe,0xC5C0,&[0x55,0x8B,0xEC,0x83,0xEC,0x08,0x53,0x56,0x57])?;
    let mut paths=Vec::new();
    for sf in [false,true]{
        for of in [false,true]{
            let mut trace=Vec::new();
            let mut pc=0xC5C9u32;
            for _ in 0..16{
                trace.push(pc);
                pc=match pc{
                    0xC5C9=>{
                        check(dll,pe,pc,&[0x7F,0x09])?;
                        if sf==of{0xC5D4}else{0xC5CB}
                    },
                    0xC5D4=>{
                        check(dll,pe,pc,&[0x77,0x00])?;
                        0xC5D6
                    },
                    0xC5D6=>{
                        check(dll,pe,pc,&[0x7F,0xF3])?;
                        if sf!=of{return Err("C5D6 JG predicate contradicts the path into C5D4".into())}
                        0xC5CB
                    },
                    0xC5CB=>{check(dll,pe,pc,&[0x90])?;0xC5CC},
                    0xC5CC=>{check(dll,pe,pc,&[0x87,0xED])?;0xC5CE},
                    0xC5CE=>{
                        check(dll,pe,pc,&[0x78,0x09])?;
                        if sf{0xC5D9}else{0xC5D0}
                    },
                    0xC5D0=>{check(dll,pe,pc,&[0x87,0xD2])?;0xC5D2},
                    0xC5D2=>{
                        check(dll,pe,pc,&[0x79,0x05])?;
                        if sf{return Err("C5D2 JNS reached when SF=1".into())}
                        0xC5D9
                    },
                    0xC5D9=>{
                        check(dll,pe,pc,&[0x68,0x08,0x05,0x90,0x00])?;
                        0xC5DE
                    },
                    0xC5DE=>{
                        if dll_relative_call_target(dll,pe,pc)?!=0xE25C{
                            return Err("C5C0 first CALL no longer targets E25C".into());
                        }
                        break;
                    },
                    _=>return Err(format!("unexpected C5C0 branch to RVA 0x{pc:08X}")),
                };
            }
            if trace.last().copied()!=Some(0xC5DE){
                return Err("C5C0 did not reach its first CALL within bounded steps".into());
            }
            paths.push(trace);
        }
    }
    Ok(paths)
}

// The third attach callee 0xE25C contains overlapping x86 branch
// instructions, but its first real work can be determined without
// executing DLL instructions. Its prologue preserves *incoming*
// x86 flags, which are not known: check every one of 32 possibilities.
//
// After an unconditional RET-decoy at 0xE2FE, all paths push
// immediate 128 and CALL the ordinary CRT allocation wrapper F850.
// The wrapper passes the request to F870 -> F8C0 (allocator).
// The result is stored into [ebp-4] and tested for NULL. If
// allocation succeeded, an E8 at E2DE calls DF96. These are
// conditional static properties, not cryptographic decoding.
#[derive(Clone,Copy)]
struct FiveFlags { zf:bool,cf:bool,pf:bool,sf:bool,of:bool }
fn dll_next_opaque_branch(dll:&[u8],pe:&Pe32,pc:u32,f:FiveFlags)->Result<Option<u32>>{
    let at=executable_rva_to_file_offset(pe,pc,1)?;
    let opcode=dll[at];
    if opcode==0x0F {
        let ins=executable_rva_to_file_offset(pe,pc,6)?;
        if dll[ins+1]!=0x8C{return Err("unexpected near Jcc opcode in bounded E25C path".into())}
        let disp=i32::from_le_bytes(dll[ins+2..ins+6].try_into().unwrap());
        let target=if f.sf!=f.of {i64::from(pc)+6+i64::from(disp)} else {i64::from(pc)+6};
        return Ok(Some(u32::try_from(target).map_err(|_|"E25C near branch overflow")?));
    }
    let take=match opcode {
        0xEB=>true,0x70=>f.of,0x71=>!f.of,
        0x73=>!f.cf,0x74=>f.zf,
        0x76=>f.cf||f.zf,0x77=>!f.cf&&!f.zf,
        0x78=>f.sf,0x79=>!f.sf,
        0x7A=>f.pf,0x7B=>!f.pf,
        0x7C=>f.sf!=f.of,0x7D=>f.sf==f.of,
        0x7F=>!f.zf&&f.sf==f.of,
        _=>return Ok(None),
    };
    let ins=executable_rva_to_file_offset(pe,pc,2)?;
    let rel=i64::from(dll[ins+1] as i8);
    let target=i64::from(pc)+2+if take{rel}else{0};
    Ok(Some(u32::try_from(target).map_err(|_|"E25C short branch overflow")?))
}
fn dll_e25c_allocator_paths(dll:&[u8],pe:&Pe32)->Result<(usize,usize,usize)>{
    fn exact(dll:&[u8],pe:&Pe32,pc:u32,bytes:&[u8])->Result<()>{
        let at=executable_rva_to_file_offset(pe,pc,bytes.len())?;
        if dll.get(at..at+bytes.len())!=Some(bytes){
            return Err(format!("E25C original bytes changed at RVA 0x{pc:08X}"));
        }
        Ok(())
    }
    exact(dll,pe,0xE25C,&[0x55,0x8B,0xEC,0x51,0x53,0x56,0x57])?;
    exact(dll,pe,0xE2FE,&[0xC3])?;
    exact(dll,pe,0xE298,&[0x68,0x80,0x00,0x00,0x00])?;
    exact(dll,pe,0xF850,&[0xA1,0x14,0x52,0x90,0x00,0x8B,0x4C,0x24,0x04,0x50,0x51])?;
    if dll_relative_call_target(dll,pe,0xF85B)?!=0xF870 {
        return Err("F850 allocator adapter no longer invokes F870".into());
    }
    if dll_relative_call_target(dll,pe,0xF88E)?!=0xF8C0 {
        return Err("F870 allocator no longer invokes F8C0".into());
    }
    // After the malloc(128) CALL, the stack is cleaned, its
    // return value is saved as the local pointer, then opaque
    // branch sequences reconverge on the NULL test.
    exact(dll,pe,0xE2A2,&[0x83,0xC4,0x04,0x89,0x45,0xFC])?;
    exact(dll,pe,0xE2CC,&[0x83,0x7D,0xFC,0x00,0x75,0x04])?;
    exact(dll,pe,0xE2D6,&[
        0x8B,0x45,0x08,0x50,0x8B,0x4D,0xFC,0x51,
    ])?;
    if dll_relative_call_target(dll,pe,0xE2DE)?!=0xDF96{
        return Err("E25C after allocation no longer invokes DF96".into());
    }
    let mut first_paths=std::collections::HashSet::new();
    let mut later_paths=std::collections::HashSet::new();
    let mut fake_call_count=0usize;
    for flags in 0..32u8 {
        let f=FiveFlags{
            zf: flags&1!=0,cf:flags&2!=0,pf:flags&4!=0,
            sf:flags&8!=0,of:flags&16!=0,
        };
        let mut pc=0xE263u32;
        let mut path=Vec::new();
        let mut saw_immediate=false;
        let mut saw_decoy=false;
        for _ in 0..48{
            path.push(pc);
            if pc==0xE29D {
                if !saw_immediate || dll_relative_call_target(dll,pe,pc)?!=0xF850 {
                    return Err("E25C first real call is not malloc(128)".into());
                }
                break;
            }
            if let Some(next)=dll_next_opaque_branch(dll,pe,pc,f)? { pc=next; }
            else {match pc {
                0xE26C|0xE26D|0xE285=>{exact(dll,pe,pc,&[0x90])?;pc+=1},
                0xE270=>{exact(dll,pe,pc,&[0x87,0xF6])?;pc+=2},
                0xE27F=>{exact(dll,pe,pc,&[0x87,0xFF])?;pc+=2},
                0xE283=>{exact(dll,pe,pc,&[0x87,0xC9])?;pc+=2},
                0xE28D=>{exact(dll,pe,pc,&[0x50])?;pc+=1},
                0xE28E=>{
                    if dll_relative_call_target(dll,pe,pc)?!=0xE2FE{
                        return Err("E25C decoy CALL no longer goes to one-byte RET".into());
                    }
                    saw_decoy=true;pc+=5
                },
                0xE293=>{exact(dll,pe,pc,&[0x58])?;pc+=1},
                0xE298=>{saw_immediate=true;pc+=5},
                _=>return Err(format!("unrecognized reachable E25C opcode at 0x{pc:08X}")),
            }}
            if !(0xE263..=0xE29D).contains(&pc){
                return Err("E25C allocation path escapes bounded first segment".into());
            }
        }
        if path.last().copied()!=Some(0xE29D){
            return Err("E25C did not reach malloc(128) within 48 bounded instructions".into());
        }
        if saw_decoy{fake_call_count+=1}
        first_paths.insert(path);
        // ADD ESP,4 changes EFLAGS, so the original flags before
        // F850 cannot be propagated: independently enumerate all 32
        // possible flags after this adjustment.
        let mut later=Vec::new();
        let mut pc=0xE2A8u32;
        for _ in 0..40{
            later.push(pc);
            if pc==0xE2CC {break}
            if let Some(next)=dll_next_opaque_branch(dll,pe,pc,f)?{pc=next;}
            else {
                match pc {
                    0xE2AA=>{exact(dll,pe,pc,&[0x8B,0xC9])?;pc+=2},
                    0xE2AE=>{exact(dll,pe,pc,&[0x90])?;pc+=1},
                    _=>return Err(format!("E25C post-alloc unknown reached byte at 0x{pc:08X}")),
                }
            }
            if !(0xE2A8..=0xE2CC).contains(&pc){
                return Err("E25C post-allocation path escaped bounded region".into());
            }
        }
        if later.last().copied()!=Some(0xE2CC){
            return Err("E25C post-allocation did not reach the NULL check".into());
        }
        later_paths.insert(later);
    }
    Ok((first_paths.len(),later_paths.len(),fake_call_count))
}

// Real byte-transforming primitive inside DF96's 128-byte
// initialization path: E11E CALL E205. The opaque predicates
// in E205 preserve pre-existing arbitrary flags, but all 32
// flag states converge on E228. At E228 the 32-bit inputs
// are masked with 0xFF, XORed, written to local AL and returned.
// XOR of masked bytes has high 24 bits zero, OF=SF=CF=0;
// the JL at E23E is then necessarily NOT taken.
// No DLL, driver, game or x86 instruction is ever executed.
fn dll_e205_xor_proof(dll:&[u8],pe:&Pe32)->Result<usize>{
    fn check(dll:&[u8],pe:&Pe32,rva:u32,bytes:&[u8])->Result<()>{
        let at=executable_rva_to_file_offset(pe,rva,bytes.len())?;
        if dll.get(at..at+bytes.len())!=Some(bytes){
            return Err(format!("E205 opcode bytes differ at RVA 0x{rva:08X}"));
        }
        Ok(())
    }
    check(dll,pe,0xE205,&[0x55,0x8B,0xEC,0x51,0x53,0x56,0x57])?;
    if dll_relative_call_target(dll,pe,0xE11E)?!=0xE205{
        return Err("DF96 no longer calls 8-bit XOR function E205".into());
    }
    // Validate exact arithmetic and return without treating
    // misleading overlapping bytes as reachable instructions.
    check(dll,pe,0xE228,&[
        0x8B,0x45,0x08,0x25,0xFF,0,0,0,
        0x8B,0x4D,0x0C,0x81,0xE1,0xFF,0,0,0,
        0x33,0xC1,0x88,0x45,0xFC,
    ])?;
    check(dll,pe,0xE23E,&[0x7C,0x06,0x90,0x90,0x77,0x00,0xEB,0x0C])?;
    check(dll,pe,0xE252,&[0x8A,0x45,0xFC,0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3])?;
    let mut paths=std::collections::HashSet::new();
    for state in 0..32u8{
        let f=FiveFlags{
            zf:state&1!=0,cf:state&2!=0,pf:state&4!=0,
            sf:state&8!=0,of:state&16!=0,
        };
        let mut pc=0xE20Cu32;
        let mut trace=Vec::new();
        for _ in 0..24{
            trace.push(pc);
            if pc==0xE228{break}
            // Opaque branch instructions all leave EFLAGS alone.
            pc=if let Some(next)=dll_next_opaque_branch(dll,pe,pc,f)?{
                next
            } else {
                match pc {
                    0xE20E=>{check(dll,pe,pc,&[0x8B,0xED])?;pc+2},
                    0xE212|0xE21A=>{check(dll,pe,pc,&[0x90])?;pc+1},
                    0xE21B=>{check(dll,pe,pc,&[0x87,0xDB])?;pc+2},
                    0xE21F=>{check(dll,pe,pc,&[0x87,0xD2])?;pc+2},
                    _=>return Err(format!("unrecognized E205 pre-XOR instruction at 0x{pc:08X}")),
                }
            };
            if !(0xE20C..=0xE228).contains(&pc){
                return Err(format!("E205 flag path escaped bounded region at 0x{pc:08X}"));
            }
        }
        if trace.last().copied()!=Some(0xE228){
            return Err("E205 8-bit XOR is not reached by every incoming flag state".into());
        }
        paths.insert(trace);
    }
    Ok(paths.len())
}
// Algebraic description of the ORIGINAL E228..E23B instructions;
// tested with synthetic data; not a general SafeDisc decode.
fn e205_byte_xor_math(first:u32,second:u32)->u8{
    ((first&0xFF) ^ (second&0xFF)) as u8
}

fn print_dll_attach_research(dll: &[u8]) -> Result<()> {
    const ATTACH_DIRECT_CALL_SITES: [u32; 4] = [
        0x97BE, 0x97EE, 0x9821, 0x9861,
    ];
    const ATTACH_INDIRECT_CALL_SITES: [u32; 2] = [
        0x9851, 0x986A,
    ];
    let pe = parse_pe32(dll)?;
    let nt = read_u32(dll, 0x3C)? as usize;
    let base = read_u32(dll, nt+24+28)?;
    let bindings = dll_import_bindings(dll, &pe)?;
    println!("Original SafeDisc DLL process-attach overlap research (STATIC ONLY):");
    let path=dll_attach_first_call_path(dll,&pe)?;
    println!("  Conditional PROCESS_ATTACH=1 path, preserving x86 CMP flags (ZF=PF=1, CF=SF=OF=0):");
    println!("    reached RVAs: {}",path.iter().map(|rva|format!("0x{rva:X}")).collect::<Vec<_>>().join(" -> "));
    println!("    First reached E8 CALL at RVA 0x97BE -> RVA 0xD62C (semantic flag proof, NOT runtime trace).");
    println!("    Function 0xD62C is NOT yet demonstrated to decrypt .txt.");
    let d62c=dll_d62c_first_call_paths(dll,&pe)?;
    println!("  First 0xD62C callee on valid-stack path: 0xD664 -> 0xD7BE.");
    println!("    PF/SF/OF cases enumerated: {}; all agree on first direct CALL.",d62c.len());
    println!("    This is conditional on normal SUB ESP,8 stack flags (CF=ZF=0), not a runtime trace.");
    println!("    No conclusion yet about either callee decrypting the protected .txt bytes.");
    let next=dll_d62c_after_ret_paths(dll,&pe)?;
    println!("  Correction: D7BE consists solely of opcode C3 (RET), not a decryption routine.");
    println!("  After its immediate return: D669 POP EAX; D66A JA; D66E MOV word [EBP-8],1; further flag-preserving Jcc.");
    println!("  Next CALL at D68C -> C5C0, verified for all {} PF/SF/OF cases.",next.len());
    println!("  C5C0 functionality remains unproven. No encrypted .txt bytes were transformed.");
    let c5c0=dll_c5c0_first_call_paths(dll,&pe)?;
    println!("  C5C0 first direct E8 call: RVA C5DE -> RVA E25C, for all {} SF/OF states.",c5c0.len());
    println!("  Original C5D9 PUSH uses constant preferred VA 0x00900508 (original .data state).");
    println!("  E25C also appears in earlier .text/.txt2 startup call research; its purpose remains under investigation.");
    let (first,after,decoys)=dll_e25c_allocator_paths(dll,&pe)?;
    println!("  Inside E25C: 32/32 possible entry-flag combinations reach E29D CALL F850 after PUSH 0x80.");
    println!("    distinct pre-allocation paths: {first}; RET-only fake CALL at E28E observed in {decoys}/32 cases.");
    println!("    F850 -> F870 -> F8C0 is the original CRT allocation call chain.");
    println!("    After saving allocation pointer [EBP-4], {} distinct paths reach the NULL check E2CC.",after);
    println!("    If non-NULL, E2DE CALL DF96 receives pointer and original caller argument.");
    println!("    Neither the allocator nor DF96 is yet a proven protected-section decryptor.");
    let xor_paths=dll_e205_xor_proof(dll,&pe)?;
    println!("  DF96 calls E205 at RVA E11E; E205 has {} distinct flag-preserving paths, all reaching a masked 8-bit XOR.",xor_paths);
    println!("  E205 arithmetic: AL = ((argument1 & 0xFF) XOR (argument2 & 0xFF)), returns AL.");
    println!("  This is a verified byte-transforming helper within the 128-byte initialization path.");
    println!("  It does not establish decryption of the DLL .txt section.");
    println!("  DLL preferred base: 0x{base:08X}; section and IAT RVAs are checked against its exact original PE.");
    println!("  RVA 0x9850: 2B FF 15 ... has distinct decode starts at 0x9850 and 0x9851.");
    println!("  The 0x9851 interpretation is an FF15 indirect CALL, not a proven runtime branch.");
    for site in ATTACH_DIRECT_CALL_SITES {
        let target = dll_relative_call_target(dll, &pe, site)?;
        let section = pe.sections.iter().find(|sec| {
            sec.virtual_address <= target && target - sec.virtual_address <
                (if sec.virtual_size == 0 { sec.raw_len as u32 }
                 else { sec.virtual_size.min(sec.raw_len as u32) })
        }).ok_or("direct CALL target not in a backed PE section")?;
        let section_name = String::from_utf8_lossy(
            section.name.split(|&b| b == 0).next().unwrap_or(&[])
        );
        println!("  Candidate E8 at RVA 0x{site:08X}: target 0x{target:08X} in {section_name}");
        print_dll_code_window(dll, &pe, "candidate direct CALL target raw bytes", target, 128)?;
    }
    for site in ATTACH_INDIRECT_CALL_SITES {
        let (va, rva) = dll_indirect_import_call(dll, &pe, site, base)?;
        if let Some(import) = bindings.iter().find(|i| i.iat_rva == rva) {
            println!("  Candidate FF15 at RVA 0x{site:08X}: IAT VA 0x{va:08X} -> {}!{}",
                import.library, import.symbol);
        } else {
            println!("  Candidate FF15 at RVA 0x{site:08X}: IAT VA 0x{va:08X}, no bound original PE import entry");
        }
    }
    // The .txt2 candidate has cross-section static callsites shared
    // with the original .text process-attach region. For the exact
    // hash-verified original PE, assert the literal opcode boundaries
    // and decode the rel32 targets rather than hard-coding outcomes.
    // A byte sequence on an overlapping x86 path is a candidate
    // instruction, not proof the instruction is ever reached.
    println!("  Cross-section .txt2 candidate call chain (STATIC; no reachability claim):");
    for (site, expected_target, description) in [
        (0x1A247u32, 0xE25Cu32, ".txt2 -> shared .text routine"),
        (0x1A25Fu32, 0xA550u32, ".txt2 -> shared .text struct helper"),
    ] {
        let target = dll_relative_call_target(dll, &pe, site)?;
        if target != expected_target {
            return Err(format!("unexpected title-specific .txt2 CALL target at 0x{site:08X}"));
        }
        println!("    E8 at RVA 0x{site:08X} -> 0x{target:08X} ({description})");
    }
    for (site, role) in [
        (0x1A23Au32, ".txt2 process-handle API candidate"),
        (0x1A250u32, ".txt2 module-handle API candidate"),
        (0xA578u32, ".text file-position API"),
        (0xA597u32, ".text file-read API"),
    ] {
        let (va, rva) = dll_indirect_import_call(dll, &pe, site, base)?;
        match bindings.iter().find(|i| i.iat_rva == rva) {
            Some(b) => println!("    FF15 at RVA 0x{site:08X} -> IAT VA 0x{va:08X}: {}!{} ({role})",
                b.library, b.symbol),
            None => println!("    FF15 at RVA 0x{site:08X} -> IAT VA 0x{va:08X}: unresolved original PE IAT ({role})"),
        }
    }
    // Full static scan of original file-backed .text/.txt2 E8 bytes
    // identifies five direct CALL candidates into high-entropy .txt.
    // Unlike PE relocation-backed VA pointers, rel32 does NOT require
    // a base relocation and thus was invisible to --xref-research.
    // Match only the five specific sites independently recovered by
    // inspection and assert the actual original 5-byte rel32 encoding
    // and target. Overlapping / opaque x86 paths remain unresolved.
    let protected = pe.section(b".txt")?;
    let protected_end = protected.virtual_address.checked_add(
        protected.virtual_size.min(protected.raw_len as u32)
    ).ok_or("protected code range overflow")?;
    println!("  Verified relative CALL encodings into on-disk encrypted .txt (STATIC CANDIDATES):");
    for (site, expected_target, source) in [
        (0x9197u32, 0x3B70u32, ".text"),
        (0x989Fu32, 0x7212u32, ".text"),
        (0x98C5u32, 0x7C4Fu32, ".text"),
        (0x1ACF6u32, 0x8620u32, ".txt2"),
        (0x1AD4Fu32, 0x8480u32, ".txt2"),
    ] {
        let target = dll_relative_call_target(dll, &pe, site)?;
        if target != expected_target
            || target < protected.virtual_address || target >= protected_end {
            return Err(format!("original DLL .txt E8 candidate mismatch at RVA 0x{site:08X}"));
        }
        println!("    {source} E8 at 0x{site:08X} -> encrypted .txt RVA 0x{target:08X}");
    }
    println!("  Relative E8 cross-section calls are not PE HIGHLOW relocation references.");
    println!("  Original encrypted .txt target bytes are NOT valid proof of decoded runtime code.");
    // This small deterministic helper is important when deciding
    // whether the helper CALL at 0x1A25F transforms any ciphertext.
    // On disk it simply copies argument 2 into [argument 1 + 8].
    let small_at = executable_rva_to_file_offset(&pe, 0xA550, 12)?;
    let expected: [u8;12] = [
        0x8B,0x4C,0x24,0x04, 0x8B,0x44,0x24,0x08,
        0x89,0x41,0x08, 0xC3,
    ];
    if dll.get(small_at..small_at+expected.len()) != Some(&expected[..]) {
        return Err("unexpected bytes in cross-section .text helper 0xA550".into());
    }
    println!("    RVA 0xA550 is a 12-byte helper: mov ecx,[esp+4]; mov eax,[esp+8]; mov [ecx+8],eax; ret");
    println!("    No TEA or .txt section decryption is established by this shared helper.");
    println!("  Direct or indirect CALL encodings alone do not establish reachability.");
    println!("  In particular, 0x1A20D is inside .txt2, not proof of .txt decoding.");
    println!("  No DLL was loaded or executed; no file was decrypted or modified.");
    Ok(())
}

// Build a bounded, statically validated next-stage call map from
// instructions actually visible in the user's exact original DLL.
// These RVAs are direct CALL instruction starts, not guessed functions.
// We never load, execute or change either the historical DLL or a game.
// Only recognize a *specific* two-byte x86 MOV r32,[absolute VA]
// instruction (8B /r with disp32 absolute), and bind it to an
// original DLL import. Do not treat random embedded DWORDs as code.
fn dll_absolute_import_load(
    dll:&[u8],pe:&Pe32,site:u32,op:[u8;2],base:u32
)->Result<u32>{
    let at=executable_rva_to_file_offset(pe,site,6)?;
    if dll.get(at..at+2)!=Some(&op){
        return Err(format!("expected exact x86 absolute import load opcode at RVA 0x{site:08X}"));
    }
    let va=read_u32(dll,at+2)?;
    va.checked_sub(base).ok_or("absolute import pointer lies below preferred DLL ImageBase".into())
}

fn print_dll_startup_calls(dll: &[u8]) -> Result<()> {
    const CALLS: [(u32, &str); 10] = [
        (0xFD81, "DllMain startup wrapper"),
        (0xFC18, "initialization helper A"),
        (0xFC53, "initialization helper B"),
        (0xFC5C, "initialization cleanup"),
        (0xFC71, "startup routine C"),
        (0xFC89, "startup routine D"),
        (0xFC8E, "startup routine E"),
        (0xFC93, "startup routine F"),
        (0xFC98, "startup routine G"),
        (0xFC9D, "startup routine H"),
    ];
    let pe = parse_pe32(dll)?;
    let nt = read_u32(dll, 0x3C)? as usize;
    let base = read_u32(dll, nt + 24 + 28)?;
    println!("SafeDisc 1.50.20 DLL startup call research (no disassembly or execution):");
    println!("  Preferred DLL image base 0x{base:08X}, DLL entrypoint RVA 0x{:08X}", pe.entry_rva);
    println!("  TLS directory: {}", if dll_tls_metadata(dll, &pe)?.is_some() {
        "present (see --modifier-research for callback details)"
    } else { "absent" });
    let bindings = dll_import_bindings(dll, &pe)?;
    println!("  Verified PE import bindings: {}", bindings.len());
    // Distinguish the *ordinary* CRT, heap, TLS and environment setup
    // from the obfuscated process-attach handler. All instruction
    // boundaries below are anchored to the exact known SHA-256 DLL,
    // not inferred by a blind linear disassembler.
    println!("  Verified ordinary startup-chain semantics (original Windows import bindings):");
    for (callsite,dest,description) in [
        (0xFC18u32,0x116E0u32,"heap initialization"),
        (0xFC53,0x10820,"TLS setup"),
        (0xFC71,0x12FF0,"CRT environment strings"),
        (0xFD81,0x9770,"separate obfuscated DllMain handler"),
    ] {
        let actual=dll_relative_call_target(dll,&pe,callsite)?;
        if actual!=dest {
            return Err(format!("unexpected original startup E8 target at RVA 0x{callsite:08X}"));
        }
        println!("    E8 0x{callsite:08X} -> 0x{actual:08X} ({description})");
    }
    for (site,expected) in [
        (0x116E9u32,"HeapCreate"),
        (0x10826u32,"TlsAlloc"),
    ] {
        let (_,iat_rva)=dll_indirect_import_call(dll,&pe,site,base)?;
        let binding=bindings.iter().find(|b|b.iat_rva==iat_rva)
            .ok_or("startup FF15 IAT pointer not present in original DLL imports")?;
        if binding.symbol!=expected || !binding.library.eq_ignore_ascii_case("KERNEL32.dll"){
            return Err(format!("unexpected Windows API at exact original startup FF15 RVA 0x{site:08X}"));
        }
        println!("    FF15 RVA 0x{site:08X}: {}!{} (verified)",binding.library,binding.symbol);
    }
    for (site,op,expected) in [
        (0x12FF7u32,[0x8Bu8,0x2Du8],"GetEnvironmentStrings"),
        (0x13003u32,[0x8Bu8,0x3Du8],"GetEnvironmentStringsW"),
        (0x119C1u32,[0x8Bu8,0x3Du8],"VirtualAlloc"),
    ]{
        let iat_rva=dll_absolute_import_load(dll,&pe,site,op,base)?;
        let binding=bindings.iter().find(|b|b.iat_rva==iat_rva)
            .ok_or("startup MOV absolute IAT pointer not present in original DLL imports")?;
        if binding.symbol!=expected || !binding.library.eq_ignore_ascii_case("KERNEL32.dll"){
            return Err(format!("unexpected Windows API at original startup import MOV RVA 0x{site:08X}"));
        }
        println!("    MOV absolute import RVA 0x{site:08X}: {}!{} (verified)",binding.library,binding.symbol);
    }
    println!("  The observed VirtualAlloc load is within the conventional heap allocator at RVA 0x11990.");
    println!("  It is NOT independent proof of .txt decryption; the separate 0x9770 handler remains unresolved.");

    // These IAT VAs are actually referenced by the user-provided
    // 0xFC00 / startup helper code. Name only recognized entries.
    for va in [
        0x008F_E020u32, 0x008F_E0A0, 0x008F_E0A4,
        0x008F_E090, 0x008F_E08C, 0x008F_E054,
        0x008F_E078, 0x008F_E074,
        0x008F_E0E0, 0x008F_E0E4,
    ] {
        let rva = va.checked_sub(base).ok_or("DLL imported API VA below preferred image base")?;
        if let Some(b) = bindings.iter().find(|b| b.iat_rva == rva) {
            println!("  IAT VA 0x{va:08X} -> {}!{}", b.library, b.symbol);
        } else {
            println!("  IAT VA 0x{va:08X}: no named import found in original PE import table");
        }
    }
    // All three paths below are backed by known x86 branch opcode
    // boundaries supplied by the user's exact verified original.
    // For reasons other than process attach, two branches leave the
    // obfuscated region and reach a common tail; process attach
    // enters the suspicious code at 0x9792 instead.
    let zero_reason = dll_known_near_branch_target(dll, &pe, 0x9781, true)?;
    let non_attach = dll_known_near_branch_target(dll, &pe, 0x978D, false)?;
    if zero_reason != non_attach {
        return Err("DLL initialization non-attach branches unexpectedly diverge".into());
    }
    println!("  DLL initialization branch map (verified x86 opcodes, not executed):");
    println!("    0x9781 JZ  -> 0x{zero_reason:08X} (reason == 0)");
    println!("    0x978D JMP -> 0x{non_attach:08X} (reason not 0 or 1)");
    println!("    process-attach reason == 1: falls through into irregular code near 0x9792");
    print_dll_code_window(dll, &pe, "DllMain process attach region (contains obfuscation)", 0x9770, 256)?;
    print_dll_code_window(dll, &pe, "shared non-attach branch target", non_attach, 192)?;
    let mut relevant = 0usize;
    for binding in &bindings {
        let name = binding.symbol.to_ascii_lowercase();
        if [
            "virtualprotect", "virtualalloc", "virtualquery",
            "getprocaddress", "loadlibrary", "getmodulehandle",
            "flushinstructioncache", "writeprocessmemory",
            "readprocessmemory", "ntprotectvirtualmemory",
        ].iter().any(|pattern| name.contains(pattern)) {
            println!("  Potential code-loading/memory-protection import: IAT RVA 0x{:08X} {}!{}",
                binding.iat_rva, binding.library, binding.symbol);
            relevant += 1;
        }
    }
    if relevant == 0 {
        println!("  No direct imports matching common code-loading/protection APIs; dynamic resolution and internal calls remain possible.");
    }
    for va in [0x0090_519Cu32, 0x0090_6C70u32] {
        let rva = va.checked_sub(base).ok_or("startup data VA below preferred image base")?;
        if let Some(section) = dll_zero_filled_section(&pe, rva, 4) {
            let name = String::from_utf8_lossy(
                section.name.split(|&b| b == 0).next().unwrap_or(&[]));
            println!("  Data VA 0x{va:08X} -> RVA 0x{rva:08X}: section {name}, zero-filled virtual memory (not backed on disk)");
        } else if let Ok(at) = dll_backed_rva_offset(&pe, rva, 4) {
            println!("  Data VA 0x{va:08X} -> RVA 0x{rva:08X}: file +0x{at:06X} DWORD 0x{:08X}",
                read_u32(dll, at)?);
        } else {
            println!("  Data VA 0x{va:08X} -> RVA 0x{rva:08X}: not found in backed or zero-fill PE section");
        }
    }
    for (call_rva, purpose) in CALLS {
        let dest = dll_relative_call_target(dll, &pe, call_rva)?;
        println!("  CALL RVA 0x{call_rva:08X} -> 0x{dest:08X} ({purpose}; target only, no inferred semantics)");
        print_dll_code_window(dll, &pe, "raw target bytes", dest, 64)?;
    }
    println!("  Call target RVAs come from E8 signed rel32 instructions in the exact hash-verified file.");
    println!("  Code appearance does not establish that any call decrypts protected .txt or .txt2.");
    Ok(())
}

// These addresses identify where *loaded* code is expected by
// historical analysis. Their on-disk bytes need NOT be decoded x86:
// SafeDisc DLL initialization may transform its executable section.
// Therefore the excerpts below are intentionally labelled raw bytes,
// not validated callable functions.
// The two SafeDisc 1.50.20 EAX modifier routine RVAs are documented in
// ArthaXerXes' June 2001 version analysis and in historical unSafeDisc
// 1.5.5's call sequence at 0x401F62. These are RVA offsets within the
// exact hash-verified game-specific dplayerx.dll, NOT portable mixers.
// 0x6621 = 0x65BE + 0x63. Show only two small code windows suitable for
// independent static x86 disassembly; do not execute the protected DLL.
// The on-disk .txt bytes have very high entropy, but the real
// PE32 HIGHLOW slots remain usable as ordinary absolute VAs.
// Count only file-backed 4-byte slots and classify their values.
// The result is a structural property, NOT decoded x86 or a
// runnable implementation of either EAX modifier function.
#[derive(Debug, Default, PartialEq, Eq)]
struct ProtectedDllRelocAudit {
    slots: usize,
    targets_data: usize,
    targets_rdata: usize,
    targets_txt: usize,
    targets_other: usize,
    out_of_image: usize,
    page_crossings: usize,
}
fn audit_protected_dll_relocations(
    dll: &[u8], pe: &Pe32,
    relocations: &[RelocBlock], base: u32,
) -> Result<ProtectedDllRelocAudit> {
    let protected=pe.section(b".txt")?;
    let end=protected.virtual_address.checked_add(
        protected.virtual_size.min(protected.raw_len as u32))
        .ok_or("protected .txt extent overflow")?;
    let mut out=ProtectedDllRelocAudit::default();
    for block in relocations {
        for &offset in &block.highlow_offsets {
            let source=block.page_rva.checked_add(offset as u32)
                .ok_or("protected DLL HIGHLOW RVA overflow")?;
            if source<protected.virtual_address || source>=end {
                continue;
            }
            let backed_end=source.checked_add(4)
                .ok_or("protected DLL HIGHLOW extent overflow")?;
            if backed_end>end {
                return Err("relocation slot straddles the raw backed .txt extent".into());
            }
            let file=dll_backed_rva_offset(pe,source,4)?;
            let va=read_u32(dll,file)?;
            out.slots+=1;
            if offset>0xFFC {out.page_crossings+=1;}
            let Some(target_rva)=va.checked_sub(base) else {
                out.out_of_image+=1;continue;
            };
            let name=pe.sections.iter().find(|sec|{
                target_rva.checked_sub(sec.virtual_address)
                    .is_some_and(|local|local<sec.virtual_size.max(sec.raw_len as u32))
            }).map(|sec|sec.name.split(|&b|b==0).next().unwrap_or(&[]));
            match name {
                Some(b".data")=>out.targets_data+=1,
                Some(b".rdata")=>out.targets_rdata+=1,
                Some(b".txt")=>out.targets_txt+=1,
                Some(_)=>out.targets_other+=1,
                None=>out.out_of_image+=1,
            }
        }
    }
    Ok(out)
}

// Bounded, exact-byte confirmation of a likely on-the-fly
// protected-code writer. The decrypted output region is not
// identified here: source bytes, actual arguments and runtime
// reachability must be recovered before claiming .txt decoding.
fn dll_expect_raw_x86(dll:&[u8],pe:&Pe32,rva:u32,bytes:&[u8])->Result<()> {
    if bytes.is_empty() || bytes.len()>256{
        return Err("invalid DLL exact instruction pattern length".into());
    }
    let offset=executable_rva_to_file_offset(pe,rva,bytes.len())?;
    if dll.get(offset..offset+bytes.len())!=Some(bytes){
        return Err(format!("expected original DLL instruction bytes at RVA 0x{rva:08X}"));
    }
    Ok(())
}
// The native byte-transforming chunk routine at 0x92B0
// deliberately hides two small constants behind 32-bit
// arithmetic instead of storing literal 0x43 and 0x15.
// Decode the MOV immediates from the checked original x86
// instructions. No encrypted .txt data is read or written.
fn dll_original_chunk_constants(dll:&[u8],pe:&Pe32)->Result<(u8,u8)>{
    fn imm(dll:&[u8],pe:&Pe32,at:u32,opcode:u8)->Result<u32>{
        let file=executable_rva_to_file_offset(pe,at,5)?;
        if dll[file]!=opcode{
            return Err(format!("unexpected original MOV opcode at RVA 0x{at:08X}"));
        }
        read_u32(dll,file+1)
    }
    let xor_a=imm(dll,pe,0x92F6,0xB8)?;
    let xor_b=imm(dll,pe,0x92FB,0xBB)?;
    let sub_a=imm(dll,pe,0x9309,0xBB)?;
    let sub_b=imm(dll,pe,0x9313,0xB9)?;
    for (rva,bytes) in [
        (0x92B3u32,&[0xB8,0x2C,0x10,0x00,0x00][..]),
        (0x9302,&[0x33,0xC3][..]),
        (0x9305,&[0x89,0x45,0xE8][..]),
        (0x931A,&[0x2B,0xD9][..]),
        (0x931C,&[0x8B,0xC3][..]),
        (0x931F,&[0x89,0x45,0xE0][..]),
    ]{
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
    }
    let first=xor_a ^ xor_b;
    let second=sub_a.wrapping_sub(sub_b);
    if first!=0x43 || second!=0x15{
        return Err("original chunk-mixer scrambled constants do not decode to 0x43,0x15".into());
    }
    Ok((first as u8,second as u8))
}

// Checked against *instruction-aligned* bytes in the original DLL, not a
// linear decode across the intentionally overlapping opaque-branch decoys.
// The initial byte/state and the per-index recurrence are kept separate.
fn dll_chunk_byte_recurrence_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    for (rva,code) in [
        (0x9478u32,&[0x8B,0x8D,0xD8,0xEF,0xFF,0xFF,0x81,0xE1,0xFF,0,0,0,0x8B,0x55,0xE8,0x81,0xE2,0xFF,0,0,0,0x33,0xCA,0x88,0x8D,0xD8,0xEF,0xFF,0xFF][..]),
        (0x94B1,&[0x8B,0x85,0xD8,0xEF,0xFF,0xFF,0x25,0xFF,0,0,0,0x8B,0x4D,0xE0,0x81,0xE1,0xFF,0,0,0,0x33,0xC1,0x88,0x45,0xE4][..]),
        (0x94EA,&[0xC7,0x45,0xF0,0x01,0,0,0][..]),
        (0x94F3,&[0x8B,0x55,0xF0,0x83,0xC2,0x01,0x89,0x55,0xF0][..]),
        (0x94FC,&[0x8B,0x45,0xF0,0x3B,0x45,0xD8,0x0F,0x83,0x9E,0,0,0][..]),
        (0x9508,&[0x8B,0x4D,0xF0,0x33,0xD2,0x8A,0x94,0x0D,0xD8,0xEF,0xFF,0xFF][..]),
        (0x9514,&[0x8B,0x45,0xF0,0x33,0xC9,0x8A,0x8C,0x05,0xD7,0xEF,0xFF,0xFF][..]),
        (0x9520,&[0x33,0xD1,0x8B,0x45,0xF0,0x88,0x94,0x05,0xD8,0xEF,0xFF,0xFF][..]),
        (0x9549,&[0x8B,0x4D,0xF0,0x33,0xD2,0x8A,0x94,0x0D,0xD8,0xEF,0xFF,0xFF][..]),
        (0x9555,&[0x8B,0x45,0xE4,0x25,0xFF,0,0,0,0x03,0xD0][..]),
        (0x955F,&[0x8B,0x4D,0xF0,0x88,0x94,0x0D,0xD8,0xEF,0xFF,0xFF][..]),
        (0x9581,&[0x8B,0x55,0xF0,0x8A,0x45,0xE4,0x02,0x84,0x15,0xD8,0xEF,0xFF,0xFF,0x88,0x45,0xE4][..]),
    ] { dll_expect_raw_x86(dll,pe,rva,code)?; }
    Ok(())
}

// A standalone bytewise model for synthetic tests, NOT a verified full
// protected-image decoder. Each call models a separately initialized chunk.
fn synthetic_chunk_byte_model(input:&[u8], first:u8, second:u8)->Vec<u8> {
    let mut buf=input.to_vec();
    if buf.is_empty(){return buf;}
    buf[0]^=first;
    let mut state=buf[0]^second;
    for i in 1..buf.len(){
        buf[i]=(buf[i]^buf[i-1]).wrapping_add(state);
        state=state.wrapping_add(buf[i]);
    }
    buf
}

// Inverse of the provisional per-chunk recurrence, applied ONLY to
// generated synthetic vectors. This cannot validate whether SafeDisc
// actually supplies this recurrence with the presumed inputs/states.
fn synthetic_chunk_byte_inverse(output:&[u8],first:u8,second:u8)->Vec<u8> {
    let mut input=output.to_vec();
    if input.is_empty(){return input;}
    let mut state=output[0]^second;
    input[0]=output[0]^first;
    for i in 1..output.len() {
        input[i]=output[i].wrapping_sub(state)^output[i-1];
        state=state.wrapping_add(output[i]);
    }
    input
}

// Pure in-memory statistical diagnostic; it never emits private bytes.
// Zero entropy denotes a constant/empty byte stream; maximum is 8 bits.
fn synthetic_byte_entropy(data:&[u8])->f64 {
    if data.is_empty(){return 0.0;}
    let mut counts=[0usize;256];
    for &b in data {counts[b as usize]+=1;}
    let n=data.len() as f64;
    counts.iter().filter(|&&c|c!=0).map(|&c|{
        let p=c as f64/n;
        -p*p.log2()
    }).sum()
}

// These isolated comparators model exact unsigned x86 arithmetic. They are
// only used with synthetic vectors, not to modify any protected binary.
fn synthetic_probe_before_start(start:u32,probe:u32)->bool { probe < start }
fn synthetic_adjacent_ranges(a:(u32,u32,u32),b:(u32,u32,u32))->bool {
    a.2==b.2 &&
    (a.0.wrapping_add(a.1)==b.0 || b.0.wrapping_add(b.1)==a.0)
}

// The index is a raw 32-bit caller argument in the original x86. This
// synthetic guard states the legal geometry, NOT a runtime x86 bounds check.
// Synthetic, isolated reproduction of the observed C350 unsigned 32-bit
// comparisons. The caller's input validity constraints are not known.
fn synthetic_extend_matching_range(
    stored:&mut (u32,u32,u32), address:u32, size:u32, tag:u32,
)->bool {
    let start=stored.0;
    let exclusive_end=start.wrapping_add(stored.1);
    if start>address || address>=exclusive_end {return false;}
    let requested_end=address.wrapping_add(size);
    let last=requested_end.wrapping_sub(1);
    if last<start || last>=exclusive_end {
        if stored.2!=tag {return false;}
        stored.1=requested_end.wrapping_sub(start);
        return true;
    }
    stored.2==tag
}

fn synthetic_range_category_field(index:u32)->Option<u32> {
    if index<8 { Some(0x14+4*index) } else { None }
}

fn original_category_dispatch_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    dll_expect_raw_x86(dll,pe,0xA2BE,&[0x8B,0x74,0xB8,0x14])?;
    // PUSH imm8 is the fourth caller argument; the adjacent E8 is the
    // actual dispatcher call. The seven independently witnessed numeric
    // categories do not assign names or document every possible caller.
    for (push_site,call_site,category) in [
        (0xAAB3u32,0xAAB8u32,6u8),
        (0xAB43,0xAB48,5),
        (0xABD3,0xABD8,4),
        (0xAC28,0xAC2D,7),
        (0xAE27,0xAE2C,0),
        (0xB0B1,0xB0B7,2),
        (0xBFED,0xBFF2,3),
    ] {
        dll_expect_raw_x86(dll,pe,push_site,&[0x6A,category])?;
        if dll_relative_call_target(dll,pe,call_site)?!=0xA2A0 {
            return Err("category dispatch E8 target mismatch".into());
        }
        if synthetic_range_category_field(u32::from(category)).is_none(){
            return Err("observed category outside eight-record table".into());
        }
    }
    println!("  0xA2BE indexes one of eight category pointers using [record + 0x14 + category*4].");
    println!("  Original callers demonstrably pass numeric categories 0,2,3,4,5,6,7; no category meaning is inferred.");
    println!("  No bounds check at 0xA2BE has been established; synthetic tooling rejects indices >=8.");
    Ok(())
}

// Mirror the first-match CODE|EXECUTE selection on synthetic section flags.
fn synthetic_first_code_exec(flags:&[u32])->Option<usize> {
    flags.iter().position(|f| (*f&0x2000_0020)==0x2000_0020)
}

// Synthetic representation of the three scalar outputs of selector case 1.
// Do not confuse a PE raw file offset with an in-memory virtual RVA.
fn synthetic_selector_fields(raw:u32,rva:u32,vsize:u32,raw_size:u32)->(u32,u32,u32) {
    (raw,rva,if vsize==0 {raw_size} else {vsize.min(raw_size)})
}

// Reproduce *only* the section-characteristics predicate as observed in
// the selector. Its separate exclusion predicate (0x187B0) is not modeled.
fn synthetic_first_section_with_mask(flags:&[u32],mask:u32,required:u32)->Option<usize>{
    flags.iter().position(|f| (*f&mask)==required)
}

// Actual PE32 directory RVAs; a zero-valued entry is not a section RVA.
fn original_pe_directory_rvas(bytes:&[u8])->Result<[u32;16]> {
    let nt=read_u32(bytes,0x3C)? as usize;
    let optional_len=read_u16(bytes,nt+20)? as usize;
    if optional_len<96+16*8 {return Err("PE32 optional header lacks 16 complete data directories".into());}
    let start=nt.checked_add(24+96).ok_or("PE32 directory offset overflow")?;
    let mut out=[0u32;16];
    for (i,val) in out.iter_mut().enumerate() {
        *val=read_u32(bytes,start.checked_add(i*8).ok_or("directory slot offset overflow")?)?;
    }
    Ok(out)
}
fn synthetic_section_excluding_directories(
    sections:&[(u32,u32)],mask:u32,required:u32,directories:&[u32;16],
)->Option<usize> {
    sections.iter().position(|&(rva,flags)|
        (flags&mask)==required && !directories.contains(&rva)
    )
}

fn original_category_selector_relation(dll:&[u8],pe:&Pe32)->Result<()> {
    // 0xAA80, 0xAB10 and 0xABA0 enumerate actual section headers via
    // 0x18800, then register ranges in categories 6, 5 and 4.
    for (selector_site,selector,selection_call,category_site,category,register_call) in [
        (0xAA9Au32,10u8,0xAA9Du32,0xAAB3u32,6u8,0xAAB8u32),
        (0xAB2A,2,0xAB2D,0xAB43,5,0xAB48),
        (0xABBA,1,0xABBD,0xABD3,4,0xABD8),
    ] {
        dll_expect_raw_x86(dll,pe,selector_site,&[0x6A,selector])?;
        dll_expect_raw_x86(dll,pe,category_site,&[0x6A,category])?;
        if dll_relative_call_target(dll,pe,selection_call)?!=0x18800 ||
           dll_relative_call_target(dll,pe,register_call)?!=0xA2A0 {
            return Err("section-selector to category-list call mapping changed".into());
        }
    }
    for (site,bytes) in [
        (0x183C9u32,&[0x68,0x20,0,0,0x20,0x68,0x20,0,0,0x20][..]),
        (0x18514,&[0x68,0x40,0,0,0xC0,0x68,0x40,0,0,0xC0][..]),
        (0x18645,&[0x68,0x60,0,0,0xC0,0x68,0x40,0,0,0x40][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    for (site,target) in [(0x183D4u32,0x189F0u32),(0x1851F,0x189F0),(0x18650,0x189F0)] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err("section-characteristics filtering function call changed".into());
        }
    }
    let flags:Vec<u32>=pe.sections.iter().map(|section|section.characteristics).collect();
    let cases=[(4u8,0x2000_0020u32,0x2000_0020u32,b".txt".as_slice()),
               (5,0xC000_0040,0xC000_0040,b".data".as_slice()),
               (6,0xC000_0060,0x4000_0040,b".rdata".as_slice())];
    for (category,mask,required,expected_name) in cases {
        let idx=synthetic_first_section_with_mask(&flags,mask,required)
            .ok_or("no section matches original category's first-stage flags")?;
        let section=&pe.sections[idx];
        if section.name.split(|&b|b==0).next()!=Some(expected_name){
            return Err("unexpected first section matching category's flags".into());
        }
        println!("  Category {category}: initial section-filter candidate {} (mask 0x{mask:08X}, required 0x{required:08X}).",String::from_utf8_lossy(expected_name));
    }
    println!("  Category 4 -> selector 1; 5 -> selector 2; 6 -> selector 10 (verified caller PUSH/E8 pairs).");
    println!("  Categories 5 and 6 pass through an extra 0x187B0 exclusion filter; candidate != proven final selection.");
    // The second filter is not an opaque heuristic. RVA 0x18770 reads
    // 0x80 bytes (16 IMAGE_DATA_DIRECTORY entries), and 0x187B0 compares
    // the section VirtualAddress to each directory.VirtualAddress.
    for (site,bytes) in [
        (0x18770u32,&[0x8B,0x54,0x24,0x04,0x81,0xEC,0x38,0x01,0x00,0x00][..]),
        (0x18787,&[0xE8,0xB4,0x01,0x00,0x00][..]),
        (0x18793,&[0xB9,0x20,0x00,0x00,0x00][..]),
        (0x18798,&[0x8D,0xB4,0x24,0xCC,0x00,0x00,0x00][..]),
        (0x187A2,&[0xF3,0xA5][..]),
        (0x187D1,&[0x8D,0x4C,0x24,0x00,0x3B,0x11,0x74,0x12][..]),
        (0x187DA,&[0x83,0xC1,0x08,0x83,0xF8,0x10,0x72,0xF3][..]),
        (0x18533,&[0xE8,0x78,0x02,0x00,0x00][..]),
        (0x18668,&[0xE8,0x43,0x01,0x00,0x00][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    for (site,target) in [(0x18787u32,0x18940u32),(0x187C0,0x18770),(0x18533,0x187B0),(0x18668,0x187B0)] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err("PE directory exclusion-helper CALL target mismatch".into());
        }
    }
    let directory_rvas=original_pe_directory_rvas(dll)?;
    if directory_rvas[5]!=0x27000 || directory_rvas[12]!=0x1E000 {
        return Err("unexpected DLL base-reloc / IAT directory RVA".into());
    }
    let mapped:Vec<(u32,u32)>=pe.sections.iter().map(|sec|
        (sec.virtual_address,sec.characteristics)
    ).collect();
    let data_idx=synthetic_section_excluding_directories(&mapped,0xC000_0040,0xC000_0040,&directory_rvas)
        .ok_or("writable .data should survive PE directory exclusion")?;
    if pe.sections[data_idx].name.split(|&c|c==0).next()!=Some(b".data".as_slice()) {
        return Err("category 5 no longer chooses DLL .data".into());
    }
    if synthetic_section_excluding_directories(&mapped,0xC000_0060,0x4000_0040,&directory_rvas).is_some() {
        return Err("original category 6 must skip IAT .rdata and relocation .reloc".into());
    }
    println!("  0x18770 copies 16 PE data-directory entries; 0x187B0 excludes sections whose RVA equals a directory RVA.");
    println!("  Category 5 keeps .data; category 6 skips .rdata (IAT directory) and .reloc (base reloc directory).");
    println!("  No section remains for category 6 of this verified original DLL; this does NOT imply category 6 is globally unused.");

    Ok(())
}

// Unlike the longer metadata strings decoded by unsafedisc_strings,
// this small original .txt2 identifier uses a chained (input-1) XOR.
// The read bound prevents accidental traversal beyond its NUL terminator.
fn decode_original_section_tag(data:&[u8])->Result<Vec<u8>> {
    let first=*data.first().ok_or("empty encoded section tag")?;
    if first==0 {return Err("empty encoded section name".into());}
    let mut out=vec![first];
    for &c in data.iter().skip(1).take(63) {
        if c==0 {return Ok(out);}
        let next=c.wrapping_sub(1)^out[out.len()-1];
        out.push(next);
    }
    Err("encoded section tag lacks bounded terminator".into())
}

// Model only the known flag-preserving opaque detour AFTER cursor+0x28.
// All eight CF/ZF/OF combinations are deliberately tested; PF/SF are not
// consulted here. This is a bounded control-flow check, not DLL execution.
fn synthetic_section_cursor_detour(cf:bool,zf:bool,of:bool)->Result<Vec<u32>> {
    let mut pc=0x9397u32;
    let mut path=Vec::new();
    for _ in 0..32 {
        if pc==0x93C0 {
            return Ok(path);
        }
        path.push(pc);
        pc=match pc {
            0x9397=>0x93A0,
            0x93A0=>0x9399,
            0x9399=>0x939B, // MOV EDX, EDX preserves flags
            0x939B=>if !cf {0x93A3} else {0x939D}, // JAE
            0x939D=>0x939E, // NOP
            0x939E=>if cf {0x93A3} else {return Err("unexpected JB".into())},
            0x93A3=>if of {0x93B0} else {0x93A5}, // JO
            0x93B0=>if of {0x93A5} else {return Err("unexpected JO".into())},
            0x93A5=>if !cf && !zf {0x93AE} else {0x93A7}, // JA
            0x93A7=>0x93A9, // XCHG ESI, ESI
            0x93A9=>if zf {0x93AE} else {0x93AB}, // JE
            0x93AB=>0x93AD, // XCHG EBX, EBX
            0x93AD=>0x93AE, // NOP
            0x93AE=>0x93B3, // JMP
            0x93B3=>if cf || zf {0x93C0} else {0x93B5}, // JBE
            0x93B5=>0x93B6, // PUSH EAX
            0x93B6=>0x93BB, // CALL 0x93FC (a single RET)
            0x93BB=>0x93BC, // POP EAX
            0x93BC=>if !cf && !zf {0x93C0} else {return Err("unexpected JA".into())},
            _=>return Err(format!("unexpected selector-detour RVA 0x{pc:X}")),
        };
    }
    Err("selector-detour budget exhausted".into())
}

// Synthetic model of what the real selector's three zero stores do at
// entry to EVERY lookup (0x18255, 0x1825C, 0x18266). Not a runtime emulator.
fn synthetic_exhausted_selector_outputs(earlier:[u32;3],matched:bool)->([u32;3],bool){
    let _=earlier;
    ([0,0,0],matched)
}

fn original_selector_exhaustion_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    // 0x18800 forwards three output pointers to 0x181E0. It zeroes
    // all three output DWORDs BEFORE any dispatch. A failed final
    // section lookup consequently clears caller [EBP-0x14] (extent).
    for (site,bytes) in [
        (0x18800u32,&[0x8B,0x4C,0x24,0x18,0x8B,0x54,0x24,0x14][..]),
        (0x18808,&[0x8D,0x44,0x24,0x18,0x50][..]),
        (0x18811,&[0x51,0x8B,0x4C,0x24,0x14][..]),
        (0x18816,&[0x52,0x8B,0x54,0x24,0x14][..]),
        (0x1881B,&[0x50,0x8B,0x44,0x24,0x14][..]),
        (0x18820,&[0x51,0x52,0x50][..]),
        (0x18239,&[0x8B,0x94,0x24,0x80,0x01,0x00,0x00][..]),
        (0x18247,&[0x8B,0xAC,0x24,0x84,0x01,0x00,0x00][..]),
        (0x1824E,&[0x8B,0xB4,0x24,0x88,0x01,0x00,0x00][..]),
        (0x18255,&[0x49,0xC7,0x02,0x00,0x00,0x00,0x00][..]),
        (0x1825C,&[0xC7,0x45,0x00,0x00,0x00,0x00,0x00][..]),
        (0x18266,&[0xC7,0x06,0x00,0x00,0x00,0x00][..]),
        (0x18500,&[0x8B,0xC6][..]), // return path for failed match
        (0x9347,&[0x74,0x7C][..]), // zero selector result -> 0x93C5
        (0x9403,&[0x83,0x7D,0xEC,0x00][..]), // cmp selected extent,0
        (0x9407,&[0x0F,0x84,0x20,0x02,0x00,0x00][..]), // zero -> 0x962D
        (0x962D,&[0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    if dll_relative_call_target(dll,pe,0x18823)?!=0x181E0 ||
       dll_known_near_branch_target(dll,pe,0x9407,true)?!=0x962D {
        return Err("selector exhaustive empty result / zero-sized exit changed".into());
    }
    for seen in [false,true] {
        let (output,flag)=synthetic_exhausted_selector_outputs(
            [0x400,0x18000,0x5497],seen);
        if output!=[0,0,0] || flag!=seen {
            return Err("synthetic exhausted selector did not clear outputs".into());
        }
    }
    println!("  0x18800 -> 0x181E0: every selector iteration clears all three output DWORDs first.");
    println!("  If the final call returns zero, selected extent [EBP-0x14] is also reset to zero.");
    println!("  0x9347 -> 0x93C5; on a set name-match flag, 0x9403/0x9407 skips all chunk copies when extent is zero.");
    println!("  On NO .txt2 match, ordinary selector exhaustion clears extent and bypasses copies.");
    println!("  On .txt2 match, 0x938C jumps to 0x93C5 BEFORE next selector call; nonzero current section extent is preserved.");
    // A minimal independent observational check: the selected original
    // .txt2 is *already* valid x86 machine code. If the first-byte mixer
    // runs and the later protected copy succeeds, it changes the initial
    // PUSH EBP (0x55) to 0x16. This strongly distinguishes self-scrambling
    // from restoration of an encrypted original .txt2 prologue.
    let section=pe.section(b".txt2")?;
    let first=dll.get(section.raw_offset).copied()
        .ok_or("original .txt2 first byte beyond file")?;
    if section.virtual_address!=0x18000 || first!=0x55 {
        return Err("unexpected original .txt2 prologue or section RVA".into());
    }
    if first^0x43!=0x16 {
        return Err("first-byte XOR regression".into());
    }
    println!("  Original .txt2 starts with valid PUSH EBP (0x55); candidate XOR 0x43 would change it to 0x16.");
    println!("  This suggests potential post-initialization self-scrambling, NOT verified decryption or observed runtime execution.");
    let backed=dll.get(section.raw_offset..section.raw_offset+4096)
        .ok_or("original .txt2 first backed 4KiB not available")?;
    let candidate=synthetic_chunk_byte_model(backed,0x43,0x15);
    let before=synthetic_byte_entropy(backed);
    let after=synthetic_byte_entropy(&candidate);
    if !(before<after && after>7.5){
        return Err("unexpected original first .txt2 chunk entropy trend".into());
    }
    println!("  First DLL .txt2 chunk entropy: original {before:.3} bits/byte, candidate transformed {after:.3} bits/byte.");
    println!("  Higher candidate entropy is consistent with a scrambling hypothesis, not evidence of runtime transformation.");


    println!("  Valid section match reaches the 0x9403 chunk path; actual transformed plaintext remains unvalidated.");
    Ok(())
}

fn dll_selector_detour_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    // Every reached instruction in the candidate detour is checked against
    // the exact hash-validated original on-disk x86 byte encoding.
    for (site,bytes) in [
        (0x9397u32,&[0xEB,0x07][..]),
        (0x9399,&[0x8B,0xD2][..]),
        (0x939B,&[0x73,0x06][..]),
        (0x939D,&[0x90][..]),
        (0x939E,&[0x72,0x03][..]),
        (0x93A0,&[0xEB,0xF7][..]),
        (0x93A3,&[0x70,0x0B][..]),
        (0x93A5,&[0x77,0x07][..]),
        (0x93A7,&[0x87,0xF6][..]),
        (0x93A9,&[0x74,0x03][..]),
        (0x93AB,&[0x87,0xDB][..]),
        (0x93AD,&[0x90][..]),
        (0x93AE,&[0xEB,0x03][..]),
        (0x93B0,&[0x70,0xF3][..]),
        (0x93B3,&[0x76,0x0B][..]),
        (0x93B5,&[0x50][..]),
        (0x93B6,&[0xE8,0x41,0x00,0x00,0x00][..]),
        (0x93BB,&[0x58][..]),
        (0x93BC,&[0x77,0x02][..]),
        (0x93BE,&[0xC3][..]), // intentionally unreachable false RET
        (0x93C0,&[0xE9,0x5D,0xFF,0xFF,0xFF][..]),
        (0x93FC,&[0xC3][..]), // no-effect CALL/RET detour
        (0x9343,&[0x83,0x7D,0xF4,0x00][..]), // selector-result check
        (0x9347,&[0x74,0x7C][..]), // absent -> final match-flag check
        (0x93C5,&[0x8B,0x45,0xFC,0x25,0xFF,0xFF,0x00,0x00,0x85,0xC0,0x75,0x32][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    if dll_relative_call_target(dll,pe,0x93B6)?!=0x93FC ||
       dll_known_near_branch_target(dll,pe,0x93C0,false)?!=0x9322 {
        return Err("original selector back-edge/RET-decoy target mismatch".into());
    }
    for cf in [false,true] {
        for zf in [false,true] {
            for of in [false,true] {
                let path=synthetic_section_cursor_detour(cf,zf,of)?;
                if path.is_empty() || path.contains(&0x93BE) ||
                   path.last()!=Some(&if cf || zf {0x93B3} else {0x93BC}) {
                    return Err("synthetic flag path reaches an unexpected return".into());
                }
            }
        }
    }
    println!("  0x9397..0x93C0: all eight CF/ZF/OF paths converge on the loop back-edge, no false RET.");
    println!("  0x9347: selector exhausted -> 0x93C5; matching .txt2 is a required flag for the chunk-loop gate.");
    println!("  Still unverified: exact persistence of selector output fields across the final exhausted iteration.");
    Ok(())
}

fn original_txt2_section_name_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    // Wrapper constructs a local decoded string and passes its address
    // as the third argument of 0x92B0. The section comparator reads 0x28
    // bytes from the candidate section header and compares that name.
    for (site,bytes) in [
        (0x9201u32,&[0x68,0x88,0x00,0x90,0x00,0x8D,0x45,0xF8,0x50][..]),
        (0x920Au32,&[0xE8,0xA1,0xF9,0x00,0x00][..]),
        (0x922Eu32,&[0x8D,0x4D,0xF8,0x51][..]),
        (0x9349u32,&[0x8B,0x55,0x14,0x52][..]),
        (0x9355u32,&[0xE8,0xA6,0xF7,0x00,0x00][..]),
        (0x935Du32,&[0x85,0xC0,0x74,0x2D][..]),
        (0x18BC5u32,&[0x8B,0x45,0x0C,0x03,0x45,0xFC][..]),
        (0x18BE6u32,&[0x83,0xE9,0x01][..]),
        (0x18BEFu32,&[0x33,0xC0,0x8A,0x42,0xFF,0x33,0xC8][..]),
        (0x18B11u32,&[0x50,0x6A,0x28][..]),
        (0x18B3Bu32,&[0x8A,0x10,0x8A,0x1E,0x8A,0xCA,0x3A,0xD3][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    for (site,target) in [
        (0x920Au32,0x18BB0u32),(0x9355,0x18B00),(0x18B16,0xA810),
    ] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err("original encoded section-name call target mismatch".into());
        }
    }
    let off=dll_backed_rva_offset(pe,0x20088,64)?;
    let tag=decode_original_section_tag(&dll[off..off+64])?;
    if tag!=b".txt2" {return Err("unexpected original decoded section tag".into());}
    println!("  0x18BB0 decodes original data RVA 0x20088 to the 5-byte section name '.txt2'.");
    println!("  0x922E passes that local tag as the third argument to 0x92B0.");
    println!("  0x18B00 reads a candidate 0x28-byte section header and compares its name with '.txt2'.");
    for (site,bytes) in [
        (0x9361u32,&[0x66,0xC7,0x45,0xFC,0x01,0x00][..]),
        (0x938E,&[0x8B,0x55,0xF4,0x83,0xC2,0x28,0x89,0x55,0xF4][..]),
        (0x93BC,&[0x77,0x02][..]),
        (0x93C0,&[0xE9,0x5D,0xFF,0xFF,0xFF][..]),
        (0x93C5,&[0x8B,0x45,0xFC,0x25,0xFF,0xFF,0x00,0x00,0x85,0xC0,0x75,0x32][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    if dll_known_near_branch_target(dll,pe,0x93C0,false)?!=0x9322 {
        return Err("section-header scan back-edge mismatch".into());
    }
    println!("  Comparison success sets [EBP-4]=1; another path increments the header cursor by 0x28.");
    println!("  A real branch at 0x93C0 returns to selector setup at 0x9322.");
    println!("  At 0x93C5 the nonzero match flag branches to 0x9403 (block transformation).");
    let nt=read_u32(dll,0x3C)? as usize;
    let opt_size=read_u16(dll,nt+20)? as usize;
    let section_headers=nt.checked_add(24).and_then(|v|v.checked_add(opt_size))
        .ok_or("original DLL section-table offset overflow")?;
    if section_headers!=0x1A8 {
        return Err("original DLL first section-header cursor is no longer 0x1A8".into());
    }
    for (index,name) in [(0usize,b".txt".as_slice()),(1,b".text".as_slice()),(2,b".txt2".as_slice())] {
        let off=section_headers.checked_add(index*0x28)
            .ok_or("original section-header cursor overflow")?;
        let candidate=dll.get(off..off+8)
            .ok_or("original header cursor outside DLL file")?
            .split(|&x|x==0).next().unwrap_or(&[]);
        if candidate!=name || pe.sections.get(index)
            .is_none_or(|sec|sec.name.split(|&x|x==0).next()!=Some(name)) {
            return Err("original executable section order/name mismatch".into());
        }
    }
    println!("  Original PE section-header cursors: .txt 0x1A8; .text 0x1D0; .txt2 0x1F8.");
    println!("  The 0x28 scan step follows these actual header cursors, not the section VirtualAddress values.");
    // A complete boolean return from 0x18B00 always goes through a
    // recognizable branch prefix: mismatch takes JE directly to 0x938E;
    // match writes the flag and follows flag-preserving Jccs until 0x938C.
    // The intervening MOV and Jcc instructions preserve TEST EAX flags.
    for (site,bytes) in [
        (0x935Fu32,&[0x74,0x2D][..]),
        (0x9367,&[0x78,0x11][..]),
        (0x9369,&[0x79,0x0F][..]),
        (0x937A,&[0x7F,0xEE][..]),
        (0x936A,&[0x0F,0x80,0x0F,0,0,0][..]),
        (0x9370,&[0x77,0x0D][..]),
        (0x937F,&[0x7B,0x0B][..]),
        (0x938C,&[0xEB,0x37][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    println!("  Comparator=0: TEST -> JE directly to 0x938E (cursor increment).");
    println!("  Comparator=1: TEST -> flag set -> 0x937A -> 0x936A -> 0x9370 -> 0x937F -> 0x938C.");
    if dll.get(executable_rva_to_file_offset(pe,0x938C,2)?..executable_rva_to_file_offset(pe,0x938C,2)?+2)!=Some(&[0xEB,0x37][..]) {
        return Err("matched .txt2 conditional exit jump lost".into());
    }
    println!("  For comparator=1, flags from TEST 1 give 0x937F JNP -> 0x938C JMP 0x93C5: the MATCH bypasses cursor advance.");
    println!("  For comparator=0, 0x935F JE -> 0x938E advances cursor; only MISMATCH follows the 0x9397..0x93C0 back-edge.");
    println!("  This is a local prefix proof only: subsequent opaque branches and overall loop exit are still unverified.");


    println!("  Only an exact .txt2 name match exits scanning with the current selected section outputs intact.");
    println!("  This proves named-section comparison; it does NOT prove whether .txt2 is decoded, skipped or acts as a boundary.");
    Ok(())
}

fn original_dll_self_section_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    // A conditionally reached startup site initializes descriptor mode
    // to 0x67. The actual runtime reachability through opaque branches
    // remains separate from this byte-exact finding.
    dll_expect_raw_x86(dll,pe,0x97E7,&[
        0x6A,0x67,0x68,0xE0,0x59,0x90,0x00,0xE8,0x5D,0x0D,0x00,0x00])?;
    dll_expect_raw_x86(dll,pe,0xA550,&[
        0x8B,0x4C,0x24,0x04,0x8B,0x44,0x24,0x08,0x89,0x41,0x08,0xC3])?;
    for (site,target) in [
        (0x97EEu32,0xA550u32),
        (0xA84E,0xA620),
        (0xA858,0xA560),
    ] {if dll_relative_call_target(dll,pe,site)?!=target {
        return Err("DLL descriptor mode helper call target changed".into());
    }}
    for (site,bytes) in [
        (0xA810u32,&[0x56,0x8B,0x74,0x24,0x08,0x83,0x7E,0x08,0x64][..]),
        (0xA836,&[0x8B,0x46,0x08,0x8B,0x4C,0x24,0x10][..]),
        (0xA841,&[0x83,0xF8,0x65][..]),
        (0xA620,&[0x56,0x8B,0x74,0x24,0x08,0x57,0x8B,0x46,0x08,0x83,0xF8,0x67][..]),
        (0xA687,&[0x8B,0x76,0x04,0x8B,0x4C,0x24,0x14,0x8B,0x7C,0x24,0x18][..]),
        (0xA692,&[0x03,0xF2,0x8B,0xD1][..]),
        (0xA69A,&[0xC1,0xE9,0x02,0xF3,0xA5][..]),
        (0xA69F,&[0x8B,0xCA,0x83,0xE1,0x03,0xF3,0xA4][..]),
        (0x18A26,&[0x74,0x12][..]),
        (0x18A2C,&[0x83,0xC6,0x28][..]),
        (0x18A3A,&[0x8B,0xC6][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    let matching:Vec<&PeSection>=pe.sections.iter().filter(|section|
        (section.characteristics&0x2000_0020)==0x2000_0020
    ).collect();
    if matching.len()!=3 ||
       matching[0].name.split(|&b|b==0).next()!=Some(b".txt".as_slice()) ||
       matching[1].name.split(|&b|b==0).next()!=Some(b".text".as_slice()) ||
       matching[2].name.split(|&b|b==0).next()!=Some(b".txt2".as_slice()) ||
       matching[0].virtual_address!=0x1000 || matching[0].virtual_size!=0x7671 {
        return Err("unexpected original DLL executable-section order".into());
    }
    println!("  A startup opcode sequence sets descriptor mode 0x67 through 0xA550; reachability remains to establish.");
    println!("  Read helper 0xA810 dispatches mode 0x67 to 0xA620, which can copy bytes from module_base+offset.");
    println!("  Original DLL CODE|EXEC sections, in header order: .txt (RVA 0x1000), .text, .txt2.");
    println!("  Selector 1 uses a first-match loop; with DLL headers it selects .txt (RVA 0x1000, VirtualSize 0x7671).");
    for (rva,code) in [
        (0x183EDu32,&[0x8B,0x44,0x24,0x24][..]),
        (0x183F1,&[0x8B,0x54,0x24,0x1C][..]),
        (0x183F5,&[0x89,0x01][..]),
        (0x183FE,&[0x8D,0x4C,0x24,0x10][..]),
        (0x18403,&[0x89,0x10][..]),
        (0x944D,&[0x8D,0x85,0xD8,0xEF,0xFF,0xFF,0x50][..]),
        (0x9458,&[0x8B,0x55,0xDC,0x52][..]),
        (0x95B1,&[0x8B,0x45,0xDC,0x50][..]),
        (0x9717,&[0x8B,0x45,0x08,0x8B,0x48,0x04,0x03,0x4D,0x0C,0x89,0x4D,0xFC][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,code)?;}
    for (site,target) in [(0x9460u32,0x9710u32),(0x95B9,0x9640),(0x18405,0x18B90)] {
        if dll_relative_call_target(dll,pe,site)?!=target {
            return Err("DLL selector-to-copy x86 call target changed".into());
        }
    }
    let selected=matching[0];
    if selected.raw_offset!=0x400 || selected.raw_len!=0x7800 {
        return Err("unexpected DLL .txt file-backed geometry".into());
    }
    let (disk_offset,virtual_rva,extent)=synthetic_selector_fields(
        selected.raw_offset as u32,selected.virtual_address,
        selected.virtual_size,selected.raw_len as u32);
    if (disk_offset,virtual_rva,extent)!=(0x400,0x1000,0x7671) {
        return Err("unexpected DLL .txt selector outputs".into());
    }
    let nt=read_u32(dll,0x3C)? as usize;
    let preferred_base=read_u32(dll,nt+24+28)?;
    let (relocations,_)=pe_relocation_blocks(dll,pe)?;
    let audit=audit_protected_dll_relocations(dll,pe,&relocations,preferred_base)?;
    if audit.slots!=214 || audit.targets_data!=195 ||
       audit.targets_rdata!=18 || audit.targets_txt!=1 ||
       audit.targets_other!=0 || audit.out_of_image!=0 {
        return Err("unexpected original .txt relocation target distribution".into());
    }
    println!("  Original protected .txt contains 214 backed HIGHLOW DWORDs: 195 -> .data, 18 -> .rdata, 1 -> .txt.");
    println!("  All 214 raw pointer values target this DLL image; the section is NOT uniformly opaque ciphertext.");
    println!("  Relocation operands cannot be naively transformed as undifferentiated code bytes.");
    println!("  Selector-1 section output: disk pointer 0x400, VirtualAddress 0x1000, bounded extent 0x7671.");
    println!("  0x9458 and 0x95B1 both pass [EBP-0x24] (the section RVA + current chunk offset).");
    println!("  Both 0x9710 (copy-in) and 0x9640 (copy-out) derive module_base + that RVA offset.");
    println!("  This establishes the address formula for whichever CODE|EXEC section survives the later named-section comparison.");
    println!("  This is conditional on the passed descriptor referring to the original DLL; no plaintext or execution asserted.");
    Ok(())
}

fn original_range_sort_and_adjacency_proof(dll:&[u8],pe:&Pe32)->Result<()> {
    // 0xC3D0's CMP/SBB/NEG returns 1 iff the unsigned probe is BELOW
    // the current node start; equality returns 0. It is NOT an equality test.
    dll_expect_raw_x86(dll,pe,0xC3D0,&[
        0x8B,0x4C,0x24,0x04,0x8B,0x44,0x24,0x08,0x3B,0x01,
        0x1B,0xC0,0xF7,0xD8,0xC3])?;
    // 0xC4B0 checks same tag and either a.end==b.start or
    // b.end==a.start (with native x86 wrapping 32-bit arithmetic).
    dll_expect_raw_x86(dll,pe,0xC4B0,&[
        0x8B,0x54,0x24,0x04,0x56,0x8B,0x74,0x24,0x14,0x33,
        0xC0,0x8B,0x4A,0x08,0x3B,0xCE,0x75,0x1E,0x8B,0x0A,
        0x8B,0x72,0x04,0x8B,0x54,0x24,0x0C,0x03,0xF1,
        0x3B,0xD6,0x74,0x0A,0x8B,0x74,0x24,0x10,0x03,
        0xF2,0x3B,0xF1,0x75,0x05,0xB8,0x01,0x00,0x00,
        0x00,0x5E,0xC3])?;
    // On a same-tag extension, C350 stores (address+size)-start,
    // i.e. exclusive end minus original start; NOT (last_address-start).
    for (site,bytes) in [
        (0xC350u32,&[0x8B,0x44,0x24,0x08,0x56][..]),
        (0xC35C,&[0x3B,0xC8,0x77,0x44][..]),
        (0xC36E,&[0x03,0xC2,0x8D,0x50,0xFF][..]),
        (0xC373,&[0x3B,0xCA,0x77,0x16][..]),
        (0xC377,&[0x3B,0xD7,0x73,0x12][..]),
        (0xC398,&[0x2B,0xC1,0x89,0x46,0x04][..]),
        (0xC3A4,&[0x5F,0x66,0x33,0xC0,0x5E,0xC3][..]),
    ] {dll_expect_raw_x86(dll,pe,site,bytes)?;}
    // The record-versus-record variant is the merge-loop predicate.
    dll_expect_raw_x86(dll,pe,0xC4F0,&[
        0x56,0x8B,0x74,0x24,0x08,0x57,0x8B,0x7C,0x24,
        0x10,0x8B,0x4E,0x08,0x33,0xC0,0x3B,0x4F,0x08,
        0x75,0x1B,0x8B,0x0E,0x8B,0x76,0x04,0x8B,0x17,
        0x03,0xF1,0x3B,0xD6,0x74,0x09,0x8B,0x77,0x04,
        0x03,0xF2,0x3B,0xF1,0x75,0x05,0xB8,0x01,0x00,
        0x00,0x00,0x5F,0x5E,0xC3])?;
    for (site,target) in [
        (0xBC12u32,0xC3D0u32),(0xBC5E,0xC4B0),
        (0xBC6F,0xC3D0),(0xBEA1,0xC3D0),
        (0xB9E6,0xC4F0),(0xBA35,0xC4F0),
        (0xBA12,0xC310),(0xBA61,0xC310),
    ] { if dll_relative_call_target(dll,pe,site)?!=target {
        return Err(format!("range-list call target mismatch at RVA 0x{site:X}"));
    }}
    println!("  Original 0xC3D0 is unsigned (probe < node.start), NOT start equality.");
    println!("  Original 0xC4B0/0xC4F0 test same-tag adjacency in either direction.");
    println!("  0xB9D0 merges neighboring range records through 0xC4F0 and 0xC310.");
    println!("  0xC350 may extend a matching range by storing (address+size)-start in its length DWORD.");
    println!("  The eight list categories and any connection to protected ICD code remain unproven.");
    Ok(())
}

fn print_dll_memory_patch_research(dll:&[u8],pe:&Pe32)->Result<()>{
    let nt=read_u32(dll,0x3C)? as usize;
    let base=read_u32(dll,nt+24+28)?;
    let (first,second)=dll_original_chunk_constants(dll,pe)?;
    dll_chunk_byte_recurrence_proof(dll,pe)?;
    for (rva,code) in [
        (0x95A6u32,&[0x8D,0x8D,0xD8,0xEF,0xFF,0xFF,0x51,0x8B,0x55,0xD8,0x52,0x8B,0x45,0xDC,0x50,0x8D,0x4D,0x08,0x51][..]),
        (0x9649,&[0x8B,0x45,0x08,0x8B,0x48,0x04,0x03,0x4D,0x0C,0x89,0x4D,0xF8][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,code)?;}
    for (rva,code) in [
        (0x18823u32,&[0xE8,0xB8,0xF9,0xFF,0xFF][..]),
        (0x18255,&[0x49,0xC7,0x02,0,0,0,0][..]),
        (0x18263,&[0x83,0xF9,0x0A][..]),
        (0x18272,&[0xFF,0x24,0x8D,0x80,0x86,0x8F,0x00][..]),
        (0x183C9,&[0x68,0x20,0,0,0x20,0x68,0x20,0,0,0x20][..]),
        (0x183D4,&[0xE8,0x17,0x06,0,0][..]),
        (0x18A1A,&[0x8B,0x57,0x24,0x8B,0xCB,0x23,0xCA][..]),
        (0x18A24,&[0x3B,0xCD][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,code)?;}
    let jump_table_rva=0x18680u32;
    let table=dll_backed_rva_offset(pe,jump_table_rva,4)?;
    let case_va=read_u32(dll,table)?;
    if case_va.checked_sub(base)!=Some(0x183C2){return Err("selector 1 no longer maps to x86 case RVA 0x183C2".into());}
    for (site,target) in [(0x18823u32,0x181E0u32),(0x183D4,0x189F0)] {
        if dll_relative_call_target(dll,pe,site)?!=target{return Err("selector helper call target changed".into());}
    }
    // Original 0x189F0 walks IMAGE_SECTION_HEADER entries in 0x28-byte
    // strides and returns the matching entry address, not a destination VA.
    // The selector case subsequently reads offsets from its caller-owned
    // scratch structure. These checks guard that distinction.
    for (rva, code) in [
        (0x18A1Au32, &[0x8B,0x57,0x24,0x8B,0xCB,0x23,0xCA][..]),
        (0x18A24, &[0x3B,0xCD,0x74,0x12][..]),
        (0x18A2C, &[0x83,0xC6,0x28][..]),
        (0x18A3A, &[0x8B,0xC6,0x5F,0x5E,0x5D,0x5B,0xC3][..]),
        (0x183E6, &[0x8B,0x8C,0x24,0x80,0x01,0x00,0x00][..]),
        (0x183ED, &[0x8B,0x44,0x24,0x24][..]),
        (0x183F1, &[0x8B,0x54,0x24,0x1C][..]),
        (0x183F5, &[0x89,0x01][..]),
        (0x183F7, &[0x8B,0x84,0x24,0x88,0x01,0x00,0x00][..]),
        (0x183FE, &[0x8D,0x4C,0x24,0x10][..]),
        (0x18405, &[0xE8,0x86,0x07,0x00,0x00][..]),
        (0x18B90, &[0x8B,0x4C,0x24,0x04,0x8B,0x41,0x08][..]),
    ] { dll_expect_raw_x86(dll,pe,rva,code)?; }
    if dll_relative_call_target(dll,pe,0x18405)?!=0x18B90 {
        return Err("selector case section-size helper target mismatch".into());
    }
    println!("  RVA 0x189F0: section-header walk compares Characteristics at +0x24 and advances 0x28 bytes per entry.");
    println!("  RVA 0x183E6..0x18405: selector case copies out descriptor fields and calls 0x18B90 for bounded size.");
    println!("  The selected PE header address is NOT itself proof of final write destination.");
    println!("  Selector 1 at 0x18800 forwards to 0x181E0; (selector-1)==0 uses jump table case 0x183C2.");
    println!("  Case 0x183C2 calls 0x189F0 with required section flags 0x20000020 (CODE | EXECUTE).");
    println!("  0x189F0 compares section Characteristics at +0x24 after AND mask; no specific destination RVA is asserted.");
    // Check VirtualProtect's branch semantics rather than merely the FF15
    // instruction: if the first call fails, the REP MOVS copy is skipped.
    // These are verified byte sequences, not a claim of runtime execution.
    for (at,bytes) in [
        (0x967Du32,&[0x85,0xC0,0x75,0x0F,0xEB,0x7F][..]),
        (0x9690,&[0x8B,0x4D,0x10,0x8B,0x75,0x14,0x8B,0x7D,0xF8][..]),
        (0x969B,&[0xC1,0xE9,0x02,0xF3,0xA5,0x8B,0xCA,0x83,0xE1,0x03,0xF3,0xA4][..]),
    ] {dll_expect_raw_x86(dll,pe,at,bytes)?;}
    println!("  Writer branch: TEST EAX,EAX at 0x967D; nonzero jumps to copy at 0x9690, zero jumps toward 0x9702.");
    println!("  The first VirtualProtect must succeed before REP MOVSD / MOVSB; section destination remains unresolved.");
    // Both call sites in the same 0x92B0 stack frame pass &([EBP+8]).
    // This establishes the descriptor identity between section selection
    // and the later guarded memory copy, not its absolute runtime value.
    for (rva,code) in [
        (0x9332u32,&[0x6A,0x01,0x8D,0x4D,0x08,0x51][..]),
        (0x9338,&[0xE8,0xC3,0xF4,0x00,0x00][..]),
        (0x95B5,&[0x8D,0x4D,0x08,0x51][..]),
        (0x95B9,&[0xE8,0x82,0x00,0x00,0x00][..]),
        (0x18B94,&[0x8B,0x41,0x08,0x85,0xC0,0x74,0x0A][..]),
        (0x18B9B,&[0x8B,0x49,0x10,0x3B,0xC1,0x72,0x06][..]),
        (0x18BA2,&[0x8B,0xC1,0xC3][..]),
        (0x18BA5,&[0x8B,0x41,0x10,0xC3][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,code)?;}
    println!("  Identity: 0x9334 and 0x95B5 both form &([EBP+8]) in the same 0x92B0 frame.");
    println!("  The first call selects the section via selector 1; 0x95B9 passes the descriptor onward to 0x9640.");
    println!("  The helper 0x18B90 yields min(nonzero field +8, field +0x10), otherwise field +0x10.");
    println!("  The stored descriptor +4 field still needs runtime-base/section-header provenance validation.");
    // The caller constructs a by-value 12-byte descriptor on the stack.
    // Its initial fields are loaded from dynamic (zero-on-disk) DLL .data.
    // This shows where the writer base comes from, without claiming a value
    // for that runtime-initialized field.
    for (rva,bytes) in [
        (0x9232u32,&[0x83,0xEC,0x0C,0x8B,0xD4][..]),
        (0x9237,&[0xA1,0xE0,0x59,0x90,0x00][..]),
        (0x923C,&[0x89,0x02][..]),
        (0x923E,&[0x8B,0x0D,0xE4,0x59,0x90,0x00][..]),
        (0x9244,&[0x89,0x4A,0x04][..]),
        (0x9247,&[0xA1,0xE8,0x59,0x90,0x00][..]),
        (0x924C,&[0x89,0x42,0x08][..]),
        (0x924F,&[0xE8,0x5C,0x00,0x00,0x00][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,bytes)?;}
    for rva in [0x259E0u32,0x259E4,0x259E8] {
        if dll_zero_filled_section(pe,rva,4).is_none() {
            return Err("descriptor source DWORD is not in original virtual-only .data".into());
        }
    }
    if dll_relative_call_target(dll,pe,0x924F)?!=0x92B0 {return Err("descriptor producer call target changed".into());}
    println!("  Caller 0x9232..0x924F copies virtual .data DWORDs 0x259E0/0x259E4/0x259E8 into a by-value stack descriptor.");
    // The first initializer for the descriptor's second DWORD uses the
    // *ordinary* PE import GetModuleHandleA rather than a decoded name.
    for (site,code) in [
        (0x9869u32,&[0x50,0xFF,0x15,0x98,0xE0,0x8F,0x00][..]),
        (0x9870,&[0xA3,0xE4,0x59,0x90,0x00][..]),
        (0x988E,&[0xA1,0xE4,0x59,0x90,0x00][..]),
        (0x9893,&[0x89,0x41,0x04][..]),
    ] {dll_expect_raw_x86(dll,pe,site,code)?;}
    let (_,iat_rva)=dll_indirect_import_call(dll,pe,0x986A,base)?;
    let bindings=dll_import_bindings(dll,pe)?;
    let binding=bindings.iter().find(|b|b.iat_rva==iat_rva)
        .ok_or("module-handle initializer does not reference a declared import")?;
    if binding.symbol!="GetModuleHandleA" || !binding.library.eq_ignore_ascii_case("KERNEL32.dll"){
        return Err("descriptor base initializer is not KERNEL32!GetModuleHandleA".into());
    }
    println!("  RVA 0x986A calls KERNEL32!GetModuleHandleA, then 0x9870 stores EAX into virtual .data RVA 0x259E4.");
    println!("  Hence descriptor +4 contains a module handle/base when initialization succeeds, not a hardcoded preferred .text address.");
    dll_expect_raw_x86(dll,pe,0x985C,&[0x68,0x30,0x00,0x90,0x00])?;
    if dll_relative_call_target(dll,pe,0x9861)?!=0xE25C {return Err("module-name decoder call target changed".into());}
    let encoded_at=dll_backed_rva_offset(pe,0x20030,128)?;
    let (module_name,_)=unsafedisc_strings::decode(&dll[encoded_at..])
        .ok_or("original module name cannot be decoded")?;
    if module_name!="dplayerx.dll"{return Err("unexpected original GetModuleHandleA module name".into());}
    println!("  The argument decoded at 0x9861 from data RVA 0x20030 is 'dplayerx.dll'.");
    // A140 does not merely stash a pointer: B540 copies the 12-byte
    // descriptor into one of five 0xD0-byte per-module records.
    for (rva,op) in [
        (0x9875u32,&[0x6A,0x01,0x68,0xE0,0x59,0x90,0x00][..]),
        (0x987C,&[0xE8,0xBF,0x08,0x00,0x00][..]),
        (0xA14B,&[0x50,0x53,0x33,0xFF][..]),
        (0xA14F,&[0xE8,0xEC,0x13,0x00,0x00][..]),
        (0xB554,&[0xBE,0xE8,0x4A,0x90,0x00][..]),
        (0xB55F,&[0x81,0xC6,0xD0,0x00,0x00,0x00][..]),
        (0xB565,&[0x81,0xFE,0xF8,0x4E,0x90,0x00][..]),
        (0xB573,&[0xB9,0x34,0x00,0x00,0x00][..]),
        (0xB57C,&[0xF3,0xAB][..]),
        (0xB583,&[0x8B,0x0B][..]),
        (0xB58B,&[0x8B,0x53,0x04][..]),
        (0xB591,&[0x8B,0x4B,0x08][..]),
        (0xB5A4,&[0xE8,0x27,0x01,0x00,0x00][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,op)?;}
    for (call,target) in [(0x987Cu32,0xA140u32),(0xA14F,0xB540),(0xB5A4,0xB6D0)] {
        if dll_relative_call_target(dll,pe,call)?!=target {return Err("module record initializer call target changed".into());}
    }
    if (0x24EF8u32-0x24AE8u32)/0xD0!=5 {return Err("module-record table geometry invalid".into());}
    println!("  0xA140 -> 0xB540: five 0xD0-byte module records at RVA 0x24AE8..0x24EF7.");
    println!("  0xB540 copies all three descriptor DWORDs at offsets +4/+8/+12 and calls 0xB6D0 for further initialization.");
    for (rva,opcode) in [
        (0xB6D0u32,&[0x83,0xEC,0x20,0x56,0x57,0x33,0xF6][..]),
        (0xB6DB,&[0x56,0xE8,0x5F,0xFF,0xFF,0xFF][..]),
        (0xB6E4,&[0x89,0x07,0x85,0xC0,0x74,0x22][..]),
        (0xB6EE,&[0x83,0xFE,0x08,0x7C,0xE8][..]),
        (0xB6F7,&[0xB9,0x08,0x00,0x00,0x00][..]),
        (0xB700,&[0x66,0xB8,0x01,0x00,0xF3,0xA5][..]),
        (0xB70C,&[0x8D,0x44,0x24,0x08,0x50][..]),
        (0xB711,&[0xE8,0x5A,0x00,0x00,0x00][..]),
        (0xB719,&[0x66,0x33,0xC0][..]),
        (0xB640,&[0x56,0x57,0x6A,0x10,0x68,0xCC,0x00,0x00,0x00][..]),
        (0xB676,&[0x89,0x06,0x85,0xC0][..]),
        (0xB692,&[0x89,0x41,0x04][..]),
        (0xB69A,&[0x89,0x42,0x08][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,opcode)?;}
    for (site,target) in [(0xB6DCu32,0xB640u32),(0xB711,0xB770),(0xB649,0xB610)] {
        if dll_relative_call_target(dll,pe,site)?!=target {return Err("auxiliary allocator call target changed".into());}
    }
    println!("  0xB6D0 calls 0xB640 eight times and commits eight DWORD pointers only after all calls succeed.");
    println!("  Failed allocation invokes 0xB770 for cleanup, then returns AX=0; success returns AX=1.");
    println!("  0xB640 requests a 0xCC-byte record aligned to 0x10, initializes two linked objects via 0xB7C0.");
    original_range_sort_and_adjacency_proof(dll,pe)?;
    original_category_dispatch_proof(dll,pe)?;
    original_dll_self_section_proof(dll,pe)?;
    original_txt2_section_name_proof(dll,pe)?;
    dll_selector_detour_proof(dll,pe)?;
    original_selector_exhaustion_proof(dll,pe)?;
    original_category_selector_relation(dll,pe)?;
    // The child list stores 12-byte range nodes rather than embedded code.
    // The first two words of each node represent start and length; the
    // third is a caller-supplied classification/tag value.
    for (rva,op) in [
        (0xB7C0u32,&[0x56,0x6A,0x0C,0x68,0xCD,0x00,0x00,0x00][..]),
        (0xB7E5,&[0xC7,0x46,0x04,0x00,0x00,0x00,0x00][..]),
        (0xB7EC,&[0xC7,0x46,0x08,0x00,0x00,0x00,0x00][..]),
        (0xB7F3,&[0xE8,0xA8,0x0A,0x00,0x00][..]),
        (0xC2A0,&[0x6A,0x0C,0x68,0xCB,0x00,0x00,0x00][..]),
        (0xC2BB,&[0x89,0x08][..]),
        (0xC2C1,&[0x89,0x50,0x04][..]),
        (0xC2C4,&[0x89,0x48,0x08][..]),
        (0xBAD7,&[0x8B,0x70,0x08][..]),
        (0xBAF1,&[0x8B,0x06][..]),
        (0xBB15,&[0x8B,0x76,0x04][..]),
        (0xC350,&[0x8B,0x44,0x24,0x08][..]),
        (0xC35A,&[0x8B,0x0E,0x3B,0xC8][..]),
    ] {dll_expect_raw_x86(dll,pe,rva,op)?;}
    for (call,target) in [(0xB7F3u32,0xC2A0u32),(0xBAF5,0xC3D0),(0xBB08,0xC350)] {
        if dll_relative_call_target(dll,pe,call)?!=target {return Err("auxiliary range list call target changed".into());}
    }
    println!("  0xB7C0 -> 0xC2A0 allocates a 12-byte node with start/length/tag DWORDs.");
    println!("  0xBAD0 iterates linked nodes and uses 0xC3D0/0xC350 to test range bounds; eight pointers index range lists.");
    println!("  This is range bookkeeping evidence, not eight plaintext buffers.");
    println!("  Thus on successful resolution writer descriptor +4 is dplayerx.dll module base (not CRIMSON.ICD image base).");
    println!("  Actual loaded base and exact relative target within dplayerx.dll remain to be established.");
    println!("  Copy destination in 0x9640: *(u32*)(first_argument+4) + chunk_offset (0x9649..0x9652).");
    println!("  The first_argument is the address of [EBP+8] in caller 0x95B9, and its pointee must be traced before identifying a PE section.");
    println!("  Original x86 verifies index starts at 1; both indexed reads use the same [EBP-0x10] loop index.");
    println!("  Candidate recurrence: out[0]=in[0]^0x43; state=out[0]^0x15; out[i]=(in[i]^out[i-1])+state (mod 256); state+=out[i] (mod 256).");
    println!("  This is a local byte model, NOT yet validated against plaintext protected code or confirmed across chunk boundaries.");
    println!("  Original RVA 0x92B0 chunk routine disguises byte constants as 32-bit immediates.");
    println!("    XOR pair 0xF3E97EC9 ^ 0xF3E97E8A => 0x{first:02X}.");
    println!("    SUB pair 0xC502BDE6 - 0xC502BDD1 => 0x{second:02X}.");
    for (site,target,role) in [
        (0x924Fu32,0x92B0u32,"chunk-transform entry"),
        (0x9338,0x18800,"original .txt2 chunk source/selection helper"),
        (0x9355,0x18B00,"original .txt2 range validation helper")
    ] {
        let actual=dll_relative_call_target(dll,pe,site)?;
        if actual!=target{
            return Err(format!("original chunk setup E8 at 0x{site:X} no longer targets 0x{target:X}"));
        }
        println!("    E8 0x{site:X} -> 0x{target:X}: {role}");
    }
    let data=pe.section(b".data")?;
    let zero_fill_start=data.virtual_address.checked_add(data.raw_len as u32)
        .ok_or("original DLL .data backed boundary overflow")?;
    let virtual_end=data.virtual_address.checked_add(data.virtual_size)
        .ok_or("original DLL .data virtual upper bound overflow")?;
    for (site,dest,role) in [
        (0x9460u32,0x9710u32,"copy current input chunk into stack buffer"),
        (0x95B9,0x9640,"commit modified stack buffer to protected destination"),
    ]{
        let actual=dll_relative_call_target(dll,pe,site)?;
        if actual!=dest{
            return Err(format!("original DLL memory-copy CALL 0x{site:X} unexpectedly targets 0x{actual:X}"));
        }
        println!("  E8 0x{site:05X} -> 0x{dest:05X}: {role}");
    }
    for site in [0x9677u32,0x96DC] {
        let (_,iat_rva)=dll_indirect_import_call(dll,pe,site,base)?;
        // 0x24F00 is a *runtime-resolved* pointer in zero-filled
        // .data, NOT a PE import-descriptor entry. Its static
        // name association is proven separately by the original
        // decoded-label -> C708 -> A3 0x904F00 chain.
        if iat_rva!=0x24F00 || iat_rva<zero_fill_start ||
           iat_rva.checked_add(4).is_none_or(|end|end>virtual_end){
            return Err("original copy helper does not call the virtual-only VirtualProtect slot 0x24F00".into());
        }
        println!("  FF15 0x{site:05X}: indirect through runtime-resolved VirtualProtect slot 0x{iat_rva:X} (not an ordinary PE import)");
    }
    // 0x940D tests the remaining input length against the
    // maximum 0x1000-byte chunk size. One copy-in function
    // and a separate temporarily-writable copy-out function
    // are verified from concrete original instructions.
    for (rva,bytes,description) in [
        (0x940Du32,&[0x81,0x7D,0xEC,0x00,0x10,0x00,0x00][..],"clamp remaining length to 4096 bytes"),
        (0x9669,&[0x8D,0x55,0xF4,0x52,0x6A,0x04][..],"pass old-protection pointer and PAGE_READWRITE"),
        (0x969B,&[0xC1,0xE9,0x02][..],"divide copy size by four"),
        (0x969E,&[0xF3,0xA5][..],"copy DWORDs into writable destination"),
        (0x96A2,&[0x83,0xE1,0x03][..],"compute trailing byte count"),
        (0x96A5,&[0xF3,0xA4][..],"copy trailing bytes"),
        (0x950Du32,&[0x8A,0x94,0x0D,0xD8,0xEF,0xFF,0xFF][..],"load current stack-buffer byte"),
        (0x9519,&[0x8A,0x8C,0x05,0xD7,0xEF,0xFF,0xFF][..],"load stack-buffer byte indexed by EAX (adjacency unproved)"),
        (0x9520,&[0x33,0xD1][..],"XOR EDX, ECX (32-bit register operation)"),
        (0x9525,&[0x88,0x94,0x05,0xD8,0xEF,0xFF,0xFF][..],"store XOR-transformed byte"),
        (0x955D,&[0x03,0xD0][..],"ADD EDX, EAX (32-bit register operation)"),
    ] {
        dll_expect_raw_x86(dll,pe,rva,bytes)?;
        println!("  Original x86 RVA 0x{rva:05X}: {description}");
    }
    // The two indexed reads do NOT alone prove adjacent addresses: one uses
    // ECX and the other EAX. Adjacency requires a separately proven relation
    // between the live register values. Check the actual x86 SIB operands.
    for (at, expected) in [
        (0x950Du32, [0x8Au8,0x94,0x0D,0xD8,0xEF,0xFF,0xFF]),
        (0x9519u32, [0x8A,0x8C,0x05,0xD7,0xEF,0xFF,0xFF]),
    ] {
        dll_expect_raw_x86(dll,pe,at,&expected)?;
    }
    println!("  Operand caveat: at 0x950D the address is [EBP + ECX - 0x1028]; at 0x9519 it is [EBP + EAX - 0x1029].");
    println!("  These addresses are adjacent ONLY if the live EAX and ECX values satisfy the required relation.");
    println!("  0x9520 XOR EDX,ECX and 0x955D ADD EDX,EAX are 32-bit operations; byte truncation/state semantics remain unproved.");
    println!("  Static sequence: 4KiB-bounded copy-in -> byte transform -> VirtualProtect -> memory copy -> restore protection.");
    println!("  Unknown: input buffer mapping, per-byte state, actual destination and whether the modified region is protected .txt.");
    println!("  These checks do not emulate SafeDisc, decrypt any code, or call Win32 APIs.");
    // Keep the evidence local: dump only bounded, original file-backed code
    // windows for the unresolved selector and per-byte state transitions.
    // No executable bytes or generated plaintext are stored in the repo.
    for (title, rva, length) in [
        ("chunk setup and state (0x92B0)", 0x92B0u32, 96usize),
        ("selector callsites and chunk initialization", 0x9300, 96),
        ("input bounds and buffer initialization", 0x9400, 96),
        ("byte recurrence and state update", 0x9500, 128),
        (".txt2 selection helper (0x18800)", 0x18800, 96),
        (".txt2 range helper (0x18B00)", 0x18B00, 96),
    ] {
        print_dll_code_window(dll, pe, title, rva, length)?;
    }
    println!("  Windows are raw bytes from the verified local DLL, not disassembly or proof of execution.");
    Ok(())
}

fn print_modifier_research(dll: &[u8]) -> Result<()> {
    const WINDOW: usize = 128;
    const ROUTINES: [(&str, u32); 2] = [
        ("first EAX modifier", 0x65BE),
        ("second EAX modifier", 0x6621),
    ];
    let pe = parse_pe32(dll)?;
    println!("SafeDisc 1.50.20 game-specific modifier research (STATIC CODE BYTES ONLY):");
    print_dll_memory_patch_research(dll,&pe)?;
    unsafedisc_dll_category_dispatch::verify_original(dll,&pe)?;
    unsafedisc_dll_state_provenance::audit_original(dll,&pe)?;
    unsafedisc_dll_keyboard_probe::audit_original(dll,&pe)?;
    unsafedisc_dll_worker_finalizer::audit_original(dll,&pe)?;
    unsafedisc_dll_worker_entry::audit_original(dll,&pe)?;
    unsafedisc_dll_io_backends::audit_original(dll,&pe)?;
    unsafedisc_dll_pe_sections::audit_original(dll,&pe)?;
    unsafedisc_dll_a310::audit_original(dll,&pe)?;
    unsafedisc_dll_bed0::audit_original(dll,&pe)?;
    println!("  Original A310->BED0 stage engine: 196A0 PUSH literal 4, forwarded to BED0 at A3F2. BED0 copies eight DWORD slots locally, invokes BDD0 first with selected slot 4.");
    println!("    If BDD0 AX != 0, BED0 checks the seven OTHER slots through BCA0 in order 0,1,2,3,5,6,7, stopping on first returned AX==0. A separate selector 3 would skip all followups.");
    println!("    BED0 writes TWO working DWORDs back unconditionally even after helper AX==0; BDD0 failure can itself change a local cursor. AX==0 DOES NOT imply no state mutation.");
    println!("    BDD0 failure AX=0 branch still writes selected pointer's DWORD = (another working DWORD - 1) modulo 2^32. No safe pointer assumptions made.");
    println!("    Exact C550 unsigned range predicate plus C430 lower=max(start,lower), C450 upper=min(start+len-1, upper) with 32-bit wrap are original-byte audited.");
    println!("    BDD0/BCA0 use other original C* helpers; actual stage outputs, runtime pointers and byte-mixer state remain UNKNOWN.");
    println!("  Former opaque A310 source helper is now partially recovered in readable original .text: null cursor/limit, cursor>=limit (unsigned), or missing B5D0 map -> AX=0.");
    println!("    On accepted source it checks a 4 KiB page cache, optionally invokes original C0D0/C0C0/BFB0 cache helpers, then delegates to BED0; BED0 result is NOT inferred.");
    println!("    Aligned nonzero cursor probe decrements by one before 4 KiB same-page comparison. Cache globals and runtime data are NOT reconstructed.");
    println!("  Original first worker case is a PE32 section finder: 18940 reads 0x40 DOS + 0xF8 NT header bytes via A810 and compares DWORD PE signature.");
    println!("    At 18204 the dispatcher derives the section-header range from NumberOfSections (40 bytes each) and SizeOfOptionalHeader.");
    println!("    Original case 183C2 calls 189F0 with mask 0x20000020 (IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE); first matching section's raw offset and RVA are returned as outputs.");
    println!("    18B90 selects section length min(VirtualSize,SizeOfRawData), or raw size when VirtualSize is zero; this is section metadata, not decoded code.");
    println!("  Original .text IO backends: type 0x65 -> A560 SetFilePointer/ReadFile with exact byte count; type 0x66 -> A620 dynamic ReadProcessMemory; type 0x67 -> A620 in-process memcpy (mapped or direct fallback).");
    println!("    Matching WRITE gateway AA00: type 0x65 -> A5C0 SetFilePointer/WriteFile (exact byte count); type 0x66 -> A720 dynamic WriteProcessMemory (exact byte count); type 0x67 -> A720 memory copy using VirtualProtect.");
    println!("    B5D0 source map lookup scans exactly five virtual .data descriptors: 0xD0 stride, active WORD=1, 12-byte exact key; runtime records are NOT available in the file.");
    println!("    Dynamically loaded API names decoded from .data: Kernel32.dll, ReadProcessMemory, WriteProcessMemory, VirtualProtect.");
    println!("    Original A970 cumulative XOR decoder and A870 resolver fill global API slots 904AE0/904ADC/904AD8; no Windows API invoked, no live file/process memory accessed.");
    println!("    Important: 0x67 mapped branch caps copy count at min(request, total map size), NOT map size minus offset; this static audit is not a safety proof.");
    println!("  Original 18F81 worker entry: its 18F91 flag-preserving maze always reaches 18FA9 for all 32 EFLAGS sets (4 routes, 5..8 instruction boundaries).");
    println!("    First worker CALL at 18FBF -> wrapper 18800 -> 181E0; the dispatcher invokes 18940 before its selector check. Entry-to-worker CALL sites are verified at 1A1B2 and 1A39C, not proven reached from DllMain.");
    println!("    Original 181E0 dispatcher: generic selector 1..11 maps to 11 individually relocated HIGHLOW VAs at 18680; out-of-range -> 184EB (also case 4).");
    println!("    INITIAL worker call selector is now proven: 18FB9 PUSH literal 1 -> 18800 forwards arg2 -> 181E0 [ESP+0x178] reads 1, DEC -> case index 0 -> target 183C2 -> CALL 189F0. This requires original 18940 to return normally.");
    println!("    First original case 183C2 invokes 189F0, a 0x28-byte record scanner. It calls A810, checks (record DWORD at +0x24 & mask) == expected, and returns matched offset or zero.");
    println!("    Native-port-only checked_section_header_scan additionally enforces full 0x28-byte records, read-status/size, bounded iterations, and a distinct Some(0) versus None. This is NOT an original 189F0 safety feature.");
    println!("    189F0 does NOT check A810 AX before comparing buffer +0x24, and checks offset<limit without guaranteeing an entire 0x28-byte record remains; neither a failed read nor a partial tail is asserted to occur in retail gameplay.");
    println!("    189F0 returns offset 0 on a hypothetical match at zero AND on no match: static return-value sentinel collision, not an observed initial scan offset.");
    println!("    Original A810 in .text: source type [arg+8]==0x64 -> AX=0; otherwise WORD [0x00904AE4]!=1 triggers A870 init test; when ready, type 0x65 -> backend A560, other types -> A620.");
    println!("    Real record contents, backend effects and actual first-dispatch result remain UNKNOWN; no original backend is run.");
    println!("    The first wrapper's full-DWORD EAX return test at 18FCA bypasses to 19022 on ZERO; on NONZERO the sign-dependent maze reaches 18FFC CALL 18B00, with 12 or 15 instruction boundaries.");
    println!("  Original worker non-exit branch: CALL 196AE -> opaque helper A310; AND EAX,FFFFh / TEST / JE 19819: low16 zero skips current mixer preparation.");
    println!("    Nonzero low16 continues through three 32/32 EFLAGS-only networks: 196C3->196DB (8 paths), 196E4->196F8 (3 paths), 19704->19728 (4 paths).");
    println!("    On this nonzero A310 branch, AND EAX,FFFF at 196B6 clears the high word; the intervening proven opaque paths and ECX/EDX arithmetic preserve it, so 19728 MOV AX,[EBP-8]; PUSH EAX actually pushes [EBP-8]&FFFF (never stale high EAX on this path).");
    println!("    196F8 computes a 32-bit wrapping candidate step [EBP-14]-[EBP-0C]+1, but the immediate 19728 push uses only its low16; candidate length 65536 would push zero. No claim that later 19965/19C78 observes that exact value.");
    println!("    Conditional same-local scenario: later 197D4 reads the FULL 32-bit [EBP-8] for the cursor update, unlike 19728's low16 immediate push; interim opaque helper writes to that local are unknown.");
    println!("    Exact original 1974A CALL 19965 / ADD ESP,14h and 19798 CALL 19C78 / ADD ESP,18h reclaim 20 and 24 stack bytes; this does not establish exact helper argument signatures.");
    println!("  Original worker mixer setup: CALL 1974A -> 19965; if it returns, the 19752 branch network joins 1976F for all 32 flags (6 paths, 7..10 steps).");
    println!("    Static 19965 neighborhood: at 19A4D a pointer to global mixer state is passed to 1A77F (Windows GetVersionExA helper), and at 19A8B the same global += ([EBP+18]&FFFF). Address order is NOT proof of a reachable same-function route.");
    println!("    Hypothetical GetVersionExA-failure + subsequent direct add, with no intervening state writes: (original_state&FF)+(arg&FFFF), 32-bit wrapping. Actual 19965 CFG, OS result and 19C78 initial state remain unknown.");
    println!("    Exactly 8/32 cases pass via 19765 CALL to a bare RET at 197F8; the following JA 1976B must be taken, avoiding the apparent early RET. No original CALL is executed by this tool.");
    println!("  Original worker CALL at 19798 -> byte mixer 19C78. If it returns, ADD ESP,0x18 at 1979D updates flags; all 32 flag assignments from 197A0 reach counter update 197D4 (24 unique paths, 12..23 instructions).");
    println!("  Original worker counter update at 197D4: [EBP-0x0C] += [EBP-0x08] (wrapping). 197DD -> 197FA for all 32 EFLAGS patterns (8 unique paths, 5..13 instructions).");
    println!("    16/32 patterns use a stack-balanced no-op CALL 1965A (bare RET) and forced JGE; 16 enter 197FA directly. Then [EBP-0x14] := [EBP-0x04].");
    println!("  Original state caller finalizer: all 32 EFLAGS paths from 19800 reach 19686 (5 paths, 8..12 instructions); if unsigned [EBP-0x0C] >= [EBP-0x14], JAE reaches 19819.");
    println!("    All 32 flag assignments from 19819 reach PUSH 00905010h at 1987D (16 paths, 31..41 instructions), followed by CALL 1A9B6.");
    println!("    After the helper, 1A9B6 forwards its helper AX via a local WORD; the caller at 1988A masks EAX to 16 bits, then adds 0x4F3 to global state iff AX != 0.");
    println!("    Conditional special case: if original Win9x helper executes with accepted state guard, +0x34F within helper AND then +0x4F3 at caller => global state += 0x842 (wrapping), NOT universally.");
    println!("    Caller loop-exit branch and return from both nested Windows APIs / INT 68h remain runtime conditions; no 19C78 mixer-entry state is asserted.");
    println!("  Verified original .txt2 keyboard probe: RVA 0x1D42F calls USER32!GetKeyboardType(0/1), accepts type=7 and subtype 0xD01..0xD07 (AX=1).");
    println!("    An original E8 at RVA 0x1CD46 targets this predicate. The adjacent TEST/JNE forks around a literal INT 0x68 with AX=0x4300 when the gate returns zero.");
    println!("    Independently audited predecessor: entry at 0x1CD16 (JBE/JA), 32/32 CF/ZF/PF/SF/OF assignments join CALL 0x1CD46 in 8..18 anchored instruction boundaries.");
    println!("    NEW original Win9x helper entry proof: starting at 0x1C806 immediately after the 1C7F0 prologue, all 32 EFLAGS assignments reach CALL 0x1CD46 via 32 unique paths (210..395 instruction boundaries).");
    println!("    Only Jcc/JMP, NOP and register self-MOV/XCHG are reached before this call. Proves local helper-entry -> keyboard-gate reachability; does NOT prove full DLL startup path or later DWORD mutations.");
    println!("    Conditional keyboard PASS (AX=1) skips INT 0x68, then all 32 flags reach stack-local init at 1CE3B (30..58 steps), OSVERSIONINFOA size init 1CFFF (68..125), and LEA at 1D051 (12..25).");
    println!("    Original 1D058 FF15 imports KERNEL32!GetVersionExA at IAT 1E058; its return and platform ID are runtime-dependent.");
    println!("    Conditional nested Win9x query SUCCESS with dwPlatformId != 1 -> original stack-local 0x0F at 1D133 -> 1D19D -> pointer guard 1D26A.");
    println!("    Keyboard REJECTED: hypothetical NORMAL return from original INT 0x68 -> full EAX comparison with 0x4300. Return != 0x4300 sets stack-local [EBP-0x98]=1; equal retains zero.");
    println!("    Historical identification: INT 0x68 with AX=0x4300 matches a published SafeDisc/Win9x anti-SoftICE debugger probe (see docs/dll-state-references.md). No modern Windows/Wine runtime behavior is inferred.");
    println!("    Keyboard accepted or rejected paths join the same nested version test. If nested version fails or platform=1, 1D19D reaches the pointer guard only when the rejected interrupt's returned EAX differed from 0x4300.");
    println!("    If nested version succeeds on platform !=1, stack-local becomes 15 and reaches pointer guard independently of interrupt. Interrupt availability/return on Windows/Wine is UNKNOWN.");
    println!("    Final Win9x conditional returns: bypass pointer guard -> XOR AX,AX at 1D425 (AX=0, no write); guard with NULL pointer -> MOV AX,1 at 1D2FC (no write); nonnull pointer -> exactly one +0x34F DWORD store, then AX=1.");
    println!("    Final flag-only suffixes proven for all 32 assignments: 1D2C7 -> 1D2FC (16 distinct, 10..21 steps, safe backward branches), 1D305 -> 1D425 (32 distinct, 44..77 steps). Upper 16 EAX bits are NOT determined.");
    println!("    Original pointer provenance also verified: 1987D PUSH global 0x00905010 -> 19882 CALL 1A9B6 -> Win9x branch 1AA9E CALL 1C7F0, which forwards the same address to the conditional +0x34F store.");
    println!("    This may perturb the REAL global mixer state under the conditional historical anti-SoftICE path, but 1987D startup reachability, actual interrupt return and final 19C78 state remain UNPROVEN.");
    println!("    All four intervening flag-only networks converge for 32/32 flags; only a nonnull passed pointer causes original DWORD +0x34F. Outer vs inner Windows platform reports must not be assumed inconsistent.");
    println!("    This is STATIC x86 source evidence only: entrypoint reachability, Windows interrupt semantics, and any relation to a real compatibility failure remain UNKNOWN.");
    // Relocation-backed references are a bounded address-level audit, not
    // a disassembly or a proof that all aliases to the state are covered.
    let state_refs=unsafedisc_dll_state_provenance::scan_original_state_relocations(dll,&pe)?;
    use unsafedisc_dll_state_provenance::StateRefKind;
    let mut known_read=0usize;
    let mut known_write=0usize;
    let mut known_address_pass=0usize;
    let mut unknown=0usize;
    for reference in &state_refs {
        match reference.kind {
            StateRefKind::KnownRead => known_read+=1,
            StateRefKind::KnownWrite => known_write+=1,
            StateRefKind::KnownAddressPass => known_address_pass+=1,
            StateRefKind::Unclassified => unknown+=1,
        }
    }
    println!("  Original PE HIGHLOW relocated DWORDs pointing to state 0x00905010: {} total; {known_read} known reads, {known_write} known writes, {known_address_pass} known address-passes, {unknown} unclassified.",
        state_refs.len());
    for reference in state_refs.iter()
        .filter(|reference|reference.kind==StateRefKind::Unclassified).take(24)
    {
        println!("    Unclassified relocated state pointer: {} operand RVA 0x{:05X} (not necessarily executable code).",
            reference.section,reference.operand_rva);
    }
    if unknown>24 {
        println!("    ... {} additional unclassified relocated pointer(s) omitted.",unknown-24);
    }
    println!("    Relocations exclude nonrelocated address arithmetic and pointer aliases; classification is NOT proof of runtime reachability.");
    println!("  Original global mixer state: preferred VA 0x00905010, RVA 0x25010, .data virtual zero-fill before initial runtime writes.");
    println!("    Seven verified direct x86 writes: conditional +0x4F3, twice +argument_low16, +output_byte, +stack_local, and two explicit resets.");
    println!("    Three pointer-passing state callsites: .txt2 0x19882 -> 0x1A9B6, 0x19A4D -> 0x1A77F, 0x19E30 -> 0x1A77F.");
    println!("    Both 1A9B6 and 1A77F use KERNEL32!GetVersionExA; 1A77F additionally checks OSVERSIONINFOA.dwPlatformId == 2 (Windows NT).");
    println!("    Verified GetVersionExA failure route in 1A77F: conditional JE 1A7B8 -> 1A840 -> 1A849 -> 1A854, where *[passed state pointer] &= 0xFF and AX=1.");
    println!("    This is a proven conditional LOW-BYTE MASK, NOT a full 32-bit reset, successful-version path, or established mixer-entry state.");
    println!("    Independently verified GetVersionExA failure route in the other helper 1A9B6: EAX=0 -> 1AB34 -> original parity/flag detour -> AX=1 and return; no argument-pointer DWORD store on this path.");
    println!("    The two failure branches differ: 1A77F masks the low byte; 1A9B6 keeps the passed DWORD unchanged.");
    println!("    Isolated Win9x helper pointer guard at 1D26A: null [EBP+8] -> AX=1 and return; nonnull -> 32/32 EFLAGS assignments join 1D2A4, DWORD *[EBP+8] += 0x34F (wrapping).");
    println!("    Both original JNE/JE pairs converge before and after this store. Reachability of 1D28F, other helper effects and actual mixer-entry state remain UNKNOWN.");
    println!("    1A77F GetVersionExA-SUCCESS dispatch, original conditional x86 path verified: dwPlatformId=1 -> 1BEEE (Win9x), =2 -> 1AE3F (NT), other -> low-byte mask at 1A854.");
    println!("    Both platform-specific helper bodies invoke the actual KERNEL32!GetModuleHandleA import through IAT 0x1E098; their later state effects are NOT recovered.");
    println!("    1A9B6 GetVersionExA-SUCCESS original x86 route: dwPlatformId=1 -> 1C7F0 (Win9x), =2 -> 1BAC7 (NT), other -> 1AB34 fallback entry.");
    println!("    Original 1AB34 fallback: all 32 combinations of the five relevant x86 flags converge to AX=1 and return, without changing the passed DWORD on that branch.");
    println!("    All FOUR platform-specific helper BODIES and both helpers' runtime effects are unresolved; dispatch alone does not determine the state at 19C78.");
    println!("    Initial PE zero-fill is NOT necessarily the state at 19C78 entry: different paths may modify/reset it beforehand.");
    println!("    Two exact .txt2 E8 sources call shared worker 0x18F81: process path 0x1A1B2, DLL path 0x1A39C.");
    println!("    DLL path has reset / worker / reset instructions at 0x1A360 / 0x1A39C / 0x1A3C0 IN STATIC SOURCE ORDER ONLY; reachability remains unproven.");
    unsafedisc_dll_byte_mix::audit_original(dll,&pe)?;
    println!("  Conditional original .txt2 byte mixer: 0x19798 -> 0x19C78, XOR buffer byte with 4 bytes of 32-bit state at preferred VA 0x00905010 and second-buffer byte.");
    println!("    The byte result is added to global state mod 2^32; source/destination byte pointers advance; guarded parity-detour returns to 0x19D85.");
    println!("    This is instruction-backed LOCAL BYTE ARITHMETIC, not a reconstructed 195DF loop or any verified decrypted .txt page.");
    let verified_pipeline_calls=unsafedisc_dll_write_pipeline::audit_original(dll,&pe)?;
    println!("  Original readable .txt2 dual-read / local-helper / write chain: {verified_pipeline_calls} proven direct CALL targets (including A310 category-4 region checks).");
    println!("    A810 at 0x19327 -> buffer A, A810 at 0x19348 -> buffer B, processing helper 0x195DF at 0x1940B, AA00 at 0x19448 -> buffer A.");
    println!("    Final write shares the original FIRST read's descriptor, offset, length and buffer STACK SLOTS; runtime values and execution are NOT proven.");
    println!("    0x195DF calls 0xA310 with category 4: checks registered module page bounds (+0x34/+0x38), optionally adjusts region metadata and calls secondary range helpers.");
    println!("    This connects the local processing helper to code-section CATEGORY MANAGEMENT, not to a verified decryption algorithm.");
    let memory_apis=unsafedisc_dll_memory_io::audit_original(dll,&pe)?;
    println!("  Verified original A870 chained-string decoder and dynamic KERNEL32 memory API resolution:");
    for api in &memory_apis {
        println!("    {}: encoded RVA 0x{:05X}, resolved function slot RVA 0x{:05X} (read-only).",
            api.name,api.encoded_rva,api.resolved_slot_rva);
    }
    println!("  A810/AA00 shared descriptor dispatch: mode 0x65 -> file, 0x66 -> process memory API, 0x67 -> module-backed memory, 0x64 -> reject.");
    println!("  Original .txt2 RVA 0x19448 calls AA00, the wrapper that may forward mode-0x67 writes to A720.");
    println!("  A620/A720: descriptor mode 0x66 uses process memory API calls; mode 0x67 uses module-backed direct read/write, optional record shadow and VirtualProtect.");
    println!("  These are protected DLL static code paths, NOT proof that any encrypted .txt page was decoded at runtime.");
    let range_edges=unsafedisc_dll_ranges::verify_original(dll,&pe)?;
    println!("  Original B850/B880/B8E0 handlers: {range_edges} verified range-list and interval-helper E8 calls.");
    println!("  Confirmed 12-byte record allocation, category/interval comparisons and linked-list field stores; NOT .txt plaintext restoration.");
    println!("  A2A0 module category dispatch: B5D0 record lookup, B850 first, B880/B8E0 only after prior zero returns.");
    println!("  Original control-flow signatures validated; no branch execution or .txt decoding is asserted.");
    let a140=unsafedisc_dll_bootstrap::model_a140_completion(true);
    println!("  A140 control-flow: once B540 finds/allocates a module record, all {} directory helpers + {} section-class iterators run; AX=1.",
        a140.directory_helpers_called,a140.class_iterators_called);
    println!("  A140 does not branch on individual handler outcomes; downstream A2A0 handlers still have their own early exits.");
    println!("  Additional .txt2 direct references to A140: RVAs 0x1A137 and 0x1A299 (stack-local descriptor pointers).");
    println!("  .txt2 0x1A137 prepares a process descriptor: GetCurrentProcess + GetModuleHandleA(NULL) (main executable), then mode 0x67/A550.");
    println!("  Main-module result is conditional on the original OS API calls; no module base address is assumed.");
    println!("  Direct readable caller 0x9821 -> .txt2 0x1A20D passes original encoded module label dplayerx.dll (verified RVA 0x20030).");
    println!("  .txt2 caller 0x1A299: caller-supplied module-name pointer -> E25C decoder -> GetModuleHandleA -> mode 0x67 -> A550 -> A140.");
    println!("  Their actual descriptor contents and runtime reachability are NOT established by these static E8 references.");
    let (startup_selectors,final_iterators)=unsafedisc_dll_bootstrap::audit_original(dll,&pe)?;
    println!("  Verified DllMain -> A140 bootstrap source-selection chain (read-only):");
    for s in &startup_selectors {
        println!("    category {}, selector {}: source directory {} (RVA 0x{:08X}, size {}) => early size guard {}; selector directory {} {:?}=0x{:08X} -> {}",
            s.category,s.selector,s.source_directory_index,s.source_rva,s.source_size,
            if s.passes_nonzero_size_guard {"PASS"} else {"ZERO -> EARLY EXIT"},
            s.directory_index,s.directory_field,
            s.candidate_rva,s.matching_section.as_deref().unwrap_or("none"));
    }
    let possible=startup_selectors.iter().filter(|s|s.passes_nonzero_size_guard).count();
    if possible!=3 { return Err("pinned DLL early bootstrap source-directory guards changed".into()); }
    println!("  Early-exit reachability: 3 of 5 helpers have nonzero source sizes; 2 abort before their category update.");
    println!("  A failed later section lookup does NOT rule out earlier registry updates.");
    println!("  Final A140 section-class iterators (ordered, read-only):");
    for walk in &final_iterators {
        println!("    category {}, selector {}: {} eligible section headers, {} directory-start exclusions -> {}",
            walk.category,walk.selector,walk.matching_first_stage,
            walk.skipped_directory_starts,
            walk.selected_section.as_deref().unwrap_or("none"));
    }
    println!("  Verified category registry call tree: A2A0 -> B850/B880/B8E0, with C2A0 allocation of a 12-byte node and C2F0 start/extent arithmetic.");
    println!("  These direct calls track range metadata; no original .txt plaintext or runtime write is inferred.");
    println!("  The .txt first match is a registered memory range candidate, NOT decrypted plaintext.");
    println!("  These are static selector outcomes for the exact original DLL, not runtime decoding proof.");
    println!("  Original DLL PE32 entry RVA: 0x{:08X} (on-disk code, NOT a memory dump).",
        pe.entry_rva);
    println!("  DLL PE sections:");
    for section in &pe.sections {
        let name = String::from_utf8_lossy(
            section.name.split(|&b| b == 0).next().unwrap_or(&[])
        );
        println!("    {}: RVA 0x{:08X}, raw +0x{:06X}, raw bytes {}, vsize {}, flags 0x{:08X}",
            name, section.virtual_address, section.raw_offset, section.raw_len,
            section.virtual_size, section.characteristics);
    }
    let nt = read_u32(dll, 0x3C)? as usize;
    let preferred_base = read_u32(dll, nt + 24 + 28)?;
    println!("  DLL preferred image base: 0x{preferred_base:08X}");
    let (reloc_blocks,_) = pe_relocation_blocks(dll,&pe)?;
    let protected_relocs=audit_protected_dll_relocations(
        dll,&pe,&reloc_blocks,preferred_base)?;
    println!("  On-disk encrypted .txt HIGHLOW relocation slots: {}",protected_relocs.slots);
    println!("    Absolute VA targets: .data {}, .rdata {}, .txt {}, other section {}, outside image {}",
        protected_relocs.targets_data,protected_relocs.targets_rdata,
        protected_relocs.targets_txt,protected_relocs.targets_other,
        protected_relocs.out_of_image);
    println!("    HIGHLOW DWORDs spanning a 4-KiB page: {}",protected_relocs.page_crossings);
    println!("    Protected .txt still needs decoding; relocation-backed VA values alone do not constitute decrypted x86.");
    if protected_relocs != (ProtectedDllRelocAudit {
        slots:214,targets_data:195,targets_rdata:18,targets_txt:1,
        targets_other:0,out_of_image:0,page_crossings:1
    }) {
        return Err("original protected DLL .txt relocation slot geometry differs from separately checked original".into());
    }
    match dll_tls_metadata(dll, &pe)? {
        None => println!("  PE TLS directory: absent (does not exclude other decoding mechanisms)."),
        Some(tls) => {
            println!("  PE TLS directory: RVA 0x{:08X}, size {}, AddressOfCallbacks VA 0x{:08X}",
                tls.directory_rva, tls.directory_size, tls.callbacks_va);
            println!("  TLS callback entries (before DLL entrypoint): {}", tls.callback_rvas.len());
            for (index, &callback_rva) in tls.callback_rvas.iter().enumerate() {
                println!("    TLS callback[{index}] VA 0x{:08X}, RVA 0x{callback_rva:08X}",
                    preferred_base.wrapping_add(callback_rva));
                print_dll_code_window(dll, &pe, "TLS callback raw code", callback_rva, 64)?;
            }
        }
    }
    print_dll_code_window(dll, &pe, "DLL entrypoint", pe.entry_rva, 96)?;
    // 0xFD6C is a relative CALL with destination 0xFC00 in the
    // user-verified original entrypoint bytes. The offset is NOT guessed
    // from a different title's DLL. These are bounded static excerpts.
    print_dll_code_window(dll, &pe, "DllMain startup helper (entrypoint CALL)", 0xFC00, 160)?;
    print_dll_code_window(dll, &pe, "DllMain branch continuation", 0xFD7E, 96)?;
    for (name, absolute_va) in [
        ("DllMain referenced pointer A", 0x90519Cu32),
        ("DllMain referenced pointer B", 0x906C70u32),
    ] {
        if let Some(rva) = absolute_va.checked_sub(preferred_base) {
            match dll_backed_rva_offset(&pe, rva, 4) {
                Ok(at) => {
                    let word = read_u32(dll, at)?;
                    println!("  {name}: VA 0x{absolute_va:08X}, RVA 0x{rva:08X}, raw +0x{at:06X}, on-disk DWORD 0x{word:08X}");
                }
                Err(_) => {
                    if let Some(section) = dll_zero_filled_section(&pe, rva, 4) {
                        let section_name = String::from_utf8_lossy(
                            section.name.split(|&b| b == 0).next().unwrap_or(&[]));
                        println!("  {name}: VA 0x{absolute_va:08X}, RVA 0x{rva:08X}, virtual zero-fill in section {section_name} (not backed on disk)");
                    } else {
                        println!("  {name}: VA 0x{absolute_va:08X}, RVA 0x{rva:08X}, not mapped to file-backed or virtual-only section");
                    }
                },
            }
        } else {
            println!("  {name}: VA 0x{absolute_va:08X} is below the preferred image base");
        }
    }
    println!("  NOTE: raw modifier bytes may be encrypted on disk or intended to be decoded at load time.");
    println!("  The historical tool dynamically loaded its DLL; PE execute flags alone do not imply valid raw x86.");
    for (name, rva) in ROUTINES {
        let offset = executable_rva_to_file_offset(&pe, rva, WINDOW)?;
        let block = dll.get(offset..offset+WINDOW)
            .ok_or("modifier excerpt exceeds verified original DLL")?;
        println!("  {name}: DLL RVA 0x{rva:08X}, raw file +0x{offset:06X}, 128-byte static excerpt");
        for (line, sixteen) in block.chunks(16).enumerate() {
            let at_rva = rva + (line * 16) as u32;
            let hex = sixteen.iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>().join(" ");
            println!("    0x{at_rva:08X}: {hex}");
        }
    }
    println!("  Unexpected privileged x86-like opcodes in raw excerpts do NOT prove executable code.");
    println!("  This report cannot establish whether or how the DLL loader decodes these ranges.");
    println!("  Not a disassembly, executable code validation, or decoded function.");
    println!("  Only the exact SHA-256 verified local DLL was read; no files or PE sections were changed.");
    Ok(())
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

// Independently count the original directory's ZERO ABSOLUTE entries
// versus nonzero ABSOLUTE entries. The historical 1.5.5 x86 checks
// whether the *whole* 16-bit word is zero and can therefore treat
// nonzero type-0 padding differently from a modern HIGHLOW parser.
// This reads only SHA-256-verified original on-disk ICD bytes.
fn historical_icd_reloc_word_profile(
    image:&[u8],pe:&Pe32,
)->Result<(unsafedisc_historical_155_reloc_builder::HistoricalRelocWordProfile,
            Vec<unsafedisc_historical_155_reloc_hazards::RelocHazardSite>)>{
    let nt=read_u32(image,0x3C)? as usize;
    let opt_at=nt.checked_add(24).ok_or("reloc profile optional header offset overflow")?;
    let opt_len=read_u16(image,nt+20)? as usize;
    let dir_offset=96+5*8;
    if opt_len<dir_offset+8 {
        return Err("reloc profile optional header lacks relocation directory".into());
    }
    let rva=read_u32(image,opt_at+dir_offset)?;
    let len=read_u32(image,opt_at+dir_offset+4)? as usize;
    if rva==0 || len==0 || len>1_048_576 {
        return Err("reloc profile directory missing or too large".into());
    }
    let section=pe.section(b".reloc")?;
    let relative=rva.checked_sub(section.virtual_address)
        .ok_or("reloc profile RVA before .reloc section")? as usize;
    let end_relative=relative.checked_add(len)
        .ok_or("reloc profile directory section span overflow")?;
    if end_relative>section.raw_len {
        return Err("reloc profile beyond original .reloc raw length".into());
    }
    let start=section.raw_offset.checked_add(relative)
        .ok_or("reloc profile raw offset overflow")?;
    let end=start.checked_add(len)
        .ok_or("reloc profile raw end overflow")?;
    let data=image.get(start..end)
        .ok_or("reloc profile exceeds original file")?;
    unsafedisc_historical_155_reloc_hazards::
        locate_historical_reloc_hazards(data,12)
        .map_err(|error|error.into())
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

// Historical unSafeDisc v1.5.5 original retail release, 20,480-byte
// unSafedisc.exe SHA-256 e484c9aa...f0f1eaa9f:
//  0x402257..0x4022B0: include sections unless the FIRST FOUR
//  literal header name bytes match one of these eight exclusions.
//  0x401F01: the separate checksum byte mixer runs only if the
//  selected section has IMAGE_SCN_CNT_CODE (0x20).
//  0x401F43..0x401F5B: for DLL size 0x21E00 (138,752)
//  bytes, its extra per-DWORD pre-TEA adapter is reached.
// This is supported by STATIC x86 disassembly and matches the
// authors' 1.5.4/1.5.5 notes about Crimson Skies .rdata/CRC.
// No executable is loaded and no decryptor is run.
fn historical_155_section_selected(sec: &PeSection) -> bool {
    let prefix = &sec.name[..4];
    ![
        b".rsr",b".bss",b"BSS\0",b".tls",
        b".ida",b".rda",b".eda",b".rel",
    ].iter().any(|excluded| prefix == &excluded[..])
}
fn historical_155_section_checksum_eligible(sec: &PeSection) -> bool {
    historical_155_section_selected(sec) && sec.characteristics & 0x20 != 0
}
// Independently read the exact original ICD's 40-byte section records
// and cross-check the newly reconstructed 1.5.5 assembler name filter
// against the already-established four-byte selection helper.
// The exact-hash gate for ICD has run before this function is called.
fn audit_original_icd_historical_section_list(
    original:&[u8],pe:&Pe32,
)->Result<(Vec<usize>,usize)>{
    let nt=read_u32(original,0x3C)? as usize;
    let optional_len=read_u16(original,nt+20)? as usize;
    let table=nt.checked_add(24)
        .and_then(|n|n.checked_add(optional_len))
        .ok_or("original ICD section table header overflow")?;
    let count=pe.sections.len();
    let size=count.checked_mul(40)
        .ok_or("original ICD section table length overflow")?;
    let end=table.checked_add(size)
        .ok_or("original ICD section table end overflow")?;
    let bytes=original.get(table..end)
        .ok_or("original ICD section headers beyond file")?;
    let selected=unsafedisc_historical_155::
        historical_155_selected_section_indices(bytes,count)?;
    for (i,section) in pe.sections.iter().enumerate(){
        let selected_by_new=selected.contains(&i);
        let selected_by_existing=historical_155_section_selected(section);
        if selected_by_new!=selected_by_existing {
            return Err(format!(
                "original ICD section #{i} disagrees with 1.5.5 name-filter audit"
            ));
        }
        if bytes[i*40..i*40+8]!=section.name {
            return Err(format!(
                "original ICD section #{i} metadata differs between PE parser and raw records"
            ));
        }
    }
    Ok((selected,count))
}

fn print_historical_155_section_filter(pe: &Pe32) {
    println!("Original unSafeDisc 1.5.5 section-name/CRC filter (STATIC original x86):");
    println!("  Source: historic 2000 executable, SHA-256 e484c9aa...f0f1eaa9f.");
    println!("  Filename filtering uses exactly four section-name bytes (code RVA 0x2257).");
    println!("  Checksum selection additionally requires IMAGE_SCN_CNT_CODE=0x20 (code RVA 0x1F01).");
    for sec in &pe.sections {
        let name=String::from_utf8_lossy(sec.name.split(|&b|b==0).next().unwrap_or(&[]));
        let included=historical_155_section_selected(sec);
        let crc=historical_155_section_checksum_eligible(sec);
        let backed=sec.raw_len.min(if sec.virtual_size == 0 {
            sec.raw_len
        } else {sec.virtual_size as usize});
        println!("  {:8}: processing {}, CODE checksum {}, on-disk selected extent {} bytes, flags 0x{:08X}",
            name,if included {"YES"} else {"no"},
            if crc {"YES"} else {"no"},backed,sec.characteristics);
    }
    println!("  Selection is reproduced from the supplied original unSafeDisc assembly, not an executed trace.");
    println!("  This research mode does NOT perform section decryption, DLL API calls or a final CRC.");
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
    modifier_research: bool,
    modifier_snapshot: Option<PathBuf>,
    startup_research: bool,
    attach_research: bool,
    xref_research: bool,
    literal_research: bool,
    reference_audit: bool,
    historical_selection: bool,
    strings: bool,
}

fn help() {
    println!("unSafeDisc Linux research port (native Rust, initial read-only stage)\n\
Usage:\n\
  unsafedisc-linux inspect --loader CRIMSON.EXE --icd CRIMSON.ICD \\\n\
      --dplayerx dplayerx.dll [--reference TESTME.EXE] [--reported-key 0x7496538F] [--layout] [--checksum-research] [--sections] [--key-research] [--modifier-research] [--modifier-snapshot CS98-DLL-00.BIN] [--startup-research] [--attach-research] [--xref-research] [--literal-research] [--reference-audit] [--historical-selection] [--strings]\n\
  unsafedisc-linux inspect-snapshot --snapshot CS98-DLL-00.BIN\n\\
  unsafedisc-linux inspect-historical --unsafedisc unSafedisc.exe\n\
  unsafedisc-linux --help\n\
\n\
The --key-research first word defaults to the exact-version 0x7496538F;\n\
use --reported-key only to override it for controlled experiments.\n\
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
        if key == "--modifier-research" {
            if options.modifier_research { return Err("repeated --modifier-research argument".into()); }
            options.modifier_research = true;
            continue;
        }
        if key == "--startup-research" {
            if options.startup_research { return Err("repeated --startup-research argument".into()); }
            options.startup_research = true;
            continue;
        }
        if key == "--attach-research" {
            if options.attach_research { return Err("repeated --attach-research argument".into()); }
            options.attach_research = true;
            continue;
        }
        if key == "--xref-research" {
            if options.xref_research { return Err("repeated --xref-research argument".into()); }
            options.xref_research = true;
            continue;
        }
        if key == "--literal-research" {
            if options.literal_research { return Err("repeated --literal-research argument".into()); }
            options.literal_research = true;
            continue;
        }
        if key == "--reference-audit" {
            if options.reference_audit { return Err("repeated --reference-audit argument".into()); }
            options.reference_audit = true;
            continue;
        }
        if key == "--historical-selection" {
            if options.historical_selection { return Err("repeated --historical-selection argument".into()); }
            options.historical_selection = true;
            continue;
        }
        if key == "--strings" {
            if options.strings { return Err("repeated --strings argument".into()); }
            options.strings = true;
            continue;
        }
        let value = args.next().ok_or_else(|| format!("missing value for {key}"))?;
        match key.as_str() {
            "--loader" if options.loader.is_none() => options.loader = Some(PathBuf::from(value)),
            "--icd" if options.icd.is_none() => options.icd = Some(PathBuf::from(value)),
            "--dplayerx" if options.dplayerx.is_none() => options.dplayerx = Some(PathBuf::from(value)),
            "--reference" if options.reference.is_none() => options.reference = Some(PathBuf::from(value)),
            "--reported-key" if options.reported_key.is_none() => options.reported_key = Some(parse_hex_u32(&value)?),
            "--modifier-snapshot" if options.modifier_snapshot.is_none() => options.modifier_snapshot = Some(PathBuf::from(value)),
            _ => return Err(format!("unrecognized or repeated argument: {key}")),
        }
    }
    if options.loader.is_none() || options.icd.is_none() || options.dplayerx.is_none() {
        return Err("--loader, --icd, and --dplayerx are all required".into());
    }
    if options.key_research && options.reported_key.is_none() {
        // This is a per-title, per-version exact-hash gated constant, now
        // independently checked against the original known-plaintext tails.
        // The optional --reported-key override remains available for tests.
        options.reported_key=Some(REPORTED_FIRST_WORD);
    }
    if options.reference_audit && options.reference.is_none() {
        return Err("--reference-audit requires --reference TESTME.EXE".into());
    }
    if options.modifier_snapshot.is_some() && (!options.key_research || options.reference.is_none()) {
        return Err("--modifier-snapshot requires --key-research and --reference TESTME.EXE".into());
    }
    Ok(Some(options))
}

fn inspect(options: Options) -> Result<()> {
    let captured_modifiers=options.modifier_snapshot.as_ref()
        .map(|p|load_verified_modifier_snapshot(p.as_path()))
        .transpose()?;
    let loader = read_original(options.loader.as_ref().unwrap(), LOADER_LEN, LOADER_SHA)?;
    let icd = read_original(options.icd.as_ref().unwrap(), ICD_LEN, ICD_SHA)?;
    let dll = read_original(options.dplayerx.as_ref().unwrap(), DLL_LEN, DLL_SHA)?;
    if options.strings {
        let pe = parse_pe32(&dll)?;
        let section = pe.section(b".data")?;
        let end = section.raw_offset.checked_add(section.raw_len)
            .ok_or("original DLL data file extent overflow")?;
        let bytes = dll.get(section.raw_offset..end)
            .ok_or("original DLL data extent outside file")?;
        let entries = unsafedisc_strings::audit_original(bytes, section.virtual_address)?;
        println!("Original DLL encoded metadata: {} verified printable labels", entries.len());
        for item in &entries {
            println!("  RVA 0x{:08X}: {}", item.rva, item.value);
        }
        let code = pe.section(b".text")?;
        let code_end=code.raw_offset.checked_add(code.raw_len)
            .ok_or("DLL code raw extent overflow")?;
        let code_bytes=dll.get(code.raw_offset..code_end)
            .ok_or("DLL code raw extent outside file")?;
        let nt=read_u32(&dll,0x3C)? as usize;
        let base=read_u32(&dll,nt+24+28)?;
        let references=unsafedisc_strings::scan_push_references(
            code_bytes,code.virtual_address,base,&entries)?;
        if references.len()!=23 {
            return Err(format!("unexpected original DLL encoded-metadata PUSH reference count {}",references.len()));
        }
        // The immediate operand in a genuine absolute PUSH is the DWORD
        // relocated by IMAGE_REL_BASED_HIGHLOW, at (opcode RVA + 1).
        // Cross-check candidates against the original PE relocation directory.
        // Literal matches without this evidence remain visible but weaker;
        // even relocation-backed matches do not establish instruction reachability.
        let (dll_relocs, _) = pe_relocation_blocks(&dll, &pe)?;
        let mut relocation_sites = std::collections::HashSet::new();
        for block in &dll_relocs {
            for &offset in &block.highlow_offsets {
                let site = block.page_rva.checked_add(u32::from(offset))
                    .ok_or("DLL HIGHLOW RVA overflow")?;
                relocation_sites.insert(site);
            }
        }
        let flags = unsafedisc_strings::push_operand_highlow_flags(
            &references, &relocation_sites)?;
        let backed = flags.iter().filter(|&&is_highlow| is_highlow).count();
        println!("  Static .text PUSH-imm32 candidate byte patterns: {}",references.len());
        println!("    immediate operands backed by PE HIGHLOW: {backed}; no matching operand HIGHLOW: {}",
            references.len() - backed);
        for (reference, &is_highlow) in references.iter().zip(flags.iter()) {
            let label=entries.iter().find(|e|e.rva==reference.label_rva)
                .ok_or("decoded PUSH references unknown metadata")?;
            let evidence = if is_highlow { "HIGHLOW-backed" } else { "raw-byte candidate only" };
            println!("    x86 PUSH RVA 0x{:08X} -> data RVA 0x{:08X}: {} [{}]",
                reference.instruction_rva,reference.label_rva,label.value,evidence);
        }
        println!("  A matching operand relocation strengthens an absolute-address interpretation.");
        println!("  Scanned byte patterns do not prove instruction alignment, reachability or execution.");
        // Invert the search: inspect all genuine base-relocation DWORDs
        // in unencrypted, file-backed .text and .txt2. This may identify
        // address references using opcodes other than PUSH imm32, without
        // guessing instruction boundaries in x86 overlapping code.
        println!("  Relocation-backed metadata pointer survey (all backed HIGHLOW slots):");
        for name in [b".text".as_slice(), b".txt2".as_slice()] {
            let section = pe.section(name)?;
            let raw_end = section.raw_offset.checked_add(section.raw_len)
                .ok_or("DLL relocated section raw end overflow")?;
            let source = dll.get(section.raw_offset..raw_end)
                .ok_or("DLL relocated section bytes outside file")?;
            let matches = unsafedisc_strings::scan_relocated_label_pointers(
                source, section.virtual_address, base, &entries, &relocation_sites)?;
            let name = std::str::from_utf8(name).map_err(|_| "invalid static section name")?;
            println!("    {name}: {} metadata-address relocation slots", matches.len());
            for hit in matches.iter().take(128) {
                let label=entries.iter().find(|e| e.rva==hit.label_rva)
                    .ok_or("relocated pointer refers to unknown metadata")?;
                let prev=hit.previous_byte.map(|b| format!("{b:02X}"))
                    .unwrap_or_else(|| "n/a".to_string());
                println!("      operand RVA 0x{:08X} -> data RVA 0x{:08X}: {} (preceding byte {prev})",
                    hit.operand_rva,hit.label_rva,label.value);
            }
            if matches.len() > 128 {
                println!("      (first 128 slots shown; full count above)");
            }
        }
        println!("  This scan recognizes relocatable pointer data, not reached instructions.");

        // A much stricter follow-up matches the complete original
        // named-API resolver sequence: three independently HIGHLOW-
        // relocated operands, E8 to the same routine C708, then a
        // write + null-check to the SAME originally zero-filled .data
        // DWORD slot. These are bounded static bytes, never executed.
        let slots=unsafedisc_strings::scan_resolver_slots(
            code_bytes,code.virtual_address,base,&entries,&relocation_sites,0xC708)?;
        const EXPECTED: [&str;13]=[
            "ReadProcessMemory","WriteProcessMemory","VirtualProtect",
            "CreateProcessA","CreateProcessW","GetStartupInfoA",
            "GetStartupInfoW","GetSystemTime","GetSystemTimeAsFileTime",
            "TerminateProcess","Sleep","SendASPI32Command",
            "GetASPI32SupportInfo"
        ];
        if slots.len()!=EXPECTED.len(){
            return Err(format!("original DLL resolver-slot sequence count {} differs from expected {}",
                slots.len(),EXPECTED.len()));
        }
        let raw_end=section.virtual_address.checked_add(section.raw_len as u32)
            .ok_or("DLL file-backed data extent overflow")?;
        let virtual_end=section.virtual_address.checked_add(section.virtual_size)
            .ok_or("DLL virtual data extent overflow")?;
        println!("  Verified C708 resolver-style x86 sequences: {} (three independent HIGHLOW pointers each)",slots.len());
        for (index,(slot,expected)) in slots.iter().zip(EXPECTED).enumerate(){
            let label=entries.iter().find(|l|l.rva==slot.label_rva)
                .ok_or("original resolver referenced unknown decoded string")?;
            let expected_slot=0x24EF8u32.checked_add((index as u32)*4)
                .ok_or("resolver slot index overflow")?;
            if label.value!=expected || slot.output_slot_rva!=expected_slot {
                return Err(format!("original resolver slot mismatch for {expected} at index {index}"));
            }
            let end=slot.output_slot_rva.checked_add(4)
                .ok_or("resolver slot upper bound overflow")?;
            if slot.output_slot_rva<raw_end || end>virtual_end {
                return Err("original resolved-API pointer slot is not wholly in virtual-only .data".into());
            }
            println!("    {}: label RVA 0x{:X}, PUSH 0x{:X}, CALL C708 at 0x{:X}, virtual-only storage RVA 0x{:X}",
                label.value,slot.label_rva,slot.instruction_rva,slot.call_rva,slot.output_slot_rva);
        }
        println!("  All 13 destinations form a contiguous virtual-only .data pointer table (4-byte spacing).");
        println!("  E8 call encoding and real PE HIGHLOW metadata support these STATIC resolver candidates.");
        println!("  Actual execution and whether the C708 routine returns GetProcAddress results are not yet established.");
        println!("No DLL code was executed or decrypted.");
    }
    if options.modifier_research {
        let loader_profile=unsafedisc_dll_bootstrap::audit_original_loader_header(&loader)?;
        println!("  Separate main-executable header profile: original on-disk CRIMSON.EXE, NOT reconstructed CRIMSON.ICD or observed runtime memory.");
        println!("    Original loader A140 source-directory size guards: {:?} (IAT, imports, resources, exports, TLS).",
            loader_profile.size_guard_eligible);
        for walk in &loader_profile.class_iterators {
            println!("    Original loader category {}: {} candidate section(s), {} filtered directory-start section(s), initial selection {}.",
                walk.category,walk.matching_first_stage,walk.skipped_directory_starts,
                walk.selected_section.as_deref().unwrap_or("none"));
        }
        print_modifier_research(&dll)?;
    }
    if options.startup_research {
        print_dll_startup_calls(&dll)?;
    }
    if options.attach_research {
        print_dll_attach_research(&dll)?;
    }
    if options.xref_research {
        print_dll_protected_xrefs(&dll)?;
    }
    if options.literal_research {
        print_dll_literal_research(&dll)?;
    }
    let source_pe = parse_pe32(&loader)?;
    let protected_pe = parse_pe32(&icd)?;
    let (relocs, reloc_padding) = pe_relocation_blocks(&icd, &protected_pe)?;
    if options.historical_selection || options.checksum_research || options.key_research {
        let (historic,hazard_sites)=historical_icd_reloc_word_profile(&icd,&protected_pe)?;
        let modern_highlow:usize=relocs.iter()
            .map(|block|block.highlow_offsets.len()).sum();
        if historic.highlow_words!=modern_highlow ||
           historic.zero_padding_words+historic.nonzero_type_zero_words!=reloc_padding ||
           historic.blocks!=relocs.len() {
            return Err("independent historical and PE relocation WORD profile disagree".into());
        }
        println!("Original ICD historical relocation WORD audit (read-only, no DLL execution):");
        println!("  {} PE relocation blocks; {} HIGHLOW WORDs ({} at page offset zero, including {} noninitial instances); {} zero ABSOLUTE words; {} nonzero type-0 ABSOLUTE words; {} odd-sized WORD blocks; {} empty-entry blocks; {} zero padding WORDs preceding further entries; {} all-zero nonempty relocation blocks.",
            historic.blocks,historic.highlow_words,historic.highlow_page_start_words,
            historic.highlow_page_start_not_first_words,
            historic.zero_padding_words,historic.nonzero_type_zero_words,
            historic.odd_word_blocks,historic.empty_word_blocks,
            historic.nonterminal_zero_padding_words,historic.zero_only_blocks);
        println!("  Public historical unSafeDisc 1.5.5 checks entire relocation WORD for zero, then masks low 12 bits. Native HIGHLOW-only checksum hypotheses may differ if nonzero type-0 entries exist.");
        if hazard_sites.is_empty() {
            println!("  No historical relocation hazard sites found in metadata (the checksum and private modifier remain unverified).");
        } else {
            println!("  First {} historical relocation hazard locations (metadata only, no protected bytes):",hazard_sites.len());
            for site in &hazard_sites {
                let where_word=site.entry_index.map(|i|format!("entry [{}]",i))
                    .unwrap_or_else(||"whole block".to_string());
                let word=site.raw_word.map(|w|format!(", word=0x{w:04X}"))
                    .unwrap_or_default();
                println!("    PE block page RVA=0x{:08X}, directory +0x{:X}, {}: {}{}",
                    site.block_page_rva,site.block_directory_offset,where_word,
                    site.kind.label(),word);
            }
            println!("  Listing is capped at 12 sites; summary counts above cover the entire verified relocation directory.");
        }
        if historic.odd_word_blocks!=0 || historic.empty_word_blocks!=0 {
            println!("  WARNING: the archived relocation walker uses an unconditional initial read and a LOOP; odd/empty blocks can cause unsafe accesses or loop-wrap. No such reads are attempted.");
        }
        if options.key_research {
            // Necessary only, not sufficient: matching PE metadata
            // does not establish the runtime checksum, adapter or TEA key.
            unsafedisc_historical_155_reloc_builder::
                necessary_highlow_only_candidate_preflight(
                    historic,modern_highlow,reloc_padding
                )?;
            println!("  HIGHLOW-only key-candidate necessary preflight PASSED (not proof of historical checksum equivalence).");
        } else {
            if historic.nonzero_type_zero_words>0 {
                println!("  WARNING: nonzero type-0 words are excluded by native HIGHLOW checksum research, but not by the archived 1.5.5 cutpoint builder.");
            }
            if historic.nonterminal_zero_padding_words>0 {
                println!("  WARNING: zero padding before later relocation WORDs can terminate the historical per-block consumer before modern HIGHLOW parsing does; key-candidate equivalence is NOT established.");
            }
            if historic.highlow_page_start_not_first_words>0 {
                println!("  WARNING: noninitial HIGHLOW at page offset zero bypasses the audited first-entry 401E16 special case; historical checksum equivalence is NOT established.");
            }
            if historic.highlow_page_start_words>0 {
                println!("  NOTE: initial 0x3000 HIGHLOW is deliberately handled by the historical 401E16->401E28 branch; it alone is not a preflight failure.");
            }
            if historic.zero_only_blocks>0 {
                println!("  WARNING: an all-zero relocation block may affect historical 401E14 offset-zero handling; it is not safely equivalent to an empty HIGHLOW block.");
            }
        }
    }
    if options.historical_selection {
        let (selected,total)=audit_original_icd_historical_section_list(
            &icd,&protected_pe)?;
        println!("Independent original-ICD PE header list: {}/{} accepted section names in source order; both native filters agree.",
            selected.len(),total);
        print_historical_155_section_filter(&protected_pe);
        let (word_count,tail_count)=
            unsafedisc_historical_155::original_complete_dwords_and_tail(4095);
        println!("  Historical 1.5.5 modifier inner loop: full DWORDs only (example: 4095 bytes => {word_count} words, {tail_count} untouched by THIS layer).");
        println!("  Original 401F96 CALL EDX uses DLL base + 0x65BE; 401F9A ADD EDX,0x63 and 401F9D CALL EDX reuse EDX after first modifier.");
        println!("  Original 401F98 XOR EAX,ECX reuses ECX as the index mask after first modifier; both ECX and EDX are caller-saved on Win32 x86.");
        println!("  Thus the usual F2(F1(input XOR mask) XOR mask) model and second RVA 0x6621 are conditional on the FIRST modifier preserving ECX and EDX (and EBX/ESI/EDI for loop progress).");
        println!("  Source: publicly archived version.dll-adapted unSafeDisc 1.5.5, core CODE at 0x401F62..0x401FA7; private original executable was NOT rechecked here.");
    }
    if options.key_research {audit_original_pretea_fragments(&protected_pe,&relocs)?;}
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
    println!("Full TEA key needs independent TESTME.EXE plaintext-tail verification; the title-specific DLL modifier stage remains unrecovered.");
    let mut first_word_reference_matches: Option<usize> = None;
    let mut verified_reference: Option<Vec<u8>> = None;
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
        if decoded_relocs.0.as_slice() != relocs.as_slice() || decoded_relocs.1 != reloc_padding {
            return Err("unwrapped reference relocation entries differ from original ICD".into());
        }
        println!("Original ICD and TESTME.EXE relocation tables match.");

        if decoded_pe.entry_rva != protected_pe.entry_rva {
            return Err("original ICD and unwrapped reference entrypoints differ".into());
        }
        println!("Verified user-supplied TESTME.EXE reference (exact known hash).");
        if options.reference_audit {
            print_reference_audit(&icd,&reference,&protected_pe,&decoded_pe,&relocs)?;
        }
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
        verified_reference=Some(reference);
    } else if options.reported_key.is_some() {
        println!("To verify a reported key, also provide the local --reference TESTME.EXE.");
    }
    if let Some((checksum, key)) = tea_candidate {
        println!("SafeDisc TEA key reconstruction (read-only):");
        println!("  first word: 0x{:08X} (exact-version default or --reported-key override)", key[0]);
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
            println!("  TEA word[{i}] = 0x{word:08X}");
        }
        if let Some(ref_bytes)=verified_reference.as_deref() {
            let reference_pe=parse_pe32(ref_bytes)?;
            let proof=validate_tea_key_by_unchanged_fragment_tails(
                &icd,ref_bytes,&protected_pe,&reference_pe,&relocs,key)?;
            println!("  Independent key oracle: {} / {} exact non-DWORD fragment tails; {} / {} identical tail bytes.",
                proof.matching_fragments,proof.fragments,proof.matching_bytes,proof.checked_bytes);
            if proof.fragments!=28_947 || proof.checked_bytes!=58_076 ||
                proof.matching_fragments!=proof.fragments ||
                proof.matching_bytes!=proof.checked_bytes {
                return Err("reconstructed TEA key FAILED exact original/reference tail oracle; no full-key validation".into());
            }
            println!("  CONFIRMED: the four TEA key words and overlapping block order match 58,076 independently checkable original bytes.");
            // Falsifiability control: for the exact same source/reference
            // data, toggling only one bit of the fourth TEA key word must
            // destroy essentially all agreement. This also rules out a
            // comparison accidentally dominated by key-independent bytes.
            let mut wrong_key=key;
            wrong_key[3]^=1;
            let negative=validate_tea_key_by_unchanged_fragment_tails(
                &icd,ref_bytes,&protected_pe,&reference_pe,&relocs,wrong_key)?;
            println!("  TEA one-bit negative control: {} / {} exact tails; {} / {} equal bytes.",
                negative.matching_fragments,negative.fragments,
                negative.matching_bytes,negative.checked_bytes);
            if negative.fragments!=28_947 || negative.checked_bytes!=58_076 ||
               negative.matching_fragments!=39 || negative.matching_bytes!=177 {
                return Err("TEA one-bit negative control differs from independent original/reference result".into());
            }

            let (composite,algebra,indexed_affine,mask_alias,second_stage,first_stage)=inspect_original_modifier_reference_pairs(
                &icd,ref_bytes,&protected_pe,&reference_pe,&relocs,key,captured_modifiers.as_ref())?;
            println!("  Additional extra-layer audit: {} / {} unchanged fragment-tail bytes.",
                composite.matching_tail_bytes,composite.checked_tail_bytes);
            println!("  Composite EAX pair observations at index=0: {}; unique input/output DWORDs: {} / {}; contradictions: {} forward, {} reverse.",
                composite.modifier_fragments,composite.distinct_inputs,
                composite.distinct_outputs,composite.input_conflicts,composite.output_conflicts);
            println!("  Index-aware extra-layer observations: {} words; {} unique (index,masked-input) pairs; {} repeated, {} forward contradictions, {} reverse collisions.",
                composite.modified_dwords,composite.indexed_distinct_inputs,
                composite.indexed_repeated_observations,
                composite.indexed_forward_conflicts,composite.indexed_reverse_conflicts);
            println!("  Maximum observed fragment-local DWORD index: {} (original x86 full-width mask).",
                composite.max_index);
            if composite!=(ModifierReferenceAudit{
                total_fragments:59_622,modifier_fragments:48_149,
                modified_dwords:440_422,
                checked_tail_bytes:93_801,matching_tail_bytes:93_801,
                distinct_inputs:31_799,distinct_outputs:31_799,
                input_conflicts:0,output_conflicts:0,
                indexed_distinct_inputs:361_480,indexed_distinct_outputs:361_480,
                indexed_repeated_observations:78_942,
                indexed_forward_conflicts:0,indexed_reverse_conflicts:0,
                max_index:1006,
            }){
                return Err("independent title-specific composite modifier observations differ from verified reference".into());
            }
            println!("  Algebraic probe (index=0; {} distinct reference-backed pairs): GF(2) affine input rank {}/32; {} contradicting samples.",
                algebra.pairs,algebra.affine_input_rank,algebra.affine_violations);
            println!("  Candidate families passing every observed pair: rotate(x,r)+k: {} of 32; rotate(x+k,r): {} of 32.",
                algebra.rotate_then_add_matches,algebra.add_then_rotate_matches);
            println!("  Indexed GF(2) two-modifier model over {} distinct observations: joint rank {}/64; {} contradicting samples.",
                indexed_affine.samples,indexed_affine.feature_rank,indexed_affine.contradictory_samples);
            println!("  Equal-mask alias test: {} masks ({} shared by distinct word indices), {} canonical (mask,input) pairs, {} cross-index repeats.",
                mask_alias.distinct_masks,mask_alias.masks_with_multiple_indices,
                mask_alias.distinct_masked_input_pairs,mask_alias.repeated_masked_input_pairs);
            println!("  Canonical-mask violations: {} forward inconsistencies, {} inverse collisions.",
                mask_alias.forward_conflicts,mask_alias.inverse_conflicts);
            if mask_alias.forward_conflicts!=0 || mask_alias.inverse_conflicts!=0 {
                println!("  WARNING: this new mask-alias hypothesis disagrees with observations; do NOT infer title-specific modifier recovery.");
            }
            let historic_mask_rank=unsafedisc_second_stage::original_mask_rank(1006);
            println!("  Full original 0..1006 index-mask GF(2) rank: {historic_mask_rank}/32 (joint rank ceiling: {}/64).",
                32+historic_mask_rank);
            if second_stage.differential_rank>historic_mask_rank {
                return Err("retail/reference F2 mask-difference rank exceeds original-mask mathematical ceiling".into());
            }
            println!("  Differential mask subspace coverage: {}/{} independent original-mask dimensions; {} unobserved independent mask dimensions. This is an observation-coverage bound, NOT a recovered F2.",
                second_stage.differential_rank,historic_mask_rank,
                historic_mask_rank-second_stage.differential_rank);
            println!("  F2-only affine differential test: {} shared inputs spanning different masks, {} relations, rank {}/32, {} contradictions.",
                second_stage.multi_mask_inputs,second_stage.differential_observations,
                second_stage.differential_rank,second_stage.differential_contradictions);
            println!("  F2-only conditional injective-affine rank check: observed mask-delta rank {}, output-delta rank {}, rank deficit {}. A deficit rules out a bijective affine second modifier IF the observed composite semantics and input/output samples are correct.",
                second_stage.differential_rank,second_stage.output_differential_rank,
                second_stage.injective_affine_rank_deficit);
            if second_stage.differential_contradictions!=0 {
                println!("  NOTE: contradictions already rule out the affine-F2 hypothesis; the conditional injectivity rank check is not independently interpretable.");
            }
            println!("  F2-only canonicalization: {} observations, {} conflicts, {} distinct observed masks.",
                second_stage.canonical_observations,second_stage.conflicting_canonical_pairs,
                second_stage.distinct_masks_observed);
            println!("  F2-only injectivity probe: {} same-input output collisions across different masks (advisory).",
                second_stage.distinct_mask_output_collisions);
            println!("  F2-only directly compared differences: {} distinct mask deltas, {} repeated classes, {} classes with contradictory output deltas.",
                second_stage.distinct_mask_difference_classes,
                second_stage.repeated_mask_difference_classes,
                second_stage.conflicting_mask_difference_classes);
            if second_stage.differential_observations==0 {
                println!("  F2-only audit is INCONCLUSIVE: no input recurs under different masks.");
            }
            if second_stage.conflicting_canonical_pairs!=0 {
                println!("  WARNING: F2-only audit cannot be trusted until conflicting canonical pairs are independently explained.");
            }
            // Verified exclusively on the pinned game/reference SHA-256
            // pair. These expected aggregate values were independently
            // cross-checked offline, without preserving proprietary
            // per-word observations in the repository.
            if algebra!=(unsafedisc_algebra::AlgebraAudit{
                pairs:31_799,affine_input_rank:32,affine_violations:31_766,
                rotate_then_add_matches:0,add_then_rotate_matches:0,
            }) {
                return Err("pinned retail/reference index-zero algebraic audit signature mismatch".into());
            }
            if indexed_affine!=(unsafedisc_indexed_affine::IndexedAffineAudit{
                samples:361_480,feature_rank:42,contradictory_samples:361_437,
            }) {
                return Err("pinned retail/reference joint GF(2) audit signature mismatch".into());
            }
            if mask_alias!=(unsafedisc_indexed_affine::MaskAliasAudit{
                samples:361_480,distinct_masks:572,
                masks_with_multiple_indices:312,
                distinct_masked_input_pairs:361_346,
                repeated_masked_input_pairs:134,
                forward_conflicts:0,inverse_conflicts:0,
            }) {
                return Err("pinned retail/reference original-mask alias audit signature mismatch".into());
            }
            if second_stage!=(unsafedisc_second_stage::SecondStageAudit{
                raw_observations:361_480,canonical_observations:361_346,
                conflicting_canonical_pairs:0,multi_mask_inputs:13,
                differential_observations:13,differential_rank:8,
                differential_contradictions:5,
                distinct_mask_difference_classes:12,
                repeated_mask_difference_classes:1,
                conflicting_mask_difference_classes:1,
                distinct_mask_output_collisions:0,
                distinct_masks_observed:572,
                // The two NEW output-rank fields are not claimed as
                // historical pinned signatures until verified against
                // user-owned retail/reference raw data. Synthetic
                // tests independently exercise their exact meaning.
                ..second_stage
            }) {
                return Err("pinned retail/reference F2-only audit signature mismatch".into());
            }
            println!("  CONFIRMED: all four title-specific aggregate research signatures match the independent original/reference audit.");
            println!("  Conditional F1 XOR graph (requires injective F2): {} equal-output groups; {} relative XOR edges; {} vertices; {} components; largest {}.",
                first_stage.multi_context_outputs,first_stage.xor_relations,first_stage.graph_vertices,
                first_stage.graph_components,first_stage.largest_component);
            println!("  Conditional F1 graph cycles: {} consistent, {} contradictory; F1-affinity ranks input {}/32, augmented {}/64 (obstruction rank {}).",
                first_stage.compatible_cycles,first_stage.contradictory_cycles,
                first_stage.affine_input_rank,first_stage.augmented_rank,
                first_stage.augmented_rank-first_stage.affine_input_rank);
            println!("  Conditional F1 direct XOR differences: {} repeated input-delta classes, {} incompatible mask-delta classes.",
                first_stage.repeated_input_delta_classes,first_stage.contradictory_input_delta_classes);
            println!("  Conditional F1 modular-rotation screening (LSB necessary conditions only): rotl(x,r)+k {} / 32; rotl(x+k,r) {} / 32.",
                first_stage.rotate_then_add_lsb_candidates,
                first_stage.add_then_rotate_lsb_candidates);
            if first_stage!=(unsafedisc_first_stage::FirstStageAudit{
                raw_observations:361_480,canonical_observations:361_346,
                canonical_conflicts:0,multi_context_outputs:11_857,
                xor_relations:25_168,graph_vertices:37_025,
                graph_components:11_857,largest_component:140,
                compatible_cycles:0,contradictory_cycles:0,
                affine_input_rank:32,augmented_rank:42,
                repeated_input_delta_classes:299,
                contradictory_input_delta_classes:52,
                rotate_then_add_lsb_candidates:0,
                add_then_rotate_lsb_candidates:0,
            }) {
                return Err("pinned retail/reference F1-only equal-output graph signature mismatch".into());
            }
            println!("  F1-only constraints are conditional: injectivity of F2 on unseen values remains UNPROVEN.");
            println!("  F2-only test makes NO affine assumption about the still-unknown F1.");
            println!("  HISTORICAL x86 CONTRACT CAVEAT: this F2-only mathematical model assumes the FIRST encrypted DLL modifier preserves ECX (mask) and EDX (second CALL target).");
            println!("  The first helper's register effects are NOT yet recovered; therefore 13/8/5 differential contradictions reject an affine F2 under that ASSUMPTION, not unconditionally for the real DLL.");
            println!("  This 64-feature test is a necessary condition only if BOTH original EAX modifiers are bit-affine.");
            println!("  A fitted hypothesis describes ONLY the observed pairs, not unseen inputs or the two original EAX functions.");
            println!("  CONFIRMED: the first-DWORD composite modifier acts consistently and injectively on 31,799 observed inputs.");
            println!("  These are observed pairs only; the two protected DLL functions are NOT yet reconstructed.");
            println!("  This validates TEA key material, NOT the missing title-specific EAX modifier implementation.");
        } else {
            println!("  Key is only provisional: provide --reference TESTME.EXE to independently validate 128 bits.");
        }
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
// The full four words are independently validated via the known-reference
// untouched-tail oracle, even while the title-specific modifiers are unknown.
// Without the verified reference, the same formula remains a candidate.
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

#[derive(Debug,PartialEq,Eq)]
struct PreTeaFragmentGeometry {
    highlow_slots:usize,
    highlow_bytes:usize,
    modified_dwords:usize,
    skipped_tail_bytes:usize,
    calls:usize,
    unaligned_calls:usize,
    unaligned_dwords:usize,
}
fn pretea_fragment_geometry(extent:usize, highlow_offsets:&[usize])->Result<PreTeaFragmentGeometry> {
    let mut marked=vec![false;extent];
    let mut previous_end=0usize;
    for &start in highlow_offsets {
        let end=start.checked_add(4).ok_or("HIGHLOW position overflow")?;
        if start<previous_end || end>extent {
            return Err("unsorted, overlapping, or out-of-extent HIGHLOW offset".into());
        }
        marked[start..end].fill(true);
        previous_end=end;
    }
    let mut out=PreTeaFragmentGeometry{
        highlow_slots:highlow_offsets.len(),
        highlow_bytes:highlow_offsets.len().checked_mul(4)
            .ok_or("HIGHLOW total overflow")?,
        modified_dwords:0,skipped_tail_bytes:0,calls:0,
        unaligned_calls:0,unaligned_dwords:0,
    };
    for page in marked.chunks(4096){
        let mut pos=0usize;
        while pos<page.len(){
            if page[pos]{pos+=1;continue;}
            let begin=pos;
            while pos<page.len() && !page[pos]{pos+=1;}
            let len=pos-begin;
            if len>=4 {
                out.calls+=1;
                out.modified_dwords+=len/4;
                if begin%4!=0 {
                    out.unaligned_calls+=1;
                    out.unaligned_dwords+=len/4;
                }
            }
            out.skipped_tail_bytes+=len%4;
        }
    }
    let total=out.modified_dwords.checked_mul(4)
        .and_then(|n|n.checked_add(out.highlow_bytes))
        .and_then(|n|n.checked_add(out.skipped_tail_bytes))
        .ok_or("fragment audit accounting overflow")?;
    if total!=extent{return Err("fragment audit lost one or more bytes".into());}
    Ok(out)
}
fn audit_original_pretea_fragments(pe:&Pe32,relocs:&[RelocBlock])->Result<()>{
    let section=pe.section(b".text")?;
    let len=section.virtual_size.min(section.raw_len as u32) as usize;
    let start=section.virtual_address;
    let end=start.checked_add(len as u32).ok_or("text extent overflow")?;
    let mut slots=Vec::new();
    for block in relocs {
        for &offset in &block.highlow_offsets {
            let rva=block.page_rva.checked_add(u32::from(offset))
                .ok_or("HIGHLOW section RVA overflow")?;
            if rva>=start && rva.checked_add(4).is_some_and(|hi|hi<=end) {
                slots.push((rva-start) as usize);
            }
        }
    }
    slots.sort_unstable();
    let result=pretea_fragment_geometry(len,&slots)?;
    // Independent interval-based algorithm (not the bitmap used by
    // pretea_fragment_geometry) checks the same SHA-verified ICD
    // section geometry. Neither approach loads or decrypts the DLL.
    let independent=unsafedisc_historical_155_page_fragments::
        independent_page_fragment_geometry(len,&slots)?;
    if independent.relocations!=result.highlow_slots ||
       independent.excluded_bytes!=result.highlow_bytes ||
       independent.complete_dwords!=result.modified_dwords ||
       independent.incomplete_tail_bytes!=result.skipped_tail_bytes ||
       independent.fragment_calls!=result.calls ||
       independent.unaligned_calls!=result.unaligned_calls ||
       independent.unaligned_dwords!=result.unaligned_dwords {
        return Err(format!("independent interval and bitmap pre-TEA fragment geometry disagree: {independent:?} vs {result:?}"));
    }
    if result!=(PreTeaFragmentGeometry{
        highlow_slots:61_743,highlow_bytes:246_972,
        modified_dwords:440_422,skipped_tail_bytes:93_801,
        calls:48_149,unaligned_calls:35_878,unaligned_dwords:315_414,
    }){
        return Err(format!("original relocation-fragment geometry changed: {result:?}"));
    }
    println!("Relocation-aware 1.50.20 extra-layer fragment geometry (STATIC, original ICD):");
    println!("  .text bounded virtual extent: {len} bytes; HIGHLOW operands: {}",result.highlow_slots);
    println!("  modifier calls (>=1 DWORD per fragment): {}",result.calls);
    println!("  complete transformable DWORDs: {}",result.modified_dwords);
    println!("  unaligned page-local fragments: {} / {}; complete DWORDs in them: {} / {}",
        result.unaligned_calls,result.calls,result.unaligned_dwords,result.modified_dwords);
    println!("  unchanged HIGHLOW bytes: {}; sub-DWORD gap tails: {}",
        result.highlow_bytes,result.skipped_tail_bytes);
    println!("  Fragment-local word mask restarts at 0; these are structural counts, not decoded content.");
    println!("  Independent 4KiB interval walker agrees: {} cross-page HIGHLOWs, max {} complete DWORDs in one fragment, then local index restarts at 0.",
        independent.crossing_relocations,independent.max_fragment_dwords);
    Ok(())
}

// Historical unSafeDisc 1.5.5 calls its per-DWORD adapter (0x401F62)
// separately for each NON-relocated byte interval emitted by 0x401EF5,
// not necessarily only once per 4KiB page. Each such call initializes
// EBX=0 as its word index. This variant models the relocation-aware path
// without touching protected user files or executing the DLL functions.
//
// 'highlow_offsets' are section-relative positions of full 4-byte HIGHLOW
// slots; they must be sorted, nonoverlapping and fully backed by 'data'.
// Byte runs separated by relocated DWORDs restart the word index; trailing
// incomplete bytes in each run remain unchanged, matching SHR ECX,2.
#[allow(dead_code)]
fn pre_tea_relocation_fragments_with_modifiers<F,G>(
    data:&mut[u8], highlow_offsets:&[usize],
    mut first:F,mut second:G,
)->Result<usize>
where F:FnMut(u32)->u32,G:FnMut(u32)->u32 {
    let mut previous_end=0usize;
    for &start in highlow_offsets {
        let end=start.checked_add(4).ok_or("HIGHLOW slot offset overflow")?;
        if start<previous_end || end>data.len() {
            return Err("unsorted, overlapping or out-of-range HIGHLOW offset".into());
        }
        previous_end=end;
    }
    let mut protected=vec![false;data.len()];
    for &start in highlow_offsets {
        protected[start..start+4].fill(true);
    }
    let mut calls=0usize;
    for page in data.chunks_mut(4096).enumerate() {
        let (page_index,buf)=page;
        let page_start=page_index.checked_mul(4096).ok_or("page position overflow")?;
        let mut i=0usize;
        while i<buf.len() {
            if protected[page_start+i] {i+=1;continue;}
            let begin=i;
            while i<buf.len() && !protected[page_start+i] {i+=1;}
            let run=&mut buf[begin..i];
            if run.len()>=4 {
                pre_tea_dwords_with_modifiers(run,&mut first,&mut second);
                calls+=1;
            }
        }
    }
    Ok(calls)
}

// Invert precisely the original 1.5.5 0x4019F5/0x401A01 block
// ordering on a *reference plaintext fragment*. The caller needs no
// game-specific EAX modifier to check the last 1..3 unmodified bytes:
// 0x401F62 works in complete 4-byte DWORDs, leaving the fragment tail
// unchanged, even though the overlapping TEA blocks touch those bytes.
//
// For a >=8-byte fragment, the legacy decryptor consumes leading 8-byte
// blocks until <=15 remain, then decrypts a last overlapping block
// (if length !=8) BEFORE decrypting the current head. Inverse must
// encrypt those exact blocks in the reverse order.
fn tea_encrypt_block(v0:u32,v1:u32,key:[u32;4])->(u32,u32){
    let mut left=v0;let mut right=v1;
    let mut sum=0x9E37_79B9u32;
    for _ in 0..32 {
        left=left.wrapping_add(
            (right.wrapping_shl(4).wrapping_add(key[0]))
                ^(sum.wrapping_add(right))
                ^((right>>5).wrapping_add(key[1])));
        right=right.wrapping_add(
            (left.wrapping_shl(4).wrapping_add(key[2]))
                ^(sum.wrapping_add(left))
                ^((left>>5).wrapping_add(key[3])));
        sum=sum.wrapping_add(0x9E37_79B9);
    }
    (left,right)
}

fn original_tea_reverse_fragment(plain:&[u8],key:[u32;4])->Result<Vec<u8>>{
    let mut intermediate=plain.to_vec();
    if intermediate.len()<8 {return Ok(intermediate);}
    let mut order=Vec::new();
    let mut cursor=0usize;
    let mut remaining=intermediate.len();
    while remaining>15 {
        order.push(cursor);
        cursor+=8;
        remaining-=8;
    }
    if remaining>=8 {
        if remaining!=8 {order.push(cursor+remaining-8);}
        order.push(cursor);
    }
    for &at in order.iter().rev() {
        let left=read_u32(&intermediate,at)?;
        let right=read_u32(&intermediate,at+4)?;
        let (a,b)=tea_encrypt_block(left,right,key);
        intermediate[at..at+4].copy_from_slice(&a.to_le_bytes());
        intermediate[at+4..at+8].copy_from_slice(&b.to_le_bytes());
    }
    Ok(intermediate)
}

#[derive(Debug,Default,PartialEq,Eq)]
struct TeaTailOracle {
    fragments:usize,
    matching_fragments:usize,
    checked_bytes:usize,
    matching_bytes:usize,
}
fn validate_tea_key_by_unchanged_fragment_tails(
    icd:&[u8], reference:&[u8],
    protected_pe:&Pe32, reference_pe:&Pe32,
    relocation_blocks:&[RelocBlock],key:[u32;4],
)->Result<TeaTailOracle>{
    let sec=protected_pe.section(b".text")?;
    let refsec=reference_pe.section(b".text")?;
    if sec.virtual_address!=refsec.virtual_address ||
       sec.virtual_size!=refsec.virtual_size ||
       sec.raw_offset!=refsec.raw_offset ||
       sec.raw_len!=refsec.raw_len {
        return Err("TEA reference .text layout differs from protected ICD".into());
    }
    let extent=sec.virtual_size.min(sec.raw_len as u32) as usize;
    let end=sec.raw_offset.checked_add(extent).ok_or("TEA section extent overflow")?;
    let c=icd.get(sec.raw_offset..end).ok_or("protected TEA source out of bounds")?;
    let p=reference.get(refsec.raw_offset..end).ok_or("reference TEA source out of bounds")?;
    let limit=sec.virtual_address.checked_add(extent as u32).ok_or("TEA RVA upper bound overflow")?;
    let mut protected=vec![false;extent];
    for block in relocation_blocks {
        for &slot in &block.highlow_offsets {
            let rva=block.page_rva.checked_add(u32::from(slot))
                .ok_or("TEA HIGHLOW RVA overflow")?;
            if rva<sec.virtual_address || rva>=limit {continue;}
            let begin=(rva-sec.virtual_address) as usize;
            let finish=begin.checked_add(4).ok_or("TEA HIGHLOW length overflow")?;
            if finish>extent {return Err("TEA HIGHLOW straddles section end".into());}
            if protected[begin..finish].contains(&true){
                return Err("overlapping TEA HIGHLOW positions".into());
            }
            protected[begin..finish].fill(true);
        }
    }
    let mut out=TeaTailOracle::default();
    for (page_idx,page) in protected.chunks(4096).enumerate(){
        let start=page_idx.checked_mul(4096).ok_or("TEA page index overflow")?;
        let mut i=0usize;
        while i<page.len(){
            if page[i]{i+=1;continue;}
            let begin=i;
            while i<page.len() && !page[i] {i+=1;}
            let a=start+begin;
            let b=start+i;
            let len=b-a;
            if len<8 || len%4==0 {continue;}
            let tail=len%4;
            let candidate=original_tea_reverse_fragment(&p[a..b],key)?;
            out.fragments+=1;
            out.checked_bytes+=tail;
            let mut exact=true;
            for (&x,&y) in candidate[len-tail..].iter().zip(&c[b-tail..b]){
                if x==y {out.matching_bytes+=1;} else {exact=false;}
            }
            if exact {out.matching_fragments+=1;}
        }
    }
    Ok(out)
}

// Read-only known-plaintext characterization of the composite Crimson
// SafeDisc extra layer, without executing or reimplementing either
// protected EAX modifier. The first DWORD in every non-relocated fragment
// has mask=0, so (protected_first_word -> inverse_TEА_first_word) is a
// directly observed sample of second_modifier(first_modifier(input)).
// Repeated inputs MUST map to identical outputs; for the observed data,
// outputs are unique across distinct inputs as well. Never store or print
// the user's actual word pairs outside local process memory.
#[derive(Debug,Default,PartialEq,Eq)]
struct ModifierReferenceAudit {
    total_fragments:usize,
    modifier_fragments:usize,
    modified_dwords:usize,
    checked_tail_bytes:usize,
    matching_tail_bytes:usize,
    distinct_inputs:usize,
    distinct_outputs:usize,
    input_conflicts:usize,
    output_conflicts:usize,
    indexed_distinct_inputs:usize,
    indexed_distinct_outputs:usize,
    indexed_repeated_observations:usize,
    indexed_forward_conflicts:usize,
    indexed_reverse_conflicts:usize,
    max_index:usize,
}
fn insert_observed_composite_pair(
    forward:&mut std::collections::HashMap<u32,u32>,
    reverse:&mut std::collections::HashMap<u32,u32>,
    source:u32,output:u32,
)->(bool,bool){
    let forward_conflict=forward.get(&source)
        .is_some_and(|&prior|prior!=output);
    let reverse_conflict=reverse.get(&output)
        .is_some_and(|&prior|prior!=source);
    if !forward_conflict && !reverse_conflict {
        forward.insert(source,output);
        reverse.insert(output,source);
    }
    (forward_conflict,reverse_conflict)
}
// The composite is F2(F1(encrypted_word XOR mask) XOR mask).
// Different fragment-local word indices have different masks and must
// never be treated as observations of the same unindexed mapping.
// Reference-derived pair tables are kept strictly in process memory.
fn insert_indexed_modifier_observation(
    forward:&mut std::collections::HashMap<(u32,u32),u32>,
    reverse:&mut std::collections::HashMap<(u32,u32),u32>,
    index:u32,masked_input:u32,output:u32,
)->(bool,bool,bool){
    let key=(index,masked_input);
    let reverse_key=(index,output);
    let previously_seen=forward.contains_key(&key);
    let forward_conflict=forward.get(&key).is_some_and(|&prior|prior!=output);
    let reverse_conflict=reverse.get(&reverse_key)
        .is_some_and(|&prior|prior!=masked_input);
    if !forward_conflict && !reverse_conflict {
        forward.entry(key).or_insert(output);
        reverse.entry(reverse_key).or_insert(masked_input);
    }
    (previously_seen,forward_conflict,reverse_conflict)
}

fn inspect_original_modifier_reference_pairs(
    icd:&[u8], reference:&[u8],
    original:&Pe32, decoded:&Pe32,
    relocations:&[RelocBlock], key:[u32;4],
    captured:Option<&unsafedisc_modifier_snapshot::Pair>,
)->Result<(ModifierReferenceAudit,unsafedisc_algebra::AlgebraAudit,
           unsafedisc_indexed_affine::IndexedAffineAudit,
           unsafedisc_indexed_affine::MaskAliasAudit,
           unsafedisc_second_stage::SecondStageAudit,
           unsafedisc_first_stage::FirstStageAudit)> {
    let protected=original.section(b".text")?;
    let plain=decoded.section(b".text")?;
    if protected.virtual_address!=plain.virtual_address ||
       protected.virtual_size!=plain.virtual_size ||
       protected.raw_offset!=plain.raw_offset ||
       protected.raw_len!=plain.raw_len {
        return Err("composite modifier audit: protected/plaintext section layout mismatch".into());
    }
    let extent=protected.virtual_size.min(protected.raw_len as u32) as usize;
    let start=protected.raw_offset;
    let end=start.checked_add(extent).ok_or("composite audit length overflow")?;
    let encrypted=icd.get(start..end).ok_or("composite audit ICD .text out of bounds")?;
    let plaintext=reference.get(start..end).ok_or("composite audit reference .text out of bounds")?;
    let mut reserved=vec![false;extent];
    let mut relocation_slots=Vec::new();
    let section_end=protected.virtual_address.checked_add(extent as u32)
        .ok_or("composite audit virtual section overflow")?;
    for block in relocations {
        for &in_page in &block.highlow_offsets {
            let slot=block.page_rva.checked_add(u32::from(in_page))
                .ok_or("composite audit relocation RVA overflow")?;
            if slot<protected.virtual_address || slot>=section_end {continue;}
            let begin=(slot-protected.virtual_address) as usize;
            let finish=begin.checked_add(4)
                .ok_or("composite audit HIGHLOW extent overflow")?;
            if finish>extent || reserved[begin..finish].contains(&true) {
                return Err("invalid or overlapping composite audit HIGHLOW offset".into());
            }
            reserved[begin..finish].fill(true);
            relocation_slots.push(begin);
        }
    }
    let mut audit=ModifierReferenceAudit::default();
    let mut forward=std::collections::HashMap::<u32,u32>::new();
    let mut reverse=std::collections::HashMap::<u32,u32>::new();
    let mut indexed_forward=std::collections::HashMap::<(u32,u32),u32>::new();
    let mut indexed_reverse=std::collections::HashMap::<(u32,u32),u32>::new();
    let mut captured_matches=0usize;
    let mut captured_mismatches=0usize;
    for (page_number,page) in reserved.chunks(4096).enumerate() {
        let base=page_number.checked_mul(4096)
            .ok_or("composite audit page number overflow")?;
        let mut cursor=0usize;
        while cursor<page.len() {
            if page[cursor] {cursor+=1;continue;}
            let begin=cursor;
            while cursor<page.len() && !page[cursor] {cursor+=1;}
            let absolute_start=base+begin;
            let absolute_end=base+cursor;
            let length=absolute_end-absolute_start;
            audit.total_fragments+=1;
            let inverse=original_tea_reverse_fragment(
                &plaintext[absolute_start..absolute_end],key)?;
            let leftover=length%4;
            audit.checked_tail_bytes+=leftover;
            for (&x,&y) in inverse[length-leftover..].iter()
                .zip(&encrypted[absolute_end-leftover..absolute_end]) {
                if x==y {audit.matching_tail_bytes+=1;}
            }
            if length<4 {continue;}
            audit.modifier_fragments+=1;
            audit.modified_dwords+=length/4;
            let source=read_u32(encrypted,absolute_start)?;
            let target=read_u32(&inverse,0)?;
            let (fconf,rconf)=insert_observed_composite_pair(
                &mut forward,&mut reverse,source,target);
            audit.input_conflicts+=usize::from(fconf);
            audit.output_conflicts+=usize::from(rconf);
            for index in 0..length/4 {
                let idx=u32::try_from(index)
                    .map_err(|_| "composite fragment word index overflow")?;
                let mask=unsafedisc_indexed_affine::index_mask(idx);
                let at=absolute_start+index*4;
                let encrypted_word=read_u32(encrypted,at)?;
                let source=encrypted_word^mask;
                let intermediate=read_u32(&inverse,index*4)?;
                if let Some(modifiers)=captured {
                    if modifiers.extra_word(encrypted_word,idx)==intermediate {
                        captured_matches+=1;
                    } else {
                        captured_mismatches+=1;
                    }
                }
                let (seen,fconf,rconf)=insert_indexed_modifier_observation(
                    &mut indexed_forward,&mut indexed_reverse,idx,source,intermediate);
                audit.indexed_repeated_observations+=usize::from(seen);
                audit.indexed_forward_conflicts+=usize::from(fconf);
                audit.indexed_reverse_conflicts+=usize::from(rconf);
                audit.max_index=audit.max_index.max(index);
            }
        }
    }
    if captured.is_some() {
        println!("  Win98 capture EAX composite vs verified retail/reference: {} matching words, {} mismatches (of {} checked).",
            captured_matches,captured_mismatches,audit.modified_dwords);
        if captured_matches+captured_mismatches!=audit.modified_dwords || captured_mismatches!=0 {
            return Err("Win98 captured F1/F2 does not fully explain protected/reference extra-layer observations; refusing any decrypt claim".into());
        }
        let modifiers=captured.ok_or("snapshot unexpectedly missing")?;
        let forward=unsafedisc_candidate_forward::compare_text(
            encrypted,plaintext,&relocation_slots,key,modifiers)?;
        println!("  Full forward F1/F2 -> overlapping TEA reference audit: {} / {} equal non-HIGHLOW bytes; {} mismatches; {} HIGHLOW bytes excluded.",
            forward.matching_bytes,forward.checked_bytes,
            forward.differing_bytes,forward.excluded_relocation_bytes);
        if forward.fragments!=48_149 || forward.complete_words!=440_422 ||
           forward.excluded_relocation_bytes!=246_972 ||
           forward.checked_bytes!=1_855_489 || forward.differing_bytes!=0 {
            return Err("Win98 full forward candidate does not exactly recover reference non-HIGHLOW code; no unpacking may be enabled".into());
        }
        // The historical 1.5.5 section filter selects BOTH .text and
        // .data for the extra DWORD and TEA stages. The exact owner
        // reference separately validates the non-HIGHLOW .data bytes.
        let protected_data=original.section(b".data")?;
        let reference_data=decoded.section(b".data")?;
        if protected_data.virtual_address!=reference_data.virtual_address ||
           protected_data.raw_offset!=reference_data.raw_offset ||
           protected_data.raw_len!=reference_data.raw_len ||
           protected_data.virtual_size!=reference_data.virtual_size {
            return Err("Win98 .data reference PE geometry differs".into());
        }
        let data_extent=protected_data.virtual_size.min(protected_data.raw_len as u32) as usize;
        let data_start=protected_data.raw_offset;
        let data_end=data_start.checked_add(data_extent).ok_or("data extent overflow")?;
        let source_data=icd.get(data_start..data_end).ok_or("data ciphertext span out of bounds")?;
        let plain_data=reference.get(data_start..data_end).ok_or("data reference span out of bounds")?;
        let first=protected_data.virtual_address;
        let last=first.checked_add(data_extent as u32).ok_or("data RVA end overflow")?;
        let mut slots=Vec::new();
        for block in relocations {
            for &offset in &block.highlow_offsets {
                let at=block.page_rva.checked_add(offset as u32).ok_or("data HIGHLOW RVA overflow")?;
                if first<=at && at<last {
                    let rel=(at-first) as usize;
                    if rel.checked_add(4).is_none_or(|end|end>data_extent){
                        return Err("data HIGHLOW crosses raw section extent".into());
                    }
                    slots.push(rel);
                }
            }
        }
        let data_forward=unsafedisc_candidate_forward::compare_text(
            source_data,plain_data,&slots,key,modifiers)?;
        println!("  Win98 .data F1/F2 -> TEA: {} / {} equal non-HIGHLOW bytes; {} mismatches; {} relocations excluded.",
            data_forward.matching_bytes,data_forward.checked_bytes,
            data_forward.differing_bytes,slots.len());
        if data_forward.fragments!=321 ||
           data_forward.complete_words!=42_018 ||
           data_forward.excluded_relocation_bytes!=3_960 ||
           data_forward.checked_bytes!=168_072 ||
           data_forward.differing_bytes!=0 {
            return Err("Win98 full forward candidate does not recover reference .data".into());
        }
    }
    audit.distinct_inputs=forward.len();
    audit.distinct_outputs=reverse.len();
    audit.indexed_distinct_inputs=indexed_forward.len();
    audit.indexed_distinct_outputs=indexed_reverse.len();
    let algebra=unsafedisc_algebra::diagnose(&forward);
    let indexed_affine=unsafedisc_indexed_affine::diagnose(&indexed_forward);
    let mask_alias=unsafedisc_indexed_affine::audit_mask_aliases(&indexed_forward);
    let second_stage=unsafedisc_second_stage::diagnose(&indexed_forward);
    let first_stage=unsafedisc_first_stage::diagnose(&indexed_forward);
    Ok((audit,algebra,indexed_affine,mask_alias,second_stage,first_stage))
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

/// Standalone, read-only, exact SHA-256-gated disassembly audit of
/// the OWNER's unmodified historical tool. It does not require
/// CRIMSON.EXE, CRIMSON.ICD, dplayerx.dll or TESTME.EXE.
/// A public compatibility-modified build is deliberately rejected.
fn inspect_historical_155_cli() -> Result<()> {
    let mut args=env::args().skip(2);
    if args.next().as_deref()!=Some("--unsafedisc") {
        return Err("usage: inspect-historical --unsafedisc unSafedisc.exe".into());
    }
    let path=args.next().map(PathBuf::from)
        .ok_or("missing historical unSafedisc.exe path")?;
    if args.next().is_some() {
        return Err("unexpected extra inspect-historical argument".into());
    }
    let image=read_original(&path,HISTORICAL_155_LEN,HISTORICAL_155_SHA)?;
    let pe=parse_pe32(&image)?;
    let nt=read_u32(&image,0x3C)? as usize;
    let base=read_u32(&image,nt+24+28)?;
    let code=pe.section(b"CODE")?;
    if code.characteristics&0x2000_0000==0 {
        return Err("historical CODE section is not executable".into());
    }
    let code_end=code.raw_offset.checked_add(
        code.raw_len.min(code.virtual_size as usize)
    ).ok_or("historical CODE raw range overflow")?;
    let backed=image.get(code.raw_offset..code_end)
        .ok_or("historical CODE section exceeds executable")?;
    unsafedisc_historical_155::audit_155_entry_to_modifier_loop(
        backed,base,code.virtual_address
    )?;
    let checksum_bridge=
        unsafedisc_historical_155::audit_155_codebit_crc_modifier_bridge(
            backed,base,code.virtual_address
        )?;
    let iterator=unsafedisc_historical_155::audit_155_section_iterator(
        backed,base,code.virtual_address
    )?;
    let builder=unsafedisc_historical_155::audit_155_section_list_builder(
        backed,base,code.virtual_address
    )?;
    // Additional static region verified against the pinned PUBLIC
    // variant; the owner's distinct original must pass exact bytes
    // before the CLI asserts the same historical control flow.
    unsafedisc_historical_155_list_plan::audit_155_post_builder_rva_mapper(
        backed,base,code.virtual_address
    )?;
    let reloc_builder=unsafedisc_historical_155_reloc_builder::
        audit_155_reloc_cutpoint_builder(backed,base,code.virtual_address)?;
    let reloc_consumer=unsafedisc_historical_155_reloc_consumer::
        audit_155_reloc_consumer(backed,base,code.virtual_address)?;
    let provenance=unsafedisc_historical_155::audit_155_section_table_provenance(
        backed,base,code.virtual_address
    )?;
    let crc_callers=unsafedisc_historical_155::audit_155_crc_helper_callers(
        backed,base,code.virtual_address
    )?;
    let startup=unsafedisc_historical_155::analyze_155_dll_startup(
        backed,base,code.virtual_address
    )?;
    // Do not rely on the recovered import's name from a comment:
    // resolve the actual FF 25 trampoline through the user's
    // SHA-verified PE32 import descriptors. No DLL is loaded.
    let import_trampoline_rva=startup.loadlibrary_thunk_va
        .checked_sub(base).ok_or("historical DLL-load thunk precedes PE base")?;
    let trampoline_at=executable_rva_to_file_offset(
        &pe,import_trampoline_rva,6)?;
    if image.get(trampoline_at..trampoline_at+2)!=Some(&[0xFF,0x25][..]) {
        return Err("historical LoadLibraryExA trampoline is not FF 25".into());
    }
    let imported_iat_va=read_u32(&image,trampoline_at+2)?;
    let imported_iat_rva=imported_iat_va.checked_sub(base)
        .ok_or("historical DLL-load IAT VA precedes PE base")?;
    let import_bindings=dll_import_bindings(&image,&pe)?;
    let matching:Vec<_>=import_bindings.iter()
        .filter(|b|b.iat_rva==imported_iat_rva).collect();
    if matching.len()!=1 ||
       !matching[0].library.eq_ignore_ascii_case("KERNEL32.dll") ||
       matching[0].symbol!="LoadLibraryExA" {
        return Err("historical DLL-load thunk does not resolve to KERNEL32!LoadLibraryExA".into());
    }
    println!("Historical original unSafedisc.exe 1.5.5: exact SHA-256 and CODE opcode audit PASSED.");
    println!("  Historical CRC and extra layer: CODE bit 0x{:X} gates checksum only; non-code JZ -> 0x{:08X}, code path CALL -> 0x{:08X}.",
        checksum_bridge.codebit,
        checksum_bridge.bypass_checksum_target_va,
        checksum_bridge.after_checksum_call_target_va);
    println!("  PE section list builder: table at [0x{:08X}], count [0x{:08X}], pointer-list destination [0x{:08X}]; {}-byte headers, {}-byte output pointers.",
        builder.source_header_pointer_va,builder.section_count_va,
        builder.pointer_list_destination_va,builder.section_header_stride,
        builder.pointer_list_stride);
    println!("  Historical NT -> PE section-table origin: stored NT header at [0x{:08X}], MOV CX from NT+0x{:X} to full ECX global [0x{:08X}], section table at NT+0x{:X}.",
        provenance.nt_header_save_va,provenance.section_count_field_offset,
        provenance.section_count_global_va,provenance.fixed_section_table_delta);
    println!("  Verified upstream XOR ECX,ECX at 0x4024A5 clears high16 on the audited normal path, despite MOV CX only updating low16. Historical e_lfanew is also read as a WORD (16 bits).");
    println!("  Native PE parser correctly reads 32-bit e_lfanew, 16-bit NumberOfSections and the actual optional-header length; it does not replicate legacy truncation.");
    println!("  Historical selection: eight exact four-byte section-name exclusions; skipped branches converge at 0x{:08X}.",
        builder.skip_header_va);
    println!("  Historical section LOOP with ECX=0 would execute {} iterations (32-bit wrap); native implementations must reject a zero section count.",
        unsafedisc_historical_155_list_plan::historical_155_loop_iterations(0));
    println!("  Audited builder LOOP does not write a final zero DWORD; iterator needs a zero slot. Code after the audited window may still initialize one; runtime behavior is unverified.");
    println!("  Follow-on post-builder RVA mapper: 0x4022B2..0x40230B; source RVA [0x403B43], section RVA at +0x0C, RawSize at +0x10, raw pointer at +0x14.");
    println!("  The archived code tests inclusive raw end and next-header VirtualAddress [EAX+0x34], then stores delta at [0x403B3F] and raw offset at [0x403B43].");
    println!("  This opcode audit ran on the exact 20,480-byte SHA-verified original in THIS invocation; its x86 windows match the published archive. No runtime behavior is inferred.");
    println!("  Archived relocation cutpoint builder: mode=0x{:X} written at [0x{:08X}] (sentinel); first sorted cutpoint at 0x{:08X}.",
        reloc_builder.reloc_mode_value,reloc_builder.mode_global_va,
        reloc_builder.first_cutpoint_va);
    println!("  Nonzero relocation WORDs are converted into RVA and RVA+4 cutpoints; archived code does NOT filter the four-bit PE relocation type.");
    println!("  This archival relocation-builder opcode window is byte-exact verified against the original 20,480-byte SHA-pinned executable in THIS invocation.");
    println!("  Archived relocation walker strips type nibbles in packed pairs with mask 0x{:08X}; safe model rejects an unpaired final relocation WORD.",
        reloc_consumer.paired_word_mask);
    println!("  Span helper at 0x401EF5 clips against section end [0x{:08X}]; on overrun it redirects its return to 0x{:08X}, then enters shared helper 0x{:08X}.",
        reloc_consumer.section_end_va,reloc_consumer.clip_return_va,
        reloc_consumer.normal_call_helper_va);
    println!("  These are x86-byte-verifiable control semantics, not evidence of retail runtime reachability or recovered protected plaintext.");
    // Advisory static DATA inspection; the owner's original 1.5.5
    // may differ from the compatibility-modified public archive.
    // Its on-disk initial zeros are NOT evidence of runtime clearing.
    if let Ok(data_sec)=pe.section(b"DATA") {
        let backed_len=if data_sec.virtual_size==0 {
            data_sec.raw_len
        } else {
            data_sec.raw_len.min(data_sec.virtual_size as usize)
        };
        let end=data_sec.raw_offset.checked_add(backed_len)
            .ok_or("original historical DATA file-range overflow")?;
        let data_bytes=image.get(data_sec.raw_offset..end)
            .ok_or("original historical DATA file-range unbacked")?;
        match unsafedisc_historical_155::inspect_155_pointer_list_disk_state(
            data_bytes,data_sec.virtual_address,base
        ) {
            Ok(state)=>{
                println!(
                    "  On-disk selected-section list: global 0x{:08X}, initial pointer 0x{:08X}, {} complete backed pointer slots, {} leading zero DWORDs. NO runtime-zeroing claim.",
                    state.pointer_global_va,state.initial_pointer_list_va,
                    state.capacity_words_in_backed_data,state.leading_zero_words
                );
                if state.leading_zero_words==0 {
                    println!("  No zero terminator is evidenced at the on-disk list start. Runtime state remains unverified.");
                } else {
                    println!(
                        "  Hypothetical unchanged-snapshot maximum: {} selected pointers while retaining one pre-existing zero terminator. NOT runtime capacity.",
                        unsafedisc_historical_155_list_plan::hypothetical_zero_terminated_capacity(
                            state.leading_zero_words)
                    );
                }
            },
            Err(reason)=>println!(
                "  On-disk pointer-list state is UNCONFIRMED for this original binary: {reason}"
            ),
        }
    } else {
        println!("  On-disk pointer-list state unconfirmed: historical DATA section not identified.");
    }

    println!("  PE section iterator: initial JMP -> 0x{:08X} skips POPAD; completed sections -> 0x{:08X} execute POPAD before next pointer.",
        iterator.first_entry_va,iterator.next_entry_restore_va);
    println!("  Section pointer-list stride {} bytes; section's effective raw extent min(RawSize, nonzero VirtualSize); header fields raw +0x{:X}, virtual +0x{:X}.",
        iterator.header_stride_bytes,iterator.raw_size_field_offset,iterator.virtual_size_field_offset);
    println!("  Mode=4 uses relocation-aware path at 0x{:08X}; separate CODE flag only controls CRC, not all decryption.",
        iterator.relocated_branch_va);
    println!("  Relocation walker final E9 rel32 joins at 0x{:08X} (POP ESI, restore scratch cursor), then returns via 401D05 POPAD.",
        iterator.relocation_return_through_va);
    println!("  All three PAGE/FULL, PAGE/TAIL and HIGHLOW-fragment paths CALL the same RVA 0x{:08X} helper (sites 0x{:08X}, 0x{:08X}, 0x{:08X}).",
        crc_callers.shared_helper_va,
        crc_callers.full_page_call_va,crc_callers.tail_call_va,
        crc_callers.relocation_call_va);
    println!("  Both code/non-code paths converge on DLL-size-gated pre-TEA modifier; section selection is a separate gate.");
    println!("  Original DLL loading strategy: LoadLibraryExA dwFlags={} (call wrapper VA 0x{:08X}).",
        startup.loadlibrary_flags,startup.loadlibrary_thunk_va);
    println!("  Validated from original PE import metadata: KERNEL32!LoadLibraryExA IAT RVA 0x{imported_iat_rva:08X}.");
    match startup.attach_branch {
        unsafedisc_historical_155::HistoricalAttachBranch::AlwaysSkipManualAttach=>
            println!("  DLL entrypoint: unconditional skip to 0x{:08X} (no explicit attach from 402D98).",startup.skip_target_va),
        unsafedisc_historical_155::HistoricalAttachBranch::SkipWhenDsBit2Clear=>
            println!("  DLL entrypoint: DS-bit2 ZF=1 skips manual DLL_PROCESS_ATTACH; else manual CALL at 0x{:08X}.",startup.manual_dll_entry_call_va),
        unsafedisc_historical_155::HistoricalAttachBranch::SkipWhenDsBit2Set=>
            println!("  DLL entrypoint: DS-bit2 ZF=0 skips manual DLL_PROCESS_ATTACH; else manual CALL at 0x{:08X}.",startup.manual_dll_entry_call_va),
    }
    println!("  Original exact-hash release uses LoadLibraryExA flags=1 and a DS-bit2 guarded manual attach; the public version.dll-adapted variant instead uses flags=0 and always skips manual attach.");
    println!("  DLL size 0x21E00 -> 401F43 guarded trampoline -> 401F62 extra-DWORD-layer loop.");
    println!("  401F56: EBX buffer -> ESI, EAX length -> EDI; loop processes EDI>>2 DWORDs.");
    println!("  401F96: CALL [DLL base+0x65BE] via EDX.");
    println!("  401F98: XOR EAX,ECX uses post-first-call ECX.");
    println!("  401F9A/401F9D: ADD EDX,0x63; CALL EDX uses post-first-call EDX.");
    println!("  Exact 0x6621 second entry assumes first helper preserved EDX; same mask assumes ECX.");
    println!("  The original title-specific modifier bodies remain encrypted/unrecovered.");
    println!("  The file was read only. No legacy Windows execution, DLL load or game decryption.");
    Ok(())
}

fn load_verified_modifier_snapshot(path:&Path)->Result<unsafedisc_modifier_snapshot::Pair> {
    let image=read_original(path,SNAPSHOT_LEN,SNAPSHOT_SHA)?;
    let pe=parse_pe32(&image)?;
    let nt=read_u32(&image,0x3C)? as usize;
    let optional=nt.checked_add(24).ok_or("PE optional header overflow")?;
    let size_image=read_u32(&image,optional+56)?;
    let preferred=read_u32(&image,optional+28)?;
    if size_image!=SNAPSHOT_LEN as u32 || preferred!=0x008E_0000 {
        return Err("snapshot PE image geometry differs from verified Win98 capture".into());
    }
    if pe.sections.len()!=6 ||
       pe.sections.iter().map(|s|s.name.split(|b| *b==0).next().unwrap_or(&[]))
       .collect::<Vec<_>>()!=[
           b".txt".as_slice(),b".text".as_slice(),b".txt2".as_slice(),
           b".rdata".as_slice(),b".data".as_slice(),b".reloc".as_slice()
       ] ||
       pe.sections.iter().map(|s|s.virtual_address).collect::<Vec<_>>()
       !=[0x1000,0x9000,0x18000,0x1E000,0x20000,0x27000]
    {
        return Err("snapshot section geometry differs from verified Win98 capture".into());
    }
    let pair=unsafedisc_modifier_snapshot::Pair::from_mapped_image(&image)?;
    if pair.first.operations()!=24 || pair.second.operations()!=33 ||
       pair.first.forward(0)!=0x8BD9_668C ||
       pair.second.forward(0)!=0xF01C_8BF1 ||
       pair.extra_word(0,0)!=0xC293_E587 ||
       pair.extra_word(0,256)!=0x32A3_7588
    {
        return Err("snapshot modifier opcode semantics differ from verified independent analysis".into());
    }
    Ok(pair)
}

/// Inspect only the exact, owner-supplied Windows 98 passive DLL capture.
/// Unlike a PE file on disk, this input is a complete *mapped* image whose
/// RVA corresponds to the byte offset. No Windows code is ever executed.
fn inspect_snapshot_cli() -> Result<()> {
    let mut args=env::args().skip(2);
    if args.next().as_deref()!=Some("--snapshot") {
        return Err("usage: inspect-snapshot --snapshot CS98-DLL-00.BIN".into());
    }
    let path=PathBuf::from(args.next().ok_or("missing snapshot path")?);
    if args.next().is_some() {
        return Err("inspect-snapshot accepts only one --snapshot path".into());
    }
    let pair=load_verified_modifier_snapshot(&path)?;
    // Reversibility is a necessary check, not proof of a complete ICD decryptor.
    let mut state=0x51C0_A71E_u32;
    for source in [0,1,2,0x1234_5678,0xDEAD_BEEF,0x8000_0000,u32::MAX] {
        for index in [0,1,255,256,257,0xABCD_1234] {
            let out=pair.extra_word(source,index);
            if pair.inverse_extra_word(out,index)!=source {
                return Err("indexed EAX modifier inverse failed".into());
            }
        }
    }
    for _ in 0..512 {
        state=state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        for index in [0,255,256] {
            if pair.inverse_extra_word(pair.extra_word(state,index),index)!=state {
                return Err("synthetic indexed EAX modifier inverse failed".into());
            }
        }
    }
    println!("Verified owner-supplied Win98 passive DLL snapshot (exact SHA-256 and PE geometry).");
    println!("  F1 RVA=0x65BE, EAX-only operations={}",pair.first.operations());
    println!("  F2 RVA=0x6621, EAX-only operations={}",pair.second.operations());
    println!("  F1(0)=0x{:08X}; F2(0)=0x{:08X}",
        pair.first.forward(0),pair.second.forward(0));
    println!("  indexed composite at X=0: i=0 -> 0x{:08X}; i=256 -> 0x{:08X}",
        pair.extra_word(0,0),pair.extra_word(0,256));
    println!("  519 synthetic sources checked for indexed roundtrip (and four anchored results).");
    println!("  Read-only. No DLL execution, no game decryption and no executable written.");
    Ok(())
}

fn main() {
    if env::args().nth(1).as_deref()==Some("inspect-snapshot") {
        if let Err(e)=inspect_snapshot_cli() {
            eprintln!("ERROR: {e}");
            process::exit(1);
        }
        return;
    }
    if env::args().nth(1).as_deref()==Some("inspect-historical") {
        if let Err(e)=inspect_historical_155_cli() {
            eprintln!("ERROR: {e}");
            process::exit(1);
        }
        return;
    }
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
    fn fragment_geometry_accounts_for_protected_slots_and_nonword_tails(){
        let result=pretea_fragment_geometry(22,&[5,13]).unwrap();
        assert_eq!(result,PreTeaFragmentGeometry{
            highlow_slots:2,highlow_bytes:8,
            modified_dwords:3,skipped_tail_bytes:2,calls:3,
            unaligned_calls:2,unaligned_dwords:2
        });
        // This crossing slot starts on the final two bytes of page 0.
        let crossed=pretea_fragment_geometry(4107,&[4094]).unwrap();
        assert_eq!(crossed.highlow_slots,1);
        assert_eq!(crossed.highlow_bytes,4);
        assert_eq!(crossed.calls,2);
        assert_eq!(crossed.skipped_tail_bytes,3);
        assert_eq!(crossed.modified_dwords,1025);
        assert_eq!(crossed.unaligned_calls,1);
        assert_eq!(crossed.unaligned_dwords,2);
        assert!(pretea_fragment_geometry(8,&[6]).is_err());
        assert!(pretea_fragment_geometry(16,&[4,6]).is_err());
    }

    #[test]
    fn relocation_fragment_adapter_skips_slots_and_resets_word_index(){
        let original:Vec<u8>=(0..64).map(|i| ((i*17+9)&255) as u8).collect();
        let first=|w:u32|w.rotate_left(3).wrapping_add(0x1234_5678);
        let second=|w:u32|w^0xA55A_9669;
        let mut page_only=original.clone();
        pre_tea_section_pages_with_modifiers(&mut page_only,first,second);
        let mut split=original.clone();
        let calls=pre_tea_relocation_fragments_with_modifiers(
            &mut split,&[12,32,48],first,second).unwrap();
        assert_eq!(calls,4);
        for &offset in &[12usize,32,48] {
            assert_eq!(&split[offset..offset+4],&original[offset..offset+4]);
        }
        // Distinct word indices after a relocated slot must matter.
        assert_ne!(&split[16..20],&page_only[16..20]);
        let mut expected=original.clone();
        for &(begin,end) in &[(0usize,12usize),(16,32),(36,48),(52,64)] {
            pre_tea_dwords_with_modifiers(&mut expected[begin..end],first,second);
        }
        assert_eq!(split,expected);
    }

    #[test]
    fn relocation_fragment_adapter_handles_cross_page_highlow_and_tails(){
        let mut bytes=vec![0x44u8;4107];
        let original=bytes.clone();
        let calls=pre_tea_relocation_fragments_with_modifiers(
            &mut bytes,&[4094],|x|x.wrapping_add(3),|x|x.rotate_right(2)
        ).unwrap();
        assert_eq!(calls,2);
        assert_eq!(&bytes[4094..4098],&original[4094..4098]);
        assert_eq!(&bytes[4092..4094],&original[4092..4094]);
        assert_eq!(&bytes[4106..4107],&original[4106..4107]);
        assert_ne!(&bytes[..4],&original[..4]);
        assert_ne!(&bytes[4098..4102],&original[4098..4102]);
    }

    #[test]
    fn relocation_fragment_adapter_rejects_malformed_slots(){
        let mut bytes=vec![0u8;36];
        for slots in [&[5usize,4][..],&[4,6][..],&[33][..],&[usize::MAX][..]]{
            assert!(pre_tea_relocation_fragments_with_modifiers(
                &mut bytes,slots,|x|x,|x|x
            ).is_err());
        }
        assert_eq!(pre_tea_relocation_fragments_with_modifiers(
            &mut bytes,&[],|x|x,|x|x).unwrap(),1);
    }

    #[test]
    fn tea_encrypt_is_inverse_of_original_decrypt_vector(){
        assert_eq!(tea_encrypt_block(0,0,[0;4]),(0x41EA_3A0A,0x94BA_A940));
        for key in [[0u32;4],[1,2,3,4],[0x7496_538F,0xE9C7_914F,0xCF10_EA3F,0x82F5_DE2F]] {
            for (a,b) in [(0u32,0u32),(1,2),(0xFFFF_FFFF,0x1234_5678)]{
                let (x,y)=tea_encrypt_block(a,b,key);
                assert_eq!(tea_decrypt(x,y,key),(a,b));
            }
        }
    }

    #[test]
    fn tea_reverse_overlap_matches_historic_tail_first_decrypt_order(){
        let key=[0x7496_538F,0xE9C7_914F,0xCF10_EA3F,0x82F5_DE2F];
        for size in [8usize,9,10,11,12,13,14,15,16,17,23,31,32,33,127,4096] {
            let intermediate:Vec<u8>=(0..size).map(|i|(i as u8).wrapping_mul(37)).collect();
            // Test-only independent simulation of forward historical
            // DECRYPT: full 8B blocks; overlapping tail first, then head.
            let mut plain=intermediate.clone();
            let mut cursor=0usize;let mut remaining=size;
            let mut positions=Vec::new();
            while remaining>15 {positions.push(cursor);cursor+=8;remaining-=8;}
            if remaining>=8 {
                if remaining!=8{positions.push(cursor+remaining-8);}
                positions.push(cursor);
            }
            for at in positions {
                let a=read_u32(&plain,at).unwrap();
                let b=read_u32(&plain,at+4).unwrap();
                let (x,y)=tea_decrypt(a,b,key);
                plain[at..at+4].copy_from_slice(&x.to_le_bytes());
                plain[at+4..at+8].copy_from_slice(&y.to_le_bytes());
            }
            assert_eq!(original_tea_reverse_fragment(&plain,key).unwrap(),intermediate);
        }
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
    fn executable_rva_must_resolve_to_backed_code() {
        let mut image = synthetic_pe();
        let sec = 0x178usize;
        image[sec..sec+8].copy_from_slice(b".text\0\0\0");
        image[sec+8..sec+12].copy_from_slice(&0xB0u32.to_le_bytes());
        image[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert_eq!(executable_rva_to_file_offset(&pe, 0x100A, 64).unwrap(), 0x20A);
        // Can't extract virtual padding or straddle the raw backing.
        assert!(executable_rva_to_file_offset(&pe, 0x1090, 64).is_err());
        assert!(executable_rva_to_file_offset(&pe, 0x1000, 0).is_err());
        assert!(executable_rva_to_file_offset(&pe, 0x1000, 257).is_err());
        assert!(executable_rva_to_file_offset(&pe, 0x2000, 16).is_err());
        image[sec+36..sec+40].copy_from_slice(&0x4000_0040u32.to_le_bytes());
        let non_executable = parse_pe32(&image).unwrap();
        assert!(executable_rva_to_file_offset(&non_executable, 0x100A, 16).is_err());
    }

    #[test]
    fn dll_tls_reader_distinguishes_absent_and_valid_callbacks() {
        let mut image = synthetic_pe();
        let section_at = 0x178usize;
        let optional_at = 0x98usize;
        let dir_at = optional_at + 96 + 9*8;
        image[optional_at+28..optional_at+32].copy_from_slice(&0x0090_0000u32.to_le_bytes());
        image[section_at+12..section_at+16].copy_from_slice(&0x3000u32.to_le_bytes());
        image[section_at+8..section_at+12].copy_from_slice(&0x100u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert_eq!(dll_tls_metadata(&image, &pe).unwrap(), None);
        image[dir_at..dir_at+4].copy_from_slice(&0x3020u32.to_le_bytes());
        image[dir_at+4..dir_at+8].copy_from_slice(&24u32.to_le_bytes());
        // Section raw starts at 0x200; TLS directory at RVA +0x20.
        image[0x22C..0x230].copy_from_slice(&0x0090_3040u32.to_le_bytes());
        // callback list at RVA 0x3040 -> file 0x240.
        image[0x240..0x244].copy_from_slice(&0x0090_1040u32.to_le_bytes());
        image[0x244..0x248].fill(0);
        let pe = parse_pe32(&image).unwrap();
        let tls = dll_tls_metadata(&image, &pe).unwrap().unwrap();
        assert_eq!(tls.directory_rva, 0x3020);
        assert_eq!(tls.directory_size, 24);
        assert_eq!(tls.callback_rvas, vec![0x1040]);
        assert_eq!(dll_backed_rva_offset(&pe, 0x3040, 4).unwrap(), 0x240);
        assert!(dll_backed_rva_offset(&pe, 0x3100, 4).is_err());

        // An unterminated or malformed directory may not be presented as valid.
        image[dir_at+4..dir_at+8].copy_from_slice(&4u32.to_le_bytes());
        assert!(dll_tls_metadata(&image, &pe).is_err());
        image[dir_at+4..dir_at+8].copy_from_slice(&24u32.to_le_bytes());
        image[0x22C..0x230].copy_from_slice(&0x0080_0000u32.to_le_bytes());
        assert!(dll_tls_metadata(&image, &pe).is_err());
    }

    #[test]
    fn dll_zero_fill_mapping_vs_unmapped_and_raw_sections() {
        let mut image = synthetic_pe();
        let section = 0x178usize;
        image[section..section+8].copy_from_slice(b".data\0\0\0");
        image[section+12..section+16].copy_from_slice(&0x2000u32.to_le_bytes());
        image[section+8..section+12].copy_from_slice(&0x180u32.to_le_bytes());
        image[section+16..section+20].copy_from_slice(&0x100u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert!(dll_zero_filled_section(&pe, 0x20FF, 4).is_none());
        assert!(dll_zero_filled_section(&pe, 0x2100, 4).is_some());
        assert!(dll_zero_filled_section(&pe, 0x217C, 4).is_some());
        assert!(dll_zero_filled_section(&pe, 0x217D, 4).is_none());
        assert!(dll_zero_filled_section(&pe, 0x2180, 4).is_none());
        assert!(dll_zero_filled_section(&pe, 0x2100, 0).is_none());
        assert!(dll_zero_filled_section(&pe, 0xFFFF_FFFF, 4).is_none());
        image[section+8..section+12].copy_from_slice(&0x80u32.to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert!(dll_zero_filled_section(&pe, 0x2100, 4).is_none());
    }

    #[test]
    fn dll_rel32_call_targets_follow_signed_x86_displacement() {
        let mut image = synthetic_pe();
        let section = 0x178usize;
        image[section..section+8].copy_from_slice(b".text\0\0\0");
        image[section+12..section+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[section+8..section+12].copy_from_slice(&0x100u32.to_le_bytes());
        image[section+36..section+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        // Raw section file +0x200, RVA 0x1000. 0x1008 -> 0x1040.
        image[0x208] = 0xE8;
        image[0x209..0x20D].copy_from_slice(&0x33i32.to_le_bytes());
        // 0x1040 -> 0x1008 uses a negative signed rel32.
        image[0x240] = 0xE8;
        image[0x241..0x245].copy_from_slice(&(-0x3Di32).to_le_bytes());
        let pe = parse_pe32(&image).unwrap();
        assert_eq!(dll_relative_call_target(&image, &pe, 0x1008).unwrap(), 0x1040);
        assert_eq!(dll_relative_call_target(&image, &pe, 0x1040).unwrap(), 0x1008);
        assert!(dll_relative_call_target(&image, &pe, 0x1009).is_err());
        image[0x241..0x245].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(dll_relative_call_target(&image, &pe, 0x1040).is_err());
    }

    #[test]
    fn dll_import_directory_resolves_pe32_original_thunks_and_ordinals() {
        let mut image = synthetic_pe();
        let section = 0x178usize;
        let opt = 0x98usize;
        let dir = opt + 96 + 8;
        image[section+12..section+16].copy_from_slice(&0x2000u32.to_le_bytes());
        image[section+8..section+12].copy_from_slice(&0x100u32.to_le_bytes());
        image[dir..dir+4].copy_from_slice(&0x2000u32.to_le_bytes());
        image[dir+4..dir+8].copy_from_slice(&40u32.to_le_bytes());
        image[0x200..0x204].copy_from_slice(&0x2080u32.to_le_bytes()); // OriginalFirstThunk
        image[0x20C..0x210].copy_from_slice(&0x2050u32.to_le_bytes()); // Name
        image[0x210..0x214].copy_from_slice(&0x2090u32.to_le_bytes()); // FirstThunk
        image[0x250..0x25D].copy_from_slice(b"KERNEL32.dll\0");
        image[0x280..0x284].copy_from_slice(&0x20A0u32.to_le_bytes());
        image[0x284..0x288].copy_from_slice(&0x8000_0032u32.to_le_bytes());
        image[0x288..0x28C].fill(0); // end lookup list
        image[0x290..0x294].copy_from_slice(&0x20A0u32.to_le_bytes());
        image[0x294..0x298].copy_from_slice(&0x8000_0032u32.to_le_bytes());
        image[0x298..0x29C].fill(0); // end IAT list
        image[0x2A0..0x2A2].copy_from_slice(&7u16.to_le_bytes()); // hint
        image[0x2A2..0x2AD].copy_from_slice(b"HeapCreate\0");
        let pe = parse_pe32(&image).unwrap();
        let found = dll_import_bindings(&image,&pe).unwrap();
        assert_eq!(found,vec![
            DllImportBinding { iat_rva:0x2090,library:"KERNEL32.dll".into(),symbol:"HeapCreate".into() },
            DllImportBinding { iat_rva:0x2094,library:"KERNEL32.dll".into(),symbol:"#50".into() },
        ]);
        assert!(dll_pe_import_string(&image,&pe,0x20F0).is_err());
        image[dir..dir+4].fill(0);
        image[dir+4..dir+8].fill(0);
        assert_eq!(dll_import_bindings(&image,&pe).unwrap(),Vec::new());
        image[dir+4..dir+8].copy_from_slice(&40u32.to_le_bytes());
        assert!(dll_import_bindings(&image,&pe).is_err());
    }

    #[test]
    fn dll_obfuscated_entry_near_branches_must_converge_on_valid_code() {
        let mut data = synthetic_pe();
        let sec = 0x178usize;
        data[sec..sec+8].copy_from_slice(b".text\0\0\0");
        data[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        data[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        data[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        // Relative branches from 0x1005 and 0x1010 share 0x1040.
        data[0x205..0x20B].copy_from_slice(&[0x0F,0x84,0x35,0,0,0]);
        data[0x210..0x215].copy_from_slice(&[0xE9,0x2B,0,0,0]);
        let pe = parse_pe32(&data).unwrap();
        assert_eq!(dll_known_near_branch_target(&data,&pe,0x1005,true).unwrap(),0x1040);
        assert_eq!(dll_known_near_branch_target(&data,&pe,0x1010,false).unwrap(),0x1040);
        assert!(dll_known_near_branch_target(&data,&pe,0x1005,false).is_err());
        assert!(dll_known_near_branch_target(&data,&pe,0x1010,true).is_err());
        data[0x211..0x215].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(dll_known_near_branch_target(&data,&pe,0x1010,false).is_err());
    }

    #[test]
    fn dll_indirect_call_requires_exact_ff15_and_valid_base() {
        let mut image = synthetic_pe();
        let sec = 0x178usize;
        image[sec..sec+8].copy_from_slice(b".text\0\0\0");
        image[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        image[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        image[0x208..0x20E].copy_from_slice(&[0xFF,0x15,0x20,0xE0,0x8F,0x00]);
        let pe = parse_pe32(&image).unwrap();
        assert_eq!(dll_indirect_import_call(&image,&pe,0x1008,0x8E0000).unwrap(),
                   (0x8FE020,0x1E020));
        assert!(dll_indirect_import_call(&image,&pe,0x1007,0x8E0000).is_err());
        assert!(dll_indirect_import_call(&image,&pe,0x1008,0x900000).is_err());
        image[0x208] = 0x2B;
        assert!(dll_indirect_import_call(&image,&pe,0x1008,0x8E0000).is_err());
    }

    #[test]
    fn cross_section_static_calls_are_decoded_not_assumed() {
        let mut dll = synthetic_pe();
        let sec = 0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        // A forward rel32 direct CALL at RVA 0x1004 => 0x1070.
        dll[0x204] = 0xE8;
        dll[0x205..0x209].copy_from_slice(&0x67u32.to_le_bytes());
        // A nearby *indirect* IAT CALL: FF 15 absolute_VA32.
        dll[0x209..0x20F].copy_from_slice(&[0xFF,0x15,0x20,0xE0,0x8F,0x00]);
        let pe = parse_pe32(&dll).unwrap();
        assert_eq!(dll_relative_call_target(&dll,&pe,0x1004).unwrap(),0x1070);
        assert_eq!(dll_indirect_import_call(&dll,&pe,0x1009,0x8E0000).unwrap(),
            (0x8FE020,0x1E020));
        dll[0x204] = 0x90;
        assert!(dll_relative_call_target(&dll,&pe,0x1004).is_err());
        dll[0x209] = 0x90;
        assert!(dll_indirect_import_call(&dll,&pe,0x1009,0x8E0000).is_err());
    }

    #[test]
    fn protected_txt_cross_references_require_highlow_and_executable_source() {
        let mut dll = synthetic_pe();
        dll.resize(0xA00,0);
        dll[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        let sec0 = 0x178usize;
        dll[sec0..sec0+8].copy_from_slice(b".text\0\0\0");
        dll[sec0+8..sec0+12].copy_from_slice(&0x180u32.to_le_bytes());
        dll[sec0+12..sec0+16].copy_from_slice(&0x1000u32.to_le_bytes());
        dll[sec0+16..sec0+20].copy_from_slice(&0x200u32.to_le_bytes());
        dll[sec0+36..sec0+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let sec1=sec0+40;
        dll[sec1..sec1+8].copy_from_slice(b".txt\0\0\0\0");
        dll[sec1+8..sec1+12].copy_from_slice(&0x180u32.to_le_bytes());
        dll[sec1+12..sec1+16].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[sec1+16..sec1+20].copy_from_slice(&0x200u32.to_le_bytes());
        dll[sec1+20..sec1+24].copy_from_slice(&0x600u32.to_le_bytes());
        dll[sec1+36..sec1+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let base=0x008E_0000u32;
        // Only this value is associated with a synthetic HIGHLOW relocation.
        dll[0x220..0x224].copy_from_slice(&(base+0x2050).to_le_bytes());
        // This unrelocated matching byte pattern must NOT be reported.
        dll[0x228..0x22C].copy_from_slice(&(base+0x2060).to_le_bytes());
        // Relocated pointer points outside target section.
        dll[0x230..0x234].copy_from_slice(&(base+0x1600).to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        let blocks=[RelocBlock {page_rva:0x1000,highlow_offsets:vec![0x20,0x30]}];
        let (count, found) = dll_code_reloc_refs_to_section(
            &dll,&pe,&blocks,base,pe.section(b".txt").unwrap(),false).unwrap();
        assert_eq!(count,2);
        assert_eq!(found,vec![DllCodeReference {
            source_rva:0x1020, source_section:".text".to_string(),target_rva:0x2050
        }]);
        // No code-section relocation => no pointer reported.
        dll[sec0+36..sec0+40].copy_from_slice(&0x4000_0040u32.to_le_bytes());
        let nonexec=parse_pe32(&dll).unwrap();
        let (count,empty)=dll_code_reloc_refs_to_section(
            &dll,&nonexec,&blocks,base,nonexec.section(b".txt").unwrap(),false).unwrap();
        assert_eq!(count,0);
        assert!(empty.is_empty());
        let (all_count, all_refs)=dll_code_reloc_refs_to_section(
            &dll,&nonexec,&blocks,base,nonexec.section(b".txt").unwrap(),true).unwrap();
        assert_eq!(all_count,2);
        assert_eq!(all_refs.len(),1);
        assert_eq!(all_refs[0].source_rva,0x1020);
        assert_eq!(all_refs[0].target_rva,0x2050);
        assert_eq!(all_refs[0].source_section,".text");
        assert_eq!(count,0);
        assert!(empty.is_empty());
    }

    #[test]
    fn dll_literal_scanner_excludes_relocations_and_protected_source() {
        let mut dll = synthetic_pe();
        dll.resize(0xA00,0);
        dll[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        let first=0x178usize;
        dll[first..first+8].copy_from_slice(b".text\0\0\0");
        dll[first+8..first+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[first+12..first+16].copy_from_slice(&0x1000u32.to_le_bytes());
        dll[first+16..first+20].copy_from_slice(&0x100u32.to_le_bytes());
        dll[first+36..first+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let second=first+40;
        dll[second..second+8].copy_from_slice(b".txt\0\0\0\0");
        dll[second+8..second+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[second+12..second+16].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[second+16..second+20].copy_from_slice(&0x100u32.to_le_bytes());
        dll[second+20..second+24].copy_from_slice(&0x600u32.to_le_bytes());
        dll[second+36..second+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let base=0x008E_0000u32;
        dll[0x208..0x20C].copy_from_slice(&0x2050u32.to_le_bytes());
        dll[0x220..0x224].copy_from_slice(&(base+0x2060).to_le_bytes());
        dll[0x240..0x244].copy_from_slice(&(base+0x2070).to_le_bytes());
        // This literal is exactly a candidate but marked relocated.
        let blocks=[RelocBlock{page_rva:0x1000,highlow_offsets:vec![0x40]}];
        let pe=parse_pe32(&dll).unwrap();
        let hits=dll_unrelocated_literals(&dll,pe.section(b".text").unwrap(),
            &blocks,base,pe.section(b".txt").unwrap()).unwrap();
        assert!(hits.rvas.contains(&DllLiteralHit{source_rva:0x1008,value:0x2050}));
        assert!(hits.absolute_vas.contains(&DllLiteralHit{source_rva:0x1020,value:base+0x2060}));
        assert!(!hits.absolute_vas.iter().any(|h|h.source_rva==0x1040));
        // End-exclusive bounds.
        dll[0x250..0x254].copy_from_slice(&0x2100u32.to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        let hits=dll_unrelocated_literals(&dll,pe.section(b".text").unwrap(),
            &blocks,base,pe.section(b".txt").unwrap()).unwrap();
        assert!(!hits.rvas.iter().any(|h|h.source_rva==0x1050));
    }

    #[test]
    fn literal_context_requires_bounded_verified_word() {
        let mut image=synthetic_pe();
        let sec=0x178usize;
        image[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        image[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[0x240..0x244].copy_from_slice(&0x7800u32.to_le_bytes());
        let pe=parse_pe32(&image).unwrap();
        assert!(print_dll_exact_literal_context(
            &image,&pe,0x1040,0x7800,"fixture").is_ok());
        assert!(print_dll_exact_literal_context(
            &image,&pe,0x1040,0x1000,"wrong value").is_err());
        assert!(print_dll_exact_literal_context(
            &image,&pe,0x10FF,0x7800,"out of backed section").is_err());
    }

    #[test]
    fn reference_text_audit_preserves_relocation_bytes_and_detects_ambiguous_imports() {
        let base=0x400000u32;
        let mut cipher=vec![0x55u8;64];
        let mut plain=cipher.clone();
        let reloc=RelocBlock{page_rva:0x1000,highlow_offsets:vec![4,12,20]};
        let idata=PeSection{
            name:*b".idata\0\0",virtual_address:0x3000,virtual_size:0x100,
            raw_offset:0,raw_len:0x100,characteristics:0xC0000040
        };
        cipher[4..8].copy_from_slice(&(base+0x3010).to_le_bytes());
        plain[4..8].copy_from_slice(&(base+0x3020).to_le_bytes());
        cipher[12..16].copy_from_slice(&(base+0x3010).to_le_bytes());
        plain[12..16].copy_from_slice(&(base+0x3030).to_le_bytes());
        cipher[20..24].copy_from_slice(&(base+0x3040).to_le_bytes());
        plain[20..24].copy_from_slice(&(base+0x3040).to_le_bytes());
        // One ordinary nonrelocated byte is modified.
        cipher[40]=0xEF;
        let a=audit_reference_text_bytes(&cipher,&plain,0x1000,&[reloc],base,&idata).unwrap();
        assert_eq!(a.relocation_words,3);
        assert_eq!(a.identical_relocation_words,1);
        assert_eq!(a.relocated_bytes,12);
        assert_eq!(a.identical_relocated_bytes,10);
        assert_eq!(a.plain_bytes,52);
        assert_eq!(a.identical_plain_bytes,51);
        assert_eq!(a.import_pointer_mismatches,2);
        assert_eq!(a.ambiguous_import_va_sources,1);
        assert_eq!(a.first_page_plain_total,52);
        assert_eq!(a.first_page_plain_equal,51);
    }

    #[test]
    fn original_unsafedisc_155_section_name_exclusions_and_crc_code_bit() {
        let mut section = PeSection {
            name:*b".text\0\0\0", virtual_address:0x1000,
            virtual_size:0x180, raw_offset:0x400,raw_len:0x200,
            characteristics:0x6000_0020,
        };
        assert!(historical_155_section_selected(&section));
        assert!(historical_155_section_checksum_eligible(&section));
        section.name=*b".data\0\0\0";
        section.characteristics=0xC000_0040;
        assert!(historical_155_section_selected(&section));
        assert!(!historical_155_section_checksum_eligible(&section));
        for name in [
            *b".rsrc\0\0\0",*b".bss\0\0\0\0",*b"BSS\0\0\0\0\0",
            *b".tls\0\0\0\0",*b".idata\0\0",
            *b".rdata\0\0",*b".edata\0\0",*b".reloc\0\0",
        ] {
            section.name=name;
            section.characteristics=0x6000_0020;
            assert!(!historical_155_section_selected(&section),
                "historical original unSafeDisc must exclude {:?}",name);
            assert!(!historical_155_section_checksum_eligible(&section));
        }
    }

    #[test]
    fn historical_155_import_unrotation_matches_known_crimson_sites() {
        // First and second KERNEL32 examples; exact original private
        // ICD vs TESTME data, independently validated offline.
        assert_eq!(historical_155_unrotate_import_va(
            0x1294,0x00A1_FD78,REPORTED_FIRST_WORD,0x1000),
            Some(0x00A1_FD9C));
        assert_eq!(historical_155_unrotate_import_va(
            0x12D4,0x00A1_FD50,REPORTED_FIRST_WORD,0x1000),
            Some(0x00A1_FD9C));
        // Two different USER32 sites, modulo 62.
        assert_eq!(historical_155_unrotate_import_va(
            0x48C5,0x00A2_0458,REPORTED_FIRST_WORD,0x1000),
            Some(0x00A2_0430));
        assert_eq!(historical_155_unrotate_import_va(
            0x48E5,0x00A2_03C0,REPORTED_FIRST_WORD,0x1000),
            Some(0x00A2_0410));
        // Reject inputs which cannot belong to the two specific tables.
        assert_eq!(historical_155_unrotate_import_va(
            0x1294,0x00A1_0000,REPORTED_FIRST_WORD,0x1000),None);
        for (first,count) in [(0x00A1_FC7Cu32,74u32),(0x00A2_039Cu32,62u32)] {
            for index in 0..count {
                let x=first+index*4;
                let corrected=historical_155_unrotate_import_va(
                    0x2000,x,REPORTED_FIRST_WORD,0x1000).unwrap();
                assert!(corrected>=first && corrected<first+4*count);
                assert_eq!((corrected-first)%4,0);
            }
        }
    }

    #[test]
    fn relative_call_into_protected_section_is_distinct_from_highlow_import_va() {
        let mut image=synthetic_pe();
        image.resize(0xA00,0);
        image[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        let first=0x178usize;
        image[first..first+8].copy_from_slice(b".text\0\0\0");
        image[first+8..first+12].copy_from_slice(&0x200u32.to_le_bytes());
        image[first+12..first+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[first+16..first+20].copy_from_slice(&0x200u32.to_le_bytes());
        image[first+36..first+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let second=first+40;
        image[second..second+8].copy_from_slice(b".txt\0\0\0\0");
        image[second+8..second+12].copy_from_slice(&0x180u32.to_le_bytes());
        image[second+12..second+16].copy_from_slice(&0x2000u32.to_le_bytes());
        image[second+16..second+20].copy_from_slice(&0x200u32.to_le_bytes());
        image[second+20..second+24].copy_from_slice(&0x600u32.to_le_bytes());
        image[second+36..second+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        // At RVA 0x1050, call .txt RVA 0x2050 by signed rel32.
        let site=0x1050u32;
        let dest=0x2050u32;
        let disp=(i64::from(dest)-i64::from(site+5)) as i32;
        image[0x250]=0xE8;
        image[0x251..0x255].copy_from_slice(&disp.to_le_bytes());
        let pe=parse_pe32(&image).unwrap();
        assert_eq!(dll_relative_call_target(&image,&pe,site).unwrap(),dest);
        // These raw bytes have no corresponding HIGHLOW relocation!
        let source=pe.section(b".txt").unwrap();
        let (count,hits)=dll_code_reloc_refs_to_section(
            &image,&pe,&[],0x008E0000,source,true).unwrap();
        assert_eq!(count,0);
        assert!(hits.is_empty());
    }

    #[test]
    fn encrypted_dll_relocation_slots_remain_classifiable_with_highlow_metadata() {
        let mut dll=synthetic_pe();
        dll.resize(0xA00,0);
        dll[0x86..0x88].copy_from_slice(&3u16.to_le_bytes());
        let first=0x178usize;
        dll[first..first+8].copy_from_slice(b".txt\0\0\0\0");
        dll[first+8..first+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[first+12..first+16].copy_from_slice(&0x1000u32.to_le_bytes());
        dll[first+16..first+20].copy_from_slice(&0x100u32.to_le_bytes());
        dll[first+36..first+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let second=first+40;
        dll[second..second+8].copy_from_slice(b".data\0\0\0");
        dll[second+8..second+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[second+12..second+16].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[second+16..second+20].copy_from_slice(&0x100u32.to_le_bytes());
        dll[second+20..second+24].copy_from_slice(&0x600u32.to_le_bytes());
        dll[second+36..second+40].copy_from_slice(&0xC000_0040u32.to_le_bytes());
        let third=second+40;
        dll[third..third+8].copy_from_slice(b".rdata\0\0");
        dll[third+8..third+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[third+12..third+16].copy_from_slice(&0x3000u32.to_le_bytes());
        dll[third+16..third+20].copy_from_slice(&0x100u32.to_le_bytes());
        dll[third+20..third+24].copy_from_slice(&0x700u32.to_le_bytes());
        dll[third+36..third+40].copy_from_slice(&0x4000_0040u32.to_le_bytes());
        let base=0x008E_0000u32;
        for (offset,rva) in [(0x10usize,0x2004u32),(0x20,0x3008),(0x30,0x1080)] {
            dll[0x200+offset..0x204+offset].copy_from_slice(&(base+rva).to_le_bytes());
        }
        // The unmarked DWORD bytes are not counted, even if they
        // happen to look like a valid in-image absolute VA.
        dll[0x240..0x244].copy_from_slice(&(base+0x2004).to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        let blocks=[RelocBlock{page_rva:0x1000,highlow_offsets:vec![0x10,0x20,0x30]}];
        assert_eq!(audit_protected_dll_relocations(
            &dll,&pe,&blocks,base).unwrap(),ProtectedDllRelocAudit {
                slots:3,targets_data:1,targets_rdata:1,
                targets_txt:1,targets_other:0,out_of_image:0,page_crossings:0
        });
    }

    #[test]
    fn fixed_absolute_import_load_checks_original_x86_operand_encoding() {
        let mut image=synthetic_pe();
        // The general synthetic PE fixture has a non-executable
        // .idata section. Give this isolated instruction fixture
        // a real file-backed executable .text section.
        let sec=0x178usize;
        image[sec..sec+8].copy_from_slice(b".text\0\0\0");
        image[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        image[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        image[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let at=0x250usize;
        image[at..at+6].copy_from_slice(&[
            0x8B,0x2D,0xAC,0xE0,0x8F,0x00
        ]);
        let pe=parse_pe32(&image).unwrap();
        assert_eq!(dll_absolute_import_load(
            &image,&pe,0x1050,[0x8B,0x2D],0x008E0000).unwrap(),0x1E0AC);
        assert!(dll_absolute_import_load(
            &image,&pe,0x1050,[0x8B,0x3D],0x008E0000).is_err());
    }

    #[test]
    fn attach_opaque_flag_predicates_prove_first_conditional_call() {
        let mut dll=synthetic_pe();
        dll.resize(0x7200,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x7000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0x9000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x7000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (rva, bytes) in [
            (0x9787u32,&[0x83u8,0x7D,0xFC,0x01][..]),
            (0x978B,&[0x74,0x05]),(0x9792,&[0x76,0x04]),
            (0x9794,&[0x90,0x90,0xEB,0x0E]),
            (0x9798,&[0x74,0x07]),(0x97A1,&[0x76,0xF1]),
            (0x97A6,&[0x7B,0x11]),(0x97A8,&[0x7A,0x0F]),
            (0x97A9,&[0x0F,0x8C,0x0F,0,0,0]),
            (0x97AF,&[0x90,0x90]),(0x97B1,&[0x77,0x0B]),
            (0x97B3,&[0x7D,0x09]),(0x97B9,&[0x70,0xEE]),
            (0x97BB,&[0x71,0xEC]),
        ] {
            let at=0x200+(rva-0x9000) as usize;
            dll[at..at+bytes.len()].copy_from_slice(bytes);
        }
        let call=0x97BEu32;
        let disp=(i64::from(0xD62Cu32)-i64::from(call+5)) as i32;
        let offset=0x200+(call-0x9000) as usize;
        dll[offset]=0xE8;
        dll[offset+1..offset+5].copy_from_slice(&disp.to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        assert_eq!(dll_attach_first_call_path(&dll,&pe).unwrap(),vec![
            0x9787,0x978B,0x9792,0x9798,0x97A1,0x9794,0x9795,
            0x9796,0x97A6,0x97A8,0x97B9,0x97BB,0x97A9,
            0x97AF,0x97B0,0x97B1,0x97B3,0x97BE,
        ]);
        dll[0x200+(0x97B3-0x9000) as usize]=0xCC;
        assert!(dll_attach_first_call_path(&dll,&pe).is_err());
    }

    #[test]
    fn first_attach_callee_opaque_predicates_converge_for_eight_flag_states(){
        let mut dll=synthetic_pe();
        dll.resize(0x2200,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0xD000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (rva,bytes) in [
            (0xD62Cu32,&[0x55u8,0x8B,0xEC,0x83,0xEC,0x08,0x53,0x56,0x57][..]),
            (0xD635,&[0x76,0x0A]),(0xD637,&[0x90,0x90]),
            (0xD639,&[0x74,0x04]),(0xD63B,&[0x87,0xC0]),
            (0xD63D,&[0x77,0x00]),(0xD63F,&[0xEB,0x08]),
            (0xD649,&[0x7B,0x11]),(0xD64B,&[0x7A,0x0F]),
            (0xD64C,&[0x0F,0x8C,0x0F,0,0,0]),
            (0xD652,&[0x90,0x90]),(0xD654,&[0x77,0x0B]),
            (0xD65C,&[0x70,0xEE]),(0xD65E,&[0x71,0xEC]),
            (0xD661,&[0x76,0x0B]),(0xD663,&[0x50]),
        ]{
            let at=0x200+(rva-0xD000) as usize;
            dll[at..at+bytes.len()].copy_from_slice(bytes);
        }
        let at=0x200+(0xD664-0xD000) as usize;
        let disp=(i64::from(0xD7BEu32)-i64::from(0xD669u32)) as i32;
        dll[at]=0xE8;
        dll[at+1..at+5].copy_from_slice(&disp.to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        let paths=dll_d62c_first_call_paths(&dll,&pe).unwrap();
        assert_eq!(paths.len(),8);
        assert!(paths.iter().all(|p|p.last()==Some(&0xD664)));
        // Variations in parity/overflow/sign change control flow,
        // but all eight choices agree about the first callee.
        let unique=paths.iter().map(|p|p.clone()).collect::<std::collections::HashSet<_>>();
        assert!(unique.len()>1);
        dll[0x200+(0xD654-0xD000) as usize]=0xCC;
        assert!(dll_d62c_first_call_paths(&dll,&pe).is_err());
    }

    #[test]
    fn ret_decoy_is_verified_and_next_call_is_c5c0_for_every_flag_state() {
        let mut dll=synthetic_pe();
        dll.resize(0x2200,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0xC000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x2000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (rva,code) in [
            (0xD7BEu32,&[0xC3u8][..]),
            (0xD669,&[0x58]),(0xD66A,&[0x77,0x02]),
            (0xD66E,&[0x66,0xC7,0x45,0xF8,0x01,0x00]),
            (0xD674,&[0x76,0x11]),(0xD676,&[0x77,0x0F]),
            (0xD687,&[0x78,0xEE]),(0xD689,&[0x79,0xEC]),
            (0xD677,&[0x0F,0x8B,0x0F,0,0,0]),
            (0xD67D,&[0x7A,0x0D]),
        ]{
            let offset=0x200+(rva-0xC000) as usize;
            dll[offset..offset+code.len()].copy_from_slice(code);
        }
        for (call,target) in [(0xD664u32,0xD7BEu32),(0xD68C,0xC5C0)] {
            let off=0x200+(call-0xC000) as usize;
            let disp=(i64::from(target)-i64::from(call+5)) as i32;
            dll[off]=0xE8;
            dll[off+1..off+5].copy_from_slice(&disp.to_le_bytes());
        }
        let pe=parse_pe32(&dll).unwrap();
        let paths=dll_d62c_after_ret_paths(&dll,&pe).unwrap();
        assert_eq!(paths.len(),8);
        assert!(paths.iter().all(|p|p.last()==Some(&0xD68C)));
        // PF and SF lead to different, overlapping byte paths,
        // but all must converge at the same nontrivial CALL.
        let distinct=paths.iter().cloned().collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(),4);
        dll[0x200+(0xD7BE-0xC000) as usize]=0x90;
        assert!(dll_d62c_after_ret_paths(&dll,&pe).is_err());
    }

    #[test]
    fn third_attach_level_four_flag_states_reach_e25c(){
        let mut dll=synthetic_pe();
        dll.resize(0x6200,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x6000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0x9000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x6000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (rva,bytes) in [
            (0xC5C0u32,&[0x55u8,0x8B,0xEC,0x83,0xEC,0x08,0x53,0x56,0x57][..]),
            (0xC5C9,&[0x7F,0x09]),(0xC5D4,&[0x77,0x00]),
            (0xC5D6,&[0x7F,0xF3]),(0xC5CB,&[0x90]),
            (0xC5CC,&[0x87,0xED]),(0xC5CE,&[0x78,0x09]),
            (0xC5D0,&[0x87,0xD2]),(0xC5D2,&[0x79,0x05]),
            (0xC5D9,&[0x68,0x08,0x05,0x90,0x00]),
        ]{
            let off=0x200+(rva-0x9000) as usize;
            dll[off..off+bytes.len()].copy_from_slice(bytes);
        }
        let off=0x200+(0xC5DE-0x9000) as usize;
        let disp=(i64::from(0xE25Cu32)-i64::from(0xC5E3u32)) as i32;
        dll[off]=0xE8;
        dll[off+1..off+5].copy_from_slice(&disp.to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        let paths=dll_c5c0_first_call_paths(&dll,&pe).unwrap();
        assert_eq!(paths.len(),4);
        assert!(paths.iter().all(|p|p.last()==Some(&0xC5DE)));
        let distinct=paths.iter().cloned().collect::<std::collections::HashSet<_>>();
        assert!(distinct.len()>1);
        dll[0x200+(0xC5D9-0x9000) as usize]=0xCC;
        assert!(dll_c5c0_first_call_paths(&dll,&pe).is_err());
    }

    #[test]
    fn e25c_all_flag_states_reach_allocator_then_df96_null_check(){
        let mut dll=synthetic_pe();
        dll.resize(0x3300,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x3000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0xD000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x3000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let fragments:&[(u32,&[u8])]=&[
            (0xE25C,&[0x55,0x8B,0xEC,0x51,0x53,0x56,0x57]),
            (0xE263,&[0x7B,0x11]),(0xE265,&[0x7A,0x0F]),
            (0xE266,&[0x0F,0x8C,0x0F,0,0,0]),
            (0xE26C,&[0x90,0x90]),(0xE26E,&[0x77,0x0B]),
            (0xE270,&[0x87,0xF6]),(0xE272,&[0x7D,0x07]),
            (0xE274,&[0x74,0x00]),(0xE276,&[0x70,0xEE]),
            (0xE278,&[0x71,0xEC]),(0xE27B,&[0x73,0x0B]),
            (0xE27D,&[0x74,0x07]),(0xE27F,&[0x87,0xFF]),
            (0xE281,&[0x77,0x03]),(0xE283,&[0x87,0xC9]),
            (0xE285,&[0x90]),(0xE286,&[0xEB,0x03]),
            (0xE288,&[0x73,0xF3]),(0xE28B,&[0x7C,0x0B]),
            (0xE28D,&[0x50]),(0xE293,&[0x58]),
            (0xE294,&[0x7D,0x02]),(0xE298,&[0x68,0x80,0,0,0]),
            (0xE2A2,&[0x83,0xC4,0x04,0x89,0x45,0xFC]),
            (0xE2A8,&[0xEB,0x07]),(0xE2AA,&[0x8B,0xC9]),
            (0xE2AC,&[0x78,0x06]),(0xE2AE,&[0x90]),
            (0xE2AF,&[0x79,0x03]),(0xE2B1,&[0xEB,0xF7]),
            (0xE2B4,&[0x7B,0x11]),(0xE2B6,&[0x7A,0x0F]),
            (0xE2B7,&[0x0F,0x8C,0x0F,0,0,0]),
            (0xE2BD,&[0x7D,0x0D]),(0xE2C7,&[0x70,0xEE]),
            (0xE2C9,&[0x71,0xEC]),
            (0xE2CC,&[0x83,0x7D,0xFC,0x00,0x75,0x04]),
            (0xE2D6,&[0x8B,0x45,0x08,0x50,0x8B,0x4D,0xFC,0x51]),
            (0xE2FE,&[0xC3]),
            (0xF850,&[0xA1,0x14,0x52,0x90,0x00,0x8B,0x4C,0x24,0x04,0x50,0x51]),
        ];
        for &(rva,bytes) in fragments {
            let offset=0x200+(rva-0xD000) as usize;
            dll[offset..offset+bytes.len()].copy_from_slice(bytes);
        }
        for (call,target) in [
            (0xE28Eu32,0xE2FEu32),(0xE29D,0xF850),
            (0xE2DE,0xDF96),(0xF85B,0xF870),(0xF88E,0xF8C0)
        ]{
            let offset=0x200+(call-0xD000) as usize;
            dll[offset]=0xE8;
            let rel=(i64::from(target)-i64::from(call+5)) as i32;
            dll[offset+1..offset+5].copy_from_slice(&rel.to_le_bytes());
        }
        let pe=parse_pe32(&dll).unwrap();
        assert_eq!(dll_e25c_allocator_paths(&dll,&pe).unwrap(),(32,8,16));
        let offset=0x200+(0xE2FE-0xD000) as usize;
        dll[offset]=0x90;
        assert!(dll_e25c_allocator_paths(&dll,&pe).is_err());
    }

    #[test]
    fn e205_xor_all_32_flag_states_and_byte_math(){
        let mut dll=synthetic_pe();
        dll.resize(0x3300,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x3000u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0xD000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x3000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let parts:&[(u32,&[u8])]=&[
            (0xE205,&[0x55,0x8B,0xEC,0x51,0x53,0x56,0x57]),
            (0xE20C,&[0xEB,0x07]),(0xE20E,&[0x8B,0xED]),
            (0xE210,&[0x78,0x06]),(0xE212,&[0x90]),
            (0xE213,&[0x79,0x03]),(0xE215,&[0xEB,0xF7]),
            (0xE218,&[0x7F,0x09]),(0xE21A,&[0x90]),
            (0xE21B,&[0x87,0xDB]),(0xE21D,&[0x78,0x09]),
            (0xE21F,&[0x87,0xD2]),(0xE221,&[0x79,0x05]),
            (0xE223,&[0x77,0x00]),(0xE225,&[0x7F,0xF3]),
            (0xE228,&[0x8B,0x45,0x08,0x25,0xFF,0,0,0,
                0x8B,0x4D,0x0C,0x81,0xE1,0xFF,0,0,0,0x33,0xC1,0x88,0x45,0xFC]),
            (0xE23E,&[0x7C,0x06,0x90,0x90,0x77,0x00,0xEB,0x0C]),
            (0xE252,&[0x8A,0x45,0xFC,0x5F,0x5E,0x5B,0x8B,0xE5,0x5D,0xC3]),
        ];
        for &(rva,bytes) in parts {
            let off=0x200+(rva-0xD000) as usize;
            dll[off..off+bytes.len()].copy_from_slice(bytes);
        }
        let site=0xE11Eu32;
        let disp=(i64::from(0xE205u32)-i64::from(site+5)) as i32;
        let off=0x200+(site-0xD000) as usize;
        dll[off]=0xE8;
        dll[off+1..off+5].copy_from_slice(&disp.to_le_bytes());
        let pe=parse_pe32(&dll).unwrap();
        assert_eq!(dll_e205_xor_proof(&dll,&pe).unwrap(),4);
        assert_eq!(e205_byte_xor_math(0x1000FF,0x3011),0xEE);
        assert_eq!(e205_byte_xor_math(0,0),0);
        assert_eq!(e205_byte_xor_math(0xFF,0x80),0x7F);
        for i in 0..256u32 {
            assert_eq!(e205_byte_xor_math(i,0x55),(i as u8)^0x55);
        }
        let off=0x200+(0xE228-0xD000) as usize;
        dll[off+3]=0x24; // Tamper with AND EAX,0xFF opcode.
        assert!(dll_e205_xor_proof(&dll,&pe).is_err());
    }

    #[test]
    fn protected_copy_instruction_check_is_bounded_and_fail_closed(){
        let mut dll=synthetic_pe();
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x100u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0x1000u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        dll[0x260..0x262].copy_from_slice(&[0xF3,0xA5]);
        let pe=parse_pe32(&dll).unwrap();
        assert!(dll_expect_raw_x86(&dll,&pe,0x1060,&[0xF3,0xA5]).is_ok());
        assert!(dll_expect_raw_x86(&dll,&pe,0x1060,&[0xF3,0xA4]).is_err());
        assert!(dll_expect_raw_x86(&dll,&pe,0xF000,&[0xF3,0xA5]).is_err());
        assert!(dll_expect_raw_x86(&dll,&pe,0x1060,&[]).is_err());
    }

    #[test]
    fn synthetic_byte_recurrence_vectors_and_bounds(){
        assert!(synthetic_chunk_byte_model(&[],0x43,0x15).is_empty());
        assert_eq!(synthetic_chunk_byte_model(&[0],0x43,0x15),vec![0x43]);
        assert_eq!(synthetic_chunk_byte_model(&[0,0,0],0x43,0x15),vec![0x43,0x99,0x88]);
        let src=[0x10,0x20,0x30,0x40,0x50];
        let actual=synthetic_chunk_byte_model(&src,0x43,0x15);
        assert_eq!(actual.len(),src.len());
        assert_ne!(actual,src);
        assert_eq!(actual[0],0x53);
    }

    #[test]
    fn synthetic_chunk_model_inverse_roundtrip_boundary_lengths(){
        for len in [0usize,1,2,3,15,16,4095,4096,4097] {
            let original:Vec<u8>=(0..len).map(|i|
                ((i as u32).wrapping_mul(73).wrapping_add((i>>8) as u32)) as u8
            ).collect();
            let encoded=synthetic_chunk_byte_model(&original,0x43,0x15);
            assert_eq!(encoded.len(),original.len());
            let decoded=synthetic_chunk_byte_inverse(&encoded,0x43,0x15);
            assert_eq!(decoded,original);
        }
        for first in [0u8,0x43,0xFF] {
            for second in [0u8,0x15,0xFF] {
                let source=[0u8,1,0xFF,0x80,0xAA,0x55];
                let transformed=synthetic_chunk_byte_model(&source,first,second);
                assert_eq!(synthetic_chunk_byte_inverse(&transformed,first,second),source);
            }
        }
    }

    #[test]
    fn synthetic_category_field_geometry_is_bounded_to_eight_slots(){
        assert_eq!(synthetic_range_category_field(0),Some(0x14));
        assert_eq!(synthetic_range_category_field(1),Some(0x18));
        assert_eq!(synthetic_range_category_field(7),Some(0x30));
        assert_eq!(synthetic_range_category_field(8),None);
        assert_eq!(synthetic_range_category_field(u32::MAX),None);
    }

    #[test]
    fn synthetic_c350_range_extension_uses_exclusive_end_and_matching_tag(){
        let mut record=(100u32,10u32,3u32);
        assert!(synthetic_extend_matching_range(&mut record,104,1,3));
        assert_eq!(record,(100,10,3));
        assert!(!synthetic_extend_matching_range(&mut record,105,10,4));
        assert_eq!(record,(100,10,3));
        assert!(synthetic_extend_matching_range(&mut record,105,10,3));
        assert_eq!(record,(100,15,3)); // end-exclusive 115, not 114
        assert!(!synthetic_extend_matching_range(&mut record,115,1,3));
        assert!(!synthetic_extend_matching_range(&mut record,99,1,3));
    }

    #[test]
    fn synthetic_selector_distinguishes_raw_offset_rva_and_virtual_extent(){
        assert_eq!(synthetic_selector_fields(0x400,0x1000,0x7671,0x7800),
                   (0x400,0x1000,0x7671));
        assert_eq!(synthetic_selector_fields(0x400,0x1000,0,0x7800),
                   (0x400,0x1000,0x7800));
        assert_eq!(synthetic_selector_fields(0x400,0x1000,0x8000,0x7800),
                   (0x400,0x1000,0x7800));
    }

    #[test]
    fn synthetic_directory_exclusion_removes_iat_and_reloc_sections(){
        let sections=[(0x1000,0x6000_0020),(0x1E000,0x4000_0040),
                      (0x20000,0xC000_0040),(0x27000,0x4200_0040)];
        let mut directories=[0u32;16];
        directories[5]=0x27000;directories[12]=0x1E000;
        assert_eq!(synthetic_section_excluding_directories(&sections,0xC000_0040,0xC000_0040,&directories),Some(2));
        assert_eq!(synthetic_section_excluding_directories(&sections,0xC000_0060,0x4000_0040,&directories),None);
        directories[12]=0;
        assert_eq!(synthetic_section_excluding_directories(&sections,0xC000_0060,0x4000_0040,&directories),Some(1));
    }

    #[test]
    fn synthetic_category_section_filters_and_first_matches(){
        let flags=[0x6000_0020u32,0x4000_0040,0xC000_0040,0x4200_0040];
        assert_eq!(synthetic_first_section_with_mask(&flags,0x2000_0020,0x2000_0020),Some(0));
        assert_eq!(synthetic_first_section_with_mask(&flags,0xC000_0040,0xC000_0040),Some(2));
        assert_eq!(synthetic_first_section_with_mask(&flags,0xC000_0060,0x4000_0040),Some(1));
        assert_eq!(synthetic_first_section_with_mask(&[],0x2000_0020,0x2000_0020),None);
        // The match on a .reloc-like 0x42000040 is only a preliminary flag match.
        assert_eq!(synthetic_first_section_with_mask(&[0x4200_0040],0xC000_0060,0x4000_0040),Some(0));
    }

    #[test]
    fn synthetic_entropy_is_bounded_and_handles_constant_data(){
        assert_eq!(synthetic_byte_entropy(&[]),0.0);
        assert_eq!(synthetic_byte_entropy(&[4,4,4,4]),0.0);
        assert!((synthetic_byte_entropy(&[0,1])-1.0).abs()<1e-12);
        let all:Vec<u8>=(0u16..256).map(|x|x as u8).collect();
        assert!((synthetic_byte_entropy(&all)-8.0).abs()<1e-12);
    }

    #[test]
    fn synthetic_original_prologue_xor_direction_is_unambiguous() {
        // First byte only; deliberately no proprietary code is used here.
        assert_eq!(0x55u8 ^ 0x43,0x16u8);
        assert_eq!(0x16u8 ^ 0x43,0x55u8);
        assert_ne!(0x16u8,0x55u8);
    }

    #[test]
    fn synthetic_selector_failure_clears_even_previous_successful_extent(){
        for matched in [false,true] {
            let (out,flag)=synthetic_exhausted_selector_outputs(
                [0x16200,0x18000,0x5497],matched);
            assert_eq!(out,[0;3]);
            assert_eq!(flag,matched);
            assert_eq!(out[2],0);
        }
    }

    #[test]
    fn synthetic_section_cursor_detour_converges_for_all_flag_inputs() {
        let mut different=std::collections::HashSet::new();
        for cf in [false,true] {
            for zf in [false,true] {
                for of in [false,true] {
                    let path=synthetic_section_cursor_detour(cf,zf,of).unwrap();
                    assert!(!path.contains(&0x93BE));
                    assert!(path.contains(&0x93B3));
                    assert_eq!(path.last(),
                        Some(&if cf || zf {0x93B3} else {0x93BC}));
                    different.insert(path);
                }
            }
        }
        assert!(different.len()>=4);
    }

    #[test]
    fn short_section_name_chain_decoder_handles_boundaries() {
        // Self-contained synthetic string; no original protected content.
        assert_eq!(decode_original_section_tag(&[0x4F,0x05,0]).unwrap(),b"OK");
        assert!(decode_original_section_tag(&[]).is_err());
        assert!(decode_original_section_tag(&[0]).is_err());
        assert!(decode_original_section_tag(&[0x41,0x43]).is_err());
        assert!(decode_original_section_tag(&vec![0x41;65]).is_err());
        assert_eq!(decode_original_section_tag(&[0x41,0]).unwrap(),b"A");
    }

    #[test]
    fn synthetic_first_code_exec_section_respects_header_order(){
        assert_eq!(synthetic_first_code_exec(&[0x4000_0040,0x6000_0020,0x6000_0020]),Some(1));
        assert_eq!(synthetic_first_code_exec(&[0x6000_0020,0x6000_0020,0x6000_0020]),Some(0));
        assert_eq!(synthetic_first_code_exec(&[0x4000_0040,0xC000_0040]),None);
    }

    #[test]
    fn synthetic_range_ordering_is_strict_unsigned_less_than(){
        assert!(synthetic_probe_before_start(100,99));
        assert!(!synthetic_probe_before_start(100,100));
        assert!(!synthetic_probe_before_start(100,101));
        assert!(synthetic_probe_before_start(0x8000_0000,0x7FFF_FFFF));
        assert!(!synthetic_probe_before_start(0x7FFF_FFFF,0x8000_0000));
    }

    #[test]
    fn synthetic_same_tag_adjacency_both_sides_and_wrapping(){
        let a=(100,20,4);
        assert!(synthetic_adjacent_ranges(a,(120,10,4)));
        assert!(synthetic_adjacent_ranges(a,(85,15,4)));
        assert!(!synthetic_adjacent_ranges(a,(120,10,5)));
        assert!(!synthetic_adjacent_ranges(a,(110,5,4)));
        assert!(!synthetic_adjacent_ranges(a,(130,5,4)));
        assert!(synthetic_adjacent_ranges((0xFFFF_FFF0,0x20,7),(0x10,10,7)));
    }

    #[test]
    fn original_chunk_state_constants_are_recovered_from_x86_immediates(){
        let mut dll=synthetic_pe();
        dll.resize(0x600,0);
        let sec=0x178usize;
        dll[sec..sec+8].copy_from_slice(b".text\0\0\0");
        dll[sec+8..sec+12].copy_from_slice(&0x400u32.to_le_bytes());
        dll[sec+12..sec+16].copy_from_slice(&0x9000u32.to_le_bytes());
        dll[sec+16..sec+20].copy_from_slice(&0x400u32.to_le_bytes());
        dll[sec+36..sec+40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        for (rva,bytes) in [
            (0x92B3u32,&[0xB8,0x2C,0x10,0,0][..]),
            (0x92F6,&[0xB8,0xC9,0x7E,0xE9,0xF3]),
            (0x92FB,&[0xBB,0x8A,0x7E,0xE9,0xF3]),
            (0x9302,&[0x33,0xC3]),(0x9305,&[0x89,0x45,0xE8]),
            (0x9309,&[0xBB,0xE6,0xBD,0x02,0xC5]),
            (0x9313,&[0xB9,0xD1,0xBD,0x02,0xC5]),
            (0x931A,&[0x2B,0xD9]),(0x931C,&[0x8B,0xC3]),
            (0x931F,&[0x89,0x45,0xE0]),
        ]{
            let offset=0x200+(rva-0x9000) as usize;
            dll[offset..offset+bytes.len()].copy_from_slice(bytes);
        }
        let pe=parse_pe32(&dll).unwrap();
        assert_eq!(dll_original_chunk_constants(&dll,&pe).unwrap(),(0x43,0x15));
        dll[0x200+(0x931A-0x9000) as usize]=0x90;
        assert!(dll_original_chunk_constants(&dll,&pe).is_err());
    }

    #[test]
    fn synthetic_indexed_modifier_observations_separate_different_word_masks() {
        let mut f=std::collections::HashMap::new();
        let mut r=std::collections::HashMap::new();
        assert_eq!(insert_indexed_modifier_observation(
            &mut f,&mut r,0,123,999),(false,false,false));
        assert_eq!(insert_indexed_modifier_observation(
            &mut f,&mut r,0,123,999),(true,false,false));
        // Same input but different index is not a contradiction:
        // the mask is XORed between F1 and F2.
        assert_eq!(insert_indexed_modifier_observation(
            &mut f,&mut r,1,123,998),(false,false,false));
        assert_eq!(insert_indexed_modifier_observation(
            &mut f,&mut r,0,123,998),(true,true,false));
        assert_eq!(insert_indexed_modifier_observation(
            &mut f,&mut r,0,124,999),(false,false,true));
        assert_eq!(f.len(),2);
        assert_eq!(r.len(),2);
    }

    #[test]
    fn composite_eax_pair_audit_detects_nonfunctional_and_noninjective_samples() {
        let mut f=std::collections::HashMap::new();
        let mut r=std::collections::HashMap::new();
        assert_eq!(insert_observed_composite_pair(&mut f,&mut r,1,101),(false,false));
        assert_eq!(insert_observed_composite_pair(&mut f,&mut r,1,101),(false,false));
        assert_eq!(insert_observed_composite_pair(&mut f,&mut r,2,202),(false,false));
        assert_eq!(insert_observed_composite_pair(&mut f,&mut r,1,303),(true,false));
        assert_eq!(insert_observed_composite_pair(&mut f,&mut r,3,202),(false,true));
        assert_eq!(f.len(),2);
        assert_eq!(r.len(),2);
        assert_eq!(f.get(&1),Some(&101));
        assert_eq!(r.get(&202),Some(&2));
    }

    #[test]
    fn known_first_word_matches_four_word_reference_proof() {
        // Four words only for the exact pinned Crimson Skies inputs.
        // This test alone is NOT the independent oracle; --key-research
        // with the SHA-256-validated reference performs that check.
        assert_eq!(REPORTED_FIRST_WORD,0x7496_538F);
        assert_eq!(construct_tea_candidate(
            REPORTED_FIRST_WORD,0x1B09_633B,0x001F_7056,
            [0xE51D_C3D7,0x63CA_5EAC,0x4493_379C,0x08B4_2B8C]
        ),[0x7496_538F,0xE9C7_914F,0xCF10_EA3F,0x82F5_DE2F]);
    }

    #[test]
    fn reported_key_hex_syntax() {
        assert_eq!(parse_hex_u32("7496538f").unwrap(), REPORTED_FIRST_WORD);
        assert_eq!(parse_hex_u32("0x7496538F").unwrap(), REPORTED_FIRST_WORD);
        assert!(parse_hex_u32("0x100000000").is_err());
        assert!(parse_hex_u32("not_a_key").is_err());
    }
}
