//! Read-only, exact-identity verifier for the game's original 1.02
//! CRIMPTCH.ROF or CrimsonSkiesFix's backed-up, two-script GOS overlay.
//! This is not a repair tool. It never changes an existing game asset.
//! It prevents a valid owner-generated overlay being treated as CD damage.
use std::{fs,io::Read,path::Path};
use flate2::read::ZlibDecoder;
use crate::{sha256,update102};

const BACKUP: &str = "crimptch.rof.CrimsonSkiesFix.original.bak";
const MODIFIED_VIDEO_HASH: &str =
    "a5f716d2e1ad007ac217a172e9d17b459444f7ffb23855a5a10d9a1841b9b22d";
const VIDEO_SIZE: usize = 4413;
const MAX_OVERLAY: u64 = 4 * 1024 * 1024;
const MAX_SCRIPT: usize = 1024 * 1024;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum State { Official102, VerifiedManaged }
impl State {
    pub fn label(self)->&'static str {
        match self {Self::Official102=>"OFFICIAL_102",Self::VerifiedManaged=>"VERIFIED_MANAGED"}
    }
}
#[derive(Debug,Clone,Copy)]
struct Member {offset:usize,stored:usize,expanded:usize,kind:u32}

fn field(bytes:&[u8],offset:usize)->Result<usize,String>{
    let end=offset.checked_add(4).ok_or("ROF field overflow")?;
    let slice=bytes.get(offset..end).ok_or("truncated ROF field")?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()) as usize)
}

