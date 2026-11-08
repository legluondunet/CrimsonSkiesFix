//! Native Windows-only, read-only telemetry for the game's original ROFFILE.DLL.
//!
//! Hook ONLY the named CreateFileA/W imports of the exact on-disk
//! SHA-256-admitted ROFFILE.DLL, never KERNEL32 exports or another module.
//! The original Win32 call receives identical parameters; return value and
//! GetLastError are preserved. Never log installation directory or full paths.
//!
//! An archive OPEN is not proof which member script the engine selects.
//! This probe is diagnostic only and makes no GOS UI changes.
use std::{
    ffi::{c_char,c_void,OsString},
    fs,os::windows::ffi::OsStringExt,
    path::PathBuf,
    sync::{Mutex,atomic::{AtomicUsize,Ordering}},
};
use crate::{diagnostics,exe_gate,log,win32};

const ROFFILE_HASH:&str="1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a";
const ROFFILE_SIZE:usize=135_168;
const ROFFILE_IMAGE:usize=0x23000;
const MAX_RECORDS:usize=32;
const INVALID_HANDLE_VALUE:usize=usize::MAX;

type CreateA=unsafe extern "system" fn(*const c_char,u32,u32,*mut c_void,u32,u32,*mut c_void)->*mut c_void;
type CreateW=unsafe extern "system" fn(*const u16,u32,u32,*mut c_void,u32,u32,*mut c_void)->*mut c_void;
static ORIGINAL_A:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_W:AtomicUsize=AtomicUsize::new(0);
static EVENTS_A:AtomicUsize=AtomicUsize::new(0);
static EVENTS_W:AtomicUsize=AtomicUsize::new(0);
static TRIED:AtomicUsize=AtomicUsize::new(0);
static PROBE_LOCK:Mutex<()>=Mutex::new(());

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetModuleFileNameW(module:*mut c_void,path:*mut u16,capacity:u32)->u32;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetLastError()->u32;
    fn SetLastError(code:u32);
}

fn le16(bytes:&[u8],offset:usize)->Option<u16>{
    Some(u16::from_le_bytes(bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?))
}
fn le32(bytes:&[u8],offset:usize)->Option<u32>{
    Some(u32::from_le_bytes(bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?))
}
fn nul_ascii(bytes:&[u8],start:usize,max:usize)->Option<&[u8]>{
    let slice=bytes.get(start..start.checked_add(max)?.min(bytes.len()))?;
    let stop=slice.iter().position(|x|*x==0)?;
    Some(&slice[..stop])
}
#[derive(Default,Debug,PartialEq,Eq)]
struct ImportSlots{ansi:Option<usize>,wide:Option<usize>}

/// Parse RVAs as loaded/mapped PE32 image offsets, not file offsets.
/// Every read is length-checked; do not follow arbitrary pointers.
fn find_create_file_imports(image:&[u8])->Option<ImportSlots>{
    if image.len()!=ROFFILE_IMAGE || image.get(..2)!=Some(b"MZ") {return None;}
    let pe=le32(image,0x3c)? as usize;
    if pe<0x40 || pe>0x400 || le32(image,pe)?!=0x4550{return None;}
    let opt=pe.checked_add(24)?;
    if le16(image,opt)?!=0x10b || le32(image,opt+56)? as usize!=ROFFILE_IMAGE {
        return None;
    }
    let imports=le32(image,opt+104)? as usize;
    let import_bytes=le32(image,opt+108)? as usize;
    if imports==0 || import_bytes<20 || imports.checked_add(import_bytes)?>image.len(){
        return None;
    }
    let mut slots=ImportSlots::default();
    for i in 0usize..64 {
        let at=imports.checked_add(i.checked_mul(20)?)?;
        if at.checked_add(20)?>imports+import_bytes{return None;}
        let original=le32(image,at)? as usize;
        let module_rva=le32(image,at+12)? as usize;
        let first=le32(image,at+16)? as usize;
        if original==0 && module_rva==0 && first==0 {break;}
        if original==0 || first==0{continue;} // no named lookup table
        let name=nul_ascii(image,module_rva,32)?;
        if !name.eq_ignore_ascii_case(b"kernel32.dll"){continue;}
        for n in 0usize..512 {
            let rva=original.checked_add(n.checked_mul(4)?)?;
            let thunk=le32(image,rva)? as usize;
            if thunk==0 {break;}
            if thunk&0x8000_0000!=0{continue;}
            let symbol=nul_ascii(image,thunk.checked_add(2)?,64)?;
            let iat=first.checked_add(n.checked_mul(4)?)?;
            if iat.checked_add(4)?>image.len(){return None;}
            if symbol==b"CreateFileA"{slots.ansi=Some(iat);}
            if symbol==b"CreateFileW"{slots.wide=Some(iat);}
        }
    }
    Some(slots)
}

