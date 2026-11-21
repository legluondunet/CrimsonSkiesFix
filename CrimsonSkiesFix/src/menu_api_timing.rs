//! Opt-in, menu-only wall-time measurements of original Direct3D/DirectDraw
//! COM calls. Never changes arguments, return values, pacing or input.
//! Enabled by the existing [Debug] Log=1 + FrameTimeTrace=1 pair.
//! Counters are cumulative, read once per 120 virtual-Flip completions.
//! Each call is measured around the *original* COM function only.
//! The virtual Flip presenter is explicitly excluded on the current thread.
//! Concurrent threads can overlap; totals are NOT additive wall-clock time.
use std::{
    cell::Cell,
    sync::atomic::{AtomicBool,AtomicU64,Ordering},
    time::{Duration,Instant},
};
use crate::{cfg,menu_scene,menu_gos_event};

#[derive(Clone,Copy)]
pub(crate) enum Kind{BeginScene,EndScene,Blt,BltFast,Unlock}
impl Kind{
    const fn index(self)->usize{
        match self{
            Self::BeginScene=>0,Self::EndScene=>1,Self::Blt=>2,
            Self::BltFast=>3,Self::Unlock=>4
        }
    }
}
const METHODS:usize=5;
const SLOW_16_US:u64=16_667;
const SLOW_33_US:u64=33_333;
struct Bucket{
    calls:AtomicU64,
    micros:AtomicU64,
    slow16:AtomicU64,
    slow33:AtomicU64,
    max_us:AtomicU64,
}
impl Bucket{
    const fn new()->Self{
        Self{
            calls:AtomicU64::new(0),
            micros:AtomicU64::new(0),
            slow16:AtomicU64::new(0),
            slow33:AtomicU64::new(0),
            max_us:AtomicU64::new(0),
        }
    }
    fn record(&self,elapsed:Duration){
        let us=elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.micros.fetch_add(us,Ordering::Relaxed);
        self.slow16.fetch_add(u64::from(us>=SLOW_16_US),Ordering::Relaxed);
        self.slow33.fetch_add(u64::from(us>=SLOW_33_US),Ordering::Relaxed);
        self.max_us.fetch_max(us,Ordering::Relaxed);
        // Publish count last. A snapshot taken while another thread is
        // finishing a call may be off by one: windows are approximate.
        self.calls.fetch_add(1,Ordering::Relaxed);
    }
    fn snapshot(&self)->Counts{
        Counts{
            calls:self.calls.load(Ordering::Relaxed),
            micros:self.micros.load(Ordering::Relaxed),
            slow16:self.slow16.load(Ordering::Relaxed),
            slow33:self.slow33.load(Ordering::Relaxed),
            max_us:self.max_us.load(Ordering::Relaxed),
        }
    }
}
static BUCKETS:[Bucket;METHODS]=[
    Bucket::new(),Bucket::new(),Bucket::new(),
    Bucket::new(),Bucket::new(),
];
static GOS_MENU_GRAPH:AtomicBool=AtomicBool::new(false);
thread_local!{static IN_OUR_VIRTUAL_FLIP:Cell<u32>=const{Cell::new(0)};}

fn trace_enabled()->bool{
    let c=cfg();
    c.log_enabled && c.frame_time_trace
}
fn menu_enabled()->bool{
    trace_enabled() &&
    GOS_MENU_GRAPH.load(Ordering::Acquire) &&
    (menu_scene::main_menu_active() || menu_scene::preferences_active() ||
     menu_gos_event::preferences_transaction_active())
}
pub(crate) fn set_graph_mode(w:u32,h:u32,bpp:u32){
    GOS_MENU_GRAPH.store((w,h,bpp)==(800,600,16),Ordering::Release);
}
pub(crate) fn clear_graph_mode(){
    GOS_MENU_GRAPH.store(false,Ordering::Release);
}
pub(crate) fn start()->Option<Instant>{
    if !menu_enabled(){return None;}
    let nested=IN_OUR_VIRTUAL_FLIP.with(|depth|depth.get()!=0);
    (!nested).then(Instant::now)
}
pub(crate) fn finish(kind:Kind,start:Option<Instant>){
    if let Some(start)=start{
        BUCKETS[kind.index()].record(start.elapsed());
    }
}
pub(crate) struct VirtualFlipExclusion{active:bool}
pub(crate) fn exclude_virtual_flip()->VirtualFlipExclusion{
    let active=trace_enabled();
    if active{
        IN_OUR_VIRTUAL_FLIP.with(|depth|
            depth.set(depth.get().saturating_add(1)));
    }
    VirtualFlipExclusion{active}
}
impl Drop for VirtualFlipExclusion{
    fn drop(&mut self){
        if self.active{
            IN_OUR_VIRTUAL_FLIP.with(|depth|
                depth.set(depth.get().saturating_sub(1)));
        }
    }
}

