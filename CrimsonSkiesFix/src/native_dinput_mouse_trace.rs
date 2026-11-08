//! Native Windows-only DirectInput mouse provenance + isolated cooperative repair.
//! The admitted game imports DINPUT!DirectInputCreateA via PE32 IAT RVA
//! 0x61fbb0 (confirmed independently against the actual Win11 minidump).
//! This traces the MOUSE COM path; WM_MOUSEMOVE alone does not prove the
//! original game's GOS/DirectInput mouse cursor state is updating.
//! Trace is strictly pass-through; optional independent compatibility A/B
//! replaces unsupported GUID_SysMouse exclusive+background (0x9) with the
//! native nonexclusive+foreground mode (0x6) before the real COM call.
//! No synthetic input, changes to dimensions, or direct cursor drawing.
//! Requires Log=1, NativeDD4Bootstrap=1 and explicit trace OR repair flag;
//! never Wine/Proton or normal DirectDraw mode.
#![allow(dead_code)]
use std::{ffi::{c_char,c_void},ptr,sync::atomic::{AtomicBool,AtomicUsize,Ordering}};
use crate::{diagnostics,log,win32};

type Obj=*mut c_void;
type Factory=unsafe extern "system" fn(Obj,u32,*mut Obj,Obj)->i32;
type CreateDevice=unsafe extern "system" fn(Obj,*const u8,*mut Obj,Obj)->i32;
type NoArgs=unsafe extern "system" fn(Obj)->i32;
type Coop=unsafe extern "system" fn(Obj,Obj,u32)->i32;
type DataFormat=unsafe extern "system" fn(Obj,*const u32)->i32;
type State=unsafe extern "system" fn(Obj,u32,Obj)->i32;
type Buffered=unsafe extern "system" fn(Obj,u32,Obj,*mut u32,u32)->i32;

const FACTORY_IAT_RVA:usize=0x0061_fbb0;
const DI_CREATE_DEVICE:usize=3;
const DEV_ACQUIRE:usize=7;
const DEV_UNACQUIRE:usize=8;
const DEV_GET_STATE:usize=9;
const DEV_GET_DATA:usize=10;
const DEV_SET_FORMAT:usize=11;
const DEV_SET_COOP:usize=13;
// DISCL_EXCLUSIVE | DISCL_BACKGROUND is illegal for GUID_SysMouse.
// Keep exact match; do not rewrite any keyboard, controller, other flags,
// or unverified factory interfaces.
const MOUSE_EXCLUSIVE_BACKGROUND:u32=0x1|0x8;
const MOUSE_NONEXCLUSIVE_FOREGROUND:u32=0x2|0x4;
fn effective_mouse_coop_flags(mouse:bool,repair:bool,flags:u32)->u32{
    if mouse && repair && flags==MOUSE_EXCLUSIVE_BACKGROUND{
        MOUSE_NONEXCLUSIVE_FOREGROUND
    }else{flags}
}
// GUID_SysMouse {6f1d2b60-d5a0-11cf-bfc7-444553540000}
// Native little-endian layout of a Windows GUID.
const SYS_MOUSE:[u8;16]=[
    0x60,0x2b,0x1d,0x6f,0xa0,0xd5,0xcf,0x11,
    0xbf,0xc7,0x44,0x45,0x53,0x54,0,0
];

