// Crimson Skies resolution/FOV and Wine ROF patcher.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
// ROF NUL guard behavior adapted from the MIT-licensed CrimsonSkies-Wine-Fix
// by thehack904. See THIRD_PARTY_NOTICES.md.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

const EXE_LEN: usize = 2_580_480;
const WIDTH: usize = 0x20254B;
const HEIGHT: usize = 0x20254F;
const FOV60: usize = 0x20409C;
const FOV80: usize = 0x2040A0;
const DEFAULT_60: [u8; 4] = [0x92, 0x0A, 0x86, 0x3F];
const DEFAULT_80: [u8; 4] = [0xC2, 0xB8, 0xB2, 0x3F];
const EXE_NORMALIZED_SHA: &str = "5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67";
// Verified Windows PC 1.0.2.0 SafeDisc v1 loader / companion executable.
const PROTECTED_102_SHA: &str = "675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736";
const ICD_102_SHA: &str = "0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b";
const ICD_102_LEN: usize = 2_580_578;
const DPLAYERX_15020_SHA: &str = "dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff";
const DPLAYERX_15020_LEN: usize = 138_752;
const SD_DPLAYERX_MAGIC: usize = 0x1CC58;
const DPLAYERX_MODIFIER_RVAS: [u32; 2] = [0x65BE, 0x6621];
// The exact checked 1.0.2.0 originals identify the DRM itself as SafeDisc 1.50.20.
const SD_15020_MAGIC: &[u8] = b"BoG_ *90.0&!!  Yy>";
const SD_LOADER_MAGIC: usize = 0x29250;
const SD_ICD_MAGIC: usize = 0x0FD4;
fn checked_safedisc_version(data: &[u8], magic: usize, version_at: usize) -> Result<()> {
    if data.get(magic..magic+SD_15020_MAGIC.len()) != Some(SD_15020_MAGIC) {
        return Err("SafeDisc version marker mismatch".into());
    }
    let numbers=[read_u32(data,version_at)?,read_u32(data,version_at+4)?,read_u32(data,version_at+8)?];
    if numbers!=[1,50,20] {
        return Err(format!("unsupported SafeDisc protection version {}.{}.{}",numbers[0],numbers[1],numbers[2]));
    }
    Ok(())
}
const ROF_ORIGINAL_SHA: &str = "1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a";
const ROF_RVA: u32 = 0x231E;
const ROF_ORIGINAL: [u8; 8] = [0x80, 0x3E, 0x20, 0x7D, 0x01, 0x46, 0x89, 0x37];
const ROF_PATCHED: [u8; 8] = [0x80, 0x3E, 0x20, 0x7D, 0x01, 0x90, 0x89, 0x37];

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String { e.to_string() }

// Minimal SHA-256, so the finished binary has no third-party/runtime dependencies.
fn sha256(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let bits = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&bits.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes(chunk[4*i..4*i+4].try_into().unwrap()); }
        for i in 16..64 {
            let x = w[i-15]; let y = w[i-2];
            let s0 = x.rotate_right(7)^x.rotate_right(18)^(x>>3);
            let s1 = y.rotate_right(17)^y.rotate_right(19)^(y>>10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut z] = h;
        for i in 0..64 {
            let s1=e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
            let ch=(e&f)^((!e)&g);
            let t1=z.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0=a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
            let maj=(a&b)^(a&c)^(b&c);
            let t2=s0.wrapping_add(maj);
            z=g;g=f;f=e;e=d.wrapping_add(t1);d=c;c=b;b=a;a=t1.wrapping_add(t2);
        }
        for (x,y) in h.iter_mut().zip([a,b,c,d,e,f,g,z]) { *x=x.wrapping_add(y); }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}
fn read_u16(b: &[u8], p: usize) -> Result<u16> { Ok(u16::from_le_bytes(b.get(p..p+2).ok_or("truncated u16")?.try_into().unwrap())) }
fn read_u32(b: &[u8], p: usize) -> Result<u32> { Ok(u32::from_le_bytes(b.get(p..p+4).ok_or("truncated u32")?.try_into().unwrap())) }
fn write_u32(b: &mut [u8], p: usize, n: u32) { b[p..p+4].copy_from_slice(&n.to_le_bytes()); }
fn read_f32(b: &[u8], p: usize) -> Result<f32> { Ok(f32::from_le_bytes(b.get(p..p+4).ok_or("truncated float")?.try_into().unwrap())) }
fn write_f32(b: &mut [u8], p: usize, n: f32) { b[p..p+4].copy_from_slice(&n.to_le_bytes()); }

