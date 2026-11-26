//! No-argument verified CD installer / repair / launcher.
//! All game bytes originate from the owner's exact verified original disc,
//! official 1.02 updater and local hash-pinned decrypted executable.
//! Never redistribute them, and never overwrite a damaged install in place.
use super::*;
use std::ffi::OsStr;

const REBUILT:&str=if cfg!(feature="unified-fix-install"){"game.rebuilding"}
    else if cfg!(feature="winmm-split-install"){"crimsonskies.rebuilding"}else{"CrimsonSkiesFix.rebuilding"};
const PREVIOUS:&str=if cfg!(feature="unified-fix-install"){"game.previous"}
    else if cfg!(feature="winmm-split-install"){"crimsonskies.previous"}else{"CrimsonSkiesFix.previous"};
const EXE_BACKUP:&str="CRIMSON.EXE.protected.bak";
const EXE_PENDING:&str="CRIMSON.EXE.approved.installing";
// The installer and game use independent logs. The last installation or
// repair report survives routine verified game launches; the DLL always
// starts a fresh CrimsonSkiesFix.log for each game session.
const OLD_AUTO_LOG:&str="CrimsonSkiesFix.installer.log";
const PREVIOUS_INSTALLER_LOG:&str="CrimsonSkiesFix-installer.log";

/// Recognize only a previous launcher-generated report before migrating or
/// retiring it. Unknown user files, symlinks and directories are preserved.
fn previous_installer_report(path:&Path)->bool{
    if !regular(path){return false;}
    let Ok(meta)=fs::metadata(path) else{return false;};
    if meta.len()>128*1024{return false;}
    let Ok(bytes)=fs::read(path) else{return false;};
    [
        "CrimsonSkiesFix AUTO INSTALL v1\n",
        "CrimsonSkiesFix CHAPTER SHORT NAMES AUDIT v1\n",
        "CrimsonSkiesFix CD INSTALL VERIFY v1\n",
        "CrimsonSkiesFix CD INSTALL v1\n",
        "CrimsonSkiesFix UPDATE102 EXTRACT v1\n",
        "CrimsonSkiesFix CD EXTRACT v1\n",
        "CrimsonSkiesFix UPDATE102 INSPECT v1\n",
        "CrimsonSkiesFix CD INSPECT v1\n",
    ].iter().any(|prefix|bytes.starts_with(prefix.as_bytes()))
}

/// Preserve the previous diagnostic across a verified-only game launch
/// after adopting the new spelling. Never replace an existing report.
pub(super) fn migrate_previous_installer_log(base:&Path){
    let target=base.join(INSTALLER_LOG);
    let previous=base.join(PREVIOUS_INSTALLER_LOG);
    if !any_entry(&target) && previous_installer_report(&previous){
        let _=fs::rename(previous,target);
    }
}


/// Routine verified launches do not replace an installer/repair diagnosis.
/// A first install, repair, activation, or any failed preparation does.
fn report_needed(action:Option<&str>)->bool{
    !matches!(action,Some("VERIFIED_EXISTING_AND_LAUNCH"|"VERIFIED_LEGACY_IMPORT_AND_LAUNCH"))
}

/// Write only installer-owned records; never touch the DLL's gameplay log.
/// A failed or real installation replaces the previous installer report.
fn write_installer_report(base:&Path,report:&str,needed:bool)->Result<(),String>{
    if !needed{return Ok(());}
    let target=base.join(INSTALLER_LOG);
    if any_entry(&target) && !regular(&target){
        return Err("unsafe CrimsonSkiesFix_installer.log: refusing non-regular file".into());
    }
    fs::write(&target,report).map_err(error)?;
    remove_old_installer_log(base);
    Ok(())
}

/// Preserve the prior installation report on ordinary successful launches.
/// A real installation/repair or nonzero child exit produces one bounded
/// report, not an accumulated history. Diagnostics never change exit codes.
fn record_game_exit(base:&Path,report:&str,needed:bool,summary:Option<&str>,code:i32)->Result<(),String>{
    if !needed && code==0{return Ok(());}
    let mut completed=report.to_owned();
    if let Some(summary)=summary{
        completed.push_str(summary);
        completed.push('\n');
    }
    completed.push_str(&format!("\nCrimsonSkiesFix AUTO INSTALL: GAME_EXIT_CODE={code}\nGameExitCodeHex=0x{:08X}\n",code as u32));
    write_installer_report(base,&completed,true)
}

/// Retire generated old spellings only after a new report was written.
/// Preserve unknown files or owner data at any old spelling.
fn remove_old_installer_log(base:&Path){
    for filename in [OLD_AUTO_LOG,PREVIOUS_INSTALLER_LOG]{
        let old=base.join(filename);
        if previous_installer_report(&old){
            let _=fs::remove_file(old);
        }
    }
}


/// Discover an external executable by its *full byte identity*, not its
/// basename, because owners legitimately rename their decrypted file.
fn approved_executable(base:&Path,existing:&Path)->Result<PathBuf,String>{
    // A corrupt/missing ROF must not prevent reuse of a perfectly valid
    // approved EXE that was already installed. Check the EXE's bytes
    // separately from the game's asset verifier during repair preflight.
    let mut installed_valid=Vec::<PathBuf>::new();
    for name in ["CRIMSON.EXE","CRIMSON-UNWRAPPED.EXE"] {
        let candidate=existing.join(name);
        if regular(&candidate)
            && fs::metadata(&candidate).map_err(error)?.len()==GAME_EXE_SIZE
            && digest(&candidate)?==GAME_EXE_HASH {
            installed_valid.push(candidate);
        }
    }
    if installed_valid.len()>1 {
        return Err("ambiguous approved game EXEs in existing installation".into());
    }
    if let Some(exe)=installed_valid.pop(){
        // A protected-pending install can be activated only with an EXE
        // outside its game directory: keeping an extra alternate candidate
        // there would make validated_game_exe reject the final directory.
        let pending=fs::read_to_string(receipt_path(existing))
            .map(|s|s.lines().any(|l|
                l=="InstallState=GAME_FILES_READY_EXECUTABLE_PENDING"))
            .unwrap_or(false);
        if !pending || exe.file_name().is_some_and(|name|name==OsStr::new("CRIMSON.EXE")) {
            return Ok(exe);
        }
    }
    let mut valid=Vec::<PathBuf>::new();
    for entry in fs::read_dir(base).map_err(error)?{
        let entry=entry.map_err(error)?;
        let candidate=entry.path();
        // Do not read unknown full files, or follow links/junctions.
        if !regular(&candidate) {continue;}
        if candidate.extension().and_then(OsStr::to_str)
            .map(|x|!x.eq_ignore_ascii_case("exe")).unwrap_or(true) {continue;}
        if fs::metadata(&candidate).map_err(error)?.len()!=GAME_EXE_SIZE {continue;}
        if digest(&candidate)?==GAME_EXE_HASH {valid.push(candidate);}
    }
    match valid.len(){
        1=>Ok(valid.remove(0)),
        0=>Err(format!(
            "approved decrypted Crimson Skies 1.02 EXE not found beside launcher; \
             provide a locally owned {}-byte EXE with SHA-256 {} \
             (the original protected CD EXE is never run)",
            GAME_EXE_SIZE,GAME_EXE_HASH)),
        n=>Err(format!("{n} identical approved decrypted EXE candidates beside launcher; keep exactly one")),
    }
}

