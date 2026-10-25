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
    fn DefineDosDeviceW(flags:u32,device:*const u16,target:*const u16)->i32;
    fn QueryDosDeviceW(device:*const u16,target:*mut u16,length:u32)->u32;
    fn CreateFileW(name:*const u16,access:u32,share:u32,security:*mut c_void,
        disposition:u32,attributes:u32,template:*mut c_void)->*mut c_void;
    fn CloseHandle(handle:*mut c_void)->i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn VirtualQuery(addr: *const c_void, info: *mut MemoryBasicInformation, length: usize) -> usize;
    fn ReadProcessMemory(process: *mut c_void, addr: *const c_void,
        destination: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn AddVectoredExceptionHandler(first: u32,
        callback: Option<unsafe extern "system" fn(*mut ExceptionPointers) -> i32>) -> *mut c_void;
    fn RemoveVectoredExceptionHandler(handle: *mut c_void) -> u32;
}

// x86 Windows MEMORY_BASIC_INFORMATION, used only to find the loaded image base.
// VirtualQuery does not execute code in the protected DLL.
#[repr(C)]
struct MemoryBasicInformation {
    base_address:*mut c_void,
    allocation_base:*mut c_void,
    allocation_protect:u32,
    region_size:usize,
    state:u32,
    protect:u32,
    kind:u32,
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
// Sample both documented candidate decoder RVAs once at the first .txt2 AV
// and once at the first .txt AV. If a protection stage transformed the code,
// a changed candidate sample is direct observable evidence.
const CANDIDATES:[usize;2]=[0x65BE,0x6621];
static STAGE_DONE:[AtomicUsize;2]=[const {AtomicUsize::new(0)};2];
static STAGE_BASE:[AtomicUsize;2]=[const {AtomicUsize::new(0)};2];
static CANDIDATE_SAMPLES:[AtomicUsize;16]=[const {AtomicUsize::new(0)};16];
static CANDIDATE_OK:[AtomicUsize;4]=[const {AtomicUsize::new(0)};4];
// Optional private research snapshots. Storage is allocated *before* registering
// the exception observer, avoiding heap allocation in the SEH callback.
static STAGE_DUMP_ADDR:AtomicUsize=AtomicUsize::new(0);
static STAGE_DUMP_OK:[AtomicUsize;2]=[const {AtomicUsize::new(0)};2];

unsafe fn sample_candidate_rvas(fault:*const c_void) {
    let mut mbi=std::mem::MaybeUninit::<MemoryBasicInformation>::uninit();
    if VirtualQuery(fault,mbi.as_mut_ptr(),std::mem::size_of::<MemoryBasicInformation>())==0{return;}
    let info=mbi.assume_init();
    let base=info.allocation_base as usize;
    if base==0 || (fault as usize)<base {return;}
    let rva=(fault as usize)-base;
    // No samples are taken in memory unrelated to this game's DLL.
    let stage=if (0x18000..0x1D497).contains(&rva){0}
       else if (0x1000..0x8671).contains(&rva){1}
       else{return;};
    if STAGE_DONE[stage].swap(1,Ordering::SeqCst)!=0{return;}
    STAGE_BASE[stage].store(base,Ordering::SeqCst);
    for (k,rva) in CANDIDATES.iter().enumerate() {
        let mut snippet=[0u8;16];
        let mut read=0usize;
        let addr=(base+*rva) as *const c_void;
        if ReadProcessMemory(GetCurrentProcess(),addr,snippet.as_mut_ptr().cast::<c_void>(),16,&mut read)==0 || read!=16 {continue;}
        for q in 0..4 {
            let p=(stage*2+k)*4+q;
            CANDIDATE_SAMPLES[p].store(u32::from_le_bytes(snippet[q*4..q*4+4].try_into().unwrap()) as usize,Ordering::SeqCst);
        }
        CANDIDATE_OK[stage*2+k].store(1,Ordering::SeqCst);
    }
    let storage=STAGE_DUMP_ADDR.load(Ordering::SeqCst);
    if storage!=0 {
        let destination=(storage+stage*TXT_VSIZE) as *mut c_void;
        let address=(base+TXT_RVA) as *const c_void;
        let mut read=0usize;
        if ReadProcessMemory(GetCurrentProcess(),address,destination,TXT_VSIZE,&mut read)!=0 && read==TXT_VSIZE {
            STAGE_DUMP_OK[stage].store(1,Ordering::SeqCst);
        }
    }
}


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
    sample_candidate_rvas(record.address);
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
    println!("Candidate modifier instructions sampled at first .txt2 vs first .txt access violation:");
    for stage in 0..2 {
        let label=if stage==0{"first .txt2 AV"}else{"first .txt AV"};
        if STAGE_DONE[stage].load(Ordering::SeqCst)==0 {
            println!("  {}: not observed",label);
            continue;
        }
        println!("  {}: DLL base 0x{:08X}",label,STAGE_BASE[stage].load(Ordering::SeqCst));
        for (k,rva) in CANDIDATES.iter().enumerate() {
            if CANDIDATE_OK[stage*2+k].load(Ordering::SeqCst)==0 {
                println!("    RVA 0x{:04X}: unavailable",rva);
                continue;
            }
            let mut hex=String::new();
            for q in 0..4 {
                let word=CANDIDATE_SAMPLES[(stage*2+k)*4+q].load(Ordering::SeqCst) as u32;
                for b in word.to_le_bytes(){hex.push_str(&format!("{:02X}",b));}
            }
            println!("    RVA 0x{:04X}: {}",rva,hex);
        }
    }
    println!("These candidate bytes describe two runtime stages; changes alone do NOT prove valid decryption.");
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

fn write_private_stage_files(prefix:&PathBuf, snapshots:&[u8], disk:&[u8]) -> Result<(),String> {
    let first_ok=STAGE_DUMP_OK[0].load(Ordering::SeqCst)!=0;
    let second_ok=STAGE_DUMP_OK[1].load(Ordering::SeqCst)!=0;
    println!("Complete .txt stage captures: first .txt2 AV={}, first .txt AV={}",first_ok,second_ok);
    let mut output_paths=Vec::new();
    for stage in 0..2 {
        if STAGE_DUMP_OK[stage].load(Ordering::SeqCst)==0 {continue;}
        let mut name=prefix.as_os_str().to_os_string();
        name.push(if stage==0{"-stage1.bin"}else{"-stage2.bin"});
        let path=PathBuf::from(name);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(format!("Refusing to overwrite an existing stage capture: {}",path.display()));
        }
        output_paths.push((stage,path));
    }
    if first_ok && second_ok {
        let a=&snapshots[..TXT_VSIZE];
        let b=&snapshots[TXT_VSIZE..TXT_VSIZE*2];
        let changes=a.iter().zip(b).filter(|(x,y)|x!=y).count();
        println!("Stage 1 -> stage 2 .txt changed bytes: {} / {}",changes,TXT_VSIZE);
    }
    for (stage,path) in output_paths {
        let snapshot=&snapshots[stage*TXT_VSIZE..(stage+1)*TXT_VSIZE];
        let count=disk.iter().zip(snapshot).filter(|(a,b)|a!=b).count();
        println!("Stage {} .txt entropy: {:.4}, changes vs disk (includes PE relocations): {} / {}",
            stage+1,entropy(snapshot),count,TXT_VSIZE);
        let mut file=OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|e.to_string())?;
        if let Err(e)=file.write_all(snapshot).and_then(|()|file.sync_all()) {
            drop(file);
            let _=fs::remove_file(&path);
            return Err(format!("Stage {} file write failed: {e}",stage+1));
        }
        println!("Private research snapshot: {} ({} bytes; SHA-256 {})",
            path.display(),snapshot.len(),sha256(snapshot));
    }
    if !first_ok && !second_ok {
        println!("No stage snapshot available; no files written.");
    }
    println!("Stage snapshots may contain copyrighted executable code. DO NOT commit or distribute them.");
    Ok(())
}



