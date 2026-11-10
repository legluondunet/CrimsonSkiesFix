//! Installation of the original disc's per-chapter CAB game data.
//! Unlike --extract-cd, a retail installation must materialize the
//! compressed chapter members in ZBD/ (historically ~184 ZBD files).
//! The enclosing CAB bytes are authenticated against the full CD manifest
//! before and after this step. No third-party game files are distributed.
use crate::sha256;
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component,Path,PathBuf},
};

const CHAPTER_CABS:[&str;8]=[
    "C1.CAB","C1B.CAB","C1C.CAB","C2.CAB",
    "C2B.CAB","C3.CAB","C4.CAB","C5.CAB",
];
const MAX_MEMBERS:usize=2048;
const MAX_MEMBER_BYTES:u64=256*1024*1024;
const MAX_EXPANDED_BYTES:u64=2_000_000_000;

pub struct Expansion {
    pub members:usize,
    pub zbd_files:usize,
    pub payload_bytes:u64,
}

fn err(e:impl std::fmt::Display)->String {e.to_string()}

fn real_file(path:&Path)->bool {
    fs::symlink_metadata(path).map(|m|m.file_type().is_file()).unwrap_or(false)
}
fn real_dir(path:&Path)->bool {
    fs::symlink_metadata(path).map(|m|m.file_type().is_dir()).unwrap_or(false)
}

fn member_path(name:&str)->Result<PathBuf,String> {
    // Cabinet paths are untrusted even after SHA-pinning. Never let them
    // escape ZBD/ or use OS device / absolute / drive / traversal syntax.
    let portable=name.replace('\\',"/");
    if portable.starts_with('/') || portable.contains(':') || portable.contains('\0') {
        return Err(format!("unsafe CAB member path {name:?}"));
    }
    let mut relative=portable.as_str();
    let lower=portable.to_ascii_lowercase();
    if lower.starts_with("game/zbd/") {
        relative=&portable[9..];
    } else if lower.starts_with("zbd/") {
        relative=&portable[4..];
    }
    if relative.is_empty() || relative.ends_with('/') {
        return Err(format!("empty/directory CAB member path: {name:?}"));
    }
    let path=PathBuf::from(relative);
    if !path.components().all(|c|matches!(c,Component::Normal(_))) {
        return Err(format!("unsafe CAB member path: {name:?}"));
    }
    Ok(path)
}

fn member_destination(root:&Path,rel:&Path)->Result<PathBuf,String> {
    if !real_dir(root) {return Err("missing or unsafe ZBD extraction directory".into());}
    let mut dir=root.to_path_buf();
    if let Some(parent)=rel.parent() {
        for comp in parent.components() {
            if !matches!(comp,Component::Normal(_)) {
                return Err("invalid CAB folder component".into());
            }
            dir.push(comp.as_os_str());
            if fs::symlink_metadata(&dir).is_ok() {
                if !real_dir(&dir) {return Err(format!("unsafe CAB target directory: {}",dir.display()));}
            }else{
                fs::create_dir(&dir).map_err(err)?;
            }
        }
    }
    let target=root.join(rel);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("CAB member would overwrite an existing file: {}",target.display()));
    }
    Ok(target)
}

