//! Standalone bootstrap launcher for the owner's already-installed Crimson Skies 1.02.
//! No copyrighted game assets, executable or disc patches are distributed.
mod sha256;
mod disc_inspect;
mod update102;
mod managed_gos_patch;
mod chapter_cabs;
mod auto_install;
#[cfg(feature="native-usde-updater")]
mod error_dialog;
#[cfg(all(feature="winmm-split-install",not(feature="import-alias-install")))]
mod split_process_probe;
#[cfg(feature="import-alias-install")]
#[path="../../loader-winmm/src/bin/alias_import_launcher.rs"]
mod alias_import_launcher;

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
// The opt-in WinMM installer is compiled separately from the v1.0 DirectDraw
// installer, with a distinct bundled runtime and version identity.
const BUNDLED_RUNTIME: &[u8] = include_bytes!(env!("CRIMSONSKIESFIX_BUNDLED_DLL"));
#[cfg(feature="sdl3-probe-install")]
const BUNDLED_SDL3: &[u8] = include_bytes!(env!("CRIMSONSKIESFIX_SDL3_DLL"));
#[cfg(feature="sdl3-probe-install")]
const SDL3_LICENSE: &str = include_str!("../third_party/SDL3-LICENSE.txt");
#[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
const BUNDLED_CORE: &[u8] = include_bytes!(env!("CRIMSONSKIESFIX_BUNDLED_CORE_DLL"));
const RUNTIME_DLL: &str = if cfg!(feature="unified-fix-install") {"fix.dll"}
    else if cfg!(feature="import-alias-install") {"CSFIX.dll"}
    else if cfg!(feature="winmm-installer") {"winmm.dll"} else {"ddraw.dll"};
const GAME_FOLDER: &str = if cfg!(feature="unified-fix-install") {"game"}
    else if cfg!(feature="winmm-split-install") {"crimsonskies"} else {"CrimsonSkiesFix"};
const PATCH_FOLDER: &str = "crimsonskiesfix";
const CORE_DLL: &str = "CrimsonSkiesFix.dll";

fn patch_dir(game:&Path)->PathBuf{
    if cfg!(feature="unified-fix-install"){game.parent().unwrap_or(game).to_path_buf()}
    else if cfg!(feature="winmm-split-install"){game.join(PATCH_FOLDER)}else{game.to_path_buf()}
}
fn ensure_patch_dir(game:&Path)->Result<PathBuf,String>{
    let dir=patch_dir(game);
    if !directory(&dir){
        if any_entry(&dir){return Err(format!("unsafe game-fix directory: {}",dir.display()));}
        fs::create_dir(&dir).map_err(error)?;
    }
    Ok(dir)
}
// The unified layout keeps generated documentation out of the launcher root.
// Legacy build variants retain their existing layout unchanged.
fn docs_dir(game:&Path)->PathBuf{
    let parent=patch_dir(game);
    if cfg!(feature="unified-fix-install"){parent.join("docs")}else{parent}
}
fn ensure_docs_dir(game:&Path)->Result<PathBuf,String>{
    let parent=ensure_patch_dir(game)?;
    if !cfg!(feature="unified-fix-install"){return Ok(parent);}
    let dir=parent.join("docs");
    if !directory(&dir){
        if any_entry(&dir){return Err(format!("unsafe documentation directory: {}",dir.display()));}
        fs::create_dir(&dir).map_err(error)?;
    }
    Ok(dir)
}
#[cfg(feature="unified-fix-install")]
fn ensure_state_dir(game:&Path)->Result<PathBuf,String>{
    let parent=game.parent().ok_or("game directory has no parent")?;
    let state=parent.join("state");
    if !directory(&state){
        if any_entry(&state){return Err(format!("unsafe installation state directory: {}",state.display()));}
        fs::create_dir(&state).map_err(error)?;
    }
    Ok(state)
}
#[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
fn bundled_core_matches(game:&Path)->bool{
    let core=game.join(PATCH_FOLDER).join(CORE_DLL);
    regular(&core) && digest(&core).ok().as_deref()==Some(sha256::hex(BUNDLED_CORE).as_str())
}
#[cfg(any(not(feature="winmm-split-install"),feature="unified-fix-install"))]
fn bundled_core_matches(_game:&Path)->bool{true}

