//! Modern DirectDrawSurface2/3/4/7 call-through hooks.
//! Mechanical extraction of native COM callbacks: no changes to vtable slots,
//! live virtual Surface4 identity, RG565/ROFFILE hooks, tracing or call order.
use super::*;

pub(super) unsafe fn modern_surface_caps(this:*mut c_void)->Option<u32> {
    // The first DWORD is identical in DDSCAPS and DDSCAPS2.
    let method=method(this,SURFACE_GET_CAPS);
    if method==0{return None;}
    let getcaps:GetCapsFn=std::mem::transmute(method);
    let mut caps=[0u32;4];
    (getcaps(this,caps.as_mut_ptr())>=0).then_some(caps[0])
}
unsafe fn modern_surface_primary(this:*mut c_void)->Option<bool> {
    modern_surface_caps(this).map(|caps|caps&DDSCAPS_PRIMARYSURFACE!=0)
}

pub(super) unsafe fn probe_modern_surface_layout(surface:*mut c_void,reason:&str) {
    if surface.is_null() || !cfg().log_enabled{return;}
    let Some(caps)=modern_surface_caps(surface) else{return;};
    if caps&(presentation::PRIMARY|presentation::BACKBUFFER|presentation::ZBUFFER)==0{
        return;
    }
    // Unrelated textures must not exhaust the bounded display-surface probe.
    if SURFACE_FORMAT_PROBE_COUNT.fetch_add(1,Ordering::Relaxed)>=20{return;}
    // Query original GetSurfaceDesc to avoid going through our own logging
    // hook. A method already patched on this vtable has an original mapping.
    let address=orig(surface,SURFACE_GET_DESC).or_else(||{
        let addr=method(surface,SURFACE_GET_DESC);
        (addr!=0 && addr!=modern_get_surface_desc as *const () as usize).then_some(addr)
    });
    let Some(address)=address else{return;};
    let call:GetSurfaceDescFn=std::mem::transmute(address);
    let mut desc=[0u32;31]; // 124-byte DDSURFACEDESC2, 32-bit Win32 ABI
    desc[0]=124;
    let hr=call(surface,desc.as_mut_ptr().cast());
    if hr<0{
        log(&format!("SURFACE-FORMAT: source={reason} role={} GetSurfaceDesc failed hr={hr:#x} (unchanged)",
            surface_roles(caps)));
        return;
    }
    let Some(info)=presentation::parse_surface_desc2(&desc) else{
        log(&format!("SURFACE-FORMAT: source={reason} unexpected descriptor size={} (unchanged)",desc[0]));
        return;
    };
    // A 16-bit game may use RGB565, RGB555 or an unusual format. Never
    // assume it is RGB565 without confirming the returned masks.
    let masks=info.color_masks.map(|m|format!("R={:#06x},G={:#06x},B={:#06x},A={:#06x}",
        m[0],m[1],m[2],m[3])).unwrap_or_else(||"not-RGB-or-unreported".into());
    let pixel=info.pixel_flags.map(|x|format!("{x:#x}")).unwrap_or_else(||"Unavailable".into());
    let bpp=info.bits_per_pixel.map(|x|x.to_string()).unwrap_or_else(||"Unavailable".into());
    let buffers=info.backbuffer_count.map(|x|x.to_string()).unwrap_or_else(||"Unavailable".into());
    log(&format!("SURFACE-FORMAT: source={reason} role={} dimensions={}x{} backbuffers={} pf_flags={} bits={} masks={} (read-only)",
        surface_roles(caps),info.width,info.height,buffers,pixel,bpp,masks));
}