/// Exact three-level tree, matching the two known 1.02 overlay layouts:
/// ASSETS / SCRIPTS / (AIRFRAME.SCRIPT[, VIDEO.SCRIPT]).
/// Never follow an arbitrary name, path or symlink from the archive.
fn node(bytes:&[u8],at:usize,names:&[&str])->Result<Vec<Member>,String>{
    let count=field(bytes,at)?;
    let pool_size=field(bytes,at.checked_add(4).ok_or("ROF node overflow")?)?;
    if count!=names.len() || pool_size>4096 {return Err("unexpected GOS overlay node".into());}
    let start=at.checked_add(8).ok_or("ROF node overflow")?;
    let pool=start.checked_add(count.checked_mul(24).ok_or("ROF count overflow")?)
        .ok_or("ROF pool overflow")?;
    let pool_end=pool.checked_add(pool_size).ok_or("ROF pool overflow")?;
    if pool_end>bytes.len(){return Err("GOS overlay node exceeds archive".into());}
    let mut result=Vec::with_capacity(count);
    for (i,expected) in names.iter().enumerate(){
        let e=start+i*24;
        let member=Member{
            offset:field(bytes,e)?,
            expanded:field(bytes,e+4)?,
            stored:field(bytes,e+8)?,
            kind:field(bytes,e+12)? as u32,
        };
        let name_len=field(bytes,e+16)?;
        let name_off=field(bytes,e+20)?;
        let limit=name_off.checked_add(name_len).ok_or("ROF name overflow")?;
        if name_len!=expected.len()+1 || limit>pool_size {
            return Err("unexpected GOS member name".into());
        }
        let value=&bytes[pool+name_off..pool+limit];
        if value[..value.len()-1]!=expected.as_bytes()[..] || value[value.len()-1]!=0 {
            return Err("GOS member identity mismatch".into());
        }
        if member.kind==1 {
            if member.stored!=0 || member.expanded!=0 || member.offset>=bytes.len() {
                return Err("invalid GOS directory".into());
            }
        }else if !matches!(member.kind,0|2) ||
            member.offset.checked_add(member.stored).is_none_or(|n|n>bytes.len()) ||
            (member.kind==0 && member.stored!=member.expanded) {
            return Err("GOS payload outside archive".into());
        }
        result.push(member);
    }
    Ok(result)
}
fn scripts(bytes:&[u8],modified:bool)->Result<Vec<Member>,String>{
    let root=node(bytes,0,&["ASSETS"])?;
    if root[0].kind!=1 {return Err("GOS ASSETS must be a directory".into());}
    let assets=node(bytes,root[0].offset,&["SCRIPTS"])?;
    if assets[0].kind!=1 {return Err("GOS SCRIPTS must be a directory".into());}
    let names:&[&str]=if modified{
        &["AIRFRAME.SCRIPT","VIDEO.SCRIPT"]
    }else{
        &["AIRFRAME.SCRIPT"]
    };
    let entries=node(bytes,assets[0].offset,names)?;
    if entries.iter().any(|x|x.kind==1){return Err("unexpected directory in GOS scripts".into());}
    // Do not permit script members to alias or overlap each other.
    if entries.len()==2 {
        let a=entries[0];
        let b=entries[1];
        let ae=a.offset+a.stored;
        let be=b.offset+b.stored;
        if a.offset<be && b.offset<ae {return Err("overlapping GOS scripts".into());}
    }
    Ok(entries)
}
fn stored<'a>(bytes:&'a [u8],entry:Member)->&'a [u8]{
    &bytes[entry.offset..entry.offset+entry.stored]
}
fn expanded(bytes:&[u8],entry:Member)->Result<Vec<u8>,String>{
    if entry.expanded>MAX_SCRIPT || entry.stored>MAX_SCRIPT {
        return Err("GOS script exceeds size limit".into());
    }
    let payload=stored(bytes,entry);
    if entry.kind==0 {return Ok(payload.to_vec());}
    if entry.kind!=2 {return Err("unsupported compressed GOS member".into());}
    let mut decoded=Vec::new();
    ZlibDecoder::new(payload).take(entry.expanded as u64+1)
        .read_to_end(&mut decoded).map_err(|e|format!("GOS zlib stream invalid: {e}"))?;
    if decoded.len()!=entry.expanded {return Err("GOS expanded length mismatch".into());}
    Ok(decoded)
}
fn verify_pair(official:&[u8],managed:&[u8])->Result<(),String>{
    let original=scripts(official,false)?;
    let patched=scripts(managed,true)?;
    let a=original[0];
    let b=patched[0];
    if a.kind!=b.kind || a.expanded!=b.expanded ||
        stored(official,a)!=stored(managed,b) {
        return Err("GOS AIRFRAME differs from authenticated original".into());
    }
    let video=patched[1];
    if video.kind!=2 || video.expanded!=VIDEO_SIZE ||
        sha256::hex(&expanded(managed,video)?)!=MODIFIED_VIDEO_HASH {
        return Err("GOS VIDEO.SCRIPT is not the exact supported managed postimage".into());
    }
    Ok(())
}
pub fn verify_game_overlay(game:&Path)->Result<State,String>{
    let assets=game.join("GOSDATA").join("ASSETS");
    let patch=assets.join("CRIMPTCH.ROF");
    if !crate::regular(&patch){return Err("missing or unsafe 1.02 CRIMPTCH.ROF".into());}
    let meta=fs::metadata(&patch).map_err(crate::error)?;
    if meta.len()>MAX_OVERLAY{return Err("CRIMPTCH.ROF exceeds supported bound".into());}
    let current=fs::read(&patch).map_err(crate::error)?;
    if sha256::hex(&current)==update102::ASSET_SHA256 {
        return Ok(State::Official102);
    }
    let backup=assets.join(BACKUP);
    if !crate::regular(&backup){
        return Err("modified CRIMPTCH.ROF lacks a regular verified original backup".into());
    }
    if fs::metadata(&backup).map_err(crate::error)?.len()!=797 {
        return Err("original 1.02 GOS backup has incorrect length".into());
    }
    let original=fs::read(&backup).map_err(crate::error)?;
    if sha256::hex(&original)!=update102::ASSET_SHA256 {
        return Err("original GOS backup SHA-256 differs from official 1.02".into());
    }
    verify_pair(&original,&current)?;
    Ok(State::VerifiedManaged)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_invalid_overlay_and_unknown_script_members(){
        assert!(scripts(&[],false).is_err());
        assert!(scripts(&[0u8;32],false).is_err());
        assert!(verify_pair(&[],&[]).is_err());
    }
    #[test]
    fn exact_compressed_video_postimage_is_pinned(){
        assert_eq!(VIDEO_SIZE,4413);
        assert_eq!(MODIFIED_VIDEO_HASH.len(),64);
        assert!(MODIFIED_VIDEO_HASH.bytes().all(|b|b.is_ascii_hexdigit()));
    }
    #[test]
    fn no_modified_archive_is_accepted_without_backup(){
        let root=std::env::temp_dir().join(format!(
            "crimson-gos-verify-{}-{:?}",std::process::id(),
            std::thread::current().id()
        ));
        let assets=root.join("GOSDATA").join("ASSETS");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("CRIMPTCH.ROF"),b"not official").unwrap();
        assert!(verify_game_overlay(&root).unwrap_err().contains("backup"));
        fs::remove_dir_all(root).unwrap();
    }
}
