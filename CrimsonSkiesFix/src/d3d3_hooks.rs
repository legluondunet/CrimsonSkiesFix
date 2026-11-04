//! Original native IDirect3D3 and IDirect3DDevice3 callbacks.
//! Extracted from the DirectDraw proxy without changing the Win32 x86 ABI,
//! original COM AddRef/Release ownership, or native Surface4 remapping.
//! The game proxy still owns the shared COM registry and counters.
use super::*;

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
