//! Original native IDirect3D3 and IDirect3DDevice3 callbacks.
//! Extracted from the DirectDraw proxy without changing the Win32 x86 ABI,
//! original COM AddRef/Release ownership, or native Surface4 remapping.
//! The game proxy still owns the shared COM registry and counters.
use super::*;

#[cfg(feature="winmm-bootstrap")]
#[path="projected_y43_probe.rs"]
mod projected_y43_probe;

#[cfg(feature="winmm-bootstrap")]
#[path="d3d3_path_coverage.rs"]
mod d3d3_path_coverage;

// Read-only projection telemetry for the WinMM 1.1 widescreen investigation.
// Direct3DDevice3::GetTransform is vtable slot 26 in the DirectX 6 ABI;
// D3DTRANSFORMSTATE_PROJECTION = 3. No matrix is written back to the game.
#[cfg(feature="winmm-bootstrap")]
const D3D3_DEVICE_GET_TRANSFORM:usize=26;
#[cfg(feature="winmm-bootstrap")]
const D3DTRANSFORMSTATE_PROJECTION:u32=3;
#[cfg(feature="winmm-bootstrap")]
static PROJECTION_SAMPLE_CALLS:AtomicUsize=AtomicUsize::new(0);

#[cfg(feature="winmm-bootstrap")]
const D3D3_DEVICE_DRAW_PRIMITIVE:usize=28;
#[cfg(feature="winmm-bootstrap")]
const D3D3_DEVICE_DRAW_INDEXED_PRIMITIVE:usize=29;
#[cfg(feature="winmm-bootstrap")]
static PRIMITIVE_DRAW_CALLS:AtomicUsize=AtomicUsize::new(0);
#[cfg(feature="winmm-bootstrap")]
static INDEXED_DRAW_CALLS:AtomicUsize=AtomicUsize::new(0);

// DirectX 6 D3DFVF_XYZRHW=0x004: vertices are already transformed into
// screen/viewport space, so GetTransform(PROJECTION) need not affect them.
// Classify only the FVF bits; never read or rewrite caller vertex memory.
#[cfg(feature="winmm-bootstrap")]
fn vertex_position_mode(fvf:u32)->&'static str{
    match fvf & 0x0e {
        0x04=>"XYZRHW-pretransformed-screen",
        0x02=>"XYZ-model-space",
        _=>"other-or-blended",
    }
}

#[cfg(feature="winmm-bootstrap")]
fn log_vertex_call(api:&str,ordinal:usize,primitive:u32,fvf:u32,vertices:u32,hr:Hr) {
    // Repeated full per-primitive logs would overwhelm the game. Keep a
    // bounded prefix and periodic samples over a long mission.
    if ordinal<=12 || (ordinal<=32768 && ordinal%4096==0) {
        log(&format!(
            "D3D3-VERTEX: method={api} call={ordinal} primitive_type={primitive} fvf={fvf:#010x} position={} vertex_count={vertices} hr={hr:#010x}; observation only",
            vertex_position_mode(fvf)));
    }
}

#[cfg(feature="winmm-bootstrap")]
unsafe extern "system" fn d3d3_device_draw_primitive(
    this:*mut c_void,primitive:u32,fvf:u32,vertices:*const c_void,
    vertex_count:u32,flags:u32
)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_DRAW_PRIMITIVE) else{return DDERR_GENERIC};
    type DrawPrimitive=unsafe extern "system" fn(*mut c_void,u32,u32,*const c_void,u32,u32)->Hr;
    let original:DrawPrimitive=std::mem::transmute(addr);
    // Only for an explicitly opted-in, authenticated in-game 3D epoch:
    // clone pretransformed vertices and modify shadow Y, never user memory.
    // The shadow remains alive across the original COM Draw call.
    let shadow=projected_y43_probe::shadow_y_corrected(
        primitive,fvf,vertices,vertex_count,false
    );
    let payload=shadow.as_ref().map_or(vertices,|copy|copy.as_ptr().cast::<c_void>());
    let hr=original(this,primitive,fvf,payload,vertex_count,flags);
    if cfg().log_enabled {
        let ordinal=PRIMITIVE_DRAW_CALLS.fetch_add(1,Ordering::Relaxed)+1;
        d3d3_path_coverage::observe_plain(false,primitive,fvf,vertex_count);
        log_vertex_call("DrawPrimitive",ordinal,primitive,fvf,vertex_count,hr);
    }
    hr
}