// Named status based entirely on the public DirectDraw DDSCAPS flags.
// We deliberately do not log raw COM pointer addresses or identifiers.
// Final preflight only: this never changes the DirectDraw cooperative
// mode, surfaces, HWND or desktop. Until an actual adapter implements
// virtualized front/back/Z surfaces, log the safe 4:3 viewport as a plan.
// The per-mode state is independent of display-resolution labels: a new
// successful SetDisplayMode invalidates *all* earlier front/back/Z evidence,
// even if it switches to the same game resolution.
fn log_render_chain_preflight(request:u32,parent:u32,child:u32) {
    let Ok(mut session)=PRESENTATION_SESSION.lock() else{return;};
    let (changed,ready)=session.observe(request,parent,child);
    let mode=session.mode;
    let epoch=session.epoch;
    let snapshot=session.chain;
    drop(session);
    if changed{
        log(&format!("CHAIN-CLASSIFY: generation={epoch} front_to_back={} back_to_depth={} texture_mipmaps={} (non-invasive)",
            snapshot.front_to_back,snapshot.back_to_depth,snapshot.mipmaps));
    }
    if !ready{return;}
    let Some(mode)=mode else{return;};
    let Some(&(desktop_width,desktop_height))=PRE_EXCLUSIVE_DESKTOP.get() else{
        log("BORDERLESS-PREFLIGHT: original desktop bounds unavailable; keeping exclusive renderer");
        return;
    };
    if let Some(plan)=presentation::preflight(mode,desktop_width,desktop_height,snapshot){
        log(&format!(
            "BORDERLESS-PREFLIGHT: generation={epoch} complete 3D front->back->Z chain; internal={}x{}x{}; monitor={}x{}; aspect_viewport=x{},y{},{}x{}; requires_virtual_primary=true; applied=false",
            mode.width,mode.height,mode.bpp,plan.desktop_width,plan.desktop_height,
            plan.viewport.x,plan.viewport.y,plan.viewport.width,plan.viewport.height));
    }
}

// The GetAttachedSurface ABI is the same for Surface2/3/4/7.
// Call through with the original DDSCAPS or DDSCAPS2 pointer unchanged.
// Do not Release the returned COM interface: ownership belongs to the game.
unsafe extern "system" fn modern_get_attached_surface(
    this:*mut c_void,caps:*const u32,out:*mut *mut c_void
)->Hr {
    let Some(addr)=orig(this,SURFACE_GET_ATTACHED) else{return DDERR_GENERIC};
    let call:GetAttachedSurfaceFn=std::mem::transmute(addr);
    let hr=call(this,caps,out);
    record_surface_usage(this,surface_usage::Method::GetAttachedSurface,hr);
    if cfg().log_enabled {
        let n=SURFACE_ATTACH_TRACE_COUNT.fetch_add(1,Ordering::Relaxed);
        if n<24 {
            let requested=if caps.is_null(){None}else{Some(*caps)};
            let parent=modern_surface_caps(this);
            let child=if hr>=0 && !out.is_null() && !(*out).is_null(){
                modern_surface_caps(*out)
            }else{None};
            log(&format!(
                "SURFACE-ATTACH: requested={} parent={} ({}) attached={} ({}) hr={hr:#x} returned={} (unchanged)",
                requested.map(|x|format!("{x:#010x}")).unwrap_or_else(||"NULL".into()),
                parent.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
                parent.map(surface_roles).unwrap_or_else(||"unknown".into()),
                child.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
                child.map(surface_roles).unwrap_or_else(||"unknown".into()),
                hr>=0 && !out.is_null() && !(*out).is_null()));
        }
        if hr>=0 && !out.is_null() && !(*out).is_null(){
            if let (Some(a),Some(b))=(modern_surface_caps(this),modern_surface_caps(*out)){
                if a&DDSCAPS_PRIMARYSURFACE!=0 && b&DDSCAPS_BACKBUFFER!=0 {
                    SURFACE_FIRST_PRIMARY_ATTACH.call_once(||log(
                        "SURFACE-CHAIN: confirmed primary front buffer -> attached backbuffer (game-provided; no modification)"));
                }
                // The requested caps pointer is game-owned, not modified.
                if !caps.is_null(){log_render_chain_preflight(*caps,a,b);}
                if !caps.is_null() && (*caps&(presentation::BACKBUFFER|presentation::ZBUFFER))!=0 {
                    probe_modern_surface_layout(*out,"GetAttachedSurface");
                }
            }
        }
    }
    if hr>=0 && !out.is_null() && !(*out).is_null(){
        observe_modern_surface(*out,"GetAttachedSurface");
    }
    hr
}

