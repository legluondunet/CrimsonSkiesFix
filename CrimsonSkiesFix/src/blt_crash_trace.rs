//! Bounded rotating crash-only native Blt call trace.
//! The original file stream, 2048-call segment policy, ENTER-before-COM and
//! EXIT-after-COM semantics are unchanged. No global trace is enabled unless
//! [Debug] Log=1 and CrashBltTrace=1.
use std::{fs::File,io::{Write,Seek,SeekFrom},sync::{Mutex,OnceLock,atomic::AtomicUsize}};

pub(crate) static CRASH_BLT_LOG:OnceLock<Mutex<File>>=OnceLock::new();
pub(crate) const CRASH_BLT_ROTATE_CALLS:usize=2048;
pub(crate) static NATIVE_BLT_CRASH_TRACE_ID:AtomicUsize=AtomicUsize::new(0);

pub(crate) fn crash_blt_rotation_needed(id:usize)->bool {
    id>1 && (id-1)%CRASH_BLT_ROTATE_CALLS==0
}

// All crash tracing uses a separate game-local file so the normal project
// log stays small. Rewind once per 2048 completed/attempted native calls;
// the most recent ENTER is retained even if Wine terminates inside Blt.
// The lock is never held while entering Wine's original COM implementation.
pub(crate) fn crash_blt_line(id:usize,is_enter:bool,line:&str){
    let Some(lock)=CRASH_BLT_LOG.get() else{return;};
    let Ok(mut file)=lock.lock() else{return;};
    if is_enter && crash_blt_rotation_needed(id){
        if file.set_len(0).is_err() || file.seek(SeekFrom::Start(0)).is_err(){
            // A failed rewind is not allowed to change game graphics behavior.
            return;
        }
        let _=writeln!(file,"CRASH-BLT: ROLLING SEGMENT starts_at_id={id} span={CRASH_BLT_ROTATE_CALLS}");
    }
    let _=writeln!(file,"{line}");
}