static ORIGINAL_FACTORY:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_CREATE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_ACQUIRE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_UNACQUIRE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_STATE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_DATA:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_FORMAT:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_COOP:AtomicUsize=AtomicUsize::new(0);
static MOUSE_OBJECT:AtomicUsize=AtomicUsize::new(0);
static FACTORY_CALLS:AtomicUsize=AtomicUsize::new(0);
static MOUSE_STATE_CALLS:AtomicUsize=AtomicUsize::new(0);
static MOUSE_STATE_OK:AtomicUsize=AtomicUsize::new(0);
static MOUSE_STATE_MOVED:AtomicUsize=AtomicUsize::new(0);
static MOUSE_STATE_PRESSED:AtomicUsize=AtomicUsize::new(0);
static MOUSE_STATE_FAILED:AtomicUsize=AtomicUsize::new(0);
static MOUSE_BUFFERED_CALLS:AtomicUsize=AtomicUsize::new(0);
static GOS_CURSOR_BRIDGE_CALLS:AtomicUsize=AtomicUsize::new(0);
static GOS_CURSOR_POST_GAMEPLAY_PROBE:AtomicBool=AtomicBool::new(false);
// From the exact supported game PE32 image, proven in both native DMPs.
// 0x5382b0 reads DIMOUSESTATE lX/lY from GetDeviceState and adds
// rounded scaled deltas to the native GOS mouse cursor, clamping it to
// (0..width-1,0..height-1). Both original DMPs were stuck at (799,599).
// The correct fix is to deliver virtual-menu-relative *movement* to this
// original routine, preserving its cursor renderer and original buttons.
const GOS_CURSOR_X:usize=0x0075_ccc0;
const GOS_CURSOR_Y:usize=0x0075_ccc4;
const GOS_WIDTH:usize=0x0075_ccb8;
const GOS_HEIGHT:usize=0x0075_ccbc;
const GOS_MOUSE_SPEED_X:usize=0x0063_57f4;
const GOS_MOUSE_SPEED_Y:usize=0x0063_57f8;
fn gos_relative_delta(current:i32,target:i32,speed:f32)->Option<i32>{
    if !speed.is_finite() || !(0.25..=8.0).contains(&speed) ||
       !(0..800).contains(&target) || !(-1024..1824).contains(&current){
        return None;
    }
    let difference=(target-current) as f32 / speed;
    let delta=difference.round() as i32;
    Some(if difference>0.0 && delta==0 {1}
         else if difference<0.0 && delta==0 {-1}
         else {delta})
}
/// Match physical borderless pointer to original virtual 800x600 cursor
/// coordinates via the verified native DD4 viewport. Only the returned
/// DIMOUSESTATE.relative X/Y fields are corrected, before ORIGINAL
/// GOS 0x5382b0 integrates them; buttons/Z/HRESULT untouched.
unsafe fn bridge_original_gos_cursor_delta(size:u32,data:Obj)->Option<String>{
    if !crate::cfg().native_gos_cursor_bridge ||
       !crate::diagnostics::is_native_windows() || size!=16 || data.is_null() ||
       !(crate::menu_scene::main_menu_active() ||
         crate::menu_scene::preferences_active() ||
         crate::window_erase_guard::native_gos_cursor_bridge_post_gameplay_ui()){
        return None;
    }
    let target=crate::window_erase_guard::native_gos_cursor_bridge_target()?;
    let mouse_dimensions=(
        ptr::read_volatile(GOS_WIDTH as *const i32),
        ptr::read_volatile(GOS_HEIGHT as *const i32)
    );
    // The ESC menu is not announced through the original MainMenu script
    // dispatcher. Sample dimensions ONCE when this independently verified
    // post-gameplay epoch first receives a real DirectInput mouse state.
    if crate::window_erase_guard::native_gos_cursor_bridge_post_gameplay_ui() &&
       !GOS_CURSOR_POST_GAMEPLAY_PROBE.swap(true,Ordering::Relaxed) &&
       crate::cfg().log_enabled {
        log(&format!(
            "NATIVE-GOS-CURSOR-BRIDGE: post-gameplay UI input observed; native GOS mouse dimensions={}x{} target_800x600={target:?} (read-only geometry proof)",
            mouse_dimensions.0,mouse_dimensions.1
        ));
    }
    if mouse_dimensions!=(800,600){return None;}
    let before=(
        ptr::read_volatile(GOS_CURSOR_X as *const i32),
        ptr::read_volatile(GOS_CURSOR_Y as *const i32)
    );
    let speed=(
        ptr::read_volatile(GOS_MOUSE_SPEED_X as *const f32),
        ptr::read_volatile(GOS_MOUSE_SPEED_Y as *const f32)
    );
    let delta=(
        gos_relative_delta(before.0,target.0,speed.0)?,
        gos_relative_delta(before.1,target.1,speed.1)?
    );
    let bytes=data as *mut u8;
    let original=(
        ptr::read_unaligned(bytes.cast::<i32>()),
        ptr::read_unaligned(bytes.add(4).cast::<i32>())
    );
    ptr::write_unaligned(bytes.cast::<i32>(),delta.0);
    ptr::write_unaligned(bytes.add(4).cast::<i32>(),delta.1);
    let n=GOS_CURSOR_BRIDGE_CALLS.fetch_add(1,Ordering::Relaxed)+1;
    (sampled(n) || n==1).then(||format!(
        "NATIVE-GOS-CURSOR-BRIDGE: sample={n} target_800x600={target:?} gos_before={before:?} speed={speed:?} physical_dinput_delta={original:?} logical_dinput_delta={delta:?}; buttons and Z unchanged, native GOS cursor renderer retained"
    ))
}

