use std::{ffi::{c_char, c_void, OsString}, fs, os::windows::ffi::{OsStrExt, OsStringExt}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub type Hwnd = *mut c_void;
pub const PAGE_READWRITE: u32 = 0x04;
pub const SWP_FRAMECHANGED: u32 = 0x0020;
pub const SWP_SHOWWINDOW: u32 = 0x0040;
pub const SWP_NOOWNERZORDER: u32 = 0x0200;
pub const GWL_STYLE: i32 = -16;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Point { pub x:i32, pub y:i32 }
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Rect { pub left:i32, pub top:i32, pub right:i32, pub bottom:i32 }
#[repr(C)]
#[derive(Default)]
pub struct MonitorInfo { pub size:u32, pub monitor:Rect, pub work:Rect, pub flags:u32 }

#[link(name = "kernel32")]
extern "system" {
    pub fn GetSystemDirectoryW(buffer:*mut u16,len:u32)->u32;
    pub fn LoadLibraryW(path:*const u16)->*mut c_void;
    pub fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    pub fn GetModuleFileNameW(module:*mut c_void,buffer:*mut u16,len:u32)->u32;
    pub fn VirtualProtect(address:*mut c_void,size:usize,new_protect:u32,old_protect:*mut u32)->i32;
    pub fn QueryPerformanceCounter(value:*mut i64)->i32;
    pub fn QueryPerformanceFrequency(value:*mut i64)->i32;
    pub fn Sleep(ms:u32);
    fn MoveFileExW(from:*const u16,to:*const u16,flags:u32)->i32;
    fn GetLastError()->u32;
    fn SetLastError(code:u32);
    fn GetCurrentProcessId()->u32;
}
#[link(name = "user32")]
extern "system" {
    fn GetCursorPos(out:*mut Point)->i32;
    fn MonitorFromPoint(point:Point,flags:u32)->*mut c_void;
    fn MonitorFromWindow(hwnd:Hwnd,flags:u32)->*mut c_void;
    fn GetMonitorInfoW(monitor:*mut c_void,info:*mut MonitorInfo)->i32;
    fn GetWindowLongW(hwnd:Hwnd,index:i32)->i32;
    fn SetWindowLongW(hwnd:Hwnd,index:i32,new:i32)->i32;
    fn SetWindowPos(hwnd:Hwnd,insert_after:Hwnd,x:i32,y:i32,width:i32,height:i32,flags:u32)->i32;
    fn IsWindow(hwnd:Hwnd)->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,process_id:*mut u32)->u32;
}

