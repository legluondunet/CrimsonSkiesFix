//! Opt-in, bounded native GOS dropdown lock without Python.
//!
//! The original GOS Video.script is **never bundled**. On the first
//! DirectDraw export (after the exact EXE SHA-256 gate, never DllMain),
//! optionally verify the original ROF archive and derive a 2-member
//! resource overlay from the user's installed originals. The original
//! AIRFRAME patch remains byte-for-byte intact in its compressed form.
//!
//! Unlike a UI object/mouse hook, this uses GameOS's own mail(10000, ...)
//! in VIDEO.SCRIPT. Native control class @ctl@PM clears its hit region,
//! disables arrows and greys the displayed text. The owner validated
//! the identical text transformation under Wine/GE-Proton using a
//! reversible Python prototype.
//!
//! This is experimental and OFF by default until new-DLL startup-order
//! validation. "Restore" is an independent, explicit INI action.

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use crate::{exe_gate, log};

const BASE_SIZE: usize = 60_236_221;
const BASE_HASH: &str = "acc9946874110e9741183384010bc02fd48923ae3d608fcf02a96498c3731174";
const ORIGINAL_VIDEO_HASH: &str = "f0b0ae62dc582a55dfc6eaf4a8b2937ef2084a03596b8fa52542a5de4512afa9";
const MODIFIED_VIDEO_HASH: &str = "a5f716d2e1ad007ac217a172e9d17b459444f7ffb23855a5a10d9a1841b9b22d";
const VIDEO_PATH: &str = "ASSETS/SCRIPTS/VIDEO.SCRIPT";
const AIRFRAME_PATH: &str = "ASSETS/SCRIPTS/AIRFRAME.SCRIPT";
const VIDEO_SIZE: usize = 4337;
const VIDEO_MODIFIED_SIZE: usize = 4413;
const MAX_PATCH_BYTES: usize = 4 * 1024 * 1024;
const MAX_SCRIPT_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 1500;
const MAX_DEPTH: usize = 20;
const BACKUP_SUFFIX: &str = ".CrimsonSkiesFix.original.bak";
const TEMP_SUFFIX: &str = ".CrimsonSkiesFix.pending";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Off, On, Restore }

#[derive(Clone, Debug)]
struct Entry {
    name: String,
    offset: usize,
    compressed: usize,
    expanded: usize,
    kind: u32,
}

fn u32_at(data: &[u8], off: usize) -> Result<usize, String> {
    let b = data.get(off..off.checked_add(4).ok_or("offset overflow")?)
        .ok_or("ROF field outside file")?;
    Ok(u32::from_le_bytes(b.try_into().unwrap()) as usize)
}

fn put_u32(data: &mut [u8], offset: usize, value: usize) -> Result<(), String> {
    let x: u32 = value.try_into().map_err(|_| "ROF size overflows u32")?;
    let target = data.get_mut(offset..offset.checked_add(4).ok_or("ROF offset overflow")?)
        .ok_or("ROF patch location outside file")?;
    target.copy_from_slice(&x.to_le_bytes());
    Ok(())
}