#[derive(Clone,Copy,Debug,Default)]
struct Counts{
    calls:u64,
    micros:u64,
    slow16:u64,
    slow33:u64,
    max_us:u64, // process-lifetime maximum, not window maximum
}
impl Counts{
    fn since(self,previous:Self)->Self{
        Self{
            calls:self.calls.saturating_sub(previous.calls),
            micros:self.micros.saturating_sub(previous.micros),
            slow16:self.slow16.saturating_sub(previous.slow16),
            slow33:self.slow33.saturating_sub(previous.slow33),
            max_us:self.max_us,
        }
    }
}
#[derive(Clone,Copy,Debug,Default)]
pub(crate) struct Snapshot{
    entries:[Counts;METHODS],
}
pub(crate) fn snapshot()->Snapshot{
    Snapshot{
        entries:std::array::from_fn(|i|BUCKETS[i].snapshot()),
    }
}
fn entry(kind:Kind,previous:Snapshot,current:Snapshot)->String{
    let d=current.entries[kind.index()].since(previous.entries[kind.index()]);
    format!(
        "calls={} total_ms={:.3} avg_ms={:.3} slow16={} slow33={} lifetime_max_ms={:.3}",
        d.calls,d.micros as f64/1000.,
        if d.calls==0{0.0}else{d.micros as f64/(d.calls as f64*1000.)},
        d.slow16,d.slow33,d.max_us as f64/1000.
    )
}
pub(crate) fn summary(previous:Snapshot,current:Snapshot)->String{
    // Group each measured function without summing across them. The
    // totals represent (possibly concurrent) instrumented API calls
    // between two snapshots, not a partition of 120 image intervals.
    format!(
        "MENU-API-TIMING: approximately_120_Flips=true          BeginScene[{}] EndScene[{}] Blt[{}] BltFast[{}] Unlock[{}]          [wall-time only around original COM calls; this thread's virtual Flip excluded; overlapping and out-of-scope GOS work unmeasured; max is lifetime, not interval]",
        entry(Kind::BeginScene,previous,current),
        entry(Kind::EndScene,previous,current),
        entry(Kind::Blt,previous,current),
        entry(Kind::BltFast,previous,current),
        entry(Kind::Unlock,previous,current),
    )
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn elapsed_count_delta_never_wraps(){
        let old=Counts{calls:10,micros:12_000,slow16:2,slow33:1,max_us:6000};
        let now=Counts{calls:13,micros:20_000,slow16:3,slow33:1,max_us:7000};
        let d=now.since(old);
        assert_eq!(d.calls,3);
        assert_eq!(d.micros,8_000);
        assert_eq!(d.slow16,1);
        assert_eq!(d.slow33,0);
        assert_eq!(d.max_us,7000);
        assert_eq!(old.since(now).calls,0);
    }
    #[test]
    fn snapshot_summary_is_bounded_and_no_calls_are_zero_averages(){
        let summary=summary(Snapshot::default(),Snapshot::default());
        assert!(summary.contains("BeginScene[calls=0 total_ms=0.000 avg_ms=0.000"));
        assert!(summary.contains("Unlock[calls=0"));
        assert_eq!(summary.matches("total_ms=").count(),METHODS);
    }
    #[test]
    fn virtual_flip_suppression_is_reentrant_and_restored(){
        assert_eq!(IN_OUR_VIRTUAL_FLIP.with(Cell::get),0);
        {
            let _a=exclude_virtual_flip();
            // Tests must not rely on debug flag being enabled: if it is
            // disabled this scope is deliberately a no-op.
            let expected=u32::from(trace_enabled());
            assert_eq!(IN_OUR_VIRTUAL_FLIP.with(Cell::get),expected);
            let _b=exclude_virtual_flip();
            assert_eq!(IN_OUR_VIRTUAL_FLIP.with(Cell::get),expected*2);
        }
        assert_eq!(IN_OUR_VIRTUAL_FLIP.with(Cell::get),0);
    }
}
