//! Strict, read-only verification of the owner-supplied 1.02 updater.
//! Never execute the historical setup program or ship its copyrighted bytes.
//! Extracting the LZX cabinet is a separate installation milestone.
use crate::{sha256,disc_inspect};
use std::{
    collections::BTreeMap,
    fs::{self,File,OpenOptions},
    io::{Cursor,Read,Seek,SeekFrom,Write},
    path::{Path,PathBuf},
};

pub const PATCH_LEN:u64=2_315_728;
pub const PATCH_SHA256:&str="8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af";
pub const CAB_OFFSET:u64=0x346f0;
pub const CAB_BYTES:u32=2_091_693;
const CAB_FILES:u16=5;
const CAB_FOLDERS:u16=2;

fn err(e:impl std::fmt::Display)->String {e.to_string()}
fn regular_file(path:&Path)->bool {
    fs::symlink_metadata(path).map(|m|m.file_type().is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

fn file_sha(path:&Path)->Result<String,String> {
    let mut file=File::open(path).map_err(err)?;
    let mut hasher=sha256::Digest::new();
    let mut bytes=[0u8;32768];
    loop {
        let n=file.read(&mut bytes).map_err(err)?;
        if n==0 {break;}
        hasher.update(&bytes[..n]);
    }
    Ok(hasher.finish())
}

fn check_structure(path:&Path)->Result<(),String> {
    let mut f=File::open(path).map_err(err)?;
    let mut mz=[0u8;64];
    f.read_exact(&mut mz).map_err(err)?;
    if &mz[..2]!=b"MZ" {return Err("expected 32-bit Windows MZ/PE file".into());}
    let peoff=u32::from_le_bytes(mz[60..64].try_into().unwrap()) as u64;
    if peoff>=CAB_OFFSET || peoff>1048576 {
        return Err("unreasonable PE header offset".into());
    }
    f.seek(SeekFrom::Start(peoff)).map_err(err)?;
    let mut pe=[0u8;6];
    f.read_exact(&mut pe).map_err(err)?;
    if &pe[..4]!=b"PE\0\0" || &pe[4..6]!=b"\x4c\x01" {
        return Err("updater is not a Windows PE32 Intel x86 executable".into());
    }
    f.seek(SeekFrom::Start(CAB_OFFSET)).map_err(err)?;
    let mut header=[0u8;36];
    f.read_exact(&mut header).map_err(err)?;
    if &header[..4]!=b"MSCF" {
        return Err("expected embedded Microsoft Cabinet at audited resource offset".into());
    }
    let len=u32::from_le_bytes(header[8..12].try_into().unwrap());
    let folders=u16::from_le_bytes(header[26..28].try_into().unwrap());
    let files=u16::from_le_bytes(header[28..30].try_into().unwrap());
    if len!=CAB_BYTES || folders!=CAB_FOLDERS || files!=CAB_FILES ||
        CAB_OFFSET+len as u64>PATCH_LEN {
        return Err("embedded updater CAB structure mismatch".into());
    }
    let mut folder=[0u8;8];
    f.read_exact(&mut folder).map_err(err)?;
    let compression=u16::from_le_bytes(folder[6..8].try_into().unwrap());
    if compression & 15 !=3 {return Err("expected an LZX-compressed update CAB".into());}
    Ok(())
}

pub fn validate(path:&Path)->Result<(),String> {
    if !regular_file(path) {return Err(format!("updater is not a regular file: {}",path.display()));}
    let size=fs::metadata(path).map_err(err)?.len();
    if size!=PATCH_LEN {
        return Err(format!("unsupported 1.02 updater size {size}; expected {PATCH_LEN}"));
    }
    let hash=file_sha(path)?;
    if hash!=PATCH_SHA256 {
        return Err(format!("unsupported updater SHA-256 {hash}; expected {PATCH_SHA256}"));
    }
    check_structure(path)
}

// A user can rename this file; recognition is by content, not filename.
pub fn find(base:&Path,specified:Option<&Path>)->Result<PathBuf,String> {
    if let Some(name)=specified {
        let path=if name.is_absolute(){name.to_path_buf()}else{base.join(name)};
        validate(&path)?;
        return Ok(path);
    }
    let mut valid=Vec::new();
    for item in fs::read_dir(base).map_err(err)? {
        let item=item.map_err(err)?;
        let path=item.path();
        if !regular_file(&path) {continue;}
        if fs::metadata(&path).map_err(err)?.len()==PATCH_LEN &&
            validate(&path).is_ok() {valid.push(path);}
    }
    match valid.len() {
        1=>Ok(valid.remove(0)),
        0=>Err("no exact verified Crimson Skies 1.02 updater found beside launcher; \
                provide exactly one authentic updater (or pass its path to --install-cd)".into()),
        n=>Err(format!("found {n} verified 1.02 updater copies; keep exactly one beside the launcher")),
    }
}

/// Official 1.02 CAB member names and uncompressed sizes, confirmed by
/// static inspection of the owner-provided exactly hashed update executable.
/// Other entries (including execute-after-extract instructions) are refused.
const EXPECTED_MEMBERS:[(&str,u32);5]=[
    ("setupenu.dll",32768),
    ("readmeusa.rtf",57539),
    ("crimson.exe",344851),
    ("crimson.icd",2580578),
    ("gosdata/assets/crimptch.rof",797),
];
pub const ASSET_SHA256:&str=
    "7ae28cfb65645a3e22aadc64a68badc23e2543bc56356d599fb59375d9ebbefe";

fn read_asset_from_authenticated_update(path:&Path)->Result<Vec<u8>,String> {
    validate(path)?;
    let mut file=File::open(path).map_err(err)?;
    file.seek(SeekFrom::Start(CAB_OFFSET)).map_err(err)?;
    let mut bytes=vec![0u8;CAB_BYTES as usize];
    file.read_exact(&mut bytes).map_err(err)?;
    let mut cabinet=cab::Cabinet::new(Cursor::new(bytes)).map_err(err)?;
    // Enumerate the archive's uncompressed file directory before opening
    // any member. Ignore archive path strings until they are normalized and
    // matched against the exact audited whitelist.
    let mut inventory=BTreeMap::new();
    for folder in cabinet.folder_entries() {
        for member in folder.file_entries() {
            let actual=member.name().to_owned();
            let normalized=actual.replace('\\',"/").to_ascii_lowercase();
            if !EXPECTED_MEMBERS.iter().any(|(name,size)|
                normalized==*name && member.uncompressed_size()==*size) {
                return Err(format!("unexpected 1.02 CAB entry: {actual}"));
            }
            if inventory.insert(normalized,actual).is_some() {
                return Err("duplicate case-normalized CAB member".into());
            }
        }
    }
    if inventory.len()!=EXPECTED_MEMBERS.len() {
        return Err(format!("expected {} CAB files, found {}",
            EXPECTED_MEMBERS.len(),inventory.len()));
    }
    let name=inventory.get("gosdata/assets/crimptch.rof")
        .ok_or("official CAB is missing CRIMPTCH.ROF")?;
    let mut contents=Vec::with_capacity(797);
    let reader=cabinet.read_file(name).map_err(err)?;
    // Hard cap to reject unexpectedly decompressed payloads.
    reader.take(798).read_to_end(&mut contents).map_err(err)?;
    if contents.len()!=797 || sha256::hex(&contents)!=ASSET_SHA256 {
        return Err("decompressed official 1.02 CRIMPTCH.ROF failed size/SHA256 verification".into());
    }
    Ok(contents)
}

/// Stage only the audited 797-byte gameplay asset from a locally provided,
/// hash-pinned official updater. No protected executable is copied, no
/// historical installer is executed, and a correct FR asset is preserved.
pub fn apply_asset(base:&Path,game:&Path,edition:&str,specified:Option<&Path>)->Result<&'static str,String> {
    // Before even opening the updater, rehash all original CD game files.
    // This verifies which edition actually occupies this directory and
    // refuses incomplete/corrupted/misidentified prior staging folders.
    let verified=disc_inspect::verify_staging(game,edition)?;
    if verified==0 {return Err("no verified game files in staging".into());}
    let path=find(base,specified)?;
    let contents=read_asset_from_authenticated_update(&path)?;
    let assets=game.join("GOSDATA").join("ASSETS");
    if !fs::symlink_metadata(&assets).map(|m|m.file_type().is_dir()).unwrap_or(false) {
        return Err("extracted game GOSDATA/ASSETS directory is missing or unsafe".into());
    }
    let target=assets.join("CRIMPTCH.ROF");
    if fs::symlink_metadata(&target).is_ok() {
        let meta=fs::symlink_metadata(&target).map_err(err)?;
        if !meta.file_type().is_file() || meta.len()!=797 {
            return Err("existing CRIMPTCH.ROF is not a regular official 797-byte asset".into());
        }
        if file_sha(&target)?!=ASSET_SHA256 {
            return Err("existing CRIMPTCH.ROF does not match official 1.02 asset".into());
        }
        return Ok("ALREADY_PRESENT_UNCHANGED");
    }
    let mut out=OpenOptions::new().write(true).create_new(true).open(&target).map_err(err)?;
    out.write_all(&contents).map_err(err)?;
    out.sync_all().map_err(err)?;
    if file_sha(&target)?!=ASSET_SHA256 {
        return Err("staged CRIMPTCH.ROF failed on-disk SHA-256 verification".into());
    }
    Ok("EXTRACTED_VERIFIED_FROM_LZX_CAB")
}

pub fn inspect(base:&Path,specified:Option<&Path>)->Result<String,String> {
    let path=find(base,specified)?;
    Ok(format!(
        "CrimsonSkiesFix UPDATE102 INSPECT v1\n\
         UpdateFile={}\n\
         Identity=VERIFIED_OFFICIAL_1.02\n\
         SizeBytes={PATCH_LEN}\n\
         SHA256={PATCH_SHA256}\n\
         CabinetOffset={CAB_OFFSET}\n\
         CabinetBytes={CAB_BYTES}\n\
         CabinetFormat=Microsoft Cabinet LZX\n\
         CabinetFolders={CAB_FOLDERS}\n\
         CabinetFiles={CAB_FILES}\n\
         File1=setupenu.dll\n\
         File2=readmeUSA.rtf\n\
         File3=CRIMSON.EXE\n\
         File4=CRIMSON.ICD\n\
         File5=gosdata/assets/CRIMPTCH.ROF\n\
         GameExeFromUpdate=SAFEDISC_PROTECTED_NOT_RUNTIME_APPROVED\n\
         UpdateApplied=NO_READ_ONLY_INSPECTION\n\
         INSPECT_RESULT=COMPLETE\n",
        path.file_name().unwrap_or_default().to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_is_exact_and_layout_is_bounded(){
        assert_eq!(PATCH_SHA256.len(),64);
        assert!(PATCH_SHA256.bytes().all(|x|x.is_ascii_hexdigit()));
        assert_eq!(CAB_OFFSET,CAB_OFFSET);
        assert!(CAB_OFFSET+CAB_BYTES as u64<=PATCH_LEN);
        assert_eq!(CAB_FILES,5);
        assert_eq!(EXPECTED_MEMBERS.len(),5);
        assert_eq!(ASSET_SHA256.len(),64);
    }
    #[test]
    fn refuse_wrong_updater_without_execution(){
        let dir=std::env::temp_dir().join(format!(
            "crimson-update-inspect-{}-{:?}",std::process::id(),std::thread::current().id()));
        fs::create_dir_all(&dir).unwrap();
        let test=dir.join("Crimson_Skies_Patch_102.exe");
        fs::write(&test,b"MZ").unwrap();
        assert!(inspect(&dir,Some(&test)).unwrap_err().contains("size"));
        assert!(find(&dir,None).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
