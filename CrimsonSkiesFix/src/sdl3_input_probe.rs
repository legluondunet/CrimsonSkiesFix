//! 1.2 input research: compare authenticated GOS joystick state and SDL3.
//! No input injection or native joystick suppression in this diagnostic build.
//! SDL is initialized only on the foreground game's verified Flip thread.
use std::{cell::RefCell, ffi::{c_char,c_void,CStr}, fs, os::windows::ffi::OsStrExt,
    ptr, sync::atomic::{AtomicU32,Ordering}, time::{Duration,Instant}};
type Obj=*mut c_void;
const GAMEPAD:u32=0x2000;
const GYRO:i32=2;
const DEAD_ZONE:i32=4096;
const BASE:usize=0x400000;
const READY:usize=0x75c1dc;
const STATE:usize=0x75c1e4;
static OWNER:AtomicU32=AtomicU32::new(0);
#[link(name="kernel32")]
extern "system" {
    fn LoadLibraryExW(name:*const u16,file:Obj,flags:u32)->Obj;
    fn FreeLibrary(module:Obj)->i32;
    fn GetProcAddress(module:Obj,name:*const c_char)->*const c_void;
    fn GetModuleHandleW(name:*const u16)->Obj;
    fn GetCurrentThreadId()->u32;
    fn GetCurrentProcessId()->u32;
}
#[link(name="user32")]
extern "system" {
    fn GetForegroundWindow()->Obj;
    fn GetWindowThreadProcessId(window:Obj,process:*mut u32)->u32;
}
type GetIds=unsafe extern "C" fn(*mut i32)->*mut u32;
struct Api {
    module:Obj,
    quit:unsafe extern "C" fn(u32),
    free:unsafe extern "C" fn(Obj),
    pads:GetIds,
    sticks:GetIds,
    open:unsafe extern "C" fn(u32)->Obj,
    close:unsafe extern "C" fn(Obj),
    connected:unsafe extern "C" fn(Obj)->bool,
    name:unsafe extern "C" fn(Obj)->*const c_char,
    update:unsafe extern "C" fn(),
    axis:unsafe extern "C" fn(Obj,i32)->i16,
    button:unsafe extern "C" fn(Obj,i32)->bool,
    sensor:unsafe extern "C" fn(Obj,i32)->bool,
    sensor_enable:unsafe extern "C" fn(Obj,i32,bool)->bool,
    sensor_data:unsafe extern "C" fn(Obj,i32,*mut f32,i32)->bool,
    touchpads:unsafe extern "C" fn(Obj)->i32,
}
impl Api {
    unsafe fn load()->Result<Self,String>{
        let expected=option_env!("CRIMSONSKIESFIX_SDL3_SHA256").ok_or("no compiled SDL3 hash")?;
        let path=std::env::current_exe().map_err(|e|e.to_string())?.with_file_name("SDL3.dll");
        let meta=fs::symlink_metadata(&path).map_err(|e|e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>16*1024*1024 {
            return Err("unsafe SDL3 path or size".into());
        }
        let bytes=fs::read(&path).map_err(|e|e.to_string())?;
        if crate::exe_gate::sha256(&bytes)!=expected{return Err("SDL3 hash mismatch".into());}
        let wide:Vec<u16>=path.as_os_str().encode_wide().chain([0]).collect();
        let module=LoadLibraryExW(wide.as_ptr(),ptr::null_mut(),0x900);
        if module.is_null(){return Err("LoadLibraryExW failed".into());}
        macro_rules! sym {($name:literal,$ty:ty)=>{{
            let p=GetProcAddress(module,concat!($name,"\0").as_ptr().cast());
            if p.is_null(){FreeLibrary(module);return Err(format!("missing {}",$name));}
            std::mem::transmute::<*const c_void,$ty>(p)
        }};}
        let init=sym!("SDL_InitSubSystem",unsafe extern "C" fn(u32)->bool);
        let hint=sym!("SDL_SetHint",unsafe extern "C" fn(*const c_char,*const c_char)->bool);
        let events=sym!("SDL_SetGamepadEventsEnabled",unsafe extern "C" fn(bool));
        let jevents=sym!("SDL_SetJoystickEventsEnabled",unsafe extern "C" fn(bool));
        let version=sym!("SDL_GetVersion",unsafe extern "C" fn()->i32);
        let api=Self {
            module,
            quit:sym!("SDL_QuitSubSystem",unsafe extern "C" fn(u32)),
            free:sym!("SDL_free",unsafe extern "C" fn(Obj)),
            pads:sym!("SDL_GetGamepads",GetIds),
            sticks:sym!("SDL_GetJoysticks",GetIds),
            open:sym!("SDL_OpenGamepad",unsafe extern "C" fn(u32)->Obj),
            close:sym!("SDL_CloseGamepad",unsafe extern "C" fn(Obj)),
            connected:sym!("SDL_GamepadConnected",unsafe extern "C" fn(Obj)->bool),
            name:sym!("SDL_GetGamepadName",unsafe extern "C" fn(Obj)->*const c_char),
            update:sym!("SDL_UpdateGamepads",unsafe extern "C" fn()),
            axis:sym!("SDL_GetGamepadAxis",unsafe extern "C" fn(Obj,i32)->i16),
            button:sym!("SDL_GetGamepadButton",unsafe extern "C" fn(Obj,i32)->bool),
            sensor:sym!("SDL_GamepadHasSensor",unsafe extern "C" fn(Obj,i32)->bool),
            sensor_enable:sym!("SDL_SetGamepadSensorEnabled",unsafe extern "C" fn(Obj,i32,bool)->bool),
            sensor_data:sym!("SDL_GetGamepadSensorData",unsafe extern "C" fn(Obj,i32,*mut f32,i32)->bool),
            touchpads:sym!("SDL_GetNumGamepadTouchpads",unsafe extern "C" fn(Obj)->i32),
        };
        // SDL owns no window. Explicit process-foreground gating below
        // controls sampling; this hint allows HID updates in a non-SDL game.
        hint(c"SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS".as_ptr(),c"1".as_ptr());
        if !init(GAMEPAD){FreeLibrary(module);return Err("SDL_InitSubSystem failed".into());}
        events(false);jevents(false);
        crate::log(&format!("SDL3-PROBE: initialized version={} sha256={expected}; input injection=false",version()));
        Ok(api)
    }
    unsafe fn ids(&self,get:GetIds)->Vec<u32>{
        let mut n=0;
        let p=get(&mut n);
        if p.is_null(){return Vec::new();}
        let out=if (0..=128).contains(&n){
            std::slice::from_raw_parts(p,n as usize).to_vec()
        }else{Vec::new()};
        (self.free)(p.cast());
        out
    }
}
fn signed_axis(raw:i16)->i32{
    let n=i32::from(raw);
    let mag=n.abs();
    if mag<=DEAD_ZONE{return 0;}
    let max=if n<0{32768}else{32767};
    n.signum()*((mag-DEAD_ZONE)*1000/(max-DEAD_ZONE))
}
fn mapped_axes(a:[i16;6])->[i32;4]{
    // Proposed logical mapping, not yet delivered to GOS:
    // roll, pitch (forward positive), yaw, throttle change (forward positive).
    [signed_axis(a[0]),-signed_axis(a[1]),signed_axis(a[2]),-signed_axis(a[3])]
}
#[derive(Debug,PartialEq,Eq)]
struct Native {ready:bool,axes:[i32;6],pov:u32,buttons:u32}
unsafe fn native_snapshot()->Option<Native>{
    if GetModuleHandleW(ptr::null()) as usize!=BASE{return None;}
    // Byte-exact supported EXE accessor preimages; no pointer chains.
    if std::slice::from_raw_parts(0x536b40 as *const u8,15)!=
        [0x8b,0x0d,0xdc,0xc1,0x75,0,0x33,0xc0,0x83,0xf9,1,0x0f,0x94,0xc0,0xc3]
        || std::slice::from_raw_parts(0x536c30 as *const u8,6)!=[0xb8,0xe4,0xc1,0x75,0,0xc3] {
        return None;
    }
    let ready=ptr::read_volatile(READY as *const u32)==1;
    if !ready{return Some(Native{ready,axes:[0;6],pov:0xffff,buttons:0});}
    let axes=std::array::from_fn(|i|ptr::read_volatile((STATE+4*i) as *const i32));
    let pov=ptr::read_volatile((STATE+32) as *const u32);
    let buttons=(0..32).fold(0u32,|m,i|m|((ptr::read_volatile((STATE+48+i) as *const u8)&0x80!=0) as u32)<<i);
    Some(Native{ready,axes,pov,buttons})
}
struct Probe {
    api:Api,pad:Obj,id:u32,gyro:bool,last_poll:Instant,last_scan:Option<Instant>,
    inventory:Vec<u32>,last_native:Option<Native>,last_buttons:u32,last_axes:[i32;4],
    had_sample:bool,foreground:bool,empty_reported:bool,
}
impl Probe {
    unsafe fn new()->Result<Self,String>{
        Ok(Self{api:Api::load()?,pad:ptr::null_mut(),id:0,gyro:false,
            last_poll:Instant::now()-Duration::from_secs(1),last_scan:None,inventory:Vec::new(),
            last_native:None,last_buttons:0,last_axes:[0;4],had_sample:false,foreground:true,empty_reported:false})
    }
    unsafe fn tick(&mut self,foreground:bool){
        if !foreground {
            if self.foreground{crate::log("SDL3-PROBE: focus lost; no input sampled");}
            self.foreground=false;self.had_sample=false;return;
        }
        if !self.foreground{crate::log("SDL3-PROBE: focus regained");self.foreground=true;}
        let now=Instant::now();
        if now.duration_since(self.last_poll)<Duration::from_millis(100){return;}
        self.last_poll=now;
        (self.api.update)();
        if !self.pad.is_null() && !(self.api.connected)(self.pad){
            crate::log(&format!("SDL3-PROBE: disconnected id={}",self.id));
            (self.api.close)(self.pad);self.pad=ptr::null_mut();self.id=0;
            self.gyro=false;self.had_sample=false;self.last_scan=None;
        }
        if self.last_scan.is_none_or(|t|now.duration_since(t)>=Duration::from_secs(1)){
            self.last_scan=Some(now);
            let mut ids=self.api.ids(self.api.pads);ids.sort_unstable();
            let raw=self.api.ids(self.api.sticks);
            if ids!=self.inventory || !self.empty_reported {
                crate::log(&format!("SDL3-PROBE: inventory mapped={ids:?} total_joysticks={}",raw.len()));
                self.inventory=ids.clone();self.empty_reported=true;
            }
            if self.pad.is_null(){
                for id in ids {
                    let pad=(self.api.open)(id);
                    if pad.is_null(){continue;}
                    self.pad=pad;self.id=id;self.had_sample=false;
                    let p=(self.api.name)(pad);
                    let name=if p.is_null(){"unknown".to_owned()}else{
                        CStr::from_ptr(p).to_string_lossy().chars().take(100)
                            .map(|c|if c.is_control(){' '}else{c}).collect()
                    };
                    let gyro=(self.api.sensor)(pad,GYRO);
                    self.gyro=gyro&&(self.api.sensor_enable)(pad,GYRO,true);
                    crate::log(&format!("SDL3-PROBE: selected id={id} name={name:?} gyro_present={gyro} gyro_enabled={} accelerometer={} touchpads={}",self.gyro,(self.api.sensor)(pad,1),(self.api.touchpads)(pad)));
                    break;
                }
            }
        }
        let native=native_snapshot();
        let native_changed=native!=self.last_native;
        if self.pad.is_null(){
            if native_changed{crate::log(&format!("SDL3-PROBE: native-only {native:?}"));}
            self.last_native=native;return;
        }
        let raw=std::array::from_fn(|i|(self.api.axis)(self.pad,i as i32));
        let mapped=mapped_axes(raw);
        let buttons=(0..26).fold(0u32,|m,i|m|(((self.api.button)(self.pad,i) as u32)<<i));
        let changed=mapped.iter().zip(self.last_axes).any(|(x,y)|(*x-y).abs()>=100);
        if !self.had_sample || changed || buttons!=self.last_buttons || native_changed {
            let mut gyro=[0.0;3];
            let g=(self.gyro&&(self.api.sensor_data)(self.pad,GYRO,gyro.as_mut_ptr(),3)).then_some(gyro);
            crate::log(&format!("SDL3-PROBE: sample id={} raw={raw:?} roll/pitch/yaw/throttle={mapped:?} buttons={buttons:#010x} gyro_rad_s={g:?} native={native:?}",self.id));
            self.last_axes=mapped;self.last_buttons=buttons;self.had_sample=true;
        }
        self.last_native=native;
    }
}
impl Drop for Probe{
    fn drop(&mut self){unsafe{
        if !self.pad.is_null(){(self.api.close)(self.pad);}
        (self.api.quit)(GAMEPAD);
        FreeLibrary(self.api.module);
    }}
}
enum Slot{Waiting,Failed,Active(Probe)}
thread_local!{static PROBE:RefCell<Slot>=const{RefCell::new(Slot::Waiting)};}
pub fn on_game_flip(){
    if !crate::cfg().log_enabled || !crate::exe_gate::verify_once(){return;}
    unsafe{
        let mut process=0;
        let hwnd=GetForegroundWindow();
        let foreground=!hwnd.is_null() && GetWindowThreadProcessId(hwnd,&mut process)!=0 && process==GetCurrentProcessId();
        let thread=GetCurrentThreadId();
        let owner=OWNER.load(Ordering::Acquire);
        if owner==0 {
            if !foreground || OWNER.compare_exchange(0,thread,Ordering::AcqRel,Ordering::Acquire).is_err(){return;}
        }else if owner!=thread{return;}
        PROBE.with(|slot|{
            let Ok(mut slot)=slot.try_borrow_mut() else{return;};
            if matches!(*slot,Slot::Waiting){
                *slot=match Probe::new(){Ok(p)=>Slot::Active(p),Err(e)=>{
                    crate::log(&format!("SDL3-PROBE: unavailable: {e}; native input untouched"));Slot::Failed
                }};
            }
            if let Slot::Active(p)=&mut *slot{p.tick(foreground);}
        });
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn axes_are_bounded_and_deadzone_is_centered(){
        assert_eq!(signed_axis(i16::MIN),-1000);
        assert_eq!(signed_axis(i16::MAX),1000);
        for n in [-4096,0,4096]{assert_eq!(signed_axis(n),0);}
        assert!(signed_axis(16384)>0);
        assert_eq!(mapped_axes([32767,-32768,-32768,32767,0,0]),[1000,1000,-1000,-1000]);
    }
}