fn valid_pe(data: &[u8]) -> Result<usize> {
    if data.len()<0x40 || data.get(..2)!=Some(&b"MZ"[..]) { return Err("not an MZ executable".into()); }
    let p = read_u32(data,0x3c)? as usize;
    if data.get(p..p.checked_add(24).ok_or("PE overflow")?).is_none() || data.get(p..p+4)!=Some(&b"PE\0\0"[..]) { return Err("bad PE header".into()); }
    if read_u16(data,p+4)? != 0x14c { return Err("expected 32-bit x86 PE".into()); }
    Ok(p)
}
fn pe_rva_to_offset(data: &[u8], rva: u32) -> Result<usize> {
    let pe = valid_pe(data)?;
    let n = read_u16(data,pe+6)? as usize;
    let size = read_u16(data,pe+20)? as usize;
    let start = (pe+24).checked_add(size).ok_or("section table overflow")?;
    for i in 0..n {
        let s = start.checked_add(i.checked_mul(40).ok_or("section overflow")?).ok_or("section overflow")?;
        if data.get(s..s+40).is_none() { return Err("truncated section table".into()); }
        let vsize=read_u32(data,s+8)?;
        let va=read_u32(data,s+12)?;
        let rawsize=read_u32(data,s+16)?;
        let rawoff=read_u32(data,s+20)?;
        let mapped=if vsize==0 {rawsize} else {vsize.min(rawsize)};
        if rva>=va && rva-va<mapped {
            let pos=rawoff as usize+(rva-va) as usize;
            if pos<data.len() { return Ok(pos); }
            return Err("RVA maps outside file".into());
        }
    }
    Err("RVA not mapped by PE sections".into())
}

#[derive(PartialEq,Debug,Clone,Copy)]
enum ExeKind { Protected, Widescreen, Unknown }
fn classify_exe(data: &[u8]) -> ExeKind {
    if valid_pe(data).is_err() { return ExeKind::Unknown; }
    let sha=sha256(data);
    if sha == PROTECTED_102_SHA { return ExeKind::Protected; }
    if data.len()!=EXE_LEN { return ExeKind::Unknown; }
    let mut normalized=data.to_vec();
    write_u32(&mut normalized,WIDTH,1920);
    write_u32(&mut normalized,HEIGHT,1080);
    normalized[FOV60..FOV60+4].copy_from_slice(&DEFAULT_60);
    normalized[FOV80..FOV80+4].copy_from_slice(&DEFAULT_80);
    if sha256(&normalized)==EXE_NORMALIZED_SHA { ExeKind::Widescreen } else { ExeKind::Unknown }
}
fn geometry(width: u32, height: u32) -> Result<()> {
    if !(320..=16384).contains(&width) || !(240..=16384).contains(&height) { return Err("resolution outside allowed range".into()); }
    let aspect=width as f64/height as f64;
    if !(0.5..=4.0).contains(&aspect) { return Err("unsupported aspect ratio".into()); }
    Ok(())
}
fn horizontal_fov(reference: f64,width: u32,height: u32) -> f64 {
    let ratio=(width as f64/height as f64)/(4.0/3.0);
    2.0*((reference.to_radians()/2.0).tan()*ratio).atan().to_degrees()
}
fn make_exe_patch(source: &[u8], width: Option<u32>, height: Option<u32>, fov: Option<f64>) -> Result<Vec<u8>> {
    if classify_exe(source)!=ExeKind::Widescreen { return Err("unrecognized widescreen EXE version".into()); }
    let w=width.unwrap_or(read_u32(source,WIDTH)?);
    let h=height.unwrap_or(read_u32(source,HEIGHT)?);
    geometry(w,h)?;
    let mut out=source.to_vec();
    write_u32(&mut out,WIDTH,w);write_u32(&mut out,HEIGHT,h);
    if let Some(target)=fov {
        if !target.is_finite() || !(10.0..=160.0).contains(&target) { return Err("FOV must be between 10 and 160 degrees".into()); }
        let factor=(w as f64/h as f64)/(4.0/3.0);
        let multiplier=(target.to_radians()/2.0).tan()/((80f64.to_radians()/2.0).tan()*factor);
        let ref80=2.0*((80f64.to_radians()/2.0).tan()*multiplier).atan();
        let ref60=2.0*((60f64.to_radians()/2.0).tan()*multiplier).atan();
        write_f32(&mut out,FOV80,ref80 as f32);
        write_f32(&mut out,FOV60,ref60 as f32);
    } else {
        // Automatic aspect scaling from the original widescreen patch's 80/60-degree references.
        out[FOV80..FOV80+4].copy_from_slice(&DEFAULT_80);
        out[FOV60..FOV60+4].copy_from_slice(&DEFAULT_60);
    }
    if classify_exe(&out)!=ExeKind::Widescreen { return Err("post-patch EXE validation failed".into()); }
    Ok(out)
}
fn rof_patch(original: &[u8]) -> Result<Vec<u8>> {
    if sha256(original)!=ROF_ORIGINAL_SHA { return Err("unsupported roffile.dll hash".into()); }
    let offset=pe_rva_to_offset(original,ROF_RVA)?;
    if offset<5 || original.get(offset-5..offset+3)!=Some(&ROF_ORIGINAL[..]) { return Err("ROF instruction context mismatch".into()); }
    let mut patched=original.to_vec();patched[offset]=0x90;
    if patched.get(offset-5..offset+3)!=Some(&ROF_PATCHED[..]) { return Err("ROF patch self-check failed".into()); }
    Ok(patched)
}
fn inspect_file(path: &Path) -> Result<Vec<u8>> {
    let meta=fs::symlink_metadata(path).map_err(err)?;
    if meta.file_type().is_symlink() || !meta.is_file() { return Err(format!("refusing symlink/non-regular input: {}",path.display())); }
    fs::read(path).map_err(err)
}
fn backup_path(path: &Path, rof: bool) -> PathBuf {
    let suffix=if rof {".before-nul-guard"} else {".crimson-original.bak"};
    let mut out=path.as_os_str().to_os_string();out.push(suffix);PathBuf::from(out)
}
fn optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(m) => {
            if m.file_type().is_symlink() || !m.is_file() { return Err(format!("backup is a symlink/non-file: {}",path.display())); }
            Ok(Some(fs::read(path).map_err(err)?))
        }
        Err(e) if e.kind()==io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}
