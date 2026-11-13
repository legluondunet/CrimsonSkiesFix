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

/// Explicit fast path: do not call any of the verification, source-discovery,
/// repair, or extraction routines. Let the OS report an absent or invalid EXE.
/// Never write to the installer report: a direct launch is not an installation.
fn launch_without_verification(game:&Path)->Result<i32,String>{
    let exe=game.join("CRIMSON.EXE");
    println!("--launch: skipping all integrity, existence and patch checks; starting {}",
        exe.display());
    let exit=Command::new(&exe).current_dir(game).status()
        .map_err(|e|format!("--launch could not start {}: {e}",exe.display()))?;
    let code=exit.code().unwrap_or(2);
    println!("--launch: game exited with code {code}");
    Ok(code)
}

/// Accepted CLI forms, kept in one place so removed diagnostic commands cannot
/// accidentally remain reachable via the old ad hoc argument-count checks.
fn supported_command(args:&[OsString])->bool{
    let Some((name,rest))=args.split_first() else{return true;};
    match (name.to_str(),rest.len()){
        (Some("--help"|"--version"|"--verify"|"--install-only"|"--launch"|
              "--inspect-cd"|"--extract-cd"|"--install-cd"|
              "--verify-install"),0)=>true,
        (Some("--inspect-cd"|"--extract-cd"|"--install-cd"|
              "--verify-install"),1)=>true,
        (Some("--install-cd"),2)=>true,
        _=>false,
    }
}

