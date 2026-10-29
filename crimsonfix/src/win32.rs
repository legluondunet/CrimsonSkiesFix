use std::{ffi::{c_char, c_void, OsString}, os::windows::ffi::OsStringExt, path::PathBuf};

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
}

pub fn path_next_to(module:*mut c_void,name:&str)->Option<PathBuf> {
    let mut buffer=[0u16;32768];
    let n=unsafe {GetModuleFileNameW(module,buffer.as_mut_ptr(),buffer.len() as u32)} as usize;
    if n==0 || n>=buffer.len(){return None;}
    let filename=PathBuf::from(OsString::from_wide(&buffer[..n]));
    Some(filename.parent()?.join(name))
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

pub fn borderless_window(hwnd:Hwnd,area:Rect)->bool {
    if hwnd.is_null() {return false;}
    unsafe {
        if IsWindow(hwnd)==0{return false;}
        // Preserve the window's existing unrelated style flags; remove borders
        // and add WS_POPUP while retaining visibility.
        let existing=GetWindowLongW(hwnd,GWL_STYLE) as u32;
        let borders=0x00CF_0000u32; // caption, thickframe, sysmenu, min/max boxes
        let new_style=((existing & !borders)|WS_POPUP|WS_VISIBLE) as i32;
        SetWindowLongW(hwnd,GWL_STYLE,new_style);
        let w=area.right.saturating_sub(area.left);
        let h=area.bottom.saturating_sub(area.top);
        w>0 && h>0 && SetWindowPos(hwnd,std::ptr::null_mut(),area.left,area.top,w,h,
                                  SWP_FRAMECHANGED|SWP_SHOWWINDOW|SWP_NOOWNERZORDER)!=0
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