// Walk only the well-formed ROF directory tree. Never interpret a
// payload as a directory and never follow an ancestor node twice.
// The root is at offset 0, with 24-byte entry records and a local
// NUL-terminated name pool.
fn walk(data: &[u8], node: usize, prefix: &str, depth: usize,
        ancestors: &mut Vec<usize>, out: &mut Vec<Entry>) -> Result<(), String> {
    if depth > MAX_DEPTH || ancestors.contains(&node) {
        return Err("ROF cycle or excessive directory nesting".into());
    }
    ancestors.push(node);
    let result = (|| {
        let count = u32_at(data, node)?;
        let pool_size = u32_at(data, node.checked_add(4).ok_or("node overflow")?)?;
        if count > MAX_NODES || pool_size > 2 * 1024 * 1024 {
            return Err("implausible ROF directory counts".into());
        }
        let records = node.checked_add(8).ok_or("node overflow")?;
        let pool = records.checked_add(count.checked_mul(24).ok_or("record overflow")?)
            .ok_or("record overflow")?;
        let end = pool.checked_add(pool_size).ok_or("pool overflow")?;
        if end > data.len() { return Err("truncated ROF node".into()); }
        for index in 0..count {
            if out.len() > 20000 { return Err("ROF member count exceeded".into()); }
            let e = records + index * 24;
            let offset = u32_at(data, e)?;
            let expanded = u32_at(data, e + 4)?;
            let compressed = u32_at(data, e + 8)?;
            let kind = u32_at(data, e + 12)? as u32;
            let len = u32_at(data, e + 16)?;
            let noff = u32_at(data, e + 20)?;
            if len < 2 || len > 256 || noff.checked_add(len).is_none_or(|n| n > pool_size) {
                return Err("ROF member name outside pool".into());
            }
            let raw = &data[pool + noff..pool + noff + len];
            if raw[len - 1] != 0 || raw[..len - 1].contains(&0) {
                return Err("invalid ROF name terminator".into());
            }
            let name = std::str::from_utf8(&raw[..len - 1])
                .map_err(|_| "non-UTF8 ROF name")?;
            if !name.bytes().all(|c| c.is_ascii_alphanumeric() ||
                    matches!(c, b'_' | b'-' | b'.' | b'$')) ||
                name == "." || name == ".." {
                return Err("unsafe ROF member name".into());
            }
            let full = format!("{prefix}{name}").to_ascii_uppercase();
            match kind {
                1 => {
                    if expanded != 0 || compressed != 0 {
                        return Err("ROF directory with payload".into());
                    }
                    walk(data, offset, &(full + "/"), depth + 1, ancestors, out)?;
                }
                0 | 2 => {
                    if offset > data.len() ||
                        compressed > data.len() - offset ||
                        (kind == 0 && compressed != expanded) {
                        return Err("ROF payload outside archive".into());
                    }
                    out.push(Entry {
                        name: full, offset, compressed, expanded, kind
                    });
                }
                _ => return Err("unknown ROF member kind".into())
            }
        }
        Ok(())
    })();
    ancestors.pop();
    result
}

fn parse(data: &[u8]) -> Result<Vec<Entry>, String> {
    if data.len() < 8 { return Err("truncated ROF archive".into()); }
    let mut entries = Vec::new();
    walk(data, 0, "", 0, &mut Vec::new(), &mut entries)?;
    let mut names = HashSet::new();
    for item in &entries {
        if !names.insert(item.name.clone()) {
            return Err("duplicate ROF member name".into());
        }
    }
    Ok(entries)
}
fn named<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries.iter().find(|entry| entry.name == name)
}
fn inflate(data: &[u8], entry: &Entry) -> Result<Vec<u8>, String> {
    if entry.expanded > MAX_SCRIPT_BYTES || entry.compressed > MAX_SCRIPT_BYTES {
        return Err("ROF script member exceeds size bound".into());
    }
    let payload = data.get(entry.offset..entry.offset + entry.compressed)
        .ok_or("ROF member out of range")?;
    if entry.kind == 0 { return Ok(payload.to_vec()); }
    let mut decoder = ZlibDecoder::new(payload);
    let mut result = Vec::new();
    decoder.take(entry.expanded as u64 + 1).read_to_end(&mut result)
        .map_err(|e| format!("ROF script zlib error: {e}"))?;
    if result.len() != entry.expanded {
        return Err("ROF decompressed size mismatch".into());
    }
    Ok(result)
}

fn insert_once_or_more(data: &[u8], needle: &[u8], insertion: &[u8],
                       expected: usize) -> Result<Vec<u8>, String> {
    let mut result = Vec::with_capacity(data.len() + expected * insertion.len());
    let mut start = 0usize;
    let mut count = 0;
    while start < data.len() {
        if let Some(rel) = data[start..].windows(needle.len()).position(|b| b == needle) {
            let end = start + rel + needle.len();
            result.extend_from_slice(&data[start..end]);
            result.extend_from_slice(insertion);
            start = end;
            count += 1;
        } else {
            result.extend_from_slice(&data[start..]);
            break;
        }
    }
    if count != expected { return Err("GOS Video init preimage count mismatch".into()); }
    Ok(result)
}

