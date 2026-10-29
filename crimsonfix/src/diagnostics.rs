//! Passive Windows/Wine diagnostics, translated from the GPL-3.0-or-later
//! SpiderMan2k1Fix system_diagnostics.c (PR #83 + #84).
//! https://github.com/legluondunet/SpiderMan2k1Fix
//! Never called from DllMain; only on explicit [Debug] Log=1.

use std::{
    env, ffi::{c_char, c_void, CStr, CString},
    mem, ptr, slice, time::Instant,
};

use crate::diagnostics_graphics;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Guid { data1:u32, data2:u16, data3:u16, data4:[u8;8] }

#[repr(C)]
struct OsVersion {
    size:u32, major:u32, minor:u32, build:u32, platform:u32, service_pack:[u16;128],
}

#[repr(C)]
struct SystemInfo {
    architecture:u16, reserved:u16, page_size:u32,
    min_address:*mut c_void, max_address:*mut c_void,
    active_mask:usize, logical:u32, processor_type:u32,
    allocation_granularity:u32, level:u16, revision:u16,
}

#[repr(C)]
struct MemoryStatus {
    size:u32, load:u32, total_physical:u64, available_physical:u64,
    total_page_file:u64, available_page_file:u64,
    total_virtual:u64, available_virtual:u64, extended_virtual:u64,
}

#[repr(C)]
struct DisplayDeviceW {
    size:u32, device_name:[u16;32], description:[u16;128], flags:u32,
    device_id:[u16;128], registry_key:[u16;128],
}

#[repr(C)]
struct DevModeW {
    device_name:[u16;32], spec:u16, driver:u16, size:u16, extra:u16,
    fields:u32, device_union:[u8;16],
    color:u16, duplex:u16, y_resolution:u16, tt_option:u16, collate:u16,
    form_name:[u16;32], log_pixels:u16, bits_per_pel:u32,
    width:u32, height:u32, display_flags:u32, refresh:u32,
    icm_method:u32, icm_intent:u32, media_type:u32, dither_type:u32,
    reserved1:u32, reserved2:u32, panning_width:u32, panning_height:u32,
}

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void, name:*const c_char)->*const c_void;
    fn GetSystemInfo(info:*mut SystemInfo);
    fn GlobalMemoryStatusEx(info:*mut MemoryStatus)->i32;
    fn GetLogicalProcessorInformation(info:*mut c_void, size:*mut u32)->i32;
    fn GetLastError()->u32;
}
#[link(name="advapi32")]
extern "system" {
    fn RegOpenKeyExW(key:*mut c_void, subkey:*const u16, reserved:u32, access:u32,
                     result:*mut *mut c_void)->i32;
    fn RegQueryValueExW(key:*mut c_void,name:*const u16,reserved:*mut u32,
                        value_type:*mut u32,data:*mut u8,size:*mut u32)->i32;
    fn RegCloseKey(key:*mut c_void)->i32;
}
#[link(name="user32")]
extern "system" {
    fn GetSystemMetrics(index:i32)->i32;
    fn EnumDisplayDevicesW(device:*const u16,index:u32,out:*mut DisplayDeviceW,flags:u32)->i32;
    fn EnumDisplaySettingsW(device:*const u16,index:u32,out:*mut DevModeW)->i32;
}

fn wstr(s:&str)->Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn wide_to_string(s:&[u16])->String {
    String::from_utf16_lossy(&s[..s.iter().position(|x|*x==0).unwrap_or(s.len())])
}
fn utf8(ptr:*const c_char,max:usize)->String {
    if ptr.is_null(){return String::new();}
    // Source APIs return NUL-terminated static or structure-bounded buffers.
    // Callers cap the displayed output and never print raw C strings.
    let s=unsafe {CStr::from_ptr(ptr)}.to_string_lossy();
    s.chars().take(max).collect()
}

