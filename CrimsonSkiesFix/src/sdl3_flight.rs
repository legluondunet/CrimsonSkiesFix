//! Experimental SDL3 ownership of the exact 1.02 GOS joystick lifecycle.
//! All hooks are installed after the on-disk executable hash gate.
use super::{Api,Obj,BASE,READY,STATE,GetModuleHandleW,GetCurrentThreadId,
    GetCurrentProcessId,GetForegroundWindow,GetWindowThreadProcessId,signed_axis};
use std::{cell::RefCell,ffi::{c_void,CStr},ptr,sync::atomic::{AtomicU8,AtomicU32,Ordering},
    time::{Duration,Instant}};
const PREVIOUS:usize=STATE+0x110;
const NATIVE_COM:usize=0x75c1e0;
const AXES_COUNT:usize=0x75c424;
const CALIBRATION:usize=0x75c438;
const MODE_FLAG:usize=0x71c2a0;
const CALLBACK_CONTEXT:usize=0x75caf4;
const INIT:usize=0x5363a0;
const POLL:usize=0x536b50;
const RELEASE:usize=0x536b10;
const SITE_INIT:usize=0x53510d;
const SITE_RELEASE:usize=0x53540f;
const SITE_POLL:usize=0x5355f5;
const SITE_SELECT:usize=0x487516;
const STATE_LEN:usize=0x110;
const WAIT:u8=1;
const ACTIVE:u8=2;
const FALLBACK:u8=3;
const CLOSED:u8=4;
static MODE:AtomicU8=AtomicU8::new(0);
static OWNER:AtomicU32=AtomicU32::new(0);
#[link(name="kernel32")]
extern "system"{
    fn VirtualProtect(address:Obj,size:usize,flags:u32,previous:*mut u32)->i32;
    fn FlushInstructionCache(process:Obj,address:*const c_void,size:usize)->i32;
    fn GetCurrentProcess()->Obj;
}
pub(super) fn owns_or_waits()->bool{
    matches!(MODE.load(Ordering::Acquire),WAIT|ACTIVE|CLOSED)
}
fn call_bytes(site:usize,target:usize)->[u8;5]{
    let b=(target as u32).wrapping_sub((site as u32).wrapping_add(5)).to_le_bytes();
    [0xe8,b[0],b[1],b[2],b[3]]
}
unsafe fn read32(address:usize)->u32{ptr::read_volatile(address as *const u32)}
unsafe fn write32(address:usize,value:u32){ptr::write_volatile(address as *mut u32,value);}
fn neutral(throttle:i32)->[u8;STATE_LEN]{
    let mut b=[0u8;STATE_LEN];
    b[8..12].copy_from_slice(&(1000-2*throttle.clamp(0,1000)).to_le_bytes());
    b[32..48].fill(0xff);
    b
}
fn advance_throttle(old:f32,raw:i16,seconds:f32)->f32{
    (old-(signed_axis(raw) as f32)*0.5*seconds.clamp(0.0,0.1)).clamp(0.0,1000.0)
}
fn provisional_state(raw:[i16;6],buttons:u32,throttle:i32)->[u8;STATE_LEN]{
    let mut b=neutral(throttle);
    for (offset,axis) in [(0,0),(4,1),(20,2)]{
        b[offset..offset+4].copy_from_slice(&signed_axis(raw[axis]).to_le_bytes());
    }
    // RT/LT, shoulders, face buttons, stick clicks. The game still resolves
    // these ten slots via its original action bindings; final mapping follows.
    let bits=[raw[5]>8192,raw[4]>8192,buttons&(1<<10)!=0,buttons&(1<<9)!=0,
        buttons&1!=0,buttons&2!=0,buttons&4!=0,buttons&8!=0,
        buttons&(1<<7)!=0,buttons&(1<<8)!=0];
    for (i,pressed) in bits.into_iter().enumerate(){b[48+i]=if pressed{0x80}else{0};}
    let x=i32::from(buttons&(1<<14)!=0)-i32::from(buttons&(1<<13)!=0);
    let y=i32::from(buttons&(1<<12)!=0)-i32::from(buttons&(1<<11)!=0);
    let pov:u32=match(x,y){(0,-1)=>0,(1,-1)=>4500,(1,0)=>9000,(1,1)=>13500,
        (0,1)=>18000,(-1,1)=>22500,(-1,0)=>27000,(-1,-1)=>31500,_=>0xffff_ffff};
    b[32..36].copy_from_slice(&pov.to_le_bytes());b
}
fn centered(raw:[i16;6],buttons:u32)->bool{
    raw[..4].iter().all(|&x|signed_axis(x)==0)&&raw[4]<=8192&&raw[5]<=8192&&buttons==0
}
unsafe fn publish(state:&[u8;STATE_LEN],ready:bool,reset:bool){
    if reset {
        ptr::copy_nonoverlapping(state.as_ptr(),PREVIOUS as *mut u8,STATE_LEN);
    }else{
        ptr::copy_nonoverlapping(STATE as *const u8,PREVIOUS as *mut u8,STATE_LEN);
    }
    ptr::copy_nonoverlapping(state.as_ptr(),STATE as *mut u8,STATE_LEN);
    write32(READY,ready as u32);
}
unsafe fn calibrate(){
    // Six contiguous native axis descriptors, each 0x118 bytes.
    // Original consumer uses (value-center)*scale; original setup
    // computes scale=2/(max-min). No COM pointer or native FF capability.
    ptr::write_bytes(CALIBRATION as *mut u8,0,6*0x118);
    for i in 0..6{
        let at=CALIBRATION+i*0x118;
        write32(at,(-1000i32) as u32);write32(at+4,1000);
        write32(at+8,0.001f32.to_bits());write32(at+12,0);
    }
    write32(AXES_COUNT,4);
    for addr in [0x75c428,0x75c42c,0x75c430]{write32(addr,0);}
    publish(&neutral(500),false,true);
}
unsafe fn foreground()->bool{
    let mut pid=0;let hwnd=GetForegroundWindow();
    !hwnd.is_null()&&GetWindowThreadProcessId(hwnd,&mut pid)!=0&&pid==GetCurrentProcessId()
}
struct Flight{
    api:Api,pad:Obj,id:u32,armed:bool,throttle:f32,
    last:Instant,scan:Option<Instant>,last_log:Option<Instant>,last_state:[u8;STATE_LEN],
    was_foreground:bool,
}
impl Flight{
    unsafe fn new()->Result<Self,String>{
        Ok(Self{api:Api::load()?,pad:ptr::null_mut(),id:0,armed:false,throttle:500.0,
            last:Instant::now(),scan:None,last_log:None,last_state:neutral(500),
            was_foreground:false})
    }
    unsafe fn close_pad(&mut self){
        if !self.pad.is_null(){(self.api.close)(self.pad);}
        self.pad=ptr::null_mut();self.id=0;self.armed=false;self.scan=None;
    }
    unsafe fn tick(&mut self)->bool{
        let now=Instant::now();
        let dt=now.duration_since(self.last).as_secs_f32();self.last=now;
        if !foreground(){
            if self.was_foreground{crate::log("SDL3-FLIGHT: focus lost; neutral and disarmed");}
            self.was_foreground=false;self.armed=false;
            publish(&neutral(self.throttle as i32),false,true);return false;
        }
        if !self.was_foreground{
            crate::log("SDL3-FLIGHT: focus acquired; waiting for neutral controls");
            self.was_foreground=true;self.armed=false;
        }
        (self.api.update)();
        if !self.pad.is_null()&&!(self.api.connected)(self.pad){
            crate::log(&format!("SDL3-FLIGHT: disconnected id={}",self.id));
            self.close_pad();
            publish(&neutral(self.throttle as i32),false,true);
        }
        if self.pad.is_null()&&self.scan.is_none_or(|t|now.duration_since(t)>=Duration::from_millis(500)){
            self.scan=Some(now);
            let mut ids=self.api.ids(self.api.pads);ids.sort_unstable();
            for id in ids{
                let p=(self.api.open)(id);if p.is_null(){continue;}
                self.pad=p;self.id=id;self.armed=false;
                let n=(self.api.name)(p);
                let name=if n.is_null(){"unknown".to_owned()}else{
                    CStr::from_ptr(n).to_string_lossy().chars().take(100)
                    .map(|c|if c.is_control(){' '}else{c}).collect()
                };
                crate::log(&format!("SDL3-FLIGHT: selected id={id} name={name:?}; gyro_available={} touchpads={}",(self.api.sensor)(p,2),(self.api.touchpads)(p)));
                break;
            }
        }
        if self.pad.is_null(){publish(&neutral(self.throttle as i32),false,true);return false;}
        let raw=std::array::from_fn(|i|(self.api.axis)(self.pad,i as i32));
        let buttons=(0..26).fold(0u32,|m,i|m|(((self.api.button)(self.pad,i) as u32)<<i));
        if !self.armed{
            if !centered(raw,buttons){publish(&neutral(self.throttle as i32),false,true);return false;}
            self.armed=true;crate::log("SDL3-FLIGHT: neutral acquired; analog controls armed");
            publish(&neutral(self.throttle as i32),true,true);return true;
        }
        self.throttle=advance_throttle(self.throttle,raw[3],dt);
        let state=provisional_state(raw,buttons,self.throttle.round() as i32);
        publish(&state,true,false);
        if self.last_log.is_none_or(|t|now.duration_since(t)>=Duration::from_millis(250))&&
            (state!=self.last_state||self.last_log.is_none_or(|t|now.duration_since(t)>=Duration::from_secs(2))){
            self.last_log=Some(now);self.last_state=state;
            if crate::cfg().log_enabled{
                let axes:[i32;6]=std::array::from_fn(|i|i32::from_le_bytes(state[i*4..i*4+4].try_into().unwrap()));
                crate::log(&format!("SDL3-FLIGHT: id={} axes={axes:?} throttle={} raw={raw:?} buttons={buttons:#010x} native_com={:#x}",self.id,self.throttle.round(),read32(NATIVE_COM)));
            }
        }
        true
    }
}
thread_local!{static FLIGHT:RefCell<Option<Flight>>=const{RefCell::new(None)};}
unsafe fn native_init(hwnd:Obj)->i32{
    let f:unsafe extern "C" fn(Obj)->i32=std::mem::transmute(INIT);f(hwnd)
}
unsafe fn native_poll(callback:i32)->Obj{
    let f:unsafe extern "C" fn(i32)->Obj=std::mem::transmute(POLL);f(callback)
}
unsafe fn native_release()->i32{
    let f:unsafe extern "C" fn()->i32=std::mem::transmute(RELEASE);f()
}
unsafe fn start(late:bool)->bool{
    let thread=GetCurrentThreadId();
    if OWNER.compare_exchange(0,thread,Ordering::AcqRel,Ordering::Acquire).is_err()
        && OWNER.load(Ordering::Acquire)!=thread{return false;}
    if read32(NATIVE_COM)!=0&&(!late||read32(0x75c428)!=0){
        crate::log("SDL3-FLIGHT: unsafe existing native force-feedback/lifecycle; preserving native path");
        MODE.store(FALLBACK,Ordering::Release);return false;
    }
    let f=match Flight::new(){Ok(f)=>f,Err(e)=>{
        crate::log(&format!("SDL3-FLIGHT: SDL init failed: {e}; native fallback for this test build"));
        MODE.store(FALLBACK,Ordering::Release);return false;
    }};
    if read32(NATIVE_COM)!=0{native_release();}
    if read32(NATIVE_COM)!=0{
        crate::log("SDL3-FLIGHT: native release failed; native path retained");
        MODE.store(FALLBACK,Ordering::Release);return false;
    }
    calibrate();
    FLIGHT.with(|s|*s.borrow_mut()=Some(f));
    MODE.store(ACTIVE,Ordering::Release);
    crate::log(&format!("SDL3-FLIGHT: exclusive SDL joystick ownership initialized late={late}; keyboard/mouse native; native COM null; calibration fixed"));
    true
}
unsafe extern "C" fn init_hook(hwnd:Obj)->i32{
    match MODE.load(Ordering::Acquire){
        FALLBACK=>native_init(hwnd),
        ACTIVE=>1,
        _=>if start(false){1}else{native_init(hwnd)}
    }
}
unsafe extern "C" fn poll_hook(callback:i32)->Obj{
    let mode=MODE.load(Ordering::Acquire);
    if mode==FALLBACK{return native_poll(callback);}
    if mode==WAIT&&!start(true){return native_poll(callback);}
    if MODE.load(Ordering::Acquire)!=ACTIVE{return ptr::null_mut();}
    if OWNER.load(Ordering::Acquire)!=GetCurrentThreadId(){
        // Do not call SDL or mutate concurrently from another thread.
        return ptr::null_mut();
    }
    let ready=FLIGHT.with(|s|s.try_borrow_mut().ok()
        .and_then(|mut s|s.as_mut().map(|f|f.tick())).unwrap_or(false));
    if ready&&callback as u8!=0{
        let context=read32(CALLBACK_CONTEXT) as Obj;
        if !context.is_null(){
            let dispatch:unsafe extern "thiscall" fn(Obj)=std::mem::transmute(0x537440usize);
            dispatch(context);
        }
    }
    if ready{STATE as Obj}else{ptr::null_mut()}
}
unsafe extern "C" fn release_hook()->i32{
    match MODE.load(Ordering::Acquire){
        ACTIVE=>{
            if OWNER.load(Ordering::Acquire)==GetCurrentThreadId(){
                FLIGHT.with(|s|if let Ok(mut s)=s.try_borrow_mut(){
                    if let Some(f)=s.as_mut(){f.close_pad();}
                    *s=None;
                });
                publish(&neutral(500),false,true);
                OWNER.store(0,Ordering::Release);
                MODE.store(CLOSED,Ordering::Release);
                crate::log("SDL3-FLIGHT: released; native COM unused");
            }
            1
        },
        CLOSED=>1,
        _=>native_release(),
    }
}
unsafe extern "C" fn selection_hook()->i32{
    if MODE.load(Ordering::Acquire)==ACTIVE{(read32(READY)==1) as i32}
    else{((read32(MODE_FLAG)&4)!=0) as i32}
}
pub fn install(){
    // Caller has already verified the disk hash. Reject all modified
    // preimages before changing a single byte. No on-disk changes.
    unsafe{
        if GetModuleHandleW(ptr::null()) as usize!=BASE{return;}
        let patches=[
            (SITE_INIT,call_bytes(SITE_INIT,INIT).to_vec(),call_bytes(SITE_INIT,init_hook as usize).to_vec()),
            (SITE_RELEASE,call_bytes(SITE_RELEASE,RELEASE).to_vec(),call_bytes(SITE_RELEASE,release_hook as usize).to_vec()),
            (SITE_POLL,call_bytes(SITE_POLL,POLL).to_vec(),call_bytes(SITE_POLL,poll_hook as usize).to_vec()),
            (SITE_SELECT,vec![0xf6,0x05,0xa0,0xc2,0x71,0,4,0x0f,0x84,0x3a,1,0,0],{
                let mut b=call_bytes(SITE_SELECT,selection_hook as usize).to_vec();
                b.extend([0x85,0xc0,0x0f,0x84,0x3a,1,0,0]);b
            }),
        ];
        for (addr,old,_) in &patches{
            if std::slice::from_raw_parts(*addr as *const u8,old.len())!=old.as_slice(){
                crate::log(&format!("SDL3-FLIGHT: exact preimage mismatch {addr:#x}; no changes"));
                return;
            }
        }
        let pages=[0x535000usize,0x487000usize];let mut prot=[0u32;2];
        for i in 0..2{
            if VirtualProtect(pages[i] as Obj,0x1000,0x40,&mut prot[i])==0{
                for j in 0..i{let mut ignored=0;VirtualProtect(pages[j] as Obj,0x1000,prot[j],&mut ignored);}
                crate::log("SDL3-FLIGHT: patch page access denied; no changes");return;
            }
        }
        for (addr,_,new) in &patches{ptr::copy_nonoverlapping(new.as_ptr(),*addr as *mut u8,new.len());}
        let flushed=pages.iter().all(|&p|FlushInstructionCache(GetCurrentProcess(),p as *const c_void,0x1000)!=0);
        if !flushed{
            for (addr,old,_) in &patches{ptr::copy_nonoverlapping(old.as_ptr(),*addr as *mut u8,old.len());}
            for p in pages{FlushInstructionCache(GetCurrentProcess(),p as *const c_void,0x1000);}
        }
        for i in 0..2{let mut ignored=0;VirtualProtect(pages[i] as Obj,0x1000,prot[i],&mut ignored);}
        if flushed{MODE.store(WAIT,Ordering::Release);crate::log("SDL3-FLIGHT: four exact in-memory sites installed; awaiting GOS joystick lifecycle");}
        else{crate::log("SDL3-FLIGHT: instruction cache flush failed; restored original calls");}
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    fn axis(b:&[u8;STATE_LEN],i:usize)->i32{i32::from_le_bytes(b[i*4..i*4+4].try_into().unwrap())}
    #[test]fn state_has_exact_native_layout_and_neutral_pov(){
        let n=neutral(500);assert_eq!(n.len(),272);
        assert!(n[..32].iter().all(|&v|v==0));assert_eq!(&n[32..48],&[255;16]);
        assert!(n[48..].iter().all(|&v|v==0));
    }
    #[test]fn analog_axes_and_trigger_slots(){
        let s=provisional_state([32767,-32768,32767,0,32767,32767],1<<14,1000);
        assert_eq!([axis(&s,0),axis(&s,1),axis(&s,2),axis(&s,5)],[1000,-1000,-1000,1000]);
        assert_eq!(&s[48..50],&[128,128]);assert_eq!(axis(&s,8),9000);
    }
    #[test]fn throttle_holds_and_bounds(){
        assert_eq!(advance_throttle(500.0,0,0.1),500.0);
        assert_eq!(advance_throttle(500.0,-32768,1.0),550.0);
        assert_eq!(advance_throttle(990.0,-32768,0.1),1000.0);
        assert_eq!(advance_throttle(10.0,32767,0.1),0.0);
    }
    #[test]fn neutral_gate_rejects_held_controls(){
        assert!(centered([128,0,0,0,0,0],0));
        assert!(!centered([0,0,0,0,0,9000],0));
        assert!(!centered([0;6],1));assert!(!centered([10000,0,0,0,0,0],0));
    }
    #[test]fn verified_call_encodings(){
        assert_eq!(call_bytes(SITE_INIT,INIT),[0xe8,0x8e,0x12,0,0]);
        assert_eq!(call_bytes(SITE_POLL,POLL),[0xe8,0x56,0x15,0,0]);
        assert_eq!(call_bytes(SITE_RELEASE,RELEASE),[0xe8,0xfc,0x16,0,0]);
    }
}
