//! Standalone bootstrap launcher for the owner's already-installed Crimson Skies 1.02.
//! No copyrighted game assets, executable or disc patches are distributed.
mod sha256;
mod disc_inspect;
mod update102;
mod managed_gos_patch;
mod chapter_cabs;
mod auto_install;

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

const INSTALL_RECEIPT: &str = "CrimsonSkiesFix.install-receipt.txt";

// Explicit multi-edition testing must not silently reuse the default game
// directory. Only these fixed sibling names are accepted: no traversal,
// arbitrary Windows devices, symlinks to other directories, or overwrites.
fn test_install_destination(base:&Path,selector:&str)->Result<(&'static str,PathBuf),String> {
    let edition=disc_inspect::edition_selector(selector)
        .ok_or("unknown test installation edition: expected USA, FR or DE")?;
    let suffix=match edition {
        "C_SKIES_DISC_001"=>"USA",
        "C_SKIES_FR_001"=>"FR",
        "C_SKIES_DE_001"=>"DE",
        _=>return Err("unknown test installation edition".into()),
    };
    Ok((edition,base.join(format!("CrimsonSkiesFix.{suffix}"))))
}

struct DiscInstallation {
    edition:&'static str,
    file_count:usize,
    byte_count:u64,
    asset_status:&'static str,
    chapter_members:usize,
    chapter_zbd_files:usize,
    chapter_bytes:u64,
    chapter_short_aliases:usize,
    retail_support_files:usize,
    retail_support_bytes:u64,
    retail_renamed_files:usize,
    destination:PathBuf,
}

// The receipt records provenance, not proof: --verify-install must independently
// hash every original GAME/ file and the official overlay before trusting it.
fn installed_cd_edition(game:&Path)->Result<&'static str,String> {
    let file=game.join(INSTALL_RECEIPT);
    if !regular(&file) {return Err("missing or unsafe CD installation receipt".into());}
    if fs::metadata(&file).map_err(error)?.len()>2048 {
        return Err("CD installation receipt is unexpectedly large".into());
    }
    let content=fs::read_to_string(&file).map_err(error)?;
    let states=content.lines().filter_map(|l|l.strip_prefix("InstallState="))
        .collect::<Vec<_>>();
    if states.len()!=1 || !matches!(states[0],
        "GAME_FILES_READY_EXECUTABLE_PENDING"|"GAME_READY") {
        return Err("unexpected CD installation state".into());
    }
    let matches=content.lines().filter_map(|l|l.strip_prefix("Edition="))
        .collect::<Vec<_>>();
    if matches.len()!=1 {return Err("CD installation receipt missing unique edition".into());}
    disc_inspect::edition_selector(matches[0])
        .ok_or("unrecognized CD installation edition".into())
}