pub(super) fn clean_label(raw:&str,max:usize)->String {
    if raw.is_empty() || raw.chars().any(|c|c=='/' || c=='\\' || c=='@' || c.is_control()){
        return "Unavailable".into();
    }
    let line:String=raw.chars().take(max).map(|c|match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|' '|'.'|'_'|'-'|'+'|':'|'('|')'|','|'#'=>c,
        _=>'?',
    }).collect();
    let text=line.trim();
    if text.is_empty(){"Unavailable".into()}else{text.to_owned()}
}
fn env_clean(name:&str,max:usize)->Option<String> {
    let raw=env::var(name).ok()?;
    if raw.len()>191 || raw.contains(['/', '\\', '@', '\n', '\r','\t']) {return None;}
    let clean=clean_label(&raw,max);
    (clean!="Unavailable").then_some(clean)
}
fn proton_label()->Option<String> {
    // Scan directory *names* only. Never publish the full env path.
    let raw=env::var("STEAM_COMPAT_TOOL_PATHS").ok()?;
    if raw.len()>4096 {return None;}
    for part in raw.split(['/', '\\', ':', ';']) {
        if part.is_empty() || part.len()>89 || !part.is_ascii(){continue;}
        let low=part.to_ascii_lowercase();
        let ok=low.strip_prefix("ge-proton").is_some_and(|rest|rest.starts_with(|c:char|c.is_ascii_digit()))
          || low.strip_prefix("proton ").is_some_and(|rest|rest.starts_with(|c:char|c.is_ascii_digit())
                                                  || rest=="experimental" || rest=="hotfix")
          || low.strip_prefix("proton-").is_some_and(|rest|rest.starts_with(|c:char|c.is_ascii_digit()));
        if ok && part.bytes().all(|c|c.is_ascii_alphanumeric() || b" .-_+".contains(&c)){
            return Some(clean_label(part,89));
        }
    }
    None
}
unsafe fn api(module:*mut c_void,name:&[u8])->*const c_void {
    if module.is_null(){ptr::null()}else{GetProcAddress(module,name.as_ptr().cast())}
}

fn log_os(emit:&mut dyn FnMut(&str)) {
    unsafe {
        let ntdll=GetModuleHandleA(b"ntdll.dll\0".as_ptr().cast());
        type WineVersion=unsafe extern "C" fn()->*const c_char;
        type WineHost=unsafe extern "C" fn(*mut *const c_char,*mut *const c_char);
        type RtlVersion=unsafe extern "system" fn(*mut OsVersion)->i32;
        let wine_ptr=api(ntdll,b"wine_get_version\0");
        let host_ptr=api(ntdll,b"wine_get_host_version\0");
        let rtl_ptr=api(ntdll,b"RtlGetVersion\0");
        let under_wine=!wine_ptr.is_null();
        emit(if under_wine {"SYS-OS: runtime=Wine (Win32 compatibility layer)"}
             else {"SYS-OS: runtime=Windows native"});
        let mut version:OsVersion=mem::zeroed();
        version.size=mem::size_of::<OsVersion>() as u32;
        if !rtl_ptr.is_null() {
            let rtl:RtlVersion=mem::transmute(rtl_ptr);
            if rtl(&mut version)==0 {
                let label=if version.major==10 && version.build>=22000 {"Windows 11-compatible"}
                    else if version.major==10 {"Windows 10-compatible"}
                    else if version.major==6 && version.minor==3 {"Windows 8.1-compatible"}
                    else if version.major==6 && version.minor==1 {"Windows 7-compatible"}
                    else {"Windows NT version reported by RtlGetVersion"};
                emit(&format!("SYS-OS: Win32 version={}.{} build={} ({}{})",
                    version.major,version.minor,version.build,label,
                    if under_wine {"; emulated by Wine, NOT host OS"}else{""}));
            } else {emit("SYS-OS: Win32 version=Unavailable (RtlGetVersion)");}
        } else {emit("SYS-OS: Win32 version=Unavailable (RtlGetVersion)");}
        if under_wine {
            let wine:WineVersion=mem::transmute(wine_ptr);
            emit(&format!("SYS-WINE: version={}",clean_label(&utf8(wine(),100),100)));
            if !host_ptr.is_null() {
                let host:WineHost=mem::transmute(host_ptr);
                let mut os=ptr::null();
                let mut release=ptr::null();
                host(&mut os,&mut release);
                emit(&format!("SYS-HOST: OS={} kernel={} (reported by Wine)",
                    clean_label(&utf8(os,60),60),clean_label(&utf8(release,100),100)));
            } else {emit("SYS-HOST: OS=Unavailable kernel=Unavailable");}
            let steam=["STEAM_COMPAT_DATA_PATH","STEAM_COMPAT_CLIENT_INSTALL_PATH",
                       "STEAM_COMPAT_TOOL_PATHS","PROTON_VERSION"]
                .iter().any(|name|env::var_os(name).is_some());
            if steam {
                if let Some(version)=env_clean("PROTON_VERSION",95){
                    emit(&format!("SYS-PROTON: Steam compatibility environment=yes version={} (environment hint, unverified)",version));
                } else if let Some(label)=proton_label() {
                    emit(&format!("SYS-PROTON: Steam compatibility environment=yes tool={} (directory name hint, NOT verified runtime version)",label));
                } else {
                    emit("SYS-PROTON: Steam compatibility environment=yes version=Unavailable (not exposed by Wine/Steam)");
                }
            } else {emit("SYS-PROTON: environment hint=not detected; version=Unavailable (cannot exclude Proton)");}
        } else {
            emit("SYS-WINE: not detected");
            emit("SYS-HOST: OS=Windows");
            emit("SYS-PROTON: not detected (native Windows)");
        }
    }
}

