//! Pure bookkeeping for successful Direct3D3 scene calls that occur
//! between a fixed group of successful primary-surface DirectDraw Flips.
//!
//! This is diagnostic-only. A scene call is NOT necessarily a displayed
//! frame, and a Flip can occur without any 3D scene (e.g. in menus).
//! The actual counters are kept by the COM instrumentation in lib.rs.

pub const FLIPS_PER_SAMPLE:usize=120;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct SceneGroup {
    pub generation:u64,
    pub flips:usize,
    pub successful_begins:usize,
    pub successful_ends:usize,
}

#[derive(Debug,Clone,Copy)]
pub struct SceneCorrelation {
    generation:u64,
    flips:usize,
    begin_baseline:usize,
    end_baseline:usize,
    initialized:bool,
}

impl SceneCorrelation {
    pub const fn new()->Self {
        Self { generation:0, flips:0, begin_baseline:0,
               end_baseline:0, initialized:false }
    }

    /// Only on successful DirectDraw SetDisplayMode: forget all prior
    /// render evidence, even when the new mode has the same dimensions.
    pub fn reset(&mut self,generation:u64,begins:usize,ends:usize) {
        self.generation=generation;
        self.flips=0;
        self.begin_baseline=begins;
        self.end_baseline=ends;
        self.initialized=true;
    }

    /// Observe a single successful primary-surface Flip and the
    /// cumulative number of successful 3D BeginScene/EndScene calls.
    /// No approximate wall-clock windows or arbitrary FPS assumptions.
    pub fn observe_flip(&mut self,begins:usize,ends:usize)->Option<SceneGroup>{
        if !self.initialized{return None;}
        self.flips+=1;
        if self.flips<FLIPS_PER_SAMPLE{return None;}
        let sample=SceneGroup {
            generation:self.generation,
            flips:self.flips,
            successful_begins:begins.saturating_sub(self.begin_baseline),
            successful_ends:ends.saturating_sub(self.end_baseline),
        };
        self.flips=0;
        self.begin_baseline=begins;
        self.end_baseline=ends;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precisely_four_3d_scene_pairs_per_successful_primary_flip(){
        let mut c=SceneCorrelation::new();
        c.reset(1,0,0);
        for i in 1..120 {
            assert_eq!(c.observe_flip(i*4,i*4),None);
        }
        assert_eq!(c.observe_flip(480,480),Some(SceneGroup{
            generation:1,flips:120,successful_begins:480,successful_ends:480
        }));
        assert_eq!(c.observe_flip(484,484),None);
    }
    #[test]
    fn independent_group_can_show_noninteger_scene_ratio(){
        let mut c=SceneCorrelation::new();
        c.reset(1,30,28);
        for _ in 0..119 {assert_eq!(c.observe_flip(65,63),None);}
        assert_eq!(c.observe_flip(73,71),Some(SceneGroup{
            generation:1,flips:120,successful_begins:43,successful_ends:43
        }));
        for _ in 0..119 {assert_eq!(c.observe_flip(73,71),None);}
        assert_eq!(c.observe_flip(74,72),Some(SceneGroup{
            generation:1,flips:120,successful_begins:1,successful_ends:1
        }));
    }
    #[test]
    fn changes_of_mode_reset_partial_window_and_prior_device_counters(){
        let mut c=SceneCorrelation::new();
        c.reset(1,50,40);
        for _ in 0..90 {assert!(c.observe_flip(100,100).is_none());}
        c.reset(2,100,100);
        for _ in 0..119 {assert!(c.observe_flip(500,500).is_none());}
        assert_eq!(c.observe_flip(500,500),Some(SceneGroup{
            generation:2,flips:120,successful_begins:400,successful_ends:400
        }));
        c.reset(3,500,500);
        assert_eq!(c.observe_flip(501,501),None);
    }
    #[test]
    fn requires_known_mode_and_ignores_counter_wrap_or_stale_values(){
        let mut c=SceneCorrelation::new();
        for _ in 0..150 {assert!(c.observe_flip(0,0).is_none());}
        c.reset(4,1000,1000);
        for _ in 0..119 {c.observe_flip(0,0);}
        assert_eq!(c.observe_flip(0,0).unwrap().successful_begins,0);
    }
}