/// Full installation prerequisite gate. Every source must be available
/// and authenticated *before* a CD extraction transaction is started.
/// The patch is embedded in the single-file launcher, not a separately
/// distributed DLL. It must exist and be a plausible Windows PE image;
/// CI independently verifies its entire 22-export PE32 layout.
/// Precisely one installation input plan. Unknown discs continue to require
/// the original explicit, hash-pinned updater + approved EXE sources.
#[derive(Debug)]
pub(super) enum InstallSources {
    Approved { executable:PathBuf, updater:update102::Update102Source },
    #[cfg(feature="native-fr-cd")]
    NativeCd102{edition:&'static str},
    #[cfg(feature="native-usde-updater")]
    NativeWithUpdater{edition:&'static str,updater:update102::Update102Source},
}

/// Authenticate the CD once. Do not mask inspection failures as missing
/// EXE/updater prerequisites or trust a filename or volume label.
#[cfg(feature="native-fr-cd")]
fn authenticated_cd_edition(report:&str)->Result<&'static str,String>{
    let edition=report.lines().filter_map(|line|line.strip_prefix("Edition="))
        .collect::<Vec<_>>();
    if edition.len()!=1 || !report.lines().any(|line|
        line=="EditionRecognition=EXACT_INTERNAL_FILE_MANIFEST"){
        return Err("CD content does not match a supported exact internal file manifest; run --inspect with the image filename and provide CrimsonSkiesFix_inspect.log".into());
    }
    match edition[0] {
        "C_SKIES_FR_001"=>Ok("C_SKIES_FR_001"),
        "C_SKIES_FR_002"=>Ok("C_SKIES_FR_002"),
        "C_SKIES_DISC_001"=>Ok("C_SKIES_DISC_001"),
        "C_SKIES_DISC_002"=>Ok("C_SKIES_DISC_002"),
        "C_SKIES_DE_001"=>Ok("C_SKIES_DE_001"),
        _=>Err("unrecognized authenticated CD edition".into()),
    }
}

#[cfg(feature="native-usde-updater")]
fn earlier_cd_update_source(base:&Path,update:Option<&Path>,edition:&str)
    ->Result<update102::Update102Source,String>{
    update102::source_for_install(base,update)
        .map_err(|e|format!("Recognized CD {edition}: official 1.02 updater required: {e}. No separate decrypted EXE is needed; no extraction attempted."))
}

pub(super) fn preflight_install_sources(
    base:&Path,existing:&Path,update:Option<&Path>,
)->Result<InstallSources,String>{
    // No staging or modification until the bundled runtime and chosen input
    // sources are authenticated. CD inspection reads all original manifest
    // entries for native FR, rather than trusting the CD name.
    if BUNDLED_RUNTIME.len()<4096 || !BUNDLED_RUNTIME.starts_with(b"MZ"){
        return Err(format!(
            "embedded CrimsonSkiesFix {RUNTIME_DLL} patch is missing or invalid; extraction refused"
        ));
    }
    let disc=disc_inspect::preflight_media(base);
    // All five profiles in the consolidated native installer can derive
    // the approved EXE without a separately supplied decrypted file.
    // Until a CD is identified, its updater requirement is unknown.
    #[cfg(feature="native-usde-updater")]
    if let Err(detail)=&disc{
        return Err(format!("Missing or invalid original game CD image (.iso or .cue + .bin/.img): {detail}. Place exactly one supported CD image beside CrimsonSkiesFix.exe; no extraction or game launch attempted."));
    }
    #[cfg(feature="native-fr-cd")]
    if disc.is_ok() {
        let report=disc_inspect::inspect(base,None)
            .map_err(|e|format!("CD content inspection failed; no extraction attempted: {e}. Run --inspect with the image filename."))?;
        let edition=authenticated_cd_edition(&report)?;
        // The native CD path never needs the external updater, even if a
        // caller passes one. No fallback to the legacy decrypted-EXE route.
        if matches!(edition,"C_SKIES_FR_001"|"C_SKIES_DISC_002"){
            return Ok(InstallSources::NativeCd102{edition});
        }
        #[cfg(feature="native-usde-updater")]
        if matches!(edition,"C_SKIES_DISC_001"|"C_SKIES_DE_001"|"C_SKIES_FR_002"){
            let updater=earlier_cd_update_source(base,update,edition)?;
            return Ok(InstallSources::NativeWithUpdater{edition,updater});
        }
    }

    let updater=update102::source_for_install(base,update);
    let exe=approved_executable(base,existing);
    let mut problems=Vec::new();
    if let Err(detail)=&disc{
        problems.push(format!(
            "- original game CD image (.iso or .cue + .bin/.img): {detail}"
        ));
    }
    if let Err(detail)=&updater{
        problems.push(format!("- official 1.02 updater: {detail}"));
    }
    if let Err(detail)=&exe{
        problems.push(format!("- approved decrypted 1.02 EXE: {detail}"));
    }
    if !problems.is_empty(){
        return Err(format!(
            "Missing or invalid installation prerequisites; no extraction or game launch attempted:\n{}\n\
             Place the missing files beside CrimsonSkiesFix.exe, then rerun --install-only or launch normally. \
             Existing game files and saves are preserved.",
            problems.join("\n")
        ));
    }
    Ok(InstallSources::Approved{executable:exe?,updater:updater?})
}

/// Transactionally replace the original CD-protected entry *inside a newly
/// staged directory*, preserving its original bytes under a deterministic
/// backup name. The immutable CD-manifest verifier checks that backup.
fn activate_verified_exe(game:&Path,source:&Path)->Result<(),String>{
    if !directory(game){return Err("unsafe game stage for approved EXE activation".into());}
    if !regular(source) || fs::metadata(source).map_err(error)?.len()!=GAME_EXE_SIZE
        || digest(source)?!=GAME_EXE_HASH {
        return Err("decrypted EXE did not match the only approved 1.02 size/SHA-256".into());
    }
    let receipt=receipt_path(game);
    let raw=fs::read_to_string(&receipt).map_err(error)?;
    if !raw.lines().any(|l|l=="InstallState=GAME_FILES_READY_EXECUTABLE_PENDING"){
        return Err("cannot activate an installation that is not verified protected/pending".into());
    }
    if raw.lines().filter(|l|l.starts_with("InstallState=")).count()!=1{
        return Err("ambiguous installation state in receipt".into());
    }
    let active=game.join("CRIMSON.EXE");
    let backup=game.join(EXE_BACKUP);
    let pending=game.join(EXE_PENDING);
    if !regular(&active) || any_entry(&backup) || any_entry(&pending)
        || any_entry(&game.join("CRIMSON-UNWRAPPED.EXE")){
        return Err("unsafe protected EXE activation target or abandoned transaction".into());
    }
    // The original protected EXE must match the authentic CD manifest
    // immediately before backing it up. No unknown source is ever replaced.
    verify_installed_cd_assets(game)?;
    fs::copy(source,&pending).map_err(error)?;
    if digest(&pending)?!=GAME_EXE_HASH{
        let _=fs::remove_file(&pending);
        return Err("approved EXE copy failed its SHA-256 check".into());
    }
    // Construct next receipt before any renames: this function is used only
    // on a private reconstruction or on a fully verified pending install.
    let new_receipt=raw
        .replace("InstallState=GAME_FILES_READY_EXECUTABLE_PENDING","InstallState=GAME_READY")
        .replace("ExecutablePolicy=ORIGINAL_PROTECTED_PRESERVED",
                 "ExecutablePolicy=APPROVED_UNWRAPPED_102_ORIGINAL_BACKED_UP");
    let receipt_parent=receipt.parent().ok_or("receipt has no parent")?;
    let receipt_pending=receipt_parent.join("CrimsonSkiesFix.install-receipt.ready");
    if any_entry(&receipt_pending){
        let _=fs::remove_file(&pending);
        return Err("abandoned ready receipt exists; refusing to replace it".into());
    }
    if let Err(err)=fs::write(&receipt_pending,new_receipt) {
        let _=fs::remove_file(&pending);
        return Err(format!("cannot stage ready installation receipt: {err}"));
    }
    if let Err(err)=fs::rename(&active,&backup){
        let _=fs::remove_file(&pending);
        let _=fs::remove_file(&receipt_pending);
        return Err(format!("cannot save original protected EXE: {err}"));
    }
    if let Err(err)=fs::rename(&pending,&active){
        let _=fs::rename(&backup,&active);
        let _=fs::remove_file(&pending);
        let _=fs::remove_file(&receipt_pending);
        return Err(format!("cannot activate verified 1.02 EXE: {err}"));
    }
    // Windows does not always support replacing an existing file using
    // rename. Keep a reversible receipt backup until completed.
    let receipt_backup=receipt_parent.join("CrimsonSkiesFix.install-receipt.pending.bak");
    if any_entry(&receipt_backup){
        return Err("abandoned pending receipt backup; leaving staged data for inspection".into());
    }
    fs::rename(&receipt,&receipt_backup).map_err(error)?;
    if let Err(err)=fs::rename(&receipt_pending,&receipt){
        let _=fs::rename(&receipt_backup,&receipt);
        return Err(format!("cannot publish ready installation receipt: {err}"));
    }
    // Validate both *different* EXE identities (protected backup matches
    // original CD, active matches hash-pinned decrypted 1.02 binary),
    // every retail file, patch ROF and the canonical chapter manifest.
    verify_installed_cd_assets(game)?;
    verify(game)?;
    fs::remove_file(&receipt_backup).map_err(error)?;
    Ok(())
}

/// Activate a verified, locally reconstructed French 1.02 EXE, or retain the
/// pre-existing exact-hash import policy for other retail editions. This
/// produces a sibling temporary file, never an extra DLL or executable for
/// users to locate. Do not modify an original protected CD file.
/// Reconstruct the approved 1.02 executable from the owner's authentic
/// Microsoft update CAB, processed entirely in memory. No protected updater
/// or original SafeDisc source file is written as an intermediate artifact.
/// The only temporary game EXE is SHA-pinned before activation and deleted.
#[cfg(feature="native-usde-updater")]
fn activate_native_usde_from_update(
    base:&Path,game:&Path,updater:&update102::Update102Source,
)->Result<(),String>{
    let edition=installed_cd_edition(game)?;
    if !matches!(edition,"C_SKIES_DISC_001"|"C_SKIES_DE_001"|"C_SKIES_FR_002"){
        return Err("native updater reconstruction requires an approved earlier retail CD".into());
    }
    verify_installed_cd_assets(game)?;
    let protected=update102::read_protected_102_sources_from(updater)?;
    let image=unsafedisk_native::reconstruct_verified_crimson_102_bytes(
        &protected.loader,&protected.icd
    ).map_err(|e|format!("native 1.02 reconstruction from owner updater failed: {e}"))?;
    if image.len()!=GAME_EXE_SIZE as usize || sha256::hex(&image)!=GAME_EXE_HASH{
        return Err("native updater reconstruction produced unexpected game EXE hash".into());
    }
    let temporary=base.join("CrimsonSkiesFix.native-usde-102.pending.exe");
    if any_entry(&temporary){
        return Err(format!(
            "abandoned native US/DE candidate exists; never overwriting: {}",
            temporary.display()));
    }
    let mut file=OpenOptions::new().write(true).create_new(true)
        .open(&temporary).map_err(error)?;
    if let Err(err)=file.write_all(&image).and_then(|_|file.sync_all()){
        drop(file);
        let _=fs::remove_file(&temporary);
        return Err(format!("could not securely write reconstructed 1.02 candidate: {err}"));
    }
    drop(file);
    struct PrivateCandidate(PathBuf);
    impl Drop for PrivateCandidate{
        fn drop(&mut self){
            if regular(&self.0){let _=fs::remove_file(&self.0);}
        }
    }
    let _cleanup=PrivateCandidate(temporary.clone());
    activate_verified_exe(game,&temporary)
}

fn activate_pending_exe(base:&Path,game:&Path)->Result<(),String>{
    #[cfg(feature="native-fr-cd")]
    if matches!(installed_cd_edition(game)?,"C_SKIES_FR_001"|"C_SKIES_DISC_002") {
        // The complete CD receipt and original protected EXE/ICD must verify
        // BEFORE any disk write. An interrupted candidate is never replaced.
        verify_installed_cd_assets(game)?;
        let temporary=base.join("CrimsonSkiesFix.native-fr-102.pending.exe");
        if any_entry(&temporary){
            return Err(format!(
                "abandoned native CD 1.02 reconstruction exists (preserved, not overwritten): {}",
                temporary.display()));
        }
        unsafedisk_native::reconstruct_verified_crimson_102(
            &game.join("CRIMSON.EXE"),&game.join("CRIMSON.ICD"),&temporary
        ).map_err(|err|format!("native French 1.02 reconstruction failed: {err}"))?;
        // The native library has already validated full 1.02 SHA-256 before
        // writing; activation performs an independent on-disk SHA-256 check.
        struct TemporaryExecutable(PathBuf);
        impl Drop for TemporaryExecutable{
            fn drop(&mut self){
                if regular(&self.0){let _=fs::remove_file(&self.0);}
            }
        }
        let _cleanup=TemporaryExecutable(temporary.clone());
        return activate_verified_exe(game,&temporary);
    }
    #[cfg(feature="native-usde-updater")]
    {
        let edition=installed_cd_edition(game)?;
        if matches!(edition,"C_SKIES_DISC_001"|"C_SKIES_DE_001"|"C_SKIES_FR_002"){
            // A missing or ambiguous updater on a resumed install must not
            // turn into an unrelated request for a decrypted executable.
            let updater=earlier_cd_update_source(base,None,edition)?;
            return activate_native_usde_from_update(base,game,&updater);
        }
    }
    let source=approved_executable(base,game)?;
    activate_verified_exe(game,&source)
}

fn copy_player_files(src:&Path,dst:&Path)->Result<(),String>{
    if !directory(src)||!directory(dst){
        return Err("unsafe saved-data directory; repair refused".into());
    }
    for entry in fs::read_dir(src).map_err(error)?{
        let entry=entry.map_err(error)?;
        let from=entry.path();
        let to=dst.join(entry.file_name());
        if any_entry(&to){return Err(format!("player file would be overwritten: {}",to.display()));}
        let ft=entry.file_type().map_err(error)?;
        if ft.is_symlink(){return Err(format!("player data link refused: {}",from.display()));}
        if ft.is_dir(){
            fs::create_dir(&to).map_err(error)?;
            copy_player_files(&from,&to)?;
        }else if ft.is_file(){
            fs::copy(&from,&to).map_err(error)?;
        }else{return Err(format!("unsupported player file type: {}",from.display()));}
    }
    Ok(())
}

fn preserve_player_data(old:&Path,new:&Path)->Result<(),String>{
    for folder in ["SavedGames","Planes"] {
        let from=old.join(folder);
        if any_entry(&from){
            copy_player_files(&from,&new.join(folder))?;
        }
    }
    let active=patch_dir(old).join("CrimsonSkiesFix.ini");
    // Preserve a legacy flat game-root INI if this is the first organized
    // repair; the original remains in the old game directory.
    let old_flat=old.join("CrimsonSkiesFix.ini");
    let source=if regular(&active){Some(active)}else if regular(&old_flat){Some(old_flat)}else{None};
    if let Some(ini)=source{
        let dest_dir=ensure_patch_dir(new)?;
        let dest=dest_dir.join("CrimsonSkiesFix.ini");
        // The unified launcher keeps the original INI at the shared root.
        // A repair must not attempt to copy that file onto itself.
        if ini==dest{return Ok(());}
        if any_entry(&dest){return Err("refusing to overwrite active player INI".into());}
        fs::copy(ini,dest).map_err(error)?;
    }
    Ok(())
}

// A candidate reconstructed from the original CD owns its receipt only while
// it resides in game.rebuilding/. Publish the receipt at the launcher root
// together with the game directory, preserving the previous record for rollback.
#[cfg(feature="unified-fix-install")]
fn previous_receipt(game:&Path)->Result<PathBuf,String>{
    Ok(game.parent().ok_or("game directory has no parent")?
        .join("state").join("CrimsonSkiesFix.install-receipt.previous"))
}
#[cfg(feature="unified-fix-install")]
fn publish_unified_receipt(game:&Path)->Result<bool,String>{
    let private=game.join(INSTALL_RECEIPT);
    let active=receipt_path(game);
    let previous=previous_receipt(game)?;
    if !regular(&private){return Err("rebuilt game has no private installation receipt".into());}
    if any_entry(&previous){return Err(format!(
        "previous installation receipt backup exists; preserving {}",previous.display()));}
    if any_entry(&active)&&!regular(&active){
        return Err("unsafe existing state installation receipt".into());
    }
    ensure_state_dir(game)?; // refuse symlinks or occupied files before any rename
    let had_previous=regular(&active);
    if had_previous{fs::rename(&active,&previous).map_err(error)?;}
    if let Err(e)=fs::rename(&private,&active){
        if had_previous {
            fs::rename(&previous,&active).map_err(|restore|format!(
                "cannot publish receipt: {e}; previous receipt restore failed: {restore}"))?;
        }
        return Err(format!("cannot publish verified CD receipt: {e}"));
    }
    Ok(had_previous)
}
#[cfg(feature="unified-fix-install")]
fn rollback_unified_receipt(game:&Path,had_previous:bool)->Result<(),String>{
    let active=receipt_path(game);
    fs::rename(&active,game.join(INSTALL_RECEIPT)).map_err(error)?;
    if had_previous{
        fs::rename(previous_receipt(game)?,active).map_err(error)?;
    }
    Ok(())
}
#[cfg(feature="unified-fix-install")]
fn finish_unified_receipt(game:&Path,had_previous:bool)->Result<(),String>{
    if had_previous {fs::remove_file(previous_receipt(game)?).map_err(error)?;}
    Ok(())
}

/// Build a completely verified replacement alongside the old game, then
/// perform a reversible directory swap. The old install, including any
/// unknown/modded files, is left intact under CrimsonSkiesFix.previous.
fn build_and_publish(base:&Path,game:&Path,old_present:bool)->Result<(),String>{
    let new=base.join(REBUILT);
    let previous=base.join(PREVIOUS);
    // Staging folders may hold player files or a previous partially verified
    // install. Identify the blocking pathname but NEVER remove/replace it.
    if any_entry(&new){
        return Err(format!(
            "rebuild blocked by existing {} (preserved; inspect before any manual action)",
            new.display()));
    }
    let pending=new.with_file_name(format!("{REBUILT}.installing"));
    if any_entry(&pending){
        return Err(format!(
            "rebuild blocked by existing {} (preserved; inspect before any manual action)",
            pending.display()));
    }
    if old_present && any_entry(&previous){
        return Err("previous game backup already exists; refusing to overwrite it".into());
    }
    if old_present && !directory(game){return Err("unsafe existing game directory".into());}
    // Never stage game data until the CD, approved EXE and a verified
    // updater SOURCE have passed preflight. In explicitly private builds the
    // updater may be embedded, not a filesystem source. Public builds still
    // demand a local authentic updater.
    let sources=preflight_install_sources(base,game,None)?;
    // Bind the chosen native source plan to the exact CD edition verified
    // during preflight. Extraction independently rechecks it before writes.
    let (updater,expected_edition)=match &sources{
        InstallSources::Approved{updater,..}=>(Some(updater),None),
        #[cfg(feature="native-fr-cd")]
        InstallSources::NativeCd102{edition}=>(None,Some(*edition)),
        #[cfg(feature="native-usde-updater")]
        InstallSources::NativeWithUpdater{edition,updater}=>(Some(updater),Some(*edition)),
    };
    if old_present {
        println!("Rebuilding game in a private verified transaction...");
    }else{
        println!("Installing game from exact original CD files...");
    }
    install_from_cd(base,&new,updater,expected_edition)?;
    if old_present {preserve_player_data(game,&new)?;}
    match sources {
        InstallSources::Approved{executable,..}=>activate_verified_exe(&new,&executable)?,
        #[cfg(feature="native-fr-cd")]
        InstallSources::NativeCd102{..}=>activate_pending_exe(base,&new)?,
        #[cfg(feature="native-usde-updater")]
        InstallSources::NativeWithUpdater{updater,..}=>
            activate_native_usde_from_update(base,&new,&updater)?,
    }
    verify_installed_cd_assets(&new)?;
    verify(&new)?;
    if old_present {
        // Publish using a reversible directory swap. If the final
        // post-publish SHA check fails, put the original directory back,
        // leaving the candidate under the separate reconstruction name.
        fs::rename(game,&previous).map_err(error)?;
        if let Err(err)=fs::rename(&new,game){
            let rollback=fs::rename(&previous,game);
            return Err(format!("cannot publish rebuilt game: {err}; rollback={rollback:?}"));
        }
        #[cfg(feature="unified-fix-install")]
        let previous_receipt_present=match publish_unified_receipt(game){
            Ok(was_present)=>was_present,
            Err(reason)=>{
                let staged=fs::rename(game,&new);
                let restored=fs::rename(&previous,game);
                return Err(format!("rebuilt game receipt publication failed: {reason}; candidate={staged:?}; old game restoration={restored:?}"));
            }
        };
        let validated=verify_installed_cd_assets(game)
            .and_then(|_|verify(game).map(|_|()));
        if let Err(reason)=validated {
            #[cfg(feature="unified-fix-install")]
            rollback_unified_receipt(game,previous_receipt_present)
                .map_err(|e|format!("cannot restore previous receipt after final verification failure ({reason}): {e}; previous game is safe at {}",previous.display()))?;
            if let Err(err)=fs::rename(game,&new){
                return Err(format!(
                    "repaired installation failed final SHA verification ({reason});                      old installation is safe in {} but new game could not be moved: {err}",
                    previous.display()));
            }
            if let Err(err)=fs::rename(&previous,game){
                return Err(format!(
                    "repaired installation failed final SHA verification ({reason});                      original game remains in {} because rollback failed: {err}",
                    previous.display()));
            }
            return Err(format!(
                "rebuilt game failed post-publication verification ({reason});                  previous game restored; failed rebuild retained at {}",new.display()));
        }
        #[cfg(feature="unified-fix-install")]
        finish_unified_receipt(game,previous_receipt_present)?;
        println!("Previous installation preserved at {}.",previous.display());
    }else{
        if any_entry(game){return Err("game target appeared while installing; refusing overwrite".into());}
        fs::rename(&new,game).map_err(error)?;
        #[cfg(feature="unified-fix-install")]
        let previous_receipt_present=match publish_unified_receipt(game){
            Ok(was_present)=>was_present,
            Err(reason)=>{
                let rollback=fs::rename(game,&new);
                return Err(format!("new game receipt publication failed: {reason}; candidate rollback={rollback:?}"));
            }
        };
        let validated=verify_installed_cd_assets(game).and_then(|_|verify(game).map(|_|()));
        if let Err(reason)=validated{
            #[cfg(feature="unified-fix-install")]
            {
                rollback_unified_receipt(game,previous_receipt_present)
                    .map_err(|e|format!("cannot rollback failed fresh CD receipt ({reason}): {e}"))?;
                fs::rename(game,&new).map_err(|e|format!("cannot rollback failed new game ({reason}): {e}"))?;
            }
            return Err(format!("new game failed post-publication verification: {reason}"));
        }
        #[cfg(feature="unified-fix-install")]
        finish_unified_receipt(game,previous_receipt_present)?;
    }
    Ok(())
}

fn auto_prepare(base:&Path,game:&Path)->Result<&'static str,String>{
    // The v1.1 candidate must never mistake an old CrimsonSkiesFix
    // ddraw.dll (or a third-party wrapper) for a missing file to repair.
    // Refuse *before* CD extraction, installer staging or player-data writes.
    ensure_no_local_ddraw(game)?;
    // The WinMM-only repair shortcut must NOT intercept the unified fix.dll.
    // A unified manual import uses the dedicated, post-repair-verified path below.
    #[cfg(not(feature="unified-fix-install"))]
    if cfg!(feature="winmm-installer") && directory(game) {
        let runtime=game.join(RUNTIME_DLL);
        if any_entry(&runtime) && !regular(&runtime) {
            return Err("WinMM candidate: unsafe winmm.dll filesystem object; refusing installation".into());
        }
        let current_digest=if regular(&runtime){Some(digest(&runtime)?)}else{None};
        let expected=sha256::hex(BUNDLED_RUNTIME);
        // Any existing regular winmm.dll can be backed up automatically.
        // deploy_runtime handles transactional replacement and one .bak.
        // Existing verified manually imported game: install or upgrade the
        // complete WinMM-only runtime without CD rebuild, EXE replacement or
        // edits to save data. A CD receipt still uses strict verification.
        if !any_entry(&receipt_path(game)) &&
            validated_game_exe(game).is_ok() &&
            (current_digest.as_deref()!=Some(expected.as_str()) || !bundled_core_matches(game)
             || (cfg!(feature="import-alias-install") && any_entry(&game.join("winmm.dll")))) {
            deploy_runtime(game)?;
            deploy_documentation(game)?;
            return Ok("INSTALLED_WINMM_RUNTIME_ONLY_AND_LAUNCH");
        }
    }
    if !any_entry(game){
        build_and_publish(base,game,false)?;
        return Ok("INSTALL_FROM_CD_AND_LAUNCH");
    }
    if !directory(game){return Err("unsafe existing game path (not a real directory)".into());}
    if any_entry(&receipt_path(game)) && !regular(&receipt_path(game)){
        return Err("unsafe existing CD installation receipt; refusing to launch".into());
    }
    if regular(&receipt_path(game)){
        match verify_installed_cd_assets(game){
            Ok((_edition,_count))=>{
                if validated_game_exe(game).is_ok() {
                    return Ok("VERIFIED_EXISTING_AND_LAUNCH");
                }
                // A fully verified original CD installation awaiting the
                // approved EXE can be completed with no second CD extraction.
                activate_pending_exe(base,game)?;
                return Ok("ACTIVATE_APPROVED_EXE_AND_LAUNCH");
            }
            Err(reason)=>{
                println!("Installed CD files did not verify: {reason}");
                // Preserve the precise first verification failure. The
                // previous launcher's log lost this evidence when a later
                // stale staging directory blocked the reconstruction.
                // If only the bundled DLL changed, repair that first.
                let followup=match deploy_runtime(game).and_then(|_|deploy_documentation(game)){
                    Ok(())=>match verify_installed_cd_assets(game){
                        Ok(_)=>{
                            // Keep build-info.txt in sync with the DLL.
                            deploy_documentation(game)?;
                            if validated_game_exe(game).is_ok(){
                                return Ok("REPAIRED_BUNDLED_PROXY_AND_LAUNCH");
                            }
                            activate_pending_exe(base,game)?;
                            return Ok("REPAIRED_PROXY_AND_ACTIVATED_EXE_AND_LAUNCH");
                        }
                        Err(after)=>format!(
                            "installed CD verification after bundled DLL repair: {after}"),
                    },
                    Err(err)=>format!("bundled DLL repair not completed: {err}"),
                };
                build_and_publish(base,game,true).map_err(|err|format!(
                    "existing installed CD verification failed: {reason}; \
                     {followup}; rebuild not completed: {err}"))?;
                return Ok("REPAIRED_FROM_CD_AND_LAUNCH");
            }
        }
    }
    // A validated manual import may have the exact approved game EXE and
    // assets but an older (or third-party) fix.dll. Replace only our runtime
    // in place, without demanding the CD/updater or rebuilding player data.
    // Preserve the strict checks for the game EXE, assets and native WinMM.
    #[cfg(feature="unified-fix-install")]
    if validated_game_exe(game).is_ok() && !any_entry(&game.join("winmm.dll")){
        let runtime=game.join(RUNTIME_DLL);
        if any_entry(&runtime)&&!regular(&runtime){
            return Err("unsafe existing fix.dll: refusing replacement".into());
        }
        if !regular(&runtime)||digest(&runtime)?!=sha256::hex(BUNDLED_RUNTIME){
            deploy_runtime(game)?;
            deploy_documentation(game)?;
            verify(game)?;
            return Ok("REPAIRED_BUNDLED_FIX_DLL_AND_LAUNCH");
        }
    }
    // Backwards compatibility: a validated manually imported installation
    // may lack an original-CD receipt. Its old-format verification is less
    // comprehensive than the new immutable CD file manifest.
    match verify(game){
        Ok(_)=>Ok("VERIFIED_LEGACY_IMPORT_AND_LAUNCH"),
        Err(reason)=>{
            build_and_publish(base,game,true).map_err(|err|format!(
                "legacy installed game verification failed: {reason}; \
                 rebuild not completed: {err}"))?;
            Ok("REBUILT_LEGACY_INSTALL_FROM_CD_AND_LAUNCH")
        }
    }
}