fn preserve_permissions(src: &Path, dst: &Path) -> Result<()> {
    let permissions=fs::metadata(src).map_err(err)?.permissions();
    fs::set_permissions(dst,permissions).map_err(err)
}
fn create_backup(path: &Path, expected_current: &[u8], rof: bool) -> Result<()> {
    let target=backup_path(path,rof);
    if let Some(old)=optional_file(&target)? {
        // Existing backup must be a validated original; never overwrite it.
        if rof {
            if sha256(&old)!=ROF_ORIGINAL_SHA { return Err(format!("invalid existing ROF backup: {}",target.display())); }
        } else if !matches!(classify_exe(&old), ExeKind::Protected|ExeKind::Widescreen) {
            return Err(format!("invalid existing EXE backup: {}",target.display()));
        }
        // If source equals an unpatched supported file, backup must match or be an older protected original.
        if !rof && classify_exe(expected_current)==ExeKind::Protected && old!=expected_current {
            return Err("protected EXE differs from existing backup; refusing".into());
        }
        return Ok(());
    }
    let mut f=OpenOptions::new().write(true).create_new(true).open(&target).map_err(err)?;
    if let Err(e)=f.write_all(expected_current).and_then(|()| f.sync_all()) {
        drop(f);let _=fs::remove_file(&target);return Err(err(e));
    }
    drop(f);
    preserve_permissions(path,&target)?;
    if inspect_file(&target)?!=expected_current { return Err(format!("backup verification failed: {}",target.display())); }
    println!("Backup created: {}",target.display());
    Ok(())
}
fn atomic_replace(path: &Path, data: &[u8]) -> Result<()> {
    // Same-directory temporary file prevents partially written targets; never follow a target symlink.
    let _=inspect_file(path)?;
    let name=path.file_name().ok_or("missing file name")?.to_string_lossy();
    let parent=path.parent().unwrap_or_else(||Path::new("."));
    let mut tmp=None;
    for count in 0..100u32 {
        let candidate=parent.join(format!(".{name}.crimson-{}-{count}.tmp",process::id()));
        match OpenOptions::new().create_new(true).write(true).open(&candidate) {
            Ok(f) => {tmp=Some((candidate,f));break;},
            Err(e) if e.kind()==io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(err(e)),
        }
    }
    let (tmp_path,mut f)=tmp.ok_or("could not reserve a temporary file")?;
    let result=(||->Result<()> {
        preserve_permissions(path,&tmp_path)?;
        f.write_all(data).map_err(err)?;
        f.sync_all().map_err(err)?;
        drop(f);
        // Re-check the target before replacement (symlink mitigation, not a full lock).
        let _=inspect_file(path)?;
        fs::rename(&tmp_path,path).map_err(err)?;
        if inspect_file(path)?!=data { return Err("write verification failed".into()); }
        Ok(())
    })();
    if result.is_err() {let _=fs::remove_file(&tmp_path);}
    result
}
fn find_ci_child(parent: &Path, name: &str) -> Option<PathBuf> {
    for e in fs::read_dir(parent).ok()?.flatten() {
        if e.file_name().to_string_lossy().eq_ignore_ascii_case(name) {return Some(e.path());}
    }
    None
}
fn discover_rof(exe: &Path) -> Option<PathBuf> {
    let dir=exe.parent()?;
    let mut path=dir.to_path_buf();
    for part in ["GOSDATA","ASSETS","BINARIES","roffile.dll"] {
        path=find_ci_child(&path,part)?;
    }
    Some(path)
}
#[derive(Default)]
struct Options {
    command:String,exe:Option<PathBuf>,icd:Option<PathBuf>,reference:Option<PathBuf>,dplayerx:Option<PathBuf>,width:Option<u32>,height:Option<u32>,fov:Option<f64>,
    unpacked:Option<PathBuf>,allow_replacement:bool,wine:Option<bool>,dll:Option<PathBuf>,
}
fn help() {
    println!("Crimson Skies patcher (Rust, no crates)\n\
Usage:\n\
  crimson-patcher status --exe CRIMSON.EXE [--icd CRIMSON.ICD] [--dll roffile.dll] [--wine]\n\
  crimson-patcher probe --exe CRIMSON.EXE [--icd CRIMSON.ICD] [--reference LOCAL_UNPACKED_EXE] [--dplayerx DPLAYERX.DLL]\n\
  crimson-patcher apply --exe CRIMSON.EXE [--icd CRIMSON.ICD] --width 1920 --height 1080 [--fov 105]\n\
      [--unpacked LOCAL_WIDESCREEN_EXE --accept-local-replacement] [--wine|--no-wine] [--dll PATH]\n\
  crimson-patcher restore --exe CRIMSON.EXE [--wine|--no-wine] [--dll PATH]\n\
Wine DLL support auto-enables on native Linux and detectable Wine environments.\n\
SafeDisc 1.02: both the original loader EXE and companion ICD are verified before replacement.\n\
No autonomous SafeDisc decryption yet: replacement REQUIRES the user's own verified unpacked EXE.\n\
The retail EXE, DLL, and game files are NEVER bundled.");
}
fn parse_args() -> Result<Options> {
    let mut argv=env::args().skip(1);
    let mut args=Options {command:argv.next().unwrap_or_default(),..Default::default()};
    if args.command=="--help" || args.command=="-h" {help();process::exit(0)}
    if !["status","probe","apply","restore"].contains(&args.command.as_str()) { help();return Err("expected status, apply, or restore".into()); }
    while let Some(key)=argv.next() {
        match key.as_str() {
            "--exe"=>args.exe=Some(PathBuf::from(argv.next().ok_or("missing --exe value")?)),
            "--icd"=>args.icd=Some(PathBuf::from(argv.next().ok_or("missing --icd value")?)),
            "--reference"=>args.reference=Some(PathBuf::from(argv.next().ok_or("missing --reference value")?)),
            "--dplayerx"=>args.dplayerx=Some(PathBuf::from(argv.next().ok_or("missing --dplayerx value")?)),
            "--width"=>args.width=Some(argv.next().ok_or("missing width")?.parse().map_err(|_|"invalid width")?),
            "--height"=>args.height=Some(argv.next().ok_or("missing height")?.parse().map_err(|_|"invalid height")?),
            "--fov"=>args.fov=Some(argv.next().ok_or("missing fov")?.parse().map_err(|_|"invalid FOV")?),
            "--dll"=>args.dll=Some(PathBuf::from(argv.next().ok_or("missing DLL path")?)),
            "--unpacked"=>args.unpacked=Some(PathBuf::from(argv.next().ok_or("missing unpacked path")?)),
            "--accept-local-replacement"=>args.allow_replacement=true,
            "--wine"=>args.wine=Some(true),
            "--no-wine"=>args.wine=Some(false),
            "--help"|"-h"=>{help();process::exit(0)},
            _=>return Err(format!("unknown option: {key}")),
        }
    }
    if args.exe.is_none() {return Err("--exe is mandatory".into());}
    if (args.reference.is_some() || args.dplayerx.is_some()) && args.command!="probe" {return Err("--reference/--dplayerx are only valid for probe".into());}
    if args.command=="probe" && (args.dll.is_some() || args.width.is_some() || args.height.is_some() || args.fov.is_some() || args.unpacked.is_some() || args.allow_replacement || args.wine.is_some()) {
        return Err("probe accepts only --exe, --icd and optional --reference".into());
    }
    if args.command!="apply" && (args.width.is_some()||args.height.is_some()||args.fov.is_some()||args.unpacked.is_some()||args.allow_replacement) { return Err("patch options only valid for apply".into()); }
    if args.command=="apply" && (args.width.is_some()!=args.height.is_some()) {return Err("--width and --height must be specified together".into());}
    Ok(args)
}
fn wine_mode(options: &Options) -> bool {
    options.wine.unwrap_or_else(|| cfg!(target_os="linux")||env::var_os("WINEPREFIX").is_some()||env::var_os("WINELOADERNOEXEC").is_some())
}
fn target_dll(options: &Options, exe: &Path) -> Result<Option<PathBuf>> {
    if !wine_mode(options) && options.dll.is_none() {return Ok(None)}
    if let Some(path)=options.dll.clone() {return Ok(Some(path))}
    match discover_rof(exe) {
        Some(path)=>Ok(Some(path)),
        None if options.wine==Some(true)=>Err("Wine mode requested but roffile.dll not found; pass --dll PATH".into()),
        None=>{eprintln!("NOTE: roffile.dll not found, skipping optional Wine fix (use --dll PATH to locate it).");Ok(None)}
    }
}
/// Verify the exact SafeDisc 1.0.2.0 ICD before attempting any protected-EXE
/// replacement. The ICD contains encrypted code and is NOT itself an unpacked EXE.
fn check_original_icd(path: &Path) -> Result<()> {
    let data = inspect_file(path)?;
    if data.len() != ICD_102_LEN || valid_pe(&data).is_err() || sha256(&data) != ICD_102_SHA {
        return Err(format!(
            "unsupported CRIMSON.ICD: expected the verified 1.0.2.0 ICD (SHA-256 {}); no files changed",
            ICD_102_SHA
        ));
    }
    checked_safedisc_version(&data, SD_ICD_MAGIC, SD_ICD_MAGIC+0x20)?;
    println!("ICD: game 1.0.2.0; SafeDisc 1.50.20 verified (SHA-256 {}) at {}", ICD_102_SHA, path.display());
    Ok(())
}