fn archive_basename(name:&[u8])->Option<&'static str>{
    let basename=name.rsplit(|c|*c==b'/' || *c==b'\\').next()?;
    if basename.eq_ignore_ascii_case(b"crimptch.rof"){Some("crimptch.rof")}
    else if basename.eq_ignore_ascii_case(b"crimson.rof"){Some("crimson.rof")}
    else{None}
}
unsafe fn classify_ansi(name:*const c_char)->Option<&'static str>{
    if name.is_null(){return None;}
    let mut buf=[0u8;512];
    for (i,ch) in buf.iter_mut().enumerate(){
        let x=*name.add(i) as u8;
        if x==0{return archive_basename(&buf[..i]);}
        *ch=x;
    }
    None
}
unsafe fn classify_wide(name:*const u16)->Option<&'static str>{
    if name.is_null(){return None;}
    let mut buf=[0u8;512];
    for (i,ch) in buf.iter_mut().enumerate(){
        let x=*name.add(i);
        if x==0{return archive_basename(&buf[..i]);}
        // The relevant archive names contain only ASCII, but a Unicode
        // directory prefix may not. Replace its code units with '?'.
        *ch=if x<=0x7f{x as u8}else{b'?'};
    }
    None
}
unsafe extern "system" fn observe_a(
    name:*const c_char,access:u32,share:u32,security:*mut c_void,
    disposition:u32,flags:u32,template:*mut c_void
)->*mut c_void{
    let raw=ORIGINAL_A.load(Ordering::Acquire);
    if raw==0{return INVALID_HANDLE_VALUE as *mut c_void;}
    let original:CreateA=std::mem::transmute(raw);
    let matched=classify_ansi(name);
    let result=original(name,access,share,security,disposition,flags,template);
    let last=GetLastError();
    if let Some(archive)=matched {
        if EVENTS_A.fetch_add(1,Ordering::Relaxed)<MAX_RECORDS{
            log(&format!("GOS-ROF-OPEN: caller=verified-ROFFILE.DLL api=CreateFileA archive={archive} success={} (pass-through; resource member selection unobserved)",
                result as usize!=INVALID_HANDLE_VALUE));
        }
    }
    SetLastError(last);
    result
}
unsafe extern "system" fn observe_w(
    name:*const u16,access:u32,share:u32,security:*mut c_void,
    disposition:u32,flags:u32,template:*mut c_void
)->*mut c_void{
    let raw=ORIGINAL_W.load(Ordering::Acquire);
    if raw==0{return INVALID_HANDLE_VALUE as *mut c_void;}
    let original:CreateW=std::mem::transmute(raw);
    let matched=classify_wide(name);
    let result=original(name,access,share,security,disposition,flags,template);
    let last=GetLastError();
    if let Some(archive)=matched {
        if EVENTS_W.fetch_add(1,Ordering::Relaxed)<MAX_RECORDS{
            log(&format!("GOS-ROF-OPEN: caller=verified-ROFFILE.DLL api=CreateFileW archive={archive} success={} (pass-through; resource member selection unobserved)",
                result as usize!=INVALID_HANDLE_VALUE));
        }
    }
    SetLastError(last);
    result
}
fn loaded_path(module:*mut c_void)->Option<PathBuf>{
    let mut buffer=[0u16;4096];
    let count=unsafe{GetModuleFileNameW(module,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    if count==0 || count>=buffer.len(){return None;}
    Some(PathBuf::from(OsString::from_wide(&buffer[..count])))
}
fn verify_module(module:*mut c_void)->Result<(),&'static str>{
    let path=loaded_path(module).ok_or("loaded module path unavailable")?;
    let meta=fs::symlink_metadata(&path).map_err(|_|"cannot stat loaded module")?;
    if !meta.is_file() || meta.file_type().is_symlink() ||
       meta.len()!=ROFFILE_SIZE as u64 {return Err("module size/file type mismatch");}
    let bytes=fs::read(&path).map_err(|_|"cannot read loaded module")?;
    if exe_gate::sha256(&bytes)!=ROFFILE_HASH{return Err("module SHA-256 mismatch");}
    Ok(())
}
unsafe fn install_import(
    module:usize, rva:Option<usize>, name:&[u8],
    replacement:usize,original:&AtomicUsize
)->bool {
    let Some(rva)=rva else{return false;};
    let kernel=GetModuleHandleA(b"kernel32.dll\0".as_ptr().cast());
    if kernel.is_null(){return false;}
    let expected=GetProcAddress(kernel,name.as_ptr().cast()) as usize;
    let Some(address)=module.checked_add(rva) else{return false;};
    let slot=address as *mut usize;
    let current=slot.read();
    if expected==0 || current!=expected || current==replacement{return false;}
    if original.compare_exchange(0,current,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(slot,replacement){
        original.store(0,Ordering::Release);
        return false;
    }
    true
}

/// Called by the already existing ROFFILE lifecycle polling point.
/// No filesystem or memory reads when unavailable, disabled or under Wine.
pub fn probe(){
    if !diagnostics::is_native_windows() || !crate::cfg().log_enabled ||
       crate::cfg().directdraw_passthrough{return;}
    let module=unsafe{GetModuleHandleA(b"roffile.dll\0".as_ptr().cast())} as usize;
    if module==0 || TRIED.load(Ordering::Acquire)==module{return;}
    let Ok(_guard)=PROBE_LOCK.try_lock() else{return;};
    if TRIED.load(Ordering::Acquire)==module{return;}
    let status=(||{
        verify_module(module as *mut c_void)?;
        let pe_head=unsafe{std::slice::from_raw_parts(module as *const u8,0x1000)};
        // Exact verified module file/PE; enforce its original image size
        // before constructing an image-wide slice.
        let pe=le32(pe_head,0x3c).ok_or("missing PE offset")? as usize;
        if pe<0x40 || pe>0x400 || le32(pe_head,pe)!=Some(0x4550) ||
            le32(pe_head,pe+24+56)!=Some(ROFFILE_IMAGE as u32){
            return Err("loaded image differs from expected PE");
        }
        let image=unsafe{std::slice::from_raw_parts(module as *const u8,ROFFILE_IMAGE)};
        let imports=find_create_file_imports(image).ok_or("unsupported loaded PE import table")?;
        let a=unsafe{install_import(module,imports.ansi,b"CreateFileA\0",
            observe_a as usize,&ORIGINAL_A)};
        let w=unsafe{install_import(module,imports.wide,b"CreateFileW\0",
            observe_w as usize,&ORIGINAL_W)};
        log(&format!(
            "GOS-ROF-OPEN: verified ROFFILE.DLL import_probe CreateFileA_present={} hook={} CreateFileW_present={} hook={}; observes future opens only, not previously loaded archive members",
            imports.ansi.is_some(),a,imports.wide.is_some(),w
        ));
        Ok::<(),&'static str>(())
    })();
    if let Err(reason)=status{
        log(&format!("GOS-ROF-OPEN: skipped ({reason}); no unverified module modifications"));
    }
    TRIED.store(module,Ordering::Release);
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_exact_archive_basename_matches(){
        assert_eq!(archive_basename(b"C:\\Games\\GOSDATA\\ASSETS\\crimptch.rof"),Some("crimptch.rof"));
        assert_eq!(archive_basename(b"C:/Games/crimson.rof"),Some("crimson.rof"));
        assert_eq!(archive_basename(b"CRIMPTCH.ROF"),Some("crimptch.rof"));
        assert_eq!(archive_basename(b"crimptch.rof.bak"),None);
        assert_eq!(archive_basename(b"other.rof"),None);
    }
    #[test]
    fn rejects_unmapped_or_corrupt_pe(){
        assert_eq!(find_create_file_imports(&[]),None);
        assert_eq!(find_create_file_imports(&vec![0;ROFFILE_IMAGE]),None);
    }
    #[test]
    fn function_signatures_are_win32_x86(){
        let _:CreateA=observe_a;
        let _:CreateW=observe_w;
    }
}
