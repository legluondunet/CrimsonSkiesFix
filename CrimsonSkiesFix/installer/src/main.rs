//! Standalone bootstrap launcher for the owner's already-installed Crimson Skies 1.02.
//! No copyrighted game assets, executable or disc patches are distributed.
mod sha256;
mod disc_inspect;
mod update102;

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

// Only migrate known files from a previous nested proxy configuration.
// Never silently discard unknown content from that directory.
fn recognized_old_config(from: &Path) -> Result<(), String> {
    if !directory(from) { return Err("unsafe old CrimsonSkiesFix directory".into()); }
    for entry in fs::read_dir(from).map_err(error)? {
        let entry = entry.map_err(error)?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let known = matches!(name.as_str(),
            "crimsonskiesfix.ini" | "crimsonskiesfix.ini.example" |
            "readme.md" | "license" | "build-info.txt");
        if !known || !regular(&entry.path()) {
            return Err(format!(
                "unknown content in previous CrimsonSkiesFix subdirectory: {}; refusing to discard it",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path, root: bool) -> Result<(), String> {
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
            if root && entry.file_name().to_string_lossy().eq_ignore_ascii_case("CrimsonSkiesFix") {
                recognized_old_config(&src)?;
                continue;
            }
            copy_tree(&src, &dst, false)?;
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
    copy_tree(source, &staging, true)?;
    // Preserve the previous active INI while avoiding a second nested folder.
    let old_ini = source.join("CrimsonSkiesFix").join("CrimsonSkiesFix.ini");
    let root_ini = staging.join("CrimsonSkiesFix.ini");
    if regular(&old_ini) && !any_entry(&root_ini) {
        fs::copy(&old_ini, &root_ini).map_err(error)?;
        let mut permissions = fs::metadata(&root_ini).map_err(error)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&root_ini, permissions).map_err(error)?;
    }
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
         CrimsonSkiesFix.exe --inspect-cd          scan one adjacent ISO/CUE to CrimsonSkiesFix.log\n\
         CrimsonSkiesFix.exe --inspect-cd IMAGE    scan a specified ISO/CUE image\n\
         CrimsonSkiesFix.exe --extract-cd         extract recognized CD GAME/ to a staging folder\n\
         CrimsonSkiesFix.exe --extract-cd IMAGE   extract a specified ISO/CUE (read-only source)\n\
         CrimsonSkiesFix.exe --inspect-update       verify the original 1.02 updater (no execution)\n\
         CrimsonSkiesFix.exe --inspect-update FILE  inspect a specified updater (any filename)\n\
         CrimsonSkiesFix.exe --extract-update      verify and extract official ROF to CD test directory\n\
         CrimsonSkiesFix.exe --extract-update FILE apply specified verified updater to CD test directory\n\
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
        && !(args.len() == 2 && args[0] == "--import")
        && !(args.len() == 1 && args[0] == "--inspect-cd")
        && !(args.len() == 2 && args[0] == "--inspect-cd")
        && !(args.len() == 1 && args[0] == "--extract-cd")
        && !(args.len() == 2 && args[0] == "--extract-cd")
        && !(args.len() == 1 && args[0] == "--inspect-update")
        && !(args.len() == 2 && args[0] == "--inspect-update")
        && !(args.len() == 1 && args[0] == "--extract-update")
        && !(args.len() == 2 && args[0] == "--extract-update") {
        usage();
        return Err("unsupported command or number of arguments".into());
    }
    let launcher = env::current_exe().map_err(error)?;
    let base = launcher.parent().ok_or("cannot locate launcher directory")?;
    let game = base.join("CrimsonSkiesFix");
    if !args.is_empty() && args[0] == "--extract-update" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))}else{None};
        let stage=base.join("CrimsonSkiesFix.extract-test");
        // No update is applied to a published installation, only to a
        // non-playable game image extracted in this isolated test folder.
        let result=update102::apply_asset(base,&stage,selected.as_deref());
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: cannot overwrite non-file".into());
        }
        let report=match &result {
            Ok(status)=>format!("CrimsonSkiesFix UPDATE102 EXTRACT v1\n\
                 UpdaterIdentity=VERIFIED_OFFICIAL_1.02\n\
                 SourceSHA256={}\n\
                 Target={}\n\
                 Asset=GOSDATA/ASSETS/CRIMPTCH.ROF\n\
                 AssetSHA256={}\n\
                 AssetStatus={status}\n\
                 ProtectedGameExe=NOT_APPROVED\n\
                 InstallPublished=NO_STAGING_ONLY\n\
                 EXTRACT_RESULT=COMPLETE\n",
                 update102::PATCH_SHA256,stage.display(),update102::ASSET_SHA256),
            Err(err)=>format!("CrimsonSkiesFix UPDATE102 EXTRACT v1\n\
                 Target={}\n\
                 EXTRACT_RESULT=ERROR\nERROR={err}\n",stage.display()),
        };
        fs::write(&log,report).map_err(error)?;
        return match result {
            Ok(status)=>{
                println!("Official 1.02 LZX CAB asset: {status}; verified in {}.",stage.display());
                println!("Protected CRIMSON.EXE was NOT replaced or launched.");
                Ok(0)
            }
            Err(err)=>Err(format!("{err}; see {}",log.display())),
        };
    }
    if !args.is_empty() && args[0] == "--extract-cd" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))}else{None};
        let staging=base.join("CrimsonSkiesFix.extract-test");
        // This command is a non-destructive extraction test, NOT an install:
        // never publish under CrimsonSkiesFix/ or execute the protected EXE.
        let result=disc_inspect::extract_game(base,selected.as_deref(),&staging);
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: refusing a non-file log target".into());
        }
        let report=match &result {
            Ok(r)=>format!("CrimsonSkiesFix CD EXTRACT v1\n\
                Edition={}\nFilesExtracted={}\nBytesExtracted={}\n\
                Destination={}\nProtectedGameExe=NOT_APPROVED\n\
                InstallPublished=NO_STAGING_ONLY\nEXTRACT_RESULT=COMPLETE\n",
                r.edition,r.file_count,r.bytes,staging.display()),
            Err(why)=>format!("CrimsonSkiesFix CD EXTRACT v1\n\
                Destination={}\nInstallPublished=NO_STAGING_ONLY\n\
                EXTRACT_RESULT=ERROR\nERROR={why}\n",staging.display()),
        };
        fs::write(&log,report).map_err(error)?;
        return match result {
            Ok(r)=>{
                println!("Verified extraction of {} files ({} bytes) for {} into {}.",
                    r.file_count,r.bytes,r.edition,staging.display());
                println!("The original protected game is NOT launched. See CrimsonSkiesFix.log.");
                Ok(0)
            }
            Err(why)=>Err(format!("{why}; see {}",log.display())),
        };
    }
    if !args.is_empty() && args[0] == "--inspect-update" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))} else {None};
        let report=update102::inspect(base,selected.as_deref());
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: cannot replace non-file".into());
        }
        let content=match &report {
            Ok(content)=>content.clone(),
            Err(why)=>format!("CrimsonSkiesFix UPDATE102 INSPECT v1\nINSPECT_RESULT=ERROR\nERROR={why}\n"),
        };
        fs::write(&log,content).map_err(error)?;
        return match report {
            Ok(_)=>{
                println!("Verified official Crimson Skies 1.02 updater; no installation was performed. Log: {}",log.display());
                Ok(0)
            },
            Err(why)=>Err(format!("{why}; details: {}",log.display())),
        };
    }
    if !args.is_empty() && args[0] == "--inspect-cd" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))}else{None};
        // Explicit diagnostic scans always produce a log even with Log=0.
        // They never launch, install, patch or modify the disc image.
        let report=disc_inspect::inspect(base,selected.as_deref());
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: refusing to overwrite a non-file".into());
        }
        let text=match &report {
            Ok(content)=>content.clone(),
            Err(why)=>format!("CrimsonSkiesFix CD INSPECT v1\nINSPECT_RESULT=ERROR\nERROR={why}\n"),
        };
        fs::write(&log,text).map_err(error)?;
        return match report {
            Ok(content)=>{
                let count=content.lines().find(|line|line.starts_with("FilesTotal="))
                    .unwrap_or("FilesTotal=UNKNOWN");
                println!("CD inspection complete ({count}); full internal-file manifest: {}",log.display());
                Ok(0)
            }
            Err(why)=>Err(format!("{why}; details: {}",log.display())),
        };
    } else if args.len() == 1 && args[0] == "--verify" {
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
    #[test]
    fn old_nested_configuration_is_only_imported_when_known() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "crimsonskiesfix-flat-layout-{}-{id}", std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("CrimsonSkiesFix.ini"), "[Debug]\\nLog=1\\n").unwrap();
        assert!(recognized_old_config(&dir).is_ok());
        fs::write(dir.join("user-save.dat"), "save data").unwrap();
        assert!(recognized_old_config(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