// GetSurfaceDesc is read-only; this wrapper captures only the first
// 16-byte descriptor prefix and delegates exactly once to the system DLL.
unsafe extern "system" fn modern_get_surface_desc(this:*mut c_void,desc:*mut c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_GET_DESC) else{return DDERR_GENERIC};
    let call:GetSurfaceDescFn=std::mem::transmute(addr);
    let hr=call(this,desc);
    if cfg().log_enabled && SURFACE_DESC_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<20 {
        let layout=if hr>=0{describe_surface_layout(desc)}else{"call failed".into()};
        let caps=modern_surface_caps(this);
        log(&format!("SURFACE-DESC: {layout} caps={} role={} hr={hr:#x} (unchanged)",
            caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into())));
    }
    hr
}

// The native Win11 game uses a real DirectDraw front/back flip-chain.
// Its menu frames never traverse the virtual DD4 Flip bridge which owns
// the usual RGB565 scratch-signature compositor. Determine an exact,
// read-only 640x480/800x600 front surface before touching any pixels.
// Surface2/3 use the 108-byte DDSURFACEDESC; Surface4/7 use 124-byte
// DDSURFACEDESC2. Both share dwHeight/dwWidth offsets.
unsafe fn native_main_menu_viewport(surface:*mut c_void)->Option<borderless_native_blt::Rect>{
    // With Log=0 the native menu path installs only QI+Flip, not the
    // passive GetSurfaceDesc trampoline; borrow the genuine vtable method.
    let addr=orig(surface,SURFACE_GET_DESC).or_else(||{
        let real=method(surface,SURFACE_GET_DESC);
        (real!=0 && real!=modern_get_surface_desc as *const () as usize)
            .then_some(real)
    })?;
    let call:GetSurfaceDescFn=std::mem::transmute(addr);
    for size in [124u32,108u32] {
        let mut words=[0u32;31];
        words[0]=size;
        if call(surface,words.as_mut_ptr().cast())<0{continue;}
        if words[0]!=size{continue;}
        let (w,h)=(words[3],words[2]);
        if menu_brand::should_probe_menu_frame(w,h){
            return Some(borderless_native_blt::Rect{
                left:0,top:0,right:w as i32,bottom:h as i32
            });
        }
    }
    None
}

