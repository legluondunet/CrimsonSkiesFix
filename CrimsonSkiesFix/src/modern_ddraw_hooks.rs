//! Modern IDirectDraw2/4/7 call-through hooks.
//! Preserves DD4-specific native virtual graph ownership, monitor detection,
//! mode selection and GOS/launcher behavior through unchanged callbacks.
use super::*;

unsafe extern "system" fn modern_set_cooperative(this:*mut c_void,hwnd:Hwnd,flags:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_COOPERATIVE) else{return DDERR_GENERIC};
    let call:CooperativeFn=std::mem::transmute(addr);
    if !hwnd.is_null() && cfg().log_enabled && PRE_EXCLUSIVE_DESKTOP.get().is_none(){
        // Critical: capture monitor dimensions BEFORE exclusive DirectDraw
        // SetDisplayMode shrinks the Wine mode and game HWND to 640/800.
        if let Some(area)=win32::launch_monitor_or_game_window(hwnd){
            let w=area.right.saturating_sub(area.left) as u32;
            let h=area.bottom.saturating_sub(area.top) as u32;
            if w>0 && h>0 {
                let _=PRE_EXCLUSIVE_DESKTOP.set((w,h));
                log(&format!("BORDERLESS-PREFLIGHT: captured pre-exclusive monitor={w}x{h} (read-only)"));
            }
        }
    }
    let hr=if cfg().experimental_windowed_adapter &&
        GAME_DD4_VTABLE.load(Ordering::Relaxed)==*(this.cast::<usize>()) {
        in_game_adapter::cooperative(this,hwnd,flags,call)
            .unwrap_or_else(||call(this,hwnd,flags))
    }else if cfg().experimental_windowed_adapter && in_game_adapter::must_block_other_mode_interface(){
        log("WINDOWED-GAME: non-DD4 SetCooperativeLevel blocked after virtual graph exposure");
        0x8000_4001u32 as i32
    }else{call(this,hwnd,flags)};
    if !hwnd.is_null() && cfg().log_enabled {
        // Never print the numeric HWND. A stale HWND is checked before use.
        LAST_GAME_HWND.store(hwnd as usize,Ordering::Relaxed);
        FIRST_MODERN_VALID_HWND.call_once(||log(&format!(
            "WIN-TRACE: first game-supplied non-NULL HWND: {}",
            win32::game_window_snapshot(hwnd))));
    }
    if MODERN_COOPERATIVE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: SetCooperativeLevel hwnd={} requested_flags={flags:#x} result={hr:#x} (unchanged)",
            if hwnd.is_null(){"NULL"}else{"provided"}));
        trace_game_windows("modern DirectDraw SetCooperativeLevel");
    }
    hr
}

unsafe extern "system" fn modern_set_display_mode(
    this:*mut c_void,w:u32,h:u32,bpp:u32,refresh:u32,flags:u32
)->Hr {
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:ModernDisplayModeFn=std::mem::transmute(addr);
    let hr=if cfg().experimental_windowed_adapter &&
        GAME_DD4_VTABLE.load(Ordering::Relaxed)==*(this.cast::<usize>()) {
        in_game_adapter::display_mode(this,w,h,bpp,refresh,flags,call)
            .unwrap_or_else(||call(this,w,h,bpp,refresh,flags))
    }else if cfg().experimental_windowed_adapter && in_game_adapter::must_block_other_mode_interface(){
        log("WINDOWED-GAME: non-DD4 SetDisplayMode blocked after virtual graph exposure");
        0x8000_4001u32 as i32
    }else{call(this,w,h,bpp,refresh,flags)}; // 5 arguments since IDirectDraw2
    if hr>=0 {
        if let Ok(mut session)=PRESENTATION_SESSION.lock(){
            session.begin_mode(presentation::Mode{width:w,height:h,bpp});
            if cfg().surface_usage_trace && cfg().log_enabled {
                if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
                    audit.reset(session.epoch);
                }
                if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
                    audit.reset(session.epoch);
                }
            }
            if cfg().log_enabled {
                if let Ok(mut group)=SCENE_FLIP_GROUP.lock(){
                    group.reset(session.epoch,
                        D3D3_BEGIN_EVENTS.load(Ordering::Relaxed),
                        D3D3_END_EVENTS.load(Ordering::Relaxed));
                }
            }
            if cfg().log_enabled{
                log(&format!("CHAIN-EPOCH: generation={} new_mode={w}x{h}x{bpp}; previous chain invalidated",
                    session.epoch));
            }
        }
    }
    if MODERN_DISPLAY_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: SetDisplayMode {w}x{h} bpp={bpp} refresh={refresh} flags={flags:#x} result={hr:#x} (unchanged)"));
        let hwnd=LAST_GAME_HWND.load(Ordering::Relaxed) as Hwnd;
        if !hwnd.is_null() && cfg().log_enabled{
            log(&format!("WIN-TRACE: after SetDisplayMode {w}x{h}: {}",
                win32::game_window_snapshot(hwnd)));
        }
    }
    hr
}

