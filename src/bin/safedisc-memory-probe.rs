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
use std::sync::atomic::{AtomicUsize,Ordering};

const DLL_SIZE: usize = 138_752;
const DLL_SHA256: &str = "dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff";
const TXT_RAW_OFF: usize = 0x400;
const TXT_RVA: usize = 0x1000;
const TXT_VSIZE: usize = 0x7671;
const TXT_RAW_SIZE: usize = 0x7800;

#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn GetCurrentProcess() -> *mut c_void;
    fn ReadProcessMemory(process: *mut c_void, addr: *const c_void,
        destination: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn AddVectoredExceptionHandler(first: u32,
        callback: Option<unsafe extern "system" fn(*mut ExceptionPointers) -> i32>) -> *mut c_void;
    fn RemoveVectoredExceptionHandler(handle: *mut c_void) -> u32;
}

// Read-only exception instrumentation: records the first few observed access
// violations without patching code or changing the exception disposition.
// The Win32 exception record is a 32-bit ABI struct: this helper is x86 ONLY.
#[repr(C)]
struct ExceptionRecord {
    code:u32, flags:u32, inner:*const ExceptionRecord,
    address:*const c_void, parameter_count:u32, information:[usize;15]
}
#[repr(C)]
struct ExceptionPointers {
    record:*const ExceptionRecord,
    context:*const c_void
}
const MAX_EXCEPTIONS:usize=12;
static SEEN:AtomicUsize=AtomicUsize::new(0);
static FAULTS:[AtomicUsize;MAX_EXCEPTIONS]=[const {AtomicUsize::new(0)};MAX_EXCEPTIONS];
static READ_TARGETS:[AtomicUsize;MAX_EXCEPTIONS]=[const {AtomicUsize::new(0)};MAX_EXCEPTIONS];
static SNAP_LO:[AtomicUsize;MAX_EXCEPTIONS]=[const {AtomicUsize::new(0)};MAX_EXCEPTIONS];
static SNAP_HI:[AtomicUsize;MAX_EXCEPTIONS]=[const {AtomicUsize::new(0)};MAX_EXCEPTIONS];
static SNAP_LEN:[AtomicUsize;MAX_EXCEPTIONS]=[const {AtomicUsize::new(0)};MAX_EXCEPTIONS];

