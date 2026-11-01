use std::{ffi::{c_char, c_void, OsString}, fs, io::Write, os::windows::ffi::{OsStrExt, OsStringExt}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use crate::config::DEFAULT_INI;

pub type Hwnd = *mut c_void;
pub const PAGE_READWRITE: u32 = 0x04;
pub const SWP_FRAMECHANGED: u32 = 0x0020;
pub const SWP_SHOWWINDOW: u32 = 0x0040;
pub const SWP_NOOWNERZORDER: u32 = 0x0200;
pub const GWL_STYLE: i32 = -16;
pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;

#[link(name="user32")]
extern "system" {
    fn GetSystemMetrics(index:i32)->i32;
}

// No HWND exists when the game first calls DirectDraw. A one-monitor
// installation has a deterministic primary monitor; on multi-monitor
// systems do NOT guess based on the mouse, and defer to the game HWND.
fn sole_monitor_metrics(count:i32,width:i32,height:i32)->Option<(u32,u32)> {
    if count!=1 || width<=0 || height<=0{return None;}
    Some((u32::try_from(width).ok()?,u32::try_from(height).ok()?))
}
pub fn sole_primary_monitor_dimensions()->Option<(u32,u32)>{
    unsafe {
        sole_monitor_metrics(
            GetSystemMetrics(80), // SM_CMONITORS
            GetSystemMetrics(0),  // SM_CXSCREEN
            GetSystemMetrics(1),  // SM_CYSCREEN
        )
    }
}

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
    pub fn GetCurrentThreadId()->u32;
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
    fn EnumWindows(callback:unsafe extern "system" fn(Hwnd,isize)->i32,context:isize)->i32;
    fn IsWindowVisible(hwnd:Hwnd)->i32;
    fn GetWindow(hwnd:Hwnd,command:u32)->Hwnd;
    fn GetWindowRect(hwnd:Hwnd,rect:*mut Rect)->i32;
    fn GetClientRect(hwnd:Hwnd,rect:*mut Rect)->i32;
    fn ClientToScreen(hwnd:Hwnd,point:*mut Point)->i32;
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
    Some(exe_directory()?.join("CrimsonSkiesFix").join(name))
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

// `create_new` is atomic: no update archive or game startup can truncate
// the player's configuration. Defaults are materialized only on a new install.
fn create_ini_once(path:&Path)->std::io::Result<bool> {
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file)=>{
            file.write_all(DEFAULT_INI.as_bytes())?;
            file.sync_all()?;
            Ok(true)
        },
        Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e)=>Err(e),
    }
}

// Preserve either previous configuration. Prefer the last active layout.
fn previous_ini(exe:&Path)->Option<PathBuf> {
    [exe.join("CrimsonFix").join("CrimsonFix.ini"),
     exe.join("CrimsonFix.ini")]
        .into_iter().find(|p|regular_file(p))
}

pub fn ini_path()->Option<PathBuf> {
    let modern=project_path("CrimsonSkiesFix.ini")?;
    if regular_file(&modern){return Some(modern);}
    // Never replace a symlink, directory, or unexpected filesystem object.
    if fs::symlink_metadata(&modern).is_ok(){return None;}
    let legacy=previous_ini(&exe_directory()?);
    let dir=modern.parent()?;
    if !safe_directory(dir,true){return legacy;}
    if let Some(ref old)=legacy {
        let ticks=SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_nanos();
        let tmp=dir.join(format!(".CrimsonSkiesFix-migrate-{}-{ticks}.tmp",std::process::id()));
        if fs::copy(old,&tmp).is_ok(){
            let src:Vec<u16>=tmp.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            let dst:Vec<u16>=modern.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            // No MOVEFILE_REPLACE_EXISTING; keep all originals intact.
            unsafe{MoveFileExW(src.as_ptr(),dst.as_ptr(),0x8 /* WRITE_THROUGH */);}
        }
        let _=fs::remove_file(tmp);
        return if regular_file(&modern){Some(modern)}else{Some(old.to_path_buf())};
    }
    let _=create_ini_once(&modern);
    regular_file(&modern).then_some(modern)
}