#[cfg(feature="winmm-bootstrap")]
unsafe extern "system" fn d3d3_device_draw_indexed_primitive(
    this:*mut c_void,primitive:u32,fvf:u32,vertices:*const c_void,
    vertex_count:u32,indices:*const u16,index_count:u32,flags:u32
)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_DRAW_INDEXED_PRIMITIVE) else{return DDERR_GENERIC};
    type DrawIndexedPrimitive=unsafe extern "system" fn(
        *mut c_void,u32,u32,*const c_void,u32,*const u16,u32,u32)->Hr;
    let original:DrawIndexedPrimitive=std::mem::transmute(addr);
    let shadow=projected_y43_probe::shadow_y_corrected(
        primitive,fvf,vertices,vertex_count,true
    );
    let payload=shadow.as_ref().map_or(vertices,|copy|copy.as_ptr().cast::<c_void>());
    let hr=original(this,primitive,fvf,payload,vertex_count,indices,index_count,flags);
    if cfg().log_enabled {
        let ordinal=INDEXED_DRAW_CALLS.fetch_add(1,Ordering::Relaxed)+1;
        d3d3_path_coverage::observe_plain(true,primitive,fvf,vertex_count);
        log_vertex_call("DrawIndexedPrimitive",ordinal,primitive,fvf,vertex_count,hr);
    }
    hr
}


#[cfg(feature="winmm-bootstrap")]
fn projection_aspect(m:&[f32;16])->Option<f32>{
    let (horizontal,vertical)=(m[0],m[5]);
    if !horizontal.is_finite() || !vertical.is_finite() ||
       horizontal.abs()<0.00001 || vertical.abs()<0.00001 {return None;}
    let aspect=(vertical/horizontal).abs();
    (aspect.is_finite() && (0.2..=6.0).contains(&aspect)).then_some(aspect)
}

#[cfg(feature="winmm-bootstrap")]
unsafe fn sample_game_projection(this:*mut c_void,when:&str){
    if !cfg().log_enabled{return;}
    let count=PROJECTION_SAMPLE_CALLS.fetch_add(1,Ordering::Relaxed)+1;
    d3d3_path_coverage::periodic_summary(count);
    // Capture startup, menu, and subsequent gameplay, but cap the log.
    if count>48 && !count.is_multiple_of(480){return;}
    if count>7680{return;}
    let slot=method(this,D3D3_DEVICE_GET_TRANSFORM);
    if slot==0 {return;}
    type GetTransform=unsafe extern "system" fn(*mut c_void,u32,*mut f32)->Hr;
    let call:GetTransform=std::mem::transmute(slot);
    let mut matrix=[0.0f32;16];
    let hr=call(this,D3DTRANSFORMSTATE_PROJECTION,matrix.as_mut_ptr());
    if hr<0 {
        if count<=3 {log(&format!("D3D3-PROJECTION: sample={count} phase={when} GetTransform returned {:#010x}; observation only",hr as u32));}
        return;
    }
    let aspect=projection_aspect(&matrix);
    log(&format!(
        "D3D3-PROJECTION: sample={count} phase={when} state=3 m11={:.6} m22={:.6} matrix_aspect={} target_screen_aspect=1.777778 draw_primitive_calls={} indexed_draw_calls={}; observation only, FOV not modified",
        matrix[0],matrix[5],aspect.map(|v|format!("{v:.6}")).unwrap_or_else(||"unavailable".into()),
        PRIMITIVE_DRAW_CALLS.load(Ordering::Relaxed),
        INDEXED_DRAW_CALLS.load(Ordering::Relaxed)));
}

// Crimson Skies' IDirectDraw4::QueryInterface successfully requests
// IID_IDirect3D3. This is the legacy Direct3D3 interface, NOT Direct3D9.
// The attached device's legacy Direct3D3 interface is traced only after
// the original CreateDevice succeeds. The genuine device and surfaces
// are still owned by the game: no replacement objects or AddRef/Release.
unsafe extern "system" fn d3d3_device_begin_scene(this:*mut c_void)->Hr {
    roffile_terminator_fix::probe();
    let Some(addr)=orig(this,D3D3_DEVICE_BEGIN_SCENE) else{return DDERR_GENERIC};
    let original:D3D3SceneFn=std::mem::transmute(addr);
    let hr=original(this);
    #[cfg(feature="winmm-bootstrap")]
    if hr>=0 {sample_game_projection(this,"BeginScene");}
    if cfg().log_enabled && hr>=0 {
        FIRST_D3D3_SCENE.call_once(||log(
            "D3D3-DEVICE: first successful BeginScene; game 3D rendering active"));
        sample_render_calls("D3D3.BeginScene",&D3D3_BEGIN_EVENTS,&D3D3_BEGIN_START,&D3D3_BEGIN_WINDOW);
    }
    hr
}

unsafe extern "system" fn d3d3_device_end_scene(this:*mut c_void)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_END_SCENE) else{return DDERR_GENERIC};
    let original:D3D3SceneFn=std::mem::transmute(addr);
    let hr=original(this);
    #[cfg(feature="winmm-bootstrap")]
    if hr>=0 {sample_game_projection(this,"EndScene");}
    if cfg().log_enabled && hr>=0 {
        sample_render_calls("D3D3.EndScene",&D3D3_END_EVENTS,&D3D3_END_START,&D3D3_END_WINDOW);
    }
    hr
}

