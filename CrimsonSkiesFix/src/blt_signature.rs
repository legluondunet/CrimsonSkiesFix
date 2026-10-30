//! Optional, bounded Blt flag/source-presence audit. Does not call COM,
//! dereference surfaces, inspect pixels, or alter the game's Blt flags.
//! All entries refer to original successful/failed COM calls, not FPS.
#![allow(dead_code)]
use crate::surface_usage::Role;
const ROLES:usize=5;
const SLOTS:usize=8;
const FLIPS:usize=120;
#[derive(Copy,Clone,Debug,Default,PartialEq,Eq)]
struct Entry {used:bool,flags:u32,src:bool,ok:u64,fail:u64}
#[derive(Copy,Clone,Debug,PartialEq,Eq)]
struct Group {entries:[Entry;SLOTS],overflow_ok:u64,overflow_fail:u64}
impl Group {
    const fn empty()->Self {
        Self{entries:[Entry{used:false,flags:0,src:false,ok:0,fail:0};SLOTS],
             overflow_ok:0,overflow_fail:0}
    }
    fn record(&mut self,flags:u32,src:bool,hr:i32) {
        if let Some(e)=self.entries.iter_mut().find(|e|e.used && e.flags==flags && e.src==src){
            if hr>=0{e.ok=e.ok.saturating_add(1)}else{e.fail=e.fail.saturating_add(1)}
            return;
        }
        if let Some(e)=self.entries.iter_mut().find(|e|!e.used){
            *e=Entry{used:true,flags,src,ok:u64::from(hr>=0),fail:u64::from(hr<0)};
            return;
        }
        if hr>=0{self.overflow_ok=self.overflow_ok.saturating_add(1)}
        else{self.overflow_fail=self.overflow_fail.saturating_add(1)}
    }
}
#[derive(Debug)]
pub struct Audit {generation:u64,active:bool,flips:usize,groups:[Group;ROLES]}
impl Audit {
    pub const fn new()->Self {
        Self{generation:0,active:false,flips:0,groups:[Group::empty();ROLES]}
    }
    pub fn reset(&mut self,generation:u64){
        self.generation=generation;self.active=true;self.flips=0;
        self.groups=[Group::empty();ROLES];
    }
    pub fn record(&mut self,role:Role,flags:u32,src_provided:bool,hr:i32){
        if self.active{self.groups[role as usize].record(flags,src_provided,hr)}
    }
    pub fn primary_flip(&mut self)->Option<String>{
        if !self.active{return None;}
        self.flips+=1;
        if self.flips<FLIPS{return None;}
        let mut parts=Vec::new();
        for role in [Role::Primary,Role::Backbuffer,Role::Depth,Role::Texture,Role::Other]{
            let g=&self.groups[role as usize];
            for e in g.entries.iter().filter(|e|e.used) {
                parts.push(format!("{}.flags={:#010x}.src={}.ok/fail={}/{}",
                    role.label(),e.flags,if e.src{"provided"}else{"NULL"},e.ok,e.fail));
            }
            if g.overflow_ok>0 || g.overflow_fail>0{
                parts.push(format!("{}.overflow.ok/fail={}/{}",
                    role.label(),g.overflow_ok,g.overflow_fail));
            }
        }
        let detail=if parts.is_empty(){"none".to_string()}else{parts.join(",")};
        let result=format!("SURFACE-BLT-SIGNATURE: generation={} successful_primary_flips={} {} (exact Blt DWORD flags, NULL/provided source, result; no FX/pixel access; NOT FPS)",
                           self.generation,self.flips,detail);
        self.flips=0;
        self.groups=[Group::empty();ROLES];
        Some(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_depthfill_and_colorfill_and_source_nullness(){
        let mut a=Audit::new();a.reset(3);
        for _ in 0..240 {a.record(Role::Depth,0x02000000,false,0);}
        a.record(Role::Backbuffer,0x01000400,false,0);
        a.record(Role::Backbuffer,0x01000400,true,-1);
        for _ in 0..119 {assert!(a.primary_flip().is_none());}
        let text=a.primary_flip().unwrap();
        assert!(text.contains("depth.flags=0x02000000.src=NULL.ok/fail=240/0"));
        assert!(text.contains("backbuffer.flags=0x01000400.src=NULL.ok/fail=1/0"));
        assert!(text.contains("backbuffer.flags=0x01000400.src=provided.ok/fail=0/1"));
    }
    #[test]
    fn bounded_signatures_count_overflow_and_reset_after_each_group(){
        let mut a=Audit::new();a.reset(2);
        for flag in 0..8{a.record(Role::Texture,flag,false,0);}
        a.record(Role::Texture,0xff,true,-1);
        for _ in 0..120 {
            if let Some(text)=a.primary_flip(){
                assert!(text.contains("texture.overflow.ok/fail=0/1"));
            }
        }
        for _ in 0..119{assert!(a.primary_flip().is_none());}
        assert!(a.primary_flip().unwrap().contains("120 none"));
    }
    #[test]
    fn mode_reset_discards_partial_sample(){
        let mut a=Audit::new();
        a.record(Role::Depth,1,false,0);
        for _ in 0..120{assert!(a.primary_flip().is_none());}
        a.reset(1);
        a.record(Role::Depth,0x02000000,false,0);
        a.reset(2);
        for _ in 0..119{assert!(a.primary_flip().is_none());}
        let text=a.primary_flip().unwrap();
        assert!(text.contains("generation=2"));
        assert!(text.contains("120 none"));
    }
}
