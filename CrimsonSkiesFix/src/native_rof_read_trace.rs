//! Read-only Windows provenance probe for original ROFFILE.DLL ReadFile.
//! Enabled only after exact original executable + ROFFILE.DLL verification.
//! It verifies actual Win32 volume/file IDs (not just archive basenames),
//! logs bounded read offsets, and never records private paths or file bytes.
//! Reading the compressed VIDEO member is NOT proof of script execution.
use std::{
    ffi::{c_char,c_void,OsString},fs::{self,File},mem::MaybeUninit,
    os::windows::{ffi::{OsStrExt,OsStringExt},io::AsRawHandle},path::{Path,PathBuf},
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
    fn GetFinalPathNameByHandleW(handle:*mut c_void,path:*mut u16,
        capacity:u32,flags:u32)->u32;
    fn SetFilePointerEx(handle:*mut c_void,distance:i64,new_pointer:*mut i64,move_method:u32)->i32;
    fn CreateFileW(name:*const u16,access:u32,share:u32,security:*mut c_void,
        disposition:u32,flags:u32,template:*mut c_void)->*mut c_void;
    fn CloseHandle(handle:*mut c_void)->i32;
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
/// Classify the exact handle returned by ROFFILE.DLL CreateFileA.
/// A matching basename alone is NOT evidence that this is the managed
/// patch in the installed GOSDATA/ASSETS directory.
pub unsafe fn opened_archive_identity(handle:*mut c_void)->&'static str{
    let Some(refs)=REFERENCES.get().copied().flatten() else{return "references-unavailable";};
    let Some(id)=id_for_handle(handle) else{return "handle-id-unavailable";};
    match archive_for(id,refs){
        Some(Archive::Overlay)=>"installed-crimptch.rof",
        Some(Archive::Base)=>"installed-crimson.rof",
        None=>"different-file-id",
    }
}
/// Diagnostic labels only: do not reveal user directories or arbitrary
/// file contents. The comparison uses the exact physical path returned for
/// the same handle that ROFFILE.DLL opened.
pub struct OpenedArchiveProvenance {
    pub identity:&'static str,
    pub route:&'static str,
    pub content:&'static str,
    pub bytes:Option<u64>,
    pub installed_bytes:Option<u64>,
}
fn final_path_for_handle(handle:*mut c_void)->Option<PathBuf>{
    let mut chars=[0u16;4096];
    let n=unsafe{GetFinalPathNameByHandleW(
        handle,chars.as_mut_ptr(),chars.len() as u32,0
    )} as usize;
    if n==0 || n>=chars.len(){return None;}
    Some(PathBuf::from(OsString::from_wide(&chars[..n])))
}
fn canonical_compare_string(path:&Path)->String{
    let value=path.to_string_lossy().replace('/',"\\").to_lowercase();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}
