//! Passive, fail-closed audit of Crimson Skies 1.02's software camera.
//!
//! Based on byte-authenticated owner-supplied decrypted EXE disassembly:
//! VA 0x0042B570 converts camera FOV using H/W global [0x0071B3F8] and
//! passes two angles to 0x004D2C20. That routine verifies object type 1,
//! retrieves the camera backing pointer via [+0x38], and writes its
//! floating coefficients at [+0xE0..+0xF4] and reciprocal tangent
//! parameters at [+0x1D4/+0x1D8].
//!
//! Later Direct3D3 XYZRHW vertices are *already* projected. Reading the
//! real upstream camera coefficients lets us study clipping agreement,
//! instead of guessing a third screen-space polygon correction.
//! NO memory or render-state writes, detours, COM calls or DLL changes.
use std::{
    ffi::c_void,
    mem::{size_of,MaybeUninit},
    ptr,
};
const WRAPPER_GLOBAL:usize=0x0064_EF78;
const EXE_IMAGE_BASE:usize=0x0040_0000;
const MEM_COMMIT:u32=0x1000;
const PAGE_GUARD:u32=0x100;
const PAGE_NOACCESS:u32=0x01;
const MAX_CAMERA_VALUE:f32=1_000_000.0;

#[repr(C)]
struct MemoryBasicInformation{
    base_address:*mut c_void,
    allocation_base:*mut c_void,
    allocation_protect:u32,
    region_size:usize,
    state:u32,
    protect:u32,
    kind:u32,
}
#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn VirtualQuery(address:*const c_void,
        buffer:*mut MemoryBasicInformation,length:usize)->usize;
}
unsafe fn readable(addr:usize,bytes:usize)->bool{
    if addr==0 || bytes==0{return false;}
    let Some(end)=addr.checked_add(bytes)else{return false;};
    let mut info=MaybeUninit::<MemoryBasicInformation>::zeroed();
    if VirtualQuery(addr as *const c_void,
        info.as_mut_ptr(),size_of::<MemoryBasicInformation>())
        !=size_of::<MemoryBasicInformation>() {return false;}
    let info=info.assume_init();
    if info.state!=MEM_COMMIT || info.protect&PAGE_GUARD!=0 ||
       info.protect&0xff==PAGE_NOACCESS ||
       !matches!(info.protect&0xff,0x02|0x04|0x08|0x20|0x40|0x80){
        return false;
    }
    let start=info.base_address as usize;
    let Some(region_end)=start.checked_add(info.region_size)else{return false;};
    start<=addr && end<=region_end
}
unsafe fn u32_at(addr:usize)->Option<u32>{
    if addr%4!=0 || !readable(addr,4){return None;}
    Some(ptr::read_volatile(addr as *const u32))
}
unsafe fn f32_at(addr:usize)->Option<f32>{
    let x=f32::from_bits(u32_at(addr)?);
    (x.is_finite() && x.abs()<=MAX_CAMERA_VALUE).then_some(x)
}
fn printable(v:Option<f32>)->String{
    v.map_or_else(||"unavailable".into(),|f|format!("{f:.7}"))
}
fn ratio(a:Option<f32>,b:Option<f32>)->Option<f32>{
    let (x,y)=(a?,b?);
    if y.abs()<1e-8{return None;}
    let value=x/y;
    (value.is_finite() && value.abs()<1e6).then_some(value)
}
pub(super) unsafe fn sample(when:&str,ordinal:usize){
    // Only called at already-bounded D3D3 projection telemetry samples.
    if !crate::cfg().log_enabled || !crate::cfg().experimental_projected_y43_probe{return;}
    let Some((w,h))=crate::in_game_adapter::active_verified_widescreen_gameplay_raster()
        else{return;};
    if crate::native_widescreen::verified_launch_widescreen_dimensions()
       !=Some((w,h)) {return;}
    if GetModuleHandleW(ptr::null()) as usize!=EXE_IMAGE_BASE{return;}
    let Some(wrapper)=u32_at(WRAPPER_GLOBAL).map(|n|n as usize)else{return;};
    let Some(iface)=u32_at(wrapper).map(|n|n as usize)else{return;};
    let object_type=u32_at(iface.saturating_add(0x34));
    if object_type!=Some(1){
        if ordinal<=4 {
            crate::log(&format!(
                "CAMERA-UPSTREAM: sample={ordinal} phase={when} mode={w}x{h} object_type={object_type:?} camera_state=unavailable read_only=true"
            ));
        }
        return;
    }
    let Some(camera)=u32_at(iface.saturating_add(0x38))
        .map(|n|n as usize)else{return;};
    // Validate all 0x1DC bytes in a single page before field reads.
    if !readable(camera,0x1dc){return;}
    let angle_x=f32_at(camera+0xe0);
    let angle_y=f32_at(camera+0xe4);
    let horizontal_div=f32_at(camera+0xe8);
    let vertical_div=f32_at(camera+0xec);
    let xcoeff=f32_at(camera+0x1d4);
    let ycoeff=f32_at(camera+0x1d8);
    let aspect_global=f32_at(0x0071_B3F8);
    crate::log(&format!(
        "CAMERA-UPSTREAM: sample={ordinal} phase={when} mode={w}x{h} \
         angle_x={} angle_y={} screen_coeff_x={} screen_coeff_y={} \
         inverse_tan_x={} inverse_tan_y={} upstream_aspect_h_over_w={} \
         camera_coeff_ratio={} original_camera_unchanged=true read_only=true",
        printable(angle_x),printable(angle_y),
        printable(horizontal_div),printable(vertical_div),
        printable(xcoeff),printable(ycoeff),printable(aspect_global),
        printable(ratio(horizontal_div,vertical_div))
    ));
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn sanity_filter_keeps_valid_camera_values_only(){
        assert_eq!(ratio(Some(4.0),Some(2.0)),Some(2.0));
        assert_eq!(ratio(Some(4.0),Some(0.0)),None);
        assert_eq!(ratio(None,Some(3.0)),None);
        assert_eq!(printable(None),"unavailable");
    }
    #[test] fn struct_has_expected_windows_x86_abi_layout(){
        #[cfg(target_pointer_width="32")]
        assert_eq!(size_of::<MemoryBasicInformation>(),28);
    }
}
