//! Passive bookkeeping for successful Direct3D3 scene calls grouped by
//! successful primary-surface DirectDraw Flip calls.
//!
//! This is diagnostic-only: a scene or a Flip call is NOT necessarily
//! a displayed frame. Timing measures completed API calls, not refresh
//! scans, unique images, or the game's simulation rate.

use std::time::Instant;

pub const FLIPS_PER_SAMPLE:usize=120;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct SceneGroup {
    pub generation:u64,
    pub flips:usize,
    pub successful_begins:usize,
    pub successful_ends:usize,
    /// Number of completed-Flip-to-completed-Flip intervals measured.
    /// 120 successful calls contain 119 such intervals.
    pub observed_flip_intervals:usize,
    /// Monotonic time from first to last successful primary Flip of the
    /// group. Does not include time before its first successful Flip.
    pub first_to_last_flip_ms:u128,
}

impl SceneGroup {
    /// An API completion rate, emphatically NOT verified display FPS.
    /// None if all 120 completions fall in the same millisecond.
    pub fn successful_primary_flip_calls_per_s(&self)->Option<f64> {
        (self.first_to_last_flip_ms>0 && self.observed_flip_intervals>0)
            .then(||self.observed_flip_intervals as f64 * 1000.0 /
                self.first_to_last_flip_ms as f64)
    }
}

#[derive(Debug,Clone,Copy)]
pub struct SceneCorrelation {
    generation:u64,
    flips:usize,
    begin_baseline:usize,
    end_baseline:usize,
    first_flip_at:Option<Instant>,
    initialized:bool,
}

impl SceneCorrelation {
    pub const fn new()->Self {
        Self { generation:0, flips:0, begin_baseline:0,
               end_baseline:0, first_flip_at:None, initialized:false }
    }

    /// Only on successful DirectDraw SetDisplayMode: discard the former
    /// render chain, incomplete group, and timing evidence. This also
    /// applies when the next mode has identical dimensions.
    pub fn reset(&mut self,generation:u64,begins:usize,ends:usize) {
        self.generation=generation;
        self.flips=0;
        self.begin_baseline=begins;
        self.end_baseline=ends;
        self.first_flip_at=None;
        self.initialized=true;
    }

    /// Observe one successful primary-surface Flip (after the real COM
    /// call returns) with cumulative successful scene counters.
    pub fn observe_flip(&mut self,begins:usize,ends:usize)->Option<SceneGroup>{
        self.observe_flip_at(begins,ends,Instant::now())
    }

