//! Passive, opt-in DirectDraw surface-method audit grouped by 120
//! successful primary Flip calls. No COM calls, memory reads, surface
//! ownership changes, thread spawning, or background execution here.
//! The caller classifies the destination using the real GetCaps result.
//! All output rates are API-call counts, never displayed frames.

pub const FLIPS_PER_AUDIT:usize=120;
pub const ROLES:usize=5;
pub const METHODS:usize=13;

#[repr(usize)]
#[derive(Copy,Clone,Debug,PartialEq,Eq)]
pub enum Role {Primary,Backbuffer,Depth,Texture,Other}
impl Role {
    pub const fn label(self)->&'static str {
        match self {Self::Primary=>"primary",Self::Backbuffer=>"backbuffer",
                    Self::Depth=>"depth",Self::Texture=>"texture",Self::Other=>"other"}
    }
}
pub fn classify(caps:Option<u32>)->Role {
    const PRIMARY:u32=0x0000_0200;
    const BACKBUFFER:u32=0x0000_0004;
    const DEPTH:u32=0x0002_0000;
    const TEXTURE:u32=0x0000_1000;
    let flags=caps.unwrap_or(0);
    if flags&PRIMARY!=0 {Role::Primary}
    else if flags&BACKBUFFER!=0 {Role::Backbuffer}
    else if flags&DEPTH!=0 {Role::Depth}
    else if flags&TEXTURE!=0 {Role::Texture}
    else {Role::Other}
}

#[repr(usize)]
#[derive(Copy,Clone,Debug,PartialEq,Eq)]
pub enum Method {
    AddAttachedSurface,
    DeleteAttachedSurface,
    GetDC,
    IsLost,
    Lock,
    ReleaseDC,
    Restore,
    SetClipper,
    GetDDInterface,
    Unlock,
    Blt,
    BltFast,
    GetAttachedSurface,
}
impl Method {
    pub const fn label(self)->&'static str {
        match self {
            Self::AddAttachedSurface=>"AddAttachedSurface",
            Self::DeleteAttachedSurface=>"DeleteAttachedSurface",
            Self::GetDC=>"GetDC",
            Self::IsLost=>"IsLost",
            Self::Lock=>"Lock",
            Self::ReleaseDC=>"ReleaseDC",
            Self::Restore=>"Restore",
            Self::SetClipper=>"SetClipper",
            Self::GetDDInterface=>"GetDDInterface",
            Self::Unlock=>"Unlock",
            Self::Blt=>"Blt",
            Self::BltFast=>"BltFast",
            Self::GetAttachedSurface=>"GetAttachedSurface",
        }
    }
}

