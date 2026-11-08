//! Shared Windows/Wine observer for late ROFFILE.DLL loading.
//!
//! The first DirectDraw call precedes ROFFILE.DLL loading on owner's
//! Windows 11. Polling the module at MainMenu observes only subsequent
//! archive opens, which can hide resource initialization completely.
//! After SHA-256 game-EXE admission, inspect ONLY that EXE's named KERNEL32
//! LoadLibrary[A/W]/LoadLibraryEx[A/W] PE32 imports. If any exist and match
//! the original Win32 API pointer, redirect just the game's IAT slots to
//! transparent wrappers. A successful load triggers the original verified
//! ROFFILE.DLL file-open observer immediately on return from LoadLibrary.
//!
//! No global hooks, loader-lock callbacks, filename logging, script editing,
//! or direct GOS UI changes. Shared archive routing is installed only after
//! the exact loaded module and the managed overlay have been authenticated.

use std::{
    ffi::{c_char,c_void},
    ptr,
    sync::atomic::{AtomicUsize,Ordering},
};
use crate::{log,win32};

const MIN_IMAGE_SIZE:usize=0x0062_0464; // includes the already authenticated EXE IAT
const MAX_IMAGE_SIZE:usize=32*1024*1024;
type LoadA=unsafe extern "system" fn(*const c_char)->*mut c_void;
type LoadW=unsafe extern "system" fn(*const u16)->*mut c_void;
type LoadExA=unsafe extern "system" fn(*const c_char,*mut c_void,u32)->*mut c_void;
type LoadExW=unsafe extern "system" fn(*const u16,*mut c_void,u32)->*mut c_void;
static A:AtomicUsize=AtomicUsize::new(0);
static W:AtomicUsize=AtomicUsize::new(0);
static EXA:AtomicUsize=AtomicUsize::new(0);
static EXW:AtomicUsize=AtomicUsize::new(0);
static NOTICE:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetLastError()->u32;
    fn SetLastError(error:u32);
}