fn verify_installed_cd_assets(game:&Path)->Result<(&'static str,usize),String> {
    if !directory(game) {return Err("missing or unsafe installed game directory".into());}
    let edition=installed_cd_edition(game)?;
    let state=fs::read_to_string(game.join(INSTALL_RECEIPT)).map_err(error)?;
    let activated=state.lines().any(|l|l=="InstallState=GAME_READY");
    let count=disc_inspect::verify_installed_without_cabs(game,edition,activated)?;
    // These eight original retail loader/setup companion files come from
    // the CD root, not its GAME/ directory. Verify the original CD hashes.
    let support_count=disc_inspect::verify_retail_support(game,edition)?;
    if support_count!=disc_inspect::RETAIL_SUPPORT_FILES {
        return Err("incomplete retail root support files".into());
    }
    // A retail file installation must contain both the chapter content
    // and the seven original CD root companions plus localized Readme.
    let receipt=fs::read_to_string(game.join(INSTALL_RECEIPT)).map_err(error)?;
    let values=receipt.lines().filter_map(|line|
        line.strip_prefix("ChapterManifestSHA256=")
    ).collect::<Vec<_>>();
    if values.len()!=1 || values[0].len()!=64 {
        return Err("installation receipt is missing unique chapter manifest SHA-256".into());
    }
    let chapters=chapter_cabs::verify_without_archives(game,values[0])?;
    for (key,actual) in [
        ("ChapterCabMembers",chapters.members as u64),
        ("ChapterCabZbdFiles",chapters.zbd_files as u64),
        ("ChapterCabPayloadBytes",chapters.payload_bytes),
        ("ChapterShortAliasCandidates",chapters.short_name_candidates as u64),
    ] {
        let values=receipt.lines().filter_map(|s|
            s.strip_prefix(&format!("{key}="))
        ).collect::<Vec<_>>();
        if values.len()!=1 || values[0].parse::<u64>().ok()!=Some(actual) {
            return Err(format!("chapter CAB verification receipt does not match: {key}"));
        }
    }
    // The game's own official 1.02 asset is immutable, but our GOS Video
    // control lock replaces it with a managed two-script overlay at runtime.
    // Verify the exact official archive OR a strictly authenticated managed
    // postimage together with its exact official original backup.
    // Never trigger a full CD extraction for a known reversible GOS overlay.
    managed_gos_patch::verify_game_overlay(game)?;
    for folder in ["SavedGames","Planes"] {
        if !directory(&game.join(folder)) {
            return Err(format!("missing or unsafe writable game directory: {folder}"));
        }
    }
    let proxy=game.join("ddraw.dll");
    if !regular(&proxy) || digest(&proxy)?!=sha256::hex(BUNDLED_DDRAW) {
        return Err("installed ddraw.dll missing or differs from bundled verified DLL".into());
    }
    if !regular(&game.join("CrimsonSkiesFix.ini.example"))
        || !regular(&game.join("build-info.txt")) {
        return Err("missing installed patch documentation/build information".into());
    }
    if activated {
        // The original protected EXE is hash-checked above using the
        // immutable original-CD manifest under its saved backup name.
        // The executable used to play is separately pinned to one SHA-256.
        validated_game_exe(game)?;
        if !regular(&game.join("CRIMSON.EXE.protected.bak")) {
            return Err("completed CD installation lost protected EXE backup".into());
        }
    }
    Ok((edition,count))
}

