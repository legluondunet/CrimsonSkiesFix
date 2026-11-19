//! PR #51 experimental, opt-in Wine-only CRIMFLAG.MPG menu-video isolation.
//!
//! The game opens CHAP0.MPG successfully, but Wine 11.17 later stalls when
//! the game's IAMMultiMediaStream for CRIMFLAG.MPG seeks to t=0. To avoid
//! altering any original media or game files, intercept only the verified
//! CRIMSON.EXE OLE32!CoCreateInstance IAT slot. After the exact AMStream
//! CLSID/IID is returned, give that one in-process interface a private copy
//! of its original 19-entry COM vtable. No global Wine DLL vtable is patched.
//!
//! "video": do not run the CRIMFLAG stream and skip its zero-position seek.
//! "seek": keep video running, but skip that zero-position seek only.
//! Both are experimental; neither claims a functional/visible MainMenu.
//! Windows native, all non-CRIMFLAG files and all other COM classes/instances
//! are forwarded unchanged. No hook is installed with test=off.
use std::{
    ffi::{c_char,c_void},
    ptr,
    sync::{OnceLock,atomic::{AtomicUsize,Ordering}},
};
use crate::{diagnostics,log,win32};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Mode { Off, Seek, Video }

const IMAGE_MIN:usize=0x0062_0464;
const IMAGE_MAX:usize=32*1024*1024;
const IID_AM_MULTI:[u8;16]=[
    0x5c,0x59,0xbe,0xbe,0x6f,0x9a,0xd0,0x11,
    0x8f,0xde,0x00,0xc0,0x4f,0xd9,0x18,0x9d
];
const CLSID_AM_MULTI:[u8;16]=[
    0xe5,0x7c,0xc4,0x49,0xa4,0x9b,0xd0,0x11,
    0x82,0x12,0x00,0xc0,0x4f,0xc3,0x2c,0x45
];
// IAMMultiMediaStream inherits IMultiMediaStream and has 19 COM entries
// (Wine 11.17 include/amstream.idl, dlls/amstream/multimedia.c).
const VTABLE_LENGTH:usize=19;
const SLOT_SET_STATE:usize=7;
const SLOT_SEEK:usize=10;
const SLOT_OPEN_FILE:usize=16;
const STREAMSTATE_RUN:u32=1;

type Cocreate=unsafe extern "system" fn(*const c_void,*mut c_void,u32,*const c_void,*mut *mut c_void)->i32;
type OpenFile=unsafe extern "system" fn(*mut c_void,*const u16,u32)->i32;
type Seek=unsafe extern "system" fn(*mut c_void,i64)->i32;
type SetState=unsafe extern "system" fn(*mut c_void,u32)->i32;

static CO_CREATE_ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_OPEN_FILE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_SEEK:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_SET_STATE:AtomicUsize=AtomicUsize::new(0);
static OUR_VTABLE:OnceLock<Option<[usize;VTABLE_LENGTH]>>=OnceLock::new();
static TARGET:AtomicUsize=AtomicUsize::new(0);
static HOOK_COUNT:AtomicUsize=AtomicUsize::new(0);
static OPEN_COUNT:AtomicUsize=AtomicUsize::new(0);
static SEEK_COUNT:AtomicUsize=AtomicUsize::new(0);
static RUN_COUNT:AtomicUsize=AtomicUsize::new(0);

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetLastError()->u32;
    fn SetLastError(code:u32);
}

