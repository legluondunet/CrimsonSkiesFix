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
    raw_offset: usize,
    raw_len: usize,
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
        let raw_len = read_u32(data, at + 16)? as usize;
        let raw_offset = read_u32(data, at + 20)? as usize;
        let end = raw_offset.checked_add(raw_len).ok_or("PE section length overflow")?;
        if end > data.len() { return Err("PE section extends past EOF".into()); }
        result.push(PeSection { name, raw_offset, raw_len });
    }
    Ok(Pe32 { entry_rva: entry, sections: result })
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
}

fn help() {
    println!("unSafeDisc Linux research port (native Rust, initial read-only stage)\n\
Usage:\n\
  unsafedisc-linux inspect --loader CRIMSON.EXE --icd CRIMSON.ICD \\\n\
      --dplayerx dplayerx.dll [--reference TESTME.EXE] [--reported-key 0x7496538F]\n\
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
    Ok(Some(options))
}

fn inspect(options: Options) -> Result<()> {
    let loader = read_original(options.loader.as_ref().unwrap(), LOADER_LEN, LOADER_SHA)?;
    let icd = read_original(options.icd.as_ref().unwrap(), ICD_LEN, ICD_SHA)?;
    let dll = read_original(options.dplayerx.as_ref().unwrap(), DLL_LEN, DLL_SHA)?;
    let source_pe = parse_pe32(&loader)?;
    let protected_pe = parse_pe32(&icd)?;
    let (raw_offset, raw) = locate_raw_key(&loader, dll.len())?;
    println!("Verified original Crimson Skies 1.0.2.0 files (SHA-256 + size).");
    println!("SafeDisc: 1.50.20; native read-only inspection; no Wine.");
    println!("Loader entry RVA: 0x{:08X}", source_pe.entry_rva);
    println!("ICD entry RVA:    0x{:08X}", protected_pe.entry_rva);
    println!("Raw key offset:  0x{raw_offset:X} (CRPE + 6 + 0x148)");
    println!("Selected raw DWORDs:");
    for (i, w) in raw.iter().enumerate() { println!("  raw[{i}] = 0x{w:08X}"); }
    println!("No full 128-bit TEA key is asserted; checksum and DLL modifier stage remain unverified.");
    if let Some(path) = options.reference {
        let reference = read_original(&path, TESTME_LEN, TESTME_SHA)?;
        let decoded_pe = parse_pe32(&reference)?;
        if decoded_pe.entry_rva != protected_pe.entry_rva {
            return Err("original ICD and unwrapped reference entrypoints differ".into());
        }
        println!("Verified user-supplied TESTME.EXE reference (exact known hash).");
        if let Some(key) = options.reported_key {
            let matches = import_xor_matches(&icd, &protected_pe, &reference, &decoded_pe, key)?;
            println!("Import DWORD XOR matches for 0x{key:08X}: {}", matches.len());
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
    println!("Unpacking is NOT implemented. Original input files have not been modified.");
    Ok(())
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
    fn reported_key_hex_syntax() {
        assert_eq!(parse_hex_u32("7496538f").unwrap(), REPORTED_FIRST_WORD);
        assert_eq!(parse_hex_u32("0x7496538F").unwrap(), REPORTED_FIRST_WORD);
        assert!(parse_hex_u32("0x100000000").is_err());
        assert!(parse_hex_u32("not_a_key").is_err());
    }
}