// Newer DirectDraw versions share the opaque CreateSurface calling
// convention, though the descriptor and surface types differ.
unsafe extern "system" fn modern_surface_flip(this:*mut c_void,target:*mut c_void,flags:u32)->Hr {
    roffile_terminator_fix::probe();
    let Some(addr)=orig(this,SURFACE_FLIP) else{return DDERR_GENERIC};
    FIRST_MODERN_FLIP.call_once(||{
        log("RENDER-MODERN: first newer-surface Flip (real modern rendering path)");
        log(&format!("SURFACE-TRACE: first modern Flip surface_caps={}",
            modern_surface_caps(this).map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
        trace_game_windows("first modern Flip");
    });
    sample_render_calls("Flip",&MODERN_FLIP_EVENTS,&MODERN_FLIP_START,&MODERN_FLIP_WINDOW);
    if cfg().modern_fps_pacing {
        match modern_surface_primary(this) {
            Some(true)=>{
                FIRST_MODERN_PACED_FLIP.call_once(||log(&format!(
                    "FPS-PACING: primary modern Flip enabled at {} FPS (experimental)",cfg().fps_limit)));
                pace_frame();
            },
            Some(false)=>{
                FIRST_MODERN_NONPRIMARY_FLIP.call_once(||log(
                    "FPS-PACING: non-primary modern Flip bypassed; no throttle on non-primary surface"));
            },
            None=>{
                FIRST_MODERN_NONPRIMARY_FLIP.call_once(||log(
                    "FPS-PACING: modern surface GetCaps unavailable; no throttle applied"));
            }
        }
    }
    let call:FlipFn=std::mem::transmute(addr);
    let result=call(this,target,flags);
    // Native Windows does not reach the synthetic DD4 windowed graph;
    // its MainMenu therefore cannot use the offscreen scratch signature.
    // Use the same Campaign-button pixel-art verification on the real
    // displayed primary *after* a successful game Flip, and only for an
    // exact native 4:3 menu-sized surface. Never alter the backbuffer,
    // EXE, display mode or the returned HRESULT.
    if result>=0 && menu_scene::main_menu_was_seen() &&
        diagnostics::is_native_windows() &&
        modern_surface_primary(this)==Some(true)
    {
        if let Some(view)=native_main_menu_viewport(this){
            let _=menu_brand::draw_native_primary(this,view);
        }else{
            menu_brand::mark_not_drawn();
        }
    }
    // This code runs AFTER the original Flip and only counts successful
    // calls on the real primary surface. It never modifies the HRESULT.
    if result>=0 && cfg().log_enabled && modern_surface_primary(this)==Some(true) {
        if surface_usage_enabled(){
            if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
                if let Some(sample)=audit.observe_primary_flip(){
                    if SURFACE_USAGE_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                        log(&sample.summary());
                    }
                }
            }
            if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
                if let Some(report)=audit.primary_flip(){
                    if BLT_SIGNATURE_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                        log(&report);
                    }
                }
            }
        }
        if let Ok(mut group)=SCENE_FLIP_GROUP.lock(){
            let sample=group.observe_flip(
                D3D3_BEGIN_EVENTS.load(Ordering::Relaxed),
                D3D3_END_EVENTS.load(Ordering::Relaxed));
            if let Some(sample)=sample {
                // Limit to 12 correlated windows over the whole session.
                if SCENE_FLIP_GROUP_LOGS.fetch_add(1,Ordering::Relaxed)<12 {
                    let successful_flip_rate=sample
                        .successful_primary_flip_calls_per_s()
                        .map(|rate|format!("{rate:.2}"))
                        .unwrap_or_else(||"Unavailable".to_string());
                    log(&format!(
                        "SCENE-CORRELATION: generation={} successful_primary_flips={} successful_begin_scene={} successful_end_scene={} begins_per_flip={:.2} ends_per_flip={:.2} first_to_last_flip_ms={} flip_intervals={} successful_primary_flip_calls_per_s={} (same successful-Flip group; rates are COM calls, NOT display FPS)",
                        sample.generation,sample.flips,sample.successful_begins,
                        sample.successful_ends,
                        sample.successful_begins as f64/sample.flips as f64,
                        sample.successful_ends as f64/sample.flips as f64,
                        sample.first_to_last_flip_ms,sample.observed_flip_intervals,
                        successful_flip_rate));
                    let stats=sample.interflip;
                    log(&format!(
                        "SCENE-INTERFLIP: generation={} intervals={} begin_min={} begin_max={} end_min={} end_max={} begin_hist_0_to_12_and_13plus={:?} begin_end_mismatches={} counter_regressions={} (between successful primary Flips; NOT displayed FPS)",
                        sample.generation,stats.intervals,
                        if stats.intervals==0{0}else{stats.min_begins},
                        stats.max_begins,
                        if stats.intervals==0{0}else{stats.min_ends},
                        stats.max_ends,stats.histogram,
                        stats.mismatch_intervals,stats.counter_regressions));
                }
            }
        }
    }
    result
}
unsafe extern "system" fn modern_surface_blt(this:*mut c_void,dst:*const c_void,
    src:*mut c_void,src_rect:*const c_void,flags:u32,fx:*mut c_void)->Hr {
    roffile_terminator_fix::probe();
    let Some(addr)=orig(this,SURFACE_BLT) else{return DDERR_GENERIC};
    FIRST_MODERN_BLT.call_once(||{
        log("RENDER-MODERN: first newer-surface Blt (observed; NOT FPS-paced)");
        log(&format!("SURFACE-TRACE: first modern Blt destination_caps={}",
            modern_surface_caps(this).map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into())));
        trace_game_windows("first modern Blt");
    });
    sample_render_calls("Blt",&MODERN_BLT_EVENTS,&MODERN_BLT_START,&MODERN_BLT_WINDOW);
    // The reproduced fatal 0xC0000005 inside Wine builtin ddraw+0x3B619
    // happens during this original Surface4::Blt call. A fixed 20k
    // entry limit proved insufficient during normal gameplay, so a
    // separate rolling trace now preserves the most recent calls.
    // Pointer values are logged but NEVER dereferenced by this tracer.
    let tracing=cfg().log_enabled && cfg().crash_blt_trace && CRASH_BLT_LOG.get().is_some();
    let trace_id=if tracing{
        NATIVE_BLT_CRASH_TRACE_ID.fetch_add(1,Ordering::Relaxed)+1
    }else{0};
    if tracing{
        crash_blt_line(trace_id,true,&format!(
            "CRASH-BLT: ENTER id={trace_id} wine_tid={:#06x} dest={this:p} src={src:p} dest_rect={dst:p} src_rect={src_rect:p} flags={flags:#010x} fx={fx:p}",
            win32::GetCurrentThreadId()));
    }
    // Wine assumes every IDirectDrawSurface4 source is its own native
    // struct ddraw_surface. A game-supplied synthetic Surface4Forwarder
    // must NOT cross this call boundary. The virtual primary contributes
    // its logical 640/800 RGB565 Lock/Unlock staging (not desktop primary).
    // Foreign/native Surface4 pointers continue through exactly unchanged.
    let (native_src,was_facade)=match
        com_surface4_forward::source_for_native_blt(src){
        Ok(resolved)=>resolved,
        Err(hr)=>{
            let n=NATIVE_BLT_SOURCE_UNWRAP_COUNT.fetch_add(1,Ordering::Relaxed)+1;
            if n<=20{log(&format!(
                "WINDOWED-GAME: native Blt rejected a retired/depth/unready virtual source count={n} hr={:#010x}",hr as u32));}
            if tracing{crash_blt_line(trace_id,false,&format!(
                "CRASH-BLT: REJECT id={trace_id} virtual_source=true hr={:#010x}",hr as u32));}
            return hr;
        }
    };
    if was_facade{
        let n=NATIVE_BLT_SOURCE_UNWRAP_COUNT.fetch_add(1,Ordering::Relaxed)+1;
        if n<=20 || n%120==0{
            log(&format!("WINDOWED-GAME: native Blt virtual Surface4 source safely unwrapped count={n} flags={flags:#010x}"));
        }
        if tracing{crash_blt_line(trace_id,false,&format!(
            "CRASH-BLT: UNWRAP id={trace_id} original_src={src:p} native_src={native_src:p}"));}
    }
    let call:BltFn=std::mem::transmute(addr);
    let hr=call(this,dst,native_src,src_rect,flags,fx);
    if tracing{
        crash_blt_line(trace_id,false,&format!("CRASH-BLT: EXIT id={trace_id} hr={:#010x}",hr as u32));
    }
    record_surface_usage(this,surface_usage::Method::Blt,hr);
    if surface_usage_enabled(){
        let role=surface_usage::classify(modern_surface_caps(this));
        if let Ok(mut audit)=BLT_SIGNATURE_AUDIT.lock(){
            audit.record(role,flags,!src.is_null(),hr);
        }
    }
    hr
}
unsafe extern "system" fn modern_surface_blt_fast(this:*mut c_void,x:u32,y:u32,
    src:*mut c_void,rect:*const c_void,flags:u32)->Hr {
    let Some(addr)=orig(this,SURFACE_BLT_FAST) else{return DDERR_GENERIC};
    FIRST_MODERN_FAST.call_once(||log("RENDER-MODERN: first newer-surface BltFast (observed; NOT FPS-paced)"));
    sample_render_calls("BltFast",&MODERN_BLTFAST_EVENTS,&MODERN_BLTFAST_START,&MODERN_BLTFAST_WINDOW);
    let call:BltFastFn=std::mem::transmute(addr);
    let hr=call(this,x,y,src,rect,flags);
    record_surface_usage(this,surface_usage::Method::BltFast,hr);
    hr
}
unsafe extern "system" fn modern_surface_unlock(this:*mut c_void,data:*const c_void)->Hr {
    let Some(addr)=orig(this,SURFACE_UNLOCK) else{return DDERR_GENERIC};
    FIRST_MODERN_UNLOCK.call_once(||{
        log("RENDER-MODERN: first newer-surface Unlock (observed; NOT FPS-paced)");
        trace_game_windows("first modern Unlock");
    });
    sample_render_calls("Unlock",&MODERN_UNLOCK_EVENTS,&MODERN_UNLOCK_START,&MODERN_UNLOCK_WINDOW);
    let call:UnlockFn=std::mem::transmute(addr);
    let hr=call(this,data);
    record_surface_usage(this,surface_usage::Method::Unlock,hr);
    hr
}