// Checks a minimal loader call-chain and the on-disk Secdrv device path
// without executing any instruction. Offsets are tied to the exact
// SHA-256-verified dplayerx.dll; not a general PE disassembler.
fn raw_text_rva(dll:&[u8],rva:usize,length:usize)->Result<&[u8],String> {
    if !(0x9000..0x17506).contains(&rva) || length>0x17506-rva {
        return Err(format!("RVA 0x{rva:X} is outside verified DLL .text"));
    }
    let raw=0x7C00+(rva-0x9000);
    dll.get(raw..raw+length).ok_or("Truncated .text code".into())
}
fn static_direct_call_target(dll:&[u8],rva:usize)->Result<usize,String>{
    let bytes=raw_text_rva(dll,rva,5)?;
    if bytes[0]!=0xE8 {return Err(format!("No CALL rel32 at DLL RVA 0x{rva:X}"));}
    let displacement=i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as i64;
    let target=rva as i64+5+displacement;
    usize::try_from(target).map_err(|_|"Negative call target".into())
}
fn report_dll_startup_dependencies(dll:&[u8])->Result<(),String>{
    if static_direct_call_target(dll,0xFD81)?!=0x9770 ||
       static_direct_call_target(dll,0x989F)?!=0x7212 {
        return Err("Unexpected SafeDisc DLL protected startup call chain".into());
    }
    if dll.get(0x1D524..0x1D52B)!=Some(&b"Secdrv\0"[..]) ||
       dll.get(0x1D52C..0x1D533)!=Some(&b"\\\\.\\%s\0"[..]){
        return Err("SafeDisc device-name strings do not match verified DLL".into());
    }
    // The C compiler-obfuscated routine at RVA 0xECC9 begins by pushing
    // the Secdrv string VA and the \\.\%s template VA. The call at
    // RVA 0xED09 goes through an IAT thunk resolved to CreateFileA.
    if raw_text_rva(dll,0xECC9,10)?
        !=&[0x68,0x24,0x09,0x90,0x00,0x68,0x2C,0x09,0x90,0x00] ||
       raw_text_rva(dll,0xED09,6)?!=&[0xFF,0x15,0x5C,0xE0,0x8F,0x00] {
        return Err("Unexpected static Secdrv open sequence".into());
    }
    println!("Static DLL entry: RVA 0xFD20; direct CALL RVA 0xFD81 -> RVA 0x9770 (_DllMain@12 export)");
    println!("Static _DllMain branch: direct CALL RVA 0x989F -> protected .txt RVA 0x7212");
    println!("Static device-code evidence: Secdrv, \\\\.\\%s, and CreateFileA IAT call at RVA 0xED09");
    println!("The DLL has a Secdrv dependency; this does NOT prove which call caused Wine error 998.");
    println!("A standalone LoadLibraryW probe lacks the full SafeDisc loader, driver, and disc context.");
    Ok(())
}