fn patched_video(original: &[u8]) -> Result<Vec<u8>, String> {
    if original.len() != VIDEO_SIZE ||
        exe_gate::sha256(original) != ORIGINAL_VIDEO_HASH {
        return Err("unrecognized original VIDEO.SCRIPT; refusing GOS patch".into());
    }
    let h = insert_once_or_more(original, b"initialize(HVA)\r\n",
                                b"mail (10000, HVA)\r\n", 1)?;
    let i = insert_once_or_more(&h, b"initialize(IVA)\r\n",
                                b"mail (10000, IVA)\r\n", 3)?;
    if i.len() != VIDEO_MODIFIED_SIZE ||
        exe_gate::sha256(&i) != MODIFIED_VIDEO_HASH {
        return Err("GOS Video patched script postimage mismatch".into());
    }
    Ok(i)
}

#[derive(Clone)]
struct Payload {
    name: &'static str,
    expanded: usize,
    compressed_bytes: Vec<u8>,
    kind: u32,
}

fn append_dir(out: &mut Vec<u8>, names: &[&str]) -> Result<(usize, Vec<usize>), String> {
    let base = out.len();
    let pool_size: usize = names.iter().map(|n| n.len() + 1).sum();
    out.resize(base + 8 + 24 * names.len() + pool_size, 0);
    put_u32(out, base, names.len())?;
    put_u32(out, base + 4, pool_size)?;
    let mut off = 0;
    let mut entries = Vec::new();
    for (idx, name) in names.iter().enumerate() {
        let e = base + 8 + 24 * idx;
        put_u32(out, e + 16, name.len() + 1)?;
        put_u32(out, e + 20, off)?;
        let pool = base + 8 + 24 * names.len() + off;
        out[pool..pool + name.len()].copy_from_slice(name.as_bytes());
        entries.push(e);
        off += name.len() + 1;
    }
    Ok((base, entries))
}
fn patch_entry(out: &mut [u8], entry: usize, target: usize,
               expanded: usize, stored: usize, kind: usize) -> Result<(), String> {
    put_u32(out, entry, target)?;
    put_u32(out, entry + 4, expanded)?;
    put_u32(out, entry + 8, stored)?;
    put_u32(out, entry + 12, kind)
}
fn write_overlay(airframe: &Payload, video: &[u8]) -> Result<Vec<u8>, String> {
    if airframe.name != "AIRFRAME.SCRIPT" || airframe.expanded > MAX_SCRIPT_BYTES ||
        airframe.compressed_bytes.len() > MAX_SCRIPT_BYTES ||
        !matches!(airframe.kind, 0 | 2) {
        return Err("invalid original AIRFRAME overlay member".into());
    }
    let mut compressor = ZlibEncoder::new(Vec::new(), Compression::fast());
    compressor.write_all(video).map_err(|e| e.to_string())?;
    let compressed_video = compressor.finish().map_err(|e| e.to_string())?;

    // Fixed, verified layout: root ASSETS -> SCRIPTS -> only
    // the original AIRFRAME override and patched VIDEO script.
    let mut out = Vec::with_capacity(16384);
    let (_root, root) = append_dir(&mut out, &["ASSETS"])?;
    let (assets_at, assets) = append_dir(&mut out, &["SCRIPTS"])?;
    let (scripts_at, script_slots) =
        append_dir(&mut out, &["AIRFRAME.SCRIPT", "VIDEO.SCRIPT"])?;
    patch_entry(&mut out, root[0], assets_at, 0, 0, 1)?;
    patch_entry(&mut out, assets[0], scripts_at, 0, 0, 1)?;

    let airframe_at = out.len();
    out.extend_from_slice(&airframe.compressed_bytes);
    let video_at = out.len();
    out.extend_from_slice(&compressed_video);
    patch_entry(&mut out, script_slots[0], airframe_at, airframe.expanded,
                airframe.compressed_bytes.len(), airframe.kind as usize)?;
    patch_entry(&mut out, script_slots[1], video_at, video.len(),
                compressed_video.len(), 2)?;
    if out.len() > MAX_PATCH_BYTES {return Err("new patch archive too large".into());}
    let scanned = parse(&out)?;
    if scanned.len() != 2 ||
        inflate(&out, named(&scanned, VIDEO_PATH).ok_or("new VIDEO missing")?)? != video ||
        named(&scanned, AIRFRAME_PATH).is_none_or(|a|
            out[a.offset..a.offset + a.compressed] != airframe.compressed_bytes) {
        return Err("ROF generated overlay roundtrip failed".into());
    }
    Ok(out)
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m|
        m.is_file() && !m.file_type().is_symlink())
}
fn regular_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m|
        m.is_dir() && !m.file_type().is_symlink())
}
fn paths() -> Result<(PathBuf, PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = exe.parent().ok_or("missing executable parent")?;
    let gos = root.join("GOSDATA");
    let assets = gos.join("ASSETS");
    if !regular_dir(root) || !regular_dir(&gos) || !regular_dir(&assets) {
        return Err("game GOSDATA/ASSETS is missing or a linked directory".into());
    }
    let base = assets.join("crimson.rof");
    let overlay = assets.join("crimptch.rof");
    if !regular_file(&base) || !regular_file(&overlay) {
        return Err("game original ROF files missing or nonregular".into());
    }
    Ok((base, overlay))
}
fn checked_patch(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() ||
       meta.len() > MAX_PATCH_BYTES as u64 {
        return Err("invalid or oversized patch overlay".into());
    }
    fs::read(path).map_err(|e| e.to_string())
}
fn read_original_video(base: &Path) -> Result<Vec<u8>, String> {
    if fs::metadata(base).map_err(|e| e.to_string())?.len() as usize != BASE_SIZE {
        return Err("unsupported original crimson.rof size".into());
    }
    let bytes = fs::read(base).map_err(|e| e.to_string())?;
    if exe_gate::sha256(&bytes) != BASE_HASH {
        return Err("unsupported original crimson.rof SHA-256".into());
    }
    let entries = parse(&bytes)?;
    let video = named(&entries, VIDEO_PATH).ok_or("original GOS VIDEO.SCRIPT missing")?;
    inflate(&bytes, video)
}
fn original_airframe(overlay: &[u8]) -> Result<Payload, String> {
    let members = parse(overlay)?;
    if members.len() != 1 || members[0].name != AIRFRAME_PATH {
        return Err("unrecognized patch overlay; refusing to modify it".into());
    }
    let a = &members[0];
    if a.expanded == 0 || a.expanded > MAX_SCRIPT_BYTES {return Err("AIRFRAME payload invalid".into());}
    // Validate the compressed stream without changing its original bytes.
    let decoded = inflate(overlay, a)?;
    if decoded.is_empty() {return Err("empty AIRFRAME patch".into());}
    Ok(Payload {name:"AIRFRAME.SCRIPT",expanded:a.expanded,
        compressed_bytes:overlay[a.offset..a.offset+a.compressed].to_vec(),kind:a.kind})
}
fn overlay_video_matches(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let all = parse(bytes)?;
    if all.len()!=2 {return Ok(None);}
    let Some(video) = named(&all, VIDEO_PATH) else {return Ok(None)};
    let Some(_airframe) = named(&all, AIRFRAME_PATH) else {return Ok(None)};
    let script = inflate(bytes, video)?;
    if script.len() != VIDEO_MODIFIED_SIZE ||
       exe_gate::sha256(&script) != MODIFIED_VIDEO_HASH {return Ok(None);}
    let a = named(&all, AIRFRAME_PATH).unwrap();
    inflate(bytes, a)?; // verify both entries are readable
    Ok(Some(script))
}