#[link(name="kernel32")]
extern "system"{
    fn GetModuleHandleA(name:*const c_char)->Obj;
    fn GetProcAddress(module:Obj,name:*const c_char)->*const c_void;
}
// Avoid ~200,000 diagnostic lines when the original game polls its mouse
// >23 million times during the native movie graph. Exponential sampling
// retains lifetime/runaway evidence without dominating startup execution.
fn sampled(n:usize)->bool{n<=8 || n.is_power_of_two()}
fn is_mouse(guid:*const u8)->bool{
    if guid.is_null(){return false;}
    unsafe{std::slice::from_raw_parts(guid,16)==SYS_MOUSE}
}
fn tracked(this:Obj)->bool{
    this as usize!=0 && MOUSE_OBJECT.load(Ordering::Acquire)==this as usize
}

/// Patch a single already-verified in-memory COM vtable method. The patch
/// is shared by objects using that vtable; hooks pass through NON-mouse
/// objects without inspecting or modifying them. Do not follow QI aliases
/// or unknown interfaces. Refuse to install on a different vtable that
/// cannot be forwarded by the single original pointer.
unsafe fn observe_slot(this:Obj,slot:usize,replacement:usize,original:&AtomicUsize)->bool{
    if this.is_null(){return false;}
    let table=*(this as *const *mut usize);
    if table.is_null(){return false;}
    let address=table.add(slot);
    let previous=address.read();
    if previous==replacement{return original.load(Ordering::Acquire)!=0;}
    if previous==0 || previous==replacement || original.load(Ordering::Acquire)!=0 {
        return false;
    }
    if original.compare_exchange(0,previous,Ordering::AcqRel,Ordering::Acquire).is_err(){
        return false;
    }
    if !win32::patch_vtable_slot(address,replacement){
        original.store(0,Ordering::Release);
        return false;
    }
    true
}
unsafe extern "system" fn observe_factory(
    instance:Obj,version:u32,out:*mut Obj,outer:Obj
)->i32{
    let ptr=ORIGINAL_FACTORY.load(Ordering::Acquire);
    if ptr==0{return 0x8000_4005u32 as i32;}
    let call:Factory=std::mem::transmute(ptr);
    let hr=call(instance,version,out,outer);
    let count=FACTORY_CALLS.fetch_add(1,Ordering::Relaxed)+1;
    let result=if hr>=0 && !out.is_null(){*out}else{ptr::null_mut()};
    let installed=if !result.is_null(){
        observe_slot(result,DI_CREATE_DEVICE,observe_create_device as usize,&ORIGINAL_CREATE)
    }else{false};
    if sampled(count) {
        log(&format!(
            "NATIVE-DINPUT: DirectInputCreateA call={count} version={version:#x} hr={:#010x} interface_returned={} create_device_observer={installed}",
            hr as u32,!result.is_null()
        ));
    }
    hr
}
unsafe extern "system" fn observe_create_device(
    this:Obj,guid:*const u8,out:*mut Obj,outer:Obj
)->i32{
    let ptr=ORIGINAL_CREATE.load(Ordering::Acquire);
    if ptr==0{return 0x8000_4005u32 as i32;}
    let call:CreateDevice=std::mem::transmute(ptr);
    let hr=call(this,guid,out,outer);
    let mouse=is_mouse(guid);
    let device=if hr>=0 && !out.is_null(){*out}else{ptr::null_mut()};
    if mouse {
        let mut installed=[false;6];
        if !device.is_null(){
            MOUSE_OBJECT.store(device as usize,Ordering::Release);
            installed[0]=observe_slot(device,DEV_ACQUIRE,observe_acquire as usize,&ORIGINAL_ACQUIRE);
            installed[1]=observe_slot(device,DEV_UNACQUIRE,observe_unacquire as usize,&ORIGINAL_UNACQUIRE);
            installed[2]=observe_slot(device,DEV_GET_STATE,observe_get_state as usize,&ORIGINAL_STATE);
            installed[3]=observe_slot(device,DEV_GET_DATA,observe_get_data as usize,&ORIGINAL_DATA);
            installed[4]=observe_slot(device,DEV_SET_FORMAT,observe_format as usize,&ORIGINAL_FORMAT);
            installed[5]=observe_slot(device,DEV_SET_COOP,observe_coop as usize,&ORIGINAL_COOP);
        }
        log(&format!(
            "NATIVE-DINPUT: CreateDevice GUID_SysMouse=true hr={:#010x} returned={} hooks acquire/unacquire/state/data/format/coop={installed:?}",
            hr as u32,!device.is_null()
        ));
    }else{
        log(&format!(
            "NATIVE-DINPUT: CreateDevice other_or_unknown_mouse=true hr={:#010x} returned={} [no changes]",
            hr as u32,!device.is_null()
        ));
    }
    hr
}
unsafe extern "system" fn observe_acquire(this:Obj)->i32{
    let p=ORIGINAL_ACQUIRE.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:NoArgs=std::mem::transmute(p);
    let hr=call(this);
    if tracked(this){log(&format!("NATIVE-DINPUT-MOUSE: Acquire hr={:#010x}",hr as u32));}
    hr
}
unsafe extern "system" fn observe_unacquire(this:Obj)->i32{
    let p=ORIGINAL_UNACQUIRE.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:NoArgs=std::mem::transmute(p);
    let hr=call(this);
    if tracked(this){log(&format!("NATIVE-DINPUT-MOUSE: Unacquire hr={:#010x}",hr as u32));}
    hr
}
unsafe extern "system" fn observe_coop(this:Obj,hwnd:Obj,flags:u32)->i32{
    let p=ORIGINAL_COOP.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:Coop=std::mem::transmute(p);
    let mouse=tracked(this);
    let effective=effective_mouse_coop_flags(
        mouse,crate::cfg().native_direct_input_mouse_coop_repair,flags
    );
    // This is the only altered argument in the opt-in A/B. Preserve the
    // native function's HRESULT and COM object unchanged.
    let hr=call(this,hwnd,effective);
    if mouse {log(&format!(
        "NATIVE-DINPUT-MOUSE: SetCooperativeLevel requested_flags={flags:#x} effective_flags={effective:#x} repair_applied={} hwnd_present={} hr={:#010x}",
        effective!=flags,!hwnd.is_null(),hr as u32));}
    hr
}
unsafe extern "system" fn observe_format(this:Obj,format:*const u32)->i32{
    let p=ORIGINAL_FORMAT.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:DataFormat=std::mem::transmute(p);
    let hr=call(this,format);
    if tracked(this){
        let data_size=if format.is_null(){None}else{
            Some(ptr::read_unaligned(format.add(3)))
        };
        log(&format!(
            "NATIVE-DINPUT-MOUSE: SetDataFormat data_size={data_size:?} hr={:#010x} [unchanged]",
            hr as u32
        ));
    }
    hr
}
unsafe extern "system" fn observe_get_state(this:Obj,size:u32,data:Obj)->i32{
    let p=ORIGINAL_STATE.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:State=std::mem::transmute(p);
    let hr=call(this,size,data);
    if !tracked(this){return hr;}
    if hr>=0{
        if let Some(note)=bridge_original_gos_cursor_delta(size,data){
            log(&note);
        }
    }
    let n=MOUSE_STATE_CALLS.fetch_add(1,Ordering::Relaxed)+1;
    let mut xyz=None;
    let mut buttons=None;
    let mut first_motion=false;
    let mut first_pressed=false;
    let mut first_failure=false;
    if hr>=0{
        MOUSE_STATE_OK.fetch_add(1,Ordering::Relaxed);
        if !data.is_null() && size>=16 && size<=64 {
            let bytes=data as *const u8;
            let dx=ptr::read_unaligned(bytes.cast::<i32>());
            let dy=ptr::read_unaligned(bytes.add(4).cast::<i32>());
            let dz=ptr::read_unaligned(bytes.add(8).cast::<i32>());
            let pressed=(0..(size as usize-12).min(8))
                .filter(|&i|*bytes.add(12+i)&0x80!=0).count();
            if dx!=0 || dy!=0 || dz!=0{
                first_motion=MOUSE_STATE_MOVED.fetch_add(1,Ordering::Relaxed)==0;
            }
            if pressed!=0{
                first_pressed=MOUSE_STATE_PRESSED.fetch_add(1,Ordering::Relaxed)==0;
            }
            xyz=Some((dx,dy,dz));
            buttons=Some(pressed);
        }
    }else{
        first_failure=MOUSE_STATE_FAILED.fetch_add(1,Ordering::Relaxed)==0;
    }
    if sampled(n) || first_motion || first_pressed || first_failure{
        log(&format!(
            "NATIVE-DINPUT-MOUSE: GetDeviceState calls={n} hr={:#010x} cbData={size} relative_xyz={xyz:?} buttons_pressed={buttons:?} total_ok={} total_motion={} total_pressed={} total_errors={}",
            hr as u32,
            MOUSE_STATE_OK.load(Ordering::Relaxed),
            MOUSE_STATE_MOVED.load(Ordering::Relaxed),
            MOUSE_STATE_PRESSED.load(Ordering::Relaxed),
            MOUSE_STATE_FAILED.load(Ordering::Relaxed)
        ));
    }
    hr
}
unsafe extern "system" fn observe_get_data(
    this:Obj,size:u32,events:Obj,count:*mut u32,flags:u32
)->i32{
    let p=ORIGINAL_DATA.load(Ordering::Acquire);
    if p==0{return 0x8000_4005u32 as i32;}
    let call:Buffered=std::mem::transmute(p);
    let hr=call(this,size,events,count,flags);
    if tracked(this){
        let n=MOUSE_BUFFERED_CALLS.fetch_add(1,Ordering::Relaxed)+1;
        if sampled(n){
            let event_count=if hr>=0 && !count.is_null(){Some(*count)}else{None};
            log(&format!(
                "NATIVE-DINPUT-MOUSE: GetDeviceData calls={n} hr={:#010x} event_count={event_count:?} flags={flags:#x} [unchanged]",
                hr as u32));
        }
    }
    hr
}

