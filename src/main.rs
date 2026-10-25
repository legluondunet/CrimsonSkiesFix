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
const PROTECTED_SHAS: [&str; 2] = [
    "675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736",
    "7d6029471d4e5f2c7f4b08ce86c0ffd1f9bc8c697cf3952695fd68ff011f25a1",
];
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
    if PROTECTED_SHAS.contains(&sha.as_str()) { return ExeKind::Protected; }
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
    command:String,exe:Option<PathBuf>,width:Option<u32>,height:Option<u32>,fov:Option<f64>,
    unpacked:Option<PathBuf>,allow_replacement:bool,wine:Option<bool>,dll:Option<PathBuf>,
}
fn help() {
    println!("Crimson Skies patcher (Rust, no crates)\n\
Usage:\n\
  crimson-patcher status --exe CRIMSON.EXE [--dll roffile.dll] [--wine]\n\
  crimson-patcher apply --exe CRIMSON.EXE --width 1920 --height 1080 [--fov 105]\n\
      [--unpacked LOCAL_WIDESCREEN_EXE --accept-local-replacement] [--wine|--no-wine] [--dll PATH]\n\
  crimson-patcher restore --exe CRIMSON.EXE [--wine|--no-wine] [--dll PATH]\n\
Wine DLL support auto-enables on native Linux and detectable Wine environments.\n\
SafeDisc conversion REQUIRES the user's own verified unpacked EXE; this tool cannot decrypt SafeDisc.\n\
The retail EXE, DLL, and game files are NEVER bundled.");
}
fn parse_args() -> Result<Options> {
    let mut argv=env::args().skip(1);
    let mut args=Options {command:argv.next().unwrap_or_default(),..Default::default()};
    if args.command=="--help" || args.command=="-h" {help();process::exit(0)}
    if !["status","apply","restore"].contains(&args.command.as_str()) { help();return Err("expected status, apply, or restore".into()); }
    while let Some(key)=argv.next() {
        match key.as_str() {
            "--exe"=>args.exe=Some(PathBuf::from(argv.next().ok_or("missing --exe value")?)),
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
    #[test] fn protected_hashes() {assert_eq!(PROTECTED_SHAS.len(),2);}
}