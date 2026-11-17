//! TEST-ONLY, read-only runtime module/IAT observer for the approved 1.02
//! game. The parent installer never injects, patches or suspends the child.
//! A zero process exit status alone cannot establish that WinMM was loaded.
use std::{
    ffi::{c_void,OsString},mem::{size_of,zeroed},
    os::windows::ffi::OsStringExt,
    path::{Path,PathBuf},process::{Command,ExitStatus},
    thread,time::{Duration,Instant,SystemTime},fs,io::Read,
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
    named_winmm:bool,
    redirected_named_winmm:bool,
    redirected_local_winmm:bool,
    core_path_match:bool,
    core_module_seen:bool,
    runtime_log_attested:bool,
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
        let shims=modules.iter().filter(|m|m.name=="winmm.dll")
            .collect::<Vec<_>>();
        self.named_winmm|=!shims.is_empty();
        self.local_winmm|=shims.iter().any(|m|same_path(&m.path,&expected_shim));
        self.core_module_seen|=modules.iter().any(|m|m.name=="crimsonskiesfix.dll");
        self.core_path_match|=modules.iter().any(|m|
            m.name=="crimsonskiesfix.dll"&&same_path(&m.path,&expected_core));
        if let Some(exe)=modules.iter().find(|m|m.name=="crimson.exe"){
            let first=read_import(pid,exe,IAT_CREATE);
            let second=read_import(pid,exe,IAT_ENUMERATE);
            if let (Some(a),Some(b))=(first,second){
                let in_any_named=shims.iter().any(|m|is_inside(a,m)) &&
                    shims.iter().any(|m|is_inside(b,m));
                let in_local=shims.iter().any(|m|
                    same_path(&m.path,&expected_shim) && is_inside(a,m) && is_inside(b,m));
                if in_local{self.redirected_local_winmm=true;}
                if in_any_named{self.redirected_named_winmm=true;}
                if (shims.iter().any(|m|is_inside(a,m))) !=
                   (shims.iter().any(|m|is_inside(b,m))){
                    self.inconsistent=true;
                }
                if !in_any_named && !self.inconsistent{
                    self.unpatched=true;
                }
            }
        }
    }
    fn summary(&self)->String{
        // Wine/Proton may expose a builtin winmm.dll before or instead of
        // the app-local PE in Toolhelp32. It is NEVER sufficient to take
        // the first name match and label that "system". Cross-check
        // the actual IAT targets and a fresh runtime-owned startup log.
        let provider=if self.redirected_local_winmm {
            "local"
        }else if self.runtime_log_attested && self.redirected_named_winmm {
            "runtime-confirmed-path-unverified"
        }else if self.redirected_named_winmm {
            "winmm-module-path-unverified"
        }else if self.runtime_log_attested {
            "runtime-confirmed-module-unobserved"
        }else{
            "unverified"
        };
        let core=if self.core_path_match{"verified-path"}
        else if self.runtime_log_attested{"runtime-log-fresh"}
        else if self.core_module_seen{"module-path-unverified"}
        else{"unverified"};
        let iat=if self.redirected_local_winmm || self.redirected_named_winmm{
            "redirected"
        }else if self.inconsistent{"partial"}
        else if self.unpatched{"not-redirected"}
        else{"unknown"};
        // A missing Toolhelp32 entry is not a loaded-module absence proof,
        // particularly for Wine/Proton paths and forwarded native DLLs.
        let core_seen=if self.core_path_match || self.runtime_log_attested {
            "true"
        }else{"unverified"};
        format!(
            "SPLIT-PROBE: snapshots={} winmm_provider={} core_dll_seen={} core_evidence={} directdraw_iat={} runtime_log_fresh={} snapshot_unavailable={} (read-only; Wine module paths may differ; never infer module absence from Toolhelp alone)",
            self.samples,provider,core_seen,
            core,iat,self.runtime_log_attested,self.snapshot_unavailable)
    }
}
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
struct LogStamp{modified:Option<SystemTime>,len:u64}
fn stamp(path:&Path)->Option<LogStamp>{
    let meta=fs::metadata(path).ok()?;
    if !meta.is_file(){return None;}
    Some(LogStamp{modified:meta.modified().ok(),len:meta.len()})
}
fn current_split_runtime_log(log_path:&Path,previous:Option<LogStamp>)->bool{
    let Some(now)=stamp(log_path) else{return false;};
    if now.len==0 || now.len>32*1024*1024 || Some(now)==previous{
        return false;
    }
    let Ok(mut file)=fs::File::open(log_path) else{return false;};
    let mut bytes=Vec::new();
    if file.by_ref().take(64*1024).read_to_end(&mut bytes).is_err(){return false;}
    let Ok(prefix)=String::from_utf8(bytes) else{return false;};
    let first=prefix.lines().next().unwrap_or("");
    // The first line is written by the nested runtime, after its WinMM
    // bootstrap handoff. A stale file, no Log=1, wrong build or an
    // unrelated game process must not be upgraded to "confirmed".
    let expected=option_env!("CRIMSONSKIESFIX_COMMIT")
        .and_then(|s|s.get(..8))
        .filter(|s|s.chars().all(|c|c.is_ascii_hexdigit()));
    first.starts_with("CrimsonSkiesFix 1.1.0-rc.1-") &&
        first.contains("WinMM IAT bootstrap") &&
        expected.is_some_and(|sha|first.contains(sha)) &&
        prefix.contains("WINMM-BOOTSTRAP: game DirectDraw API reached; iat_status=7")
}