// A strictly opt-in, short-lived DOS-device *alias test*. This is NOT a
// SafeDisc driver or any IOCTL emulation, and does not load dplayerx.dll.
// It only checks whether Wine's Win32 DOS namespace can redirect the name.
struct SecdrvAlias {
    device:Vec<u16>,
    target:Vec<u16>,
}
impl Drop for SecdrvAlias {
    fn drop(&mut self) {
        // Undo ONLY the definition we registered (no pre-existing definition
        // is modified: presence is checked before installing the alias).
        const REMOVE:u32=0x00000002;
        const RAW:u32=0x00000001;
        const QUIET:u32=0x00000008;
        unsafe {
            if DefineDosDeviceW(REMOVE|RAW|QUIET,self.device.as_ptr(),self.target.as_ptr())==0 {
                eprintln!("WARNING: could not remove temporary Secdrv alias (Windows error {})",GetLastError());
            }
        }
    }
}
fn null_terminated(s:&str)->Vec<u16>{s.encode_utf16().chain(std::iter::once(0)).collect()}
fn create_secdrv_alias()->Result<SecdrvAlias,String> {
    const RAW:u32=0x00000001;
    const QUIET:u32=0x00000008;
    let device=null_terminated("Secdrv");
    let target=null_terminated(r"\Device\Null");
    let mut existing=[0u16;2048];
    let n=unsafe{QueryDosDeviceW(device.as_ptr(),existing.as_mut_ptr(),existing.len() as u32)};
    if n!=0 {
        return Err("Secdrv device mapping already exists: refusing to replace or modify it".into());
    }
    let code=unsafe{GetLastError()};
    if code!=2 {
        return Err(format!("Cannot safely verify that the Secdrv device mapping is absent (Windows error {})",code));
    }
    if unsafe{DefineDosDeviceW(RAW|QUIET,device.as_ptr(),target.as_ptr())}==0 {
        return Err(format!("DefineDosDeviceW failed: Windows error {}",unsafe{GetLastError()}));
    }
    println!("Temporary Secdrv -> \\Device\\Null DOS-device alias created.");
    Ok(SecdrvAlias{device,target})
}

