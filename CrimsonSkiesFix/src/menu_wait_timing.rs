//! Exact-original-EXE import instrumentation of Win32 waits + passive thread
//! CPU time. An independent follow-up to MENU-API-TIMING.
//!
//! The admitted SafeDisc-free 1.02 PE32 imports KERNEL32.Sleep at RVA 0x61fd1c,
//! KERNEL32.WaitForSingleObject at 0x61fd0c and USER32.MsgWaitForMultipleObjects
//! at 0x6203a4. All three are independently verified against the user-supplied
//! 2,580,480-byte SHA256 43540fc973... EXE's PE import directory and x86
//! indirect-call references. These RVAs are NOT virtual addresses.
//!
//! Only install from the existing verified EXE DirectDraw bootstrap when
//! [Debug] Log=1 and FrameTimeTrace=1. No global DLL export hooks or helper
//! process, no alterations to Sleep duration/Wait timeouts, no timers added
//! to the game and no per-wait logging. The original function executes once,
//! with identical parameters, return and GetLastError.
//!
//! Measurements are only recorded while the already-verified 800x600x16
//! MainMenu/Preferences graph is active. This is the EXE's own import path,
//! not a comprehensive trace of third-party DLL or syscalls. 120-Flip
//! snapshots add one low-rate GetThreadTimes call to identify CPU saturation
//! vs blocked/waiting elapsed wall time on the presenting thread.
use std::{
    ffi::{c_char,c_void},
    sync::atomic::{AtomicU64,AtomicUsize,Ordering},
    time::{Duration,Instant},
};
use crate::{log,menu_api_timing,win32};
const IAT_SLEEP:usize=0x0061_fd1c;
const IAT_WAIT_SINGLE:usize=0x0061_fd0c;
const IAT_MSG_WAIT:usize=0x0062_03a4;
const SLOW_16_US:u64=16_667;
const SLOW_33_US:u64=33_333;
const WAIT_TIMEOUT:u32=0x0000_0102;
const INFINITE:u32=u32::MAX;

type SleepFn=unsafe extern "system" fn(u32);
type WaitFn=unsafe extern "system" fn(*mut c_void,u32)->u32;
type MsgWaitFn=unsafe extern "system" fn(u32,*const *mut c_void,i32,u32,u32)->u32;
static ORIGINAL_SLEEP:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_WAIT:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_MSG_WAIT:AtomicUsize=AtomicUsize::new(0);

#[repr(C)]
#[derive(Clone,Copy,Default)]
struct FileTime{low:u32,high:u32}
impl FileTime{
    fn ticks(self)->u64{u64::from(self.low)|(u64::from(self.high)<<32)}
}
#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn GetLastError()->u32;
    fn SetLastError(error:u32);
    fn GetCurrentThread()->*mut c_void;
    fn GetCurrentThreadId()->u32;
    fn GetThreadTimes(thread:*mut c_void,creation:*mut FileTime,exit:*mut FileTime,
                      kernel:*mut FileTime,user:*mut FileTime)->i32;
}

