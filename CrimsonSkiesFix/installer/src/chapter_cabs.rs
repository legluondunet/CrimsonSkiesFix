//! Installation of the original disc's per-chapter CAB game data.
//! Unlike --extract-cd, a retail installation must materialize the
//! compressed chapter members in ZBD/ (historically ~184 ZBD files).
//! CAB bytes are authenticated by the full original-CD manifest before
//! expansion, then the expanded files are sealed and the CABs deleted. No third-party game files are distributed.
use crate::sha256;
use std::{
    collections::{BTreeMap,HashSet},
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

/// Generated *after* a fully authenticated CD extraction, CAB decompression,
/// and byte-for-byte verification. This manifest contains filenames and SHA256
/// of the installed chapter payloads, not the proprietary chapter payloads.
pub const INSTALLED_MANIFEST:&str="CrimsonSkiesFix.chapters.tsv";

pub struct Finalized {
    pub members:usize,
    pub zbd_files:usize,
    pub payload_bytes:u64,
    pub manifest_sha:String,
    pub short_name_candidates:usize,
}

fn hash_file(path:&Path)->Result<String,String>{
    if !real_file(path){return Err(format!("unsafe or missing file: {}",path.display()));}
    let mut stream=File::open(path).map_err(err)?;
    let mut hasher=sha256::Digest::new();
    let mut buffer=[0u8;32768];
    loop{
        let n=stream.read(&mut buffer).map_err(err)?;
        if n==0{break;}
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finish())
}

fn possible_short_alias(path:&Path)->bool{
    // No inference of a long filename from an 8.3 alias is safe. Record
    // candidates for matching against an authentic installed-game file list.
    path.components().any(|c|c.as_os_str().to_string_lossy().contains('~'))
}

/// A CAB-free game tree cannot independently re-decompress original CAB bytes.
/// Hash each verified installed member into a compact local receipt instead,
/// and carry its manifest checksum into the installation receipt.
/// The final SHA check detects accidental corruption; this is not a signature
/// against intentional rewriting of both manifests by an attacker.
pub fn finalize_without_archives(game:&Path)->Result<Finalized,String>{
    let expansion=verify_expanded(game)?;
    let zbd=game.join("ZBD");
    let mut listed=BTreeMap::<String,(String,String,u64,String)>::new();
    let mut candidates=0usize;
    for source in CHAPTER_CABS {
        let path=zbd.join(source);
        if !real_file(&path){return Err(format!("missing original chapter CAB: {source}"));}
        let cabinet=cab::Cabinet::new(File::open(&path).map_err(err)?).map_err(err)?;
        for folder in cabinet.folder_entries(){
            for entry in folder.file_entries(){
                let original=entry.name();
                let relative=member_path(original)?;
                if relative.extension().and_then(|x|x.to_str())
                    .map(|x|x.eq_ignore_ascii_case("zbd"))!=Some(true) {
                    return Err(format!("unrecognized non-ZBD chapter member: {original}"));
                }
                let target=zbd.join(&relative);
                let bytes=fs::metadata(&target).map_err(err)?.len();
                let expected=u64::from(entry.uncompressed_size());
                if bytes!=expected {return Err(format!("chapter member size changed: {original}"));}
                let hash=hash_file(&target)?;
                let normalized=relative.to_string_lossy().replace('\\',"/");
                // Tabs/newlines are unsafe even when CAB names otherwise
                // produce valid operating-system paths.
                if original.contains(|x:char|x=='\t'||x=='\n'||x=='\r')
                    || normalized.contains(|x:char|x=='\t'||x=='\n'||x=='\r'){
                    return Err(format!("unsafe chapter member metadata: {original:?}"));
                }
                let key=normalized.to_ascii_lowercase();
                if listed.insert(key,(source.to_owned(),normalized,expected,hash)).is_some(){
                    return Err(format!("duplicate chapter member: {original}"));
                }
                if possible_short_alias(&relative){candidates+=1;}
            }
        }
    }
    if listed.len()!=expansion.members{
        return Err("chapter members changed since verification".into());
    }
    // Format: source CAB, installed relative path, byte length, SHA-256.
    // The archived name is the installed name until a verified long-name
    // mapping is sourced from an authentic retail installation.
    let mut manifest=String::from("#CrimsonSkiesFixChapterManifestV1\n");
    for (_, (source,rel,bytes,sha)) in &listed {
        manifest.push_str(&format!("{source}\t{rel}\t{bytes}\t{sha}\n"));
    }
    let manifest_path=game.join(INSTALLED_MANIFEST);
    let mut file=OpenOptions::new().write(true).create_new(true)
        .open(&manifest_path).map_err(err)?;
    file.write_all(manifest.as_bytes()).map_err(err)?;
    file.sync_all().map_err(err)?;
    let checksum=sha256::hex(manifest.as_bytes());
    if hash_file(&manifest_path)?!=checksum {
        return Err("chapter manifest checksum verification failed".into());
    }
    // Delete ONLY the eight authenticated temporary chapter archives and ONLY
    // in the new unpublished installation staging directory. All members and
    // the receipt are already written and verified. A deletion failure stops
    // publication and leaves staging for manual inspection.
    drop(file);
    for name in CHAPTER_CABS{
        let path=zbd.join(name);
        if !real_file(&path){return Err(format!("unsafe CAB removal target: {name}"));}
        fs::remove_file(&path).map_err(err)?;
    }
    let result=verify_without_archives(game,&checksum)?;
    if result.members!=expansion.members || result.zbd_files!=expansion.zbd_files
        || result.payload_bytes!=expansion.payload_bytes
        || result.short_name_candidates!=candidates {
        return Err("CAB-free stage disagrees with CAB-backed verification".into());
    }
    Ok(Finalized{members:expansion.members,zbd_files:expansion.zbd_files,
        payload_bytes:expansion.payload_bytes,manifest_sha:checksum,
        short_name_candidates:candidates})
}

/// Post-install verification uses the per-file SHA-256 manifest instead of
/// the eight source CABs, which are deliberately absent from the game tree.
pub fn verify_without_archives(game:&Path,expected_manifest_sha:&str)
    ->Result<Finalized,String>{
    if !real_dir(game)||!real_dir(&game.join("ZBD")){
        return Err("missing/unsafe CAB-free installation root".into());
    }
    let root=game.join("ZBD");
    for name in CHAPTER_CABS{
        if fs::symlink_metadata(root.join(name)).is_ok() {
            return Err(format!("temporary chapter archive still present: {name}"));
        }
    }
    let path=game.join(INSTALLED_MANIFEST);
    if !real_file(&path){return Err("missing/unsafe chapter install manifest".into());}
    if fs::metadata(&path).map_err(err)?.len()>MAX_MEMBERS as u64*160+64{
        return Err("chapter manifest exceeds bounded size".into());
    }
    let bytes=fs::read(&path).map_err(err)?;
    let sha=sha256::hex(&bytes);
    if sha!=expected_manifest_sha{
        return Err("chapter install manifest SHA-256 differs from verified installation receipt".into());
    }
    let content=std::str::from_utf8(&bytes).map_err(err)?;
    let mut lines=content.lines();
    if lines.next()!=Some("#CrimsonSkiesFixChapterManifestV1"){
        return Err("unsupported chapter manifest format".into());
    }
    let mut unique=HashSet::new();
    let mut total=0u64;
    let mut count=0usize;
    let mut candidates=0usize;
    for row in lines{
        let cells=row.split('\t').collect::<Vec<_>>();
        if cells.len()!=4 || !CHAPTER_CABS.contains(&cells[0]) {
            return Err("invalid chapter manifest row".into());
        }
        let relative=member_path(cells[1])?;
        if relative.extension().and_then(|x|x.to_str())
            .map(|x|x.eq_ignore_ascii_case("zbd"))!=Some(true){
            return Err("chapter manifest contains non-ZBD file".into());
        }
        let normalized=relative.to_string_lossy().to_ascii_lowercase();
        if !unique.insert(normalized){return Err("duplicate chapter manifest entry".into());}
        let expected=cells[2].parse::<u64>().map_err(err)?;
        if expected>MAX_MEMBER_BYTES {return Err("oversize manifest member".into());}
        count+=1;
        if count>MAX_MEMBERS{return Err("too many chapter manifest entries".into());}
        total=total.checked_add(expected).ok_or("chapter size overflow")?;
        if total>MAX_EXPANDED_BYTES{return Err("chapter total too large".into());}
        let target=root.join(&relative);
        if !real_file(&target) || fs::metadata(&target).map_err(err)?.len()!=expected{
            return Err(format!("missing or wrong-sized installed chapter file: {}",target.display()));
        }
        if cells[3].len()!=64 || !cells[3].bytes().all(|b|b.is_ascii_hexdigit())
            || hash_file(&target)?!=cells[3]{
            return Err(format!("installed chapter file SHA-256 mismatch: {}",target.display()));
        }
        if possible_short_alias(&relative){candidates+=1;}
    }
    if count==0{return Err("no verified installed chapter members".into());}
    Ok(Finalized{members:count,zbd_files:count,payload_bytes:total,
        manifest_sha:sha,short_name_candidates:candidates})
}

/// Diagnostic only: emits suspect 8.3 aliases and the authenticated
/// post-install SHA-256, for comparison with a genuine retail game tree.
/// No file is renamed based on an unverified guessed expansion.
pub fn short_name_audit(game:&Path,reference:Option<&Path>)->Result<String,String>{
    let path=game.join(INSTALLED_MANIFEST);
    if !real_file(&path){return Err("missing installed chapter manifest".into());}
    let raw=fs::read_to_string(&path).map_err(err)?;
    if raw.len()>MAX_MEMBERS*160+64 {return Err("chapter manifest size limit exceeded".into());}
    let mut out=String::from("CrimsonSkiesFix CHAPTER SHORT NAMES AUDIT v1\n");
    let mut candidates=0usize;
    for line in raw.lines().skip(1) {
        let fields=line.split('\t').collect::<Vec<_>>();
        if fields.len()!=4 {return Err("invalid installed chapter manifest entry".into());}
        let relative=member_path(fields[1])?;
        if possible_short_alias(&relative){
            candidates+=1;
            out.push_str(&format!("AliasCandidate={}\t{}\t{}\t{}\n",
                fields[0],fields[1],fields[2],fields[3]));
        }
    }
    if let Some(ref_dir)=reference {
        if !real_dir(ref_dir) {return Err("reference ZBD folder must be a real directory".into());}
        let mut hashes=BTreeMap::<String,Vec<String>>::new();
        let mut total=0usize;
        for entry in fs::read_dir(ref_dir).map_err(err)?{
            let item=entry.map_err(err)?;
            let name=item.file_name().to_string_lossy().to_string();
            if !name.to_ascii_lowercase().ends_with(".zbd"){continue;}
            if !real_file(&item.path()){return Err(format!("unsafe reference entry {name}"));}
            if name.contains(|c:char|c.is_control()||"/\\:".contains(c)){
                return Err("reference file has invalid path characters".into());
            }
            total+=1;
            if total>MAX_MEMBERS {return Err("too many reference ZBD files".into());}
            hashes.entry(hash_file(&item.path())?).or_default().push(name);
        }
        out.push_str(&format!("ReferenceZbdFiles={total}\n"));
        for line in raw.lines().skip(1) {
            let fields=line.split('\t').collect::<Vec<_>>();
            let old=fields[1];
            if !possible_short_alias(Path::new(old)){continue;}
            match hashes.get(fields[3]) {
                Some(names) if names.len()==1 => {
                    out.push_str(&format!("MappingCandidate={}\t{}\t{}\n",
                        old,names[0],fields[3]));
                }
                Some(names)=>out.push_str(&format!(
                    "AmbiguousAlias={}\t{} matches for {}\n",old,names.len(),fields[3])),
                None=>out.push_str(&format!("UnmatchedAlias={}\t{}\n",old,fields[3])),
            }
        }
    }
    out.push_str(&format!("ShortNameCandidates={candidates}\n\
        LongNameMapping=REQUIRES_VALIDATED_MAPPING\n\
        AUDIT_RESULT=COMPLETE\n"));
    Ok(out)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn alias_detection_is_diagnostic_not_filename_inference(){
        assert!(possible_short_alias(Path::new("SOMETH~1.ZBD")));
        assert!(!possible_short_alias(Path::new("CHAPTER_ONE.ZBD")));
    }
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
    fn verified_cab_members_survive_archive_pruning_and_detect_corruption(){
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let base=std::env::temp_dir().join(format!("crimson-cab-prune-{}-{id}",std::process::id()));
        let zbd=base.join("ZBD");
        fs::create_dir_all(&zbd).unwrap();
        for (n,name) in CHAPTER_CABS.iter().enumerate() {
            let mut builder=cab::CabinetBuilder::new();
            let membername=format!("CHAPTER{n}.ZBD");
            builder.add_folder(cab::CompressionType::MsZip)
                .add_file(&membername);
            let file=File::create(zbd.join(name)).unwrap();
            let mut writer=builder.build(file).unwrap();
            while let Some(mut f)=writer.next_file().unwrap(){
                f.write_all(format!("data from {name}").as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        let expanded=expand(&base).unwrap();
        assert_eq!(expanded.members,CHAPTER_CABS.len());
        let finalized=finalize_without_archives(&base).unwrap();
        for name in CHAPTER_CABS{
            assert!(!zbd.join(name).exists());
        }
        assert!(base.join(INSTALLED_MANIFEST).is_file());
        assert_eq!(verify_without_archives(&base,&finalized.manifest_sha).unwrap().members,8);
        fs::write(zbd.join("CHAPTER0.ZBD"),b"bad data").unwrap();
        assert!(verify_without_archives(&base,&finalized.manifest_sha).is_err());
        fs::remove_dir_all(base).unwrap();
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