// Equivalent to SpiderMan2k1Fix's organized-install path policy:
// read the executable directory, not the process's mutable working directory.
// No directory creation, filesystem operation or migration takes place in DllMain.
fn exe_directory()->Option<PathBuf> {
    let mut buffer=[0u16;32768];
    let n=unsafe {GetModuleFileNameW(std::ptr::null_mut(),buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    if n==0 || n>=buffer.len(){return None;}
    let exe=PathBuf::from(OsString::from_wide(&buffer[..n]));
    Some(exe.parent()?.to_path_buf())
}

pub fn project_path(name:&str)->Option<PathBuf> {
    // Fixed internal file names only; never insert user-controlled paths here.
    Some(exe_directory()?.join("CrimsonFix").join(name))
}

fn regular_file(path:&Path)->bool {
    fs::symlink_metadata(path).map(|info|info.is_file() && !info.file_type().is_symlink())
        .unwrap_or(false)
}

fn safe_directory(dir:&Path,create:bool)->bool {
    if create && !dir.exists() && fs::create_dir(dir).is_err(){return false;}
    fs::symlink_metadata(dir).map(|i|i.is_dir() && !i.file_type().is_symlink())
        .unwrap_or(false)
}

pub fn ini_path()->Option<PathBuf> {
    let modern=project_path("CrimsonFix.ini")?;
    if regular_file(&modern){return Some(modern);}
    // If a file of an unsupported type exists, do not touch it.
    if fs::symlink_metadata(&modern).is_ok(){return None;}

    let legacy=exe_directory()?.join("CrimsonFix.ini");
    if !regular_file(&legacy){return Some(modern);}
    let directory=modern.parent()?;
    if !safe_directory(directory,true){return Some(modern);}

    // Exactly-once, non-overwriting migration from the old v0.1 layout.
    // Always keep the legacy root INI intact.
    let ticks=SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let temp=directory.join(format!(".CrimsonFix-migrate-{}-{ticks}.tmp",std::process::id()));
    if fs::copy(&legacy,&temp).is_ok(){
        let temp_w:Vec<u16>=temp.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let modern_w:Vec<u16>=modern.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        unsafe {MoveFileExW(temp_w.as_ptr(),modern_w.as_ptr(),0x8 /* MOVEFILE_WRITE_THROUGH */);}
        let _=fs::remove_file(temp);
    }
    Some(modern)
}

pub fn log_path()->Option<PathBuf> {
    let path=project_path("CrimsonFix.log")?;
    let directory=path.parent()?;
    if !safe_directory(directory,true){return None;}
    if fs::symlink_metadata(&path).is_ok() && !regular_file(&path){return None;}
    Some(path)
}

pub unsafe fn real_ddraw()->Option<*mut c_void> {
    let mut path=[0u16;32768];
    let n=GetSystemDirectoryW(path.as_mut_ptr(),path.len() as u32) as usize;
    if n==0 || n+11>=path.len(){return None;}
    let suffix:Vec<u16>="\\ddraw.dll".encode_utf16().chain(std::iter::once(0)).collect();
    path[n..n+suffix.len()].copy_from_slice(&suffix);
    let handle=LoadLibraryW(path.as_ptr());
    if handle.is_null(){None}else{Some(handle)}
}

pub unsafe fn system_export(handle:*mut c_void,name:&[u8])->*const c_void {
    GetProcAddress(handle,name.as_ptr().cast())
}

pub fn monitor_from_cursor_or_window(hwnd:Hwnd)->Option<Rect> {
    unsafe {
        let mut cursor=Point::default();
        let handle=if GetCursorPos(&mut cursor)!=0 {
            MonitorFromPoint(cursor,2 /* MONITOR_DEFAULTTONEAREST */)
        } else {
            MonitorFromWindow(hwnd,2)
        };
        if handle.is_null(){return None;}
        let mut info=MonitorInfo::default();
        info.size=std::mem::size_of::<MonitorInfo>() as u32;
        if GetMonitorInfoW(handle,&mut info)==0{return None;}
        Some(info.monitor)
    }
}

// The early Crimson Skies SetCooperativeLevel calls may use a NULL HWND.
// Report *why* we cannot adjust the window; never manipulate another
// process's HWND, and never invent a guessed main window at startup.
pub fn borderless_window(hwnd:Hwnd,area:Rect)->Result<(),String> {
    if hwnd.is_null() {return Err("deferred: NULL HWND supplied by game".into());}
    unsafe {
        if IsWindow(hwnd)==0 {return Err("deferred: HWND is not a window".into());}
        let mut owner=0u32;
        if GetWindowThreadProcessId(hwnd,&mut owner)==0 || owner!=GetCurrentProcessId() {
            return Err("rejected: HWND does not belong to this game process".into());
        }
        let w=area.right.saturating_sub(area.left);
        let h=area.bottom.saturating_sub(area.top);
        if w<=0 || h<=0 {return Err("rejected: monitor bounds are empty".into());}
        // GetWindowLong may legally return zero; check GetLastError.
        SetLastError(0);
        let existing=GetWindowLongW(hwnd,GWL_STYLE);
        let error=GetLastError();
        if existing==0 && error!=0 {return Err(format!("GetWindowLongW Win32 error={error}"));}
        // Preserve unrelated window styles and remove caption, frame, system menu.
        let borders=0x00CF_0000u32;
        let new_style=((existing as u32 & !borders)|WS_POPUP|WS_VISIBLE) as i32;
        SetLastError(0);
        let previous=SetWindowLongW(hwnd,GWL_STYLE,new_style);
        let error=GetLastError();
        if previous==0 && error!=0 {return Err(format!("SetWindowLongW Win32 error={error}"));}
        if SetWindowPos(hwnd,std::ptr::null_mut(),area.left,area.top,w,h,
                        SWP_FRAMECHANGED|SWP_SHOWWINDOW|SWP_NOOWNERZORDER)==0 {
            let error=GetLastError();
            return Err(format!("SetWindowPos Win32 error={error}"));
        }
        Ok(())
    }
}

pub unsafe fn patch_vtable_slot(slot:*mut usize,new:usize)->bool {
    if slot.is_null(){return false;}
    let mut previous=0;
    if VirtualProtect(slot.cast(),std::mem::size_of::<usize>(),PAGE_READWRITE,&mut previous)==0 {
        return false;
    }
    slot.write(new);
    let mut discarded=0;
    VirtualProtect(slot.cast(),std::mem::size_of::<usize>(),previous,&mut discarded);
    true
}