// An installer-created INI is the only one eligible for first-launch logging.
// The marker exists solely between first installation and the first successful
// game exit. It never grants permission to rewrite a user-customized INI.
#[cfg(feature="unified-fix-install")]
const FIRST_LAUNCH_MARKER: &str = "CrimsonSkiesFix.first-launch-log.pending";
#[cfg(feature="unified-fix-install")]
const FIRST_LAUNCH_MARKER_BYTES: &[u8] = b"CrimsonSkiesFix managed first-launch log v1\n";

#[cfg(feature="unified-fix-install")]
fn initial_unified_ini() -> &'static str {
    "; CrimsonSkiesFix: installer-managed first-launch diagnostic.\n\
     ; Logging is disabled automatically after the first successful game exit.\n\
     [Graphics]\nAspectRatio=auto\nFPSLimit=60\n\n[Debug]\nLog=1\n"
}
#[cfg(feature="unified-fix-install")]
fn quiet_unified_ini() -> String {
    initial_unified_ini().replace("Log=1\n", "Log=0\n")
}

// The runtime's once-only High lighting confirmation appends this exact
// suffix. It is not a player edit to the logging/graphics preferences.
// Permit only that byte-for-byte suffix, not arbitrary compatibility keys.
#[cfg(feature="unified-fix-install")]
const LIGHTING_CONFIRMATION_SUFFIX:&str="\n[Compatibility]\nCrimsonSkiesFixLightingHighDefaultApplied=1\n";
#[cfg(feature="unified-fix-install")]
fn managed_quiet_ini(current:&[u8])->Option<String>{
    let suffix=current.strip_prefix(initial_unified_ini().as_bytes())?;
    if suffix.is_empty(){return Some(quiet_unified_ini());}
    if suffix==LIGHTING_CONFIRMATION_SUFFIX.as_bytes(){
        return Some(quiet_unified_ini()+LIGHTING_CONFIRMATION_SUFFIX);
    }
    None
}

