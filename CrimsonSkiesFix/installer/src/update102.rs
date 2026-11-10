//! Strict, read-only verification of the owner-supplied 1.02 updater.
//! Never execute the historical setup program or ship its copyrighted bytes.
//! Extracting the LZX cabinet is a separate installation milestone.
use crate::sha256;
use std::{
    fs::{self,File},
    io::{Read,Seek,SeekFrom},
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
                pass --inspect-update PATH to diagnose a specific file".into()),
        n=>Err(format!("found {n} verified 1.02 updater copies; pass --inspect-update PATH")),
    }
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