fn u16_at(buf:&[u8],at:usize)->Option<u16>{
    Some(u16::from_le_bytes(buf.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn u32_at(buf:&[u8],at:usize)->Option<u32>{
    Some(u32::from_le_bytes(buf.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn ascii_z<'a>(buf:&'a [u8],at:usize,limit:usize)->Option<&'a [u8]>{
    let end=at.checked_add(limit)?.min(buf.len());
    let slice=buf.get(at..end)?;
    Some(&slice[..slice.iter().position(|b|*b==0)?])
}
fn co_create_import_rva(image:&[u8])->Option<usize>{
    if image.len()<IMAGE_MIN || image.len()>IMAGE_MAX || u16_at(image,0)!=Some(0x5a4d){
        return None;
    }
    let pe=u32_at(image,0x3c)? as usize;
    if !(0x40..=0x400).contains(&pe) || u32_at(image,pe)!=Some(0x4550){return None;}
    let opt=pe.checked_add(24)?;
    if u16_at(image,opt)!=Some(0x10b) || u32_at(image,opt+56)? as usize!=image.len(){
        return None;
    }
    let directory=u32_at(image,opt+104)? as usize;
    let directory_len=u32_at(image,opt+108)? as usize;
    if directory==0 || directory_len<20 || directory.checked_add(directory_len)?>image.len(){
        return None;
    }
    for d in 0usize..64{
        let entry=directory.checked_add(d.checked_mul(20)?)?;
        if entry.checked_add(20)?>directory+directory_len{return None;}
        let names=u32_at(image,entry)? as usize;
        let module=u32_at(image,entry+12)? as usize;
        let iat=u32_at(image,entry+16)? as usize;
        if names==0 && module==0 && iat==0 {break;}
        if names==0 || iat==0{continue;}
        if !ascii_z(image,module,48)?.eq_ignore_ascii_case(b"ole32.dll"){continue;}
        for n in 0usize..4096{
            let index=names.checked_add(n.checked_mul(4)?)?;
            let item=u32_at(image,index)? as usize;
            if item==0{break;}
            if item&0x8000_0000!=0{continue;}
            if ascii_z(image,item.checked_add(2)?,80)?==b"CoCreateInstance"{
                let slot=iat.checked_add(n.checked_mul(4)?)?;
                if slot.checked_add(4)?>image.len(){return None;}
                return Some(slot);
            }
        }
    }
    None
}

fn is_crimflag(path:&[u16])->bool{
    // Both slash forms permitted; never skip a similarly named non-game file.
    // Wine observations: GOSData\assets\graphics\mpg\CrimFlag.MPG.
    let ascii:Option<Vec<u8>>=path.iter().map(|ch|{
        if *ch>127{return None;}
        Some((*ch as u8).to_ascii_lowercase())
    }).collect();
    let Some(bytes)=ascii else{return false;};
    let replaced:Vec<u8>=bytes.into_iter().map(|b|if b==b'/'{b'\\'}else{b}).collect();
    replaced.ends_with(b"gosdata\\assets\\graphics\\mpg\\crimflag.mpg")
}
unsafe fn target_name(name:*const u16)->bool {
    if name.is_null(){return false;}
    let mut path=[0u16;512];
    for i in 0..path.len(){
        let ch=*name.add(i);
        if ch==0{return is_crimflag(&path[..i]);}
        path[i]=ch;
    }
    false
}
fn selected_mode()->Mode{crate::cfg().crimflag_video_test}

unsafe extern "system" fn open_file(this:*mut c_void,name:*const u16,flags:u32)->i32{
    let original=ORIGINAL_OPEN_FILE.load(Ordering::Acquire);
    if original==0{return 0x8000_4005u32 as i32;}
    let f:OpenFile=std::mem::transmute(original);
    let requested=target_name(name);
    let hr=f(this,name,flags);
    // A movie stream must have actually opened before it can be controlled.
    if requested && hr>=0 {
        TARGET.store(this as usize,Ordering::Release);
        let count=OPEN_COUNT.fetch_add(1,Ordering::Relaxed)+1;
        if count<=5{
            log(&format!(
                "PR51-CRIMFLAG: matching OpenFile succeeded count={count} mode={:?}; original HRESULT={:#010x}; no original file changed",
                selected_mode(),hr as u32));
        }
    }
    hr
}
unsafe extern "system" fn seek(this:*mut c_void,time:i64)->i32{
    let mode=selected_mode();
    if mode!=Mode::Off && this as usize==TARGET.load(Ordering::Acquire) && time==0 {
        let count=SEEK_COUNT.fetch_add(1,Ordering::Relaxed)+1;
        if count<=8 || count%120==0 {
            log(&format!("PR51-CRIMFLAG: skipped exactly targeted Seek(0) count={count} mode={mode:?}; reported S_OK"));
        }
        return 0;
    }
    let p=ORIGINAL_SEEK.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let f:Seek=std::mem::transmute(p);
    f(this,time)
}
unsafe extern "system" fn set_state(this:*mut c_void,state:u32)->i32{
    if selected_mode()==Mode::Video &&
        state==STREAMSTATE_RUN &&
        this as usize==TARGET.load(Ordering::Acquire) {
        let count=RUN_COUNT.fetch_add(1,Ordering::Relaxed)+1;
        if count<=8 || count%120==0{
            log(&format!("PR51-CRIMFLAG: suppressed CRIMFLAG.MPG SetState(RUN) count={count}; no video decoder started; reported S_OK"));
        }
        return 0;
    }
    let p=ORIGINAL_SET_STATE.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let f:SetState=std::mem::transmute(p);
    f(this,state)
}
unsafe fn install_on_object(object:*mut c_void)->bool{
    if object.is_null(){return false;}
    let object_vptr=object.cast::<*const usize>();
    let original=object_vptr.read();
    if original.is_null(){return false;}
    let Some(vtable)=OUR_VTABLE.get_or_init(||{
        let mut copy=[0usize;VTABLE_LENGTH];
        for (index,slot) in copy.iter_mut().enumerate(){
            *slot=*original.add(index);
            if *slot==0{return None;}
        }
        if [copy[SLOT_OPEN_FILE],copy[SLOT_SEEK],copy[SLOT_SET_STATE]]
            .contains(&(open_file as usize)) {return None;}
        ORIGINAL_OPEN_FILE.store(copy[SLOT_OPEN_FILE],Ordering::Release);
        ORIGINAL_SEEK.store(copy[SLOT_SEEK],Ordering::Release);
        ORIGINAL_SET_STATE.store(copy[SLOT_SET_STATE],Ordering::Release);
        copy[SLOT_OPEN_FILE]=open_file as usize;
        copy[SLOT_SEEK]=seek as usize;
        copy[SLOT_SET_STATE]=set_state as usize;
        Some(copy)
    }) else{return false;};
    let dest=vtable.as_ptr();
    if original!=dest{
        // Only this concrete IAMMultiMediaStream interface vptr changes.
        object_vptr.cast_mut().write(dest);
    }
    true
}
unsafe extern "system" fn co_create(
    clsid:*const c_void,outer:*mut c_void,context:u32,
    iid:*const c_void,out:*mut *mut c_void
)->i32{
    let p=CO_CREATE_ORIGINAL.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let real:Cocreate=std::mem::transmute(p);
    let hr=real(clsid,outer,context,iid,out);
    let last=GetLastError();
    if hr>=0 && !clsid.is_null() && !iid.is_null() && !out.is_null() &&
        !(*out).is_null() &&
        std::slice::from_raw_parts(clsid.cast::<u8>(),16)==CLSID_AM_MULTI &&
        std::slice::from_raw_parts(iid.cast::<u8>(),16)==IID_AM_MULTI{
        let installed=install_on_object(*out);
        let count=HOOK_COUNT.fetch_add(1,Ordering::Relaxed)+1;
        if count<=12 {
            log(&format!("PR51-CRIMFLAG: CoCreateInstance of IAMMultiMediaStream count={count} vtable_wrapped={installed}; COM identity unchanged"));
        }
    }
    SetLastError(last);
    hr
}
pub fn install(){
    if selected_mode()==Mode::Off || diagnostics::is_native_windows(){return;}
    unsafe{
        let module=GetModuleHandleW(ptr::null()) as usize;
        if module==0{return;}
        // Verified SHA-256 game only, and only after the first DirectDraw
        // export has completed the EXE gate. Read image mapped header first.
        let header=std::slice::from_raw_parts(module as *const u8,0x1000);
        let Some(pe)=u32_at(header,0x3c).map(|v|v as usize) else{return;};
        if !(0x40..=0x400).contains(&pe){return;}
        let Some(size)=u32_at(header,pe+24+56).map(|v|v as usize) else{return;};
        if !(IMAGE_MIN..=IMAGE_MAX).contains(&size){return;}
        let mapped=std::slice::from_raw_parts(module as *const u8,size);
        let Some(rva)=co_create_import_rva(mapped) else{
            log("PR51-CRIMFLAG: ole32!CoCreateInstance not found in verified game imports; experiment not installed");
            return;
        };
        let ole32=GetModuleHandleA(b"ole32.dll\0".as_ptr().cast());
        if ole32.is_null(){return;}
        let expected=GetProcAddress(ole32,b"CoCreateInstance\0".as_ptr().cast()) as usize;
        let Some(addr)=module.checked_add(rva) else{return;};
        let site=addr as *mut usize;
        let actual=site.read();
        if actual==0 || actual!=expected || actual==co_create as usize{
            log("PR51-CRIMFLAG: game OLE32 IAT slot does not match actual CoCreateInstance; experiment skipped");
            return;
        }
        CO_CREATE_ORIGINAL.store(actual,Ordering::Release);
        if !win32::patch_vtable_slot(site,co_create as usize){
            CO_CREATE_ORIGINAL.store(0,Ordering::Release);
            log("PR51-CRIMFLAG: unable to install game-only CoCreateInstance observer");
            return;
        }
        log(&format!("PR51-CRIMFLAG: isolated game-only OLE32 IAT hook enabled mode={:?}, Wine-only; no media assets edited",selected_mode()));
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn target_is_exact_game_menu_mpeg(){
        assert!(is_crimflag(&"GOSData\\assets\\graphics\\mpg\\CrimFlag.MPG".encode_utf16().collect::<Vec<_>>()));
        assert!(is_crimflag(&"C:/Games/GOSData/assets/graphics/mpg/CRIMFLAG.MPG".encode_utf16().collect::<Vec<_>>()));
        for other in ["CHAP0.MPG","MSOPEN1.MPG","CRIMFLAG.MPG.old","CRIMFLAG.MPG\\x","Other\\CRIMFLAG.MPG"]{
            assert!(!is_crimflag(&other.encode_utf16().collect::<Vec<_>>()));
        }
    }
    #[test]
    fn expected_com_method_slots_are_stable(){
        assert_eq!((SLOT_SET_STATE,SLOT_SEEK,SLOT_OPEN_FILE,VTABLE_LENGTH),(7,10,16,19));
        assert_eq!(std::mem::size_of::<Cocreate>(),std::mem::size_of::<usize>());
    }
}
