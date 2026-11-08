//! Native Windows diagnostic for the *actual* DirectDraw owner returned by
//! a presented primary surface's IDirectDrawSurface4::GetDDInterface.
//! Never infer interface ABI from the Surface4 version. Identify the exact
//! returned interface pointer by querying each DD version IID and comparing
//! COM pointer identity, releasing every temporary reference.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum OwnerVersion { DirectDraw1, DirectDraw2, DirectDraw4, DirectDraw7 }
impl OwnerVersion {
    fn label(self)->&'static str {
        match self {
            Self::DirectDraw1=>"IDirectDraw1",
            Self::DirectDraw2=>"IDirectDraw2",
            Self::DirectDraw4=>"IDirectDraw4",
            Self::DirectDraw7=>"IDirectDraw7",
        }
    }
    fn mode_uses_five_arguments(self)->bool {self!=Self::DirectDraw1}
}
const IID_DD1:Guid=Guid{
    d1:0x6c14db80,d2:0xa733,d3:0x11ce,d4:[0xa5,0x21,0x00,0x20,0xaf,0x0b,0xe5,0x60]
};
const IID_DD2:Guid=Guid{
    d1:0xb3a6f3e0,d2:0x2b43,d3:0x11cf,d4:[0xa2,0xde,0x00,0xaa,0x00,0xb9,0x33,0x56]
};
const IID_DD4:Guid=Guid{
    d1:0x9c59509a,d2:0x39bd,d3:0x11d1,d4:[0x8c,0x4a,0x00,0xc0,0x4f,0xd9,0x30,0xc5]
};
const IID_DD7:Guid=Guid{
    d1:0x15e65ec0,d2:0x3b9c,d3:0x11d2,d4:[0xb9,0x2f,0x00,0x60,0x97,0x97,0xea,0x5b]
};

type ReleaseFn=unsafe extern "system" fn(*mut c_void)->u32;
static COOPERATIVE_COUNT:AtomicUsize=AtomicUsize::new(0);
static THREE_MODE_COUNT:AtomicUsize=AtomicUsize::new(0);
static FIVE_MODE_COUNT:AtomicUsize=AtomicUsize::new(0);
static RESTORE_COUNT:AtomicUsize=AtomicUsize::new(0);

/// GetDDInterface returns whichever version CREATED the surface; never
/// assume IDirectDraw4 merely because we requested IDirectDrawSurface4.
/// A successful QI pointer comparison identifies the ABI of that exact
/// returned interface; this function NEVER stores or claims ownership.
pub(super) unsafe fn classify(owner:*mut c_void)->Option<OwnerVersion> {
    if owner.is_null(){return None;}
    let qi_addr=orig(owner,COM_QUERY_INTERFACE).unwrap_or_else(||method(owner,COM_QUERY_INTERFACE));
    if qi_addr==0{return None;}
    let query:QueryInterfaceFn=std::mem::transmute(qi_addr);
    for (iid,version) in [
        (&IID_DD7,OwnerVersion::DirectDraw7),
        (&IID_DD4,OwnerVersion::DirectDraw4),
        (&IID_DD2,OwnerVersion::DirectDraw2),
        (&IID_DD1,OwnerVersion::DirectDraw1),
    ] {
        let mut got:*mut c_void=std::ptr::null_mut();
        let hr=query(owner,iid,&mut got);
        let equal=hr>=0 && !got.is_null() && got==owner;
        if !got.is_null(){
            let release_addr=method(got,2);
            if release_addr!=0 {
                let release:ReleaseFn=std::mem::transmute(release_addr);
                release(got);
            }
        }
        if equal{return Some(version);}
    }
    None
}

