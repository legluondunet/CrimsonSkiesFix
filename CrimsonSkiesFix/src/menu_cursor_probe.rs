//! Passive GameOS menu mouse-poll cadence probe, shared by Wine and Windows.
//! Counts REAL DirectInput GetDeviceState outcomes. It never intercepts input
//! independently, modifies DIMOUSESTATE, generates events or alters rendering.
//! Only enabled with existing Log=1 AND FrameTimeTrace=1 and an authenticated
//! 800x600x16 MainMenu/Preferences graph; normal play has no extra work.
//! One line per 120 successful virtual Flips, never one line per mouse poll.
//! Mouse motion timestamps are times of polling, NOT hardware event times.
use std::{
    ffi::c_void, ptr,
    sync::{
        OnceLock,
        atomic::{AtomicU64,Ordering},
    },
    time::{Duration,Instant},
};

static POLLS:AtomicU64=AtomicU64::new(0);
static SUCCESS:AtomicU64=AtomicU64::new(0);
static MOTION:AtomicU64=AtomicU64::new(0);
static NO_MOTION:AtomicU64=AtomicU64::new(0);
static FAILED:AtomicU64=AtomicU64::new(0);
static OTHER_FORMAT:AtomicU64=AtomicU64::new(0);
static ABS_DX:AtomicU64=AtomicU64::new(0);
static ABS_DY:AtomicU64=AtomicU64::new(0);
static LARGEST_DX:AtomicU64=AtomicU64::new(0);
static LARGEST_DY:AtomicU64=AtomicU64::new(0);
// Unlike motion-only gaps, these count EVERY real polling interval,
// including stationary cursor polls, to locate pauses in GetDeviceState.
// Polling cadence is not the raw hardware event arrival cadence.
static LAST_POLL_US:AtomicU64=AtomicU64::new(0);
static POLL_GAPS:AtomicU64=AtomicU64::new(0);
static POLL_GAPS_OVER_50MS:AtomicU64=AtomicU64::new(0);
static POLL_GAPS_OVER_100MS:AtomicU64=AtomicU64::new(0);
static POLL_GAPS_OVER_180MS:AtomicU64=AtomicU64::new(0);
static POLL_GAP_MAX_US:AtomicU64=AtomicU64::new(0);
static MOTION_AFTER_POLL_GAP_180MS:AtomicU64=AtomicU64::new(0);
static LAST_MOTION_US:AtomicU64=AtomicU64::new(0);
static ACTIVE_GAPS:AtomicU64=AtomicU64::new(0);
static GAPS_OVER_33MS:AtomicU64=AtomicU64::new(0);
static GAPS_OVER_50MS:AtomicU64=AtomicU64::new(0);
static LARGEST_ACTIVE_GAP_US:AtomicU64=AtomicU64::new(0);
// Time spent *inside* the original DirectInput GetDeviceState itself.
// Distinguishes a slow native/Wine API from game work between calls.
static STATE_CALL_TIMED:AtomicU64=AtomicU64::new(0);
static STATE_CALL_TOTAL_US:AtomicU64=AtomicU64::new(0);
static STATE_CALL_OVER_16MS:AtomicU64=AtomicU64::new(0);
static STATE_CALL_OVER_50MS:AtomicU64=AtomicU64::new(0);
static STATE_CALL_OVER_180MS:AtomicU64=AtomicU64::new(0);
static STATE_CALL_MAX_US:AtomicU64=AtomicU64::new(0);
static CLOCK_ORIGIN:OnceLock<Instant>=OnceLock::new();

fn active()->bool{crate::menu_api_timing::active_menu_trace()}
pub(crate) fn timing_enabled()->bool{active()}

