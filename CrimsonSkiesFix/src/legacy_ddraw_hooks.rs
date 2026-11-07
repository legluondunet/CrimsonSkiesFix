//! Legacy IDirectDraw and IDirectDrawSurface hooks (Win32 x86).
//! These methods forward to the original vtable entries after the existing
//! borderless, FPS and read-only tracing checks. Functions were moved without
//! changing call order, callback signatures, or COM ownership.
use super::*;
static NATIVE_RESTORE_TRACE_COUNT:AtomicUsize=AtomicUsize::new(0);

// Read-only interface discovery: the original COM QueryInterface is called
// without changing IID, return value or output. This identifies whether the
// 3D renderer uses other DirectDraw/Direct3D interfaces beyond the v1 hooks.
pub(super) unsafe extern "system" fn ddraw_query_interface(this:*mut c_void,iid:*const Guid,out:*mut *mut c_void)->Hr {
    let Some(addr)=orig(this,COM_QUERY_INTERFACE) else{return DDERR_GENERIC};
    let call:QueryInterfaceFn=std::mem::transmute(addr);
    let hr=call(this,iid,out);
    if DDRAW_QI_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("COM-TRACE: IDirectDraw::QueryInterface iid={} result={hr:#x} object_returned={}",
            guid_label(iid),hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        if let Some(version)=ddraw_iface(iid){observe_modern_ddraw(*out,version);}
        if is_d3d3_iid(iid){observe_d3d3(*out);}
    }
    hr
}

pub(super) unsafe extern "system" fn surface_query_interface(this:*mut c_void,iid:*const Guid,out:*mut *mut c_void)->Hr {
    let Some(addr)=orig(this,COM_QUERY_INTERFACE) else{return DDERR_GENERIC};
    let call:QueryInterfaceFn=std::mem::transmute(addr);
    let hr=call(this,iid,out);
    if SURFACE_QI_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16 {
        log(&format!("COM-TRACE: IDirectDrawSurface::QueryInterface iid={} result={hr:#x} object_returned={}",
            guid_label(iid),hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        if let Some(version)=surface_iface(iid){observe_modern_surface(*out,version);}
    }
    hr
}

// Trace only. Neither method is paced yet because a CPU lock/unlock
// or offscreen BltFast is not necessarily a displayed video frame.
unsafe extern "system" fn surface_blt_fast(this:*mut c_void,x:u32,y:u32,source:*mut c_void,src_rect:*const c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT_FAST) else{return DDERR_GENERIC};
    FIRST_BLT_FAST.call_once(||log("RENDER-TRACE: first IDirectDrawSurface::BltFast call (no FPS pacing on this path)"));
    let call:BltFastFn=std::mem::transmute(addr);
    call(this,x,y,source,src_rect,flags)
}

unsafe extern "system" fn surface_unlock(this:*mut c_void,rect_or_ptr:*const c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_UNLOCK) else{return DDERR_GENERIC};
    FIRST_UNLOCK.call_once(||log("RENDER-TRACE: first IDirectDrawSurface::Unlock call (no FPS pacing on this path)"));
    let call:UnlockFn=std::mem::transmute(addr);
    call(this,rect_or_ptr)
}

pub(super) unsafe extern "system" fn set_cooperative(this:*mut c_void,hwnd:Hwnd,flags:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_COOPERATIVE) else {return DDERR_GENERIC};
    let call:CooperativeFn=std::mem::transmute(addr);
    let settings=cfg();
    let trace=COOPERATIVE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12;
    if trace {
        log(&format!("DirectDraw SetCooperativeLevel requested_flags={flags:#x} hwnd={} borderless={}",
            if hwnd.is_null(){"NULL"}else{"provided"},settings.borderless));
    }
    if settings.borderless {
        // Experimental: DDSCL_NORMAL avoids switching into an exclusive
        // hardware mode, but legacy flip chains may need further shims.
        let hr=call(this,hwnd,DDSCL_NORMAL);
        if trace {log(&format!("DirectDraw SetCooperativeLevel DDSCL_NORMAL result={hr:#x}"));}
        if hr>=0 {
            if hwnd.is_null() {
                if trace {log("Borderless deferred: game passed a NULL HWND; waiting for a valid HWND");}
                if trace {trace_game_windows("SetCooperativeLevel NULL HWND");}
            } else if let Some(monitor)=win32::monitor_from_cursor_or_window(hwnd) {
                match win32::borderless_window(hwnd,monitor) {
                    Ok(())=>log(&format!("Borderless applied: monitor [{},{},{},{}]",
                        monitor.left,monitor.top,monitor.right,monitor.bottom)),
                    Err(reason)=>if trace {log(&format!("Borderless not applied: {reason}"));},
                }
            } else if trace {log("Borderless deferred: could not resolve a monitor");}
            return hr;
        }
        if trace {log(&format!("WARNING: DDSCL_NORMAL rejected ({hr:#x}); falling back")); }
    }
    call(this,hwnd,flags)
}

