//! Bounded, diagnostic-only native DirectDraw/Direct3D API call-rate sampling.
//! Not an FPS counter. The sampling windows and log message text are retained
//! exactly from the original lib.rs implementation.
use std::{sync::{Mutex,OnceLock,atomic::{AtomicUsize,Ordering}},time::Instant};
use crate::{cfg,log};

/// API-call sampling is a deeper performance diagnostic, not part of the
/// normal Log=1 gameplay report. Both flags must be explicitly enabled.
fn sampling_enabled(config:&crate::config::Config)->bool{
    config.log_enabled && config.frame_time_trace
}

// These cumulative rates are COM call rates, not proven on-screen FPS.
// A scene can issue several Blt/Unlock calls per one displayed frame.
// At most four samples per API, bounded without background threads.
pub(crate) fn sample_render_calls(path:&str,count:&AtomicUsize,start:&OnceLock<Instant>,
    window:&OnceLock<Mutex<(usize,Instant)>>) {
    if !sampling_enabled(&cfg()){return;}
    let n=count.fetch_add(1,Ordering::Relaxed)+1;
    let first=start.get_or_init(Instant::now);
    if [10,60,300,1200].contains(&n) {
        let elapsed=first.elapsed().as_millis();
        if elapsed>0 {
            let rate=(n.saturating_sub(1) as u128 * 1000) / elapsed;
            log(&format!("RENDER-SAMPLE: method={path} calls={n} elapsed_ms={elapsed} average_calls_per_s={rate} (NOT measured display FPS)"));
        }
    }
    // Also sample short intervals: full-run averages in the early Wine
    // log hid substantial differences between menus and actual gameplay.
    // Bounded to ten per API, not one log line per video frame.
    if n % 120==0 && n<=1200 {
        let guard=window.get_or_init(||Mutex::new((0,*first)));
        if let Ok(mut previous)=guard.lock() {
            let now=Instant::now();
            let ms=now.duration_since(previous.1).as_millis();
            let interval=n-previous.0;
            if ms>0 {
                let rate=interval as u128*1000/ms;
                log(&format!("RENDER-WINDOW: method={path} calls={interval} elapsed_ms={ms} calls_per_s={rate} (API rate, NOT verified display FPS)"));
            }
            *previous=(n,now);
        }
    }
}


#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn api_rate_sampling_is_explicit_opt_in_even_when_log_is_on(){
        let mut c=crate::config::Config::default();
        assert!(!sampling_enabled(&c)); // default Log=0 FrameTimeTrace=0
        c.log_enabled=true;
        assert!(!sampling_enabled(&c)); // ordinary gameplay diagnostic
        c.frame_time_trace=true;
        assert!(sampling_enabled(&c)); // explicit deep diagnostic
        c.log_enabled=false;
        assert!(!sampling_enabled(&c)); // cannot leak rate samples with Log=0
    }
}
