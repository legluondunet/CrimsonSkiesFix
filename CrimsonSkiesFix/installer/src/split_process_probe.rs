//! TEST-ONLY, read-only runtime module/IAT observer for the approved 1.02
//! game. The parent installer never injects, patches or suspends the child.
//! A zero process exit status alone cannot establish that WinMM was loaded.
use std::{
    ffi::{c_void,OsString},mem::{size_of,zeroed},
    os::windows::ffi::OsStringExt,
    path::{Path,PathBuf},process::{Command,ExitStatus},
    ptr,thread,time::{Duration,Instant},
};

const TH32CS_SNAPMODULE:u32=0x8;
const TH32CS_SNAPMODULE32:u32=0x10;
const PROCESS_VM_READ:u32=0x10;
const PROCESS_QUERY_INFORMATION:u32=0x400;
const IAT_CREATE:usize=0x61_FBA4;
const IAT_ENUMERATE:usize=0x61_FBA8;

#[repr(C)]
struct ModuleEntryW {
    size:u32,module_id:u32,process_id:u32,glblcnt:u32,proccnt:u32,
    base:*const u8,base_size:u32,handle:*mut c_void,
    name:[u16;256],path:[u16;260],
}
#[link(name="kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags:u32,pid:u32)->*mut c_void;
    fn Module32FirstW(snapshot:*mut c_void,module:*mut ModuleEntryW)->i32;
    fn Module32NextW(snapshot:*mut c_void,module:*mut ModuleEntryW)->i32;
    fn CloseHandle(handle:*mut c_void)->i32;
    fn OpenProcess(rights:u32,inherit:i32,pid:u32)->*mut c_void;
    fn ReadProcessMemory(handle:*mut c_void,addr:*const c_void,dest:*mut c_void,len:usize,read:*mut usize)->i32;
}

struct Module {
    name:String,path:PathBuf,base:usize,size:usize,
}
fn string(buf:&[u16])->String{
    String::from_utf16_lossy(&buf[..buf.iter().position(|&ch|ch==0).unwrap_or(buf.len())])
}
fn module_list(pid:u32)->Option<Vec<Module>>{
    let raw=unsafe{CreateToolhelp32Snapshot(TH32CS_SNAPMODULE|TH32CS_SNAPMODULE32,pid)};
    if raw.is_null()||raw as isize == -1{return None;}
    let mut entry:ModuleEntryW=unsafe{zeroed()};
    entry.size=size_of::<ModuleEntryW>() as u32;
    let mut list=Vec::new();
    let mut active=unsafe{Module32FirstW(raw,&mut entry)}!=0;
    while active {
        list.push(Module{
            name:string(&entry.name).to_ascii_lowercase(),
            path:PathBuf::from(OsString::from_wide(&entry.path)),
            base:entry.base as usize,
            size:entry.base_size as usize,
        });
        active=unsafe{Module32NextW(raw,&mut entry)}!=0;
    }
    unsafe{CloseHandle(raw)};
    Some(list)
}
fn is_inside(value:usize,module:&Module)->bool{
    value>=module.base && value<module.base.saturating_add(module.size)
}
fn read_import(pid:u32,module:&Module,offset:usize)->Option<usize>{
    let process=unsafe{OpenProcess(PROCESS_VM_READ|PROCESS_QUERY_INFORMATION,0,pid)};
    if process.is_null(){return None;}
    let mut value=0u32;
    let mut len=0usize;
    let ok=unsafe{ReadProcessMemory(process,
        module.base.checked_add(offset)? as *const c_void,
        (&mut value as *mut u32).cast(),4,&mut len)};
    unsafe{CloseHandle(process)};
    (ok!=0&&len==4).then_some(value as usize)
}
fn same_path(a:&Path,b:&Path)->bool{
    a.to_string_lossy().replace('/',"\\")
        .eq_ignore_ascii_case(&b.to_string_lossy().replace('/',"\\"))
}

#[derive(Default)]
struct Evidence {
    samples:u32,
    local_winmm:bool,
    system_winmm:bool,
    core_seen:bool,
    redirected:bool,
    unpatched:bool,
    inconsistent:bool,
    snapshot_unavailable:bool,
}
impl Evidence{
    fn observe(&mut self,pid:u32,game:&Path){
        let Some(modules)=module_list(pid) else {
            self.snapshot_unavailable=true;
            return;
        };
        self.samples+=1;
        let expected_shim=game.join("winmm.dll");
        let expected_core=game.join("crimsonskiesfix").join("CrimsonSkiesFix.dll");
        let shim=modules.iter().find(|module|module.name=="winmm.dll");
        if let Some(shim)=shim{
            if same_path(&shim.path,&expected_shim){self.local_winmm=true;}
            else{self.system_winmm=true;}
        }
        self.core_seen|=modules.iter().any(|m|m.name=="crimsonskiesfix.dll"&&same_path(&m.path,&expected_core));
        let exe=modules.iter().find(|m|m.name=="crimson.exe");
        if let (Some(exe),Some(shim))=(exe,shim){
            let first=read_import(pid,exe,IAT_CREATE);
            let second=read_import(pid,exe,IAT_ENUMERATE);
            match (first,second){
                (Some(a),Some(b)) if is_inside(a,shim)&&is_inside(b,shim)=>{
                    self.redirected=true;
                }
                (Some(a),Some(b)) if is_inside(a,shim)!=is_inside(b,shim)=>{
                    self.inconsistent=true;
                }
                (Some(_),Some(_))=>self.unpatched=true,
                _=>{}
            }
        }
    }
    fn summary(&self)->String{
        let provider=if self.local_winmm{"local"}else if self.system_winmm{"system"}else{"unobserved"};
        let iat=if self.redirected{"redirected"}else if self.inconsistent{"partial"}else if self.unpatched{"not-redirected"}else{"unknown"};
        format!("SPLIT-PROBE: snapshots={} winmm_provider={} core_dll_seen={} directdraw_iat={} snapshot_unavailable={} (read-only, sampled while game ran; no loader-lock assertion)",
            self.samples,provider,self.core_seen,iat,self.snapshot_unavailable)
    }
}
pub(super) fn launch(exe:&Path,game:&Path)->Result<(ExitStatus,String),std::io::Error>{
    let mut child=Command::new(exe).current_dir(game).spawn()?;
    let pid=child.id();
    let start=Instant::now();
    let mut evidence=Evidence::default();
    // Limit sampling to twelve seconds to keep background observation bounded.
    // Exit and game rendering are never delayed by the probe.
    loop {
        evidence.observe(pid,game);
        if let Some(status)=child.try_wait()?{
            return Ok((status,evidence.summary()));
        }
        if start.elapsed()>=Duration::from_secs(12){break;}
        thread::sleep(Duration::from_millis(120));
    }
    let status=child.wait()?;
    Ok((status,evidence.summary()))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn module_range_and_path_comparison_are_bounded(){
        let m=Module{name:"winmm.dll".to_owned(),path:PathBuf::new(),base:0x1000,size:0x100};
        assert!(is_inside(0x1000,&m));
        assert!(is_inside(0x10ff,&m));
        assert!(!is_inside(0x1100,&m));
        assert!(same_path(Path::new(r"C:\Demo\winmm.dll"),Path::new(r"c:\demo\WINMM.dll")));
    }
    #[test]
    fn incomplete_sample_cannot_claim_installed_shim_worked(){
        let s=Evidence::default().summary();
        assert!(s.contains("directdraw_iat=unknown"));
        assert!(s.contains("winmm_provider=unobserved"));
    }
}
