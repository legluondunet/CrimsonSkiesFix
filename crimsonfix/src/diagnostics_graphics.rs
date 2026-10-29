//! Read-only D3D9/XInput/DirectInput8 hardware inventory.
//! Pure Rust/Win32 FFI port of SpiderMan2k1Fix system_diagnostics.c
//! (GPL-3.0-or-later). Never acquire a gamepad, create a D3D device,
//! set vibration or enumerate private device paths.

use std::{ffi::{c_char,c_void}, mem,ptr};
use crate::diagnostics::clean_label;

#[repr(C)]
#[derive(Clone,Copy,Default)]
struct Guid { a:u32,b:u16,c:u16,d:[u8;8] }

#[repr(C)]
struct D3dIdentifier {
    driver:[c_char;512],
    description:[c_char;512],
    device_name:[c_char;32],
    driver_version:i64,
    vendor_id:u32,device_id:u32,subsystem_id:u32,revision:u32,
    device_guid:Guid,
    whql_level:u32,
}

#[repr(C)]
#[derive(Default)]
struct XinputGamepad {
    buttons:u16,left_trigger:u8,right_trigger:u8,
    thumb_lx:i16,thumb_ly:i16,thumb_rx:i16,thumb_ry:i16,
}
#[repr(C)]
#[derive(Default)]
struct XinputCapabilities {
    kind:u8,subtype:u8,flags:u16,
    gamepad:XinputGamepad,vibration_left:u16,vibration_right:u16,
}
#[repr(C)]
struct DInputDeviceInstanceA {
    size:u32,guid_instance:Guid,guid_product:Guid,device_type:u32,
    instance_name:[c_char;260],product_name:[c_char;260],
    ff_driver:Guid,usage_page:u16,usage:u16,
}

#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryA(name:*const c_char)->*mut c_void;
    fn FreeLibrary(module:*mut c_void)->i32;
    fn GetModuleHandleA(name:*const c_char)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
}

unsafe fn load(name:&[u8])->*mut c_void { LoadLibraryA(name.as_ptr().cast()) }
unsafe fn symbol(module:*mut c_void,name:&[u8])->*const c_void {
    if module.is_null(){ptr::null()}else{GetProcAddress(module,name.as_ptr().cast())}
}
unsafe fn com_entry(object:*mut c_void,index:usize)->Option<*const c_void> {
    if object.is_null(){return None;}
    let vt=*(object as *const *const *const c_void);
    if vt.is_null(){return None;}
    let result=*vt.add(index);
    (!result.is_null()).then_some(result)
}
unsafe fn com_release(object:*mut c_void){
    if let Some(entry)=com_entry(object,2){
        let f:unsafe extern "system" fn(*mut c_void)->u32=mem::transmute(entry);
        f(object);
    }
}
fn c_array(value:&[c_char])->String {
    let cap=value.iter().position(|b|*b==0).unwrap_or(value.len());
    let buf=value[..cap].iter().map(|&x|x as u8).collect::<Vec<u8>>();
    String::from_utf8_lossy(&buf).to_string()
}

pub fn log_graphics(emit:&mut dyn FnMut(&str)){
    unsafe {
        type Direct3DCreate9=unsafe extern "system" fn(u32)->*mut c_void;
        type GetAdapterCount=unsafe extern "system" fn(*mut c_void)->u32;
        type GetAdapterIdentifier=unsafe extern "system" fn(*mut c_void,u32,u32,*mut D3dIdentifier)->i32;
        let lib=load(b"d3d9.dll\0");
        let create=symbol(lib,b"Direct3DCreate9\0");
        let object=if create.is_null(){ptr::null_mut()}else{
            let f:Direct3DCreate9=mem::transmute(create);
            f(32) // D3D_SDK_VERSION
        };
        if object.is_null(){
            emit("SYS-GPU: D3D9 adapter=Unavailable (Direct3DCreate9)");
            emit("SYS-GRAPHICS-DRIVER: Unavailable (Windows API does not expose host Linux Mesa/AMDGPU/NVIDIA version)");
        }else{
            if let (Some(count_entry),Some(id_entry))=(com_entry(object,4),com_entry(object,5)){
                let get_count:GetAdapterCount=mem::transmute(count_entry);
                let get_id:GetAdapterIdentifier=mem::transmute(id_entry);
                let count=get_count(object);
                if count==0 {emit("SYS-GPU: D3D9 reports zero adapters");}
                for i in 0..count.min(8) {
                    let mut id:D3dIdentifier=mem::zeroed();
                    if get_id(object,i,0,&mut id)<0{continue;}
                    let device=clean_label(&c_array(&id.description),175);
                    let driver=clean_label(&c_array(&id.driver),125);
                    emit(&format!("SYS-GPU: D3D9 adapter={} model={} vendor=0x{:04X} device=0x{:04X}",
                        i,device,id.vendor_id,id.device_id));
                    let hi=(id.driver_version as u64>>32) as u32;
                    let lo=id.driver_version as u32;
                    emit(&format!("SYS-GRAPHICS-DRIVER: D3D9 adapter={} reported_module={} reported_version={}.{}.{}.{} (may be Wine/DXVK wrapper, NOT Linux kernel/Mesa driver)",
                        i,driver,hi>>16,hi&0xffff,lo>>16,lo&0xffff));
                }
            }else{
                emit("SYS-GPU: D3D9 adapter=Unavailable (COM method lookup)");
            }
            com_release(object);
        }
        if !lib.is_null(){FreeLibrary(lib);}
    }
}