fn wide(path: &OsStr) -> Vec<u16> {
    path.encode_wide().chain(std::iter::once(0)).collect()
}
#[link(name="kernel32")]
extern "system" {
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}
fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    let a = wide(from.as_os_str());
    let b = wide(to.as_os_str());
    let ok = unsafe{MoveFileExW(a.as_ptr(),b.as_ptr(),0x1|0x8)};
    if ok == 0 {return Err(format!("atomic ROF rename failed: {}", std::io::Error::last_os_error()));}
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("refusing overwrite of {}: {e}",path.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

fn enable(base: &Path, overlay: &Path) -> Result<&'static str, String> {
    let original = checked_patch(overlay)?;
    let backup = overlay.with_file_name(format!("crimptch.rof{BACKUP_SUFFIX}"));
    if let Some(_) = overlay_video_matches(&original)? {
        if backup.exists() {
            let preserved = checked_patch(&backup)?;
            let original_air = original_airframe(&preserved)?;
            let current = parse(&original)?;
            let current_air = named(&current, AIRFRAME_PATH).ok_or("patched AIRFRAME missing")?;
            let patched_air = inflate(&original, current_air)?;
            let saved_air = {
                let saved = parse(&preserved)?;
                inflate(&preserved, &saved[0])?
            };
            if patched_air != saved_air || original_air.expanded != saved_air.len() {
                return Err("patched AIRFRAME differs from backup; aborting".into());
            }
        }
        return Ok("already-disabled (original GOS override; no write)");
    }
    if fs::symlink_metadata(&backup).is_ok() {
        return Err("backup already exists but installed patch is not recognized; manual review required".into());
    }
    let airframe = original_airframe(&original)?;
    let video = read_original_video(base)?;
    let patched = patched_video(&video)?;
    let output = write_overlay(&airframe, &patched)?;
    let temporary = overlay.with_file_name(format!("crimptch.rof{TEMP_SUFFIX}"));
    if fs::symlink_metadata(&temporary).is_ok() {
        return Err("pending ROF file already exists; refusing overwrite".into());
    }

    // Save the original complete compressed overlay (not a regenerated
    // AIRFRAME script). This backup survives every update.
    write_new(&backup, &original)?;
    if let Err(error) = write_new(&temporary, &output) {
        return Err(format!("backup preserved but temporary overlay failed: {error}"));
    }
    // Check no other installer has changed the patch in the meantime.
    if checked_patch(overlay)? != original {
        let _ = fs::remove_file(&temporary);
        return Err("patch overlay changed during preparation; backup retained".into());
    }
    let committed = replace_file(&temporary, overlay);
    if committed.is_err() {let _ = fs::remove_file(&temporary);}
    committed?;
    if overlay_video_matches(&checked_patch(overlay)?)?.is_none() {
        return Err("installed overlay postflight verification failed (backup preserved)".into());
    }
    Ok("installed native-disabled GOS Video controls (original archived in .bak)")
}

