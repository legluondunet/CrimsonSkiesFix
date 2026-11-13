//! Content-authenticated ISO9660/Joliet and CUE/BIN reader.
//! Edition identity uses the original file payloads, not the raw image size
//! or whole-image SHA-256. Full scan reports are retained only in tests.
use crate::sha256;
use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsStr,
    fs::{self,File,OpenOptions},
    io::{Read,Write,Seek,SeekFrom},
    path::{Component,Path,PathBuf},
};

const USER_SECTOR: usize = 2048;
const MAX_DIR_BYTES: u32 = 32*1024*1024;
const MAX_FILES: usize = 120000;
const MAX_DEPTH: usize = 32;

fn err(message: impl std::fmt::Display) -> String { message.to_string() }

fn safe_file(path:&Path)->Result<(),String> {
    let meta=fs::symlink_metadata(path).map_err(|e|format!("{}: {e}",path.display()))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() {
        return Err(format!("not a regular file: {}",path.display()));
    }
    Ok(())
}

fn has_ext(path:&Path, extension:&str)->bool {
    path.extension().and_then(OsStr::to_str)
        .map(|s|s.eq_ignore_ascii_case(extension)).unwrap_or(false)
}

fn select(base:&Path,specified:Option<&Path>)->Result<PathBuf,String> {
    if let Some(path)=specified {
        let resolved=if path.is_absolute() {path.to_path_buf()} else {base.join(path)};
        if !has_ext(&resolved,"iso") && !has_ext(&resolved,"cue") {
            return Err("expected an .iso or .cue file (BIN/IMG is referenced by CUE)".into());
        }
        safe_file(&resolved)?;
        return Ok(resolved);
    }
    let mut candidates=Vec::new();
    for item in fs::read_dir(base).map_err(err)? {
        let item=item.map_err(err)?;
        let path=item.path();
        if has_ext(&path,"iso") || has_ext(&path,"cue") {
            safe_file(&path)?;
            candidates.push(path);
        }
    }
    candidates.sort();
    match candidates.len() {
        1=>Ok(candidates.remove(0)),
        count=>Err(format!(
            "expected exactly one ISO or CUE beside CrimsonSkiesFix.exe; found {count}. \
             Place only one original image beside the launcher."
        ))
    }
}

fn parse_msf(text:&str)->Result<u64,String> {
    let fields=text.split(':').collect::<Vec<_>>();
    if fields.len()!=3 {return Err(format!("invalid CUE MM:SS:FF time: {text}"));}
    let n=fields.iter().map(|s|s.parse::<u64>()).collect::<Result<Vec<_>,_>>()
        .map_err(|_|format!("invalid CUE MM:SS:FF: {text}"))?;
    if n[1]>=60 || n[2]>=75 {return Err(format!("invalid CUE time units: {text}"));}
    n[0].checked_mul(60).and_then(|x|x.checked_add(n[1]))
        .and_then(|x|x.checked_mul(75)).and_then(|x|x.checked_add(n[2]))
        .ok_or_else(||"CUE time overflow".into())
}

#[derive(Clone)]
struct CueTrack {
    file:PathBuf,
    kind:String,
    file_format:String,
    index:Option<u64>,
    number:u32,
}
fn parse_cue_filename(input:&str)->Result<(String,String),String> {
    let rest=input.trim();
    let (name,tail)=if let Some(suffix)=rest.strip_prefix('"') {
        let end=suffix.find('"').ok_or("unterminated quoted FILE path")?;
        (&suffix[..end],suffix[end+1..].trim())
    } else {
        let i=rest.find(char::is_whitespace).ok_or("CUE FILE missing format")?;
        (&rest[..i],rest[i..].trim())
    };
    if name.is_empty() || tail.is_empty() {return Err("CUE FILE missing path or format".into());}
    Ok((name.to_owned(),tail.to_ascii_uppercase()))
}
fn resolve_cue_bin(base:&Path,name:&str)->Result<PathBuf,String> {
    // CUE FILE paths are relative to their own CUE location. Reject path
    // traversal and absolute paths, including Windows drive and UNC forms.
    if name.contains(':') || name.starts_with('/') || name.starts_with('\\') {
        return Err(format!("unsafe CUE FILE path: {name}"));
    }
    let portable=name.replace('\\',"/");
    let relative=Path::new(&portable);
    if !relative.components().all(|c|matches!(c,Component::Normal(_))) {
        return Err(format!("unsafe CUE FILE path: {name}"));
    }
    let path=base.join(relative);
    safe_file(&path)?;
    let root=fs::canonicalize(base).map_err(err)?;
    let actual=fs::canonicalize(&path).map_err(err)?;
    if !actual.starts_with(&root) {return Err(format!("CUE FILE escapes image folder: {name}"));}
    Ok(actual)
}