struct PadResult { descriptions:Vec<String>, total:u32 }
unsafe extern "system" fn list_dinput_device(p:*const DInputDeviceInstanceA, context:*mut c_void)->i32 {
    if p.is_null()||context.is_null(){return 0;}
    let result=&mut *(context as *mut PadResult);
    result.total+=1;
    if result.descriptions.len()>=12{return 0;} // DIENUM_STOP
    let item=&*p;
    let name=clean_label(&c_array(&item.product_name),160);
    // DirectInput GUID Data1's lower 16 bits are VID, upper 16 PID.
    result.descriptions.push(format!(
        "SYS-CONTROLLER: API=DirectInput8 slot={} model={} VID:PID={:04X}:{:04X} (enumeration only, not acquired)",
        result.descriptions.len()+1,name,item.guid_product.a&0xffff,item.guid_product.a>>16));
    1 // DIENUM_CONTINUE
}

pub fn log_controllers(emit:&mut dyn FnMut(&str)){
    unsafe {
        type XInputGetCapabilities=unsafe extern "system" fn(u32,u32,*mut XinputCapabilities)->u32;
        let mut xinput=ptr::null_mut();
        let mut get_caps=ptr::null();
        for name in [b"xinput1_4.dll\0".as_slice(),b"xinput1_3.dll\0",b"xinput9_1_0.dll\0"] {
            let lib=load(name);
            if lib.is_null(){continue;}
            let entry=symbol(lib,b"XInputGetCapabilities\0");
            if !entry.is_null(){
                xinput=lib;
                get_caps=entry;
                break;
            }
            FreeLibrary(lib);
        }
        let mut xcount=0;
        if !get_caps.is_null(){
            let caps:XInputGetCapabilities=mem::transmute(get_caps);
            for slot in 0..4 {
                let mut value=XinputCapabilities::default();
                if caps(slot,0,&mut value)==0{
                    emit(&format!("SYS-CONTROLLER: API=XInput slot={} subtype={} model=Unavailable (XInput does not supply product name)",
                        slot,value.subtype));
                    xcount+=1;
                }
            }
        }
        if !xinput.is_null(){FreeLibrary(xinput);}

        type DirectInput8Create=unsafe extern "system" fn(
            *mut c_void,u32,*const Guid,*mut *mut c_void,*mut c_void)->i32;
        type EnumDevices=unsafe extern "system" fn(
            *mut c_void,u32,unsafe extern "system" fn(*const DInputDeviceInstanceA,*mut c_void)->i32,
            *mut c_void,u32)->i32;
        let dinput=load(b"dinput8.dll\0");
        let create=symbol(dinput,b"DirectInput8Create\0");
        let mut pads=PadResult{descriptions:Vec::new(),total:0};
        let mut success=false;
        if !create.is_null(){
            // IID_IDirectInput8A = BF798030-483A-4DA2-AA99-5D64ED369700
            let iid=Guid{a:0xBF798030,b:0x483A,c:0x4DA2,
                d:[0xAA,0x99,0x5D,0x64,0xED,0x36,0x97,0x00]};
            let make:DirectInput8Create=mem::transmute(create);
            let mut object=ptr::null_mut();
            if make(GetModuleHandleA(ptr::null()),0x0800,&iid,&mut object,ptr::null_mut())>=0
               && !object.is_null(){
                // IDirectInput8::EnumDevices is COM vtable slot 4.
                if let Some(entry)=com_entry(object,4) {
                    let enumerate:EnumDevices=mem::transmute(entry);
                    success=enumerate(object,4 /* DI8DEVCLASS_GAMECTRL */,
                        list_dinput_device,&mut pads as *mut PadResult as *mut c_void,
                        1 /* DIEDFL_ATTACHEDONLY */)>=0;
                }
                com_release(object);
            }
        }
        if !dinput.is_null(){FreeLibrary(dinput);}
        if !success{emit("SYS-CONTROLLER: DirectInput8 enumeration unavailable");}
        for line in &pads.descriptions{emit(line);}
        emit(&format!("SYS-CONTROLLER: totals XInput={} DirectInput8={} (an XInput pad may also appear in DirectInput; do not add counts)",
            xcount,pads.total));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verify_dx_ffi_layouts(){
        if cfg!(target_pointer_width="32"){
            assert_eq!(mem::size_of::<Guid>(),16);
            assert_eq!(mem::size_of::<D3dIdentifier>(),1104);
            assert_eq!(mem::size_of::<XinputCapabilities>(),20);
            assert_eq!(mem::size_of::<DInputDeviceInstanceA>(),580);
        }
    }
    #[test]
    fn sanitize_driver_paths(){
        assert_eq!(clean_label("C:\\private\\driver.dll",150),"Unavailable");
        assert_eq!(clean_label("nvldumd.dll",150),"nvldumd.dll");
    }
}