fn restore(overlay: &Path) -> Result<&'static str, String> {
    let backup = overlay.with_file_name(format!("crimptch.rof{BACKUP_SUFFIX}"));
    if !regular_file(&backup) {return Err("no safe original GOS patch backup to restore".into());}
    let original = checked_patch(&backup)?;
    let airframe = original_airframe(&original)?;
    let present = checked_patch(overlay)?;
    if present == original {
        fs::remove_file(&backup).map_err(|e| e.to_string())?;
        return Ok("original patch already intact (stale backup removed)");
    }
    if overlay_video_matches(&present)?.is_none() {
        return Err("current patch is not the recognized GOS override; refusing restore".into());
    }
    let entries = parse(&present)?;
    let existing = named(&entries, AIRFRAME_PATH).ok_or("current AIRFRAME missing")?;
    if inflate(&present, existing)?.len() != airframe.expanded {
        return Err("AIRFRAME size mismatch; refusing restore".into());
    }
    // Check original bytes, not only length, to protect any owner changes.
    let original_entry = parse(&original)?;
    if inflate(&present, existing)? != inflate(&original, &original_entry[0])? {
        return Err("AIRFRAME member changed; refusing restore".into());
    }
    replace_file(&backup, overlay)?;
    Ok("restored original GOS patch overlay; backup consumed")
}

