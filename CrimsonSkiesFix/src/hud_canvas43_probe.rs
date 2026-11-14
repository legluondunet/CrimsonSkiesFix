//! Experimental, opt-in, native WinMM HUD overlay canvas aspect A/B.
//!
//! In the decrypted 1.02 PE32, the HUD overlay render loop at 0x455F30
//! calls 0x455C60. When the legacy overlay path runs, 0x455C60 invokes
//! the typed size setter 0x4D7020 with exact 640x480 values, calls the
//! renderer (0x4D6AC0), and restores the alternate 320x240 size.
//! At a native 1920x1200 16:10 output, that 4:3 source/raster mismatch
//! correlates with a measured 147x125 elliptical gauge; the same gauge
//! is 147x150 in a native 4:3, 1600x1200 output.
//!
//! This A/B changes ONLY these two HUD-local canvas HEIGHT immediates:
//!   640x480 -> 640x400 and 320x240 -> 320x200 at 1920x1200
//!   640x480 -> 640x360 and 320x240 -> 320x180 at 1920x1080.
//! The world camera/FOV, actual windowed DD4 render resolution,
//! 800x600 menu/movie path, horizontal HUD coordinates and the on-disk
//! EXE are untouched. This is a hypothesis test, NOT a proven fix:
//! the typed size setter may affect HUD placement in unforeseen ways.
//! Requires [Debug] ExperimentalHudCanvas43Probe=1 (OFF by default).
use std::{ffi::c_void,ptr,sync::atomic::{AtomicBool,Ordering}};
use super::{
    PatchResult,IMAGE_BASE,PAGE_EXECUTE_READWRITE,GetModuleHandleW,
    VirtualProtect,GetCurrentProcess,FlushInstructionCache,
};

// VA addresses; image is SHA-locked and 0x400000 fixed.
const FULL_CANVAS_SITE:usize=0x0045_5CA5; // push 480; push 640; push ecx; call 4D7020
const HALF_CANVAS_SITE:usize=0x0045_5CF2; // push 240; push 320; push eax; call 4D7020
const FULL_PREIMAGE:[u8;16]=[
    0x68,0xE0,0x01,0x00,0x00, // push 480
    0x68,0x80,0x02,0x00,0x00, // push 640
    0x51,                     // push ecx
    0xE8,0x6B,0x13,0x08,0x00, // call 0x4D7020
];
const HALF_PREIMAGE:[u8;16]=[
    0x68,0xF0,0x00,0x00,0x00, // push 240
    0x68,0x40,0x01,0x00,0x00, // push 320
    0x50,                     // push eax
    0xE8,0x1E,0x13,0x08,0x00, // call 0x4D7020
];
static INSTALLED:AtomicBool=AtomicBool::new(false);

fn transformed_canvas_height(source_height:u32,w:u32,h:u32)->Option<u32>{
    if source_height!=480 && source_height!=240 {return None;}
    if w<960||h<600||w>8192||h>4320||
        (w as u64)*(h as u64)>33_554_432||
        (w as u64)*3<=(h as u64)*4||
        (w as u64)>4*(h as u64) {return None;}
    // Vertical stretch factor relative to 4:3 is 3W/(4H).
    // Decrease source-canvas HEIGHT to compensate. Round the ratio using
    // u64 arithmetic to avoid a truncated 4:3 boundary error.
    let denom=3u64*(w as u64);
    let numerator=(source_height as u64)*4*(h as u64);
    let result=((numerator+denom/2)/denom) as u32;
    (result>=80 && result<source_height).then_some(result)
}

unsafe fn matches(site:usize,preimage:&[u8;16])->bool{
    let actual=std::slice::from_raw_parts(site as *const u8,preimage.len());
    actual==preimage
}

unsafe fn write_operand(site:usize,height:u32)->bool{
    // Only four bytes, the little-endian immediate within a verified
    // existing PUSH imm32. Never modify the push opcode or call target.
    let address=(site+1) as *mut c_void;
    let mut old_protection=0u32;
    if VirtualProtect(address,4,PAGE_EXECUTE_READWRITE,&mut old_protection)==0{
        return false;
    }
    ptr::copy_nonoverlapping(height.to_le_bytes().as_ptr(),address.cast::<u8>(),4);
    let flushed=FlushInstructionCache(
        GetCurrentProcess(),address.cast_const(),4
    )!=0;
    let mut ignored=0;
    let restored=VirtualProtect(address,4,old_protection,&mut ignored)!=0;
    flushed && restored
}