#[derive(Clone)]
struct ImageSource {
    file:PathBuf,
    sector_bytes:u64,
    user_offset:u64,
    first_byte:u64,
    end_byte:u64,
    format:String,
    tracks:usize,
}
impl ImageSource {
    fn sectors(&self)->u64 { (self.end_byte-self.first_byte)/self.sector_bytes }
    fn read_sector(&self,stream:&mut File,lba:u64)->Result<[u8;USER_SECTOR],String> {
        if lba>=self.sectors() {return Err(format!("CD sector {lba} outside data track"));}
        let pos=self.first_byte.checked_add(lba.checked_mul(self.sector_bytes)
            .ok_or("CD sector multiplication overflow")?)
            .and_then(|x|x.checked_add(self.user_offset))
            .ok_or("CD sector byte-offset overflow")?;
        if pos.checked_add(USER_SECTOR as u64).ok_or("sector offset overflow")?>self.end_byte {
            return Err(format!("short CD data sector at LBA={lba}"));
        }
        stream.seek(SeekFrom::Start(pos)).map_err(err)?;
        let mut bytes=[0u8;USER_SECTOR];
        stream.read_exact(&mut bytes).map_err(err)?;
        Ok(bytes)
    }
}
fn open_iso(path:&Path)->Result<ImageSource,String> {
    safe_file(path)?;
    let size=fs::metadata(path).map_err(err)?.len();
    if size<20*USER_SECTOR as u64 {return Err("ISO too small for ISO9660 descriptors".into());}
    Ok(ImageSource{
        file:path.to_path_buf(),sector_bytes:2048,user_offset:0,
        first_byte:0,end_byte:size,format:"ISO/2048".into(),tracks:1
    })
}
fn open_cue(path:&Path)->Result<ImageSource,String> {
    safe_file(path)?;
    let bytes=fs::read(path).map_err(err)?;
    let text=std::str::from_utf8(&bytes)
        .map_err(|_|"CUE text is not UTF-8/ASCII: please provide a UTF-8 CUE file")?
        .trim_start_matches('\u{feff}');
    let base=path.parent().ok_or("CUE has no parent directory")?;
    let mut active_file:Option<(PathBuf,String)>=None;
    let mut tracks:Vec<CueTrack>=Vec::new();
    for (i,line) in text.lines().enumerate() {
        let line=line.trim();
        if line.is_empty() || line.starts_with(';') {continue;}
        let mut words=line.splitn(2,char::is_whitespace);
        let op=words.next().unwrap_or("").to_ascii_uppercase();
        let val=words.next().unwrap_or("").trim();
        match op.as_str() {
            "REM"|"CATALOG"|"CDTEXTFILE"|"PERFORMER"|"TITLE"|"SONGWRITER"|"FLAGS"|"ISRC"|"PREGAP"|"POSTGAP"|"INDEX00" => {},
            "FILE"=>{
                let (name,format)=parse_cue_filename(val)
                    .map_err(|e|format!("CUE line {}: {e}",i+1))?;
                let bin=resolve_cue_bin(base,&name)
                    .map_err(|e|format!("CUE line {}: {e}",i+1))?;
                active_file=Some((bin,format));
            }
            "TRACK"=>{
                let fields=val.split_whitespace().collect::<Vec<_>>();
                if fields.len()!=2 {return Err(format!("CUE line {}: invalid TRACK",i+1));}
                let number=fields[0].parse::<u32>()
                    .map_err(|_|format!("CUE line {}: invalid track number",i+1))?;
                let (file,file_format)=active_file.clone()
                    .ok_or_else(||format!("CUE line {}: TRACK before FILE",i+1))?;
                tracks.push(CueTrack{
                    file,kind:fields[1].to_ascii_uppercase(),file_format,index:None,number
                });
            }
            "INDEX"=>{
                let fields=val.split_whitespace().collect::<Vec<_>>();
                if fields.len()!=2 {return Err(format!("CUE line {}: invalid INDEX",i+1));}
                let target=tracks.last_mut().ok_or("INDEX before TRACK")?;
                // CUE sheets from some imaging programs use INDEX 1
                // rather than INDEX 01. These are the same numeric index;
                // parse the number while retaining duplicate protection.
                let index=fields[0].parse::<u32>()
                    .map_err(|_|format!("CUE line {}: invalid INDEX number",i+1))?;
                if index==1 {
                    if target.index.replace(parse_msf(fields[1])?).is_some() {
                        return Err(format!("CUE line {}: repeated INDEX 01 (or 1)",i+1));
                    }
                }
            }
            _=>{}, // Metadata is not needed for scanning ISO9660 file payloads.
        }
    }
    let data_tracks=tracks.iter().enumerate()
        .filter(|(_,t)|t.kind.starts_with("MODE1/") || t.kind.starts_with("MODE2/"))
        .collect::<Vec<_>>();
    if data_tracks.len()!=1 {
        return Err(format!("CUE requires exactly one data track; found {}",data_tracks.len()));
    }
    let (data_index,data)=data_tracks[0];
    if data.file_format!="BINARY" {
        return Err(format!("data track file is not BINARY: {}",data.file_format));
    }
    let (sector_bytes,user_offset)=match data.kind.as_str() {
        "MODE1/2352" => (2352u64,16u64),
        "MODE1/2048" => (2048,0),
        "MODE2/2352" => (2352,24),
        "MODE2/2336" => (2336,8),
        _=>return Err(format!("unsupported data-track mode {}",data.kind)),
    };
    let start=data.index.ok_or("data track is missing INDEX 01")?;
    let physical_size=fs::metadata(&data.file).map_err(err)?.len();
    let first_byte=start.checked_mul(sector_bytes).ok_or("data track offset overflow")?;
    if first_byte>=physical_size {return Err("data track begins outside its BIN file".into());}
    let mut end_byte=physical_size;
    for next in tracks.iter().skip(data_index+1) {
        if next.file==data.file {
            // CUE audio tracks sharing the BIN have 2352-byte frames. End the
            // DATA track at the next track's INDEX 01, not the whole image.
            let next_index=next.index.ok_or("following CUE track missing INDEX 01")?;
            if sector_bytes!=2352 {
                return Err("mixed track types sharing a non-2352 BIN are unsupported".into());
            }
            end_byte=next_index.checked_mul(2352).ok_or("CUE end offset overflow")?;
            break;
        }
    }
    if end_byte>physical_size || end_byte<=first_byte {
        return Err("data track end is out of bounds".into());
    }
    Ok(ImageSource{
        file:data.file.clone(),sector_bytes,user_offset,first_byte,end_byte,
        format:format!("CUE/{} (track {})",data.kind,data.number),tracks:tracks.len()
    })
}