pub fn apply(mode: Mode, widescreen_ready: bool) {
    if mode == Mode::Off {return;}
    if mode == Mode::On && !widescreen_ready {
        log("GOS-VIDEO-LOCK: enabled but native widescreen adapter prerequisites inactive; skipped");
        return;
    }
    let outcome = (|| {
        let (base,overlay) = paths()?;
        match mode {
            Mode::On => enable(&base,&overlay),
            Mode::Restore => restore(&overlay),
            Mode::Off => Ok("off"),
        }
    })();
    match outcome {
        Ok(message) => log(&format!("GOS-VIDEO-LOCK: {message}")),
        Err(error) => log(&format!("GOS-VIDEO-LOCK: skipped safely: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_archive(air: &[u8], video: &[u8]) -> Vec<u8> {
        write_overlay(&Payload { name:"AIRFRAME.SCRIPT", expanded:air.len(),
            compressed_bytes:air.to_vec(), kind:0 },video).unwrap()
    }
    #[test]
    fn rof_roundtrip_preserves_uncompressed_airframe_and_zlib_video() {
        let old = b"synthetic airframe \0 data";
        let video = b"synthetic VIDEO.SCRIPT \r\n";
        let new = fixture_archive(old,video);
        let items = parse(&new).unwrap();
        assert_eq!(items.len(),2);
        assert_eq!(inflate(&new,named(&items,AIRFRAME_PATH).unwrap()).unwrap(),old);
        assert_eq!(inflate(&new,named(&items,VIDEO_PATH).unwrap()).unwrap(),video);
        assert!(overlay_video_matches(&new).unwrap().is_none());
        let mut corrupt = new.clone();
        corrupt[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse(&corrupt).is_err());
    }
    #[test]
    fn only_two_widget_initializers_are_modified() {
        let synthetic = b"initialize(HVA)\r\ninitialize(IVA)\r\ninitialize(IVA)\r\ninitialize(IVA)\r\n";
        let one = insert_once_or_more(synthetic,b"initialize(HVA)\r\n",
                        b"mail (10000, HVA)\r\n",1).unwrap();
        let four = insert_once_or_more(&one,b"initialize(IVA)\r\n",
                        b"mail (10000, IVA)\r\n",3).unwrap();
        assert_eq!(four.windows(b"mail (10000, HVA)".len())
                   .filter(|w|*w==b"mail (10000, HVA)").count(),1);
        assert_eq!(four.windows(b"mail (10000, IVA)".len())
                   .filter(|w|*w==b"mail (10000, IVA)").count(),3);
        assert!(insert_once_or_more(synthetic,b"initialize(HVA)\r\n",b"x",2).is_err());
        assert!(patched_video(synthetic).is_err()); // exact original hash only
    }
    #[test]
    fn reject_traversal_or_corrupted_nodes_and_unknown_overlays() {
        let good = fixture_archive(b"airframe",b"video");
        let mut invalid = good.clone();
        invalid[8..12].copy_from_slice(&0u32.to_le_bytes()); // ASSETS points to root
        assert!(parse(&invalid).is_err());
        assert!(original_airframe(&good).is_err()); // two-member patch
        let mut truncated = good.clone();
        truncated.truncate(11);
        assert!(parse(&truncated).is_err());
    }
    #[test]
    fn mode_is_explicit_and_fail_closed() {
        assert_eq!(Mode::Off,Mode::Off);
        assert_ne!(Mode::On,Mode::Restore);
    }
}