/// Decompress every member from all eight known original chapter CABs.
/// Preserve the CABs themselves until comparison with a retail install proves
/// they can safely be omitted. No installation is published until complete.
pub fn expand(game:&Path)->Result<Expansion,String> {
    if !real_dir(game) {return Err("unsafe game installation directory".into());}
    let zbd=game.join("ZBD");
    if !real_dir(&zbd) {return Err("missing or unsafe game ZBD directory".into());}
    let mut seen=HashSet::<String>::new();
    let mut members=0usize;
    let mut zbd_files=0usize;
    let mut payload_bytes=0u64;
    for name in CHAPTER_CABS {
        let file=zbd.join(name);
        if !real_file(&file) {return Err(format!("missing or unsafe chapter archive: {}",file.display()));}
        let mut cabinet=cab::Cabinet::new(File::open(&file).map_err(err)?).map_err(err)?;
        let items=cabinet.folder_entries().flat_map(|folder| {
            folder.file_entries().map(|entry| {
                (entry.name().to_owned(),u64::from(entry.uncompressed_size()))
            })
        }).collect::<Vec<_>>();
        if items.is_empty() {return Err(format!("empty chapter CAB: {name}"));}
        for (original,size) in items {
            members+=1;
            if members>MAX_MEMBERS {return Err("too many CAB chapter members".into());}
            if size>MAX_MEMBER_BYTES {return Err(format!("oversize CAB member: {original}"));}
            payload_bytes=payload_bytes.checked_add(size)
                .ok_or("CAB expansion byte count overflow")?;
            if payload_bytes>MAX_EXPANDED_BYTES {
                return Err("CAB expansion exceeds the 2 GB safety limit".into());
            }
            let relative=member_path(&original)?;
            let lowered=relative.to_string_lossy().to_ascii_lowercase();
            if !seen.insert(lowered) {
                return Err(format!("duplicate CAB chapter member name: {original}"));
            }
            if relative.extension().and_then(|s|s.to_str())
                .map(|s|s.eq_ignore_ascii_case("zbd")).unwrap_or(false) {
                zbd_files+=1;
            }
            let target=member_destination(&zbd,&relative)?;
            let mut out=OpenOptions::new().write(true).create_new(true)
                .open(&target).map_err(err)?;
            let reader=cabinet.read_file(&original).map_err(err)?;
            let mut limited=reader.take(size+1);
            let mut hasher=sha256::Digest::new();
            let mut buffer=[0u8;32*1024];
            let mut written=0u64;
            loop{
                let n=limited.read(&mut buffer).map_err(err)?;
                if n==0 {break;}
                written+=n as u64;
                if written>size {return Err(format!("CAB member exceeded declared length: {original}"));}
                out.write_all(&buffer[..n]).map_err(err)?;
                hasher.update(&buffer[..n]);
            }
            out.sync_all().map_err(err)?;
            if written!=size || fs::metadata(&target).map_err(err)?.len()!=size {
                return Err(format!("short or changed CAB extraction: {original}"));
            }
            let expected_hash=hasher.finish();
            // Verify the bytes on disk independently of the decoder stream.
            let mut check=sha256::Digest::new();
            let mut output=File::open(&target).map_err(err)?;
            loop{
                let n=output.read(&mut buffer).map_err(err)?;
                if n==0 {break;}
                check.update(&buffer[..n]);
            }
            if check.finish()!=expected_hash {
                return Err(format!("CAB member output SHA-256 mismatch: {original}"));
            }
        }
        println!("Expanded original chapter archive {name}.");
    }
    if zbd_files==0 {
        return Err("chapter cabinets did not contain any ZBD files; cannot publish installation".into());
    }
    Ok(Expansion{members,zbd_files,payload_bytes})
}