/// Complete the file-based installation without running the game or changing
/// the CD's protected executable. The pending executable is a *last* stage.
fn install_from_cd(base:&Path,disc:Option<&Path>,patch:Option<&Path>,game:&Path,
    expected_edition:Option<&str>)
    ->Result<DiscInstallation,String> {
    if any_entry(game) {
        return Err(format!("existing game directory is never overwritten: {}",game.display()));
    }
    let name=game.file_name().ok_or("invalid CD installation destination")?;
    let stage=game.with_file_name(format!("{}.installing",name.to_string_lossy()));
    if any_entry(&stage) {
        return Err(format!("previous incomplete installation exists; inspect it: {}",stage.display()));
    }
    // Fail before creating any output if the authentic updater is unavailable.
    let updater=update102::find(base,patch)?;
    println!("Building original game installation from verified CD and official 1.02 updater...");
    let extracted=disc_inspect::extract_for_install(base,disc,&stage)?;
    if expected_edition.is_some_and(|expected|expected!=extracted.edition) {
        return Err(format!(
            "image edition {} does not match selected test installation edition {}; staging retained for inspection",
            extracted.edition,expected_edition.unwrap()));
    }
    println!("Original CD edition authenticated: {}.",extracted.edition);
    // Follow source→destination file table of the original Microsoft setup:
    // expand all eight chapter CABs with edition-specific long ZBD names
    // and delete the CABs only after full content verification.
    let chapters=chapter_cabs::expand(&stage)?;
    // Extraction has already SHA-verified every original file against the
    // exact corresponding edition manifest. The updater verifies again.
    let asset=update102::apply_asset(base,&stage,extracted.edition,Some(&updater))?;
    // Before publication, independently compare all expanded members against
    // their authenticated CAB inputs. Seal per-file hashes in a manifest and
    // remove ONLY the eight temporary chapter CABs from this new staging tree.
    let finalized=chapter_cabs::finalize_without_archives(&stage,extracted.edition)
        .map_err(|err|format!(
            "chapter CAB source-to-destination mapping failed for authenticated edition {}: {err}",
            extracted.edition))?;
    if finalized.members!=chapters.members ||
        finalized.payload_bytes!=chapters.payload_bytes ||
        finalized.zbd_files!=chapters.zbd_files {
        return Err("chapter members changed while finalizing CAB removal".into());
    }
    // Reproduce the exact filenames from the owner's installed retail
    // directory AFTER official patch extraction verified the original names.
    let renamed=disc_inspect::normalize_game_filenames(&stage,extracted.edition)?;
    // Game saves and user-defined planes belong beside the game executable,
    // not in Program Files, system folders or a shared staging directory.
    for name in ["SavedGames","Planes"] {
        fs::create_dir(stage.join(name)).map_err(error)?;
    }
    deploy_proxy(&stage)?;
    deploy_documentation(&stage)?;
    let receipt=format!(
        "Project=CrimsonSkiesFix\nEdition={}\n\
         InstallState=GAME_FILES_READY_EXECUTABLE_PENDING\n\
         ExtractedGameFiles={}\nExtractedGameBytes={}\n\
         UpdateSHA256={}\nPatchAssetSHA256={}\n\
         ChapterCabMembers={}\nChapterCabZbdFiles={}\nChapterCabPayloadBytes={}\n\
         ChapterManifestSHA256={}\nChapterShortAliasCandidates={}\nChapterCABsPruned=8\n\
         RetailSupportFiles={}\nRetailSupportBytes={}\nRetailFilenameCorrections={}\n\
         ExecutablePolicy=ORIGINAL_PROTECTED_PRESERVED\n",
        extracted.edition,extracted.file_count,extracted.bytes,
        update102::PATCH_SHA256,update102::ASSET_SHA256,
        finalized.members,finalized.zbd_files,finalized.payload_bytes,
        finalized.manifest_sha,finalized.short_name_candidates,
        extracted.retail_support_files,extracted.retail_support_bytes,renamed
    );
    ensure_new_text(&stage.join(INSTALL_RECEIPT),&receipt)?;
    let (edition,verified_count)=verify_installed_cd_assets(&stage)?;
    if verified_count+disc_inspect::REMOVED_CHAPTER_CABS!=extracted.file_count ||
        edition!=extracted.edition ||
        extracted.retail_support_files!=disc_inspect::RETAIL_SUPPORT_FILES {

        return Err("installation verification disagrees with extracted source manifest".into());
    }
    // The final name only appears when source, ROF, proxy and writable
    // directories have all been verified. Never delete an interrupted stage.
    fs::rename(&stage,game).map_err(error)?;
    Ok(DiscInstallation{edition,file_count:extracted.file_count,
        byte_count:extracted.bytes,asset_status:asset,
        chapter_members:finalized.members,chapter_zbd_files:finalized.zbd_files,
        chapter_bytes:finalized.payload_bytes,
        chapter_short_aliases:finalized.short_name_candidates,
        retail_support_files:extracted.retail_support_files,
        retail_support_bytes:extracted.retail_support_bytes,
        retail_renamed_files:renamed,
        destination:game.to_path_buf()})
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
         CrimsonSkiesFix.exe            verify, install from adjacent CD or repair, then play\n\
         CrimsonSkiesFix.exe --import DIR   import an existing installed game and play\n\
         CrimsonSkiesFix.exe --verify   verify installed EXE and embedded ddraw.dll\n\
         CrimsonSkiesFix.exe --inspect-cd          scan one adjacent ISO/CUE to CrimsonSkiesFix.log\n\
         CrimsonSkiesFix.exe --inspect-cd IMAGE    scan a specified ISO/CUE image\n\
         CrimsonSkiesFix.exe --extract-cd         extract CD GAME/ into separate USA/FR/DE staging\n\
         CrimsonSkiesFix.exe --extract-cd IMAGE   extract a specified ISO/CUE (read-only source)\n\
         CrimsonSkiesFix.exe --install-cd          install one adjacent CD with official 1.02 updater\n\
         CrimsonSkiesFix.exe --install-cd IMAGE    install a specified ISO/CUE\n\
         CrimsonSkiesFix.exe --install-cd IMAGE PATCH    specify the official updater\n\
         CrimsonSkiesFix.exe --install-cd IMAGE PATCH --into ED    test alongside an existing install (ED=USA/FR/DE)\n\
         CrimsonSkiesFix.exe --verify-install      verify default CD installation\n\
         CrimsonSkiesFix.exe --verify-install ED   verify a named USA/FR/DE installation\n\
         CrimsonSkiesFix.exe --audit-shortnames [ED] [REFERENCE_ZBD]   inspect CAB 8.3 name aliases\n\
         CrimsonSkiesFix.exe --inspect-update       verify the original 1.02 updater (no execution)\n\
         CrimsonSkiesFix.exe --inspect-update FILE  inspect a specified updater (any filename)\n\
         CrimsonSkiesFix.exe --extract-update ED          verify official ROF for USA/FR/DE staged CD\n\
         CrimsonSkiesFix.exe --extract-update ED FILE     use verified update FILE for edition ED\n\
         CrimsonSkiesFix.exe --version\n\
         CrimsonSkiesFix.exe --help\n\
         \nA verified unwrapped 1.02 CRIMSON.EXE and installed GOSDATA files are required.\n\
         CD GAME files and official 1.02 ROF can be installed; protected CD executables are preserved.\n\
         Supply a local approved decrypted 1.02 executable alongside this launcher.\n\
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
        && !(args.len() == 2 && args[0] == "--extract-update")
        && !(args.len() == 3 && args[0] == "--extract-update")
        && !(args.len() == 1 && args[0] == "--install-cd")
        && !(args.len() == 2 && args[0] == "--install-cd")
        && !(args.len() == 3 && args[0] == "--install-cd")
        && !(args.len() == 5 && args[0] == "--install-cd" && args[3] == "--into")
        && !(args.len() == 1 && args[0] == "--verify-install")
        && !(args.len() == 2 && args[0] == "--verify-install")
        && !(args.len() == 1 && args[0] == "--audit-shortnames")
        && !(args.len() == 2 && args[0] == "--audit-shortnames")
        && !(args.len() == 3 && args[0] == "--audit-shortnames") {
        usage();
        return Err("unsupported command or number of arguments".into());
    }
    let launcher = env::current_exe().map_err(error)?;
    let base = launcher.parent().ok_or("cannot locate launcher directory")?;
    let game = base.join("CrimsonSkiesFix");
    if !args.is_empty() && args[0]=="--audit-shortnames" {
        let chosen=if args.len()>=2 {
            Some(test_install_destination(base,args[1].to_str()
                .ok_or("edition selector must be text")?)?)
        }else{None};
        let target=chosen.as_ref().map(|(_,path)|path).unwrap_or(&game);
        let reference=if args.len()==3 {Some(PathBuf::from(&args[2]))}else{None};
        let report=(||->Result<String,String>{
            let edition=installed_cd_edition(target)?;
            if chosen.as_ref().is_some_and(|(expected,_)|*expected!=edition) {
                return Err("requested edition does not match installed game".into());
            }
            if regular(&target.join(chapter_cabs::INSTALLED_MANIFEST)) {
                verify_installed_cd_assets(target)?;
                chapter_cabs::short_name_audit(target,reference.as_deref())
            }else {
                // Support a read-only name audit of older *verified* installs
                // that retain their original CABs. This does not modify
                // their files or rely on the missing new chapter manifest.
                disc_inspect::verify_staging(target,edition)?;
                chapter_cabs::short_name_audit_legacy(target,reference.as_deref())
            }
        })();
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: refusing non-file".into());
        }
        fs::write(&log,match &report {
            Ok(text)=>text.clone(),
            Err(why)=>format!("CrimsonSkiesFix CHAPTER SHORT NAMES AUDIT v1\n\
                AUDIT_RESULT=ERROR\nERROR={why}\n")
        }).map_err(error)?;
        return match report{
            Ok(_)=>{println!("Chapter filename audit complete: {}",log.display());Ok(0)}
            Err(why)=>Err(format!("{why}; see {}",log.display()))
        };
    }
    if !args.is_empty() && args[0]=="--verify-install" {
        let chosen=if args.len()==2 {
            Some(test_install_destination(base,args[1].to_str()
                .ok_or("edition selector must be text")?)?)
        } else {None};
        let verification_target=chosen.as_ref().map(|(_,path)|path).unwrap_or(&game);
        let result=verify_installed_cd_assets(verification_target).and_then(|(edition,files)|{
            if chosen.as_ref().is_some_and(|(expected,_)|*expected!=edition) {
                Err(format!("installed edition {edition} does not match requested test edition"))
            } else {Ok((edition,files))}
        });
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: refusing to replace non-file".into());
        }
        let report=match &result {
            Ok((edition,files))=>format!("CrimsonSkiesFix CD INSTALL VERIFY v1\n\
                VerifiedDirectory={}\n\
                Edition={edition}\nOriginalFilesVerified={files}\n\
                Official102Asset=SHA256_VERIFIED\nProxy=SHA256_VERIFIED\n\
                ChapterManifest=SHA256_VERIFIED\nChapterPayloads=SHA256_VERIFIED\n\
                ChapterCABsPruned=8\nRetailSupport=8_FILES_SHA256_VERIFIED\n\
                RetailNames=VERIFIED\n\
                WritableDirectories=SavedGames,Planes\nExecutableState={}\n\
                VERIFY_INSTALL_RESULT=COMPLETE\n",verification_target.display(),
                if validated_game_exe(verification_target).is_ok() {"APPROVED_READY"}else{"PROTECTED_PENDING"}),
            Err(why)=>format!("CrimsonSkiesFix CD INSTALL VERIFY v1\n\
                VerifiedDirectory={}\nVERIFY_INSTALL_RESULT=ERROR\nERROR={why}\n",verification_target.display()),
        };
        fs::write(&log,report).map_err(error)?;
        return match result {
            Ok((edition,files))=>{
                println!("CD installation files verified: edition={edition}; original files={files}; official 1.02 asset and ddraw.dll verified.");
                println!("Executable state: {}.",
                    if validated_game_exe(verification_target).is_ok() {"approved 1.02 ready"}else{"protected pending"});
                Ok(0)
            }
            Err(why)=>Err(format!("{why}; details: {}",log.display()))
        };
    }
    if !args.is_empty() && args[0]=="--install-cd" {
        let disc=if args.len()>=2 {Some(PathBuf::from(&args[1]))}else{None};
        let patch=if args.len()>=3 {Some(PathBuf::from(&args[2]))}else{None};
        let chosen=if args.len()==5 {
            Some(test_install_destination(base,args[4].to_str()
                .ok_or("test installation edition must be text")?)?)
        } else {None};
        let install_target=chosen.as_ref().map(|(_,path)|path).unwrap_or(&game);
        let expected=chosen.as_ref().map(|(edition,_)|*edition);
        let result=install_from_cd(base,disc.as_deref(),patch.as_deref(),install_target,expected);
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: cannot replace a non-file".into());
        }
        let report=match &result {
            Ok(r)=>format!("CrimsonSkiesFix CD INSTALL v1\n\
                Edition={}\nFilesInstalled={}\nGamePayloadBytes={}\n\
                Official102Asset={}\n\
                Destination={}\nInstalledProxy=VERIFIED\n\
                SavedGamesDirectory=CREATED\nPlanesDirectory=CREATED\n\
                ChapterCabMembers={}\nChapterZbdFiles={}\nChapterPayloadBytes={}\n\
                ChapterContentVerified=YES\n\
                ChapterCABsPruned=8\nChapterShortAliasCandidates={}\n\
                RetailSupportFiles={}\nRetailSupportBytes={}\nRetailFilenameCorrections={}\n\
                GameFilesVerified=YES\nExecutableState=PROTECTED_PENDING\n\
                INSTALL_RESULT=GAME_FILES_READY_EXECUTABLE_PENDING\n",
                r.edition,r.file_count,r.byte_count,r.asset_status,r.destination.display(),
                r.chapter_members,r.chapter_zbd_files,r.chapter_bytes,
                r.chapter_short_aliases,
                r.retail_support_files,r.retail_support_bytes,r.retail_renamed_files),
            Err(why)=>format!("CrimsonSkiesFix CD INSTALL v1\n\
                Destination={}\nINSTALL_RESULT=ERROR\nERROR={why}\n",
                install_target.display()),
        };
        fs::write(&log,report).map_err(error)?;
        return match result {
            Ok(r)=>{
                println!("Installed and verified {} original files ({}) for {} in {}.",
                    r.file_count,r.byte_count,r.edition,r.destination.display());
                println!("Expanded {} chapter CAB members, including {} ZBD files ({} bytes).",
                    r.chapter_members,r.chapter_zbd_files,r.chapter_bytes);
                println!("Pruned all 8 temporary chapter CABs after verification; {} filenames contain possible DOS 8.3 aliases.",
                    r.chapter_short_aliases);
                println!("Installed {} authenticated retail CD-root files and corrected {} installed game filenames.",
                    r.retail_support_files,r.retail_renamed_files);
                println!("Official 1.02 resource: {}. Bundled ddraw.dll and writable game folders installed.",r.asset_status);
                println!("Installation data ready; protected game executable kept unchanged for the final step.");
                Ok(0)
            }
            Err(why)=>Err(format!("{why}; see {}",log.display()))
        };
    }
    if !args.is_empty() && args[0] == "--extract-update" {
        let selector=args[1].to_str().ok_or("edition selector must be UTF-8")?;
        let edition=disc_inspect::edition_selector(selector)
            .ok_or("unknown update edition: use USA, FR, or DE")?;
        let selected=if args.len()==3 {Some(PathBuf::from(&args[2]))}else{None};
        let stage=disc_inspect::stage_path(base,edition)?;
        // A mandatory edition selector prevents accidentally applying an
        // update to a staging directory left over from another CD.
        // Full file hash verification also rejects a renamed/mixed staging.
        let result=update102::apply_asset(base,&stage,edition,selected.as_deref());
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: cannot overwrite non-file".into());
        }
        let report=match &result {
            Ok(status)=>format!("CrimsonSkiesFix UPDATE102 EXTRACT v1\n\
                 UpdaterIdentity=VERIFIED_OFFICIAL_1.02\n\
                 Edition={edition}\n\
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
                 Edition={edition}\n\
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
        // The CD profile chooses a unique staging directory. The former
        // CrimsonSkiesFix.extract-test (without an edition) is left untouched.
        let result=disc_inspect::extract_game(base,selected.as_deref());
        let log=base.join("CrimsonSkiesFix.log");
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix.log: refusing a non-file log target".into());
        }
        let report=match &result {
            Ok(r)=>format!("CrimsonSkiesFix CD EXTRACT v1\n\
                Edition={}\nFilesExtracted={}\nBytesExtracted={}\n\
                Destination={}\nProtectedGameExe=NOT_APPROVED\n\
                InstallPublished=NO_STAGING_ONLY\nEXTRACT_RESULT=COMPLETE\n",
                r.edition,r.file_count,r.bytes,r.destination.display()),
            Err(why)=>format!("CrimsonSkiesFix CD EXTRACT v1\n\
                Destination=NONE (edition-specific; extraction refused)\nInstallPublished=NO_STAGING_ONLY\n\
                EXTRACT_RESULT=ERROR\nERROR={why}\n"),
        };
        fs::write(&log,report).map_err(error)?;
        return match result {
            Ok(r)=>{
                println!("Verified extraction of {} files ({} bytes) for {} into {}.",
                    r.file_count,r.bytes,r.edition,r.destination.display());
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
        if regular(&game.join(INSTALL_RECEIPT)){
            verify_installed_cd_assets(&game)?;
        }
        verify(&game)?;
        println!("Verified: approved game EXE, game data and DirectDraw patch.");
        return Ok(0);
    }
    if args.len()==2 {
        if any_entry(&game) {
            return Err("CrimsonSkiesFix/ already exists; --import never overwrites it".into());
        }
        install_from(&PathBuf::from(&args[1]),&game)?;
    }
    auto_install::auto_run(base,&game)
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
    fn installed_cd_requires_receipt_and_never_treats_empty_data_as_installed() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("crimson-cd-install-check-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        assert!(verify_installed_cd_assets(&root).is_err());
        fs::write(root.join(INSTALL_RECEIPT),
            "Project=CrimsonSkiesFix\nEdition=C_SKIES_FR_001\nInstallState=GAME_FILES_READY_EXECUTABLE_PENDING\n").unwrap();
        assert!(verify_installed_cd_assets(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cd_receipt_accepts_ready_only_once_and_tracks_exact_edition() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("crimson-cd-ready-{}-{id}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let receipt=dir.join(INSTALL_RECEIPT);
        fs::write(&receipt,
            "Project=CrimsonSkiesFix\nEdition=C_SKIES_DISC_001\nInstallState=GAME_READY\n"
        ).unwrap();
        assert_eq!(installed_cd_edition(&dir).unwrap(),"C_SKIES_DISC_001");
        // A receipt alone can identify the intended edition, but it can
        // never validate the actual files or executable.
        assert!(verify_installed_cd_assets(&dir).is_err());
        fs::write(&receipt,
            "Project=CrimsonSkiesFix\nEdition=C_SKIES_DISC_001\nInstallState=GAME_READY\nInstallState=GAME_READY\n"
        ).unwrap();
        assert!(installed_cd_edition(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn side_by_side_test_directories_never_alias_default_install() {
        let base=Path::new("test");
        let d=base.join("CrimsonSkiesFix");
        let (usa,usadir)=test_install_destination(base,"USA").unwrap();
        let (fr,frdir)=test_install_destination(base,"FR").unwrap();
        let (de,dedir)=test_install_destination(base,"DE").unwrap();
        assert_eq!(usa,"C_SKIES_DISC_001");
        assert_eq!(fr,"C_SKIES_FR_001");
        assert_eq!(de,"C_SKIES_DE_001");
        for candidate in [&usadir,&frdir,&dedir] {
            assert_ne!(*candidate,d);
        }
        assert_ne!(usadir,dedir);
        assert_ne!(usadir,frdir);
        assert_ne!(frdir,dedir);
        for invalid in ["..","../FR","C:","NUL","random",""] {
            assert!(test_install_destination(base,invalid).is_err());
        }
    }
    #[test]
    fn cd_install_refuses_existing_game_and_keeps_files() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("crimson-cd-install-reject-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        let dest=root.join("CrimsonSkiesFix");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("user-save.dat"),b"keep").unwrap();
        assert!(install_from_cd(&root,None,None,&dest,None).is_err());
        assert_eq!(fs::read(dest.join("user-save.dat")).unwrap(),b"keep");
        fs::remove_dir_all(root).unwrap();
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
