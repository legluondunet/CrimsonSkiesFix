//! Read-only Windows provenance probe for original ROFFILE.DLL ReadFile.
//! Enabled only after exact original executable + ROFFILE.DLL verification.
//! It verifies actual Win32 volume/file IDs (not just archive basenames),
//! logs bounded read offsets, and never records private paths or file bytes.
//! Reading the compressed VIDEO member is NOT proof of script execution.
use std::{
    ffi::{c_char,c_void},fs::File,mem::MaybeUninit,
    os::windows::io::AsRawHandle,path::Path,
    sync::{OnceLock,atomic::{AtomicUsize,Ordering}},
};
use crate::{gos_video_lock,log,win32};
type ReadFn=unsafe extern "system" fn(*mut c_void,*mut c_void,u32,*mut u32,*mut c_void)->i32;
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static PATCH_READS:AtomicUsize=AtomicUsize::new(0);
static BASE_READS:AtomicUsize=AtomicUsize::new(0);
static OVERLAPS:AtomicUsize=AtomicUsize::new(0);
static REFERENCES:OnceLock<Option<References>>=OnceLock::new();

#[repr(C)]
#[derive(Clone,Copy)]
struct FileTime{low:u32,high:u32}
#[repr(C)]
#[derive(Clone,Copy)]
struct FileInfo{
    attrs:u32,created:FileTime,accessed:FileTime,written:FileTime,
    volume_serial:u32,size_hi:u32,size_lo:u32,links:u32,index_hi:u32,index_lo:u32,
}
#[derive(Clone,Copy,PartialEq,Eq)]
struct FileId{volume:u32,index_hi:u32,index_lo:u32}
#[derive(Clone,Copy)]
struct References{overlay:FileId,base:FileId,video_span:Option<(u64,u64)>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Archive{Overlay,Base}
#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetFileInformationByHandle(handle:*mut c_void,info:*mut FileInfo)->i32;
    fn SetFilePointerEx(handle:*mut c_void,distance:i64,new_pointer:*mut i64,move_method:u32)->i32;
    fn GetLastError()->u32;
    fn SetLastError(error:u32);
}
unsafe fn id_for_handle(handle:*mut c_void)->Option<FileId>{
    if handle.is_null() || handle as usize==usize::MAX{return None;}
    let mut info=MaybeUninit::<FileInfo>::uninit();
    if GetFileInformationByHandle(handle,info.as_mut_ptr())==0{return None;}
    let info=info.assume_init();
    Some(FileId{volume:info.volume_serial,index_hi:info.index_hi,index_lo:info.index_lo})
}
fn id_for_path(path:&Path)->Option<FileId>{
    let file=File::open(path).ok()?;
    unsafe{id_for_handle(file.as_raw_handle())}
}
fn reference_files()->Option<References>{
    let exe=std::env::current_exe().ok()?;
    let folder=exe.parent()?.join("GOSDATA").join("ASSETS");
    let overlay=id_for_path(&folder.join("crimptch.rof"))?;
    let base=id_for_path(&folder.join("crimson.rof"))?;
    let video_span=gos_video_lock::installed_video_compressed_span();
    Some(References{overlay,base,video_span})
}
fn archive_for(id:FileId,refs:References)->Option<Archive>{
    if id==refs.overlay{Some(Archive::Overlay)}
    else if id==refs.base{Some(Archive::Base)}
    else{None}
}
fn overlaps_read(start:u64,count:u32,span:(u64,u64))->bool{
    count>0 && start<span.1 && start.saturating_add(count as u64)>span.0
}
unsafe extern "system" fn observe_read(
    handle:*mut c_void,buffer:*mut c_void,length:u32,
    bytes_read:*mut u32,overlapped:*mut c_void
)->i32{
    let ptr=ORIGINAL.load(Ordering::Acquire);
    if ptr==0{return 0;}
    let real:ReadFn=std::mem::transmute(ptr);
    let incoming_error=GetLastError();
    let refs=REFERENCES.get().copied().flatten();
    let archive=refs.and_then(|r|id_for_handle(handle).and_then(|id|archive_for(id,r)));
    let mut start:i64=0;
    // FILE_CURRENT with a 0 displacement is an observation, not a seek.
    // Never use it with OVERLAPPED I/O.
    let has_start=archive.is_some() && overlapped.is_null() &&
        SetFilePointerEx(handle,0,&mut start,1)!=0 && start>=0;
    SetLastError(incoming_error);
    let result=real(handle,buffer,length,bytes_read,overlapped);
    let original_error=GetLastError();
    if let Some(archive)=archive{
        let count=match archive {
            Archive::Overlay=>PATCH_READS.fetch_add(1,Ordering::Relaxed),
            Archive::Base=>BASE_READS.fetch_add(1,Ordering::Relaxed),
        };
        let got=if result!=0 && !bytes_read.is_null(){Some(*bytes_read)}else{None};
        let span=refs.and_then(|r|r.video_span);
        let overlap=archive==Archive::Overlay && has_start &&
            got.is_some_and(|n|span.is_some_and(|range|
                overlaps_read(start as u64,n,range)));
        let extra=overlap && OVERLAPS.fetch_add(1,Ordering::Relaxed)<8;
        let limit=if archive==Archive::Overlay{32}else{12};
        if count<limit || extra{
            let name=if archive==Archive::Overlay{"crimptch.rof"}else{"crimson.rof"};
            let offset=if has_start{Some(start as u64)}else{None};
            log(&format!(
                "GOS-ROF-READ: verified_file={name} call={} requested={length} actual={got:?} offset={offset:?} video_compressed_span_overlap={overlap} success={} [pass-through; member execution unobserved]",
                count+1,result!=0
            ));
        }
    }
    SetLastError(original_error);
    result
}
/// The caller has already verified the original SHA256+PE of ROFFILE.DLL
/// and identified this named KERNEL32 ReadFile import IAT RVA.
pub unsafe fn install(module:usize,rva:Option<usize>)->bool{
    let Some(rva)=rva else{return false;};
    let Some(slot)=module.checked_add(rva) else{return false;};
    let kernel=GetModuleHandleA(b"kernel32.dll\0".as_ptr().cast());
    if kernel.is_null(){return false;}
    let expected=GetProcAddress(kernel,b"ReadFile\0".as_ptr().cast()) as usize;
    let previous=(slot as *mut usize).read();
    if expected==0 || previous!=expected || previous==observe_read as usize{return false;}
    let references=REFERENCES.get_or_init(reference_files);
    let Some(refs)=*references else {
        log("GOS-ROF-READ: expected installed archives not file-ID verified; skipped");
        return false;
    };
    log(&format!(
        "GOS-ROF-READ: file ID references ready, patched_video_span_known={} [read-only; paths hidden]",
        refs.video_span.is_some()
    ));
    if ORIGINAL.compare_exchange(0,previous,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(slot as *mut usize,observe_read as usize){
        ORIGINAL.store(0,Ordering::Release);
        return false;
    }
    true
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_verified_file_identity_matches(){
        let a=FileId{volume:1,index_hi:2,index_lo:3};
        let b=FileId{volume:1,index_hi:2,index_lo:4};
        let foreign=FileId{volume:2,index_hi:2,index_lo:3};
        let r=References{overlay:a,base:b,video_span:Some((100,200))};
        assert_eq!(archive_for(a,r),Some(Archive::Overlay));
        assert_eq!(archive_for(b,r),Some(Archive::Base));
        assert_eq!(archive_for(foreign,r),None);
        assert!(overlaps_read(99,2,(100,200)));
        assert!(overlaps_read(199,2,(100,200)));
        assert!(!overlaps_read(200,1,(100,200)));
        assert!(!overlaps_read(0,0,(100,200)));
    }
    #[test]
    fn readfile_abi_and_win32_file_info_layout(){
        let _:ReadFn=observe_read;
        assert_eq!(std::mem::size_of::<FileInfo>(),52);
    }
}