#[cfg(feature="unified-fix-install")]
fn first_launch_state_dir(base:&Path)->Result<PathBuf,String>{
    let state=base.join("state");
    if !directory(&state){
        if any_entry(&state){
            return Err("unsafe state/ path: cannot track first-launch logging".into());
        }
        fs::create_dir(&state).map_err(error)?;
    }
    Ok(state)
}

#[cfg(feature="unified-fix-install")]
fn prepare_first_launch_ini(base:&Path)->Result<(),String>{
    let ini=base.join("CrimsonSkiesFix.ini");
    if any_entry(&ini){
        if regular(&ini){return Ok(());} // Never touch a user-owned INI.
        return Err(format!("unsafe existing INI: {}",ini.display()));
    }
    let marker=first_launch_state_dir(base)?.join(FIRST_LAUNCH_MARKER);
    if any_entry(&marker){
        return Err("existing first-launch state marker must be preserved".into());
    }
    // Create the INI only once. A pre-existing INI can never be enrolled.
    let mut file=OpenOptions::new().write(true).create_new(true).open(&ini).map_err(error)?;
    let write_result=file.write_all(initial_unified_ini().as_bytes())
        .and_then(|_|file.sync_all());
    drop(file); // Windows must close the handle before rollback or replacement.
    if let Err(err)=write_result{
        // We created this file, so a partial write can be safely rolled back.
        let _=fs::remove_file(&ini);
        return Err(format!("could not initialize first-launch INI: {err}"));
    }
    let marker_result=(||->Result<(),String>{
        let mut f=OpenOptions::new().write(true).create_new(true).open(&marker).map_err(error)?;
        f.write_all(FIRST_LAUNCH_MARKER_BYTES).map_err(error)?;
        f.sync_all().map_err(error)
    })();
    if let Err(reason)=marker_result{
        // Do not leave an automatically managed Log=1 INI without its marker.
        if regular(&marker){let _=fs::remove_file(&marker);}
        if regular(&ini) && fs::read(&ini).ok().as_deref()==Some(initial_unified_ini().as_bytes()){
            let _=fs::remove_file(&ini);
        }
        return Err(format!("could not initialize first-launch logging: {reason}"));
    }
    Ok(())
}