// Separate from normal scene counters: in this *opt-in* path we
// delegate each DirectDrawSurface2/3/4/7 method exactly once, then
// observe the HRESULT. We never read or change caller buffer contents,
// retain COM pointers, or affect the game-owned object lifecycle.
fn surface_usage_enabled()->bool {
    let settings=cfg();
    settings.log_enabled && settings.surface_usage_trace
}
unsafe fn record_surface_usage(this:*mut c_void,method:surface_usage::Method,hr:Hr){
    if !surface_usage_enabled(){return;}
    let role=surface_usage::classify(modern_surface_caps(this));
    if let Ok(mut audit)=SURFACE_USAGE_AUDIT.lock(){
        audit.record(role,method,hr);
    }
}
unsafe extern "system" fn audit_surface_add_attached_surface(this:*mut c_void,attachment:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_ADD_ATTACHED_SURFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,attachment);
    record_surface_usage(this,surface_usage::Method::AddAttachedSurface,hr);
    hr
}

unsafe extern "system" fn audit_surface_delete_attached_surface(this:*mut c_void,flags:u32,attachment:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_DELETE_ATTACHED_SURFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,u32,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,flags,attachment);
    record_surface_usage(this,surface_usage::Method::DeleteAttachedSurface,hr);
    hr
}