// Obtain the actual loaded DLL directory rather than assuming that the
// process working directory or executable directory always matches.
fn dll_directory()->Option<PathBuf> {
    let handle=crate::MODULE.load(std::sync::atomic::Ordering::Relaxed) as *mut c_void;
    if handle.is_null(){return None;}
    let mut buffer=[0u16;32768];
    let n=unsafe{GetModuleFileNameW(handle,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    if n==0 || n>=buffer.len(){return None;}
    Some(PathBuf::from(OsString::from_wide(&buffer[..n])).parent()?.to_path_buf())
}

pub fn log_path()->Option<PathBuf> {
    // Same directory as ddraw.dll. Do not modify logs from earlier releases.
    let path=dll_directory()?.join("CrimsonSkiesFix.log");
    if fs::symlink_metadata(&path).is_ok() && !regular_file(&path){return None;}
    Some(path)
}

pub fn crash_blt_trace_path()->Option<PathBuf> {
    // Separate, bounded rolling trace, always beside the loaded proxy DLL.
    // Do not overwrite or follow arbitrary preexisting symlinks.
    let path=dll_directory()?.join("CrimsonSkiesFix-BltTrace.log");
    if fs::symlink_metadata(&path).is_ok() && !regular_file(&path){return None;}
    Some(path)
}

/// Strictly allowlisted capture names, next to the actual loaded proxy.
pub fn color_capture_path(label:&str)->Option<PathBuf>{
    if !matches!(label,"Copy1-Source32"|"Copy3-Source32"|"Copy3-Dest565"){
        return None;
    }
    let path=dll_directory()?.join(format!(
        "CrimsonSkiesFix-Color-{}-{label}.bmp",unsafe{GetCurrentProcessId()}));
    if fs::symlink_metadata(&path).is_ok(){return None;}
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

// Unlike the legacy cursor-based screen choice, the presentation
// preflight always targets the monitor containing the game's own HWND.
pub fn monitor_for_game_window(hwnd:Hwnd)->Option<Rect> {
    if hwnd.is_null(){return None;}
    unsafe {
        if IsWindow(hwnd)==0{return None;}
        let mut owner=0u32;
        if GetWindowThreadProcessId(hwnd,&mut owner)==0 || owner!=GetCurrentProcessId(){
            return None;
        }
        let monitor=MonitorFromWindow(hwnd,2 /* MONITOR_DEFAULTTONEAREST */);
        if monitor.is_null(){return None;}
        let mut info=MonitorInfo::default();
        info.size=std::mem::size_of::<MonitorInfo>() as u32;
        (GetMonitorInfoW(monitor,&mut info)!=0).then_some(info.monitor)
    }
}

/// Verified game-owned HWND client rectangle in screen coordinates.
/// Used by the optional adapter only AFTER setting WS_POPUP, so native
/// surface Blts never assume a width or origin that the host WM refused.
pub fn game_client_geometry(hwnd:Hwnd)->Option<(i32,i32,u32,u32)> {
    let _monitor=monitor_for_game_window(hwnd)?;
    unsafe {
        let mut client=Rect::default();
        let mut origin=Point::default();
        if GetClientRect(hwnd,&mut client)==0 || ClientToScreen(hwnd,&mut origin)==0 {
            return None;
        }
        let width=u32::try_from(client.right.checked_sub(client.left)?).ok()?;
        let height=u32::try_from(client.bottom.checked_sub(client.top)?).ok()?;
        (width>0 && height>0).then_some((origin.x,origin.y,width,height))
    }
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


#[derive(Default)]
struct GameWindowSnapshot {
    all:u32,
    visible:u32,
    top_level_visible:u32,
    geometry:Vec<String>,
}

// Diagnostic-only; never expose window titles, HWND values, raw class names
// or unrelated process windows. Bounded enumeration at DirectDraw setup time.
unsafe extern "system" fn game_window_callback(hwnd:Hwnd,context:isize)->i32 {
    let probe=&mut *(context as *mut GameWindowSnapshot);
    let mut owner_pid=0u32;
    if GetWindowThreadProcessId(hwnd,&mut owner_pid)==0 || owner_pid!=GetCurrentProcessId(){
        return 1;
    }
    probe.all+=1;
    if IsWindowVisible(hwnd)==0{return 1;}
    probe.visible+=1;
    if !GetWindow(hwnd,4 /* GW_OWNER */).is_null(){return 1;}
    probe.top_level_visible+=1;
    if probe.geometry.len()<4{
        let mut rect=Rect::default();
        if GetWindowRect(hwnd,&mut rect)!=0{
            let w=rect.right.saturating_sub(rect.left);
            let h=rect.bottom.saturating_sub(rect.top);
            probe.geometry.push(format!("{w}x{h}"));
        }
    }
    1
}

// Read-only details of a handle explicitly supplied by the game. Do not
// log the numeric HWND, title, class name, window text or external process data.
pub fn game_window_snapshot(hwnd:Hwnd)->String {
    if hwnd.is_null(){return "HWND=NULL".into();}
    unsafe {
        if IsWindow(hwnd)==0{return "HWND=invalid".into();}
        let mut pid=0u32;
        if GetWindowThreadProcessId(hwnd,&mut pid)==0 || pid!=GetCurrentProcessId(){
            return "HWND=foreign-or-owner-unavailable".into();
        }
        let mut outer=Rect::default();
        let mut client=Rect::default();
        let outer_ok=GetWindowRect(hwnd,&mut outer)!=0;
        let client_ok=GetClientRect(hwnd,&mut client)!=0;
        let outer_size=if outer_ok{format!("{}x{}",outer.right.saturating_sub(outer.left),
            outer.bottom.saturating_sub(outer.top))}else{"Unavailable".into()};
        let client_size=if client_ok{format!("{}x{}",client.right.saturating_sub(client.left),
            client.bottom.saturating_sub(client.top))}else{"Unavailable".into()};
        let visible=IsWindowVisible(hwnd)!=0;
        let top_level=GetWindow(hwnd,4 /* GW_OWNER */).is_null();
        let style=GetWindowLongW(hwnd,GWL_STYLE) as u32;
        format!("HWND=game-owned visible={visible} unowned={top_level} outer={outer_size} client={client_size} style={style:#010x}")
    }
}

pub fn game_windows_snapshot()->String {
    let mut probe=GameWindowSnapshot::default();
    let result=unsafe{EnumWindows(game_window_callback,&mut probe as *mut _ as isize)};
    if result==0 {return "EnumWindows failed".into();}
    format!("process_windows={} visible={} unowned_visible={} sizes=[{}] (observation only; no HWND selected)",
        probe.all,probe.visible,probe.top_level_visible,probe.geometry.join(","))
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

#[cfg(test)]
mod early_monitor_tests{
    use super::*;
    #[test]
    fn early_monitor_only_when_unambiguous(){
        assert_eq!(sole_monitor_metrics(1,1920,1080),Some((1920,1080)));
        assert_eq!(sole_monitor_metrics(1,3440,1440),Some((3440,1440)));
        for (n,w,h) in [(0,1920,1080),(2,1920,1080),
                         (1,0,1080),(1,1920,0),(1,-1,1080)] {
            assert_eq!(sole_monitor_metrics(n,w,h),None);
        }
    }
}

#[cfg(test)]
mod persistent_ini_tests {
    use super::*;
    #[test]
    fn prior_config_is_preferred_over_older_root_ini() {
        let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("CrimsonSkiesFix-legacy-{}-{nonce}",std::process::id()));
        fs::create_dir_all(root.join("CrimsonFix")).unwrap();
        assert!(previous_ini(&root).is_none());
        fs::write(root.join("CrimsonFix.ini"),"[Debug]\\nLog=0\\n").unwrap();
        assert_eq!(previous_ini(&root),Some(root.join("CrimsonFix.ini")));
        fs::write(root.join("CrimsonFix").join("CrimsonFix.ini"),"[Debug]\\nLog=1\\n").unwrap();
        assert_eq!(previous_ini(&root),Some(root.join("CrimsonFix").join("CrimsonFix.ini")));
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn creates_default_ini_only_once_without_overwriting_edits() {
        let unique=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("CrimsonSkiesFix-ini-test-{}-{unique}",std::process::id()));
        fs::create_dir(&dir).unwrap();
        let file=dir.join("CrimsonFix.ini");
        assert_eq!(create_ini_once(&file).unwrap(),true);
        let generated=fs::read_to_string(&file).unwrap();
        assert!(generated.contains("[Debug]\nLog=0"));
        fs::write(&file,generated.replace("Log=0","Log=1")).unwrap();
        assert_eq!(create_ini_once(&file).unwrap(),false);
        assert!(fs::read_to_string(&file).unwrap().contains("[Debug]\nLog=1"));
        fs::remove_dir_all(dir).unwrap();
    }
}
