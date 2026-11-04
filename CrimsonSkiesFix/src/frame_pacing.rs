//! Original x86 DirectDraw frame pacing implementation, moved intact from
//! lib.rs. Invoked only from the existing opt-in pacing call sites;
//! this module does not change simulation or select frame boundaries.
use std::sync::{Mutex,OnceLock};
use crate::{cfg,win32};

static LAST_FRAME:Mutex<i64>=Mutex::new(0);
static FREQUENCY:OnceLock<i64>=OnceLock::new();

pub(crate) fn pace_frame() {
    let fps=cfg().fps_limit;
    let frequency=*FREQUENCY.get_or_init(|| unsafe {
        let mut value=0;
        if win32::QueryPerformanceFrequency(&mut value)==0 || value<=0 {0}else{value}
    });
    if frequency<=0 {return;}
    let Ok(mut previous)=LAST_FRAME.lock() else {return};
    let mut now=0;
    if unsafe {win32::QueryPerformanceCounter(&mut now)}==0{return;}
    let interval=(frequency/fps as i64).max(1);
    if *previous>0 && now<*previous+interval {
        let deadline=*previous+interval;
        while now<deadline {
            let ticks=deadline-now;
            let millis=ticks.saturating_mul(1000)/frequency;
            if millis>2 {unsafe {win32::Sleep((millis-1).min(50) as u32)};}
            else {std::hint::spin_loop();}
            if unsafe {win32::QueryPerformanceCounter(&mut now)}==0{break;}
        }
    }
    *previous=now;
}