pub(super) fn launch(exe:&Path,game:&Path)->Result<(ExitStatus,String),std::io::Error>{
    let log_path=game.parent().unwrap_or(game).join("CrimsonSkiesFix.log");
    let prior_log=stamp(&log_path);
    let mut child=Command::new(exe).current_dir(game).spawn()?;
    let pid=child.id();
    let start=Instant::now();
    let mut evidence=Evidence::default();
    // Limit sampling to twelve seconds to keep background observation bounded.
    // Exit and game rendering are never delayed by the probe.
    loop {
        evidence.observe(pid,game);
        if let Some(status)=child.try_wait()?{
            evidence.runtime_log_attested=current_split_runtime_log(&log_path,prior_log);
            return Ok((status,evidence.summary()));
        }
        if start.elapsed()>=Duration::from_secs(12){break;}
        thread::sleep(Duration::from_millis(120));
    }
    let status=child.wait()?;
    evidence.runtime_log_attested=current_split_runtime_log(&log_path,prior_log);
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
    fn wine_module_name_only_never_claims_system_provider_or_absent_core(){
        let mut e=Evidence::default();
        e.named_winmm=true;
        e.redirected_named_winmm=true;
        let s=e.summary();
        assert!(s.contains("winmm_provider=winmm-module-path-unverified"));
        assert!(s.contains("core_dll_seen=unverified"));
        e.runtime_log_attested=true;
        let s=e.summary();
        assert!(s.contains("winmm_provider=runtime-confirmed-path-unverified"));
        assert!(s.contains("core_dll_seen=true"));
        assert!(s.contains("core_evidence=runtime-log-fresh"));
        assert!(s.contains("directdraw_iat=redirected"));
        e.redirected_local_winmm=true;
        assert!(e.summary().contains("winmm_provider=local"));
    }
    #[test]
    fn stale_missing_or_blank_runtime_log_cannot_attest_game_start(){
        let path=std::env::temp_dir().join(format!(
            "crimson-probe-log-test-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos()));
        assert!(!current_split_runtime_log(&path,None));
        fs::write(&path,b"not a runtime log").unwrap();
        assert!(!current_split_runtime_log(&path,None));
        let baseline=stamp(&path);
        assert!(!current_split_runtime_log(&path,baseline));
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn current_runtime_log_attestation_requires_new_matching_build_and_live_bootstrap(){
        let Some(commit)=option_env!("CRIMSONSKIESFIX_COMMIT") else{return;};
        let expected=&commit[..8];
        let path=std::env::temp_dir().join(format!(
            "crimson-probe-attest-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos()));
        fs::write(&path,format!(
            "CrimsonSkiesFix 1.1.0-rc.1-{expected} x86 - init; WinMM IAT bootstrap\nWINMM-BOOTSTRAP: game DirectDraw API reached; iat_status=7\n"
        )).unwrap();
        assert!(current_split_runtime_log(&path,None));
        let before=stamp(&path);
        assert!(!current_split_runtime_log(&path,before));
        fs::write(&path,format!(
            "CrimsonSkiesFix 1.1.0-rc.1-{expected} x86 - init; WinMM IAT bootstrap\nWINMM-BOOTSTRAP: game DirectDraw API reached; iat_status=4\n"
        )).unwrap();
        assert!(!current_split_runtime_log(&path,None));
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn native_windows_module_snapshot_can_enumerate_our_own_process(){
        let modules=module_list(std::process::id()).expect("Toolhelp32 cannot enumerate own process");
        assert!(!modules.is_empty(),"empty module inventory");
        assert!(modules.iter().all(|module|module.size>0&&module.base!=0));
    }
    #[test]
    fn readprocessmemory_can_read_a_known_marker_without_mutation(){
        static KNOWN:u32=0x45_C0_FF_EE;
        let addr=&KNOWN as *const u32 as usize;
        let offset=0x100usize;
        let module=Module {
            name:"synthetic".into(),path:PathBuf::new(),
            base:addr-offset,size:offset+4,
        };
        assert_eq!(read_import(std::process::id(),&module,offset),Some(KNOWN as usize));
    }
    #[test]
    fn incomplete_sample_cannot_claim_installed_shim_worked(){
        let s=Evidence::default().summary();
        assert!(s.contains("directdraw_iat=unknown"));
        assert!(s.contains("winmm_provider=unverified"));
    }
}