#[derive(Default,Clone,Copy)]
struct Slots{a:Option<usize>,w:Option<usize>,exa:Option<usize>,exw:Option<usize>}
fn read16(image:&[u8],at:usize)->Option<u16>{
    Some(u16::from_le_bytes(image.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn read32(image:&[u8],at:usize)->Option<u32>{
    Some(u32::from_le_bytes(image.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn asciiz(image:&[u8],at:usize,limit:usize)->Option<&[u8]>{
    let bound=at.checked_add(limit)?.min(image.len());
    let candidate=image.get(at..bound)?;
    let len=candidate.iter().position(|byte|*byte==0)?;
    Some(&candidate[..len])
}
fn named_game_load_library_imports(image:&[u8])->Option<Slots>{
    if read16(image,0)!=Some(0x5a4d){return None;}
    let pe=read32(image,0x3c)? as usize;
    if !(0x40..=0x400).contains(&pe) || read32(image,pe)!=Some(0x4550){return None;}
    let optional=pe.checked_add(24)?;
    if read16(image,optional)!=Some(0x10b) || read32(image,optional+56)? as usize!=image.len(){
        return None;
    }
    let start=read32(image,optional+104)? as usize;
    let size=read32(image,optional+108)? as usize;
    if start==0 || size<20 || start.checked_add(size)?>image.len(){return None;}
    let mut out=Slots::default();
    for d in 0usize..64 {
        let entry=start.checked_add(d.checked_mul(20)?)?;
        if entry.checked_add(20)?>start+size{return None;}
        let orig=read32(image,entry)? as usize;
        let name=read32(image,entry+12)? as usize;
        let iat=read32(image,entry+16)? as usize;
        if orig==0 && name==0 && iat==0 {return Some(out);}
        if orig==0 || iat==0 {continue;}
        if !asciiz(image,name,40)?.eq_ignore_ascii_case(b"kernel32.dll"){continue;}
        for n in 0usize..4096 {
            let offset=orig.checked_add(n.checked_mul(4)?)?;
            let value=read32(image,offset)? as usize;
            if value==0{break;}
            if value&0x8000_0000!=0{continue;}
            let symbol=asciiz(image,value.checked_add(2)?,64)?;
            let target=iat.checked_add(n.checked_mul(4)?)?;
            if target.checked_add(4)?>image.len(){return None;}
            match symbol {
                b"LoadLibraryA"=>out.a=Some(target),
                b"LoadLibraryW"=>out.w=Some(target),
                b"LoadLibraryExA"=>out.exa=Some(target),
                b"LoadLibraryExW"=>out.exw=Some(target),
                _=>{}
            }
        }
    }
    Some(out)
}
unsafe fn on_load_return(){
    // Only after the genuine Windows loader returns (outside the loader
    // callback). Never inspect the loader's own incoming filename pointer.
    if !GetModuleHandleA(b"roffile.dll\0".as_ptr().cast()).is_null() {
        if NOTICE.fetch_add(1,Ordering::Relaxed)==0 {
            log("GOS-ROF-LOAD: ROFFILE.DLL is loaded after a game LoadLibrary call; attempting immediate original-ROFFILE IAT observation");
        }
        crate::native_rof_open_trace::probe();
    }
}
unsafe extern "system" fn load_a(name:*const c_char)->*mut c_void{
    let p=A.load(Ordering::Acquire);
    if p==0{return ptr::null_mut();}
    let real:LoadA=std::mem::transmute(p);
    let value=real(name);
    let last=GetLastError();
    if !value.is_null(){on_load_return();}
    SetLastError(last);
    value
}
unsafe extern "system" fn load_w(name:*const u16)->*mut c_void{
    let p=W.load(Ordering::Acquire);
    if p==0{return ptr::null_mut();}
    let real:LoadW=std::mem::transmute(p);
    let value=real(name);
    let last=GetLastError();
    if !value.is_null(){on_load_return();}
    SetLastError(last);
    value
}
unsafe extern "system" fn load_exa(name:*const c_char,file:*mut c_void,flags:u32)->*mut c_void{
    let p=EXA.load(Ordering::Acquire);
    if p==0{return ptr::null_mut();}
    let real:LoadExA=std::mem::transmute(p);
    let value=real(name,file,flags);
    let last=GetLastError();
    if !value.is_null(){on_load_return();}
    SetLastError(last);
    value
}
unsafe extern "system" fn load_exw(name:*const u16,file:*mut c_void,flags:u32)->*mut c_void{
    let p=EXW.load(Ordering::Acquire);
    if p==0{return ptr::null_mut();}
    let real:LoadExW=std::mem::transmute(p);
    let value=real(name,file,flags);
    let last=GetLastError();
    if !value.is_null(){on_load_return();}
    SetLastError(last);
    value
}
unsafe fn patch_named_import(
    base:usize,kernel:*mut c_void,rva:Option<usize>,
    name:&'static [u8],replacement:usize,original:&AtomicUsize
)->bool{
    let Some(rva)=rva else{return false;};
    let expected=GetProcAddress(kernel,name.as_ptr().cast()) as usize;
    let Some(address)=base.checked_add(rva) else{return false;};
    let site=address as *mut usize;
    let previous=site.read();
    if expected==0 || expected!=previous || previous==replacement{return false;}
    if original.compare_exchange(0,previous,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(site,replacement){
        original.store(0,Ordering::Release);
        return false;
    }
    true
}
pub fn install(){
    if !crate::native_rof_read_trace::routing_enabled(){return;}
    unsafe{
        let base=GetModuleHandleW(ptr::null()) as usize;
        if base==0 {log("GOS-ROF-LOAD: no game EXE handle; skipping");return;}
        // The caller checked the exact EXE SHA-256 before reaching install.
        let head=std::slice::from_raw_parts(base as *const u8,0x1000);
        let pe=read32(head,0x3c).unwrap_or(0) as usize;
        if !(0x40..=0x400).contains(&pe) || read32(head,pe)!=Some(0x4550){
            log("GOS-ROF-LOAD: unexpected loaded PE header; skipped");return;
        }
        let len=read32(head,pe+24+56).unwrap_or(0) as usize;
        if !(MIN_IMAGE_SIZE..=MAX_IMAGE_SIZE).contains(&len){
            log("GOS-ROF-LOAD: unexpected loaded game image size; skipped");return;
        }
        let image=std::slice::from_raw_parts(base as *const u8,len);
        let Some(slots)=named_game_load_library_imports(image) else{
            log("GOS-ROF-LOAD: unable to enumerate named game loader imports; skipped");
            return;
        };
        let kernel=GetModuleHandleA(b"kernel32.dll\0".as_ptr().cast());
        if kernel.is_null() {log("GOS-ROF-LOAD: kernel32.dll unavailable; skipped");return;}
        let a=patch_named_import(base,kernel,slots.a,b"LoadLibraryA\0",load_a as usize,&A);
        let w=patch_named_import(base,kernel,slots.w,b"LoadLibraryW\0",load_w as usize,&W);
        let exa=patch_named_import(base,kernel,slots.exa,b"LoadLibraryExA\0",load_exa as usize,&EXA);
        let exw=patch_named_import(base,kernel,slots.exw,b"LoadLibraryExW\0",load_exw as usize,&EXW);
        log(&format!(
            "GOS-ROF-LOAD: shared Windows/Wine game loader observer LoadLibraryA={a} LoadLibraryW={w} LoadLibraryExA={exa} LoadLibraryExW={exw}; original calls forwarded, no global loader changes"
        ));
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rejects_missing_or_malformed_exe_headers(){
        assert!(named_game_load_library_imports(&[]).is_none());
        assert!(named_game_load_library_imports(&[0u8;0x1000]).is_none());
    }
    #[test]
    fn abi_matches_original_win32_signatures(){
        let _:LoadA=load_a;
        let _:LoadW=load_w;
        let _:LoadExA=load_exa;
        let _:LoadExW=load_exw;
    }
}
