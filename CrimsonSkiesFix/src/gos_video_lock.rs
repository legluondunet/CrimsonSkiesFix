//! Default-enabled, bounded native GOS dropdown lock without Python.
//!
//! The original GOS Video.script is **never bundled**. On the first
//! DirectDraw export (after the exact EXE SHA-256 gate, never DllMain),
//! verify the original ROF archive and derive a 2-member resource
//! overlay from the user's installed originals, unless explicitly disabled.
//! The original
//! AIRFRAME patch remains byte-for-byte intact in its compressed form.
//!
//! Unlike a UI object/mouse hook, this uses GameOS's own mail(10000, ...)
//! in VIDEO.SCRIPT. Native control class @ctl@PM clears its hit region,
//! disables arrows and greys the displayed text. The owner validated
//! the identical text transformation under Wine/GE-Proton using a
//! reversible Python prototype.
//!
//! The early-loading path, idempotent repeat and explicit restore have
//! been owner-validated under Wine/GE-Proton. Windows runtime testing
//! remains deferred. As requested by the owner, starting with v0.1.90
//! the feature is ON by default, with a verified, exclusive original
//! backup and explicit restore/opt-out controls. The patch persists
//! across DLL removal until restore is selected.

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
const MAX_TOTAL_DIRECTORIES: usize = 1500;
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
// payload as a directory; reject cycles AND shared directory nodes.
// Both a depth limit and a global directory budget prevent repeated
// traversal of aliased directory tables in malformed archives.
// The root is at offset 0, with 24-byte entry records and a local
// NUL-terminated name pool.
fn walk(data: &[u8], node: usize, prefix: &str, depth: usize,
        ancestors: &mut Vec<usize>, seen: &mut HashSet<usize>,
        out: &mut Vec<Entry>) -> Result<(), String> {
    if depth > MAX_DEPTH || ancestors.contains(&node) {
        return Err("ROF cycle or excessive directory nesting".into());
    }
    if seen.len() >= MAX_TOTAL_DIRECTORIES || !seen.insert(node) {
        return Err("ROF repeated directory node or directory budget exceeded".into());
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
            // Retail UI art filenames can contain spaces or punctuation.
            // Accept printable ASCII without path separators; never extract
            // these paths to disk (the reader uses names only as ROF keys).
            if name == "." || name == ".." ||
               !name.bytes().all(|c| (0x20..=0x7e).contains(&c) &&
                               !matches!(c, b'/' | b'\\' | b':')) {
                return Err("unsafe ROF member name".into());
            }
            let full = format!("{prefix}{name}").to_ascii_uppercase();
            match kind {
                1 => {
                    if expanded != 0 || compressed != 0 {
                        return Err("ROF directory with payload".into());
                    }
                    walk(data, offset, &(full + "/"), depth + 1, ancestors, seen, out)?;
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
    walk(data, 0, "", 0, &mut Vec::new(), &mut HashSet::new(), &mut entries)?;
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
    let decoder = ZlibDecoder::new(payload);
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
            &out[a.offset..a.offset + a.compressed] != airframe.compressed_bytes.as_slice()) {
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
fn overlay_video_matches(bytes: &[u8]) -> Result<bool, String> {
    let all = parse(bytes)?;
    if all.len() != 2 { return Ok(false); }
    let Some(video) = named(&all, VIDEO_PATH) else { return Ok(false); };
    let Some(airframe) = named(&all, AIRFRAME_PATH) else { return Ok(false); };
    let script = inflate(bytes, video)?;
    if script.len() != VIDEO_MODIFIED_SIZE ||
       exe_gate::sha256(&script) != MODIFIED_VIDEO_HASH { return Ok(false); }
    // Validate AIRFRAME as well before ever claiming an existing patch.
    inflate(bytes, airframe)?;
    Ok(true)
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
    // Exclusive create: preexisting user files are never overwritten.
    // If a write/sync fails after creation, close and remove only THIS
    // newly-created partial file. The original ROF is never affected.
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("refusing overwrite of {}: {e}",path.display()))?;
    let written = file.write_all(bytes)
        .and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(path);
        return Err(format!("failed to write new {}: {error}",path.display()));
    }
    Ok(())
}

fn enable(base: &Path, overlay: &Path) -> Result<&'static str, String> {
    let original = checked_patch(overlay)?;
    let backup = overlay.with_file_name(format!("crimptch.rof{BACKUP_SUFFIX}"));
    if overlay_video_matches(&original)? {
        // Existing user patches must never be rewritten. Do not claim
        // successful managed installation if there is no valid rollback.
        // A missing backup leaves the user-patched overlay untouched.
        if !regular_file(&backup) {
            return Err("already-disabled GOS overlay has no regular original backup; no write; restore unavailable".into());
        }
        let preserved = checked_patch(&backup)?;
        let original_air = original_airframe(&preserved)?;
        let current = parse(&original)?;
        let current_air = named(&current, AIRFRAME_PATH).ok_or("patched AIRFRAME missing")?;
        let saved = parse(&preserved)?;
        let saved_air = inflate(&preserved, &saved[0])?;
        if inflate(&original, current_air)? != saved_air ||
           original_air.expanded != saved_air.len() {
            return Err("patched AIRFRAME differs from backup; aborting".into());
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
        return Err(format!("backup preserved but temporary overlay failed: {error}; use GosVideoLock=restore to clear the unused backup safely"));
    }
    // Check no other installer has changed the patch in the meantime.
    if checked_patch(overlay)? != original {
        let _ = fs::remove_file(&temporary);
        return Err("patch overlay changed during preparation; backup retained".into());
    }
    let committed = replace_file(&temporary, overlay);
    if committed.is_err() {let _ = fs::remove_file(&temporary);}
    committed?;
    // Verify the complete installed archive, not just its script hash.
    // The exact original AIRFRAME compressed bytes must survive unchanged.
    if checked_patch(overlay)? != output {
        return Err("installed overlay byte-for-byte postflight failed (backup preserved)".into());
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
    if !overlay_video_matches(&present)? {
        return Err("current patch is not the recognized GOS override; refusing restore".into());
    }
    let entries = parse(&present)?;
    let existing = named(&entries, AIRFRAME_PATH).ok_or("current AIRFRAME missing")?;
    // Verify original expanded AIRFRAME bytes, not just a matching size.
    let original_entry = parse(&original)?;
    let current_air = inflate(&present, existing)?;
    if current_air.len() != airframe.expanded ||
       current_air != inflate(&original, &original_entry[0])? {
        return Err("AIRFRAME member changed; refusing restore".into());
    }
    replace_file(&backup, overlay)?;
    if checked_patch(overlay)? != original {
        return Err("restored archive differs from exact original backup after rename".into());
    }
    Ok("restored original GOS patch overlay; backup consumed")
}

// Native Windows file search order is not established by a successful ROF
// write. Diagnose alternate relative asset roots without changing them or
// recording private filesystem paths. A match does NOT prove the game loaded
// the patched VIDEO.SCRIPT; this is read-only route evidence only.
fn overlay_identity(candidate: &Path, installed: &Path) -> &'static str {
    if !regular_file(candidate) { return "missing-or-nonregular"; }
    let Ok(probe) = checked_patch(candidate) else { return "unreadable"; };
    let Ok(reference) = checked_patch(installed) else { return "reference-unreadable"; };
    if probe == reference { "same-content" } else { "different-content" }
}

fn report_archive_route(installed: &Path) {
    if !crate::cfg().log_enabled { return; }
    let Some(exe_root) = installed.parent().and_then(Path::parent).and_then(Path::parent)
        else { return; };
    let Ok(cwd) = std::env::current_dir() else {
        log("GOS-VIDEO-ROUTE-DIAG: current directory unknown; actual resource loading unobserved");
        return;
    };
    let same_root = match (cwd.canonicalize(), exe_root.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    };
    let cwd_assets = cwd.join("ASSETS").join("crimptch.rof");
    let cwd_gosdata_assets = cwd.join("GOSDATA").join("ASSETS").join("crimptch.rof");
    log(&format!(
        "GOS-VIDEO-ROUTE-DIAG: cwd_matches_exe_root={same_root} cwd_ASSETS_overlay={} cwd_GOSDATA_ASSETS_overlay={} [read-only; NOT proof which VIDEO.SCRIPT GameOS executed]",
        overlay_identity(&cwd_assets, installed),
        overlay_identity(&cwd_gosdata_assets, installed)
    ));
}

pub fn apply(mode: Mode, widescreen_ready: bool) {
    if mode == Mode::Off {return;}
    if mode == Mode::On && !widescreen_ready {
        log("GOS-VIDEO-LOCK: enabled but native widescreen adapter prerequisites inactive; skipped");
        return;
    }
    let outcome = (|| {
        let (base,overlay) = paths()?;
        report_archive_route(&overlay);
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
        assert!(!overlay_video_matches(&new).unwrap());
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
    fn reject_repeated_directory_nodes_and_global_budget_exhaustion() {
        let good = fixture_archive(b"airframe", b"video");
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        walk(&good, 0, "", 0, &mut Vec::new(), &mut seen, &mut entries).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(walk(&good, 0, "", 0, &mut Vec::new(), &mut seen, &mut Vec::new())
            .unwrap_err().contains("repeated directory"));
        let mut full: HashSet<usize> = (0..MAX_TOTAL_DIRECTORIES).collect();
        assert!(walk(&good, 0, "", 0, &mut Vec::new(), &mut full, &mut Vec::new())
            .unwrap_err().contains("budget exceeded"));
    }
    #[test]
    fn exclusively_created_files_are_not_overwritten() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_nanos();
        let file = std::env::temp_dir().join(format!(
            "csfix-rof-audit-{}-{unique}.tmp", std::process::id()
        ));
        assert!(!file.exists());
        write_new(&file, b"owner backup").unwrap();
        assert!(write_new(&file, b"unexpected overwrite").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"owner backup");
        fs::remove_file(file).unwrap();
    }
    #[test]
    fn mode_is_explicit_and_fail_closed() {
        assert_eq!(Mode::Off,Mode::Off);
        assert_ne!(Mode::On,Mode::Restore);
    }
}