/// Record calls on the REAL owner, without the current DD4 virtual adapter
/// or legacy borderless behavior. All original arguments and HRESULTs pass
/// unchanged. 32-bit IDirectDraw1 SetDisplayMode has THREE args; versions
/// 2/4/7 use FIVE. Mistaking these would corrupt the x86 stack.
unsafe extern "system" fn trace_cooperative(this:*mut c_void,hwnd:Hwnd,flags:u32)->Hr {
    let Some(addr)=orig(this,DD_SET_COOPERATIVE) else{return DDERR_GENERIC};
    let call:CooperativeFn=std::mem::transmute(addr);
    let trace=COOPERATIVE_COUNT.fetch_add(1,Ordering::Relaxed)<20;
    if trace {log(&format!("NATIVE-OWNER-CALL: SetCooperativeLevel ENTER hwnd={} flags={:#010x} mode_before={:?}",
        if hwnd.is_null(){"NULL"}else{"provided"},flags,win32::physical_primary_monitor_dimensions()));}
    let hr=call(this,hwnd,flags);
    if trace {log(&format!("NATIVE-OWNER-CALL: SetCooperativeLevel EXIT hr={:#010x} mode_after={:?}",
        hr as u32,win32::physical_primary_monitor_dimensions()));}
    hr
}
unsafe extern "system" fn trace_mode3(this:*mut c_void,w:u32,h:u32,bpp:u32)->Hr{
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:DisplayModeFn=std::mem::transmute(addr);
    let trace=THREE_MODE_COUNT.fetch_add(1,Ordering::Relaxed)<20;
    if trace {log(&format!("NATIVE-OWNER-CALL: DD1 SetDisplayMode ENTER {}x{}x{} mode_before={:?}",
        w,h,bpp,win32::physical_primary_monitor_dimensions()));}
    let hr=call(this,w,h,bpp);
    if trace {log(&format!("NATIVE-OWNER-CALL: DD1 SetDisplayMode EXIT hr={:#010x} mode_after={:?}",
        hr as u32,win32::physical_primary_monitor_dimensions()));}
    hr
}
unsafe extern "system" fn trace_mode5(
    this:*mut c_void,w:u32,h:u32,bpp:u32,refresh:u32,flags:u32
)->Hr{
    let Some(addr)=orig(this,DD_SET_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:ModernDisplayModeFn=std::mem::transmute(addr);
    let trace=FIVE_MODE_COUNT.fetch_add(1,Ordering::Relaxed)<20;
    if trace {log(&format!("NATIVE-OWNER-CALL: DD2+ SetDisplayMode ENTER {}x{}x{} refresh={} flags={:#x} mode_before={:?}",
        w,h,bpp,refresh,flags,win32::physical_primary_monitor_dimensions()));}
    let hr=call(this,w,h,bpp,refresh,flags);
    if trace {log(&format!("NATIVE-OWNER-CALL: DD2+ SetDisplayMode EXIT hr={:#010x} mode_after={:?}",
        hr as u32,win32::physical_primary_monitor_dimensions()));}
    hr
}
unsafe extern "system" fn trace_restore(this:*mut c_void)->Hr {
    let Some(addr)=orig(this,DD_RESTORE_DISPLAY_MODE) else{return DDERR_GENERIC};
    let call:RestoreModeFn=std::mem::transmute(addr);
    let trace=RESTORE_COUNT.fetch_add(1,Ordering::Relaxed)<12;
    if trace {log(&format!("NATIVE-OWNER-CALL: RestoreDisplayMode ENTER mode_before={:?}",
        win32::physical_primary_monitor_dimensions()));}
    let hr=call(this);
    if trace {log(&format!("NATIVE-OWNER-CALL: RestoreDisplayMode EXIT hr={:#010x} mode_after={:?}",
        hr as u32,win32::physical_primary_monitor_dimensions()));}
    hr
}
pub(super) unsafe fn instrument(owner:*mut c_void,version:OwnerVersion) {
    // Strictly diagnostic, native only. The returned pointer belongs to the
    // surface COM object; our caller holds one temporary AddRef while here.
    if owner.is_null() || !diagnostics::is_native_windows() || !cfg().log_enabled{return;}
    let coop=replace_method(owner,DD_SET_COOPERATIVE,trace_cooperative as usize);
    let mode=if version.mode_uses_five_arguments(){
        replace_method(owner,DD_SET_DISPLAY_MODE,trace_mode5 as usize)
    }else{
        replace_method(owner,DD_SET_DISPLAY_MODE,trace_mode3 as usize)
    };
    let restore=replace_method(owner,DD_RESTORE_DISPLAY_MODE,trace_restore as usize);
    log(&format!("NATIVE-OWNER-HOOK: interface={} cooperative={} set_display_mode={} restore_display_mode={} tracing_only=true",
        version.label(),coop,mode,restore));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn correct_com_abis_are_distinguished(){
        let _:CooperativeFn=trace_cooperative;
        let _:DisplayModeFn=trace_mode3;
        let _:ModernDisplayModeFn=trace_mode5;
        let _:RestoreModeFn=trace_restore;
        assert!(!OwnerVersion::DirectDraw1.mode_uses_five_arguments());
        assert!(OwnerVersion::DirectDraw2.mode_uses_five_arguments());
        assert!(OwnerVersion::DirectDraw4.mode_uses_five_arguments());
        assert!(OwnerVersion::DirectDraw7.mode_uses_five_arguments());
        assert_eq!(guid_label(&IID_DD4),"9c59509a-39bd-11d1-8c4a-00c04fd930c5");
    }
    #[test]
    fn null_owner_is_unclassifiable(){
        assert_eq!(unsafe{classify(std::ptr::null_mut())},None);
    }
}