struct Bucket{
    calls:AtomicU64,
    elapsed_us:AtomicU64,
    slow16:AtomicU64,
    slow33:AtomicU64,
    timeout:AtomicU64,
    infinite_requested:AtomicU64,
    zero_requested:AtomicU64,
    lifetime_max_us:AtomicU64,
}
impl Bucket{
    const fn new()->Self{
        Self{
            calls:AtomicU64::new(0),elapsed_us:AtomicU64::new(0),
            slow16:AtomicU64::new(0),slow33:AtomicU64::new(0),
            timeout:AtomicU64::new(0),infinite_requested:AtomicU64::new(0),
            zero_requested:AtomicU64::new(0),
            lifetime_max_us:AtomicU64::new(0),
        }
    }
    fn record(&self,elapsed:Duration,requested:u32,result:Option<u32>){
        let us=elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.elapsed_us.fetch_add(us,Ordering::Relaxed);
        self.slow16.fetch_add(u64::from(us>=SLOW_16_US),Ordering::Relaxed);
        self.slow33.fetch_add(u64::from(us>=SLOW_33_US),Ordering::Relaxed);
        self.timeout.fetch_add(u64::from(result==Some(WAIT_TIMEOUT)),Ordering::Relaxed);
        self.zero_requested.fetch_add(u64::from(requested==0),Ordering::Relaxed);
        self.infinite_requested.fetch_add(u64::from(requested==INFINITE),Ordering::Relaxed);
        self.lifetime_max_us.fetch_max(us,Ordering::Relaxed);
        self.calls.fetch_add(1,Ordering::Relaxed);
    }
    fn snapshot(&self)->Counts{
        Counts{
            calls:self.calls.load(Ordering::Relaxed),
            elapsed_us:self.elapsed_us.load(Ordering::Relaxed),
            slow16:self.slow16.load(Ordering::Relaxed),
            slow33:self.slow33.load(Ordering::Relaxed),
            timeout:self.timeout.load(Ordering::Relaxed),
            infinite_requested:self.infinite_requested.load(Ordering::Relaxed),
            zero_requested:self.zero_requested.load(Ordering::Relaxed),
            lifetime_max_us:self.lifetime_max_us.load(Ordering::Relaxed),
        }
    }
}
static SLEEP:Bucket=Bucket::new();
static WAIT:Bucket=Bucket::new();
static MSG_WAIT:Bucket=Bucket::new();

unsafe extern "system" fn observed_sleep(ms:u32){
    let ptr=ORIGINAL_SLEEP.load(Ordering::Acquire);
    if ptr==0 || ptr==observed_sleep as usize{return;}
    let original:SleepFn=std::mem::transmute(ptr);
    // Ensure the *input* Win32 last-error state also reaches the real API
    // unchanged: lazy diagnostic TLS/clock initialization may touch it.
    let incoming_error=GetLastError();
    let started=menu_api_timing::start();
    SetLastError(incoming_error);
    original(ms);
    let last_error=GetLastError();
    if let Some(started)=started {SLEEP.record(started.elapsed(),ms,None);}
    SetLastError(last_error);
}
unsafe extern "system" fn observed_wait(handle:*mut c_void,ms:u32)->u32{
    let ptr=ORIGINAL_WAIT.load(Ordering::Acquire);
    if ptr==0 || ptr==observed_wait as usize {return u32::MAX;}
    let original:WaitFn=std::mem::transmute(ptr);
    let incoming_error=GetLastError();
    let started=menu_api_timing::start();
    SetLastError(incoming_error);
    let result=original(handle,ms);
    let last_error=GetLastError();
    if let Some(started)=started{WAIT.record(started.elapsed(),ms,Some(result));}
    SetLastError(last_error);
    result
}
unsafe extern "system" fn observed_msg_wait(
    n:u32,handles:*const *mut c_void,all:i32,ms:u32,mask:u32
)->u32{
    let ptr=ORIGINAL_MSG_WAIT.load(Ordering::Acquire);
    if ptr==0 || ptr==observed_msg_wait as usize {return u32::MAX;}
    let original:MsgWaitFn=std::mem::transmute(ptr);
    let incoming_error=GetLastError();
    let started=menu_api_timing::start();
    SetLastError(incoming_error);
    let result=original(n,handles,all,ms,mask);
    let last_error=GetLastError();
    if let Some(started)=started{MSG_WAIT.record(started.elapsed(),ms,Some(result));}
    SetLastError(last_error);
    result
}