fn cpu_name()->String {
    unsafe {
        let mut key=ptr::null_mut();
        let path=wstr(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0");
        if RegOpenKeyExW(0x8000_0002usize as *mut c_void,path.as_ptr(),0,0x20019,&mut key)!=0 {
            return "Unavailable".into();
        }
        let mut data=[0u16;160];
        let mut size=mem::size_of_val(&data) as u32;
        let mut kind=0;
        let name=wstr("ProcessorNameString");
        let result=RegQueryValueExW(key,name.as_ptr(),ptr::null_mut(),&mut kind,
                                    data.as_mut_ptr().cast(),&mut size);
        RegCloseKey(key);
        if result!=0 || kind!=1 || size==0{return "Unavailable".into();}
        let raw=wide_to_string(&data);
        clean_label(&raw.replace(" w/ "," with "),150)
    }
}
fn log_cpu_memory(emit:&mut dyn FnMut(&str)) {
    emit(&format!("SYS-CPU: model={}",cpu_name()));
    unsafe {
        let mut info:SystemInfo=mem::zeroed();
        GetSystemInfo(&mut info);
        let mut bytes=0u32;
        let mut cores=0u32;
        // 32-bit SYSTEM_LOGICAL_PROCESSOR_INFORMATION has a 24-byte layout:
        // ULONG_PTR mask, DWORD relationship, 16-byte union. Never invoke WMI.
        let first=GetLogicalProcessorInformation(ptr::null_mut(),&mut bytes);
        if first==0 && GetLastError()==122 && bytes>=24 && bytes<=1024*1024 {
            let mut buffer=vec![0u8;bytes as usize];
            if GetLogicalProcessorInformation(buffer.as_mut_ptr().cast(),&mut bytes)!=0 {
                for chunk in buffer[..(bytes as usize).min(buffer.len())].chunks_exact(24) {
                    if u32::from_ne_bytes(chunk[4..8].try_into().unwrap())==0 { cores+=1; }
                }
            }
        }
        emit(&format!("SYS-CPU: physical_cores={} logical_processors={} (Win32 view)",
            if cores>0{cores.to_string()}else{"Unavailable".into()},info.logical));
        let mut status:MemoryStatus=mem::zeroed();
        status.size=mem::size_of::<MemoryStatus>() as u32;
        if GlobalMemoryStatusEx(&mut status)!=0 {
            emit(&format!("SYS-RAM: total={} MiB available={} MiB (Win32-reported; may differ from Linux host)",
                status.total_physical/1_048_576,status.available_physical/1_048_576));
        }else{emit("SYS-RAM: Unavailable (GlobalMemoryStatusEx)");}
    }
}

fn log_screens(emit:&mut dyn FnMut(&str)) {
    unsafe {
        emit(&format!("SYS-DISPLAYS: active_monitors={} primary_desktop={}x{} virtual_desktop={}x{}",
            GetSystemMetrics(80),GetSystemMetrics(0),GetSystemMetrics(1),
            GetSystemMetrics(78),GetSystemMetrics(79)));
        let mut active=0;
        for n in 0..16 {
            if active>=8{break;}
            let mut d:DisplayDeviceW=mem::zeroed();
            d.size=mem::size_of::<DisplayDeviceW>() as u32;
            if EnumDisplayDevicesW(ptr::null(),n,&mut d,0)==0{break;}
            if d.flags&1==0 {continue;}
            active+=1;
            let device=wide_to_string(&d.device_name);
            let adapter=clean_label(&wide_to_string(&d.description),150);
            let simple=clean_label(device.rsplit('\\').next().unwrap_or(""),48);
            let mut mode:DevModeW=mem::zeroed();
            mode.size=mem::size_of::<DevModeW>() as u16;
            let primary=if d.flags&4!=0{"yes"}else{"no"};
            if EnumDisplaySettingsW(d.device_name.as_ptr(),0xFFFF_FFFF,&mut mode)!=0 {
                emit(&format!("SYS-DISPLAY: screen={} device={} primary={} resolution={}x{} refresh={}Hz adapter={}",
                    active,simple,primary,mode.width,mode.height,mode.refresh,adapter));
            }else {
                emit(&format!("SYS-DISPLAY: screen={} device={} primary={} resolution=Unavailable adapter={}",
                    active,simple,primary,adapter));
            }
        }
        if active==0 {emit("SYS-DISPLAY: no active GDI display adapter enumerated");}
    }
}

fn log_desktop(emit:&mut dyn FnMut(&str)) {
    if let Some(de)=env_clean("XDG_CURRENT_DESKTOP",105){
        emit(&format!("SYS-DESKTOP: DE={} (XDG_CURRENT_DESKTOP hint)",de));
    } else {emit("SYS-DESKTOP: DE=Unavailable (Windows or Wine not exposing XDG_CURRENT_DESKTOP)");}
    if let Some(session)=env_clean("XDG_SESSION_TYPE",85){
        emit(&format!("SYS-DESKTOP: session={} (XDG_SESSION_TYPE hint)",session));
    }
}

pub fn report(mut emit:impl FnMut(&str)) {
    let start=Instant::now();
    emit("SYS-INFO: BEGIN (passive startup inventory; diagnostics only; no personal identifiers)");
    log_os(&mut emit);
    log_cpu_memory(&mut emit);
    log_screens(&mut emit);
    diagnostics_graphics::log_graphics(&mut emit);
    log_desktop(&mut emit);
    diagnostics_graphics::log_controllers(&mut emit);
    emit(&format!("SYS-INFO: END elapsed_ms={} (unknown means unreported, not absent)",
        start.elapsed().as_millis().min(u32::MAX as u128)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prevent_path_or_control_leaks(){
        for value in [r"C:\Users\example", "/home/someone", "foo@bar", "hey\nSYS-FAKE"] {
            assert_eq!(clean_label(value,100),"Unavailable");
        }
        assert_eq!(clean_label("AMD Ryzen 7 with Radeon Graphics",70),"AMD Ryzen 7 with Radeon Graphics");
    }
    #[test]
    fn reject_unknown_proton_env_values(){
        assert_eq!(clean_label("",20),"Unavailable");
        assert_eq!(clean_label("ordinary-model",30),"ordinary-model");
    }
    #[test]
    fn verify_struct_layouts_x86(){
        if cfg!(target_pointer_width="32") {
            assert_eq!(mem::size_of::<OsVersion>(),276);
            assert_eq!(mem::size_of::<SystemInfo>(),36);
            assert_eq!(mem::size_of::<MemoryStatus>(),64);
            assert_eq!(mem::size_of::<DisplayDeviceW>(),840);
            assert_eq!(mem::size_of::<DevModeW>(),220);
        }
    }
}