#[cfg(not(feature="winmm-installer"))]
const README: &str = include_str!("../../README.md");
#[cfg(all(feature="winmm-installer",not(feature="winmm-split-install")))]
const README: &str = include_str!("../README-winmm-candidate.md");
#[cfg(all(feature="winmm-split-install",not(feature="import-alias-install")))]
const README: &str = include_str!("../README-winmm-split.md");
#[cfg(all(feature="import-alias-install",not(feature="unified-fix-install")))]
const README: &str = include_str!("../README-import-alias.md");
#[cfg(all(feature="unified-fix-install",not(feature="native-fr-cd")))]
const README: &str = include_str!("../README-unified.md");
#[cfg(all(feature="native-fr-cd",not(feature="native-usde-updater")))]
const README: &str = include_str!("../README-native-fr.md");
#[cfg(feature="native-usde-updater")]
const README: &str = include_str!("../README-native-usde.md");
const EXAMPLE_INI: &str = include_str!("../../CrimsonSkiesFix/CrimsonSkiesFix.ini.example");
const LICENSE: &str = include_str!("../../../LICENSE");
const VERSION: &str = if cfg!(feature="sdl3-probe-install") {"1.2.0-dev.2"} else if cfg!(feature="winmm-installer") {"1.1.0-rc.1"} else {env!("CARGO_PKG_VERSION")};
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
    ensure_no_local_ddraw(game)?;
    #[cfg(feature="import-alias-install")]
    preflight_no_active_winmm(game)?;
    let proxy = game.join(RUNTIME_DLL);
    if !regular(&proxy) {
        return Err(format!("CrimsonSkiesFix {RUNTIME_DLL} missing or not a regular file"));
    }
    let expected = sha256::hex(BUNDLED_RUNTIME);
    let actual = digest(&proxy)?;
    if actual != expected {
        return Err(format!(
            "installed {RUNTIME_DLL} does not match this launcher: actual={actual} expected={expected}"
        ));
    }
    #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
    {
        let core=game.join(PATCH_FOLDER).join(CORE_DLL);
        if !regular(&core) || digest(&core)?!=sha256::hex(BUNDLED_CORE) {
            return Err("split runtime CrimsonSkiesFix.dll missing or hash mismatch".into());
        }
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

// Only launcher-managed metadata uses this helper. Never rewrite a matching
// file, and never follow a link or replace a directory.
fn ensure_managed_text(path: &Path, text: &str) -> Result<bool, String> {
    if any_entry(path) {
        if !regular(path) {
            return Err(format!("unsafe managed documentation path: {}", path.display()));
        }
        if fs::metadata(path).map_err(error)?.len() == text.len() as u64
            && fs::read(path).map_err(error)? == text.as_bytes() {
            return Ok(false);
        }
    }
    fs::write(path, text).map_err(error)?;
    Ok(true)
}

// Never overwrite the active INI. The proxy creates it on its first game launch.
fn deploy_documentation(game: &Path) -> Result<(), String> {
    // The unified installer stores documentation under docs/; launcher INI
    // and logs stay at the root. Existing owner data is never overwritten.
    let docs=ensure_docs_dir(game)?;
    // Do not create optional mods/ or third_party/ directories until a
    // feature actually needs them. Any existing user-owned paths are left alone.
    ensure_new_text(&docs.join("README.md"), README)?;
    ensure_new_text(&docs.join("LICENSE"), LICENSE)?;
    ensure_new_text(&docs.join("CrimsonSkiesFix.ini.example"), EXAMPLE_INI)?;
    let source = format!("https://github.com/legluondunet/CrimsonSkiesFix/tree/{COMMIT}");
    let mut build_info = format!(
        "Project=CrimsonSkiesFix\nVersion={VERSION}\nCommit={COMMIT}\n\
         LogIdentity=CrimsonSkiesFix {VERSION}-{} x86\nArchitecture=Windows-x86\n\
         DllSha256={}\nDllSizeBytes={}\nRuntimeDll={RUNTIME_DLL}\nSource={source}\n",
        &COMMIT[..8],
        sha256::hex(BUNDLED_RUNTIME),
        BUNDLED_RUNTIME.len()
    );
    #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
    build_info.push_str(&format!(
        "CoreDll={PATCH_FOLDER}/{CORE_DLL}\nCoreSha256={}\nCoreSizeBytes={}\nGameFolder={GAME_FOLDER}\n",
        sha256::hex(BUNDLED_CORE),BUNDLED_CORE.len()));
    #[cfg(feature="unified-fix-install")]
    build_info.push_str("GameFolder=game\nCoreDll=game/fix.dll\nFixFolder=.\n");
    #[cfg(feature="native-usde-updater")]
    build_info.push_str(&format!(
        "InstallerCargoLockSHA256={}\nRuntimeCargoLockSHA256={}\n",
        sha256::hex(include_bytes!("../Cargo.lock")),
        sha256::hex(include_bytes!("../../runtime-unified/Cargo.lock"))
    ));
    #[cfg(feature="native-usde-updater")]
    {
        const NOTICES:&str=include_str!(concat!(env!("OUT_DIR"),"/third-party-notices.txt"));
        let path=docs.join("THIRD-PARTY-NOTICES.txt");
        // Like build-info, this generated notice describes the current build.
        ensure_managed_text(&path,NOTICES)?;
        build_info.push_str(&format!("ThirdPartyNoticesSHA256={}\n",sha256::hex(NOTICES.as_bytes())));
    }
    #[cfg(feature="sdl3-probe-install")]
    {
        ensure_managed_text(&docs.join("SDL3-LICENSE.txt"), SDL3_LICENSE)?;
        build_info.push_str(&format!("SDL3Version=3.4.18\nSDL3Sha256={}\nInputMode=SDL3_EXPERIMENTAL_FLIGHT\n",sha256::hex(BUNDLED_SDL3)));
    }
    let manifest = docs.join("build-info.txt");
    // This manifest is launcher-managed provenance, not player configuration.
    ensure_managed_text(&manifest,&build_info).map(|_|())
}

// The experimental WinMM installer deliberately requires a clean graphics
// provider slot. In particular, it does not guess whether an existing ddraw.dll
// is our old proxy or a third-party wrapper. Never remove or overwrite it.
fn ensure_no_local_ddraw(game:&Path)->Result<(),String>{
    if cfg!(feature="winmm-installer") && !cfg!(feature="import-alias-install") && any_entry(&game.join("ddraw.dll")) {
        return Err("WinMM candidate: a local ddraw.dll already exists; leave it untouched, test on a separate copy without that DLL. Automatic migration and external wrappers are not yet enabled".into());
    }
    Ok(())
}

// The unified fix.dll is an installer-owned runtime, not user game data.
// Replace any different regular DLL transactionally, but do NOT retain it as
// fix.dll.bak. The displaced DLL exists only as a rollback slot until the
// authenticated bundled version is successfully published.
#[cfg(feature="unified-fix-install")]
fn deploy_unified_fix_without_backup(game:&Path)->Result<(),String>{
    let name="fix.dll";
    let active=game.join(name);
    let pending=game.join("fix.dll.installing");
    let displaced=game.join("fix.dll.replacing");
    if any_entry(&active)&&!regular(&active){
        return Err("unsafe existing fix.dll: refusing replacement of a non-regular file".into());
    }
    if any_entry(&pending)||any_entry(&displaced){
        return Err("interrupted fix.dll replacement: inspect .installing/.replacing before retrying".into());
    }
    let expected=sha256::hex(BUNDLED_RUNTIME);
    if regular(&active)&&digest(&active)?==expected{return Ok(());}
    // Do not remove an existing fix.dll before the new payload is fully
    // written, synced and hash-verified. No persistent .bak is created.
    {
        let mut file=OpenOptions::new().write(true).create_new(true)
            .open(&pending).map_err(error)?;
        if let Err(err)=file.write_all(BUNDLED_RUNTIME).and_then(|_|file.sync_all()){
            drop(file); // Required on Windows before deleting a failed staging file.
            let _=fs::remove_file(&pending);
            return Err(format!("cannot stage bundled fix.dll: {err}"));
        }
    }
    let staged=match digest(&pending){
        Ok(actual)=>actual,
        Err(err)=>{
            let _=fs::remove_file(&pending);
            return Err(format!("cannot validate staged fix.dll: {err}"));
        }
    };
    if staged!=expected{
        let _=fs::remove_file(&pending);
        return Err("staged fix.dll SHA-256 mismatch; original preserved".into());
    }
    let had_active=regular(&active);
    if had_active {
        if let Err(err)=fs::rename(&active,&displaced){
            let _=fs::remove_file(&pending);
            return Err(format!("cannot move previous fix.dll into temporary rollback slot: {err}"));
        }
    }
    if let Err(err)=fs::rename(&pending,&active){
        let rollback=if had_active{fs::rename(&displaced,&active).map_err(error)}
            else{Ok(())};
        let _=fs::remove_file(&pending);
        return Err(format!("cannot install bundled fix.dll: {err}; original restoration={rollback:?}"));
    }
    if had_active {
        // Once the verified new DLL is active, the old version serves no
        // purpose. If deletion is blocked, explicitly report this instead
        // of silently leaving a permanent old-runtime copy.
        fs::remove_file(&displaced)
            .map_err(|err|format!("bundled fix.dll installed, but could not discard temporary old version {}: {err}",displaced.display()))?;
    }
    Ok(())
}

// Stage the verified new DLL first, then rotate exactly one .bak.
// The former .bak is held temporarily for rollback and discarded only
// after the replacement DLL is successfully published. This intentionally
// accepts any pre-existing regular runtime DLL, regardless of provenance.
// If active and .bak both exist, the older .bak is replaced by active.
fn preflight_binary(dir:&Path,name:&str)->Result<(),String>{
    for filename in [name.to_owned(),format!("{name}.bak")]{
        let path=dir.join(&filename);
        if any_entry(&path)&&!regular(&path){
            return Err(format!("unsafe existing {filename}"));
        }
    }
    for filename in [format!("{name}.installing"),format!("{name}.bak.previous")]{
        if any_entry(&dir.join(filename)){
            return Err(format!("abandoned {name} transaction; inspect files manually"));
        }
    }
    Ok(())
}

// Stage and hash-verify a DLL, rotate just one active backup, and roll back
// both names if publishing fails. Shared by proxy and nested game-fix runtime.
fn deploy_binary(dir:&Path,name:&str,bundled:&[u8])->Result<(),String>{
    preflight_binary(dir,name)?;
    let active=dir.join(name);
    let pending=dir.join(format!("{name}.installing"));
    let backup=dir.join(format!("{name}.bak"));
    let old_backup=dir.join(format!("{name}.bak.previous"));
    let expected=sha256::hex(bundled);
    if regular(&active)&&digest(&active)?==expected{return Ok(());}
    if regular(&active) && cfg!(feature="winmm-installer"){
        println!("WinMM candidate: replacing existing {name}; previous active preserved as {name}.bak");
    }
    {
        let mut f=OpenOptions::new().write(true).create_new(true)
            .open(&pending).map_err(error)?;
        f.write_all(bundled).map_err(error)?;
        f.sync_all().map_err(error)?;
    }
    if digest(&pending)?!=expected{
        let _=fs::remove_file(&pending);
        return Err(format!("staged {name} SHA-256 mismatch"));
    }
    let backup_moved=if regular(&backup)&&regular(&active){
        if let Err(err)=fs::rename(&backup,&old_backup){
            let _=fs::remove_file(&pending);
            return Err(format!("cannot stage previous {name} backup: {err}"));
        }
        true
    }else{false};
    let active_moved=if regular(&active){
        if let Err(err)=fs::rename(&active,&backup){
            if backup_moved{let _=fs::rename(&old_backup,&backup);}
            let _=fs::remove_file(&pending);
            return Err(format!("cannot back up previous {name}: {err}"));
        }
        true
    }else{false};
    if let Err(err)=fs::rename(&pending,&active){
        if active_moved{let _=fs::rename(&backup,&active);}
        if backup_moved{let _=fs::rename(&old_backup,&backup);}
        let _=fs::remove_file(&pending);
        return Err(format!("cannot publish bundled {name}: {err}"));
    }
    if backup_moved{
        if let Err(err)=fs::remove_file(&old_backup){
            eprintln!("Warning: old backup retained as {}: {err}",old_backup.display());
        }
    }
    Ok(())
}
#[cfg(feature="import-alias-install")]
const LEGACY_SPLIT_WINMM_DIGESTS:[&str;3]=[
    // Previously distributed experimental test builds only. Unknown WinMM
    // providers (or user wrappers) must NEVER be renamed automatically.
    "58421e1e6299c524e87f912808eaae016cb603cf3f9d8da14a5ed755067f8193", // 8326a322
    "36a555555f9ebc2af9fa51c124714d03dd190ab8c54394ae8050289f2b118141", // b15b4181
    "69f8998b7eb45704f74dc724e5df718d4df70aa6fa3b81b1b4b746f64912838e", // 3c205897
];
#[cfg(feature="import-alias-install")]
fn preflight_legacy_winmm(game:&Path)->Result<(),String>{
    let path=game.join("winmm.dll");
    if !any_entry(&path){return Ok(())}
    if !regular(&path){return Err("unsafe app-local winmm.dll file; refusing alias migration".into())}
    let hash=digest(&path)?;
    if !LEGACY_SPLIT_WINMM_DIGESTS.contains(&hash.as_str()){
       return Err("unrecognized app-local winmm.dll; preserving third-party provider and refusing alias mode".into());
    }
    let destination=game.join("winmm.dll.before-alias.bak");
    if any_entry(&destination){
       return Err("existing winmm.dll.before-alias.bak must be preserved; cannot overwrite on migration".into());
    }
    Ok(())
}
#[cfg(feature="import-alias-install")]
fn migrate_legacy_winmm(game:&Path)->Result<(),String>{
    preflight_legacy_winmm(game)?;
    let old=game.join("winmm.dll");
    if regular(&old){
       fs::rename(&old,game.join("winmm.dll.before-alias.bak")).map_err(error)?;
       println!("IMPORT-ALIAS: managed experimental WinMM shim preserved as winmm.dll.before-alias.bak");
    }
    Ok(())
}
#[cfg(feature="import-alias-install")]
fn preflight_no_active_winmm(game:&Path)->Result<(),String>{
    if any_entry(&game.join("winmm.dll")){
        return Err("import alias mode requires native system WinMM; app-local winmm.dll still present".into())
    }
    Ok(())
}
fn deploy_runtime(game:&Path)->Result<(),String>{
    if cfg!(feature="winmm-installer"){ensure_no_local_ddraw(game)?;}
    #[cfg(feature="import-alias-install")]
    preflight_legacy_winmm(game)?;
    #[cfg(feature="unified-fix-install")]
    deploy_unified_fix_without_backup(game)?;
    #[cfg(not(feature="unified-fix-install"))]
    {
        #[cfg(feature="winmm-split-install")]
        {
            // Validate both transaction slots BEFORE touching either DLL.
            let patch=ensure_patch_dir(game)?;
            preflight_binary(game,RUNTIME_DLL)?;
            preflight_binary(&patch,CORE_DLL)?;
            deploy_binary(&patch,CORE_DLL,BUNDLED_CORE)?;
        }
        deploy_binary(game,RUNTIME_DLL,BUNDLED_RUNTIME)?;
    }
    #[cfg(feature="import-alias-install")]
    migrate_legacy_winmm(game)?;
    Ok(())
}

const INSTALL_RECEIPT: &str = "CrimsonSkiesFix.install-receipt.txt";
// Installer provenance belongs to the patch alongside build-info, never
// mixed into the original game's file tree in the organized variant.
fn receipt_path(game:&Path)->PathBuf{
    #[cfg(feature="unified-fix-install")]
    {
        // Only a published game/ stores the receipt next to the launcher.
        // A game.rebuilding/ candidate keeps its uncommitted receipt privately
        // until validation and the reversible directory publication succeed.
        if game.file_name().is_some_and(|name|name.to_string_lossy().eq_ignore_ascii_case("game")){
            return game.parent().unwrap_or(game).join("state").join(INSTALL_RECEIPT);
        }
        return game.join(INSTALL_RECEIPT);
    }
    #[cfg(not(feature="unified-fix-install"))]
    {patch_dir(game).join(INSTALL_RECEIPT)}
}

// The receipt records provenance, not proof: validate every original GAME/
// file and the official overlay independently before trusting the install.
fn installed_cd_edition(game:&Path)->Result<&'static str,String> {
    let file=receipt_path(game);
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
    ensure_no_local_ddraw(game)?;
    if !directory(game) {return Err("missing or unsafe installed game directory".into());}
    let edition=installed_cd_edition(game)?;
    let state=fs::read_to_string(receipt_path(game)).map_err(error)?;
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
    let receipt=fs::read_to_string(receipt_path(game)).map_err(error)?;
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
    let proxy=game.join(RUNTIME_DLL);
    if !regular(&proxy) || digest(&proxy)?!=sha256::hex(BUNDLED_RUNTIME) {
        return Err(format!("installed {RUNTIME_DLL} missing or differs from bundled verified DLL"));
    }
    let docs=docs_dir(game);
    if !regular(&docs.join("CrimsonSkiesFix.ini.example"))
        || !regular(&docs.join("build-info.txt")) {
        return Err("missing installed patch documentation/build information".into());
    }
    if !bundled_core_matches(game){
        return Err("missing or unverified nested CrimsonSkiesFix.dll".into());
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
fn install_from_cd(base:&Path,game:&Path,updater:Option<&update102::Update102Source>,
    expected_edition:Option<&str>)->Result<(),String> {
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
    println!("Building original game installation from verified CD; update source: {}...",
        if updater.is_some(){"authenticated official 1.02 updater"}else{"original 1.02 CD"});
    let extracted=disc_inspect::extract_for_install(base,None,&stage,expected_edition)?;
    println!("Original CD edition authenticated: {}.",extracted.edition);
    // Follow source→destination file table of the original Microsoft setup:
    // expand all eight chapter CABs with edition-specific long ZBD names
    // and delete the CABs only after full content verification.
    let chapters=chapter_cabs::expand(&stage)?;
    // Extraction has already SHA-verified every original file against the
    // exact corresponding edition manifest. The updater verifies again.
    match updater {
        Some(verified_update)=>{update102::apply_asset(&stage,extracted.edition,verified_update)?;}
        None=>{
            if !cfg!(feature="native-fr-cd") {return Err("missing authenticated updater".into());}
            update102::verify_original_cd102_asset(&stage,extracted.edition)?;
        }
    }
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
    deploy_runtime(&stage)?;
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
        if updater.is_some(){update102::PATCH_SHA256}else if extracted.edition=="C_SKIES_FR_001"{"NOT_REQUIRED_ORIGINAL_FR_102_CD"}else{"NOT_REQUIRED_ORIGINAL_EU_102_CD"},
        update102::ASSET_SHA256,
        finalized.members,finalized.zbd_files,finalized.payload_bytes,
        finalized.manifest_sha,finalized.short_name_candidates,
        extracted.retail_support_files,extracted.retail_support_bytes,renamed
    );
    ensure_new_text(&receipt_path(&stage),&receipt)?;
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
    #[cfg(feature="import-alias-install")]
    let code=alias_import_launcher::launch_child(&exe,u32::MAX)
        .map(|(code,_)|code as i32)
        .map_err(|e|format!("--launch-no-verify could not start import alias {}: {e}",exe.display()))?;
    #[cfg(not(feature="import-alias-install"))]
    let code={
        let exit=Command::new(&exe).current_dir(game).status()
          .map_err(|e|format!("--launch-no-verify could not start {}: {e}",exe.display()))?;
        exit.code().unwrap_or(2)
    };
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
    #[cfg(feature="native-usde-updater")]
    let sources=format!("\
        Supported original CDs already in version 1.02: CD image only.\n\
        Supported original CDs in version 1.0: CD image + official Microsoft 1.02 update (crmsk102.exe).\n\
        Place all required files beside CrimsonSkiesFix.exe. No separate decrypted EXE is needed.\n\
        {}\n\
        Extract crmsk102.exe if downloaded in an archive.\n\
        The launcher verifies and applies the update offline.\n\
        Blocking errors are printed to the terminal and shown in a native error dialog.\n\
        Set CRIMSONSKIESFIX_NONINTERACTIVE=1 for unattended execution.",
        update102::download_help());
    #[cfg(not(feature="native-usde-updater"))]
    let sources="Standard installs require a separately approved 1.02 EXE and authentic updater.\n\
        Experimental native CD support depends on this build's enabled features.";
    println!(
        "CrimsonSkiesFix {VERSION} ({})\n\
         Automatic installer / verifier / launcher for Crimson Skies 1.02.\n\
         \nUsage:\n\
         CrimsonSkiesFix.exe            verify, install from adjacent CD or repair, then play\n\
         CrimsonSkiesFix.exe --install-only   install/repair fully, do not launch the game\n\
         CrimsonSkiesFix.exe --launch-no-verify         start CRIMSON.EXE directly, no verification or repair\n\
         CrimsonSkiesFix.exe --verify         verify installed EXE and embedded {RUNTIME_DLL}\n\

         CrimsonSkiesFix.exe --inspect image.iso   inspect unsupported CD, separate inspect log\n\
         CrimsonSkiesFix.exe --inspect image.cue [track.bin|track.img]\n\
         CrimsonSkiesFix.exe --verbose    detailed diagnostics, can combine with a command\n\
         CrimsonSkiesFix.exe --version\n\
         CrimsonSkiesFix.exe --help (-h also accepted)\n\
         \nA verified unwrapped 1.02 CRIMSON.EXE and installed GOSDATA files are required.\n\
         CD GAME files and official 1.02 ROF can be installed; protected CD executables are preserved.\n\
         {sources}\n\
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
        if cfg!(feature="native-usde-updater"){
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=game/fix.dll NATIVE US/DE LOCAL UPDATE 1.02 EXPERIMENTAL", &COMMIT[..8]);
        }else if cfg!(feature="native-fr-cd"){
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=game/fix.dll NATIVE FR 1.02 EXPERIMENTAL", &COMMIT[..8]);
        }else if cfg!(feature="unified-fix-install"){
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=game/fix.dll native_winmm=yes EXPERIMENTAL UNIFIED", &COMMIT[..8]);
        }else if cfg!(feature="import-alias-install"){
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=CSFIX.dll+crimsonskiesfix/CrimsonSkiesFix.dll native_winmm=yes EXPERIMENTAL IMPORT ALIAS", &COMMIT[..8]);
        }else if cfg!(feature="winmm-split-install"){
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=winmm.dll+crimsonskiesfix/CrimsonSkiesFix.dll EXPERIMENTAL SPLIT", &COMMIT[..8]);
        }else if cfg!(feature="winmm-installer") {
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86 runtime=winmm.dll EXPERIMENTAL", &COMMIT[..8]);
        } else {
            println!("CrimsonSkiesFix {VERSION}-{} Windows-x86", &COMMIT[..8]);
        }
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
    let game = base.join(GAME_FOLDER);
    if verbose && !(args.len()==1 && args[0]=="--launch-no-verify") {
        println!("VERBOSE: launcher={} game={} installed_exe={} runtime_dll={}",
            launcher.display(),game.display(),
            regular(&game.join("CRIMSON.EXE")),regular(&game.join(RUNTIME_DLL)));
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
        if regular(&receipt_path(&game)){
            verify_installed_cd_assets(&game)?;
        }
        verify(&game)?;
        println!("Verified: approved game EXE, game data and {RUNTIME_DLL} patch.");
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
            // A detached GUI launch may have no writable console. Keep the
            // complete stderr diagnostic when possible, but do not let a
            // closed stderr pipe prevent the native error notification.
            let _=writeln!(std::io::stderr().lock(),"CrimsonSkiesFix: ERROR: {err}");
            #[cfg(feature="native-usde-updater")]
            error_dialog::show_unless_noninteractive(&err);
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_layout_keeps_single_dll_and_owner_files_in_place(){
        let root=std::env::temp_dir().join(format!(
            "crimson-unified-contract-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let game=root.join("game");
        let staged=root.join("game.rebuilding");
        fs::create_dir(&game).unwrap();
        fs::create_dir(&staged).unwrap();

        assert_eq!(GAME_FOLDER,"game");
        assert_eq!(RUNTIME_DLL,"fix.dll");
        assert_eq!(patch_dir(&game),root);
        assert_eq!(receipt_path(&game),root.join("state").join(INSTALL_RECEIPT));
        assert_eq!(receipt_path(&staged),staged.join(INSTALL_RECEIPT));
        let ini=root.join("CrimsonSkiesFix.ini");
        let gameplay_log=root.join("CrimsonSkiesFix.log");
        let installer_log=root.join(INSTALLER_LOG);
        fs::write(&ini,b"[Graphics]\nAspectRatio=4:3\n").unwrap();
        fs::write(&gameplay_log,b"owner gameplay log").unwrap();
        fs::write(&installer_log,b"owner installer log").unwrap();

        deploy_runtime(&game).unwrap();
        deploy_documentation(&game).unwrap();
        assert_eq!(digest(&game.join("fix.dll")).unwrap(),sha256::hex(BUNDLED_RUNTIME));
        for name in ["CSFIX.dll","CrimsonSkiesFix.dll","winmm.dll"] {
            assert!(!any_entry(&game.join(name)),"unexpected obsolete game DLL: {name}");
            assert!(!any_entry(&root.join(name)),"unexpected extra root DLL: {name}");
        }
        for name in ["CrimsonSkiesFix.ini.example","README.md","LICENSE","build-info.txt"]{
            assert!(regular(&root.join("docs").join(name)));
            assert!(!any_entry(&root.join(name)),"unexpected root document: {name}");
            assert!(!any_entry(&game.join(name)));
        }
        let info=fs::read_to_string(root.join("docs").join("build-info.txt")).unwrap();
        assert!(info.lines().any(|line|line==format!("Version={VERSION}")));
        assert!(info.lines().any(|line|line==format!("Commit={COMMIT}")));
        assert!(info.lines().any(|line|line==format!("DllSha256={}",sha256::hex(BUNDLED_RUNTIME))));
        #[cfg(feature="native-usde-updater")]
        {
            assert!(info.lines().any(|line|line==format!("InstallerCargoLockSHA256={}",sha256::hex(include_bytes!("../Cargo.lock")))));
            assert!(info.lines().any(|line|line==format!("RuntimeCargoLockSHA256={}",sha256::hex(include_bytes!("../../runtime-unified/Cargo.lock")))));
            let notices=fs::read(root.join("docs").join("THIRD-PARTY-NOTICES.txt")).unwrap();
            assert_eq!(notices,include_bytes!(concat!(env!("OUT_DIR"),"/third-party-notices.txt")));
            assert!(info.lines().any(|line|line==format!("ThirdPartyNoticesSHA256={}",sha256::hex(&notices))));
        }
        assert!(!any_entry(&root.join("state"))); // no CD receipt for manual import
        assert_eq!(fs::read(&ini).unwrap(),b"[Graphics]\nAspectRatio=4:3\n");
        assert_eq!(fs::read(&gameplay_log).unwrap(),b"owner gameplay log");
        assert_eq!(fs::read(&installer_log).unwrap(),b"owner installer log");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature="native-usde-updater")]
    #[test]
    fn managed_documentation_is_idempotent_and_restores_missing_notices(){
        let root=std::env::temp_dir().join(format!(
            "crimson-managed-docs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        deploy_documentation(&game).unwrap();
        let docs=docs_dir(&game);
        let notices=docs.join("THIRD-PARTY-NOTICES.txt");
        let manifest=docs.join("build-info.txt");
        let original=fs::read_to_string(&notices).unwrap();
        let info=fs::read_to_string(&manifest).unwrap();
        assert!(!ensure_managed_text(&notices,&original).unwrap());
        assert!(!ensure_managed_text(&manifest,&info).unwrap());
        fs::remove_file(&notices).unwrap();
        deploy_documentation(&game).unwrap();
        assert_eq!(fs::read_to_string(&notices).unwrap(),original);
        fs::write(&notices,"damaged").unwrap();
        deploy_documentation(&game).unwrap();
        assert_eq!(fs::read_to_string(&notices).unwrap(),original);
        assert_eq!(fs::read_to_string(&manifest).unwrap(),info);
        fs::remove_file(&notices).unwrap();
        fs::create_dir(&notices).unwrap();
        assert!(deploy_documentation(&game).is_err());
        assert!(directory(&notices));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_documentation_preserves_existing_optional_content(){
        let root=std::env::temp_dir().join(format!(
            "crimson-unified-optional-content-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        let mods=root.join("mods");
        fs::create_dir(&mods).unwrap();
        fs::write(mods.join("my-mod.txt"),b"owner mod").unwrap();
        // Even an unrelated user file under the future optional name must
        // not cause installation failure or be replaced by a directory.
        let third_party=root.join("third_party");
        fs::write(&third_party,b"owner data").unwrap();

        deploy_documentation(&game).unwrap();
        assert_eq!(fs::read(mods.join("my-mod.txt")).unwrap(),b"owner mod");
        assert_eq!(fs::read(&third_party).unwrap(),b"owner data");
        assert!(regular(&root.join("docs").join("README.md")));
        assert!(regular(&root.join("docs").join("build-info.txt")));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_documentation_refuses_occupied_docs_without_modifying_owner_file(){
        let root=std::env::temp_dir().join(format!(
            "crimson-unified-docs-blocker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        fs::write(root.join("docs"),b"owner file").unwrap();
        assert!(deploy_documentation(&game).is_err());
        assert_eq!(fs::read(root.join("docs")).unwrap(),b"owner file");
        assert!(!any_entry(&root.join("README.md")));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature="winmm-split-install")]
    #[test]
    fn organized_install_separates_both_dlls_and_documents_without_touching_logs(){
        let root=std::env::temp_dir().join(format!(
            "crimson-organized-layout-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let game=root.join(GAME_FOLDER);
        fs::create_dir(&game).unwrap();
        fs::write(root.join(INSTALLER_LOG),b"owned by installer").unwrap();
        deploy_runtime(&game).unwrap();
        deploy_documentation(&game).unwrap();
        assert!(regular(&game.join(RUNTIME_DLL)));
        assert_eq!(digest(&game.join(RUNTIME_DLL)).unwrap(),sha256::hex(BUNDLED_RUNTIME));
        if cfg!(feature="import-alias-install") {
            assert!(!any_entry(&game.join("winmm.dll")),
                "import-alias installer MUST NOT deploy WinMM proxy");
        }
        #[cfg(not(feature="unified-fix-install"))]
        {
            assert!(regular(&game.join(PATCH_FOLDER).join(CORE_DLL)));
            assert_eq!(digest(&game.join(PATCH_FOLDER).join(CORE_DLL)).unwrap(),sha256::hex(BUNDLED_CORE));
        }
        assert!(regular(&docs_dir(&game).join("CrimsonSkiesFix.ini.example")));
        assert!(regular(&docs_dir(&game).join("build-info.txt")));
        if cfg!(feature="unified-fix-install"){
            assert_eq!(GAME_FOLDER,"game");
            assert_eq!(RUNTIME_DLL,"fix.dll");
            assert!(!any_entry(&root.join("mods")));
            assert!(!any_entry(&root.join("third_party")));
            assert!(!any_entry(&root.join("CrimsonSkiesFix.dll")));
        }
        if !cfg!(feature="unified-fix-install"){
            assert!(!any_entry(&game.join("CrimsonSkiesFix.ini.example")));
        }
        assert!(!any_entry(&game.join(CORE_DLL)));
        assert_eq!(fs::read(root.join(INSTALLER_LOG)).unwrap(),b"owned by installer");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature="import-alias-install")]
    #[test]
    fn alias_mode_does_not_deploy_winmm_or_reject_independent_ddraw(){
        let root=std::env::temp_dir().join(format!(
            "crimson-alias-migration-test-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        assert_eq!(RUNTIME_DLL,if cfg!(feature="unified-fix-install"){"fix.dll"}else{"CSFIX.dll"});
        fs::write(root.join("ddraw.dll"),b"independent renderer").unwrap();
        deploy_runtime(&root).unwrap();
        assert!(regular(&root.join(RUNTIME_DLL)));
        assert!(!any_entry(&root.join("winmm.dll")));
        assert_eq!(fs::read(root.join("ddraw.dll")).unwrap(),b"independent renderer");
        fs::write(root.join("winmm.dll"),b"unknown user multimedia wrapper").unwrap();
        let before=digest(&root.join(RUNTIME_DLL)).unwrap();
        assert!(preflight_legacy_winmm(&root).is_err());
        assert!(deploy_runtime(&root).is_err());
        assert!(regular(&root.join("winmm.dll")));
        assert_eq!(digest(&root.join(RUNTIME_DLL)).unwrap(),before);
        fs::remove_dir_all(root).unwrap();
    }
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
        // Native media preflight now identifies unreadable media directly,
        // instead of asking for an unrelated decrypted executable.
        #[cfg(feature="native-fr-cd")]
        {
            assert!(err.contains("CD content inspection failed"),"{err}");
            assert!(!err.contains("approved decrypted Crimson Skies 1.02 EXE"),"{err}");
        }
        #[cfg(not(feature="native-fr-cd"))]
        assert!(err.contains("approved decrypted Crimson Skies 1.02 EXE"),"{err}");
        assert_eq!(fs::read(root.join("disc.iso")).unwrap(),b"fake authenticated source");
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
        assert!(!BUNDLED_RUNTIME.is_empty());
        assert_eq!(sha256::hex(BUNDLED_RUNTIME).len(), 64);
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn foreign_fix_dll_is_replaced_without_backup_and_second_install_is_noop(){
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let game=std::env::temp_dir().join(format!(
            "crimson-unified-replace-{}-{n}",std::process::id()));
        fs::create_dir(&game).unwrap();
        fs::write(game.join("fix.dll"),b"older or unknown version of fix.dll").unwrap();
        deploy_unified_fix_without_backup(&game).unwrap();
        assert_eq!(fs::read(game.join("fix.dll")).unwrap(),BUNDLED_RUNTIME);
        for name in ["fix.dll.bak","fix.dll.installing","fix.dll.replacing"]{
            assert!(!any_entry(&game.join(name)),"unexpected retained DLL: {name}");
        }
        let hash=digest(&game.join("fix.dll")).unwrap();
        deploy_unified_fix_without_backup(&game).unwrap();
        assert_eq!(digest(&game.join("fix.dll")).unwrap(),hash);
        fs::remove_dir_all(game).unwrap();
    }

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_replacement_does_not_touch_preexisting_unrelated_backup(){
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let game=std::env::temp_dir().join(format!(
            "crimson-unified-owner-backup-{}-{n}",std::process::id()));
        fs::create_dir(&game).unwrap();
        fs::write(game.join("fix.dll"),b"unrecognized previous runtime").unwrap();
        fs::write(game.join("fix.dll.bak"),b"pre-existing owner backup").unwrap();
        deploy_unified_fix_without_backup(&game).unwrap();
        assert_eq!(fs::read(game.join("fix.dll")).unwrap(),BUNDLED_RUNTIME);
        assert_eq!(fs::read(game.join("fix.dll.bak")).unwrap(),b"pre-existing owner backup");
        assert!(!any_entry(&game.join("fix.dll.replacing")));
        fs::remove_dir_all(game).unwrap();
    }

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unsafe_or_interrupted_unified_fix_never_replaces_active_dll(){
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let game=std::env::temp_dir().join(format!(
            "crimson-unified-blocked-{}-{n}",std::process::id()));
        fs::create_dir(&game).unwrap();
        let old=b"previous foreign runtime";
        fs::write(game.join("fix.dll"),old).unwrap();
        fs::write(game.join("fix.dll.replacing"),b"abandoned rollback slot").unwrap();
        assert!(deploy_unified_fix_without_backup(&game).is_err());
        assert_eq!(fs::read(game.join("fix.dll")).unwrap(),old);
        assert!(!any_entry(&game.join("fix.dll.installing")));
        fs::remove_file(game.join("fix.dll.replacing")).unwrap();
        fs::create_dir(game.join("fix.dll.installing")).unwrap();
        assert!(deploy_unified_fix_without_backup(&game).is_err());
        assert_eq!(fs::read(game.join("fix.dll")).unwrap(),old);
        fs::remove_dir_all(game).unwrap();
    }

    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unsafe_nonregular_fix_dll_is_never_deleted(){
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let game=std::env::temp_dir().join(format!(
            "crimson-unified-unsafe-{}-{n}",std::process::id()));
        fs::create_dir(&game).unwrap();
        fs::create_dir(game.join("fix.dll")).unwrap();
        fs::write(game.join("fix.dll").join("owner.txt"),b"do not touch").unwrap();
        assert!(deploy_unified_fix_without_backup(&game).is_err());
        assert_eq!(fs::read(game.join("fix.dll").join("owner.txt")).unwrap(),b"do not touch");
        assert!(!any_entry(&game.join("fix.dll.installing")));
        fs::remove_dir_all(game).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn standalone_winmm_install_is_non_destructive_and_idempotent() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let game=std::env::temp_dir().join(format!(
            "crimson-winmm-install-test-{}-{id}",std::process::id()));
        fs::create_dir(&game).unwrap();
        assert_eq!(RUNTIME_DLL,"winmm.dll");
        // The experimental candidate never creates or removes ddraw.dll.
        deploy_runtime(&game).unwrap();
        assert!(!any_entry(&game.join("ddraw.dll")));
        assert_eq!(digest(&game.join("winmm.dll")).unwrap(),
                   sha256::hex(BUNDLED_RUNTIME));
        deploy_runtime(&game).unwrap();
        assert!(!any_entry(&game.join("winmm.dll.bak")));
        assert!(!any_entry(&game.join("winmm.dll.installing")));
        let foreign=game.join("ddraw.dll");
        fs::write(&foreign,b"owner graphics wrapper").unwrap();
        let winmm_before=digest(&game.join("winmm.dll")).unwrap();
        assert!(deploy_runtime(&game).is_err());
        assert_eq!(fs::read(&foreign).unwrap(),b"owner graphics wrapper");
        assert_eq!(digest(&game.join("winmm.dll")).unwrap(),winmm_before);
        fs::remove_dir_all(&game).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn unknown_active_and_old_backup_are_replaced_with_one_rollback_copy() {
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!(
            "crimson-winmm-unconditional-{}-{n}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let active=dir.join("winmm.dll");
        let backup=dir.join("winmm.dll.bak");
        let unknown=b"unknown third-party active winmm";
        let old=b"older third-party backup";
        fs::write(&active,unknown).unwrap();
        fs::write(&backup,old).unwrap();
        deploy_runtime(&dir).unwrap();
        assert_eq!(fs::read(&active).unwrap(),BUNDLED_RUNTIME);
        assert_eq!(fs::read(&backup).unwrap(),unknown);
        assert!(!any_entry(&dir.join("winmm.dll.bak.previous")));
        assert!(!any_entry(&dir.join("winmm.dll.installing")));
        // Launching the exact same build again must not rotate .bak.
        deploy_runtime(&dir).unwrap();
        assert_eq!(fs::read(&active).unwrap(),BUNDLED_RUNTIME);
        assert_eq!(fs::read(&backup).unwrap(),unknown);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn unknown_active_without_backup_creates_one_rollback_copy() {
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!(
            "crimson-winmm-new-backup-{}-{n}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let prior=b"unrecognized winmm binary";
        fs::write(dir.join("winmm.dll"),prior).unwrap();
        deploy_runtime(&dir).unwrap();
        assert_eq!(fs::read(dir.join("winmm.dll")).unwrap(),BUNDLED_RUNTIME);
        assert_eq!(fs::read(dir.join("winmm.dll.bak")).unwrap(),prior);
        assert!(!any_entry(&dir.join("winmm.dll.bak.previous")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn no_active_dll_keeps_existing_backup_regardless_of_provenance() {
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!(
            "crimson-winmm-preserve-only-{}-{n}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let old=b"sole backup without active DLL";
        fs::write(dir.join("winmm.dll.bak"),old).unwrap();
        deploy_runtime(&dir).unwrap();
        assert_eq!(fs::read(dir.join("winmm.dll")).unwrap(),BUNDLED_RUNTIME);
        assert_eq!(fs::read(dir.join("winmm.dll.bak")).unwrap(),old);
        assert!(!any_entry(&dir.join("winmm.dll.bak.previous")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn unsafe_backup_path_still_fails_without_touching_active_dll() {
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!(
            "crimson-winmm-unsafe-backup-{}-{n}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let old=b"unrecognized active DLL";
        fs::write(dir.join("winmm.dll"),old).unwrap();
        fs::create_dir(dir.join("winmm.dll.bak")).unwrap();
        assert!(deploy_runtime(&dir).is_err());
        assert_eq!(fs::read(dir.join("winmm.dll")).unwrap(),old);
        assert!(!any_entry(&dir.join("winmm.dll.installing")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(all(feature="winmm-installer",not(feature="import-alias-install")))]
    #[test]
    fn abandoned_rotation_marker_blocks_update_without_data_loss() {
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!(
            "crimson-winmm-abandoned-{}-{n}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("winmm.dll"),b"active").unwrap();
        fs::write(dir.join("winmm.dll.bak"),b"backup").unwrap();
        fs::write(dir.join("winmm.dll.bak.previous"),b"interrupted").unwrap();
        assert!(deploy_runtime(&dir).is_err());
        assert_eq!(fs::read(dir.join("winmm.dll")).unwrap(),b"active");
        assert_eq!(fs::read(dir.join("winmm.dll.bak")).unwrap(),b"backup");
        assert_eq!(fs::read(dir.join("winmm.dll.bak.previous")).unwrap(),b"interrupted");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn installed_cd_requires_receipt_and_never_treats_empty_data_as_installed() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("crimson-cd-install-check-{}-{id}",std::process::id()));
        fs::create_dir(&root).unwrap();
        assert!(verify_installed_cd_assets(&root).is_err());
        #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
        fs::create_dir(patch_dir(&root)).unwrap();
        fs::write(receipt_path(&root),
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
        #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
        fs::create_dir(patch_dir(&dir)).unwrap();
        let receipt=receipt_path(&dir);
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
        assert!(install_from_cd(&root,&dest,Some(&update102::Update102Source::OwnerFile(root.join("update102.exe"))),None).is_err());
        assert_eq!(fs::read(dest.join("user-save.dat")).unwrap(),b"keep");
        fs::remove_dir_all(root).unwrap();
    }

}