#[derive(Clone,Copy,Debug,Default)]
struct Counts{
    calls:u64,elapsed_us:u64,slow16:u64,slow33:u64,timeout:u64,
    infinite_requested:u64,zero_requested:u64,lifetime_max_us:u64,
}
impl Counts{
    fn since(self,previous:Self)->Self{
        Self{
            calls:self.calls.saturating_sub(previous.calls),
            elapsed_us:self.elapsed_us.saturating_sub(previous.elapsed_us),
            slow16:self.slow16.saturating_sub(previous.slow16),
            slow33:self.slow33.saturating_sub(previous.slow33),
            timeout:self.timeout.saturating_sub(previous.timeout),
            infinite_requested:self.infinite_requested.saturating_sub(previous.infinite_requested),
            zero_requested:self.zero_requested.saturating_sub(previous.zero_requested),
            lifetime_max_us:self.lifetime_max_us,
        }
    }
}
#[derive(Clone,Copy)]
struct ThreadCpu{
    tid:u32,
    wall:Instant,
    cpu_ticks:u64,
}
fn thread_cpu()->Option<ThreadCpu>{
    unsafe{
        let mut creation=FileTime::default();
        let mut exit=FileTime::default();
        let mut kernel=FileTime::default();
        let mut user=FileTime::default();
        if GetThreadTimes(GetCurrentThread(),&mut creation,&mut exit,&mut kernel,&mut user)==0 {
            return None;
        }
        Some(ThreadCpu{
            tid:GetCurrentThreadId(),
            wall:Instant::now(),
            cpu_ticks:kernel.ticks().saturating_add(user.ticks()),
        })
    }
}
#[derive(Clone,Copy)]
pub(crate) struct Snapshot{
    sleep:Counts,wait:Counts,msgwait:Counts,cpu:Option<ThreadCpu>,
}
pub(crate) fn snapshot()->Snapshot{
    Snapshot{
        sleep:SLEEP.snapshot(),
        wait:WAIT.snapshot(),
        msgwait:MSG_WAIT.snapshot(),
        cpu:thread_cpu(),
    }
}
fn bucket(name:&str,now:Counts,previous:Counts)->String{
    let d=now.since(previous);
    format!(
        "{name}[calls={} total_ms={:.3} avg_ms={:.3} slow16={} slow33={} timeout={} requested_zero={} requested_infinite={} lifetime_max_ms={:.3}]",
        d.calls,d.elapsed_us as f64/1000.,
        if d.calls==0 {0.0}else{d.elapsed_us as f64/(d.calls as f64*1000.)},
        d.slow16,d.slow33,d.timeout,d.zero_requested,
        d.infinite_requested,d.lifetime_max_us as f64/1000.,
    )
}
fn cpu_summary(before:Option<ThreadCpu>,after:Option<ThreadCpu>)->String{
    match (before,after){
        (Some(a),Some(b)) if a.tid==b.tid=>{
            let ms=b.cpu_ticks.saturating_sub(a.cpu_ticks) as f64/10_000.0;
            let wall_ms=b.wall.saturating_duration_since(a.wall).as_secs_f64()*1000.0;
            format!("thread_cpu_ms={ms:.3} thread_wall_ms={wall_ms:.3} thread_cpu_pct={:.1} thread_id_stable=true",
                if wall_ms>0.0{100.0*ms/wall_ms}else{0.0})
        },
        (Some(_),Some(_))=>"thread_id_stable=false thread_cpu_ms=unavailable".to_string(),
        _=>"thread_cpu_ms=unavailable thread_id_stable=unknown".to_string(),
    }
}
pub(crate) fn summary(previous:Snapshot,current:Snapshot)->String{
    format!(
        "GAME-WAIT-TIMING: approximately_120_Flips=true {} {} {} {} \
         [exact admitted game EXE IAT calls only; original parameters/results/error unchanged; \
         only MainMenu/Preferences counted; CPU measures Flip thread, not GPU or all process threads; \
         wall/API sums need not equal frame time; max is process lifetime]",
        bucket("Sleep",current.sleep,previous.sleep),
        bucket("WaitForSingleObject",current.wait,previous.wait),
        bucket("MsgWaitForMultipleObjects",current.msgwait,previous.msgwait),
        cpu_summary(previous.cpu,current.cpu),
    )
}