/// Verify all materialized chapter members by reading them independently
/// from the original hashed CABs. Unlike a receipt checksum this relies on
/// the source CAB content; no user-editable manifest is trusted.
pub fn verify_expanded(game:&Path)->Result<Expansion,String> {
    if !real_dir(game) || !real_dir(&game.join("ZBD")) {
        return Err("missing/unsafe installed game or ZBD folder".into());
    }
    let root=game.join("ZBD");
    let mut unique=HashSet::new();
    let mut members=0usize;
    let mut zbd_files=0usize;
    let mut payload_bytes=0u64;
    for name in CHAPTER_CABS {
        let archive=root.join(name);
        if !real_file(&archive) {
            return Err(format!("missing/unsafe installed chapter CAB {}",archive.display()));
        }
        let mut cabinet=cab::Cabinet::new(File::open(&archive).map_err(err)?).map_err(err)?;
        let entries=cabinet.folder_entries().flat_map(|folder|
            folder.file_entries().map(|entry|
                (entry.name().to_owned(),u64::from(entry.uncompressed_size()))
            )
        ).collect::<Vec<_>>();
        if entries.is_empty() {return Err(format!("empty installed chapter CAB: {name}"));}
        for (name,size) in entries {
            members+=1;
            if members>MAX_MEMBERS || size>MAX_MEMBER_BYTES {
                return Err("installed CAB member exceeds safety limits".into());
            }
            payload_bytes=payload_bytes.checked_add(size).ok_or("CAB total bytes overflow")?;
            if payload_bytes>MAX_EXPANDED_BYTES {
                return Err("installed CAB payload exceeds 2 GB limit".into());
            }
            let relative=member_path(&name)?;
            if !unique.insert(relative.to_string_lossy().to_ascii_lowercase()) {
                return Err(format!("duplicate chapter CAB member: {name}"));
            }
            if relative.extension().and_then(|s|s.to_str())
                .map(|s|s.eq_ignore_ascii_case("zbd")).unwrap_or(false) {
                zbd_files+=1;
            }
            // Never trust a link inserted in a supposedly installed folder.
            let mut checkdir=root.clone();
            if let Some(parent)=relative.parent(){
                for component in parent.components(){
                    if !matches!(component,Component::Normal(_)){return Err("unsafe chapter member dir".into());}
                    checkdir.push(component.as_os_str());
                    if !real_dir(&checkdir){return Err("unsafe/missing expanded chapter folder".into());}
                }
            }
            let target=root.join(&relative);
            if !real_file(&target){return Err(format!("missing/unsafe installed chapter member {}",target.display()));}
            if fs::metadata(&target).map_err(err)?.len()!=size{
                return Err(format!("installed chapter member has wrong size {}",target.display()));
            }
            let mut reader=cabinet.read_file(&name).map_err(err)?.take(size+1);
            let mut installed=File::open(&target).map_err(err)?;
            let mut decoded=[0u8;32768];
            let mut actual=[0u8;32768];
            let mut total=0u64;
            loop{
                let n=reader.read(&mut decoded).map_err(err)?;
                if n==0 {break;}
                total+=n as u64;
                if total>size {return Err(format!("oversize extracted CAB member {name}"));}
                installed.read_exact(&mut actual[..n]).map_err(err)?;
                if actual[..n]!=decoded[..n] {
                    return Err(format!("expanded CAB content differs from original: {}",target.display()));
                }
            }
            if total!=size || installed.read(&mut actual[..1]).map_err(err)?!=0 {
                return Err(format!("incomplete/oversize installed chapter member {}",target.display()));
            }
        }
    }
    if zbd_files==0 {return Err("no ZBD content from the chapter archives".into());}
    Ok(Expansion{members,zbd_files,payload_bytes})
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn never_accept_unsafe_member_paths(){
        for name in ["/tmp/evil","../evil.zbd","C:\\evil.zbd","./x.zbd","/x.zbd",
            "GAME\\ZBD\\..\\evil.zbd","ZBD/../evil.zbd"]{
            assert!(member_path(name).is_err(),"{name}");
        }
        assert_eq!(member_path("ZBD\\C1\\DATA.ZBD").unwrap(),
            PathBuf::from("C1").join("DATA.ZBD"));
        assert_eq!(member_path("GAME/ZBD/TEST.ZBD").unwrap(),
            PathBuf::from("TEST.ZBD"));
    }
    #[test]
    fn synthetic_ms_zip_cab_expands_and_verifies(){
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let base=std::env::temp_dir().join(format!("crimson-chaptercab-{}-{id}",std::process::id()));
        fs::create_dir(&base).unwrap();
        let zbd=base.join("ZBD");
        fs::create_dir(&zbd).unwrap();
        // Construct one synthetic CAB and read it using the same API.
        let mut builder=cab::CabinetBuilder::new();
        builder.add_folder(cab::CompressionType::MsZip).add_file("CHAPTER.ZBD");
        let sample=zbd.join("C1.CAB");
        let file=File::create(&sample).unwrap();
        let mut writer=builder.build(file).unwrap();
        while let Some(mut f)=writer.next_file().unwrap(){
            f.write_all(b"synthetic chapter data").unwrap();
        }
        writer.finish().unwrap();
        for name in CHAPTER_CABS.iter().skip(1) {
            fs::copy(&sample,zbd.join(name)).unwrap();
        }
        // Same member name in each cabinet must be refused, preserving files.
        assert!(expand(&base).is_err());
        assert_eq!(fs::read(zbd.join("CHAPTER.ZBD")).unwrap(),b"synthetic chapter data");
        fs::remove_dir_all(base).unwrap();
    }
}