fn both_endian(bytes:&[u8])->Result<u32,String> {
    if bytes.len()<8 {return Err("truncated ISO 733 field".into());}
    let le=u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let be=u32::from_be_bytes(bytes[4..8].try_into().unwrap());
    if le!=be {return Err("ISO9660 extent/length endian copies disagree".into());}
    Ok(le)
}
#[derive(Clone)]
struct Entry {
    extent:u32,size:u32,name:String,is_directory:bool,multiextent:bool,
}
fn entry(record:&[u8],joliet:bool)->Result<Entry,String> {
    if record.len()<34 {return Err("truncated ISO9660 directory record".into());}
    let namelen=record[32] as usize;
    if 33+namelen>record.len() {return Err("ISO directory identifier exceeds record".into());}
    let raw=&record[33..33+namelen];
    let name=if namelen==1 && raw[0]<=1 {
        if raw[0]==0 {".".to_owned()}else{"..".to_owned()}
    }else if joliet {
        if raw.len()%2!=0 {return Err("odd-sized Joliet filename".into());}
        let v=raw.chunks_exact(2).map(|c|u16::from_be_bytes([c[0],c[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&v).map_err(|_|"invalid UCS-2 Joliet filename")?
    }else{
        if !raw.is_ascii(){return Err("non-ASCII primary ISO9660 filename".into());}
        String::from_utf8(raw.to_vec()).map_err(err)?
    };
    let name=if name=="."||name==".." {name} else {
        let no_version=if let Some((head,version))=name.rsplit_once(';') {
            if !version.is_empty() && version.chars().all(|c|c.is_ascii_digit()) {
                head.to_string()
            }else{name}
        }else{name};
        no_version.trim_end_matches('.').to_owned()
    };
    if name.is_empty() || name.contains(|c:char|c.is_control() || "/\\:*?\"<>|".contains(c)) {
        return Err(format!("unsafe ISO9660 path component: {name:?}"));
    }
    if !matches!(name.as_str(),"."|"..") && (name==".." || name.ends_with(' ')) {
        return Err("unsafe ISO9660 file name".into());
    }
    let flags=record[25];
    Ok(Entry{
        extent:both_endian(&record[2..10])?,
        size:both_endian(&record[10..18])?,
        name,is_directory:flags&2!=0,multiextent:flags&0x80!=0,
    })
}

struct Volume {
    label:String,filename_source:&'static str,root:Entry,
}
fn parse_volume(source:&ImageSource,file:&mut File)->Result<Volume,String> {
    let mut primary:Option<[u8;2048]>=None;
    let mut joliet:Option<[u8;2048]>=None;
    for lba in 16..64u64 {
        let desc=source.read_sector(file,lba)?;
        if &desc[1..6]!=b"CD001" || desc[6]!=1 {
            return Err(format!("invalid ISO9660 descriptor at LBA {lba}"));
        }
        match desc[0] {
            1=>if primary.is_none() {primary=Some(desc)},
            2=>if joliet.is_none() &&
                matches!(&desc[88..91],b"%/@"|b"%/C"|b"%/E") {
                    joliet=Some(desc);
                },
            255=>break,
            _=>{}
        }
    }
    let (desc,using_joliet)=if let Some(j)=joliet {
        (j,true)
    }else{(primary.ok_or("ISO9660 primary volume descriptor missing")?,false)};
    let label=if using_joliet {
        let chars=desc[40..72].chunks_exact(2)
            .map(|c|u16::from_be_bytes([c[0],c[1]])).collect::<Vec<_>>();
        String::from_utf16_lossy(&chars).trim().to_owned()
    }else{String::from_utf8_lossy(&desc[40..72]).trim().to_owned()};
    let length=desc[156] as usize;
    if !(34..=255).contains(&length) || 156+length>2048 {
        return Err("invalid ISO9660 root directory record".into());
    }
    let root=entry(&desc[156..156+length],using_joliet)?;
    if !root.is_directory {return Err("ISO root is not a directory".into());}
    Ok(Volume{label,filename_source:if using_joliet {"Joliet"}else{"ISO9660"},root})
}

#[derive(Clone)]
struct FileRecord {
    path:String,size:u32,sha:String,status:String,extent:u32,
}
struct Walker<'a> {
    image:&'a ImageSource,
    file:File,
    joliet:bool,
    results:BTreeMap<String,FileRecord>,
    visited:HashSet<(u32,u32)>,
}
impl Walker<'_> {
    fn walk(&mut self,extent:u32,bytes:u32,prefix:&str,depth:usize)->Result<(),String> {
        if depth>MAX_DEPTH {return Err("ISO9660 directory nesting exceeds safety limit".into());}
        if bytes>MAX_DIR_BYTES {return Err("ISO9660 directory exceeds 32 MiB safety limit".into());}
        if !self.visited.insert((extent,bytes)) {return Ok(());}
        let mut dir=Vec::with_capacity(bytes as usize);
        let mut remaining=bytes as usize;
        let mut lba=extent as u64;
        while remaining>0 {
            let sector=self.image.read_sector(&mut self.file,lba)?;
            let take=remaining.min(USER_SECTOR);
            dir.extend_from_slice(&sector[..take]);
            remaining-=take;
            lba+=1;
        }
        let mut cursor=0usize;
        while cursor<dir.len() {
            if dir[cursor]==0 {
                cursor=((cursor/USER_SECTOR)+1)*USER_SECTOR;
                continue;
            }
            let length=dir[cursor] as usize;
            if length<34 || cursor+length>dir.len() ||
                cursor/USER_SECTOR!=(cursor+length-1)/USER_SECTOR {
                return Err(format!("invalid ISO directory entry at sector offset {cursor}"));
            }
            let item=entry(&dir[cursor..cursor+length],self.joliet)?;
            cursor+=length;
            if item.name=="." || item.name==".." {continue;}
            let path=if prefix.is_empty(){item.name.clone()}
                else{format!("{prefix}/{}",item.name)};
            if path.len()>2048 {return Err("ISO9660 path too long".into());}
            let key=path.to_lowercase();
            if self.results.len()>=MAX_FILES {return Err("too many ISO9660 file entries".into());}
            if item.is_directory {
                self.walk(item.extent,item.size,&path,depth+1)?;
            }else{
                if self.results.contains_key(&key) {
                    return Err(format!("duplicate normalized CD file path: {path}"));
                }
                let sectors=(item.size as u64).div_ceil(USER_SECTOR as u64);
                let accessible=item.extent as u64 + sectors<=self.image.sectors();
                let (sha,status)=if item.multiextent {
                    ("UNAVAILABLE".to_owned(),"SUSPECT_MULTI_EXTENT".to_owned())
                }else if !accessible {
                    ("UNAVAILABLE".to_owned(),"SUSPECT_INVALID_EXTENT".to_owned())
                }else{
                    let mut hash=sha256::Digest::new();
                    let mut remaining=item.size as usize;
                    let mut lba=item.extent as u64;
                    while remaining>0 {
                        let sector=self.image.read_sector(&mut self.file,lba)?;
                        let take=remaining.min(USER_SECTOR);
                        hash.update(&sector[..take]);
                        remaining-=take;
                        lba+=1;
                    }
                    (hash.finish(),"OK".to_owned())
                };
                self.results.insert(key,FileRecord{path,size:item.size,sha,status,extent:item.extent});
            }
        }
        Ok(())
    }
}

// Edition signatures contain only internal CD file paths/sizes/SHA-256.
// Raw ISO/BIN length, whole image hash, volume label and CUE tracks are
// diagnostics only, not identifying signals.
const PROFILES: [(&str,&str);3]=[
    ("C_SKIES_DISC_001",include_str!("../profiles/c_skies_disc_001.tsv")),
    ("C_SKIES_FR_001",include_str!("../profiles/c_skies_fr_001.tsv")),
    ("C_SKIES_DE_001",include_str!("../profiles/c_skies_de_001.tsv")),
];
fn matches_profile(files:&BTreeMap<String,FileRecord>,profile:&str)->bool {
    let mut seen=0usize;
    for line in profile.lines() {
        if line.is_empty() || line.starts_with('#') {continue;}
        let mut cells=line.split('\t');
        let (Some(path),Some(size),Some(hash),None)=
            (cells.next(),cells.next(),cells.next(),cells.next()) else {return false;};
        let Ok(bytes)=size.parse::<u32>() else {return false;};
        let Some(actual)=files.get(&path.to_lowercase()) else {return false;};
        if actual.size!=bytes || actual.sha!=hash || actual.status!="OK" {
            return false;
        }
        seen+=1;
    }
    seen>0 && seen==files.len()
}

/// Selectors name an *approved* CD file-content profile, not an image
/// filename/volume label. FR and DE must never share the same test output.
pub fn edition_selector(label:&str)->Option<&'static str> {
    match label.to_ascii_uppercase().as_str() {
        "USA"|"EN"|"C_SKIES_DISC_001"=>Some("C_SKIES_DISC_001"),
        "FR"|"C_SKIES_FR_001"=>Some("C_SKIES_FR_001"),
        "DE"|"C_SKIES_DE_001"=>Some("C_SKIES_DE_001"),
        _=>None,
    }
}
pub fn stage_path(base:&Path,edition:&str)->Result<PathBuf,String> {
    let suffix=match edition {
        "C_SKIES_DISC_001"=>"USA",
        "C_SKIES_FR_001"=>"FR",
        "C_SKIES_DE_001"=>"DE",
        _=>return Err(format!("unknown staging edition: {edition}")),
    };
    Ok(base.join(format!("CrimsonSkiesFix.extract-test.{suffix}")))
}