fn usage() {
    println!(
        "CrimsonSkiesFix {VERSION} ({})\n\
         Automatic installer / verifier / launcher for Crimson Skies 1.02.\n\
         \nUsage:\n\
         CrimsonSkiesFix.exe            verify, install from adjacent CD or repair, then play\n\
         CrimsonSkiesFix.exe --install-only   install/repair fully, do not launch the game\n\
         CrimsonSkiesFix.exe --launch         start CRIMSON.EXE directly, no verification or repair\n\
         CrimsonSkiesFix.exe --verify         verify installed EXE and embedded ddraw.dll\n\
         CrimsonSkiesFix.exe --inspect-cd          scan one adjacent ISO/CUE to CrimsonSkiesFix_installer.log\n\
         CrimsonSkiesFix.exe --inspect-cd IMAGE    scan a specified ISO/CUE image\n\
         CrimsonSkiesFix.exe --extract-cd         extract CD after prerequisite checks (USA/FR/DE staging)\n\
         CrimsonSkiesFix.exe --extract-cd IMAGE   extract specified ISO/CUE after prerequisites\n\
         CrimsonSkiesFix.exe --install-cd          install one adjacent CD with official 1.02 updater\n\
         CrimsonSkiesFix.exe --install-cd IMAGE    install a specified ISO/CUE\n\
         CrimsonSkiesFix.exe --install-cd IMAGE PATCH    specify the official updater\n\
         CrimsonSkiesFix.exe --verify-install      verify default CD installation\n\
         CrimsonSkiesFix.exe --verify-install ED   verify a named USA/FR/DE installation\n\
         CrimsonSkiesFix.exe --version\n\
         CrimsonSkiesFix.exe --help\n\
         \nA verified unwrapped 1.02 CRIMSON.EXE and installed GOSDATA files are required.\n\
         CD GAME files and official 1.02 ROF can be installed; protected CD executables are preserved.\n\
         A full installation requires the approved 1.02 EXE, bundled ddraw.dll patch,\n\
         and official 1.02 updater BEFORE CD extraction starts.\n\
         --launch bypasses verification by explicit user request; it never installs.\n\
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
    if !supported_command(&args){
        usage();
        return Err("unsupported command or number of arguments".into());
    }
    let launcher = env::current_exe().map_err(error)?;
    let base = launcher.parent().ok_or("cannot locate launcher directory")?;
    let game = base.join("CrimsonSkiesFix");
    // Explicit bypass must not inspect, hash, repair, migrate or extract game
    // files or modify an installer report. OS-level process launch only.
    if args.len()==1 && args[0]=="--launch" {
        return launch_without_verification(&game);
    }
    // Preserve a previous launcher-generated hyphenated report across a
    // verified-only relaunch after the filename spelling changes.
    auto_install::migrate_previous_installer_log(base);
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
        let log=base.join(INSTALLER_LOG);
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix_installer.log: refusing to replace non-file".into());
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
        // A successful read-only verification must not erase the last
        // installation/repair report. Record it only if no report exists;
        // failures always replace it with the actionable verification error.
        if result.is_err() || !any_entry(&log){
            fs::write(&log,report).map_err(error)?;
        }
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
        // Even manual CD installation must refuse to extract anything until
        // the approved EXE, bundled DirectDraw patch and official 1.02 update
        // have been located and validated. The manual command still leaves
        // the original protected EXE pending activation.
        let install_target=&game;
        let result=auto_install::preflight_install_sources(base,install_target,patch.as_deref())
            .and_then(|_|install_from_cd(base,disc.as_deref(),patch.as_deref(),install_target,None));
        let log=base.join(INSTALLER_LOG);
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix_installer.log: cannot replace a non-file".into());
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
    if !args.is_empty() && args[0] == "--extract-cd" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))}else{None};
        // The CD profile chooses a unique staging directory. The former
        // CrimsonSkiesFix.extract-test (without an edition) is left untouched.
        // Explicit diagnostic extraction obeys the same prerequisite gate
        // as full installation: do not create ANY CD staging folder until
        // the approved 1.02 game EXE, bundled patch and official updater
        // have all been authenticated.
        let result=auto_install::preflight_install_sources(base,&game,None)
            .and_then(|_|disc_inspect::extract_game(base,selected.as_deref()));
        let log=base.join(INSTALLER_LOG);
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix_installer.log: refusing a non-file log target".into());
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
                println!("The original protected game is NOT launched. See CrimsonSkiesFix_installer.log.");
                Ok(0)
            }
            Err(why)=>Err(format!("{why}; see {}",log.display())),
        };
    }
    if !args.is_empty() && args[0] == "--inspect-cd" {
        let selected=if args.len()==2 {Some(PathBuf::from(&args[1]))}else{None};
        // Explicit diagnostic scans always produce a log even with Log=0.
        // They never launch, install, patch or modify the disc image.
        let report=disc_inspect::inspect(base,selected.as_deref());
        let log=base.join(INSTALLER_LOG);
        if any_entry(&log) && !regular(&log) {
            return Err("unsafe CrimsonSkiesFix_installer.log: refusing to overwrite a non-file".into());
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
            vec![],vec!["--help"],vec!["--version"],vec!["--verify"],
            vec!["--install-only"],vec!["--launch"],
            vec!["--inspect-cd"],vec!["--inspect-cd","disc.iso"],
            vec!["--extract-cd"],vec!["--extract-cd","disc.cue"],
            vec!["--install-cd"],vec!["--install-cd","disc.cue"],
            vec!["--install-cd","disc.cue","patch.exe"],
            vec!["--verify-install"],vec!["--verify-install","FR"],
        ]{
            let args=flags.iter().map(OsString::from).collect::<Vec<_>>();
            assert!(supported_command(&args),"expected supported: {flags:?}");
        }
        for flags in [
            vec!["--import","original"],vec!["--audit-shortnames"],
            vec!["--audit-shortnames","FR"],vec!["--audit-shortnames","FR","ref.zbd"],
            vec!["--inspect-update"],vec!["--inspect-update","update.exe"],
            vec!["--extract-update","FR"],vec!["--extract-update","FR","update.exe"],
            vec!["--install-cd","disc.iso","patch.exe","--into","FR"],
            vec!["--install-only","disc.iso"],vec!["--launch","--verify"],
            vec!["--launch","missing.exe"],vec!["--verify","other"],
            vec!["--unknown"],
        ]{
            let args=flags.iter().map(OsString::from).collect::<Vec<_>>();
            assert!(!supported_command(&args),"unexpected command accepted: {flags:?}");
        }
    }
    #[test]
    fn manual_cd_extraction_prerequisites_fail_without_creating_staging(){
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

}
