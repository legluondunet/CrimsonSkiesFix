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
// Installer/diagnostic reports are never written into the DLL's gameplay log.
const INSTALLER_LOG: &str = "CrimsonSkiesFix_installer.log";

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

const INSTALL_RECEIPT: &str = "CrimsonSkiesFix.install-receipt.txt";

// The receipt records provenance, not proof: validate every original GAME/
// file and the official overlay independently before trusting the install.
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

/// Install authenticated retail files into a private staged directory.
/// The protected original EXE is kept until the approved EXE activation.
fn install_from_cd(base:&Path,game:&Path,updater:&Path)->Result<(),String> {
    if any_entry(game) {
        return Err(format!("existing game directory is never overwritten: {}",game.display()));
    }
    let name=game.file_name().ok_or("invalid CD installation destination")?;
    let stage=game.with_file_name(format!("{}.installing",name.to_string_lossy()));
    if any_entry(&stage) {
        return Err(format!("previous incomplete installation exists; inspect it: {}",stage.display()));
    }
    // The automatic install preflight has already authenticated the updater
    // and executable before extraction. The update application rechecks it.
    println!("Building original game installation from verified CD and official 1.02 updater...");
    let extracted=disc_inspect::extract_for_install(base,None,&stage)?;
    println!("Original CD edition authenticated: {}.",extracted.edition);
    // Follow source→destination file table of the original Microsoft setup:
    // expand all eight chapter CABs with edition-specific long ZBD names
    // and delete the CABs only after full content verification.
    let chapters=chapter_cabs::expand(&stage)?;
    // Extraction has already SHA-verified every original file against the
    // exact corresponding edition manifest. The updater verifies again.
    update102::apply_asset(base,&stage,extracted.edition,Some(updater))?;
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
    Ok(())
}

/// Explicit fast path: do not call any of the verification, source-discovery,
/// repair, or extraction routines. Let the OS report an absent or invalid EXE.
/// Never write to the installer report: a direct launch is not an installation.
fn launch_without_verification(game:&Path)->Result<i32,String>{
    let exe=game.join("CRIMSON.EXE");
    println!("--launch-no-verify: skipping all integrity, existence and patch checks; starting {}",
        exe.display());
    let exit=Command::new(&exe).current_dir(game).status()
        .map_err(|e|format!("--launch-no-verify could not start {}: {e}",exe.display()))?;
    let code=exit.code().unwrap_or(2);
    println!("--launch-no-verify: game exited with code {code}");
    Ok(code)
}

/// Accepted CLI forms, kept in one place so removed diagnostic commands cannot
/// accidentally remain reachable via the old ad hoc argument-count checks.
fn supported_command(args:&[OsString])->bool{
    let Some((name,rest))=args.split_first() else{return true;};
    rest.is_empty() && matches!(name.to_str(),
        Some("--help"|"-h"|"--version"|"--verify"|"--install-only"|"--launch-no-verify"))
}

fn usage() {
    println!(
        "CrimsonSkiesFix {VERSION} ({})\n\
         Automatic installer / verifier / launcher for Crimson Skies 1.02.\n\
         \nUsage:\n\
         CrimsonSkiesFix.exe            verify, install from adjacent CD or repair, then play\n\
         CrimsonSkiesFix.exe --install-only   install/repair fully, do not launch the game\n\
         CrimsonSkiesFix.exe --launch-no-verify         start CRIMSON.EXE directly, no verification or repair\n\
         CrimsonSkiesFix.exe --verify         verify installed EXE and embedded ddraw.dll\n\

         CrimsonSkiesFix.exe --inspect image.iso   inspect unsupported CD, separate inspect log\n\
         CrimsonSkiesFix.exe --inspect image.cue [track.bin|track.img]\n\
         CrimsonSkiesFix.exe --verbose    detailed diagnostics, can combine with a command\n\
         CrimsonSkiesFix.exe --version\n\
         CrimsonSkiesFix.exe --help (-h also accepted)\n\
         \nA verified unwrapped 1.02 CRIMSON.EXE and installed GOSDATA files are required.\n\
         CD GAME files and official 1.02 ROF can be installed; protected CD executables are preserved.\n\
         A full installation requires the approved 1.02 EXE, bundled ddraw.dll patch,\n\
         and official 1.02 updater BEFORE CD extraction starts.\n\
         --launch-no-verify bypasses verification by explicit user request; it never installs.\n\
         The original game and its data are never distributed by this project.\n",
        &COMMIT[..8]
    );
}