unsafe extern "system" fn d3d3_device_set_render_target(
    this:*mut c_void,target:*mut c_void,flags:u32
)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_SET_RENDER_TARGET) else{return DDERR_GENERIC};
    let original:D3D3SetRenderTargetFn=std::mem::transmute(addr);
    let native=if cfg().experimental_windowed_adapter {
        in_game_adapter::unwrap_target(target).unwrap_or(target)
    }else{target};
    let hr=original(this,native,flags);
    if cfg().log_enabled && D3D3_RENDER_TARGET_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<16{
        let caps=if hr>=0 {modern_surface_caps(target)} else {None};
        log(&format!(
            "D3D3-DEVICE: SetRenderTarget flags={flags:#x} result={hr:#x} target_caps={} role={} (unchanged)",
            caps.map(|x|format!("{x:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into())));
    }
    hr
}

unsafe extern "system" fn d3d3_device_get_render_target(
    this:*mut c_void,out:*mut *mut c_void
)->Hr {
    let Some(addr)=orig(this,D3D3_DEVICE_GET_RENDER_TARGET) else{return DDERR_GENERIC};
    let original:D3D3GetRenderTargetFn=std::mem::transmute(addr);
    let hr=original(this,out);
    if hr>=0 && cfg().experimental_windowed_adapter && !out.is_null() && !(*out).is_null(){
        if let Some(virtual_target)=in_game_adapter::remap_render_target(*out){
            let original_ref=*out;
            let release:unsafe extern "system" fn(*mut c_void)->u32=
                std::mem::transmute(method(original_ref,2));
            let _=release(original_ref);
            *out=virtual_target;
            if cfg().log_enabled {log("WINDOWED-GAME: D3D3 GetRenderTarget mapped native RGB565 backbuffer back to game-facing Surface4 identity");}
        }
    }
    hr
}
unsafe fn observe_d3d3_device(device:*mut c_void){
    if device.is_null() || !(cfg().log_enabled || cfg().experimental_windowed_adapter){return;}
    let begin=replace_method(device,D3D3_DEVICE_BEGIN_SCENE,d3d3_device_begin_scene as *const () as usize);
    let end=replace_method(device,D3D3_DEVICE_END_SCENE,d3d3_device_end_scene as *const () as usize);
    let target=replace_method(device,D3D3_DEVICE_SET_RENDER_TARGET,d3d3_device_set_render_target as *const () as usize);
    let get_target=replace_method(device,D3D3_DEVICE_GET_RENDER_TARGET,d3d3_device_get_render_target as *const () as usize);
    #[cfg(feature="winmm-bootstrap")]
    if cfg().log_enabled {
        let draw=replace_method(device,D3D3_DEVICE_DRAW_PRIMITIVE,
            d3d3_device_draw_primitive as *const () as usize);
        let indexed=replace_method(device,D3D3_DEVICE_DRAW_INDEXED_PRIMITIVE,
            d3d3_device_draw_indexed_primitive as *const () as usize);
        // Four additional DX6 vertex-buffer/strided entrypoints are
        // strictly pass-through and only serve render-path coverage logs.
        d3d3_path_coverage::install(device);
        if D3D3_DEVICE_HOOK_TRACE_COUNT.load(Ordering::Relaxed)<12 {
            log(&format!("D3D3-VERTEX: hooks DrawPrimitive={draw} DrawIndexedPrimitive={indexed} (pass-through read-only telemetry)"));
        }
    }
    if D3D3_DEVICE_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!(
            "D3D3-DEVICE: hooks BeginScene={begin} EndScene={end} SetRenderTarget={target} GetRenderTarget={get_target} (opt-in adapter support)"));
    }
}

// In Wine d3d.h, IDirect3D3 vtable slot 8 is
// CreateDevice(REFCLSID, IDirectDrawSurface4*, IDirect3DDevice3**, IUnknown*).
// This exact Win32 x86 ABI is observed *without* changing the device.
unsafe extern "system" fn d3d3_create_device(
    this:*mut c_void,device_clsid:*const Guid,target:*mut c_void,
    out:*mut *mut c_void,outer:*mut c_void
)->Hr {
    let Some(addr)=orig(this,D3D3_CREATE_DEVICE) else{return DDERR_GENERIC};
    let call:D3D3CreateDeviceFn=std::mem::transmute(addr);
    let native_target=if cfg().experimental_windowed_adapter {
        in_game_adapter::unwrap_target(target).unwrap_or(target)
    }else{target};
    let hr=call(this,device_clsid,native_target,out,outer);
    if (cfg().log_enabled || cfg().experimental_windowed_adapter) && hr>=0 && !out.is_null() && !(*out).is_null() {
        observe_d3d3_device(*out);
    }
    if cfg().log_enabled && D3D3_CREATE_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        // If CreateDevice rejects a surface, the supplied target may not be
        // valid for post-call probing. Only inspect after success.
        let caps=if hr>=0 {modern_surface_caps(target)} else {None};
        log(&format!(
            "D3D3-TRACE: CreateDevice clsid={} render_target_caps={} role={} hr={hr:#x} object_returned={} (unchanged)",
            guid_label(device_clsid),
            caps.map(|c|format!("{c:#010x}")).unwrap_or_else(||"Unavailable".into()),
            caps.map(surface_roles).unwrap_or_else(||"unknown".into()),
            hr>=0 && !out.is_null() && !(*out).is_null()));
        // Never lock the render target or modify its 3D state.
        if hr>=0 { probe_modern_surface_layout(target,"D3D3::CreateDevice target"); }
    }
    hr
}