/// Verify every *original game file* against the selected CD profile before
/// applying a patch. This prevents accidentally updating a French staging
/// directory under a German label, even if the folder was renamed manually.
/// It also detects incomplete/modified game extractions.
pub fn verify_staging(game:&Path,edition:&str)->Result<usize,String> {
    verify_original_files(game,edition,false,false)
}

/// Post-install game payloads omit the eight original chapter CAB archives:
/// all their contents have been expanded, hashed and sealed into a separate
/// manifest. Pre-install verification still checks the exact original CABs.
pub fn verify_installed_without_cabs(game:&Path,edition:&str,activated:bool)->Result<usize,String>{
    verify_original_files(game,edition,true,activated)
}

// Authentic FR INSTALL.EXE and DE/USA SETUP.EXE copy these seven
// companions from the verified CD ROOT into the installed GAME root.
// This includes SETUPENU.DLL; the localized root README makes eight.
// These are *file-copy rules only*, never a reason to execute SETUP.EXE.
const RETAIL_ROOT_SUPPORT:[(&str,&str);7]=[
    ("CLCD16.DLL","clcd16.dll"),
    ("CLCD32.DLL","clcd32.dll"),
    ("CLOKSPL.EXE","clokspl.exe"),
    ("DPLAYERX.DLL","dplayerx.dll"),
    ("DRVMGT.DLL","drvmgt.dll"),
    ("MCP.DLL","mcp.dll"),
    ("SETUPENU.DLL","SETUPENU.DLL"),
];
pub const RETAIL_SUPPORT_FILES:usize=8; // seven common + regional Readme
fn retail_readme(edition:&str)->Result<(&'static str,&'static str),String>{
    match edition{
        "C_SKIES_FR_001"=>Ok(("LISEZMOI.RTF","Readme.rtf")),
        "C_SKIES_DE_001"|"C_SKIES_DISC_001"=>Ok(("README.RTF","Readme.rtf")),
        _=>Err("unrecognized retail installer edition".into()),
    }
}
fn installed_game_name(edition:&str,original:&str)->&'static str{
    match original {
        "CRIMSO_1.IFR"=>"crimsonff.ifr",
        "CSZONE_1.EXE"=>"cszoneregister.exe",
        "CLUF.RTF" if edition=="C_SKIES_FR_001"=>"EULA.RTF",
        _=>"",
    }
}
fn expected_installed_path<'a>(edition:&str,relative:&'a str)->&'a str{
    match installed_game_name(edition,relative){
        ""=>relative,
        "crimsonff.ifr"=>"crimsonff.ifr",
        "cszoneregister.exe"=>"cszoneregister.exe",
        "EULA.RTF"=>"EULA.RTF",
        _=>relative,
    }
}

/// Apply the original setup's exact source→destination filename table
/// for the two shortened GAME-root names, plus FR localized EULA.
/// Applies to the staged new game directory, never an existing install.
pub fn normalize_game_filenames(game:&Path,edition:&str)->Result<usize,String>{
    if !fs::symlink_metadata(game).map(|m|m.file_type().is_dir()).unwrap_or(false){
        return Err("missing/unsafe game stage for retail filename normalization".into());
    }
    let originals=if edition=="C_SKIES_FR_001"{
        &["CRIMSO_1.IFR","CSZONE_1.EXE","CLUF.RTF"][..]
    } else if edition=="C_SKIES_DISC_001"||edition=="C_SKIES_DE_001" {
        &["CRIMSO_1.IFR","CSZONE_1.EXE"][..]
    } else {
        return Err("unsupported edition for retail filename normalization".into());
    };
    for name in originals{
        let from=game.join(name);
        let dest=game.join(installed_game_name(edition,name));
        if !fs::symlink_metadata(&from).map(|m|m.file_type().is_file()).unwrap_or(false){
            return Err(format!("retail source file missing/unsafe: {}",from.display()));
        }
        if fs::symlink_metadata(&dest).is_ok(){
            return Err(format!("refusing to overwrite retail target: {}",dest.display()));
        }
        fs::rename(&from,&dest).map_err(err)?;
    }
    Ok(originals.len())
}

/// Verify the eight additional root-level files against their original CD
/// manifest, not against self-reported install receipts.
pub fn verify_retail_support(game:&Path,edition:&str)->Result<usize,String>{
    if !fs::symlink_metadata(game).map(|m|m.file_type().is_dir()).unwrap_or(false){
        return Err("missing/unsafe game directory for retail support verification".into());
    }
    let profile=PROFILES.iter().find(|(name,_)|*name==edition)
        .map(|(_,text)|*text).ok_or("unrecognized retail support edition")?;
    let readme=retail_readme(edition)?;
    let mut verified=0;
    for (src,dst) in RETAIL_ROOT_SUPPORT.into_iter().chain(std::iter::once(readme)){
        let row=profile.lines().filter(|line|!line.starts_with('#'))
            .find(|line|line.starts_with(&format!("{src}\t")))
            .ok_or(format!("regional CD manifest missing retail support: {src}"))?;
        let fields=row.split('\t').collect::<Vec<_>>();
        if fields.len()!=3 {return Err("invalid root support CD manifest row".into());}
        let target=game.join(dst);
        if !fs::symlink_metadata(&target).map(|m|m.file_type().is_file()).unwrap_or(false){
            return Err(format!("retail support missing/unsafe: {}",target.display()));
        }
        let size=fields[1].parse::<u64>().map_err(err)?;
        if fs::metadata(&target).map_err(err)?.len()!=size{
            return Err(format!("retail support file wrong size: {}",target.display()));
        }
        let mut file=File::open(&target).map_err(err)?;
        let mut digest=sha256::Digest::new();
        let mut buffer=[0u8;32768];
        loop{
            let n=file.read(&mut buffer).map_err(err)?;
            if n==0{break;}
            digest.update(&buffer[..n]);
        }
        if digest.finish()!=fields[2]{
            return Err(format!("retail support SHA-256 mismatch: {}",target.display()));
        }
        verified+=1;
    }
    Ok(verified)
}