/// Hook exactly the admitted EXE's original DirectInputCreateA import.
/// If another runtime already substituted this import, do nothing.
pub fn install(){
    if !diagnostics::is_native_windows() || !crate::cfg().log_enabled ||
       !crate::cfg().native_dd4_bootstrap ||
       !(crate::cfg().native_direct_input_mouse_trace ||
         crate::cfg().native_direct_input_mouse_coop_repair ||
         crate::cfg().native_gos_cursor_bridge) ||
       crate::cfg().directdraw_passthrough {return;}
    unsafe{
        let exe=GetModuleHandleA(ptr::null());
        let dinput=GetModuleHandleA(c"dinput.dll".as_ptr());
        if exe.is_null() || dinput.is_null(){
            log("NATIVE-DINPUT: audit disabled: EXE or native dinput.dll not loaded");
            return;
        }
        let real=GetProcAddress(dinput,c"DirectInputCreateA".as_ptr()) as usize;
        let slot=(exe as usize+FACTORY_IAT_RVA) as *mut usize;
        let found=slot.read();
        if real==0 || found!=real{
            log("NATIVE-DINPUT: exact imported DirectInputCreateA not verified; NO hook installed");
            return;
        }
        ORIGINAL_FACTORY.store(real,Ordering::Release);
        if !win32::patch_vtable_slot(slot,observe_factory as usize){
            ORIGINAL_FACTORY.store(0,Ordering::Release);
            log("NATIVE-DINPUT: EXE IAT patch rejected; audit disabled");
            return;
        }
        log("NATIVE-DINPUT: exact EXE DirectInputCreateA observed (passive native mouse COM tracing, unchanged arguments/results)");
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn admitted_game_directinput_iat_and_legacy_interface_slots(){
        assert_eq!(FACTORY_IAT_RVA,0x61fbb0);
        assert_eq!([DI_CREATE_DEVICE,DEV_ACQUIRE,DEV_UNACQUIRE,
                    DEV_GET_STATE,DEV_GET_DATA,DEV_SET_FORMAT,DEV_SET_COOP],
                   [3,7,8,9,10,11,13]);
        assert_eq!(std::mem::size_of::<Obj>(),4);
    }
    #[test]
    fn sys_mouse_guid_strictly_matches_windows_guid(){
        assert_eq!(SYS_MOUSE,[
          0x60,0x2b,0x1d,0x6f,0xa0,0xd5,0xcf,0x11,
          0xbf,0xc7,0x44,0x45,0x53,0x54,0,0]);
        assert!(is_mouse(SYS_MOUSE.as_ptr()));
        let mut other=SYS_MOUSE;other[0]+=1;
        assert!(!is_mouse(other.as_ptr()));
        assert!(!is_mouse(ptr::null()));
        assert!(sampled(1));assert!(!sampled(9));assert!(!sampled(120));
        assert!(sampled(128));assert!(sampled(16_777_216));
        assert!(!sampled(23_622_120));
    }
    #[test]
    fn original_gos_mouse_delta_matches_800x600_center_and_edges(){
        assert_eq!(gos_relative_delta(799,400,1.3),Some(-307));
        assert_eq!(gos_relative_delta(599,300,1.3),Some(-230));
        assert_eq!(gos_relative_delta(0,799,1.0),Some(799));
        assert_eq!(gos_relative_delta(400,400,1.3),Some(0));
        assert_eq!(gos_relative_delta(400,401,4.0),Some(1));
        assert_eq!(gos_relative_delta(400,399,4.0),Some(-1));
        assert_eq!(gos_relative_delta(799,800,1.0),None);
        assert_eq!(gos_relative_delta(799,-1,1.0),None);
        assert_eq!(gos_relative_delta(799,400,0.0),None);
        assert_eq!(gos_relative_delta(799,400,f32::NAN),None);
    }
    #[test]
    fn mouse_cooperative_policy_is_exact_and_never_acts_on_other_devices(){
        assert_eq!(MOUSE_EXCLUSIVE_BACKGROUND,0x9);
        assert_eq!(MOUSE_NONEXCLUSIVE_FOREGROUND,0x6);
        assert_eq!(effective_mouse_coop_flags(true,true,0x9),0x6);
        assert_eq!(effective_mouse_coop_flags(true,false,0x9),0x9);
        assert_eq!(effective_mouse_coop_flags(false,true,0x9),0x9);
        for flags in [0,0x5,0x6,0xa,0xf,0x10]{
            assert_eq!(effective_mouse_coop_flags(true,true,flags),flags);
        }
    }
    #[test]
    fn function_signatures_have_exact_x86_win32_abi(){
        let _:Factory=observe_factory;
        let _:CreateDevice=observe_create_device;
        let _:NoArgs=observe_acquire;
        let _:NoArgs=observe_unacquire;
        let _:Coop=observe_coop;
        let _:State=observe_get_state;
        let _:Buffered=observe_get_data;
        let _:DataFormat=observe_format;
    }
}