pub(super) unsafe fn observe_d3d3(object:*mut c_void) {
    if object.is_null() || !(cfg().log_enabled || cfg().experimental_windowed_adapter){return;}
    let hooked=replace_method(object,D3D3_CREATE_DEVICE,d3d3_create_device as *const () as usize);
    if D3D3_HOOK_TRACE_COUNT.fetch_add(1,Ordering::Relaxed)<12 {
        log(&format!("D3D3-TRACE: IDirect3D3 CreateDevice hook_installed={hooked} (passive)"));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature="winmm-bootstrap")]
    #[test]
    fn draw_primitive_fvf_probe_is_read_only_and_abi_correct() {
        assert_eq!(D3D3_DEVICE_DRAW_PRIMITIVE,28);
        assert_eq!(D3D3_DEVICE_DRAW_INDEXED_PRIMITIVE,29);
        assert_eq!(vertex_position_mode(0x004),"XYZRHW-pretransformed-screen");
        assert_eq!(vertex_position_mode(0x144),"XYZRHW-pretransformed-screen");
        assert_eq!(vertex_position_mode(0x002),"XYZ-model-space");
        assert_eq!(vertex_position_mode(0x006),"other-or-blended");
        let _:unsafe extern "system" fn(
            *mut c_void,u32,u32,*const c_void,u32,u32)->Hr
            =d3d3_device_draw_primitive;
        let _:unsafe extern "system" fn(
            *mut c_void,u32,u32,*const c_void,u32,*const u16,u32,u32)->Hr
            =d3d3_device_draw_indexed_primitive;
    }

    #[cfg(feature="winmm-bootstrap")]
    #[test]
    fn projection_probe_reports_aspect_without_mutation() {
        let mut matrix=[0.0f32;16];
        matrix[0]=0.75;matrix[5]=1.0;
        assert_eq!(projection_aspect(&matrix),Some(4.0/3.0));
        matrix[0]=0.5625;
        assert_eq!(projection_aspect(&matrix),Some(16.0/9.0));
        matrix[0]=0.0;
        assert_eq!(projection_aspect(&matrix),None);
        matrix[0]=f32::NAN;
        assert_eq!(projection_aspect(&matrix),None);
        // Do not modify or hook SetTransform; GetTransform is read-only.
        assert_eq!(D3D3_DEVICE_GET_TRANSFORM,26);
        assert_eq!(D3DTRANSFORMSTATE_PROJECTION,3);
    }

    #[test]
    fn callbacks_keep_original_win32_x86_abi() {
        let _:D3D3SceneFn=d3d3_device_begin_scene;
        let _:D3D3SceneFn=d3d3_device_end_scene;
        let _:D3D3SetRenderTargetFn=d3d3_device_set_render_target;
        let _:D3D3GetRenderTargetFn=d3d3_device_get_render_target;
        let _:D3D3CreateDeviceFn=d3d3_create_device;
    }

    #[test]
    fn d3d3_slots_match_directx6_abi() {
        assert_eq!(D3D3_CREATE_DEVICE,8);
        assert_eq!(D3D3_DEVICE_BEGIN_SCENE,9);
        assert_eq!(D3D3_DEVICE_END_SCENE,10);
        assert_eq!(D3D3_DEVICE_SET_RENDER_TARGET,14);
        assert_eq!(D3D3_DEVICE_GET_RENDER_TARGET,15);
    }

    #[test]
    fn null_device_observation_is_fail_closed() {
        // Important: null must return before configuration initializes
        // or an attempt is made to read/write a COM interface.
        unsafe {
            observe_d3d3(std::ptr::null_mut());
            observe_d3d3_device(std::ptr::null_mut());
        }
    }
}