/// Called AFTER the real COM method completes and BEFORE returning its
/// unchanged HRESULT/data to the game. No synchronous logging here.
pub(crate) unsafe fn observe_get_device_state(
    hr:i32,size:u32,data:*mut c_void,call_duration:Option<Duration>
){
    if !active(){return;}
    if let Some(duration)=call_duration{
        let us=duration.as_micros().min(u128::from(u64::MAX)) as u64;
        STATE_CALL_TIMED.fetch_add(1,Ordering::Relaxed);
        STATE_CALL_TOTAL_US.fetch_add(us,Ordering::Relaxed);
        STATE_CALL_OVER_16MS.fetch_add(u64::from(us>16_667),Ordering::Relaxed);
        STATE_CALL_OVER_50MS.fetch_add(u64::from(us>50_000),Ordering::Relaxed);
        STATE_CALL_OVER_180MS.fetch_add(u64::from(us>180_000),Ordering::Relaxed);
        STATE_CALL_MAX_US.fetch_max(us,Ordering::Relaxed);
    }
    POLLS.fetch_add(1,Ordering::Relaxed);
    let now=CLOCK_ORIGIN.get_or_init(Instant::now).elapsed().as_micros()
        .min(u128::from(u64::MAX)-1) as u64 + 1;
    let old_poll=LAST_POLL_US.swap(now,Ordering::Relaxed);
    let mut long_poll_gap=false;
    if old_poll>0 && now>=old_poll{
        let gap=now-old_poll;
        // Exclude full application pauses (>500ms), but never assume they
        // were caused by user inactivity: they are simply unclassified.
        if gap<=500_000{
            POLL_GAPS.fetch_add(1,Ordering::Relaxed);
            POLL_GAPS_OVER_50MS.fetch_add(u64::from(gap>50_000),Ordering::Relaxed);
            POLL_GAPS_OVER_100MS.fetch_add(u64::from(gap>100_000),Ordering::Relaxed);
            POLL_GAPS_OVER_180MS.fetch_add(u64::from(gap>180_000),Ordering::Relaxed);
            POLL_GAP_MAX_US.fetch_max(gap,Ordering::Relaxed);
            long_poll_gap=gap>180_000;
        }
    }
    if hr<0{
        FAILED.fetch_add(1,Ordering::Relaxed);
        return;
    }
    if size<16 || data.is_null(){
        OTHER_FORMAT.fetch_add(1,Ordering::Relaxed);
        return;
    }
    SUCCESS.fetch_add(1,Ordering::Relaxed);
    let bytes=data as *const u8;
    let dx=ptr::read_unaligned(bytes.cast::<i32>());
    let dy=ptr::read_unaligned(bytes.add(4).cast::<i32>());
    if dx==0 && dy==0{
        NO_MOTION.fetch_add(1,Ordering::Relaxed);
        return;
    }
    MOTION.fetch_add(1,Ordering::Relaxed);
    if long_poll_gap{
        MOTION_AFTER_POLL_GAP_180MS.fetch_add(1,Ordering::Relaxed);
    }
    let ax=u64::from(dx.unsigned_abs());
    let ay=u64::from(dy.unsigned_abs());
    ABS_DX.fetch_add(ax,Ordering::Relaxed);
    ABS_DY.fetch_add(ay,Ordering::Relaxed);
    LARGEST_DX.fetch_max(ax,Ordering::Relaxed);
    LARGEST_DY.fetch_max(ay,Ordering::Relaxed);
    let old=LAST_MOTION_US.swap(now,Ordering::Relaxed);
    if old>0 && now>=old{
        let gap=now-old;
        // Exclude user pauses / stationary periods from continuity counters.
        // A gap within 250ms remains only a candidate for irregular polling.
        if gap<=250_000{
            ACTIVE_GAPS.fetch_add(1,Ordering::Relaxed);
            GAPS_OVER_33MS.fetch_add(u64::from(gap>33_333),Ordering::Relaxed);
            GAPS_OVER_50MS.fetch_add(u64::from(gap>50_000),Ordering::Relaxed);
            LARGEST_ACTIVE_GAP_US.fetch_max(gap,Ordering::Relaxed);
        }
    }
}