fn validate_protected_pair(options: &Options, exe: &Path, kind: ExeKind) -> Result<()> {
    if kind != ExeKind::Protected {
        if options.icd.is_some() && kind == ExeKind::Unknown {
            return Err("cannot validate ICD with an unsupported EXE".into());
        }
        return Ok(());
    }
    // Restoring an already backed-up executable does not require the ICD.
    if options.command == "restore" { return Ok(()); }
    let icd = options.icd.clone().or_else(|| {
        exe.parent().and_then(|parent| find_ci_child(parent, "CRIMSON.ICD"))
    });
    match icd {
        Some(path) => check_original_icd(&path),
        None if options.command == "status" => {
            println!("ICD: companion CRIMSON.ICD not found; protected pair not yet verified");
            Ok(())
        },
        None => Err("SafeDisc 1.0.2.0 requires its matching CRIMSON.ICD; provide --icd PATH or place it next to CRIMSON.EXE; no files changed".into()),
    }
}


fn section_ranges(data: &[u8]) -> Result<Vec<(String, usize, usize)>> {
    let pe=valid_pe(data)?;
    let count=read_u16(data,pe+6)? as usize;
    let optional=read_u16(data,pe+20)? as usize;
    let start=(pe+24).checked_add(optional).ok_or("section header overflow")?;
    let mut ranges=Vec::new();
    if count>96 {return Err("unreasonable PE section count".into());}
    for i in 0..count {
        let sh=start.checked_add(i.checked_mul(40).ok_or("section offset overflow")?).ok_or("section offset overflow")?;
        let h=data.get(sh..sh+40).ok_or("truncated PE section header")?;
        let name=String::from_utf8_lossy(&h[..8]).trim_end_matches('\0').to_string();
        let len=read_u32(data,sh+16)? as usize;
        let offset=read_u32(data,sh+20)? as usize;
        if data.get(offset..offset.checked_add(len).ok_or("section length overflow")?).is_none() {
            return Err(format!("truncated PE section {}",name));
        }
        ranges.push((name,offset,len));
    }
    Ok(ranges)
}
fn byte_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {return 0.0;}
    let mut counts=[0usize;256];
    for b in data {counts[*b as usize]+=1;}
    let n=data.len() as f64;
    counts.iter().filter(|&&x|x>0).map(|&x|{
        let p=x as f64/n;
        -p*p.log2()
    }).sum()
}
fn probe_dplayerx(dll: &[u8]) -> Result<()> {
    valid_pe(dll)?;
    let hash = sha256(dll);
    if dll.len()!=DPLAYERX_15020_LEN || hash!=DPLAYERX_15020_SHA {
        return Err(format!(
            "unknown dplayerx.dll (length {}; SHA-256 {}). Expected exact SafeDisc 1.50.20 DLL {}; no decryption attempted",
            dll.len(),hash,DPLAYERX_15020_SHA
        ));
    }
    checked_safedisc_version(dll,SD_DPLAYERX_MAGIC,SD_DPLAYERX_MAGIC+0x14)?;
    println!("DPLAYERX: matching SafeDisc 1.50.20 DLL, {} bytes; SHA-256 {}",dll.len(),hash);
    let sections=section_ranges(dll)?;
    for (name,off,len) in &sections {
        if name==".txt" || name==".text" || name==".txt2" {
            println!("DPLAYERX {:8} raw=0x{:06X} size={} entropy={:.3} bits/byte",
                     name,off,len,byte_entropy(&dll[*off..*off+*len]));
        }
    }
    for rva in DPLAYERX_MODIFIER_RVAS {
        let offset=pe_rva_to_offset(dll,rva)?;
        let section=sections.iter().find(|(_,off,len)|offset>=*off && offset<*off+*len)
            .ok_or("modifier RVA did not map to an inspected DLL section")?;
        println!("Research modifier RVA 0x{:X} -> file offset 0x{:X} in section {} (encrypted on disk)",
                 rva,offset,section.0);
    }
    println!("DPLAYERX: modifier addresses refer to ciphertext on disk. Runtime unpacking / actual decoder not yet established.");
    Ok(())
}