    // Explicit monotonic timestamp lets unit tests verify the same
    // 119 intervals without waiting or relying on wall-clock jitter.
    fn observe_flip_at(&mut self,begins:usize,ends:usize,now:Instant)
        ->Option<SceneGroup>
    {
        if !self.initialized{return None;}
        if self.flips==0 {self.first_flip_at=Some(now);}
        self.flips+=1;
        if self.flips<FLIPS_PER_SAMPLE{return None;}
        let first_to_last_flip_ms=self.first_flip_at
            .and_then(|start|now.checked_duration_since(start))
            .map(|elapsed|elapsed.as_millis()).unwrap_or(0);
        let sample=SceneGroup {
            generation:self.generation,
            flips:self.flips,
            successful_begins:begins.saturating_sub(self.begin_baseline),
            successful_ends:ends.saturating_sub(self.end_baseline),
            observed_flip_intervals:self.flips.saturating_sub(1),
            first_to_last_flip_ms,
        };
        self.flips=0;
        self.begin_baseline=begins;
        self.end_baseline=ends;
        self.first_flip_at=None;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ms(start:Instant,millis:u64)->Instant {
        start+Duration::from_millis(millis)
    }

    #[test]
    fn successful_scene_pairs_share_a_completed_flip_group_and_time_span(){
        let mut c=SceneCorrelation::new();
        let start=Instant::now();
        c.reset(1,0,0);
        for i in 1..120 {
            assert!(c.observe_flip_at(i*4,i*4,ms(start,(i as u64-1)*17)).is_none());
        }
        let sample=c.observe_flip_at(480,480,ms(start,119*17)).unwrap();
        assert_eq!((sample.generation,sample.flips),(1,120));
        assert_eq!((sample.successful_begins,sample.successful_ends),(480,480));
        assert_eq!(sample.observed_flip_intervals,119);
        assert_eq!(sample.first_to_last_flip_ms,2023);
        let rate=sample.successful_primary_flip_calls_per_s().unwrap();
        assert!((rate-(119.0*1000.0/2023.0)).abs()<0.00001);
        assert!(c.observe_flip_at(484,484,ms(start,2030)).is_none());
    }

    #[test]
    fn independent_groups_reset_time_but_keep_scene_counter_baselines(){
        let mut c=SceneCorrelation::new();
        let start=Instant::now();
        c.reset(2,30,28);
        for i in 0..119 {
            assert!(c.observe_flip_at(65,63,ms(start,i*10)).is_none());
        }
        let first=c.observe_flip_at(73,71,ms(start,1190)).unwrap();
        assert_eq!((first.successful_begins,first.successful_ends),(43,43));
        assert_eq!(first.first_to_last_flip_ms,1190);
        for i in 0..119 {
            assert!(c.observe_flip_at(73,71,ms(start,2000+i*20)).is_none());
        }
        let second=c.observe_flip_at(74,72,ms(start,2000+119*20)).unwrap();
        assert_eq!((second.successful_begins,second.successful_ends),(1,1));
        assert_eq!(second.first_to_last_flip_ms,2380);
        assert_eq!(second.observed_flip_intervals,119);
    }

    #[test]
    fn changing_mode_discards_partial_groups_and_timing_even_same_resolution(){
        let mut c=SceneCorrelation::new();
        let start=Instant::now();
        c.reset(1,50,40);
        for i in 0..90 {
            assert!(c.observe_flip_at(100,100,ms(start,i*9)).is_none());
        }
        c.reset(2,100,100);
        for i in 0..119 {
            assert!(c.observe_flip_at(500,500,ms(start,5000+i*11)).is_none());
        }
        let after=c.observe_flip_at(500,500,ms(start,5000+119*11)).unwrap();
        assert_eq!(after.generation,2);
        assert_eq!(after.successful_begins,400);
        assert_eq!(after.first_to_last_flip_ms,1309);
        c.reset(3,500,500); // repeated mode is still a new generation
        assert!(c.observe_flip_at(501,501,ms(start,9000)).is_none());
    }

    #[test]
    fn unknown_mode_and_stale_scene_counters_cannot_create_fake_progress(){
        let mut c=SceneCorrelation::new();
        let start=Instant::now();
        for i in 0..150 {
            assert!(c.observe_flip_at(0,0,ms(start,i)).is_none());
        }
        c.reset(4,1000,1000);
        for i in 0..119 {
            assert!(c.observe_flip_at(0,0,ms(start,i)).is_none());
        }
        let sample=c.observe_flip_at(0,0,ms(start,119)).unwrap();
        assert_eq!((sample.successful_begins,sample.successful_ends),(0,0));
        assert_eq!(sample.first_to_last_flip_ms,119);
    }

    #[test]
    fn zero_millisecond_interval_never_invents_an_infinite_fps_rate(){
        let mut c=SceneCorrelation::new();
        let now=Instant::now();
        c.reset(1,0,0);
        for _ in 0..119 {
            assert!(c.observe_flip_at(0,0,now).is_none());
        }
        let sample=c.observe_flip_at(0,0,now).unwrap();
        assert_eq!(sample.first_to_last_flip_ms,0);
        assert_eq!(sample.successful_primary_flip_calls_per_s(),None);
    }

    #[test]
    fn no_failed_flip_can_be_included_without_explicit_observation(){
        // The caller feeds this module only after a successful primary
        // Flip; unlike the separately logged attempted-Flip counter,
        // failed operations are intentionally absent.
        let mut c=SceneCorrelation::new();
        let start=Instant::now();
        c.reset(5,0,0);
        for i in 0..60 {
            assert!(c.observe_flip_at(i,i,ms(start,i as u64*15)).is_none());
        }
        // 60 failed or non-primary flips are not fed to this correlator.
        for i in 60..119 {
            assert!(c.observe_flip_at(i,i,ms(start,(i as u64+60)*15)).is_none());
        }
        let sample=c.observe_flip_at(120,120,ms(start,179*15)).unwrap();
        assert_eq!(sample.flips,120);
        assert_eq!(sample.observed_flip_intervals,119);
        assert_eq!(sample.first_to_last_flip_ms,2685);
    }
}