unsafe extern "system" fn audit_surface_get_dc(this:*mut c_void,dc:*mut *mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_GET_DC) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,dc);
    record_surface_usage(this,surface_usage::Method::GetDC,hr);
    hr
}

unsafe extern "system" fn audit_surface_is_lost(this:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_IS_LOST) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this);
    record_surface_usage(this,surface_usage::Method::IsLost,hr);
    hr
}

unsafe extern "system" fn audit_surface_lock(this:*mut c_void,rect:*const c_void,desc:*mut c_void,flags:u32,event:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_LOCK) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,u32,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,rect,desc,flags,event);
    record_surface_usage(this,surface_usage::Method::Lock,hr);
    hr
}

unsafe extern "system" fn audit_surface_release_dc(this:*mut c_void,dc:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_RELEASE_DC) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,dc);
    record_surface_usage(this,surface_usage::Method::ReleaseDC,hr);
    hr
}

unsafe extern "system" fn audit_surface_restore(this:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_RESTORE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this);
    record_surface_usage(this,surface_usage::Method::Restore,hr);
    hr
}

unsafe extern "system" fn audit_surface_set_clipper(this:*mut c_void,clipper:*mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_SET_CLIPPER) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,clipper);
    record_surface_usage(this,surface_usage::Method::SetClipper,hr);
    hr
}