pub const REMOVED_CHAPTER_CABS:usize=8;
fn is_pruned_chapter_cab(path:&str)->bool{
    matches!(path,"GAME/ZBD/C1.CAB"|"GAME/ZBD/C1B.CAB"|"GAME/ZBD/C1C.CAB"|
        "GAME/ZBD/C2.CAB"|"GAME/ZBD/C2B.CAB"|"GAME/ZBD/C3.CAB"|
        "GAME/ZBD/C4.CAB"|"GAME/ZBD/C5.CAB")
}

fn verify_original_files(game:&Path,edition:&str,skip_cabs:bool,activated:bool)->Result<usize,String> {
    if activated && !skip_cabs {
        return Err("approved EXE backup is only valid for a completed retail install".into());
    }
    if !fs::symlink_metadata(game).map(|m|m.file_type().is_dir()).unwrap_or(false) {
        return Err(format!("missing or unsafe {edition} staging directory: {}",game.display()));
    }
    let profile=PROFILES.iter().find(|(name,_)|*name==edition)
        .map(|(_,text)|*text).ok_or("unknown edition requested")?;
    let mut checked=0usize;
    for line in profile.lines().filter(|line|!line.starts_with('#')&&!line.is_empty()) {
        let cols=line.split('\t').collect::<Vec<_>>();
        if cols.len()!=3 {return Err("invalid compiled-in CD profile".into());}
        let Some(relative)=cols[0].strip_prefix("GAME/") else {continue;};
        if skip_cabs && is_pruned_chapter_cab(cols[0]){
            if fs::symlink_metadata(game.join(relative)).is_ok() {
                return Err(format!("obsolete chapter CAB remains installed: {}",cols[0]));
            }
            continue;
        }
        // Original FR (and any other listed retail edition) has a CD
        // CRIMPTCH.ROF entry. Once the official 1.02 updater and our native
        // GOS lock have run, its bytes may intentionally differ from the
        // original CD profile. This one file is independently authenticated
        // against the official 1.02 hash or the exact managed overlay +
        // original backup. Every other retail asset remains CD-hash locked.
        if skip_cabs && relative.eq_ignore_ascii_case("GOSDATA/ASSETS/CRIMPTCH.ROF") {
            crate::managed_gos_patch::verify_game_overlay(game)?;
            checked+=1;
            continue;
        }
        // Once the owner-supplied approved 1.02 executable has been
        // activated, the exact original CD-protected CRIMSON.EXE is
        // kept under this fixed name and verified against the original
        // immutable disc manifest. Never mistake the replacement game EXE
        // for the disc's protected bytes.
        let actual_relative=if activated && relative.eq_ignore_ascii_case("CRIMSON.EXE"){
            "CRIMSON.EXE.protected.bak"
        }else if skip_cabs{expected_installed_path(edition,relative)}else{relative};
        let path=game.join(actual_relative);
        if skip_cabs && actual_relative!=relative
            && !relative.eq_ignore_ascii_case("CRIMSON.EXE")
            && fs::symlink_metadata(game.join(relative)).is_ok(){
            return Err(format!("obsolete short CD filename remains beside canonical retail file: {relative}"));
        }
        // The expected names are only normalized game-file paths from a
        // verified fixed manifest. Still reject non-directory ancestors,
        // including junctions/symlinks, before reading the target.
        let mut cur=game.to_path_buf();
        let components=Path::new(actual_relative).components().collect::<Vec<_>>();
        if components.is_empty() || !components.iter().all(|c|matches!(c,Component::Normal(_))) {
            return Err("invalid compiled-in game file path".into());
        }
        for component in &components[..components.len()-1] {
            cur.push(component.as_os_str());
            if !fs::symlink_metadata(&cur).map(|m|m.file_type().is_dir()).unwrap_or(false) {
                return Err(format!("unsafe game asset directory: {}",cur.display()));
            }
        }
        if !fs::symlink_metadata(&path).map(|m|m.file_type().is_file()).unwrap_or(false) {
            return Err(format!("missing or unsafe staged original file: {}",path.display()));
        }
        let expected_size=cols[1].parse::<u64>().map_err(err)?;
        if fs::metadata(&path).map_err(err)?.len()!=expected_size {
            return Err(format!("{edition} game file size mismatch: {}",path.display()));
        }
        let mut file=File::open(&path).map_err(err)?;
        let mut hasher=sha256::Digest::new();
        let mut buffer=[0u8;32768];
        loop {
            let n=file.read(&mut buffer).map_err(err)?;
            if n==0 {break;}
            hasher.update(&buffer[..n]);
        }
        if hasher.finish()!=cols[2] {
            return Err(format!("{edition} game file SHA-256 mismatch: {}",path.display()));
        }
        checked+=1;
    }
    if checked==0 {return Err("edition manifest had no game files".into());}
    Ok(checked)
}

/// Prepared CD-game payload; a protected original CD executable is *not* a
/// valid game executable until the separate strict 1.02 gate is satisfied.
pub struct Extraction {
    pub edition:&'static str,
    pub file_count:usize,
    pub bytes:u64,
    pub retail_support_files:usize,
    pub retail_support_bytes:u64,
}

fn extract_one(source:&ImageSource,disc:&mut File,rec:&FileRecord,path:&Path)->Result<(),String> {
    if rec.status!="OK" {return Err(format!("cannot extract suspect file: {}",rec.path));}
    let parent=path.parent().ok_or("no game file parent")?;
    fs::create_dir_all(parent).map_err(err)?;
    let mut out=OpenOptions::new().write(true).create_new(true).open(path).map_err(err)?;
    let mut digest=sha256::Digest::new();
    let mut remaining=rec.size as usize;
    let mut sector=rec.extent as u64;
    while remaining>0 {
        let bytes=source.read_sector(disc,sector)?;
        let n=remaining.min(USER_SECTOR);
        out.write_all(&bytes[..n]).map_err(err)?;
        digest.update(&bytes[..n]);
        remaining-=n;sector+=1;
    }
    out.sync_all().map_err(err)?;
    let sha=digest.finish();
    if sha!=rec.sha || fs::metadata(path).map_err(err)?.len()!=rec.size as u64 {
        return Err(format!("CD changed during extraction or copy failed: {}",rec.path));
    }
    Ok(())
}

/// Extract the authenticated retail CD into the installer's private staging
/// directory. Only the automatic installation flow can publish the result.
pub fn extract_for_install(base:&Path,specified:Option<&Path>,transaction:&Path)
    ->Result<Extraction,String> {
    extract_game_to(base,specified,transaction)
}