/// Passive diagnosis of a still-unattributed 1920x1200 -> 800x600
/// physical Windows mode switch. DirectDraw RestoreDisplayMode has the
/// same no-argument x86 COM ABI in IDirectDraw 1/2/4/7 (vtable slot 19).
/// It cannot repair the missing virtual windowed bridge by itself.
pub(super) unsafe extern "system" fn restore_display_mode(this:*mut c_void)->Hr {
    let Some(addr)=orig(this,DD_RESTORE_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:RestoreModeFn=std::mem::transmute(addr);
    let trace=cfg().log_enabled && diagnostics::is_native_windows() &&
        NATIVE_RESTORE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16;
    if trace {
        log(&format!("NATIVE-MODE-TRACE: COM RestoreDisplayMode ENTER current_physical={:?}",
            win32::physical_primary_monitor_dimensions()));
    }
    let hr=call(this);
    if trace {
        log(&format!("NATIVE-MODE-TRACE: COM RestoreDisplayMode EXIT hr={:#010x} current_physical={:?}",
            hr as u32,win32::physical_primary_monitor_dimensions()));
    }
    hr
}

pub(super) unsafe extern "system" fn set_display_mode(this:*mut c_void,w:u32,h:u32,bpp:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else {return DDERR_GENERIC};
    let call:DisplayModeFn=std::mem::transmute(addr);
    let settings=cfg();
    if settings.borderless && settings.desktop_resolution {
        // In windowed mode, avoid an exclusive monitor mode switch.
        // Does not fix internal game viewport and surface sizes yet.
        log(&format!("Keeping desktop mode; game SetDisplayMode({w},{h},{bpp}) suppressed"));
        return S_OK;
    }
    call(this,w,h,bpp)
}

unsafe extern "system" fn surface_flip(this:*mut c_void,target:*mut c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_FLIP) else{return DDERR_GENERIC};
    FIRST_FLIP.call_once(||log("PRESENT: first IDirectDrawSurface::Flip call received; FPS limiter active on this path"));
    pace_frame();
    let call:FlipFn=std::mem::transmute(addr);
    call(this,target,flags)
}

unsafe extern "system" fn surface_blt(this:*mut c_void,dst:*const c_void,source:*mut c_void,src:*const c_void,flags:u32,fx:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT) else{return DDERR_GENERIC};
    FIRST_BLT.call_once(||log("PRESENT: first primary IDirectDrawSurface::Blt call received; FPS limiter active on this path"));
    pace_frame();
    let call:BltFn=std::mem::transmute(addr);
    call(this,dst,source,src,flags,fx)
}