unsafe extern "system" fn audit_surface_get_ddinterface(this:*mut c_void,out:*mut *mut c_void)->Hr {
    let Some(address)=orig(this,SURFACE_USAGE_GET_DDINTERFACE) else {return DDERR_GENERIC;};
    let original:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=std::mem::transmute(address);
    let hr=original(this,out);
    record_surface_usage(this,surface_usage::Method::GetDDInterface,hr);
    hr
}
pub(super) unsafe fn observe_modern_surface(surface:*mut c_void,source:&str){
    if surface.is_null(){return;}
    let native_brand=cfg().experimental_windowed_adapter &&
        diagnostics::is_native_windows();
    if !(cfg().log_enabled || cfg().modern_fps_pacing || native_brand){return;}
    // The experimental virtual Surface4 owns its own vtable. Never patch
    // its global 45-slot façade through the passive native-vtable tracer.
    if cfg().experimental_windowed_adapter && in_game_adapter::unwrap_target(surface).is_some(){
        return;
    }
    let qi=replace_method(surface,COM_QUERY_INTERFACE,surface_query_interface as *const () as usize);
    let flip=replace_method(surface,SURFACE_FLIP,modern_surface_flip as *const () as usize);
    // Normal native Windows gameplay needs only QI propagation and Flip
    // for the guarded MainMenu signature. Do not turn on all passive
    // tracing methods merely because Log=0 and branding is enabled.
    if !cfg().log_enabled && !cfg().modern_fps_pacing {
        return;
    }
    let blt=replace_method(surface,SURFACE_BLT,modern_surface_blt as *const () as usize);
    let fast=replace_method(surface,SURFACE_BLT_FAST,modern_surface_blt_fast as *const () as usize);
    let unlock=replace_method(surface,SURFACE_UNLOCK,modern_surface_unlock as *const () as usize);
    let attached=replace_method(surface,SURFACE_GET_ATTACHED,modern_get_attached_surface as *const () as usize);
    let desc=replace_method(surface,SURFACE_GET_DESC,modern_get_surface_desc as *const () as usize);
    if surface_usage_enabled(){
        let installed=[
            replace_method(surface,SURFACE_USAGE_ADD_ATTACHED_SURFACE,audit_surface_add_attached_surface as *const () as usize),
            replace_method(surface,SURFACE_USAGE_DELETE_ATTACHED_SURFACE,audit_surface_delete_attached_surface as *const () as usize),
            replace_method(surface,SURFACE_USAGE_GET_DC,audit_surface_get_dc as *const () as usize),
            replace_method(surface,SURFACE_USAGE_IS_LOST,audit_surface_is_lost as *const () as usize),
            replace_method(surface,SURFACE_USAGE_LOCK,audit_surface_lock as *const () as usize),
            replace_method(surface,SURFACE_USAGE_RELEASE_DC,audit_surface_release_dc as *const () as usize),
            replace_method(surface,SURFACE_USAGE_RESTORE,audit_surface_restore as *const () as usize),
            replace_method(surface,SURFACE_USAGE_SET_CLIPPER,audit_surface_set_clipper as *const () as usize),
            replace_method(surface,SURFACE_USAGE_GET_DDINTERFACE,audit_surface_get_ddinterface as *const () as usize),
        ];
        if MODERN_SURFACE_HOOK_TRACE_COUNT.load(Ordering::Relaxed)<24 {
            log(&format!("SURFACE-USAGE-HOOKS: installed={}/{} methods (opt-in; passive)",
                installed.iter().filter(|&&yes|yes).count(),installed.len()));
        }
    }
    if MODERN_SURFACE_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<24 {
        log(&format!("MODERN-SURFACE: from={source} query_interface={qi} flip={flip} blt={blt} blt_fast={fast} unlock={unlock} get_attached={attached} get_desc={desc} (observation only)"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_surface_callback_types_preserve_win32_abi(){
        let _:FlipFn=modern_surface_flip;
        let _:BltFn=modern_surface_blt;
        let _:BltFastFn=modern_surface_blt_fast;
        let _:UnlockFn=modern_surface_unlock;
        let _:GetAttachedSurfaceFn=modern_get_attached_surface;
        let _:GetSurfaceDescFn=modern_get_surface_desc;
    }

    #[test]
    fn null_surface_discovery_fails_closed(){
        unsafe {
            assert_eq!(modern_surface_caps(std::ptr::null_mut()),None);
            assert_eq!(modern_surface_primary(std::ptr::null_mut()),None);
            observe_modern_surface(std::ptr::null_mut(),"test");
            assert_eq!(modern_get_surface_desc(std::ptr::null_mut(),std::ptr::null_mut()),DDERR_GENERIC);
            assert_eq!(modern_get_attached_surface(std::ptr::null_mut(),std::ptr::null(),std::ptr::null_mut()),DDERR_GENERIC);
            assert_eq!(modern_surface_blt_fast(std::ptr::null_mut(),0,0,std::ptr::null_mut(),std::ptr::null(),0),DDERR_GENERIC);
            assert_eq!(modern_surface_unlock(std::ptr::null_mut(),std::ptr::null()),DDERR_GENERIC);
        }
    }

    #[test]
    fn passive_surface_audit_callbacks_keep_exact_com_abi() {
        let _:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=audit_surface_add_attached_surface;
        let _:unsafe extern "system" fn(*mut c_void,u32,*mut c_void)->Hr=audit_surface_delete_attached_surface;
        let _:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=audit_surface_get_dc;
        let _:unsafe extern "system" fn(*mut c_void)->Hr=audit_surface_is_lost;
        let _:unsafe extern "system" fn(*mut c_void,*const c_void,*mut c_void,u32,*mut c_void)->Hr=audit_surface_lock;
        let _:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=audit_surface_release_dc;
        let _:unsafe extern "system" fn(*mut c_void)->Hr=audit_surface_restore;
        let _:unsafe extern "system" fn(*mut c_void,*mut c_void)->Hr=audit_surface_set_clipper;
        let _:unsafe extern "system" fn(*mut c_void,*mut *mut c_void)->Hr=audit_surface_get_ddinterface;
    }

    #[test]
    fn passive_audit_callbacks_reject_null_com_without_dispatch() {
        let p=std::ptr::null_mut();
        unsafe {
            assert_eq!(audit_surface_add_attached_surface(p,p),DDERR_GENERIC);
            assert_eq!(audit_surface_delete_attached_surface(p,0,p),DDERR_GENERIC);
            assert_eq!(audit_surface_get_dc(p,std::ptr::null_mut()),DDERR_GENERIC);
            assert_eq!(audit_surface_is_lost(p),DDERR_GENERIC);
            assert_eq!(audit_surface_lock(p,std::ptr::null(),p,0,p),DDERR_GENERIC);
            assert_eq!(audit_surface_release_dc(p,p),DDERR_GENERIC);
            assert_eq!(audit_surface_restore(p),DDERR_GENERIC);
            assert_eq!(audit_surface_set_clipper(p,p),DDERR_GENERIC);
            assert_eq!(audit_surface_get_ddinterface(p,std::ptr::null_mut()),DDERR_GENERIC);
        }
    }

    #[test]
    fn modern_surface_hooks_preserve_dx6_slots(){
        assert_eq!(SURFACE_BLT,5);
        assert_eq!(SURFACE_BLT_FAST,7);
        assert_eq!(SURFACE_FLIP,11);
        assert_eq!(SURFACE_GET_ATTACHED,12);
        assert_eq!(SURFACE_GET_CAPS,14);
        assert_eq!(SURFACE_GET_DESC,22);
        assert_eq!(SURFACE_UNLOCK,32);
    }
}