fn extract_game_to(base:&Path,specified:Option<&Path>,target:&Path)
    ->Result<Extraction,String> {
    let media=select(base,specified)?;
    let image=if has_ext(&media,"cue"){open_cue(&media)?} else {open_iso(&media)?};
    let mut file=File::open(&image.file).map_err(err)?;
    let volume=parse_volume(&image,&mut file)?;
    let mut walker=Walker{image:&image,file,joliet:volume.filename_source=="Joliet",
        results:BTreeMap::new(),visited:HashSet::new()};
    walker.walk(volume.root.extent,volume.root.size,"",0)?;
    // Fail *before* creating a directory unless the full known edition matches.
    let matches=PROFILES.iter().filter(|(_,profile)|matches_profile(&walker.results,profile))
        .map(|(name,_)|*name).collect::<Vec<_>>();
    let [edition]=matches.as_slice() else {
        return Err("unknown or ambiguous CD file manifest; no extraction performed".into());
    };
    let dest=target.to_path_buf();
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(format!("{} staging already exists; refusing to overwrite: {}",
            edition,dest.display()));
    }
    let files=walker.results.values().filter(|entry|entry.path.starts_with("GAME/"))
        .collect::<Vec<_>>();
    if files.is_empty() || files.len()>MAX_FILES {
        return Err("recognized CD contains no GAME/ files".into());
    }
    let bytes=files.iter().map(|r|r.size as u64).sum();
    if bytes>2_000_000_000 {
        return Err("recognized CD extraction exceeds the safe 2 GB limit".into());
    }
    // Always include the seven original CD-root support files plus the
    // edition-specific README, just like the retail installer.
    let mut additional=Vec::new();
    let readme=retail_readme(edition)?;
    for (src,dst) in RETAIL_ROOT_SUPPORT.into_iter().chain(std::iter::once(readme)){
        let entry=walker.results.get(&src.to_ascii_lowercase())
            .ok_or(format!("verified CD missing expected root support: {src}"))?;
        additional.push((entry,dst));
    }
    let retail_support_bytes=additional.iter().map(|(r,_)|r.size as u64).sum::<u64>();
    if bytes+retail_support_bytes>2_000_000_000 {
        return Err("CD extraction and retail support payload exceeds 2 GB".into());
    }
    fs::create_dir(&dest).map_err(err)?;
    let mut disc=File::open(&image.file).map_err(err)?;
    for (index,rec) in files.iter().enumerate() {
        let relative=rec.path.strip_prefix("GAME/").ok_or("not a game file")?;
        if relative.is_empty() {return Err("empty game file path".into());}
        let relative_path=Path::new(relative);
        if !relative_path.components().all(|c|matches!(c,Component::Normal(_))) {
            return Err(format!("unsafe game asset path: {}",rec.path));
        }
        let target=dest.join(relative_path);
        extract_one(&image,&mut disc,rec,&target)?;
        if (index+1)%10==0 || index+1==files.len() {
            println!("Extracted {}/{} game files...",index+1,files.len());
        }
    }
    for (record,name) in &additional {
        extract_one(&image,&mut disc,record,&dest.join(name))?;
    }
    Ok(Extraction{edition:*edition,file_count:files.len(),bytes,
        retail_support_files:additional.len(),retail_support_bytes})
}