// In the windowed adapter, the game's DD4 logical mode is not the monitor's
// native mode. Preserve the native HRESULT, but return the mode the game
// originally requested to avoid a false 1920x1080x32 validation failure.
unsafe extern "system" fn modern_get_display_mode(this:*mut c_void,d:*mut c_void)->Hr{
    let Some(addr)=orig(this,DD_GET_DISPLAY_MODE) else{return DDERR_GENERIC;};
    let call:GetDisplayModeFn=std::mem::transmute(addr);
    let hr=call(this,d);
    let eligible=cfg().experimental_windowed_adapter &&
        GAME_DD4_VTABLE.load(Ordering::Relaxed)==*(this.cast::<usize>());
    let rewritten=hr>=0 && eligible && in_game_adapter::rewrite_display_mode(this,d);
    if MODERN_GET_DISPLAY_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("WINDOWED-GAME: DD4 GetDisplayMode native HRESULT={hr:#x}, logical_rewrite={rewritten}"));
    }
    hr
}

// A strictly read-only GetSurfaceDesc probe of the *existing* DirectDraw
// object. No Lock/Unlock, new surfaces, pixel copying, or COM ownership changes.
// Only modern 124-byte DDSURFACEDESC2 objects can be safely probed.
unsafe extern "system" fn modern_create_surface(
    this:*mut c_void,desc:*mut c_void,out:*mut *mut c_void,outer:*mut c_void
)->Hr {
    let Some(addr)=orig(this,DD_CREATE_SURFACE) else{return DDERR_GENERIC};
    let call:SurfaceFn=std::mem::transmute(addr);
    let hr=if cfg().experimental_windowed_adapter &&
        GAME_DD4_VTABLE.load(Ordering::Relaxed)==*(this.cast::<usize>()) {
        if let Some(hr)=in_game_adapter::create_surface(this,desc,out,outer){
            hr
        }else if cfg().legacy_offscreen_rgb565{
            // The game's observed 800x600 16bpp legacy artwork writes
            // were landing in a native 32-bit DDSCL_NORMAL system-memory
            // surface. Restrict correction to the EXACT observed DD4
            // offscreen caps/descriptor/mode; clone descriptor so the
            // game-provided DD4 arguments are never mutated.
            if let Some(mut requested)=in_game_adapter::legacy_offscreen_rgb565(this,desc,outer){
                let native_hr=call(this,requested.as_mut_ptr().cast(),out,outer);
                log(&format!(
                    "WINDOWED-COLOR: LegacyOffscreenRGB565 native image source {}x{} RGB565 creation hr={:#010x} (original game descriptor unchanged)",
                    requested[3],requested[2],native_hr as u32));
                native_hr
            }else{
                call(this,desc,out,outer)
            }
        }else{
            call(this,desc,out,outer)
        }
    }else{call(this,desc,out,outer)};
    if MODERN_SURFACE_CREATE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("MODERN-DDRAW: CreateSurface result={hr:#x} object_returned={} (unchanged)",
            hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        if !desc.is_null() && *(desc as *const u32)==124 {
            probe_modern_surface_layout(*out,"CreateSurface");
        }
        if cfg().log_enabled && SURFACE_CREATE_DETAIL_COUNT.fetch_add(1,Ordering::Relaxed)<24 {
            // Read-only DWORD prefix of a valid DDSURFACEDESC / DDSURFACEDESC2.
            // Both structures begin: dwSize, dwFlags, dwHeight, dwWidth.
            if !desc.is_null(){
                let words=desc as *const u32;
                let size=*words;
                if size==108 || size==124 {
                    let flags=*words.add(1);
                    let height=*words.add(2);
                    let width=*words.add(3);
                    let caps=modern_surface_caps(*out);
                    log(&format!("SURFACE-CREATE: desc_size={size} flags={flags:#x} width={} height={} caps={} (observed; no changes)",
                        if flags&4!=0{width.to_string()}else{"unspecified".into()},
                        if flags&2!=0{height.to_string()}else{"unspecified".into()},
                        caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
                }else{
                    log(&format!("SURFACE-CREATE: unexpected descriptor size={size} (skipping inspection)"));
                }
            }
        }
        observe_modern_surface(*out,"CreateSurface");
    }
    hr
}

// A single hook can be used on all newer surface versions because these
// specific method ABIs (and vtable indices) are stable, per ddraw.h.
// Unlike legacy v1 pacing, this instrumentation NEVER sleeps or edits frames.
pub(super) unsafe fn observe_modern_ddraw(object:*mut c_void,name:&str){
    if object.is_null() || !(cfg().log_enabled || cfg().modern_fps_pacing || cfg().experimental_windowed_adapter) {return;}
    if name=="IDirectDraw4" {
        GAME_DD4_VTABLE.store(*(object.cast::<usize>()),Ordering::Release);
    }
    let qi=replace_method(object,COM_QUERY_INTERFACE,ddraw_query_interface as *const () as usize);
    let cooperative=replace_method(object,DD_SET_COOPERATIVE,modern_set_cooperative as *const () as usize);
    let mode=replace_method(object,DD_SET_DISPLAY_MODE,modern_set_display_mode as *const () as usize);
    let surface=replace_method(object,DD_CREATE_SURFACE,modern_create_surface as *const () as usize);
    if name=="IDirectDraw4" && cfg().experimental_windowed_adapter {
        let get_mode=replace_method(object,DD_GET_DISPLAY_MODE,modern_get_display_mode as *const () as usize);
        if cfg().log_enabled {log(&format!("WINDOWED-GAME: DD4 GetDisplayMode hook_installed={get_mode}"));}
    }
    if MODERN_DDRAW_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("MODERN-DDRAW: interface={name} query_interface={qi} cooperative={cooperative} mode={mode} surface={surface} (observation only)"));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_ddraw_callback_types_preserve_win32_abi() {
        let _:CooperativeFn=modern_set_cooperative;
        let _:ModernDisplayModeFn=modern_set_display_mode;
        let _:GetDisplayModeFn=modern_get_display_mode;
        let _:SurfaceFn=modern_create_surface;
    }

    #[test]
    fn modern_ddraw_null_objects_are_rejected() {
        let null=std::ptr::null_mut();
        unsafe {
            observe_modern_ddraw(null,"IDirectDraw4");
            assert_eq!(modern_set_cooperative(null,null,0),DDERR_GENERIC);
            assert_eq!(modern_set_display_mode(null,1920,1080,16,0,0),DDERR_GENERIC);
            assert_eq!(modern_get_display_mode(null,null),DDERR_GENERIC);
            assert_eq!(modern_create_surface(null,null,std::ptr::null_mut(),null),DDERR_GENERIC);
        }
    }

    #[test]
    fn modern_ddraw_abi_slots_match_original() {
        assert_eq!(COM_QUERY_INTERFACE,0);
        assert_eq!(DD_CREATE_SURFACE,6);
        assert_eq!(DD_GET_DISPLAY_MODE,12);
        assert_eq!(DD_SET_COOPERATIVE,20);
        assert_eq!(DD_SET_DISPLAY_MODE,21);
    }
}