#[derive(Copy,Clone,Debug,Default,PartialEq,Eq)]
pub struct Count {pub ok:u64,pub failed:u64}
impl Count {
    fn note(&mut self,hr:i32){
        if hr>=0 {self.ok=self.ok.saturating_add(1);}
        else {self.failed=self.failed.saturating_add(1);}
    }
    pub const fn any(self)->bool {self.ok>0 || self.failed>0}
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Sample {
    pub generation:u64,
    pub flips:usize,
    pub counts:[[Count;METHODS];ROLES],
}
impl Sample {
    pub fn summary(&self)->String {
        let mut fields=Vec::new();
        for role in [Role::Primary,Role::Backbuffer,Role::Depth,Role::Texture,Role::Other]{
            for method in [
                Method::AddAttachedSurface,Method::DeleteAttachedSurface,
                Method::GetDC,Method::IsLost,Method::Lock,Method::ReleaseDC,
                Method::Restore,Method::SetClipper,Method::GetDDInterface,
                Method::Unlock,Method::Blt,Method::BltFast,Method::GetAttachedSurface
            ]{
                let c=self.counts[role as usize][method as usize];
                if c.any(){
                    fields.push(format!("{}.{}={}/{}",role.label(),method.label(),c.ok,c.failed));
                }
            }
        }
        let usage=if fields.is_empty(){"none".to_owned()}else{fields.join(",")};
        format!("SURFACE-USAGE: generation={} successful_primary_flips={} calls_ok/failed={} (same successful primary Flip group, opt-in passive audit; NOT display FPS)",
                self.generation,self.flips,usage)
    }
}

#[derive(Debug)]
pub struct Audit {
    generation:u64,
    initialized:bool,
    flips:usize,
    counts:[[Count;METHODS];ROLES],
}
impl Audit {
    pub const fn new()->Self {
        Self{generation:0,initialized:false,flips:0,
             counts:[[Count{ok:0,failed:0};METHODS];ROLES]}
    }
    /// Only after successful SetDisplayMode. Invalidate incomplete samples.
    pub fn reset(&mut self,generation:u64){
        self.generation=generation;
        self.initialized=true;
        self.flips=0;
        self.counts=[[Count::default();METHODS];ROLES];
    }
    /// Called after the real method has returned; HRESULT is never changed.
    pub fn record(&mut self,role:Role,method:Method,hr:i32){
        if !self.initialized{return;}
        self.counts[role as usize][method as usize].note(hr);
    }
    /// Only on successful primary Flip. Includes all methods since reset
    /// or the previous group's final Flip; at most 12 samples are logged.
    pub fn observe_primary_flip(&mut self)->Option<Sample>{
        if !self.initialized{return None;}
        self.flips+=1;
        if self.flips<FLIPS_PER_AUDIT{return None;}
        let sample=Sample{generation:self.generation,flips:self.flips,counts:self.counts};
        self.flips=0;
        self.counts=[[Count::default();METHODS];ROLES];
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caps_roles_keep_3d_buffers_and_textures_separate(){
        assert_eq!(classify(Some(0x1000e238)),Role::Primary);
        assert_eq!(classify(Some(0x1000601c)),Role::Backbuffer);
        assert_eq!(classify(Some(0x10024000)),Role::Depth);
        assert_eq!(classify(Some(0x00401808)),Role::Texture);
        assert_eq!(classify(None),Role::Other);
    }
    #[test]
    fn no_events_are_invented_before_a_successful_mode(){
        let mut audit=Audit::new();
        audit.record(Role::Primary,Method::Lock,0);
        for _ in 0..240 {assert!(audit.observe_primary_flip().is_none());}
    }
    #[test]
    fn full_sample_counts_success_failures_and_excludes_zero_entries(){
        let mut audit=Audit::new();
        audit.reset(2);
        audit.record(Role::Backbuffer,Method::Lock,0);
        audit.record(Role::Backbuffer,Method::Lock,-1);
        audit.record(Role::Depth,Method::Restore,0);
        for _ in 0..119{assert!(audit.observe_primary_flip().is_none());}
        let sample=audit.observe_primary_flip().unwrap();
        assert_eq!(sample.generation,2);
        assert_eq!(sample.flips,120);
        assert_eq!(sample.counts[Role::Backbuffer as usize][Method::Lock as usize],
                   Count{ok:1,failed:1});
        assert!(sample.summary().contains("backbuffer.Lock=1/1"));
        assert!(sample.summary().contains("depth.Restore=1/0"));
        assert!(!sample.summary().contains("primary.Lock"));
        for _ in 0..119{assert!(audit.observe_primary_flip().is_none());}
        let empty=audit.observe_primary_flip().unwrap();
        assert!(empty.summary().contains("calls_ok/failed=none"));
    }
    #[test]
    fn mode_change_discards_partial_flips_and_old_method_totals(){
        let mut audit=Audit::new();
        audit.reset(1);
        audit.record(Role::Primary,Method::GetDC,0);
        for _ in 0..90{assert!(audit.observe_primary_flip().is_none());}
        audit.reset(2);
        for _ in 0..119{assert!(audit.observe_primary_flip().is_none());}
        let sample=audit.observe_primary_flip().unwrap();
        assert_eq!(sample.generation,2);
        assert_eq!(sample.counts[Role::Primary as usize][Method::GetDC as usize],Count::default());
    }
    #[test]
    fn saturating_event_counters_never_wrap(){
        let mut c=Count{ok:u64::MAX,failed:u64::MAX};
        c.note(0);
        c.note(-1);
        assert_eq!(c,Count{ok:u64::MAX,failed:u64::MAX});
    }
    #[test]
    fn added_real_hook_calls_are_counted_with_lock_unlock_parity(){
        let mut a=Audit::new();a.reset(3);
        for _ in 0..2 {
            a.record(Role::Texture,Method::Lock,0);
            a.record(Role::Texture,Method::Unlock,0);
        }
        a.record(Role::Backbuffer,Method::Blt,0);
        a.record(Role::Texture,Method::BltFast,-1);
        a.record(Role::Primary,Method::GetAttachedSurface,0);
        for _ in 0..119 {assert!(a.observe_primary_flip().is_none());}
        let s=a.observe_primary_flip().unwrap().summary();
        for term in ["texture.Lock=2/0","texture.Unlock=2/0",
                     "backbuffer.Blt=1/0","texture.BltFast=0/1",
                     "primary.GetAttachedSurface=1/0"] {assert!(s.contains(term));}
    }
    #[test]
    fn new_method_counts_reset_on_mode_change(){
        let mut a=Audit::new();a.reset(2);
        a.record(Role::Other,Method::Unlock,0);
        a.record(Role::Primary,Method::GetAttachedSurface,0);
        a.reset(3);
        for _ in 0..120 {let _=a.observe_primary_flip();}
        let empty=a.observe_primary_flip();
        assert!(empty.is_none());
    }
}