fn probe_secdrv_alias_open()->bool {
    const INVALID_HANDLE:usize=usize::MAX;
    let name=null_terminated(r"\\.\Secdrv");
    let handle=unsafe {
        CreateFileW(name.as_ptr(),0xC0000000,0x00000003,std::ptr::null_mut(),
            3,0x80,std::ptr::null_mut())
    };
    if handle as usize==INVALID_HANDLE {
        println!("CreateFileW(\\\\.\\Secdrv) failed with Windows error {}",unsafe{GetLastError()});
        return false;
    }
    let _=unsafe{CloseHandle(handle)};
    println!("CreateFileW(\\\\.\\Secdrv) succeeded with the *null device alias*.");
    true
}
fn test_secdrv_alias()->Result<(),String> {
    let alias=create_secdrv_alias()?;
    let opened=probe_secdrv_alias_open();
    println!("No SafeDisc IOCTL protocol is supported. A null device is NOT a substitute for secdrv.sys.");
    drop(alias);
    println!("Temporary Secdrv alias cleanup requested; no protected DLL code executed.");
    if !opened{return Err("Wine could not open the test alias".into());}
    Ok(())
}

fn read_word(bytes:&[u8],at:usize)->Result<u32,String>{
    let raw=bytes.get(at..at.checked_add(4).ok_or("offset overflow")?).ok_or("truncated dword")?;
    Ok(u32::from_le_bytes(raw.try_into().map_err(|_|"invalid dword")?))
}
/// Verified dplayerx.dll only: .reloc at RVA 0x27000 (raw 0x20A00).
/// Return each .txt-relative HIGHLOW relocation index.
fn txt_relocation_indices(dll:&[u8])->Result<Vec<usize>,String>{
    const OFF:usize=0x20A00;
    const SIZE:usize=0x0E64;
    let end=OFF+SIZE;
    if dll.get(OFF..end).is_none(){return Err("Verified DLL .reloc is missing".into());}
    let mut cursor=OFF;
    let mut indices=Vec::new();
    while cursor<end {
        let page=read_word(dll,cursor)? as usize;
        let size=read_word(dll,cursor+4)? as usize;
        if size<8||size%2!=0||size>end-cursor {
            return Err("Invalid protected DLL relocation block".into());
        }
        for pos in (cursor+8..cursor+size).step_by(2){
            let code=u16::from_le_bytes(
                dll.get(pos..pos+2).ok_or("Truncated relocation pair")?
                    .try_into().map_err(|_|"Invalid relocation pair")?);
            let kind=code>>12;
            if kind==0{continue;}
            if kind!=3{return Err("Unexpected PE relocation type".into());}
            let rva=page+(code as usize&0x0FFF);
            if (TXT_RVA..TXT_RVA+TXT_VSIZE).contains(&rva) {
                let index=rva-TXT_RVA;
                if index+4>TXT_VSIZE{return Err("Relocation beyond .txt virtual size".into());}
                indices.push(index);
            }
        }
        cursor+=size;
    }
    indices.sort_unstable();
    if indices.len()!=214 {
        return Err(format!("Unexpected number of .txt relocations: {}",indices.len()));
    }
    Ok(indices)
}
fn compare_saved_stages(dll:&[u8],one:&PathBuf,two:&PathBuf)->Result<(),String>{
    let s1=fs::read(one).map_err(|e|format!("Cannot read stage 1: {e}"))?;
    let s2=fs::read(two).map_err(|e|format!("Cannot read stage 2: {e}"))?;
    if s1.len()!=TXT_VSIZE || s2.len()!=TXT_VSIZE {
        return Err(format!("Stage files must each have exactly {} bytes",TXT_VSIZE));
    }
    let original=&dll[TXT_RAW_OFF..TXT_RAW_OFF+TXT_VSIZE];
    let relocations=txt_relocation_indices(dll)?;
    let first=relocations[0];
    // The PE base delta is inferred from an actual relocation instead of
    // assuming a particular Wine ASLR allocation base.
    let delta=read_word(&s1,first)?.wrapping_sub(read_word(original,first)?);
    let mut expected=original.to_vec();
    let mut mask=vec![false;TXT_VSIZE];
    for index in relocations.iter().copied(){
        let relocated=read_word(&expected,index)?.wrapping_add(delta);
        expected[index..index+4].copy_from_slice(&relocated.to_le_bytes());
        for flag in &mut mask[index..index+4]{*flag=true;}
    }
    let unexplained=s1.iter().zip(&expected).filter(|(x,y)|x!=y).count();
    println!("SafeDisc 1.50.20 offline .txt stage analysis; no DLL code executed.");
    println!("Stage 1: {} bytes, SHA-256 {}",s1.len(),sha256(&s1));
    println!("Stage 2: {} bytes, SHA-256 {}",s2.len(),sha256(&s2));
    println!("Stage 1 entropy: {:.4} bits/byte",entropy(&s1));
    println!("Stage 2 entropy: {:.4} bits/byte",entropy(&s2));
    println!("PE relocation entries in .txt: {}",relocations.len());
    println!("Inferred DLL relocation delta: 0x{:08X}",delta);
    println!("Stage 1 bytes not explained by original DLL + PE relocations: {}",unexplained);
    if unexplained!=0 {
        return Err("Stage 1 is not an exact mapped copy of the verified original DLL; refusing further interpretation".into());
    }
    let different=s1.iter().zip(&s2).filter(|(a,b)|a!=b).count();
    let outside=(0..TXT_VSIZE).filter(|&i|!mask[i]&&s1[i]!=s2[i]).count();
    let inside=(0..TXT_VSIZE).filter(|&i|mask[i]&&s1[i]!=s2[i]).count();
    println!("Stage 1 -> 2 changed bytes: {} / {} ({:.2}%)",different,TXT_VSIZE,100.0*different as f64/TXT_VSIZE as f64);
    println!("Changes outside relocations: {}; within relocations: {}",outside,inside);
    for rva in CANDIDATES {
        let offset=rva-TXT_RVA;
        let a=&s1[offset..offset+16];
        let b=&s2[offset..offset+16];
        let changed=a.iter().zip(b).filter(|(x,y)|x!=y).count();
        println!("Candidate RVA 0x{:04X}: {} / 16 bytes changed",rva,changed);
    }
    if s2.starts_with(&[0x55,0x8B,0xEC,0x51,0x53,0x56,0x57]) {
        println!("Stage 2 starts with a plausible x86 function prologue, but most code is not yet demonstrated to be valid.");
    }
    report_dll_startup_dependencies(dll)?;
    println!("No unpacking attempted, no output written, no game code distributed.");
    Ok(())
}