// Called only after a game process exits. A failed launch/crash never switches
// off first-launch logging. No switch occurs without a real session log.
#[cfg(feature="unified-fix-install")]
fn finish_first_launch_log(base:&Path,code:i32)->Result<bool,String>{
    if code!=0{return Ok(false);}
    let marker=base.join("state").join(FIRST_LAUNCH_MARKER);
    if !any_entry(&marker){return Ok(false);}
    if !regular(&marker) || fs::read(&marker).map_err(error)?!=FIRST_LAUNCH_MARKER_BYTES{
        return Err("unknown first-launch logging state preserved without changes".into());
    }
    let ini=base.join("CrimsonSkiesFix.ini");
    // Apart from the exact runtime-generated compatibility marker, any
    // edit invalidates the automatic opt-out.
    let next=if regular(&ini){
        managed_quiet_ini(&fs::read(&ini).map_err(error)?)
    }else{None};
    let Some(next)=next else{
        fs::remove_file(&marker).map_err(error)?; // Owner has taken control.
        return Ok(false);
    };
    let session=base.join("CrimsonSkiesFix.log");
    if !regular(&session) || fs::metadata(&session).map_err(error)?.len()==0{
        return Ok(false); // No evidence of a first gameplay session.
    }
    let temporary=base.join("state").join("CrimsonSkiesFix.ini.first-launch.pending");
    if any_entry(&temporary){
        return Err("first-launch INI replacement blocked by an existing pending file".into());
    }
    let mut file=OpenOptions::new().write(true).create_new(true)
        .open(&temporary).map_err(error)?;
    let staged=file.write_all(next.as_bytes()).and_then(|_|file.sync_all());
    drop(file); // The pending INI must be closed before Windows can rename it.
    if let Err(err)=staged{
        let _=fs::remove_file(&temporary);
        return Err(format!("could not stage quiet first-launch INI: {err}"));
    }
    // Same volume, atomic file replacement; never modify a customized INI.
    if let Err(err)=fs::rename(&temporary,&ini){
        let _=fs::remove_file(&temporary);
        return Err(format!("cannot finalize first-launch INI: {err}"));
    }
    fs::remove_file(&marker).map_err(error)?;
    Ok(true)
}


// Provenance fields are diagnostic only. The edition passed here has
// already passed independent installed-asset verification. Never print
// arbitrary receipt text as a log line or infer the original update path
// just from a known edition or the currently installed patch asset.
fn recorded_update_source(edition:&str,receipt:&str)->&'static str{
    let mut values=receipt.lines().filter_map(|line|line.strip_prefix("UpdateSHA256="));
    let value=values.next();
    if values.next().is_some(){return "UNKNOWN";}
    match value{
        Some("NOT_REQUIRED_ORIGINAL_FR_102_CD") if edition=="C_SKIES_FR_001"=>
            "ORIGINAL_FR_102_CD",
        Some("NOT_REQUIRED_ORIGINAL_EU_102_CD") if edition=="C_SKIES_DISC_002"=>
            "ORIGINAL_EU_102_CD",
        Some(hash) if hash==update102::PATCH_SHA256=>"OFFICIAL_102_UPDATER",
        _=>"UNKNOWN",
    }
}

fn install_provenance_fields(game:&Path,edition:Option<&str>)->String{
    let Some(edition)=edition else{
        return "Edition=UNKNOWN\nRecordedUpdateSource=UNKNOWN\n".into();
    };
    let path=receipt_path(game);
    let source=if regular(&path) && fs::metadata(&path).is_ok_and(|m|m.len()<=2048){
        fs::read_to_string(path).ok().map(|receipt|recorded_update_source(edition,&receipt))
            .unwrap_or("UNKNOWN")
    }else{"UNKNOWN"};
    format!("Edition={edition}\nRecordedUpdateSource={source}\n")
}