// Synthetic ISO/CUE regression diagnostics only; no longer shipped as CLI.
#[cfg(test)]
fn inspect(base:&Path,specified:Option<&Path>)->Result<String,String> {
    let media=select(base,specified)?;
    let image=if has_ext(&media,"cue"){open_cue(&media)?}else{open_iso(&media)?};
    let mut file=File::open(&image.file).map_err(err)?;
    let volume=parse_volume(&image,&mut file)?;
    let mut walk=Walker{
        image:&image,file,joliet:volume.filename_source=="Joliet",
        results:BTreeMap::new(),visited:HashSet::new()
    };
    walk.walk(volume.root.extent,volume.root.size,"",0)?;
    if walk.results.is_empty() {return Err("ISO9660 filesystem has no files".into());}
    let good=walk.results.values().filter(|r|r.status=="OK").count();
    let bad=walk.results.len()-good;
    let total=walk.results.values().filter(|r|r.status=="OK")
        .map(|r|r.size as u64).sum::<u64>();
    let edition=if bad==0 {
        PROFILES.iter().find(|(_,profile)|matches_profile(&walk.results,profile))
            .map(|(name,_)|*name).unwrap_or("UNKNOWN")
    } else {"UNKNOWN"};
    let recognition=if edition=="UNKNOWN" {
        "NO_EXACT_INTERNAL_FILE_MANIFEST_MATCH"
    } else {
        "EXACT_INTERNAL_FILE_MANIFEST"
    };
    let mut report=format!(
        "CrimsonSkiesFix CD INSPECT v1\n\
         SourceMedia={}\n\
         Format={}\n\
         DataTracks={}\n\
         Filesystem={}\n\
         VolumeLabel={}\n\
         Edition={edition}\n\
         EditionRecognition={recognition}\n\
         IdentificationRule=normalized internal paths + internal file sizes + internal file SHA256\n\
         WholeImageSHA256=NOT_USED\n\
         WholeImageSize=NOT_USED\n\
         FilesTotal={}\nFilesHashed={good}\nFilesSuspect={bad}\nHashedPayloadBytes={total}\n\
         FILE\\tSIZE_BYTES\\tSHA256\\tSTATUS\n",
        media.file_name().unwrap_or_default().to_string_lossy(),image.format,image.tracks,
        volume.filename_source,volume.label,walk.results.len()
    );
    for record in walk.results.values() {
        report.push_str(&format!("{}\t{}\t{}\t{}\n",
            record.path,record.size,record.sha,record.status));
    }
    report.push_str("INSPECT_RESULT=COMPLETE\n");
    Ok(report.replace("FILE\\tSIZE_BYTES\\tSHA256\\tSTATUS","FILE\tSIZE_BYTES\tSHA256\tSTATUS"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn iso733(out:&mut [u8],number:u32) {
        out[..4].copy_from_slice(&number.to_le_bytes());
        out[4..8].copy_from_slice(&number.to_be_bytes());
    }
    fn directory_record(extent:u32,size:u32,flags:u8,name:&[u8])->Vec<u8> {
        let n=33+name.len()+(1-name.len()%2);
        let mut r=vec![0u8;n];
        r[0]=n as u8;
        iso733(&mut r[2..10],extent);
        iso733(&mut r[10..18],size);
        r[25]=flags;
        r[28..30].copy_from_slice(&1u16.to_le_bytes());
        r[30..32].copy_from_slice(&1u16.to_be_bytes());
        r[32]=name.len() as u8;
        r[33..33+name.len()].copy_from_slice(name);
        r
    }
    fn make_iso()->Vec<u8> {
        let mut iso=vec![0u8;21*USER_SECTOR];
        let pvd=16*USER_SECTOR;
        iso[pvd]=1;
        iso[pvd+1..pvd+6].copy_from_slice(b"CD001");
        iso[pvd+6]=1;
        iso[pvd+40..pvd+47].copy_from_slice(b"CRIMSON");
        iso733(&mut iso[pvd+80..pvd+88],21);
        let root=directory_record(19,2048,2,&[0]);
        iso[pvd+156..pvd+156+root.len()].copy_from_slice(&root);
        let term=17*USER_SECTOR;
        iso[term]=255;iso[term+1..term+6].copy_from_slice(b"CD001");
        iso[term+6]=1;
        let entries=[
            directory_record(19,2048,2,&[0]),
            directory_record(19,2048,2,&[1]),
            directory_record(20,5,0,b"CRIMSON.EXE;1"),
        ];
        let mut pos=19*USER_SECTOR;
        for r in entries {
            iso[pos..pos+r.len()].copy_from_slice(&r);
            pos+=r.len();
        }
        iso[20*USER_SECTOR..20*USER_SECTOR+5].copy_from_slice(b"hello");
        iso
    }
    fn workdir()->PathBuf {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("crimson-inspect-cd-{}-{id}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir
    }
    #[test]
    fn all_three_profiles_recognize_only_exact_internal_file_content() {
        let expected=[("C_SKIES_DISC_001",112),("C_SKIES_FR_001",112),("C_SKIES_DE_001",107)];
        for ((edition,profile),(expected_edition,count)) in PROFILES.iter().zip(expected) {
            assert_eq!(*edition,expected_edition);
            let mut files=BTreeMap::new();
            for line in profile.lines().filter(|s|!s.is_empty() && !s.starts_with('#')) {
                let cols=line.split('\t').collect::<Vec<_>>();
                assert_eq!(cols.len(),3);
                let path=cols[0].to_owned();
                let size=cols[1].parse::<u32>().unwrap();
                let sha=cols[2].to_owned();
                assert!(files.insert(path.to_lowercase(),FileRecord {
                    path,size,sha,status:"OK".to_owned(),extent:0
                }).is_none());
            }
            assert_eq!(files.len(),count);
            assert!(matches_profile(&files,profile),"{edition} failed exact matching");
            // A byte-content change must not match.
            files.get_mut("game/crimson.exe").unwrap().sha="0".repeat(64);
            assert!(!matches_profile(&files,profile));
            // Neither should an incomplete profile.
            files.remove("game/crimson.exe");
            assert!(!matches_profile(&files,profile));
        }
    }
    #[test]
    fn refuse_existing_install_transaction_without_overwriting() {
        let dir=workdir();
        let transaction=dir.join("CrimsonSkiesFix.installing");
        fs::create_dir(&transaction).unwrap();
        let marker=transaction.join("user-data.txt");
        fs::write(&marker,b"preserve").unwrap();
        let image=dir.join("missing.iso");
        assert!(extract_for_install(&dir,Some(&image),&transaction).is_err());
        assert_eq!(fs::read(marker).unwrap(),b"preserve");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn completed_install_checks_protected_original_backup_not_active_executable() {
        let profile=PROFILES.iter().find(|(name,_)|*name=="C_SKIES_DISC_001").unwrap().1;
        let row=profile.lines().find(|l|l.starts_with("GAME/CRIMSON.EXE\t")).unwrap();
        let columns=row.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(),3);
        assert_eq!(columns[2].len(),64);
        // A completed installation keeps this original in a separate
        // backup; the installed CRIMSON.EXE must pass the *other*
        // approved unwrapped executable SHA-256 gate.
        assert_ne!(columns[2],
            "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75");
    }
    #[test]
    fn retail_file_renames_keep_bytes_and_refuse_overwrite() {
        let dir=workdir();
        for (short,bytes) in [
            ("CRIMSO_1.IFR",&b"ifrlocal"[..]),
            ("CSZONE_1.EXE",&b"zone"[..]),
            ("CLUF.RTF",&b"french license"[..])
        ] {
            fs::write(dir.join(short),bytes).unwrap();
        }
        assert_eq!(normalize_game_filenames(&dir,"C_SKIES_FR_001").unwrap(),3);
        assert_eq!(fs::read(dir.join("crimsonff.ifr")).unwrap(),b"ifrlocal");
        assert_eq!(fs::read(dir.join("cszoneregister.exe")).unwrap(),b"zone");
        assert_eq!(fs::read(dir.join("EULA.RTF")).unwrap(),b"french license");
        assert!(!dir.join("CRIMSO_1.IFR").exists());
        assert!(!dir.join("CSZONE_1.EXE").exists());
        assert!(!dir.join("CLUF.RTF").exists());
        // Staging must never replace an already present canonical file.
        fs::write(dir.join("CRIMSO_1.IFR"),b"unexpected").unwrap();
        assert!(normalize_game_filenames(&dir,"C_SKIES_FR_001").is_err());
        assert_eq!(fs::read(dir.join("crimsonff.ifr")).unwrap(),b"ifrlocal");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn retail_cd_root_file_names_are_mapped_to_historical_game_layout(){
        assert_eq!(retail_readme("C_SKIES_FR_001").unwrap(),("LISEZMOI.RTF","Readme.rtf"));
        assert_eq!(retail_readme("C_SKIES_DISC_001").unwrap(),("README.RTF","Readme.rtf"));
        for (edition,expected) in [
            ("C_SKIES_FR_001",3),
            ("C_SKIES_DE_001",2),
            ("C_SKIES_DISC_001",2),
        ]{
            let count=["CRIMSO_1.IFR","CSZONE_1.EXE","CLUF.RTF"]
                .iter().filter(|s|!installed_game_name(edition,s).is_empty()).count();
            assert_eq!(count,expected);
        }
        assert_eq!(expected_installed_path("C_SKIES_FR_001","CLUF.RTF"),"EULA.RTF");
        assert_eq!(expected_installed_path("C_SKIES_DE_001","EULA.RTF"),"EULA.RTF");
        assert_eq!(RETAIL_ROOT_SUPPORT.len()+1,RETAIL_SUPPORT_FILES);
        for (edition,profile) in PROFILES{
            let readme=retail_readme(edition).unwrap();
            for (source,_) in RETAIL_ROOT_SUPPORT.into_iter().chain(std::iter::once(readme)){
                assert!(profile.lines().any(|line|line.starts_with(&format!("{source}\t"))),
                    "missing {source} in {edition}");
            }
        }
    }
    #[test]
    fn only_eight_fixed_chapter_cabs_may_be_removed() {
        let paths=["GAME/ZBD/C1.CAB","GAME/ZBD/C1B.CAB","GAME/ZBD/C1C.CAB",
            "GAME/ZBD/C2.CAB","GAME/ZBD/C2B.CAB","GAME/ZBD/C3.CAB",
            "GAME/ZBD/C4.CAB","GAME/ZBD/C5.CAB"];
        assert_eq!(paths.iter().filter(|p|is_pruned_chapter_cab(p)).count(),
            REMOVED_CHAPTER_CABS);
        assert!(!is_pruned_chapter_cab("GAME/GOSDATA/C1.CAB"));
        assert!(!is_pruned_chapter_cab("GAME/ZBD/INTERP.ZBD"));
        assert!(!is_pruned_chapter_cab("GAME/ZBD/OTHER.CAB"));
    }
    #[test]
    fn separate_edition_staging_paths_and_fail_closed_selectors() {
        let base=Path::new("test");
        assert_ne!(stage_path(base,"C_SKIES_FR_001").unwrap(),
                   stage_path(base,"C_SKIES_DE_001").unwrap());
        assert_eq!(edition_selector("DE"),Some("C_SKIES_DE_001"));
        assert_eq!(edition_selector("fr"),Some("C_SKIES_FR_001"));
        assert_eq!(edition_selector("USA"),Some("C_SKIES_DISC_001"));
        assert_eq!(edition_selector("unknown"),None);
    }
    #[test]
    fn reject_staging_edition_confusion_before_asset_application() {
        let dir=workdir();
        let wrong=stage_path(&dir,"C_SKIES_DE_001").unwrap();
        fs::create_dir(&wrong).unwrap();
        assert!(verify_staging(&wrong,"C_SKIES_DE_001").is_err());
        assert!(verify_staging(&wrong,"C_SKIES_FR_001").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn extracted_bytes_match_source_and_reject_replacement() {
        let dir=workdir();
        let iso_file=dir.join("disc.iso");
        fs::write(&iso_file,make_iso()).unwrap();
        let image=open_iso(&iso_file).unwrap();
        let mut src=File::open(&iso_file).unwrap();
        let output=dir.join("output").join("CRIMSON.EXE");
        let r=FileRecord{
            path:"GAME/CRIMSON.EXE".to_owned(),size:5,extent:20,
            sha:sha256::hex(b"hello"),status:"OK".to_owned()
        };
        extract_one(&image,&mut src,&r,&output).unwrap();
        assert_eq!(fs::read(&output).unwrap(),b"hello");
        assert!(extract_one(&image,&mut src,&r,&output).is_err());
        let mut changed=r.clone();
        changed.sha="0".repeat(64);
        assert!(extract_one(&image,&mut src,&changed,&dir.join("invalid")).is_err());
        let stage=dir.join("CrimsonSkiesFix.installing");
        assert!(extract_for_install(&dir,Some(&iso_file),&stage).is_err());
        assert!(!stage.exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn inspect_internal_file_hash_not_iso_image_hash() {
        let dir=workdir();
        let path=dir.join("Disc.iso");
        fs::write(&path,make_iso()).unwrap();
        let first=inspect(&dir,None).unwrap();
        let expected=sha256::hex(b"hello");
        assert!(first.contains(&format!("CRIMSON.EXE\t5\t{expected}\tOK")));
        assert!(first.contains("WholeImageSHA256=NOT_USED"));
        let mut padded=make_iso();
        padded.extend_from_slice(&vec![0u8;5000]); // An image-size-only difference is irrelevant.
        fs::write(&path,padded).unwrap();
        let second=inspect(&dir,None).unwrap();
        assert_eq!(first,second);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cue_bin_raw_mode1_equivalent_to_iso() {
        let dir=workdir();
        let iso=make_iso();
        let mut bin=vec![0u8;22*2352]; // Additional AUDIO frame is not part of DATA.
        for (i,sector) in iso.chunks_exact(USER_SECTOR).enumerate() {
            bin[i*2352+16..i*2352+16+USER_SECTOR].copy_from_slice(sector);
        }
        fs::write(dir.join("Disc.bin"),bin).unwrap();
        fs::write(dir.join("Disc.cue"),
            "FILE \"Disc.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 00:00:21\n").unwrap();
        let report=inspect(&dir,None).unwrap();
        assert!(report.contains(&format!("CRIMSON.EXE\t5\t{}\tOK",sha256::hex(b"hello"))));
        assert!(report.contains("Format=CUE/MODE1/2352"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cue_unpadded_index_one_with_img_is_identical_to_original_iso() {
        // Exact CUE layout supplied by the owner: CRIMSON.img with
        // TRACK 1 MODE1/2352 and INDEX 1 00:00:00 (CRLF line endings).
        let dir=workdir();
        let iso=make_iso();
        let mut img=vec![0u8;21*2352];
        for (i,sector) in iso.chunks_exact(USER_SECTOR).enumerate(){
            img[i*2352+16..i*2352+16+USER_SECTOR].copy_from_slice(sector);
        }
        fs::write(dir.join("CRIMSON.img"),img).unwrap();
        let cue=dir.join("CRIMSON.cue");
        fs::write(&cue,
            "FILE \"CRIMSON.img\" BINARY\r\n   TRACK 1 MODE1/2352\r\n   INDEX 1 00:00:00\r\n"
        ).unwrap();
        let result=inspect(&dir,Some(&cue)).unwrap();
        assert!(result.contains("Format=CUE/MODE1/2352"));
        assert!(result.contains(&format!("CRIMSON.EXE\t5\t{}\tOK",sha256::hex(b"hello"))));
        // Changing only the zero-padding must not change interpretation.
        fs::write(&cue,
            "FILE \"CRIMSON.img\" BINARY\n TRACK 01 MODE1/2352\n INDEX 01 00:00:00\n"
        ).unwrap();
        assert_eq!(inspect(&dir,Some(&cue)).unwrap(),result);
        // Both spellings refer to the same INDEX 01; reject duplicate.
        fs::write(&cue,
            "FILE \"CRIMSON.img\" BINARY\n TRACK 1 MODE1/2352\n INDEX 1 00:00:00\n INDEX 01 00:00:00\n"
        ).unwrap();
        assert!(inspect(&dir,Some(&cue)).is_err());
        // INDEX 00 alone is still not the data-track start.
        fs::write(&cue,
            "FILE \"CRIMSON.img\" BINARY\n TRACK 1 MODE1/2352\n INDEX 0 00:00:00\n"
        ).unwrap();
        assert!(inspect(&dir,Some(&cue)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn refuse_unsafe_cue_file_paths_and_ambiguous_media() {
        let dir=workdir();
        assert!(resolve_cue_bin(&dir,"../outside.bin").is_err());
        assert!(resolve_cue_bin(&dir,"C:\\test.bin").is_err());
        fs::write(dir.join("a.iso"),make_iso()).unwrap();
        fs::write(dir.join("b.iso"),make_iso()).unwrap();
        let ambiguous=inspect(&dir,None).unwrap_err();
        assert!(ambiguous.contains("expected exactly one ISO or CUE"));
        assert!(!ambiguous.contains("--inspect-cd"));
        assert!(inspect(&dir,Some(Path::new("a.iso"))).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
}