fn research_probe(exe: &[u8], icd: &[u8], reference: Option<&[u8]>, dplayerx: Option<&[u8]>) -> Result<()> {
    if classify_exe(exe)!=ExeKind::Protected || icd.len()!=ICD_102_LEN || sha256(icd)!=ICD_102_SHA {
        return Err("probe requires the verified original SafeDisc 1.0.2.0 EXE and ICD".into());
    }
    checked_safedisc_version(exe, SD_LOADER_MAGIC, SD_LOADER_MAGIC+0x14)?;
    checked_safedisc_version(icd, SD_ICD_MAGIC, SD_ICD_MAGIC+0x20)?;
    println!("Crimson Skies 1.0.2.0 — SafeDisc 1.50.20 research probe (read only)");
    println!("NOTE: SafeDisc 1.50.20 uses an additional ICD cipher layer involving dplayerx.dll.");
    if let Some(dll)=dplayerx {
        probe_dplayerx(dll)?;
        println!("The DLL is inspected only; no code is executed or copied into an output.");
    } else {
        println!("dplayerx.dll not provided: its SafeDisc-specific decoder still needs to be analyzed.");
    }
    println!("Protected loader SHA-256: {}",sha256(exe));
    println!("ICD SHA-256: {}",sha256(icd));
    println!("ICD byte length: {} ({} bytes beyond unpacked widescreen reference size)",icd.len(),icd.len().saturating_sub(EXE_LEN));
    let marker=b"DECRPE\0\0";
    let matches:Vec<_>=exe.windows(marker.len()).enumerate().filter(|(_,w)|*w==&marker[..]).map(|(i,_)|i).collect();
    if matches.len()!=1 {return Err("SafeDisc loader DECRPE marker is not unique".into());}
    let pos=matches[0];
    println!("DECRPE marker at loader file offset 0x{:X}",pos);
    if let Some(raw)=exe.get(pos+8..pos+24) {
        print!("16 bytes following marker (UNVERIFIED key material): ");
        for b in raw {print!("{:02X}",b);}
        println!();
    }
    println!("WARNING: DECRPE data is not itself a verified TEA key. No decryption is attempted.");
    let icd_sections=section_ranges(icd)?;
    let ref_sections=match reference {
        Some(r) if classify_exe(r)==ExeKind::Widescreen => Some(section_ranges(r)?),
        Some(_)=>return Err("reference must be an already verified widescreen executable (read-only comparison)".into()),
        None=>None,
    };
    for (name,off,len) in icd_sections {
        let data=&icd[off..off+len];
        print!("ICD {:8} raw={:#08x} size={:7} entropy={:.3} bits/byte",name,off,len,byte_entropy(data));
        if let (Some(reference),Some(ref ranges))=(reference,&ref_sections) {
            if let Some((_,ro,rl))=ranges.iter().find(|(n,_,_)|n==&name) {
                let n=len.min(*rl);
                let changes=data[..n].iter().zip(reference[*ro..*ro+n].iter()).filter(|(a,b)|a!=b).count();
                print!(" vs reference: {} / {} bytes changed ({:.2}%)",changes,n,100.0*changes as f64/n.max(1) as f64);
            }
        }
        println!();
    }
    Ok(())
}

