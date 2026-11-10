//! Standalone bootstrap launcher for the owner's already-installed Crimson Skies 1.02.
//! No copyrighted game assets, executable or disc patches are distributed.
mod sha256;

use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const GAME_EXE_HASH: &str =
    "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75";
const GAME_EXE_SIZE: u64 = 2_580_480;
const BUNDLED_DDRAW: &[u8] = include_bytes!(env!("CRIMSONSKIESFIX_BUNDLED_DLL"));
const README: &str = include_str!("../../README.md");
const EXAMPLE_INI: &str = include_str!("../../CrimsonSkiesFix/CrimsonSkiesFix.ini.example");
const LICENSE: &str = include_str!("../../../LICENSE");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("CRIMSONSKIESFIX_COMMIT");

fn error(message: impl std::fmt::Display) -> String {
    message.to_string()
}

fn regular(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_dir() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn any_entry(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn digest(path: &Path) -> Result<String, String> {
    if !regular(path) {
        return Err(format!("not a regular file: {}", path.display()));
    }
    let mut file = File::open(path).map_err(error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(error)?;
    Ok(sha256::hex(&bytes))
}

fn validated_game_exe(game: &Path) -> Result<PathBuf, String> {
    if !directory(game) {
        return Err(format!("missing or unsafe game directory: {}", game.display()));
    }
    let choices = ["CRIMSON.EXE", "CRIMSON-UNWRAPPED.EXE"]
        .iter()
        .map(|name| game.join(name))
        .filter(|path| any_entry(path))
        .collect::<Vec<_>>();
    if choices.len() != 1 {
        return Err("expected exactly one CRIMSON.EXE or CRIMSON-UNWRAPPED.EXE".into());
    }
    let exe = &choices[0];
    if !regular(exe) {
        return Err("game executable must be a regular file (not a link)".into());
    }
    let size = fs::metadata(exe).map_err(error)?.len();
    if size != GAME_EXE_SIZE {
        return Err(format!(
            "unsupported game executable: size {size}; expected {GAME_EXE_SIZE} bytes (1.02 unwrapped)"
        ));
    }
    let hash = digest(exe)?;
    if hash != GAME_EXE_HASH {
        return Err(format!(
            "unsupported game executable SHA-256: {hash}; expected {GAME_EXE_HASH}"
        ));
    }
    for name in ["crimson.rof", "crimptch.rof"] {
        let path = game.join("GOSDATA").join("ASSETS").join(name);
        if !regular(&path) {
            return Err(format!("missing original game asset: {}", path.display()));
        }
    }
    Ok(exe.clone())
}

fn verify(game: &Path) -> Result<PathBuf, String> {
    let exe = validated_game_exe(game)?;
    let proxy = game.join("ddraw.dll");
    if !regular(&proxy) {
        return Err("CrimsonSkiesFix ddraw.dll missing or not a regular file".into());
    }
    let expected = sha256::hex(BUNDLED_DDRAW);
    let actual = digest(&proxy)?;
    if actual != expected {
        return Err(format!(
            "installed ddraw.dll does not match this launcher: actual={actual} expected={expected}"
        ));
    }
    Ok(exe)
}

fn ensure_new_text(path: &Path, text: &str) -> Result<(), String> {
    if any_entry(path) {
        if regular(path) {
            return Ok(()); // User edits (including an old template) are preserved.
        }
        return Err(format!("unsafe existing file: {}", path.display()));
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(error)?;
    file.write_all(text.as_bytes()).map_err(error)?;
    file.sync_all().map_err(error)
}

// Never overwrite the active INI. The proxy creates it on its first game launch.
fn deploy_documentation(game: &Path) -> Result<(), String> {
    // Flat installed-game layout: never create CrimsonSkiesFix/CrimsonSkiesFix/.
    // The proxy creates CrimsonSkiesFix.ini directly in this game directory.
    ensure_new_text(&game.join("README.md"), README)?;
    ensure_new_text(&game.join("LICENSE"), LICENSE)?;
    ensure_new_text(&game.join("CrimsonSkiesFix.ini.example"), EXAMPLE_INI)?;
    let source = format!("https://github.com/legluondunet/CrimsonSkiesFix/tree/{COMMIT}");
    let build_info = format!(
        "Project=CrimsonSkiesFix\nVersion={VERSION}\nCommit={COMMIT}\n\
         LogIdentity=CrimsonSkiesFix {VERSION}-{} x86\nArchitecture=Windows-x86\n\
         DllSha256={}\nDllSizeBytes={}\nSource={source}\n",
        &COMMIT[..8],
        sha256::hex(BUNDLED_DDRAW),
        BUNDLED_DDRAW.len()
    );
    let manifest = game.join("build-info.txt");
    if any_entry(&manifest) && !regular(&manifest) {
        return Err("unsafe build-info.txt (not a regular file)".into());
    }
    // This manifest is launcher-managed provenance, not player configuration.
    fs::write(manifest, build_info).map_err(error)
}

// Stage first. Keep exactly one previous DLL backup; never discard it before
// the new DLL has been written and verified. Refuse abandoned transactions.
fn deploy_proxy(game: &Path) -> Result<(), String> {
    let active = game.join("ddraw.dll");
    let pending = game.join("ddraw.dll.installing");
    let backup = game.join("ddraw.dll.bak");
    let old_backup = game.join("ddraw.dll.bak.previous");
    if any_entry(&pending) || any_entry(&old_backup) {
        return Err("abandoned ddraw.dll transaction; inspect files manually".into());
    }
    if any_entry(&active) && !regular(&active) {
        return Err("unsafe existing ddraw.dll".into());
    }
    if any_entry(&backup) && !regular(&backup) {
        return Err("unsafe existing ddraw.dll.bak".into());
    }
    let expected = sha256::hex(BUNDLED_DDRAW);
    if regular(&active) && digest(&active)? == expected {
        return Ok(());
    }
    {
        let mut f = OpenOptions::new().write(true).create_new(true)
            .open(&pending).map_err(error)?;
        f.write_all(BUNDLED_DDRAW).map_err(error)?;
        f.sync_all().map_err(error)?;
    }
    if digest(&pending)? != expected {
        let _ = fs::remove_file(&pending);
        return Err("staged ddraw.dll SHA-256 mismatch".into());
    }
    let backup_moved = if regular(&backup) {
        if let Err(err) = fs::rename(&backup, &old_backup) {
            let _ = fs::remove_file(&pending);
            return Err(format!("cannot stage previous DLL backup: {err}"));
        }
        true
    } else { false };
    let active_moved = if regular(&active) {
        if let Err(err) = fs::rename(&active, &backup) {
            if backup_moved { let _ = fs::rename(&old_backup, &backup); }
            let _ = fs::remove_file(&pending);
            return Err(format!("cannot back up previous ddraw.dll: {err}"));
        }
        true
    } else { false };
    if let Err(err) = fs::rename(&pending, &active) {
        if active_moved { let _ = fs::rename(&backup, &active); }
        if backup_moved { let _ = fs::rename(&old_backup, &backup); }
        let _ = fs::remove_file(&pending);
        return Err(format!("cannot publish bundled ddraw.dll: {err}"));
    }
    if backup_moved {
        if let Err(err) = fs::remove_file(&old_backup) {
            eprintln!("Warning: old backup retained as {}: {err}", old_backup.display());
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if !directory(from) {
        return Err(format!("source is not a real directory: {}", from.display()));
    }
    fs::create_dir(to).map_err(error)?;
    for entry in fs::read_dir(from).map_err(error)? {
        let entry = entry.map_err(error)?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let kind = entry.file_type().map_err(error)?;
        if kind.is_symlink() {
            return Err(format!("refusing source link: {}", src.display()));
        } else if kind.is_dir() {
            copy_tree(&src, &dst)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst).map_err(error)?;
            // The original CD may mark files read-only; the GOS overlay needs
            // a writable copy. Never change permissions on the source files.
            let mut permissions = fs::metadata(&dst).map_err(error)?.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&dst, permissions).map_err(error)?;
        } else {
            return Err(format!("unsupported source entry: {}", src.display()));
        }
    }
    Ok(())
}

fn install_from(source: &Path, dest: &Path) -> Result<(), String> {
    if any_entry(dest) {
        return Err(format!(
            "refusing to overwrite existing installation: {}",
            dest.display()
        ));
    }
    validated_game_exe(source)?;
    let canonical_source = fs::canonicalize(source).map_err(error)?;
    let canonical_parent = fs::canonicalize(dest.parent().ok_or("no destination parent")?)
        .map_err(error)?;
    if canonical_parent.starts_with(&canonical_source) {
        return Err("launcher destination is inside source game directory".into());
    }
    let staging = dest.with_extension("installing");
    if any_entry(&staging) {
        return Err("abandoned CrimsonSkiesFix.installing directory; inspect it manually".into());
    }
    println!("Importing existing game files into {}...", dest.display());
    // Failed copies leave staging for manual inspection; never expose an
    // incomplete tree under the finished game directory.
    copy_tree(source, &staging)?;
    validated_game_exe(&staging)?;
    deploy_proxy(&staging)?;
    deploy_documentation(&staging)?;
    verify(&staging)?;
    fs::rename(&staging, dest).map_err(error)?;
    println!("Installation verified and published.");
    Ok(())
}

fn source_candidates(base: &Path, game: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    for item in fs::read_dir(base).map_err(error)? {
        let entry = item.map_err(error)?;
        let path = entry.path();
        if path == game || !directory(&path) { continue; }
        if ["CRIMSON.EXE", "CRIMSON-UNWRAPPED.EXE"]
            .iter().any(|name| any_entry(&path.join(name))) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

fn usage() {
    println!(
        "CrimsonSkiesFix {VERSION} ({})\n\
         Automatic installer / verifier / launcher for Crimson Skies 1.02.\n\
         \nUsage:\n\
         CrimsonSkiesFix.exe            verify or import an adjacent installed game, then play\n\
         CrimsonSkiesFix.exe --import DIR   import an existing installed game and play\n\
         CrimsonSkiesFix.exe --verify   verify installed EXE and embedded ddraw.dll\n\
         CrimsonSkiesFix.exe --version\n\
         CrimsonSkiesFix.exe --help\n\
         \nA verified unwrapped 1.02 CRIMSON.EXE and installed GOSDATA files are required.\n\
         ISO/CUE extraction and protected retail executables are not yet supported.\n\
         The original game and its data are never distributed by this project.\n",
        &COMMIT[..8]
    );
}

fn run() -> Result<i32, String> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        usage();
        return Ok(0);
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("CrimsonSkiesFix {VERSION}-{} Windows-x86", &COMMIT[..8]);
        return Ok(0);
    }
    if !args.is_empty() && !(args.len() == 1 && args[0] == "--verify")
        && !(args.len() == 2 && args[0] == "--import") {
        usage();
        return Err("unsupported command or number of arguments".into());
    }
    let launcher = env::current_exe().map_err(error)?;
    let base = launcher.parent().ok_or("cannot locate launcher directory")?;
    let game = base.join("CrimsonSkiesFix");
    if args.len() == 1 && args[0] == "--verify" {
        verify(&game)?;
        println!("Verified: original game EXE and bundled DirectDraw patch.");
        return Ok(0);
    }
    if !any_entry(&game) {
        let source = if args.len() == 2 {
            PathBuf::from(&args[1])
        } else {
            let choices = source_candidates(base, &game)?;
            if choices.len() != 1 {
                return Err(format!(
                    "expected one adjacent installed game directory, found {}; \
                     use --import DIR or place your existing verified game in CrimsonSkiesFix/. \
                     ISO/CUE installation is not supported yet",
                    choices.len()
                ));
            }
            choices[0].clone()
        };
        install_from(&source, &game)?;
    } else if args.len() == 2 {
        return Err("CrimsonSkiesFix/ already exists; --import never overwrites it".into());
    }
    let exe = validated_game_exe(&game)?;
    deploy_proxy(&game)?;
    deploy_documentation(&game)?;
    verify(&game)?;
    println!("Starting {}...", exe.display());
    let status = Command::new(&exe).current_dir(&game).status().map_err(error)?;
    let code = status.code().unwrap_or(2);
    println!("Game exited with status {code}.");
    Ok(code)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("CrimsonSkiesFix: ERROR: {err}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_game_hash_is_full_length() {
        assert_eq!(GAME_EXE_HASH.len(), 64);
        assert!(GAME_EXE_HASH.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(GAME_EXE_SIZE, 2_580_480);
    }
    #[test]
    fn embedded_proxy_present() {
        assert!(!BUNDLED_DDRAW.is_empty());
        assert_eq!(sha256::hex(BUNDLED_DDRAW).len(), 64);
    }
}