/// Entry point for no-argument execution. The installer keeps its last
/// meaningful installation/repair/error report in the root installer log.
/// The DLL writes an independent fresh gameplay log on every game start.
pub(super) fn auto_run(base:&Path,game:&Path,launch:bool)->Result<i32,String>{
    // Refuse unsafe installer-log paths before modifying an installation.
    // A verified, no-op relaunch does not overwrite any existing report.
    let log=base.join(INSTALLER_LOG);
    if any_entry(&log) && !regular(&log){
        return Err("unsafe CrimsonSkiesFix_installer.log: refusing non-regular file".into());
    }
    // Never log READY unless the final independent validation succeeded.
    let result=auto_prepare(base,game).and_then(|action|{
        let edition=if regular(&receipt_path(game)){
            Some(verify_installed_cd_assets(game)?.0)
        }else{None};
        let exe=verify(game)?;
        // The unified installer records a one-shot first-launch log only
        // for an INI it created. Manually supplied INIs remain untouched.
        #[cfg(feature="unified-fix-install")]
        prepare_first_launch_ini(&patch_dir(game))?;
        #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
        {
            let content="; Experimental split-loader compatibility test.\n\
                [Graphics]\nAspectRatio=auto\nFPSLimit=60\n\n[Debug]\nLog=1\n";
            ensure_new_text(&patch_dir(game).join("CrimsonSkiesFix.ini"),content)?;
        }
        Ok((action,exe,edition))
    });
    let needed=report_needed(result.as_ref().ok().map(|(action,_,_)|*action));
    let message=match &result{
        Ok((action,_exe,edition))=>{
            let result_action=if launch {(*action).to_owned()}else{
                format!("{}_ONLY",action.strip_suffix("_AND_LAUNCH").unwrap_or(action))
            };
            let launch_line=if launch {""}else{"GameLaunch=SKIPPED\n"};
            let provenance=install_provenance_fields(game,*edition);
            format!(
            "CrimsonSkiesFix AUTO INSTALL v1\nBuild={VERSION}-{}\nAction={result_action}\nGame={}\n\
             {provenance}VerifiedGameData={}\nApprovedExecutable={}\n\
             OriginalProtectedBackup={}\nGosOverlayState={}\n{launch_line}AUTO_INSTALL_RESULT=READY\n",
            &COMMIT[..8],game.display(),if edition.is_some(){"YES"}else{"LEGACY_ONLY"},
            if validated_game_exe(game).is_ok(){"SHA256_VERIFIED"}else{"NO"},
            if regular(&game.join(EXE_BACKUP)){"PRESERVED"}else{"LEGACY_OR_UNKNOWN"},
            crate::managed_gos_patch::verify_game_overlay(game)
                .map(|state|state.label()).unwrap_or("INVALID_OR_UNKNOWN"))
        },
        Err(err)=>format!(
            "CrimsonSkiesFix AUTO INSTALL v1\nBuild={VERSION}-{}\nGame={}\nAUTO_INSTALL_RESULT=ERROR\nERROR={err}\n",
            &COMMIT[..8],game.display()),
    };
    write_installer_report(base,&message,needed)?;
    // Return the complete preflight error to the caller: main() prints it
    // to stderr for both interactive terminals and non-interactive scripts.
    // Never display a modal GUI dialog.
    let (action,exe,edition)=result?;
    if !launch{
        println!("--install-only: installation verified and ready at {}; game NOT launched.",
            game.display());
        return Ok(0);
    }
    println!("{action}: launching {}...",exe.display());
    #[cfg(feature="winmm-split-install")]
    let mut launch_summary:Option<String>=None;
    #[cfg(not(feature="winmm-split-install"))]
    let launch_summary:Option<String>=None;
    // Organized prototype only: inspect loaded DLLs and read the exact
    // DirectDraw import slots from the *running process*, without patching
    // or suspending it. The user-visible game exit code is unchanged.
    #[cfg(feature="import-alias-install")]
    let status={
        use std::os::windows::process::ExitStatusExt;
        super::alias_import_launcher::launch_child(&exe,u32::MAX)
            .map(|(code,summary)|{
                println!("{summary}");
                launch_summary=Some(summary);
                std::process::ExitStatus::from_raw(code)
            })
            .map_err(std::io::Error::other)
    };
    #[cfg(all(feature="winmm-split-install",not(feature="import-alias-install")))]
    let status=super::split_process_probe::launch(&exe,game).map(|(exit,summary)|{
        println!("{summary}");
        launch_summary=Some(summary);
        exit
    });
    #[cfg(not(feature="winmm-split-install"))]
    let status=Command::new(&exe).current_dir(game).status();
    match status{
        Ok(status)=>{
            let code=status.code().unwrap_or(2);
            // Keep the install report bounded and retain unexpected exits
            // even after removing the separate experimental session journal.
            if let Err(err)=record_game_exit(base,&message,needed,launch_summary.as_deref(),code){
                eprintln!("Warning: game exit diagnostic unavailable: {err}");
            }
            #[cfg(feature="unified-fix-install")]
            match finish_first_launch_log(base,code){
                Ok(true)=>println!("First-launch diagnostics retained; subsequent game sessions use Log=0."),
                Ok(false)=>{},
                Err(err)=>eprintln!("Warning: first-launch log finalization skipped: {err}"),
            }
            println!("Game exited with status {code}.");
            Ok(code)
        }
        Err(err)=>{
            // A failed game start is a new launcher error worth recording,
            // but never alters the independent DLL-owned gameplay log.
            let provenance=install_provenance_fields(game,edition);
            let failure=format!(
                "CrimsonSkiesFix AUTO INSTALL v1\nBuild={VERSION}-{}\nAction={action}\nGame={}\n{provenance}AUTO_INSTALL_RESULT=ERROR\nERROR=GAME_LAUNCH_FAILED: {err}\n",
                &COMMIT[..8],game.display());
            write_installer_report(base,&failure,true)?;
            Err(error(err))
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::atomic::{AtomicU64,Ordering};
    // Windows SystemTime can have coarse precision even though as_nanos()
    // is used, and Rust tests execute concurrently within one process.
    // Use a monotonic per-process nonce and retry existing test directories.
    static NEXT_TEMP:AtomicU64=AtomicU64::new(0);
    fn temp()->PathBuf{
        let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        for _ in 0..64{
            let seq=NEXT_TEMP.fetch_add(1,Ordering::Relaxed);
            let p=std::env::temp_dir().join(format!(
                "crimson-auto-flow-{}-{n}-{seq}",std::process::id()));
            match fs::create_dir(&p){
                Ok(())=>return p,
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>continue,
                Err(e)=>panic!("cannot create isolated test directory: {e}"),
            }
        }
        panic!("could not reserve distinct auto-install test directory")
    }
    #[test]
    fn update_source_diagnostic_reports_only_recognized_receipt_values(){
        assert_eq!(recorded_update_source("C_SKIES_FR_001","UpdateSHA256=NOT_REQUIRED_ORIGINAL_FR_102_CD\n"),"ORIGINAL_FR_102_CD");
        assert_eq!(recorded_update_source("C_SKIES_DISC_002","UpdateSHA256=NOT_REQUIRED_ORIGINAL_EU_102_CD\n"),"ORIGINAL_EU_102_CD");
        for edition in ["C_SKIES_DISC_001","C_SKIES_FR_002","C_SKIES_DE_001"]{
            let receipt=format!("UpdateSHA256={}\n",update102::PATCH_SHA256);
            assert_eq!(recorded_update_source(edition,&receipt),"OFFICIAL_102_UPDATER");
        }
        for receipt in ["","UpdateSHA256=unverified\n",
            "UpdateSHA256=NOT_REQUIRED_ORIGINAL_FR_102_CD\n",
            "UpdateSHA256=NOT_REQUIRED_ORIGINAL_EU_102_CD\nUpdateSHA256=unknown\n"]{
            assert_eq!(recorded_update_source("C_SKIES_DISC_002",receipt),"UNKNOWN");
        }
    }
    #[test]
    fn provenance_diagnostic_does_not_infer_missing_source_or_legacy_edition(){
        let root=temp();
        let game=root.join(GAME_FOLDER);
        let receipt=receipt_path(&game);
        fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        fs::write(&receipt,b"Edition=C_SKIES_DISC_002\nUpdateSHA256=NOT_REQUIRED_ORIGINAL_EU_102_CD\n").unwrap();
        assert_eq!(install_provenance_fields(&game,Some("C_SKIES_DISC_002")),
            "Edition=C_SKIES_DISC_002\nRecordedUpdateSource=ORIGINAL_EU_102_CD\n");
        assert_eq!(install_provenance_fields(&game,None),
            "Edition=UNKNOWN\nRecordedUpdateSource=UNKNOWN\n");
        fs::write(&receipt,b"Edition=C_SKIES_DISC_002\n").unwrap();
        assert_eq!(install_provenance_fields(&game,Some("C_SKIES_DISC_002")),
            "Edition=C_SKIES_DISC_002\nRecordedUpdateSource=UNKNOWN\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_generated_installer_log_removed_only_when_replaced(){
        let root=temp();
        let old=root.join(OLD_AUTO_LOG);
        let new_log=root.join(INSTALLER_LOG);
        let game_log=root.join("CrimsonSkiesFix.log");
        fs::write(&old,b"CrimsonSkiesFix AUTO INSTALL v1\nAUTO_INSTALL_RESULT=READY\n").unwrap();
        fs::write(&game_log,b"player game diagnostics\n").unwrap();
        write_installer_report(&root,"Action=INSTALL_FROM_CD_AND_LAUNCH\n",true).unwrap();
        assert!(!old.exists());
        assert!(new_log.exists());
        assert_eq!(fs::read(&game_log).unwrap(),b"player game diagnostics\n");
        fs::write(&old,b"owner diagnostics; preserve me\n").unwrap();
        write_installer_report(&root,"Action=REPAIRED_FROM_CD_AND_LAUNCH\n",true).unwrap();
        assert_eq!(fs::read(&old).unwrap(),b"owner diagnostics; preserve me\n");
        assert_eq!(fs::read(&game_log).unwrap(),b"player game diagnostics\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hyphenated_installer_report_migrates_without_loss_on_relaunch(){
        let root=temp();
        let old=root.join(PREVIOUS_INSTALLER_LOG);
        let target=root.join(INSTALLER_LOG);
        let game=root.join("CrimsonSkiesFix.log");
        let original=b"CrimsonSkiesFix AUTO INSTALL v1\nAction=INSTALL_FROM_CD_AND_LAUNCH\n";
        fs::write(&old,original).unwrap();
        fs::write(&game,b"previous game session").unwrap();
        migrate_previous_installer_log(&root);
        assert!(!old.exists());
        assert_eq!(fs::read(&target).unwrap(),original);
        assert_eq!(fs::read(&game).unwrap(),b"previous game session");
        write_installer_report(&root,"Action=SKIP",false).unwrap();
        assert_eq!(fs::read(&target).unwrap(),original);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unrecognized_legacy_log_is_never_deleted_or_renamed(){
        let root=temp();
        let old=root.join(PREVIOUS_INSTALLER_LOG);
        let target=root.join(INSTALLER_LOG);
        fs::write(&old,b"player private debugging notes").unwrap();
        migrate_previous_installer_log(&root);
        assert!(!target.exists());
        assert_eq!(fs::read(&old).unwrap(),b"player private debugging notes");
        write_installer_report(&root,"new installer report",true).unwrap();
        assert_eq!(fs::read(&target).unwrap(),b"new installer report");
        assert_eq!(fs::read(&old).unwrap(),b"player private debugging notes");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recognized_hyphenated_log_retired_only_after_real_report(){
        let root=temp();
        let old=root.join(PREVIOUS_INSTALLER_LOG);
        let previous=b"CrimsonSkiesFix CD INSPECT v1\nINSPECT_RESULT=COMPLETE\n";
        fs::write(&old,previous).unwrap();
        write_installer_report(&root,"new repair report",false).unwrap();
        assert_eq!(fs::read(&old).unwrap(),previous);
        write_installer_report(&root,"new repair report",true).unwrap();
        assert!(!old.exists());
        assert_eq!(fs::read(root.join(INSTALLER_LOG)).unwrap(),b"new repair report");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn installer_log_survives_verified_game_relaunch_but_not_repair(){
        let root=temp();
        let log=root.join(INSTALLER_LOG);
        let game_log=root.join("CrimsonSkiesFix.log");
        let install=b"Action=INSTALL_FROM_CD_AND_LAUNCH\n";
        write_installer_report(&root,std::str::from_utf8(install).unwrap(),true).unwrap();
        for action in ["VERIFIED_EXISTING_AND_LAUNCH","VERIFIED_LEGACY_IMPORT_AND_LAUNCH"]{
            assert!(!report_needed(Some(action)));
            write_installer_report(&root,"Action=SHOULD_NOT_REPLACE\n",report_needed(Some(action))).unwrap();
            fs::write(&game_log,format!("GameSession={action}\n")).unwrap();
            assert_eq!(fs::read(&log).unwrap(),install);
        }
        for action in ["INSTALL_FROM_CD_AND_LAUNCH","REPAIRED_FROM_CD_AND_LAUNCH",
            "REPAIRED_BUNDLED_PROXY_AND_LAUNCH","ACTIVATE_APPROVED_EXE_AND_LAUNCH"]{
            assert!(report_needed(Some(action)));
        }
        assert!(report_needed(None)); // errors are always retained
        write_installer_report(&root,"AUTO_INSTALL_RESULT=ERROR\n",report_needed(None)).unwrap();
        assert_eq!(fs::read(&log).unwrap(),b"AUTO_INSTALL_RESULT=ERROR\n");
        assert!(fs::read_to_string(&game_log).unwrap().starts_with("GameSession="));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn exit_report_does_not_accumulate_on_verified_relaunch(){
        let root=temp();
        let target=root.join(INSTALLER_LOG);
        let game_log=root.join("CrimsonSkiesFix.log");
        let report="CrimsonSkiesFix AUTO INSTALL v1\nAction=INSTALL_FROM_CD_AND_LAUNCH\n";
        fs::write(&game_log,b"owner gameplay log").unwrap();
        record_game_exit(&root,report,true,Some("ALIAS-LAUNCH: first"),0).unwrap();
        let first=fs::read(&target).unwrap();
        for _ in 0..10{
            record_game_exit(&root,"Action=VERIFIED_EXISTING_AND_LAUNCH\n",false,Some("ALIAS-LAUNCH: later"),0).unwrap();
        }
        assert_eq!(fs::read(&target).unwrap(),first);
        let current="CrimsonSkiesFix AUTO INSTALL v1\nAction=VERIFIED_EXISTING_AND_LAUNCH\n";
        for code in [1,0xC0000005u32 as i32]{
            record_game_exit(&root,current,false,Some("ALIAS-LAUNCH: abnormal"),code).unwrap();
        }
        let last=fs::read_to_string(&target).unwrap();
        assert_eq!(last.matches("ALIAS-LAUNCH:").count(),1);
        assert_eq!(last.matches("GAME_EXIT_CODE=").count(),1);
        assert!(last.contains("GameExitCodeHex=0xC0000005"));
        assert!(!last.contains("INSTALL_FROM_CD"));
        assert_eq!(fs::read(&game_log).unwrap(),b"owner gameplay log");
        assert!(!root.join("CrimsonSkiesFix_last-session.log").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsafe_installer_log_never_truncates_game_log_or_owner_data(){
        let root=temp();
        let log=root.join(INSTALLER_LOG);
        fs::create_dir(&log).unwrap();
        fs::write(log.join("keep-me"),b"owner diagnostics").unwrap();
        fs::write(root.join("CrimsonSkiesFix.log"),b"existing game log").unwrap();
        assert!(write_installer_report(&root,"repair",true).is_err());
        assert_eq!(fs::read(log.join("keep-me")).unwrap(),b"owner diagnostics");
        assert_eq!(fs::read(root.join("CrimsonSkiesFix.log")).unwrap(),b"existing game log");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn blocking_staging_directory_reports_initial_cd_failure_without_deletion(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        let stale=root.join(REBUILT);
        fs::create_dir(&game).unwrap();
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join("keep-me"),b"owner data").unwrap();
        #[cfg(all(feature="winmm-split-install",not(feature="unified-fix-install")))]
        fs::create_dir(patch_dir(&game)).unwrap();
        fs::write(receipt_path(&game),b"invalid receipt").unwrap();
        let error=auto_prepare(&root,&game).unwrap_err();
        assert!(error.contains("existing installed CD verification failed"));
        assert!(error.contains("rebuild blocked by existing"));
        assert!(error.contains(REBUILT));
        assert_eq!(fs::read(stale.join("keep-me")).unwrap(),b"owner data");
        assert!(game.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pending_staging_directory_is_identified_without_removal(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        let pending=root.join(format!("{REBUILT}.installing"));
        fs::create_dir(&game).unwrap();
        fs::create_dir(&pending).unwrap();
        fs::write(pending.join("keep-me"),b"pending owner data").unwrap();
        let error=build_and_publish(&root,&game,true).unwrap_err();
        assert!(error.contains(&format!("{REBUILT}.installing")));
        assert_eq!(fs::read(pending.join("keep-me")).unwrap(),b"pending owner data");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn damaged_existing_install_is_left_intact_without_authentic_sources(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        fs::create_dir(&game).unwrap();
        fs::write(game.join("player-unique-file.txt"),b"must survive").unwrap();
        fs::create_dir(game.join("SavedGames")).unwrap();
        fs::write(game.join("SavedGames").join("pilot.dat"),b"pilot").unwrap();
        assert!(auto_prepare(&root,&game).is_err());
        assert_eq!(fs::read(game.join("player-unique-file.txt")).unwrap(),b"must survive");
        assert_eq!(fs::read(game.join("SavedGames").join("pilot.dat")).unwrap(),b"pilot");
        assert!(!root.join(REBUILT).exists());
        assert!(!root.join(PREVIOUS).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn repaired_player_data_is_copied_without_modifying_original(){
        let root=temp();
        let old=root.join("old");
        let new=root.join("new");
        for game in [&old,&new]{
            fs::create_dir(game).unwrap();
            fs::create_dir(game.join("SavedGames")).unwrap();
            fs::create_dir(game.join("Planes")).unwrap();
        }
        fs::create_dir(old.join("SavedGames").join("career")).unwrap();
        fs::write(old.join("SavedGames").join("career").join("pilot.dat"),b"original save").unwrap();
        fs::write(old.join("Planes").join("custom.pln"),b"custom plane").unwrap();
        fs::write(old.join("CrimsonSkiesFix.ini"),b"[System]\nCustom=1").unwrap();
        preserve_player_data(&old,&new).unwrap();
        assert_eq!(fs::read(new.join("SavedGames").join("career").join("pilot.dat")).unwrap(),b"original save");
        assert_eq!(fs::read(new.join("Planes").join("custom.pln")).unwrap(),b"custom plane");
        assert_eq!(fs::read(patch_dir(&new).join("CrimsonSkiesFix.ini")).unwrap(),b"[System]\nCustom=1");
        assert_eq!(fs::read(old.join("Planes").join("custom.pln")).unwrap(),b"custom plane");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn install_only_without_sources_refuses_extraction_and_never_launches(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        let err=auto_run(&root,&game,false).unwrap_err();
        assert!(err.contains("original game CD image"),"{err}");
        #[cfg(all(not(feature="embed-owner-patch102"),not(feature="native-usde-updater")))]
        assert!(err.contains("official 1.02 updater"),"{err}");
        #[cfg(not(feature="native-usde-updater"))]
        assert!(err.contains("approved decrypted Crimson Skies 1.02 EXE"),"{err}");
        #[cfg(feature="native-usde-updater")]
        {
            assert!(!err.contains("approved decrypted"),"{err}");
            assert!(!err.contains("official 1.02 updater"),"{err}");
        }
        assert!(!any_entry(&game));
        assert!(!any_entry(&root.join(REBUILT)));
        assert!(!any_entry(&root.join(format!("{REBUILT}.installing"))));
        let report=fs::read_to_string(root.join(INSTALLER_LOG)).unwrap();
        assert!(report.contains("AUTO_INSTALL_RESULT=ERROR"));
        assert!(!report.contains("GAME_EXIT_CODE="));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(not(feature="embed-owner-patch102"))]
    #[test]
    fn missing_exe_and_updater_report_both_without_touching_existing_game(){
        let root=temp();
        let game=root.join(if cfg!(feature="unified-fix-install"){"game"}else{"CrimsonSkiesFix"});
        fs::create_dir(&game).unwrap();
        fs::create_dir(game.join("SavedGames")).unwrap();
        fs::write(game.join("SavedGames").join("pilot.dat"),b"owner save").unwrap();
        fs::write(root.join("disc.iso"),b"owner CD image placeholder").unwrap();
        let err=preflight_install_sources(&root,&game,None).unwrap_err();
        #[cfg(feature="native-fr-cd")]
        assert!(err.contains("CD content inspection failed"),"{err}");
        #[cfg(not(feature="native-fr-cd"))]
        {
            assert!(err.contains("approved decrypted Crimson Skies 1.02 EXE"),"{err}");
            assert!(err.contains("no exact verified Crimson Skies 1.02 updater"),"{err}");
            assert!(err.contains("rerun --install-only"),"{err}");
        }
        assert_eq!(fs::read(game.join("SavedGames").join("pilot.dat")).unwrap(),b"owner save");
        assert_eq!(fs::read(root.join("disc.iso")).unwrap(),b"owner CD image placeholder");
        assert!(!any_entry(&root.join(REBUILT)));
        assert!(!any_entry(&root.join(format!("{REBUILT}.installing"))));
        assert!(!any_entry(&root.join(PREVIOUS)));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(not(feature="embed-owner-patch102"))]
    #[test]
    fn missing_prerequisites_are_both_present_in_installer_error_report(){
        let root=temp();
        let game=root.join(if cfg!(feature="unified-fix-install"){"game"}else{"CrimsonSkiesFix"});
        let err=auto_run(&root,&game,false).unwrap_err();
        let report=fs::read_to_string(root.join(INSTALLER_LOG)).unwrap();
        assert!(err.contains("original game CD image"),"{err}");
        assert!(report.contains("original game CD image"),"{report}");
        #[cfg(not(feature="native-usde-updater"))]
        {
            assert!(err.contains("official 1.02 updater"),"{err}");
            assert!(report.contains("approved decrypted 1.02 EXE"),"{report}");
            assert!(report.contains("official 1.02 updater"),"{report}");
        }
        #[cfg(feature="native-usde-updater")]
        {
            assert!(!report.contains("approved decrypted"),"{report}");
            assert!(!report.contains("official 1.02 updater"),"{report}");
        }
        assert!(report.contains("AUTO_INSTALL_RESULT=ERROR"));
        assert!(!any_entry(&game));
        assert!(!any_entry(&root.join(REBUILT)));
        assert!(!any_entry(&root.join("state")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lone_bin_or_img_is_not_an_installable_disc_source(){
        for extension in ["bin","img"]{
            let root=temp();
            let game=root.join(if cfg!(feature="unified-fix-install"){"game"}else{"CrimsonSkiesFix"});
            let media=root.join(format!("disc.{extension}"));
            fs::write(&media,b"file without CUE sheet").unwrap();
            let err=preflight_install_sources(&root,&game,None).unwrap_err();
            assert!(err.contains("original game CD image"),"{err}");
            #[cfg(all(not(feature="embed-owner-patch102"),not(feature="native-usde-updater")))]
            assert!(err.contains("official 1.02 updater"),"{err}");
            #[cfg(not(feature="native-usde-updater"))]
            assert!(err.contains("approved decrypted 1.02 EXE"),"{err}");
            #[cfg(feature="native-usde-updater")]
            {
                assert!(!err.contains("approved decrypted"),"{err}");
                assert!(!err.contains("official 1.02 updater"),"{err}");
            }
            assert!(!any_entry(&game));
            assert_eq!(fs::read(&media).unwrap(),b"file without CUE sheet");
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[cfg(all(feature="native-usde-updater",not(feature="embed-owner-patch102")))]
    #[test]
    fn earlier_cd_missing_update_keeps_native_diagnostic_on_resume(){
        let root=temp();
        let game=root.join(GAME_FOLDER);
        fs::create_dir(&game).unwrap();
        let receipt=receipt_path(&game);
        fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        fs::write(game.join("SavedGames.dat"),b"preserve").unwrap();
        for edition in ["C_SKIES_DISC_001","C_SKIES_DE_001","C_SKIES_FR_002"]{
            fs::write(&receipt,format!("Project=CrimsonSkiesFix\nEdition={edition}\nInstallState=GAME_FILES_READY_EXECUTABLE_PENDING\n")).unwrap();
            let err=activate_pending_exe(&root,&game).unwrap_err();
            assert!(err.contains(edition),"{err}");
            assert!(err.contains("official 1.02 updater required"),"{err}");
            assert!(err.contains("No separate decrypted EXE is needed"),"{err}");
            assert!(!any_entry(&root.join("CrimsonSkiesFix.native-usde-102.pending.exe")));
        }
        assert_eq!(fs::read(game.join("SavedGames.dat")).unwrap(),b"preserve");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="native-fr-cd")]
    #[test]
    fn cd_identification_does_not_fall_back_on_unknown_or_ambiguous_manifest(){
        for edition in ["C_SKIES_FR_001","C_SKIES_FR_002","C_SKIES_DISC_001","C_SKIES_DE_001","C_SKIES_DISC_002"]{
            let report=format!("Edition={edition}\nEditionRecognition=EXACT_INTERNAL_FILE_MANIFEST\n");
            assert_eq!(authenticated_cd_edition(&report).unwrap(),edition);
        }
        for report in [
            "Edition=UNKNOWN\nEditionRecognition=NO_EXACT_INTERNAL_FILE_MANIFEST_MATCH",
            "Edition=C_SKIES_FR_001",
            "Edition=C_SKIES_FR_001\nEdition=C_SKIES_FR_002\nEditionRecognition=EXACT_INTERNAL_FILE_MANIFEST",
        ]{
            assert!(authenticated_cd_edition(report).is_err());
        }
    }
    #[test]
    fn available_iso_is_not_reported_missing_when_other_two_are_absent(){
        let root=temp();
        let game=root.join(if cfg!(feature="unified-fix-install"){"game"}else{"CrimsonSkiesFix"});
        let media=root.join("disc.iso");
        fs::write(&media,b"read-only CD source placeholder").unwrap();
        let err=preflight_install_sources(&root,&game,None).unwrap_err();
        assert!(!err.contains("- original game CD image"),"{err}");
        #[cfg(feature="native-fr-cd")]
        {
            assert!(err.contains("CD content inspection failed"),"{err}");
            assert!(!err.contains("approved decrypted 1.02 EXE"),"{err}");
        }
        #[cfg(not(feature="native-fr-cd"))]
        {
            #[cfg(not(feature="embed-owner-patch102"))]
            assert!(err.contains("- official 1.02 updater"),"{err}");
            assert!(err.contains("- approved decrypted 1.02 EXE"),"{err}");
        }
        assert!(!any_entry(&game));
        assert_eq!(fs::read(&media).unwrap(),b"read-only CD source placeholder");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="embed-owner-patch102")]
    #[test]
    fn private_embed_removes_only_official_update_requirement(){
        let root=temp();
        let game=root.join(if cfg!(feature="unified-fix-install"){"game"}else{"CrimsonSkiesFix"});
        let err=preflight_install_sources(&root,&game,None).unwrap_err();
        assert!(err.contains("original game CD image"),"{err}");
        assert!(!err.contains("- official 1.02 updater"),"{err}");
        #[cfg(not(feature="native-usde-updater"))]
        assert!(err.contains("- approved decrypted 1.02 EXE"),"{err}");
        #[cfg(feature="native-usde-updater")]
        assert!(!err.contains("approved decrypted"),"{err}");
        assert!(!any_entry(&root.join(REBUILT)));
        assert!(!any_entry(&game));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prerequisite_gate_rejects_unknown_executable_before_any_cd_extraction(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        fs::write(root.join("untrusted.exe"),b"not a valid Crimson Skies binary").unwrap();
        let err=preflight_install_sources(&root,&game,None).unwrap_err();
        #[cfg(not(feature="native-usde-updater"))]
        assert!(err.contains("approved decrypted Crimson Skies 1.02 EXE"));
        #[cfg(feature="native-usde-updater")]
        {
            assert!(err.contains("original game CD image"),"{err}");
            assert!(!err.contains("approved decrypted"),"{err}");
        }
        assert!(!any_entry(&game));
        assert!(!any_entry(&root.join(REBUILT)));
        assert!(!any_entry(&root.join(format!("{REBUILT}.installing"))));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_cd_receipt_stays_private_until_game_is_published(){
        let root=temp();
        let staged=root.join(REBUILT);
        fs::create_dir(&staged).unwrap();
        assert_eq!(receipt_path(&staged),staged.join(INSTALL_RECEIPT));
        assert_eq!(receipt_path(&root.join("game")),root.join("state").join(INSTALL_RECEIPT));
        fs::write(staged.join(INSTALL_RECEIPT),b"verified pending receipt").unwrap();
        assert!(!any_entry(&root.join("state").join(INSTALL_RECEIPT)));
        fs::rename(&staged,root.join("game")).unwrap();
        assert!(!any_entry(&root.join("state").join(INSTALL_RECEIPT)));
        let game=root.join("game");
        assert!(!publish_unified_receipt(&game).unwrap());
        assert_eq!(fs::read(root.join("state").join(INSTALL_RECEIPT)).unwrap(),b"verified pending receipt");
        assert!(!any_entry(&game.join(INSTALL_RECEIPT)));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_cd_receipt_rollback_preserves_original_root_receipt(){
        let root=temp();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        fs::create_dir(root.join("state")).unwrap();
        fs::write(root.join("state").join(INSTALL_RECEIPT),b"original receipt").unwrap();
        fs::write(game.join(INSTALL_RECEIPT),b"replacement receipt").unwrap();
        assert!(publish_unified_receipt(&game).unwrap());
        assert_eq!(fs::read(root.join("state").join(INSTALL_RECEIPT)).unwrap(),b"replacement receipt");
        assert_eq!(fs::read(previous_receipt(&game).unwrap()).unwrap(),b"original receipt");
        rollback_unified_receipt(&game,true).unwrap();
        assert_eq!(fs::read(root.join("state").join(INSTALL_RECEIPT)).unwrap(),b"original receipt");
        assert_eq!(fs::read(game.join(INSTALL_RECEIPT)).unwrap(),b"replacement receipt");
        assert!(!any_entry(&previous_receipt(&game).unwrap()));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_cd_receipt_refuses_existing_rollback_backup(){
        let root=temp();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        fs::create_dir(root.join("state")).unwrap();
        fs::write(root.join("state").join(INSTALL_RECEIPT),b"original receipt").unwrap();
        fs::write(game.join(INSTALL_RECEIPT),b"replacement receipt").unwrap();
        fs::write(previous_receipt(&game).unwrap(),b"owner backup").unwrap();
        assert!(publish_unified_receipt(&game).is_err());
        assert_eq!(fs::read(root.join("state").join(INSTALL_RECEIPT)).unwrap(),b"original receipt");
        assert_eq!(fs::read(game.join(INSTALL_RECEIPT)).unwrap(),b"replacement receipt");
        assert_eq!(fs::read(previous_receipt(&game).unwrap()).unwrap(),b"owner backup");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unified_cd_receipt_refuses_occupied_state_and_preserves_owners_files(){
        let root=temp();
        let game=root.join("game");
        fs::create_dir(&game).unwrap();
        fs::write(game.join(INSTALL_RECEIPT),b"verified pending receipt").unwrap();
        fs::write(root.join("state"),b"owner file").unwrap();
        let err=publish_unified_receipt(&game).unwrap_err();
        assert!(err.contains("unsafe installation state directory"));
        assert_eq!(fs::read(root.join("state")).unwrap(),b"owner file");
        assert_eq!(fs::read(game.join(INSTALL_RECEIPT)).unwrap(),b"verified pending receipt");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn first_launch_creates_log_once_then_disables_logging_without_erasing_diagnostics(){
        let root=temp();
        let state=root.join("state");
        fs::create_dir(&state).unwrap();
        fs::write(state.join(INSTALL_RECEIPT),b"owner CD receipt").unwrap();
        prepare_first_launch_ini(&root).unwrap();
        let ini=root.join("CrimsonSkiesFix.ini");
        assert_eq!(fs::read_to_string(&ini).unwrap(),initial_unified_ini());
        assert!(regular(&state.join(FIRST_LAUNCH_MARKER)));
        fs::write(root.join("CrimsonSkiesFix.log"),b"first game diagnostics").unwrap();
        assert!(!finish_first_launch_log(&root,1).unwrap()); // crash/error
        assert_eq!(fs::read_to_string(&ini).unwrap(),initial_unified_ini());
        assert!(regular(&state.join(FIRST_LAUNCH_MARKER)));
        assert!(finish_first_launch_log(&root,0).unwrap());
        assert_eq!(fs::read_to_string(&ini).unwrap(),quiet_unified_ini());
        assert!(!any_entry(&state.join(FIRST_LAUNCH_MARKER)));
        assert!(!any_entry(&state.join("CrimsonSkiesFix.ini.first-launch.pending")));
        assert_eq!(fs::read(state.join(INSTALL_RECEIPT)).unwrap(),b"owner CD receipt");
        assert_eq!(fs::read(root.join("CrimsonSkiesFix.log")).unwrap(),b"first game diagnostics");
        prepare_first_launch_ini(&root).unwrap(); // subsequent launch cannot re-arm
        assert_eq!(fs::read_to_string(&ini).unwrap(),quiet_unified_ini());
        assert!(!finish_first_launch_log(&root,0).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn manually_supplied_log_one_ini_remains_active_even_if_identical_to_generated_template(){
        let root=temp();
        let ini=root.join("CrimsonSkiesFix.ini");
        fs::write(&ini,initial_unified_ini()).unwrap();
        prepare_first_launch_ini(&root).unwrap();
        assert!(!any_entry(&root.join("state").join(FIRST_LAUNCH_MARKER)));
        fs::write(root.join("CrimsonSkiesFix.log"),b"manual logging").unwrap();
        assert!(!finish_first_launch_log(&root,0).unwrap());
        assert_eq!(fs::read_to_string(&ini).unwrap(),initial_unified_ini());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn first_launch_user_edits_override_automatic_log_change(){
        let root=temp();
        prepare_first_launch_ini(&root).unwrap();
        let ini=root.join("CrimsonSkiesFix.ini");
        let edited=format!("{}; user chose to preserve logging\n",initial_unified_ini());
        fs::write(&ini,&edited).unwrap();
        fs::write(root.join("CrimsonSkiesFix.log"),b"first game diagnostics").unwrap();
        assert!(!finish_first_launch_log(&root,0).unwrap());
        assert_eq!(fs::read_to_string(&ini).unwrap(),edited);
        assert!(!any_entry(&root.join("state").join(FIRST_LAUNCH_MARKER)));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn first_launch_preserves_runtime_lighting_confirmation_and_disables_log(){
        let root=temp();
        prepare_first_launch_ini(&root).unwrap();
        let ini=root.join("CrimsonSkiesFix.ini");
        let with_marker=initial_unified_ini().to_owned()+LIGHTING_CONFIRMATION_SUFFIX;
        fs::write(&ini,&with_marker).unwrap();
        fs::write(root.join("CrimsonSkiesFix.log"),b"first game diagnostics").unwrap();
        assert!(!finish_first_launch_log(&root,1).unwrap());
        assert_eq!(fs::read_to_string(&ini).unwrap(),with_marker);
        assert!(finish_first_launch_log(&root,0).unwrap());
        assert_eq!(fs::read_to_string(&ini).unwrap(),quiet_unified_ini()+LIGHTING_CONFIRMATION_SUFFIX);
        assert!(!any_entry(&root.join("state").join(FIRST_LAUNCH_MARKER)));
        assert_eq!(fs::read(root.join("CrimsonSkiesFix.log")).unwrap(),b"first game diagnostics");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn lighting_suffix_does_not_authorize_other_ini_edits(){
        let edited=initial_unified_ini().replace("FPSLimit=60","FPSLimit=30")+LIGHTING_CONFIRMATION_SUFFIX;
        assert!(managed_quiet_ini(edited.as_bytes()).is_none());
        let edited=initial_unified_ini().to_owned()+LIGHTING_CONFIRMATION_SUFFIX+"Custom=1\n";
        assert!(managed_quiet_ini(edited.as_bytes()).is_none());
        let edited=initial_unified_ini().to_owned()+LIGHTING_CONFIRMATION_SUFFIX+LIGHTING_CONFIRMATION_SUFFIX;
        assert!(managed_quiet_ini(edited.as_bytes()).is_none());
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn first_launch_does_not_auto_disable_if_no_game_session_log_exists(){
        let root=temp();
        prepare_first_launch_ini(&root).unwrap();
        assert!(!finish_first_launch_log(&root,0).unwrap());
        assert_eq!(fs::read_to_string(root.join("CrimsonSkiesFix.ini")).unwrap(),
            initial_unified_ini());
        assert!(regular(&root.join("state").join(FIRST_LAUNCH_MARKER)));
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(feature="unified-fix-install")]
    #[test]
    fn unsafe_state_path_refuses_first_launch_without_touching_user_content(){
        let root=temp();
        fs::write(root.join("state"),b"owner data").unwrap();
        assert!(prepare_first_launch_ini(&root).is_err());
        assert!(!any_entry(&root.join("CrimsonSkiesFix.ini")));
        assert_eq!(fs::read(root.join("state")).unwrap(),b"owner data");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_required_media_blocks_new_install(){
        let root=temp();
        let game=root.join("CrimsonSkiesFix");
        let error=auto_prepare(&root,&game).unwrap_err();
        #[cfg(not(feature="native-usde-updater"))]
        assert!(error.contains("approved decrypted Crimson Skies 1.02 EXE"));
        #[cfg(feature="native-usde-updater")]
        {
            assert!(error.contains("original game CD image"),"{error}");
            assert!(!error.contains("approved decrypted"),"{error}");
        }
        assert!(!game.exists());
        assert!(!root.join(REBUILT).exists());
        fs::remove_dir_all(root).unwrap();
    }
}