fn run() -> Result<(),String> {
    if cfg!(not(target_pointer_width="32")) {
        return Err("This helper must be built as a Windows x86 (32-bit) program".into());
    }
    let mut args=env::args_os().skip(1);
    let (mut dll,mut output)=(None::<PathBuf>,None::<PathBuf>);
    let mut map_only=false;
    let mut trace_exceptions=false;
    let mut capture_stages=None::<PathBuf>;
    let mut analyze_stages=None::<(PathBuf,PathBuf)>;
    let mut test_alias=false;
    let mut load_with_alias=false;
    while let Some(arg)=args.next() {
        if arg=="--dll" {dll=Some(args.next().ok_or("Missing --dll path")?.into());}
        else if arg=="--output" {output=Some(args.next().ok_or("Missing --output path")?.into());}
        else if arg=="--map-only" {map_only=true;}
        else if arg=="--test-secdrv-alias" {test_alias=true;}
        else if arg=="--load-with-secdrv-alias" {load_with_alias=true;}
        else if arg=="--trace-exceptions" {trace_exceptions=true;}
        else if arg=="--capture-stages" {capture_stages=Some(args.next().ok_or("Missing --capture-stages prefix")?.into());}
        else if arg=="--analyze-stages" {
            let one=args.next().ok_or("Missing stage 1 file")?;
            let two=args.next().ok_or("Missing stage 2 file")?;
            analyze_stages=Some((one.into(),two.into()));
        }
        else {return Err(format!("Unknown option: {:?}. Usage: safedisc-memory-probe --dll dplayerx.dll [--test-secdrv-alias | --map-only | --analyze-stages STAGE1 STAGE2 | --output dplayerx-txt-memory.bin [--trace-exceptions [--capture-stages PREFIX]] [--load-with-secdrv-alias]]",arg));}
    }
    let dll=dll.ok_or("--dll is required")?;
    if analyze_stages.is_some() && (map_only || trace_exceptions || capture_stages.is_some() || output.is_some() || test_alias || load_with_alias){
        return Err("--analyze-stages is an offline-only mode and cannot be combined with other modes".into());
    }
    if test_alias && (map_only || trace_exceptions || capture_stages.is_some() || output.is_some() || load_with_alias) {
        return Err("--test-secdrv-alias cannot be combined with other modes".into());
    }
    if load_with_alias && (map_only || !trace_exceptions) {
        return Err("--load-with-secdrv-alias requires --trace-exceptions and cannot be used with --map-only".into());
    }
    if map_only && trace_exceptions {return Err("--trace-exceptions requires normal DLL initialization, not --map-only".into());}
    if capture_stages.is_some() && !trace_exceptions {return Err("--capture-stages requires --trace-exceptions".into());}
    if map_only && output.is_some() {return Err("--map-only does not create an output; do not pass --output".into());}
    if !map_only && !test_alias && analyze_stages.is_none() && output.is_none() {return Err("--output is required unless --map-only, --test-secdrv-alias or --analyze-stages is set".into());}
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
    if let Some((ref one,ref two))=analyze_stages {
        return compare_saved_stages(&disk,one,two);
    }
    if test_alias {
        println!("SafeDisc DOS-device alias availability test (verified DLL checked but never loaded).");
        return test_secdrv_alias();
    }
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
    // Keep stage snapshot buffers at stable addresses while the first-chance
    // callback is active; only read their contents after handler removal.
    let mut snapshots=if capture_stages.is_some(){vec![0u8;2*TXT_VSIZE]}else{Vec::new()};
    if !snapshots.is_empty() {
        STAGE_DUMP_ADDR.store(snapshots.as_mut_ptr() as usize,Ordering::SeqCst);
    }
    let secdrv_alias=if load_with_alias {
        println!("EXPERIMENT: loading the verified DRM DLL with a temporary null-device Secdrv alias.");
        println!("Alias does not implement the SafeDisc protocol or any driver IOCTL responses.");
        let alias=create_secdrv_alias()?;
        if !probe_secdrv_alias_open() {
            drop(alias);
            return Err("Secdrv alias could not be opened; not attempting protected DLL initialization".into());
        }
        Some(alias)
    } else {None};
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
    STAGE_DUMP_ADDR.store(0,Ordering::SeqCst);
    // Always request alias cleanup after LoadLibraryW returns, before reporting
    // its result. A failed load and a successful load follow the same path.
    drop(secdrv_alias);
    if load_with_alias {println!("Temporary Secdrv alias cleanup requested.");}
    if trace_exceptions {print_exception_trace();}
    if let Some(ref prefix)=capture_stages {
        write_private_stage_files(prefix,&snapshots,old)?;
    }
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_reader_is_little_endian_and_bounds_checked() {
        assert_eq!(read_word(&[0x78,0x56,0x34,0x12],0).unwrap(),0x12345678);
        assert!(read_word(&[0,1,2],0).is_err());
        assert!(read_word(&[0,1,2,3],usize::MAX).is_err());
    }
    #[test]
    fn relative_call_target_is_validated() {
        let mut fake=vec![0u8;0x18000];
        fake[0x7C00+(0x989F-0x9000)..0x7C00+(0x989F-0x9000)+5]
            .copy_from_slice(&[0xE8,0x6E,0xD9,0xFF,0xFF]);
        assert_eq!(static_direct_call_target(&fake,0x989F).unwrap(),0x7212);
        assert!(static_direct_call_target(&fake,0x9000).is_err());
    }
    #[test]
    fn entropy_constant_and_binary_distribution() {
        assert_eq!(entropy(&[0u8;64]),0.0);
        assert!((entropy(&[0u8,1u8])-1.0).abs()<1e-12);
    }
}

fn main(){
    if let Err(e)=run() {eprintln!("ERROR: {e}");std::process::exit(1);}
}