fn route_of(path:&Path,root:&Path)->&'static str{
    let p=canonical_compare_string(path);
    let r=canonical_compare_string(root);
    if p.starts_with(&(r.clone()+r"\gosdata\assets\")){
        "game-root/GOSDATA/ASSETS"
    }else if p.starts_with(&(r.clone()+r"\assets\")){
        "game-root/ASSETS"
    }else if p.starts_with(&(r.clone()+r"\")){
        "game-root/other"
    }else if p.contains(r"\virtualstore\"){
        "Windows-VirtualStore"
    }else{
        "outside-game-root"
    }
}
fn file_bytes(handle:*mut c_void)->Option<u64>{
    let mut info=MaybeUninit::<FileInfo>::uninit();
    if unsafe{GetFileInformationByHandle(handle,info.as_mut_ptr())}==0{return None;}
    let v=unsafe{info.assume_init()};
    Some(((v.size_hi as u64)<<32)|(v.size_lo as u64))
}
fn classify_content(opened:&Path,installed:&Path)->&'static str{
    const LIMIT:u64=4*1024*1024;
    let Ok(m)=fs::metadata(opened) else{return "unavailable";};
    if !m.is_file() || m.len()>LIMIT{return "unavailable-or-oversized";}
    let Ok(raw)=fs::read(opened) else{return "unavailable";};
    if let Ok(current)=fs::read(installed){
        if raw==current{return "installed-managed-patch";}
    }
    let backup=installed.with_file_name(
        "crimptch.rof.CrimsonSkiesFix.original.bak");
    if let Ok(original)=fs::read(backup){
        if raw==original{return "original-overlay-backup";}
    }
    "other-content"
}
/// Native-only call made immediately after a successful original
/// CreateFileA/W from the SHA-verified ROFFILE.DLL.
pub unsafe fn opened_archive_provenance(
    handle:*mut c_void,archive:&str
)->OpenedArchiveProvenance{
    let identity=opened_archive_identity(handle);
    let game_root=std::env::current_exe().ok()
        .and_then(|exe|exe.parent().map(Path::to_path_buf));
    let opened=final_path_for_handle(handle);
    let route=match (opened.as_ref(),game_root.as_ref()){
        (Some(path),Some(root))=>route_of(path,root),
        _=>"unknown",
    };
    let bytes=file_bytes(handle);
    let installed=game_root.as_ref().map(|root|root.join("GOSDATA")
        .join("ASSETS").join(archive));
    let installed_bytes=installed.as_ref()
        .and_then(|path|fs::metadata(path).ok().map(|m|m.len()));
    let content=if archive=="crimptch.rof"{
        match (opened.as_ref(),installed.as_ref()){
            (Some(opened),Some(installed))=>
                classify_content(opened,installed),
            _=>"unavailable",
        }
    }else{"not-checked"};
    OpenedArchiveProvenance{identity,route,content,bytes,installed_bytes}
}

fn readonly_existing_open(access:u32,disposition:u32)->bool{
    const GENERIC_WRITE:u32=0x4000_0000;
    const GENERIC_ALL:u32=0x1000_0000;
    const DELETE_ACCESS:u32=0x0001_0000;
    const FILE_WRITE_DATA:u32=0x2;
    const FILE_APPEND_DATA:u32=0x4;
    disposition==3 && access&(GENERIC_WRITE|GENERIC_ALL|
        DELETE_ACCESS|FILE_WRITE_DATA|FILE_APPEND_DATA)==0
}
/// Native-only fail-open repair for the confirmed wrong-file archive route.
/// No redirect if the opened file cannot be independently verified as
/// byte-for-byte identical to the preserved original patch overlay.
/// The original ROFFILE.DLL, EXE and original game asset files are untouched.
pub unsafe fn redirect_original_overlay_if_verified(
    original_handle:*mut c_void, access:u32,share:u32,security:*mut c_void,
    disposition:u32,flags:u32,template:*mut c_void
)->Option<*mut c_void>{
    if crate::cfg().gos_video_lock!=gos_video_lock::Mode::On ||
        !readonly_existing_open(access,disposition){return None;}
    if opened_archive_identity(original_handle)!="different-file-id"{return None;}
    let root=std::env::current_exe().ok()
        .and_then(|exe|exe.parent().map(Path::to_path_buf))?;
    let actual_path=final_path_for_handle(original_handle)?;
    if !matches!(route_of(&actual_path,&root),
        "game-root/GOSDATA/ASSETS"|"game-root/ASSETS"|"Windows-VirtualStore"){
        return None;
    }
    let installed=root.join("GOSDATA").join("ASSETS").join("crimptch.rof");
    if gos_video_lock::installed_video_compressed_span().is_none(){return None;}
    if classify_content(&actual_path,&installed)!="original-overlay-backup"{
        return None;
    }
    // Use Unicode for the verified game-root path, preserving every open
    // parameter and never relying on Windows ANSI path conversions.
    let mut wide:Vec<u16>=installed.as_os_str().encode_wide().collect();
    wide.push(0);
    let new_handle=CreateFileW(
        wide.as_ptr(),access,share,security,disposition,flags,template
    );
    if new_handle.is_null() || new_handle as usize==usize::MAX{return None;}
    let Some(references)=REFERENCES.get().copied().flatten() else{
        CloseHandle(new_handle);
        return None;
    };
    if id_for_handle(new_handle)!=Some(references.overlay){
        CloseHandle(new_handle);
        return None;
    }
    CloseHandle(original_handle);
    Some(new_handle)
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
    fn only_sanitized_file_route_labels_are_exposed(){
        let root=Path::new(r"C:\Games\Crimson Skies");
        assert_eq!(route_of(
            Path::new(r"\\?\C:\Games\Crimson Skies\GOSDATA\ASSETS\crimptch.rof"),root
        ),"game-root/GOSDATA/ASSETS");
        assert_eq!(route_of(
            Path::new(r"C:\Games\Crimson Skies\ASSETS\crimptch.rof"),root
        ),"game-root/ASSETS");
        assert_eq!(route_of(
            Path::new(r"C:\Users\Dummy\AppData\Local\VirtualStore\Games\crimptch.rof"),root
        ),"Windows-VirtualStore");
        assert_eq!(route_of(
            Path::new(r"D:\Other\crimptch.rof"),root
        ),"outside-game-root");
    }
    #[test]
    fn repaired_resource_open_must_be_read_only_and_open_existing(){
        assert!(readonly_existing_open(0x8000_0000,3));
        assert!(readonly_existing_open(0x0000_0001,3));
        assert!(!readonly_existing_open(0x4000_0000,3));
        assert!(!readonly_existing_open(0x1000_0000,3));
        assert!(!readonly_existing_open(0x0000_0002,3));
        assert!(!readonly_existing_open(0x8000_0000,2));
    }
    #[test]
    fn readfile_abi_and_win32_file_info_layout(){
        let _:ReadFn=observe_read;
        assert_eq!(std::mem::size_of::<FileInfo>(),52);
    }
}
