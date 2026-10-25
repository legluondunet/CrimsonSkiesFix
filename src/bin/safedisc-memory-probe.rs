// Optional SafeDisc 1.50.20 DLL memory observer for Windows x86 only.
// Copyright (C) 2026 legluondunet
// SPDX-License-Identifier: GPL-3.0-or-later
// WARNING: LoadLibraryW executes this DLL's initialization code. Only run
// the exact hash-verified DLL in an isolated/test Wine prefix.

use std::env;
use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

const DLL_SIZE: usize = 138_752;
const DLL_SHA256: &str = "dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff";
const TXT_RAW_OFF: usize = 0x400;
const TXT_RVA: usize = 0x1000;
const TXT_VSIZE: usize = 0x7671;
const TXT_RAW_SIZE: usize = 0x7800;

#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

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

fn entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {return 0.0;}
    let mut counts=[0usize;256];
    for b in bytes {counts[*b as usize]+=1;}
    let length=bytes.len() as f64;
    counts.iter().filter(|&&n|n>0).map(|&n|{
        let p=n as f64/length;
        -p*p.log2()
    }).sum()
}

fn run() -> Result<(),String> {
    if cfg!(not(target_pointer_width="32")) {
        return Err("This helper must be built as a Windows x86 (32-bit) program".into());
    }
    let mut args=env::args_os().skip(1);
    let (mut dll,mut output)=(None::<PathBuf>,None::<PathBuf>);
    while let Some(arg)=args.next() {
        if arg=="--dll" {dll=Some(args.next().ok_or("Missing --dll path")?.into());}
        else if arg=="--output" {output=Some(args.next().ok_or("Missing --output path")?.into());}
        else {return Err(format!("Unknown option: {:?}. Usage: safedisc-memory-probe --dll dplayerx.dll --output dplayerx-txt-memory.bin",arg));}
    }
    let dll=dll.ok_or("--dll is required")?;
    let output=output.ok_or("--output is required")?;
    let canonical_dll=fs::canonicalize(&dll).map_err(|e|e.to_string())?;
    let meta=fs::symlink_metadata(&canonical_dll).map_err(|e|e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink(){return Err("DLL must be a regular file".into());}
    if fs::symlink_metadata(&output).is_ok() {return Err("Output exists; refusing to overwrite it".into());}
    let disk=fs::read(&canonical_dll).map_err(|e|e.to_string())?;
    if disk.len()!=DLL_SIZE || sha256(&disk)!=DLL_SHA256 {
        return Err("Unrecognized DPLAYERX.DLL: exact SHA-256 and length do not match verified SafeDisc 1.50.20 build".into());
    }
    if disk.get(TXT_RAW_OFF..TXT_RAW_OFF+TXT_RAW_SIZE).is_none() ||
       disk.get(..2)!=Some(&b"MZ"[..]) {return Err("Unexpected DLL PE image".into());}
    let old=&disk[TXT_RAW_OFF..TXT_RAW_OFF+TXT_VSIZE];
    let mut wide:Vec<u16>=canonical_dll.as_os_str().encode_wide().collect();
    wide.push(0);
    eprintln!("WARNING: loading this game DRM DLL executes its initialization code.");
    eprintln!("Run ONLY the verified DLL in an isolated Wine prefix/test environment.");
    let image=unsafe {
        let module=LoadLibraryW(wide.as_ptr());
        if module.is_null() {
            return Err(format!("LoadLibraryW failed: Windows error {}. DLL may require its original game loader.",GetLastError()));
        }
        let bytes=std::slice::from_raw_parts((module as *const u8).add(TXT_RVA),TXT_VSIZE).to_vec();
        let _=FreeLibrary(module);
        bytes
    };
    let changes=old.iter().zip(image.iter()).filter(|(a,b)|a!=b).count();
    println!("Verified DLL SHA-256: {}",DLL_SHA256);
    println!(".txt RVA: 0x{:X}, length {}",TXT_RVA,TXT_VSIZE);
    println!("Disk entropy: {:.4} bits/byte",entropy(old));
    println!("Loaded entropy: {:.4} bits/byte",entropy(&image));
    println!("Changed bytes: {} / {}",changes,TXT_VSIZE);
    if changes==0 {
        println!("Unchanged after LoadLibraryW. DLL may require the full game loader. No dump written.");
        return Ok(());
    }
    let mut f=OpenOptions::new().create_new(true).write(true).open(&output).map_err(|e|e.to_string())?;
    if let Err(e)=f.write_all(&image).and_then(|()|f.sync_all()) {
        drop(f);
        let _=fs::remove_file(&output);
        return Err(format!("Failed to save snapshot: {e}"));
    }
    println!("Memory section written: {}",output.display());
    println!("Snapshot SHA-256: {}",sha256(&image));
    println!("Section-only research data. This is NOT a decrypted game executable.");
    Ok(())
}
fn main(){
    if let Err(e)=run() {eprintln!("ERROR: {e}");std::process::exit(1);}
}