pub(super) unsafe extern "system" fn create_surface(this:*mut c_void,desc:*mut c_void,out:*mut *mut c_void,outer:*mut c_void)->Hr {
    let Some(addr)=orig(this,DD_CREATE_SURFACE) else {
        if cfg().log_enabled {log("STARTUP-DDRAW: IDirectDraw::CreateSurface original callback unavailable");}
        return DDERR_GENERIC;
    };
    let call:SurfaceFn=std::mem::transmute(addr);
    let hr=call(this,desc,out,outer);
    if cfg().log_enabled &&
        STARTUP_LEGACY_SURFACE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<32 {
        log(&format!(
            "STARTUP-DDRAW: IDirectDraw::CreateSurface desc={} outer={} hr={:#010x} object_returned={}",
            if desc.is_null(){"NULL"}else{"provided"},
            if outer.is_null(){"NULL"}else{"provided"},
            hr as u32,hr>=0 && !out.is_null() && !(*out).is_null()));
    }
    if hr>=0 && !out.is_null() && !(*out).is_null() {
        let surface=*out;
        if replace_method(surface,COM_QUERY_INTERFACE,surface_query_interface as *const () as usize) {
            log("COM-TRACE: IDirectDrawSurface::QueryInterface tracing installed");
        }
        if replace_method(surface,SURFACE_UNLOCK,surface_unlock as *const () as usize) {
            log("RENDER-TRACE: IDirectDrawSurface::Unlock tracing installed");
        }
        trace_game_windows("CreateSurface");
        if replace_method(surface,SURFACE_FLIP,surface_flip as *const () as usize) {
            log("IDirectDrawSurface::Flip paced (1..60 FPS)");
        }
        let getcaps_addr=method(surface,SURFACE_GET_CAPS);
        if getcaps_addr!=0 {
            let getcaps:GetCapsFn=std::mem::transmute(getcaps_addr);
            let mut caps=[0u32;4]; // accommodates DDSCAPS and DDSCAPS2
            if getcaps(surface,caps.as_mut_ptr())>=0 && caps[0]&DDSCAPS_PRIMARYSURFACE!=0 {
                if replace_method(surface,SURFACE_BLT,surface_blt as *const () as usize) {
                    log("Primary IDirectDrawSurface::Blt paced (1..60 FPS)");
                }
                if replace_method(surface,SURFACE_BLT_FAST,surface_blt_fast as *const () as usize) {
                    log("RENDER-TRACE: primary IDirectDrawSurface::BltFast tracing installed");
                }
            }
        }
    }
    hr
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callbacks_keep_directdraw_win32_x86_abi() {
        let _:QueryInterfaceFn=ddraw_query_interface;
        let _:QueryInterfaceFn=surface_query_interface;
        let _:CooperativeFn=set_cooperative;
        let _:DisplayModeFn=set_display_mode;
        let _:RestoreModeFn=restore_display_mode;
        let _:SurfaceFn=create_surface;
        let _:FlipFn=surface_flip;
        let _:BltFn=surface_blt;
        let _:BltFastFn=surface_blt_fast;
        let _:UnlockFn=surface_unlock;
    }

    #[test]
    fn null_com_callbacks_fail_closed_without_cfg_or_native_calls() {
        let null=std::ptr::null_mut();
        unsafe {
            assert_eq!(ddraw_query_interface(null,std::ptr::null(),std::ptr::null_mut()),DDERR_GENERIC);
            assert_eq!(surface_query_interface(null,std::ptr::null(),std::ptr::null_mut()),DDERR_GENERIC);
            assert_eq!(set_cooperative(null,null,0),DDERR_GENERIC);
            assert_eq!(set_display_mode(null,800,600,16),DDERR_GENERIC);
            assert_eq!(restore_display_mode(null),DDERR_GENERIC);
            assert_eq!(create_surface(null,null,std::ptr::null_mut(),null),DDERR_GENERIC);
            assert_eq!(surface_flip(null,null,0),DDERR_GENERIC);
            assert_eq!(surface_blt(null,std::ptr::null(),null,std::ptr::null(),0,null),DDERR_GENERIC);
            assert_eq!(surface_blt_fast(null,0,0,null,std::ptr::null(),0),DDERR_GENERIC);
            assert_eq!(surface_unlock(null,std::ptr::null()),DDERR_GENERIC);
        }
    }

    #[test]
    fn legacy_slots_are_unchanged() {
        assert_eq!(COM_QUERY_INTERFACE,0);
        assert_eq!(DD_CREATE_SURFACE,6);
        assert_eq!(DD_SET_COOPERATIVE,20);
        assert_eq!(DD_SET_DISPLAY_MODE,21);
        assert_eq!(DD_RESTORE_DISPLAY_MODE,19);
        assert_eq!(SURFACE_BLT,5);
        assert_eq!(SURFACE_BLT_FAST,7);
        assert_eq!(SURFACE_FLIP,11);
        assert_eq!(SURFACE_UNLOCK,32);
    }
}