fn report_exe(data: &[u8]) {
    let state=classify_exe(data);
    println!("EXE: {state:?}; bytes: {}; SHA-256: {}",data.len(),sha256(data));
    if state==ExeKind::Widescreen {
        let w=read_u32(data,WIDTH).unwrap();let h=read_u32(data,HEIGHT).unwrap();
        let a=read_f32(data,FOV80).unwrap().to_degrees() as f64;
        let b=read_f32(data,FOV60).unwrap().to_degrees() as f64;
        println!("Resolution: {w}x{h}; FOV A: {:.2} deg (predicted {:.2}); FOV B: {:.2} deg (predicted {:.2})",a,horizontal_fov(a,w,h),b,horizontal_fov(b,w,h));
    }
}
fn dll_patch_if_supported(data: &[u8], backup: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    if sha256(data)==ROF_ORIGINAL_SHA {return Ok(Some(rof_patch(data)?));}
    if let Some(original)=backup {
        let patched=rof_patch(original)?;
        if data==patched {return Ok(Some(patched));}
    }
    Ok(None)
}
fn run(options: Options) -> Result<()> {
    let exe=options.exe.as_ref().unwrap();
    let before=inspect_file(exe)?;
    report_exe(&before);
    let kind=classify_exe(&before);
    validate_protected_pair(&options, exe, kind)?;
    if options.command=="probe" {
        if kind!=ExeKind::Protected {return Err("probe requires the verified SafeDisc 1.0.2.0 loader EXE".into());}
        let icd_path=options.icd.clone().or_else(||exe.parent().and_then(|parent|find_ci_child(parent,"CRIMSON.ICD"))).ok_or("probe requires CRIMSON.ICD")?;
        let icd_data=inspect_file(&icd_path)?;
        let reference=match &options.reference {Some(path)=>Some(inspect_file(path)?),None=>None};
        let dplayerx=match &options.dplayerx {Some(path)=>Some(inspect_file(path)?),None=>None};
        return research_probe(&before,&icd_data,reference.as_deref(),dplayerx.as_deref());
    }
    let exe_backup=backup_path(exe,false);
    let backup=optional_file(&exe_backup)?;
    if let Some(ref b)=backup {
        if !matches!(classify_exe(b),ExeKind::Protected|ExeKind::Widescreen) {return Err("unrecognized EXE backup; refusing".into());}
        println!("EXE backup: verified supported file: {}",exe_backup.display());
    } else {println!("EXE backup: absent");}
    let dll=target_dll(&options,exe)?;
    let mut dll_before=None;let mut dll_backup=None;let mut dll_after=None;
    if let Some(ref path)=dll {
        let data=inspect_file(path)?;
        let back=optional_file(&backup_path(path,true))?;
        let proposed=dll_patch_if_supported(&data,back.as_deref())?;
        println!("ROF: sha256={} state={}",sha256(&data),if sha256(&data)==ROF_ORIGINAL_SHA {"original"} else if proposed.as_deref()==Some(data.as_slice()) {"patched"} else {"unknown"});
        dll_after=proposed;dll_before=Some(data);dll_backup=back;
    }
    if options.command=="status" {return Ok(())}
    if options.command=="restore" {
        // Preflight both restorations before writing either target.
        if backup.is_some() && kind==ExeKind::Unknown {
            return Err("current EXE not recognized; refusing restore".into());
        }
        if let (Some(ref current),Some(ref original))=(&dll_before,&dll_backup) {
            let patched=rof_patch(original)?;
            if current!=original && current!=&patched {
                return Err("ROF not an expected patched or original build; refusing restore".into());
            }
        }
        if let Some(ref original)=backup {
            if before!=*original {atomic_replace(exe,original)?;println!("EXE restored from {}",exe_backup.display());}
        } else {println!("No EXE backup; nothing to restore");}
        if let (Some(path),Some(current),Some(original))=(dll,dll_before,dll_backup) {
            if current!=original {atomic_replace(&path,&original)?;println!("ROF restored");}
        }
        return Ok(())
    }
    // Preflight every relevant component before ANY mutation.
    if kind==ExeKind::Unknown {return Err("unsupported EXE build; no files changed".into());}
    let source=if kind==ExeKind::Protected {
        if !options.allow_replacement {return Err("SafeDisc EXE: requires --unpacked PATH AND --accept-local-replacement; cannot decrypt in place".into());}
        let path=options.unpacked.as_ref().ok_or("SafeDisc EXE requires --unpacked PATH")?;
        let local=inspect_file(path)?;
        if classify_exe(&local)!=ExeKind::Widescreen {return Err("--unpacked is not one of the verified widescreen EXEs".into());}
        println!("SafeDisc will be replaced using the user's own local unpacked image (not decrypted by this tool).");
        local
    } else {
        if options.unpacked.is_some()||options.allow_replacement {return Err("--unpacked is only accepted for protected EXEs".into());}
        before.clone()
    };
    let after=make_exe_patch(&source,options.width,options.height,options.fov)?;
    let dll_needs_patch=match (dll_before.as_ref(),dll_after.as_ref()) {
        (Some(before),Some(after)) => before!=after,
        (Some(_),None)=>return Err("unknown roffile.dll build; refusing all changes (use --no-wine to skip or supply supported DLL)".into()),
        _=>false,
    };
    if dll_needs_patch {
        if let Some(ref existing)=dll_backup {
            if sha256(existing)!=ROF_ORIGINAL_SHA {return Err("invalid ROF backup".into());}
        }
    }
    let exe_needs_patch=before!=after;
    if !exe_needs_patch && !dll_needs_patch {println!("Already patched with these options; no files changed.");return Ok(())}
    if exe_needs_patch {create_backup(exe,&before,false)?;}
    if dll_needs_patch {create_backup(dll.as_ref().unwrap(),dll_before.as_ref().unwrap(),true)?;}
    if exe_needs_patch {atomic_replace(exe,&after)?;println!("EXE patched: SHA-256 {}",sha256(&after));}
    if dll_needs_patch {
        let path=dll.as_ref().unwrap();
        if let Err(error)=atomic_replace(path,dll_after.as_ref().unwrap()) {
            if exe_needs_patch { if let Err(rollback)=atomic_replace(exe,&before) {return Err(format!("ROF failed: {error}; EXE rollback ALSO failed: {rollback}; restore backups manually"));} }
            return Err(format!("ROF failed: {error}; EXE reverted to its prior contents"));
        }
        println!("ROF patched: {}",path.display());
    }
    println!("Done. Original backups retained. Verify behavior in-game before distributing.");
    Ok(())
}
fn main() {
    match parse_args().and_then(run) {
        Ok(())=>{},
        Err(e)=>{eprintln!("ERROR: {e}");process::exit(1);}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sha_vectors() {
        assert_eq!(sha256(b""),"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    #[test] fn fov_4x3() {assert!((horizontal_fov(80.0,1024,768)-80.0).abs()<1e-10);}
    #[test] fn fov_16x9() {assert!((horizontal_fov(80.0,1920,1080)-96.418).abs()<0.03);}
    #[test] fn dimensions() {assert!(geometry(1920,1080).is_ok());assert!(geometry(1,0).is_err());}
    #[test] fn matching_dplayerx_metadata() {
        assert_eq!(DPLAYERX_15020_LEN, 138_752);
        assert_eq!(DPLAYERX_15020_SHA.len(), 64);
        assert_eq!(DPLAYERX_MODIFIER_RVAS[1]-DPLAYERX_MODIFIER_RVAS[0],0x63);
    }
    #[test] fn safedisc_metadata_signature() {
        assert_eq!(SD_15020_MAGIC, b"BoG_ *90.0&!!  Yy>");
        let mut buf=vec![0u8;64];
        buf[0..18].copy_from_slice(SD_15020_MAGIC);
        buf[32..36].copy_from_slice(&1u32.to_le_bytes());
        buf[36..40].copy_from_slice(&50u32.to_le_bytes());
        buf[40..44].copy_from_slice(&20u32.to_le_bytes());
        assert!(checked_safedisc_version(&buf,0,32).is_ok());
        buf[40..44].copy_from_slice(&21u32.to_le_bytes());
        assert!(checked_safedisc_version(&buf,0,32).is_err());
    }
    #[test] fn probe_entropy_and_sections() {
        assert_eq!(byte_entropy(&[0u8;64]),0.0);
        assert!((byte_entropy(&[0u8,1u8])-1.0).abs()<1e-10);
    }
    #[test] fn supported_protected_hash_is_102() {
        assert_eq!(PROTECTED_102_SHA, "675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736");
        assert_eq!(ICD_102_LEN, 2_580_578);
        assert_eq!(ICD_102_SHA.len(), 64);
    }
}