//! Experimental HUD/world-camera FOV normalization A/B probe.
//!
//! The genuine 4:3 vs 16:10 screenshots demonstrate a vertical gauge
//! compression of 5/6 at 1920x1200. Two operands in the verified widescreen
//! delta replace original 60-degree / 30-degree reference constants in the
//! camera-entity normalization helper at 0x455BE0. This debug-only A/B
//! reverts those TWO references to their original values, while the main
//! resolution/HFOV patch, DD4, native Surface1 and presentation remain intact.
//!
//! This does NOT claim an isolated HUD-only fix. The camera-entity loop may
//! include non-HUD objects. Test photographs must compare the 3D camera FOV,
//! gauge proportions, and on-screen placement; disable immediately if wrong.
//! The stable default is OFF; no change to on-disk game executable.
use super::{
    PatchResult,IMAGE_BASE,PAGE_EXECUTE_READWRITE,GetModuleHandleW,VirtualProtect,
    GetCurrentProcess,FlushInstructionCache,
};
use std::{ffi::c_void,ptr};

struct Site {
    rva:usize,
    new:[u8;3],
    legacy:[u8;3],
}
const SITES:[Site;2]=[
    // 0x455BFF: fcom [0x00A1EFF0] -> fcom [0x0060409C] (60 degrees)
    Site {rva:0x55C01,new:[0xF0,0xEF,0xA1],legacy:[0x9C,0x40,0x60]},
    // 0x455C0C: fld qword [0x00A1EFF4] -> fld qword [0x00604090] (30deg)
    Site {rva:0x55C0E,new:[0xF4,0xEF,0xA1],legacy:[0x90,0x40,0x60]},
];

fn eligible_dimensions(w:u32,h:u32)->bool{
    w>=960&&h>=600&&w<=8192&&h<=4320&&
    (w as u64)*(h as u64)<=33_554_432&&
    (w as u64)*3>(h as u64)*4&&
    (w as u64)<=(h as u64)*4
}

fn matches_three(original:[u8;3],actual:[u8;3])->bool{
    original==actual
}

// No changes until BOTH memory preimages exactly match the bytes of the
// authenticated 38-region widescreen patch.
unsafe fn check_sites(base:usize)->bool {
    SITES.iter().all(|site|{
        let address=(base+site.rva) as *const u8;
        let actual:[u8;3]=[
            ptr::read(address),
            ptr::read(address.add(1)),
            ptr::read(address.add(2))
        ];
        matches_three(site.new,actual)
    })
}

unsafe fn write_site(base:usize,site:&Site,bytes:&[u8;3])->bool{
    let address=(base+site.rva) as *mut c_void;
    let mut protection=0u32;
    if VirtualProtect(address,3,PAGE_EXECUTE_READWRITE,&mut protection)==0{
        return false;
    }
    ptr::copy_nonoverlapping(bytes.as_ptr(),address.cast::<u8>(),3);
    let flushed=FlushInstructionCache(GetCurrentProcess(),address.cast_const(),3)!=0;
    let mut previous=0;
    let restored=VirtualProtect(address,3,protection,&mut previous)!=0;
    flushed&&restored
}

pub(super) unsafe fn apply(w:u32,h:u32,result:PatchResult){
    // The normal optimized default is a NO-OP. Refuse Wine and 4:3,
    // because they are not the intended native 16:10 experiment.
    if !crate::cfg().experimental_hud_fov43_probe{return;}
    if !crate::diagnostics::is_native_windows() ||
       !crate::cfg().log_enabled ||
       result!=PatchResult::Patched ||
       !eligible_dimensions(w,h) {
        crate::log("HUD-FOV-A-B: requested but gated-off (native Windows + Debug.Log=1 + successful new widescreen patch + >4:3 required)");
        return;
    }
    let base=GetModuleHandleW(ptr::null()) as usize;
    if base!=IMAGE_BASE || !check_sites(base) {
        crate::log("HUD-FOV-A-B: refused; expected original verified wide patch operand bytes absent");
        return;
    }
    let mut completed=0;
    for site in SITES.iter(){
        if !write_site(base,site,&site.legacy){
            // Best-effort rollback of all modified operand bytes. If a
            // VirtualProtect restore fails we refuse to claim success.
            let _=write_site(base,site,&site.new);
            for previous in SITES[..completed].iter().rev(){
                let _=write_site(base,previous,&previous.new);
            }
            crate::log("HUD-FOV-A-B: rejected; failed operand update; rollback attempted");
            return;
        }
        completed+=1;
    }
    crate::log(&format!(
        "HUD-FOV-A-B: enabled=1 candidate=legacy-camera-reference-only sites=2 dimensions={w}x{h} old_60deg_and_30deg=true world_fov_change_possible=true verified_in_memory_only=true"
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exactly_two_verified_three_byte_operands(){
        assert_eq!(SITES.len(),2);
        assert_eq!(SITES[0].rva,0x55C01);
        assert_eq!(SITES[0].legacy,[0x9C,0x40,0x60]);
        assert_eq!(SITES[1].rva,0x55C0E);
        assert_eq!(SITES[1].legacy,[0x90,0x40,0x60]);
        assert!(SITES.iter().all(|s|s.new!=s.legacy));
        assert!(matches_three([1,2,3],[1,2,3]));
        assert!(!matches_three([1,2,3],[1,2,4]));
    }
    #[test]
    fn only_native_widescreen_above_4by3_is_eligible(){
        for (w,h) in [(1920,1200),(1920,1080),(3440,1440)]{
            assert!(eligible_dimensions(w,h));
        }
        for (w,h) in [(0,0),(800,600),(1600,1200),(1080,1920)]{
            assert!(!eligible_dimensions(w,h));
        }
    }
}