#[derive(Clone,Copy,Default)]
pub(crate) struct Snapshot{
    polls:u64,success:u64,motion:u64,no_motion:u64,failed:u64,other_format:u64,
    abs_dx:u64,abs_dy:u64,active_gaps:u64,gaps_over_33ms:u64,gaps_over_50ms:u64,
    largest_dx:u64,largest_dy:u64,largest_gap_us:u64,
    poll_gaps:u64,poll_gaps_over_50ms:u64,poll_gaps_over_100ms:u64,
    poll_gaps_over_180ms:u64,motion_after_poll_gap_180ms:u64,
    poll_gap_max_us:u64,
    state_call_timed:u64,state_call_total_us:u64,state_call_over_16ms:u64,
    state_call_over_50ms:u64,state_call_over_180ms:u64,state_call_max_us:u64,
}
pub(crate) fn snapshot()->Snapshot{
    Snapshot{
        polls:POLLS.load(Ordering::Relaxed),
        success:SUCCESS.load(Ordering::Relaxed),
        motion:MOTION.load(Ordering::Relaxed),
        no_motion:NO_MOTION.load(Ordering::Relaxed),
        failed:FAILED.load(Ordering::Relaxed),
        other_format:OTHER_FORMAT.load(Ordering::Relaxed),
        abs_dx:ABS_DX.load(Ordering::Relaxed),
        abs_dy:ABS_DY.load(Ordering::Relaxed),
        active_gaps:ACTIVE_GAPS.load(Ordering::Relaxed),
        gaps_over_33ms:GAPS_OVER_33MS.load(Ordering::Relaxed),
        gaps_over_50ms:GAPS_OVER_50MS.load(Ordering::Relaxed),
        largest_dx:LARGEST_DX.load(Ordering::Relaxed),
        largest_dy:LARGEST_DY.load(Ordering::Relaxed),
        largest_gap_us:LARGEST_ACTIVE_GAP_US.load(Ordering::Relaxed),
        poll_gaps:POLL_GAPS.load(Ordering::Relaxed),
        poll_gaps_over_50ms:POLL_GAPS_OVER_50MS.load(Ordering::Relaxed),
        poll_gaps_over_100ms:POLL_GAPS_OVER_100MS.load(Ordering::Relaxed),
        poll_gaps_over_180ms:POLL_GAPS_OVER_180MS.load(Ordering::Relaxed),
        motion_after_poll_gap_180ms:
            MOTION_AFTER_POLL_GAP_180MS.load(Ordering::Relaxed),
        poll_gap_max_us:POLL_GAP_MAX_US.load(Ordering::Relaxed),
        state_call_timed:STATE_CALL_TIMED.load(Ordering::Relaxed),
        state_call_total_us:STATE_CALL_TOTAL_US.load(Ordering::Relaxed),
        state_call_over_16ms:STATE_CALL_OVER_16MS.load(Ordering::Relaxed),
        state_call_over_50ms:STATE_CALL_OVER_50MS.load(Ordering::Relaxed),
        state_call_over_180ms:STATE_CALL_OVER_180MS.load(Ordering::Relaxed),
        state_call_max_us:STATE_CALL_MAX_US.load(Ordering::Relaxed),
    }
}
pub(crate) fn summary(before:Snapshot,after:Snapshot)->String{
    let d=|a:u64,b:u64|b.saturating_sub(a);
    format!(
        "MENU-CURSOR-INPUT: window=approximately_120_Flips dinput_polls={} successful={} motion_polls={} stationary_polls={} failed={} other_format={} abs_dx={} abs_dy={} active_motion_gaps={} gaps_over_33ms={} gaps_over_50ms={} lifetime_max_dx={} lifetime_max_dy={} lifetime_max_active_gap_ms={:.3} poll_gaps={} poll_gaps_over_50ms={} poll_gaps_over_100ms={} poll_gaps_over_180ms={} motion_after_poll_gap_180ms={} lifetime_max_poll_gap_ms={:.3} dinput_call_timed={} dinput_call_total_ms={:.3} dinput_call_over_16ms={} dinput_call_over_50ms={} dinput_call_over_180ms={} lifetime_max_dinput_call_ms={:.3} [real mouse GetDeviceState results, not raw hardware event timestamps; motion gap excludes >250ms pauses but may include intentional stops; GOS cursor sampled separately at Flip entry; zero polls may mean unobserved input route]",
        d(before.polls,after.polls),
        d(before.success,after.success),
        d(before.motion,after.motion),
        d(before.no_motion,after.no_motion),
        d(before.failed,after.failed),
        d(before.other_format,after.other_format),
        d(before.abs_dx,after.abs_dx),
        d(before.abs_dy,after.abs_dy),
        d(before.active_gaps,after.active_gaps),
        d(before.gaps_over_33ms,after.gaps_over_33ms),
        d(before.gaps_over_50ms,after.gaps_over_50ms),
        after.largest_dx,after.largest_dy,
        after.largest_gap_us as f64/1000.0,
        d(before.poll_gaps,after.poll_gaps),
        d(before.poll_gaps_over_50ms,after.poll_gaps_over_50ms),
        d(before.poll_gaps_over_100ms,after.poll_gaps_over_100ms),
        d(before.poll_gaps_over_180ms,after.poll_gaps_over_180ms),
        d(before.motion_after_poll_gap_180ms,after.motion_after_poll_gap_180ms),
        after.poll_gap_max_us as f64/1000.0,
        d(before.state_call_timed,after.state_call_timed),
        d(before.state_call_total_us,after.state_call_total_us) as f64/1000.0,
        d(before.state_call_over_16ms,after.state_call_over_16ms),
        d(before.state_call_over_50ms,after.state_call_over_50ms),
        d(before.state_call_over_180ms,after.state_call_over_180ms),
        after.state_call_max_us as f64/1000.0,
    )
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn counts_do_not_underflow_when_epoch_changes(){
        let before=Snapshot{polls:8,motion:6,active_gaps:2,..Snapshot::default()};
        let after=Snapshot{polls:3,motion:1,active_gaps:0,..Snapshot::default()};
        let text=summary(before,after);
        assert!(text.contains("dinput_polls=0"));
        assert!(text.contains("motion_polls=0"));
        assert!(text.contains("active_motion_gaps=0"));
        assert!(text.contains("poll_gaps_over_180ms=0"));
        assert!(text.contains("dinput_call_over_180ms=0"));
    }
    #[test]
    fn diagnostic_summary_identifies_poll_timestamps_not_hardware_events(){
        let text=summary(Snapshot::default(),Snapshot::default());
        assert!(text.contains("gaps_over_50ms=0"));
        assert!(text.contains("poll_gaps_over_50ms=0"));
        assert!(text.contains("dinput_call_timed=0"));
        assert!(text.contains("not raw hardware event timestamps"));
    }
}