// The resolved import must still point to the real kernel32/user32 function.
// Avoid stealing another mod's hook. Never patch anything except an exact,
// SHA256-gated game EXE slot. The caller invokes us only after exe_gate.
unsafe fn install_one(
    base:usize,module:&[u16],name:&[u8],rva:usize,
    replacement:usize,original:&AtomicUsize
)->bool{
    let lib=GetModuleHandleW(module.as_ptr());
    if lib.is_null(){return false;}
    let expected=GetProcAddress(lib,name.as_ptr().cast());
    if expected.is_null(){return false;}
    let Some(address)=base.checked_add(rva)else{return false;};
    let slot=address as *mut usize;
    let observed=slot.read();
    if observed!=expected as usize || observed==replacement || observed==0 {
        return false;
    }
    if original.compare_exchange(0,observed,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(slot,replacement){
        original.store(0,Ordering::Release);
        return false;
    }
    true
}
pub(crate) fn install(){
    let c=crate::cfg();
    if !c.log_enabled || !c.frame_time_trace || std::mem::size_of::<usize>()!=4 {
        return;
    }
    // The normal caller already verified the exact game EXE checksum.
    // Never call exe_gate::verify_once here: it may display a fatal dialog.
    unsafe{
        let base=GetModuleHandleW(std::ptr::null()) as usize;
        if base==0 {log("GAME-WAIT-IAT: refused (EXE module missing)");return;}
        let kernel:Vec<u16>="kernel32.dll\0".encode_utf16().collect();
        let user:Vec<u16>="user32.dll\0".encode_utf16().collect();
        let sleep=install_one(base,&kernel,b"Sleep\0",IAT_SLEEP,
            observed_sleep as usize,&ORIGINAL_SLEEP);
        let wait=install_one(base,&kernel,b"WaitForSingleObject\0",IAT_WAIT_SINGLE,
            observed_wait as usize,&ORIGINAL_WAIT);
        let msg_wait=install_one(base,&user,b"MsgWaitForMultipleObjects\0",IAT_MSG_WAIT,
            observed_msg_wait as usize,&ORIGINAL_MSG_WAIT);
        log(&format!(
            "GAME-WAIT-IAT: admitted_exe=true scope=game-exe-only Sleep={} WaitForSingleObject={} MsgWaitForMultipleObjects={} read_only_original_calls=true; FrameTimeTrace=1 required; no other modules modified",
            sleep,wait,msg_wait
        ));
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn known_sha_gated_pe32_iat_rvas_are_exact(){
        assert_eq!(IAT_SLEEP,0x61fd1c);
        assert_eq!(IAT_WAIT_SINGLE,0x61fd0c);
        assert_eq!(IAT_MSG_WAIT,0x6203a4);
        let _:SleepFn=observed_sleep;
        let _:WaitFn=observed_wait;
        let _:MsgWaitFn=observed_msg_wait;
    }
    #[test]
    fn wait_stats_handle_timeout_and_zero_without_underflow(){
        let old=Counts{calls:4,elapsed_us:2000,slow16:0,
            slow33:0,timeout:1,zero_requested:2,..Counts::default()};
        let new=Counts{calls:6,elapsed_us:4000,slow16:0,
            slow33:0,timeout:2,zero_requested:2,..Counts::default()};
        let delta=new.since(old);
        assert_eq!(delta.calls,2);
        assert_eq!(delta.timeout,1);
        assert_eq!(delta.zero_requested,0);
        assert_eq!(old.since(new).calls,0);
    }
    #[test]
    fn cpu_summary_rejects_thread_changes(){
        let now=Instant::now();
        let a=ThreadCpu{tid:17,wall:now,cpu_ticks:100};
        let b=ThreadCpu{tid:18,wall:now,cpu_ticks:1000};
        assert!(cpu_summary(Some(a),Some(b)).contains("thread_id_stable=false"));
    }
    #[test]
    fn zero_calls_report_zero_mean(){
        let empty=Snapshot{sleep:Counts::default(),wait:Counts::default(),
            msgwait:Counts::default(),cpu:None};
        let txt=summary(empty,empty);
        assert!(txt.contains("Sleep[calls=0 total_ms=0.000 avg_ms=0.000"));
        assert!(txt.contains("thread_cpu_ms=unavailable"));
    }
}