fn inspect_cd(base:&Path,image:&Path,track:Option<&Path>,verbose:bool)->Result<i32,String>{
    let output=base.join("CrimsonSkiesFix_inspect.log");
    if output.exists() && !regular(&output) {
        return Err(format!("inspect log is not a regular file: {}",output.display()));
    }
    let outcome=disc_inspect::inspect_with_override(base,Some(image),track);
    let report=match &outcome {
        Ok(info)=>info.clone(),
        Err(failure)=>format!("CrimsonSkiesFix INSPECT FAILED\nSource={}\nTrack={}\nError={failure}\n",
            image.display(),track.map(|p|p.display().to_string()).unwrap_or_default())
    };
    fs::write(&output,report.as_bytes()).map_err(error)?;
    println!("Inspection report: {}",output.display());
    if verbose {println!("{report}");}
    outcome.map(|_|0).map_err(|err|format!("CD inspection failed: {err}"))
}
fn run() -> Result<i32, String> {
    let input: Vec<OsString> = env::args_os().skip(1).collect();
    let verbose=input.iter().any(|arg|arg=="--verbose");
    if input.iter().filter(|arg|*arg=="--verbose").count()>1 {
        return Err("--verbose specified twice".into());
    }
    let args:Vec<OsString>=input.into_iter().filter(|arg|arg!="--verbose").collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        usage();
        return Ok(0);
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("CrimsonSkiesFix {VERSION}-{} Windows-x86", &COMMIT[..8]);
        return Ok(0);
    }
    let inspect=args.first().is_some_and(|arg|arg=="--inspect");
    if !inspect && !supported_command(&args){
        usage();
        return Err("unsupported command or number of arguments".into());
    }
    let launcher = env::current_exe().map_err(error)?;
    let base = launcher.parent().ok_or("cannot locate launcher directory")?;
    if inspect {
        if args.len()<2 || args.len()>3 {
            usage();return Err("Usage: --inspect image.iso | image.cue [track.bin|track.img]".into());
        }
        let image=PathBuf::from(&args[1]);
        let track=args.get(2).map(PathBuf::from);
        return inspect_cd(base,&image,track.as_deref(),verbose);
    }
    let game = base.join("CrimsonSkiesFix");
    if verbose && !(args.len()==1 && args[0]=="--launch-no-verify") {
        println!("VERBOSE: launcher={} game={} installed_exe={} runtime_dll={}",
            launcher.display(),game.display(),
            regular(&game.join("CRIMSON.EXE")),regular(&game.join("ddraw.dll")));
    }
    // Explicit bypass must not inspect, hash, repair, migrate or extract game
    // files or modify an installer report. OS-level process launch only.
    if args.len()==1 && args[0]=="--launch-no-verify" {
        return launch_without_verification(&game);
    }
    // Preserve a previous launcher-generated hyphenated report across a
    // verified-only relaunch after the filename spelling changes.
    auto_install::migrate_previous_installer_log(base);
    if args.len() == 1 && args[0] == "--verify" {
        if regular(&game.join(INSTALL_RECEIPT)){
            verify_installed_cd_assets(&game)?;
        }
        verify(&game)?;
        println!("Verified: approved game EXE, game data and DirectDraw patch.");
        return Ok(0);
    }
    if args.len()==1 && args[0]=="--install-only" {
        return auto_install::auto_run(base,&game,false);
    }
    auto_install::auto_run(base,&game,true)
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
    fn cli_has_only_the_supported_commands(){
        for flags in [
            vec![],vec!["--help"],vec!["-h"],vec!["--version"],vec!["--verify"],
            vec!["--install-only"],vec!["--launch-no-verify"],
        ]{
            let args=flags.iter().map(OsString::from).collect::<Vec<_>>();
            assert!(supported_command(&args),"expected supported: {flags:?}");
        }
        for flags in [
            vec!["--launch"],vec!["--import","original"],vec!["--audit-shortnames"],
            vec!["--verify-install"],vec!["--verify-install","FR"],
            vec!["--inspect-cd"],vec!["--inspect-cd","disc.iso"],
            vec!["--extract-cd"],vec!["--extract-cd","disc.cue"],
            vec!["--install-cd"],vec!["--install-cd","disc.iso"],
            vec!["--install-cd","disc.iso","patch.exe"],
            vec!["--audit-shortnames","FR"],vec!["--audit-shortnames","FR","ref.zbd"],
            vec!["--inspect-update"],vec!["--inspect-update","update.exe"],
            vec!["--extract-update","FR"],vec!["--extract-update","FR","update.exe"],
            vec!["--install-cd","disc.iso","patch.exe","--into","FR"],
            vec!["--install-only","disc.iso"],vec!["--launch-no-verify","--verify"],
            vec!["--launch-no-verify","missing.exe"],vec!["--verify","other"],
            vec!["--unknown"],
        ]{
            let args=flags.iter().map(OsString::from).collect::<Vec<_>>();
            assert!(!supported_command(&args),"unexpected command accepted: {flags:?}");
        }
    }
    #[test]
    fn automatic_cd_install_prerequisites_fail_without_creating_staging(){
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!(
            "crimson-extract-prerequisite-gate-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("disc.iso"),b"fake authenticated source").unwrap();
        let game=root.join("CrimsonSkiesFix");
        let err=auto_install::preflight_install_sources(&root,&game,None).unwrap_err();
        assert!(err.contains("approved decrypted Crimson Skies 1.02 EXE"));
        assert!(!any_entry(&root.join("CrimsonSkiesFix")));
        // The preflight must not create ANY directory, regardless of how
        // future staging names are changed.
        assert!(fs::read_dir(&root).unwrap().all(|entry|
            !entry.unwrap().file_type().unwrap().is_dir()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_launch_missing_game_makes_no_install_directory_or_report(){
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!(
            "crimson-direct-launch-no-verify-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        let game=root.join("CrimsonSkiesFix");
        assert!(launch_without_verification(&game).is_err());
        assert!(!any_entry(&game));
        assert!(!any_entry(&root.join(INSTALLER_LOG)));
        fs::remove_dir_all(root).unwrap();
    }

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
    fn cd_install_refuses_existing_game_and_keeps_files() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("crimson-cd-install-reject-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        let dest=root.join("CrimsonSkiesFix");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("user-save.dat"),b"keep").unwrap();
        assert!(install_from_cd(&root,&dest,&root.join("update102.exe")).is_err());
        assert_eq!(fs::read(dest.join("user-save.dat")).unwrap(),b"keep");
        fs::remove_dir_all(root).unwrap();
    }

}