unsafe extern "system" fn observe_exception(info:*mut ExceptionPointers)->i32 {
    // EXCEPTION_CONTINUE_SEARCH: observe only. Leave the SEH flow unchanged.
    if info.is_null() || (*info).record.is_null() {return 0;}
    let record=&*(*info).record;
    if record.code!=0xC0000005 {return 0;}
    let index=SEEN.fetch_add(1,Ordering::SeqCst);
    if index>=MAX_EXCEPTIONS {return 0;}
    FAULTS[index].store(record.address as usize,Ordering::SeqCst);
    if record.parameter_count>=2 {
        READ_TARGETS[index].store(record.information[1],Ordering::SeqCst);
    }
    let mut snippet=[0u8;8];
    let mut read=0usize;
    let ok=ReadProcessMemory(GetCurrentProcess(),record.address,
       snippet.as_mut_ptr().cast::<c_void>(),snippet.len(),&mut read);
    if ok!=0 && read==snippet.len() {
        SNAP_LO[index].store(u32::from_le_bytes(snippet[0..4].try_into().unwrap()) as usize,Ordering::SeqCst);
        SNAP_HI[index].store(u32::from_le_bytes(snippet[4..8].try_into().unwrap()) as usize,Ordering::SeqCst);
        SNAP_LEN[index].store(read,Ordering::SeqCst);
    }
    0
}
fn print_exception_trace() {
    let total=SEEN.load(Ordering::SeqCst);
    println!("Vectored observer: {} access violation(s), first {} recorded (no handler altered)",
        total,total.min(MAX_EXCEPTIONS));
    for i in 0..total.min(MAX_EXCEPTIONS) {
        let ip=FAULTS[i].load(Ordering::SeqCst);
        let target=READ_TARGETS[i].load(Ordering::SeqCst);
        let n=SNAP_LEN[i].load(Ordering::SeqCst);
        if n==8 {
            let lo=SNAP_LO[i].load(Ordering::SeqCst) as u32;
            let hi=SNAP_HI[i].load(Ordering::SeqCst) as u32;
            println!("AV #{:02}: EIP=0x{:08X}, memory operand=0x{:08X}, live 8 bytes={:08X}{:08X}",
                i+1,ip,target,lo.swap_bytes(),hi.swap_bytes());
        } else {
            println!("AV #{:02}: EIP=0x{:08X}, memory operand=0x{:08X}, instruction bytes unreadable",
                i+1,ip,target);
        }
    }
    println!("The observed live bytes are diagnostic data, NOT proof of a working decoder.");
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

// The exact DLL has a .reloc directory at RVA 0x27000, raw file offset
// 0x20A00 (size 0xE64); the .txt section contains 214 IMAGE_REL_BASED_HIGHLOW
// entries. Apply them to the raw section to distinguish relocations from
// actual modifications to the protected code.
fn relocation_analysis(disk: &[u8], mapped: &[u8], actual_base: u32) -> Result<(usize,usize),String> {
    const RELOC_RAW:usize=0x20A00;
    const RELOC_LEN:usize=0x0E64;
    const PREFERRED_BASE:u32=0x008E0000;
    if mapped.len()!=TXT_VSIZE {return Err("Unexpected mapped section length".into());}
    let end=RELOC_RAW+RELOC_LEN;
    if disk.get(RELOC_RAW..end).is_none() {return Err("Missing verified relocation table".into());}
    let read_u16=|p:usize| -> Result<u16,String> {
        Ok(u16::from_le_bytes(disk.get(p..p+2).ok_or("Truncated relocation entry")?
            .try_into().map_err(|_|"Bad relocation entry")?))
    };
    let read_u32=|p:usize| -> Result<u32,String> {
        Ok(u32::from_le_bytes(disk.get(p..p+4).ok_or("Truncated relocation header")?
            .try_into().map_err(|_|"Bad relocation header")?))
    };
    let delta=actual_base.wrapping_sub(PREFERRED_BASE);
    let mut expected=disk[TXT_RAW_OFF..TXT_RAW_OFF+TXT_VSIZE].to_vec();
    let mut cursor=RELOC_RAW;
    let mut count=0usize;
    while cursor<end {
        let page=read_u32(cursor)? as usize;
        let size=read_u32(cursor+4)? as usize;
        if size<8 || size%2!=0 || size>end-cursor {
            return Err("Malformed verified PE relocation block".into());
        }
        for entry in (cursor+8..cursor+size).step_by(2) {
            let encoded=read_u16(entry)? as usize;
            let kind=encoded>>12;
            if kind==0 {continue;}
            if kind!=3 {return Err("Unsupported relocation type in verified DLL".into());}
            let rva=page+(encoded&0xFFF);
            if rva>=TXT_RVA && rva<TXT_RVA+TXT_VSIZE {
                let p=rva-TXT_RVA;
                if p+4>expected.len() {return Err("Relocation overruns .txt".into());}
                let old=u32::from_le_bytes(expected[p..p+4].try_into().unwrap());
                expected[p..p+4].copy_from_slice(&old.wrapping_add(delta).to_le_bytes());
                count+=1;
            }
        }
        cursor+=size;
    }
    let unexpected=mapped.iter().zip(expected.iter()).filter(|(a,b)|a!=b).count();
    Ok((count,unexpected))
}

fn run() -> Result<(),String> {
    if cfg!(not(target_pointer_width="32")) {
        return Err("This helper must be built as a Windows x86 (32-bit) program".into());
    }
    let mut args=env::args_os().skip(1);
    let (mut dll,mut output)=(None::<PathBuf>,None::<PathBuf>);
    let mut map_only=false;
    let mut trace_exceptions=false;
    while let Some(arg)=args.next() {
        if arg=="--dll" {dll=Some(args.next().ok_or("Missing --dll path")?.into());}
        else if arg=="--output" {output=Some(args.next().ok_or("Missing --output path")?.into());}
        else if arg=="--map-only" {map_only=true;}
        else if arg=="--trace-exceptions" {trace_exceptions=true;}
        else {return Err(format!("Unknown option: {:?}. Usage: safedisc-memory-probe --dll dplayerx.dll [--map-only | --output dplayerx-txt-memory.bin [--trace-exceptions]]",arg));}
    }
    let dll=dll.ok_or("--dll is required")?;
    if map_only && trace_exceptions {return Err("--trace-exceptions requires normal DLL initialization, not --map-only".into());}
    if map_only && output.is_some() {return Err("--map-only does not create an output; do not pass --output".into());}
    if !map_only && output.is_none() {return Err("--output is required unless --map-only is set".into());}
    let canonical_dll=fs::canonicalize(&dll).map_err(|e|e.to_string())?;
    let meta=fs::symlink_metadata(&canonical_dll).map_err(|e|e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink(){return Err("DLL must be a regular file".into());}
    if let Some(ref output)=output {
        if fs::symlink_metadata(output).is_ok() {return Err("Output exists; refusing to overwrite it".into());}
    }
    let disk=fs::read(&canonical_dll).map_err(|e|e.to_string())?;
    if disk.len()!=DLL_SIZE || sha256(&disk)!=DLL_SHA256 {
        return Err("Unrecognized DPLAYERX.DLL: exact SHA-256 and length do not match verified SafeDisc 1.50.20 build".into());
    }
    if disk.get(TXT_RAW_OFF..TXT_RAW_OFF+TXT_RAW_SIZE).is_none() ||
       disk.get(..2)!=Some(&b"MZ"[..]) {return Err("Unexpected DLL PE image".into());}
    let old=&disk[TXT_RAW_OFF..TXT_RAW_OFF+TXT_VSIZE];
    let mut wide:Vec<u16>=canonical_dll.as_os_str().encode_wide().collect();
    wide.push(0);
    if map_only {
        // DONT_RESOLVE_DLL_REFERENCES: map the PE image *without* resolving imports,
        // executing TLS callbacks or running DllMain. Do NOT call any DLL export.
        const DONT_RESOLVE_DLL_REFERENCES: u32=0x00000001;
        println!("Mapping DLL image WITHOUT executing its initialization code.");
        let (image,mapped_base)=unsafe {
            let module=LoadLibraryExW(wide.as_ptr(),std::ptr::null_mut(),DONT_RESOLVE_DLL_REFERENCES);
            if module.is_null() {
                return Err(format!("LoadLibraryExW (map-only) failed: Windows error {}. Failure occurs even with DLL initialization disabled.",GetLastError()));
            }
            let bytes=std::slice::from_raw_parts((module as *const u8).add(TXT_RVA),TXT_VSIZE).to_vec();
            let base=module as usize as u32;
            let _=FreeLibrary(module);
            (bytes,base)
        };
        let changes=old.iter().zip(image.iter()).filter(|(a,b)|a!=b).count();
        println!("Verified DLL SHA-256: {}",DLL_SHA256);
        println!("Map-only .txt RVA: 0x{:X}; bytes: {}",TXT_RVA,TXT_VSIZE);
        println!("Disk entropy: {:.4} bits/byte",entropy(old));
        println!("Mapped entropy: {:.4} bits/byte",entropy(&image));
        println!("Mapped bytes different from raw disk: {} / {}",changes,TXT_VSIZE);
        let (relocations,unexplained)=relocation_analysis(&disk,&image,mapped_base)?;
        println!("DLL loaded at base address: 0x{:08X}",mapped_base);
        println!(".txt HIGHLOW relocation entries: {}",relocations);
        println!("Bytes remaining different after applying exact PE relocations: {}",unexplained);
        println!("Map-only SUCCESS: PE mapped without running the DLL.");
        if unexplained==0 {
            println!("All mapped differences are fully explained by PE relocations. No .txt decryption observed.");
        } else {
            println!("Some mapped bytes differ beyond PE relocations; more analysis is needed.");
        }
        println!("Normal LoadLibraryW error 998 arose during a later stage (imports or initialization); the specific fault has not been localized.");
        println!("No snapshot written.");
        return Ok(());
    }
    eprintln!("WARNING: loading this game DRM DLL executes its initialization code.");
    eprintln!("Run ONLY the verified DLL in an isolated Wine prefix/test environment.");
    let (image,load_error)=unsafe {
        let observer=if trace_exceptions {
            let handle=AddVectoredExceptionHandler(1,Some(observe_exception));
            if handle.is_null(){return Err(format!("AddVectoredExceptionHandler failed: {}",GetLastError()));}
            handle
        } else {std::ptr::null_mut()};
        let module=LoadLibraryW(wide.as_ptr());
        // Save GetLastError immediately; any subsequent Windows API can reset it.
        let error=if module.is_null(){Some(GetLastError())}else{None};
        let bytes=if !module.is_null(){
            let section=std::slice::from_raw_parts((module as *const u8).add(TXT_RVA),TXT_VSIZE).to_vec();
            let _=FreeLibrary(module);
            Some(section)
        } else {None};
        if !observer.is_null(){let _=RemoveVectoredExceptionHandler(observer);}
        (bytes,error)
    };
    if trace_exceptions {print_exception_trace();}
    if let Some(code)=load_error {
        let hint=if code==998 {" (ERROR_NOACCESS: invalid memory access during load; may involve DLL initialization)"} else {""};
        return Err(format!("LoadLibraryW failed: Windows error {}{}. No memory snapshot written.",code,hint));
    }
    let image=image.ok_or("Loader returned neither DLL image nor failure code")?;
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
    let output=output.ok_or("--output required for full DLL loading")?;
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
