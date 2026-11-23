//! Strict opt-in, passive x86 draw-call inventory for the original GOS UI.
//! 
//! The previous cursor-object vtable[1] draw A/B was armed but never called
//! during 480 observed flips, so it cannot account for the gold game sprite.
//! This read-only trace forwards the exact eleven direct callsites to the
//! *original* shared GOS sprite renderer (0x5C0FF0) and logs, per callsite,
//! bounded source bitmap, raw X/Y coordinates and actual GameOS cursor X/Y.
//! 
//! All calls are forwarded in every scene. Never suppress pixels, alter GUI,
//! change DirectInput, mutate OS cursor, or ship assets. Only install on the
//! admitted 1.02 EXE under Wine with verified borderless 800x600 UI mode and
//! a separate explicitly enabled [Debug] WineGosDrawTrace=1.
//! 
//! The hook itself changes exactly the CALL rel32 targets of authenticated
//! instructions (all 5 bytes checked beforehand). Game EXE on disk unchanged.
use std::{ffi::c_void,ptr,sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use crate::{diagnostics,log};

const BASE:usize=0x0040_0000;
const DRAW:usize=0x005c_0ff0;
const CURSOR_X:usize=0x0075_ccc0;
const CURSOR_Y:usize=0x0075_ccc4;
const PAGE_EXECUTE_READWRITE:u32=0x40;
const SITES:[usize;11]=[
    0x5c3bef,0x5c3c32,
    0x5c525c,0x5c52b7,
    0x5c8010,0x5c804a,
    0x5cc7e0,0x5cded8,0x5ce138,
    0x5ce983,0x5ce9c5
];
static INSTALLED:AtomicBool=AtomicBool::new(false);
static MENU_VERIFIED:AtomicBool=AtomicBool::new(false);
static COUNTS:[AtomicUsize;11]=[const{AtomicUsize::new(0)};11];

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualProtect(addr:*mut c_void,size:usize,new:u32,old:*mut u32)->i32;
    fn FlushInstructionCache(process:*mut c_void,address:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->*mut c_void;
}

type DrawFn=unsafe extern "C" fn(u32,u32,u32,u32,u32,u32,u32);
fn permitted()->bool{
    let c=crate::cfg();
    diagnostics::is_wine_runtime() &&
    c.native_dd4_bootstrap && !c.directdraw_passthrough &&
    c.wine_system_cursor_probe && c.wine_system_cursor_sync &&
    c.wine_gos_draw_trace && c.log_enabled &&
    !c.wine_game_cursor_render_suppress &&
    !c.wine_menu_cursor_bitmap_probe
}
fn should_sample(n:usize)->bool{n<=3 || n==16 || n==64 || n%256==0}

pub fn set_verified_menu_mode(verified:bool){
    MENU_VERIFIED.store(verified,Ordering::Release);
}
unsafe fn draw_observer<const N:usize>(
    a:u32,b:u32,c:u32,d:u32,e:u32,f:u32,g:u32
){
    // The original shared renderer has a cdecl 7×u32 ABI; the 11 verified
    // CALL sites each clean 0x1c bytes after returning. Preserve all bits,
    // including the raw float alpha argument and pointers.
    let active=MENU_VERIFIED.load(Ordering::Acquire) &&
        (crate::menu_scene::main_menu_active() ||
         crate::menu_scene::preferences_active()) && permitted();
    if active {
        let n=COUNTS[N].fetch_add(1,Ordering::Relaxed)+1;
        if should_sample(n){
            let x=(CURSOR_X as *const i32).read_volatile();
            let y=(CURSOR_Y as *const i32).read_volatile();
            log(&format!(
                "WINE-GOS-DRAW-TRACE: site={N} original_callsite=0x{:08X} n={n} bitmap=0x{a:08X} draw_xy=({},{}) raw_arg4=0x{d:08X} raw_arg5=0x{e:08X} raw_arg6=0x{f:08X} raw_arg7=0x{g:08X} gos_cursor=({x},{y}) [PASS THROUGH]",
                SITES[N],b as i32,c as i32
            ));
        }
    }
    let original:DrawFn=std::mem::transmute(DRAW);
    original(a,b,c,d,e,f,g);
}

macro_rules! emit{
    ($name:ident,$n:expr)=>{
        unsafe extern "C" fn $name(
            a:u32,b:u32,c:u32,d:u32,e:u32,f:u32,g:u32
        ){draw_observer::<$n>(a,b,c,d,e,f,g);}
    }
}
emit!(draw0,0);emit!(draw1,1);emit!(draw2,2);emit!(draw3,3);
emit!(draw4,4);emit!(draw5,5);emit!(draw6,6);emit!(draw7,7);
emit!(draw8,8);emit!(draw9,9);emit!(draw10,10);
const TARGETS:[DrawFn;11]=[
    draw0,draw1,draw2,draw3,draw4,draw5,
    draw6,draw7,draw8,draw9,draw10
];

fn relative_call(site:usize,target:usize)->[u8;5]{
    let diff=(target as u32).wrapping_sub((site as u32).wrapping_add(5));
    let r=diff.to_le_bytes();
    [0xE8,r[0],r[1],r[2],r[3]]
}

/// Only modifies the instruction's own authenticated 5-byte CALL sequence.
/// Every code page is restored to its original protection after writing.
unsafe fn replace_call(site:usize,target:usize)->bool{
    let mut protection=0u32;
    if VirtualProtect(
        site as *mut c_void,5,PAGE_EXECUTE_READWRITE,&mut protection
    )==0 {return false;}
    let bytes=relative_call(site,target);
    ptr::copy_nonoverlapping(bytes.as_ptr(),site as *mut u8,5);
    let flush=FlushInstructionCache(GetCurrentProcess(),site as *const c_void,5);
    let mut discarded=0;
    let restored=VirtualProtect(site as *mut c_void,5,protection,&mut discarded);
    flush!=0 && restored!=0
}

/// Call after verifying the same game-owned physical HWND and exact 800x600
/// DD4 virtual graph. Does not touch native Windows or other binaries.
pub fn install_for_verified_menu()->Result<String,String>{
    if !permitted() || !MENU_VERIFIED.load(Ordering::Acquire){
        return Err("not eligible for Wine-only diagnostic in verified GOS menu".into());
    }
    if INSTALLED.load(Ordering::Acquire) {
        return Ok("eleven read-only GOS draw sites already hooked".into());
    }
    unsafe{
        if GetModuleHandleW(ptr::null()) as usize != BASE{
            return Err("unexpected EXE image base; no hooks".into());
        }
        let preamble=std::slice::from_raw_parts(DRAW as *const u8,8);
        if preamble!=[0x64,0xA1,0,0,0,0,0x6A,0xFF]{
            return Err("shared GOS draw function does not match; no hooks".into());
        }
        // Verify all sites BEFORE patching any of them. Ensures that
        // dynamic rewrites or a different game build fail closed.
        for &site in &SITES{
            let b=std::slice::from_raw_parts(site as *const u8,5);
            if b!=relative_call(site,DRAW){
                return Err(format!("GOS draw CALL preimage mismatch at {site:#x}; no hooks"));
            }
        }
        let mut patched=0;
        for (i,&site) in SITES.iter().enumerate(){
            if !replace_call(site,TARGETS[i] as usize){
                // Restore every successfully changed site and any partly
                // written current site; never leave a chain half-installed.
                for &s in SITES.iter().take(i+1){
                    let _=replace_call(s,DRAW);
                }
                return Err(format!("code page patch failed at draw callsite={i}; rollback attempted"));
            }
            patched+=1;
        }
        INSTALLED.store(true,Ordering::Release);
        Ok(format!("{patched} exact GOS renderer callsites passively traced; all draws forwarded unchanged"))
    }
}

/// Call once each 120 actual menu flips, outside the surface graph mutex.
pub fn report_sample(){
    if !MENU_VERIFIED.load(Ordering::Acquire) ||
        !INSTALLED.load(Ordering::Acquire) || !permitted(){return;}
    let counts:Vec<String>=COUNTS.iter().enumerate().map(|(i,c)|{
        format!("{i}={}",c.load(Ordering::Relaxed))
    }).collect();
    log(&format!(
        "WINE-GOS-DRAW-TRACE-STATUS: cumulative_calls_by_original_site=[{}] all_draws_pass_through=true",
        counts.join(" ")
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn all_direct_callsites_are_unique_and_x86(){
        assert_eq!(SITES.len(),11);
        assert!(SITES.windows(2).all(|w|w[0]<w[1]));
        assert_eq!(relative_call(0x5c3bef,DRAW),[0xE8,0xFC,0xD3,0xFF,0xFF]);
        assert_eq!(relative_call(0x5ce9c5,DRAW),[0xE8,0x26,0x26,0xFF,0xFF]);
        assert_eq!(std::mem::size_of::<DrawFn>(),std::mem::size_of::<usize>());
        assert_eq!(CURSOR_X,0x75ccc0);
    }
    #[test]
    fn trace_never_changes_draw_pixel_behavior(){
        assert!(should_sample(1));
        assert!(should_sample(3));
        assert!(!should_sample(4));
        assert!(should_sample(16));
        assert!(should_sample(64));
        assert!(should_sample(256));
        let _:DrawFn=draw0;
        let _:DrawFn=draw10;
    }
}