pub(super) unsafe fn install(w:u32,h:u32,result:PatchResult){
    let config=crate::cfg();
    if !config.experimental_hud_canvas43_probe{return;}
    if !crate::diagnostics::is_native_windows() ||
       config.force_4_by_3 ||
       config.experimental_hud_fov43_probe ||
       config.experimental_hud_entity_y43_probe ||
       !config.log_enabled ||
       !config.native_3d_widescreen ||
       !config.experimental_windowed_adapter ||
       result!=PatchResult::Patched {
        crate::log("HUD-CANVAS-43: refused; requires native WinMM, active verified widescreen, Log=1 and both previous HUD probes disabled");
        return;
    }
    if INSTALLED.load(Ordering::Acquire) {return;}
    let (Some(full),Some(half))=(
        transformed_canvas_height(480,w,h),
        transformed_canvas_height(240,w,h)
    ) else {
        crate::log("HUD-CANVAS-43: refused unsupported native aspect dimensions");
        return;
    };
    // Preflight BOTH known callsites before touching the game.
    if GetModuleHandleW(ptr::null()) as usize!=IMAGE_BASE ||
       !matches(FULL_CANVAS_SITE,&FULL_PREIMAGE) ||
       !matches(HALF_CANVAS_SITE,&HALF_PREIMAGE) {
        crate::log("HUD-CANVAS-43: refused original 1.02 HUD overlay canvas callsite preimage mismatch; memory unchanged");
        return;
    }
    if !write_operand(FULL_CANVAS_SITE,full) {
        let rollback=write_operand(FULL_CANVAS_SITE,480);
        crate::log(&format!(
            "HUD-CANVAS-43: first canvas update failed; original 480 restored={rollback}"
        ));
        return;
    }
    if !write_operand(HALF_CANVAS_SITE,half) {
        let a=write_operand(FULL_CANVAS_SITE,480);
        let b=write_operand(HALF_CANVAS_SITE,240);
        crate::log(&format!(
            "HUD-CANVAS-43: second canvas update failed; restored_full={a} restored_half={b}"
        ));
        return;
    }
    INSTALLED.store(true,Ordering::Release);
    crate::log(&format!(
        "HUD-CANVAS-43: enabled=1 native={}x{} full=640x{} half=320x{} \
         original_full=640x480 original_half=320x240 two_hud_local_immediates_only=true \
         world_fov_untouched=true experimental=true",
        w,h,full,half
    ));
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn exact_4by3_canvas_remapping_is_resolution_independent(){
        assert_eq!(transformed_canvas_height(480,1920,1200),Some(400));
        assert_eq!(transformed_canvas_height(240,1920,1200),Some(200));
        assert_eq!(transformed_canvas_height(480,1920,1080),Some(360));
        assert_eq!(transformed_canvas_height(240,1920,1080),Some(180));
        assert_eq!(transformed_canvas_height(480,3440,1440),Some(268));
        assert_eq!(transformed_canvas_height(240,3440,1440),Some(134));
        assert_eq!(transformed_canvas_height(480,1600,1200),None);
        assert_eq!(transformed_canvas_height(480,800,600),None);
        assert_eq!(transformed_canvas_height(240,1080,1920),None);
        assert_eq!(transformed_canvas_height(100,1920,1200),None);
    }

    #[test]
    fn hud_overlay_canvas_setters_are_verifiably_unchanged_game_calls(){
        assert_eq!(FULL_PREIMAGE[0],0x68);
        assert_eq!(HALF_PREIMAGE[0],0x68);
        let full=u32::from_le_bytes(FULL_PREIMAGE[1..5].try_into().unwrap());
        let half=u32::from_le_bytes(HALF_PREIMAGE[1..5].try_into().unwrap());
        assert_eq!(full,480);
        assert_eq!(half,240);
        let full_disp=i32::from_le_bytes(FULL_PREIMAGE[12..16].try_into().unwrap());
        let half_disp=i32::from_le_bytes(HALF_PREIMAGE[12..16].try_into().unwrap());
        assert_eq!((FULL_CANVAS_SITE+16) as i64+full_disp as i64,0x4D7020);
        assert_eq!((HALF_CANVAS_SITE+16) as i64+half_disp as i64,0x4D7020);
        assert_eq!(FULL_PREIMAGE[5..10],[0x68,0x80,0x02,0x00,0x00]);
        assert_eq!(HALF_PREIMAGE[5..10],[0x68,0x40,0x01,0x00,0x00]);
    }
}
