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
const SITE_CONSUME:usize=0x489824;
const CONSUME:usize=0x487460;
const PLAYER:usize=0x71c298;
const THROTTLE_INITIALIZED:usize=0x71c524;
const SITE_SLOT:usize=0x537588;
const NATIVE_SLOT:usize=0x537130;
const SITE_ACTION:usize=0x537458;
const NATIVE_ACTION:usize=0x537b80;
const SITE_CURSOR:usize=0x53825f;
const CURSOR_INTEGRATE:usize=0x5382b0;
const SITE_SCRIPT_KEY:usize=0x5d0616;
const SITE_KEYBOARD_POLL:usize=0x535611;
const KEYBOARD_POLL:usize=0x535a80;
// FMV update 0x44af20 tests Space at this call; on a fresh press it
// branches to its own skip/cleanup at 0x44af7c. No global key injection.
const SITE_FMV_SPACE:usize=0x44af3f;
// In-flight cinematic: original checks active animation at 0x4a02ac,
// then asks the nonblocking keyboard event reader for any key. A
// nonzero result follows the game's own animation termination path.
const SITE_SCENE_SKIP:usize=0x4a02b5;
const NATIVE_ANY_KEY:usize=0x536000;
const ACTIVE_SCENE_ANIMATION:usize=0x71c50c;
const FMV_SKIP_MASK:u32=(1<<0)|(1<<1)|(1<<6); // A, B, Start
const NATIVE_KEY:usize=0x535fa0;
// Exact default registrations at 0x4936c0 et seq., cross-checked with
// the original manual. GOS slots are one-based; no preferences writes.
const ACTIONS:[u32;10]=[0x13,0x14,0x24,0x25,0x19,0x12,0x31,0x2d,0x20,0x45];
const START_THROTTLE:i32=1000;
static UI_RENDER:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub(super) fn note_render_mode(w:u32,h:u32){UI_RENDER.store((w,h)==(800,600),Ordering::Release);}
fn slot_for_action(action:u32)->u32{
    ACTIONS.iter().position(|&a|a==action).map_or(0,|i|i as u32+1)
}
fn action_for_slot(slot:u32)->u32{
    slot.checked_sub(1).and_then(|i|ACTIONS.get(i as usize)).copied().unwrap_or(0)
}
fn key_edge(now:bool,previous:bool)->u32{
    match(now,previous){(true,false)=>1,(true,true)=>2,(false,true)=>4,_=>0}
}
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
    // RT/LT, shoulders, face buttons, L3 and Start; fixed action lookup below.
    let bits=[raw[5]>8192,raw[4]>8192,buttons&(1<<10)!=0,buttons&(1<<9)!=0,
        buttons&1!=0,buttons&2!=0,buttons&4!=0,buttons&8!=0,
        buttons&(1<<7)!=0,buttons&(1<<6)!=0];
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
    publish(&neutral(START_THROTTLE),false,true);
}
unsafe fn foreground()->bool{
    let mut pid=0;let hwnd=GetForegroundWindow();
    !hwnd.is_null()&&GetWindowThreadProcessId(hwnd,&mut pid)!=0&&pid==GetCurrentProcessId()
}
// Every connected SDL gamepad is monitored on the existing GOS poll thread.
// Only the chosen device can publish state. Never combine controller axes.
const BASIC_MASK:u32=0x7fff & !(1<<5); // exclude Guide
const SWITCH_IDLE:Duration=Duration::from_millis(350);
#[derive(Clone,Copy,Debug,Default)]
struct Activity{neutral_seen:bool,was_intent:bool}
impl Activity{
    fn observe(&mut self,raw:[i16;6],buttons:u32)->bool{
        if centered(raw,buttons&BASIC_MASK){self.neutral_seen=true;}
        let intent=raw[..4].iter().any(|&v|i32::from(v).abs()>=10000)
            ||raw[4]>8192||raw[5]>8192||buttons&BASIC_MASK!=0;
        let edge=self.neutral_seen&&intent&&!self.was_intent;
        self.was_intent=intent;
        edge
    }
}
struct Candidate{id:u32,handle:Obj,activity:Activity}
struct Flight{
    api:Api,pad:Obj,id:u32,armed:bool,throttle:f32,
    last:Instant,scan:Option<Instant>,last_log:Option<Instant>,last_state:[u8;STATE_LEN],
    was_foreground:bool,
    candidates:Vec<Candidate>,idle_since:Option<Instant>,
    raw:[i16;6],buttons:u32,previous_buttons:u32,
    last_aircraft:usize,last_consume:Option<Instant>,
    menu_armed:bool,menu_seen:bool,mouse_tick:Option<Instant>,
    menu_fraction:[f32;2],menu_reported:bool,
    last_fmv_skip_poll:Option<Instant>,
}
impl Flight{
    unsafe fn new()->Result<Self,String>{
        Ok(Self{api:Api::load()?,pad:ptr::null_mut(),id:0,armed:false,throttle:START_THROTTLE as f32,
            last:Instant::now(),scan:None,last_log:None,last_state:neutral(START_THROTTLE),
            was_foreground:false,candidates:Vec::new(),idle_since:None,raw:[0;6],buttons:0,previous_buttons:0,
            last_aircraft:0,last_consume:None,menu_armed:false,menu_seen:false,
            mouse_tick:None,menu_fraction:[0.0;2],menu_reported:false,last_fmv_skip_poll:None})
    }
    unsafe fn close_pad(&mut self){
        for c in self.candidates.drain(..){(self.api.close)(c.handle);}
        self.pad=ptr::null_mut();self.id=0;self.armed=false;self.scan=None;
        self.idle_since=None;self.clear_controls();
        crate::menu_controls_panel::device_removed();
    }
    unsafe fn select(&mut self,id:u32,handle:Obj){
        let old=self.id;
        self.id=id;self.pad=handle;self.armed=true;self.idle_since=None;
        self.raw=[0;6];self.buttons=0;self.previous_buttons=0;
        self.menu_armed=false;self.menu_seen=false;
        self.mouse_tick=None;self.menu_fraction=[0.0;2];
        self.last_log=None;
        publish(&neutral(self.throttle as i32),true,true);
        let n=(self.api.name)(handle);
        let name=if n.is_null(){"unknown".to_owned()}else{
            CStr::from_ptr(n).to_string_lossy().chars().take(100)
                .map(|c|if c.is_control(){' '}else{c}).collect()
        };
        crate::menu_controls_panel::device_selected((self.api.kind)(handle),&name);
        crate::log(&format!("SDL3-FLIGHT: selected id={id} name={name:?}; previous={old}; automatic activity; basic gamepad controls only"));
    }
    unsafe fn refresh_devices(&mut self,now:Instant){
        let mut i=0;
        while i<self.candidates.len(){
            let c=&self.candidates[i];
            if (self.api.connected)(c.handle){i+=1;continue;}
            let c=self.candidates.remove(i);
            if c.id==self.id{
                crate::log(&format!("SDL3-FLIGHT: disconnected id={}",self.id));
                self.pad=ptr::null_mut();self.id=0;self.armed=false;self.idle_since=None;
                self.clear_controls();
                crate::menu_controls_panel::device_removed();
                publish(&neutral(self.throttle as i32),false,true);
            }
            (self.api.close)(c.handle);
            self.scan=None;
        }
        if self.scan.is_none_or(|t|now.saturating_duration_since(t)>=Duration::from_millis(500)){
            self.scan=Some(now);
            let mut ids=self.api.ids(self.api.pads);ids.sort_unstable();ids.dedup();
            let before=self.candidates.len();
            for id in ids{
                if self.candidates.len()>=32{break;}
                if self.candidates.iter().any(|c|c.id==id){continue;}
                let handle=(self.api.open)(id);
                if !handle.is_null(){self.candidates.push(Candidate{id,handle,activity:Activity::default()});}
            }
            self.candidates.sort_by_key(|c|c.id);
            if before!=self.candidates.len(){
                crate::log(&format!("SDL3-FLIGHT: monitored gamepads={:?}; selection requires neutral then activity",self.candidates.iter().map(|c|c.id).collect::<Vec<_>>()));
            }
        }
    }
    unsafe fn choose_device(&mut self,now:Instant)->Option<([i16;6],u32)>{
        self.refresh_devices(now);
        let mut current=None;
        let mut contender=None;
        for c in &mut self.candidates{
            let raw=std::array::from_fn(|i|(self.api.axis)(c.handle,i as i32));
            let buttons=(0..15).fold(0u32,|m,i|m|(((self.api.button)(c.handle,i) as u32)<<i))&BASIC_MASK;
            let edge=c.activity.observe(raw,buttons);
            if c.id==self.id{current=Some((raw,buttons));}
            else if edge&&contender.is_none(){contender=Some((c.id,c.handle,raw,buttons));}
        }
        let idle=if let Some((raw,buttons))=current{
            if centered(raw,buttons){
                let since=*self.idle_since.get_or_insert(now);
                now.saturating_duration_since(since)>=SWITCH_IDLE
            }else{self.idle_since=None;false}
        }else{true};
        if idle{
            if let Some((id,handle,raw,buttons))=contender{
                self.select(id,handle);
                return Some((raw,buttons));
            }
        }
        current
    }
    fn clear_controls(&mut self){
        self.raw=[0;6];self.buttons=0;self.previous_buttons=0;
        self.last_consume=None;self.menu_armed=false;self.menu_seen=false;
        self.mouse_tick=None;self.menu_fraction=[0.0;2];
    }
    // The movie reader consumes at most one rising edge per GOS poll.
    // A held button cannot skip a chain of successive clips.
    unsafe fn cinematic_skip(&mut self,now:Instant,scene_3d:bool)->bool{
        if !self.armed||!self.was_foreground||self.pad.is_null()||!foreground()
            || !(self.api.connected)(self.pad)
            || now.saturating_duration_since(self.last)>=Duration::from_millis(150)
            || !fresh_fmv_skip(self.buttons,self.previous_buttons,self.last_fmv_skip_poll==Some(self.last)){
            return false;
        }
        self.last_fmv_skip_poll=Some(self.last);
        let path=if scene_3d{"SDL3-CINEMATIC3D: skip requested by active gamepad"}else{"SDL3-FMV: skip requested by active gamepad"};
        crate::log(&format!("{path} id={}; native cinematic skip path",self.id));
        true
    }
    fn flight_recent(&self,now:Instant)->bool{
        self.last_consume.is_some_and(|t|now.saturating_duration_since(t)<Duration::from_millis(250))
    }
    unsafe fn menu_context(&self,now:Instant)->bool{
        self.armed&&self.was_foreground&&foreground()
            && now.saturating_duration_since(self.last)<Duration::from_millis(150)
            && UI_RENDER.load(Ordering::Acquire)
            && crate::menu_scene::main_menu_was_seen()
            && !self.flight_recent(now)
            && (read32(0x75ccb8),read32(0x75ccbc))==(800,600)
    }
    unsafe fn consume(&mut self,aircraft:Obj){
        // The authenticated player-control CALL, not input polling, owns
        // throttle changes. Reset on newly observed player object, not pause.
        if aircraft.is_null() || aircraft as usize!=read32(PLAYER) as usize
            || ptr::read_volatile((aircraft as usize+0x91d) as *const u8)!=0{return;}
        let now=Instant::now();
        let new=self.last_aircraft!=aircraft as usize;
        let dt=self.last_consume.map_or(0.0,|t|now.saturating_duration_since(t).as_secs_f32());
        self.last_consume=Some(now);self.last_aircraft=aircraft as usize;
        self.menu_seen=false;self.menu_armed=false;
        if new{
            self.throttle=START_THROTTLE as f32;
            ptr::write_volatile(THROTTLE_INITIALIZED as *mut u8,0);
            crate::log("SDL3-FLIGHT: new player aircraft; throttle initialized to 100 percent");
        }else if self.armed&&foreground()&&read32(READY)==1&&dt<=0.25{
            self.throttle=advance_throttle(self.throttle,self.raw[3],dt);
        }
        if self.armed&&read32(READY)==1&&foreground(){
            write32(STATE+8,(1000-2*self.throttle.round() as i32) as u32);
        }
    }
    unsafe fn menu_cursor(&mut self){
        let now=Instant::now();
        if crate::menu_controls_panel::is_open(){return;}
        if !self.menu_context(now){
            self.menu_seen=false;self.menu_armed=false;self.mouse_tick=None;
            self.menu_fraction=[0.0;2];return;
        }
        let dt=self.mouse_tick.map_or(0.0,|t|now.saturating_duration_since(t).as_secs_f32()).min(0.05);
        self.mouse_tick=Some(now);
        if !self.menu_seen{
            self.menu_seen=true;self.menu_armed=false;self.menu_fraction=[0.0;2];
        }
        if !self.menu_armed&&self.buttons&3==0{self.menu_armed=true;}
        if !self.menu_armed{return;}
        let horizontal=i32::from(self.buttons&(1<<14)!=0)-i32::from(self.buttons&(1<<13)!=0);
        let vertical=i32::from(self.buttons&(1<<12)!=0)-i32::from(self.buttons&(1<<11)!=0);
        let v=[if horizontal==0{signed_axis(self.raw[0])}else{horizontal*1000},
               if vertical==0{signed_axis(self.raw[1])}else{vertical*1000}];
        for (i,(address,speed_address)) in [(0x75ccd8,0x6357f4),(0x75ccdc,0x6357f8)].into_iter().enumerate(){
            let speed=f32::from_bits(read32(speed_address));
            if !speed.is_finite()||!(0.25..=8.0).contains(&speed){self.menu_fraction[i]=0.0;continue;}
            let (delta,fraction)=cursor_delta(v[i],dt,speed,self.menu_fraction[i]);
            self.menu_fraction[i]=fraction;
            if delta!=0{write32(address,(read32(address) as i32).saturating_add(delta) as u32);}
        }
        if self.buttons&1!=0{
            // Original poll already saved previous mouse state; the native
            // button edge reader will observe the merged left click.
            let b=0x75cadc as *mut u8;
            ptr::write_volatile(b,ptr::read_volatile(b)|0x80);
        }
        if !self.menu_reported&&(v!=[0,0]||self.buttons&1!=0){
            self.menu_reported=true;
            crate::log("SDL3-MENU: native GOS cursor bridge active; left stick/D-pad move, A clicks, B script Escape");
        }
    }
    unsafe fn tick(&mut self)->bool{
        let now=Instant::now();
        self.last=now;
        if read32(PLAYER)==0{self.last_aircraft=0;self.last_consume=None;}
        if !foreground(){
            if self.was_foreground{crate::log("SDL3-FLIGHT: focus lost; neutral and disarmed");}
            self.was_foreground=false;self.armed=false;self.clear_controls();
            crate::menu_controls_panel::reset_input();
            self.idle_since=None;
            for c in &mut self.candidates{c.activity=Activity::default();}
            publish(&neutral(self.throttle as i32),false,true);return false;
        }
        if !self.was_foreground{
            crate::log("SDL3-FLIGHT: focus acquired; waiting for neutral controls");
            self.was_foreground=true;self.armed=false;self.clear_controls();
        }
        (self.api.update)();
        let Some((raw,buttons))=self.choose_device(now) else{
            publish(&neutral(self.throttle as i32),false,true);return false;
        };
        if !self.armed{
            if !centered(raw,buttons){publish(&neutral(self.throttle as i32),false,true);return false;}
            self.armed=true;crate::log("SDL3-FLIGHT: neutral acquired; analog controls armed");
            publish(&neutral(self.throttle as i32),true,true);return true;
        }
        let axis=raw[..4].iter().any(|&v|i32::from(v).abs()>=10000)||raw[4]>8192||raw[5]>8192;
        let captured=crate::menu_controls_panel::pad_event(buttons,axis,centered(raw,buttons),self.menu_context(now));
        let (raw,buttons)=if captured{([0;6],0)}else{(raw,buttons)};
        self.raw=raw;self.previous_buttons=self.buttons;self.buttons=buttons;
        // Outside a recent player-control call, suppress combat slots.
        // Start remains available for the native mission menu.
        let state=if self.flight_recent(now){
            provisional_state(raw,buttons,self.throttle.round() as i32)
        }else{
            provisional_state([0;6],buttons&(1<<6),self.throttle.round() as i32)
        };
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
                publish(&neutral(START_THROTTLE),false,true);
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
fn cursor_delta(axis:i32,seconds:f32,speed:f32,remainder:f32)->(i32,f32){
    let motion=axis.clamp(-1000,1000) as f32*0.45*seconds.clamp(0.0,0.05)/speed+remainder;
    let delta=motion.trunc() as i32;
    (delta,motion-delta as f32)
}
unsafe extern "C" fn consume_hook(aircraft:Obj){
    if MODE.load(Ordering::Acquire)==ACTIVE&&OWNER.load(Ordering::Acquire)==GetCurrentThreadId(){
        FLIGHT.with(|s|if let Ok(mut s)=s.try_borrow_mut(){if let Some(f)=s.as_mut(){f.consume(aircraft);}});
    }
    let original:unsafe extern "C" fn(Obj)=std::mem::transmute(CONSUME);
    original(aircraft);
}
unsafe extern "thiscall" fn slot_hook(context:Obj,action:u32)->u32{
    if MODE.load(Ordering::Acquire)==ACTIVE{return slot_for_action(action);}
    let original:unsafe extern "thiscall" fn(Obj,u32)->u32=std::mem::transmute(NATIVE_SLOT);
    original(context,action)
}
unsafe extern "C" fn action_hook(slot:u32)->u32{
    if MODE.load(Ordering::Acquire)==ACTIVE{return action_for_slot(slot);}
    let original:unsafe extern "C" fn(u32)->u32=std::mem::transmute(NATIVE_ACTION);
    original(slot)
}
// The existing exact pre-integration mouse site sees the real native
// DIMOUSESTATE before edge computation and callbacks. Consume dismissal
// there, including its release, rather than clicking through the diagram.
unsafe fn capture_modal_mouse()->bool{
    let buttons=read32(0x75cadc)&0x80808080;
    let moved=[0x75ccd8,0x75ccdc,0x75cce0].into_iter()
        .any(|a|(read32(a) as i32).unsigned_abs()>=2);
    if !crate::menu_controls_panel::mouse_event(buttons,moved){return false;}
    for a in [0x75cad0,0x75cad4,0x75cad8,0x75cadc,0x75caec,0x75ccd8,0x75ccdc,0x75cce0]{write32(a,0);}
    true
}
// Exact original keyboard poll reads 16-byte DIDEVICEOBJECTDATA records
// via the native device's GetDeviceData slot 10. During the modal only,
// drain that same queue BEFORE its original dispatch and age old events
// using the game's own routine. No callbacks or native menu key presses
// are generated by the consumed dismissal. Restore normal poll on error.
#[repr(C)]#[derive(Clone,Copy,Default)]
struct KeyEvent{offset:u32,data:u32,time:u32,sequence:u32}
unsafe extern "C" fn keyboard_poll_hook(callback:i32){
    let original:unsafe extern "C" fn(i32)=std::mem::transmute(KEYBOARD_POLL);
    if MODE.load(Ordering::Acquire)!=ACTIVE||OWNER.load(Ordering::Acquire)!=GetCurrentThreadId()
        ||!foreground()||!crate::menu_controls_panel::keyboard_capture(){original(callback);return;}
    let device=read32(0x7582cc) as Obj;
    if device.is_null(){crate::menu_controls_panel::fail_preview();original(callback);return;}
    let table=*(device as *const *const usize);
    if table.is_null(){crate::menu_controls_panel::fail_preview();original(callback);return;}
    let method:unsafe extern "system" fn(Obj,u32,*mut KeyEvent,*mut u32,u32)->i32=std::mem::transmute(*table.add(10));
    let age:unsafe extern "C" fn()=std::mem::transmute(0x535a00usize);
    age();
    write32(0x7582d4,0);
    write32(0x75cb60,0);
    let mut events=[KeyEvent::default();128];let mut count=128u32;
    let hr=method(device,16,events.as_mut_ptr(),&mut count,0);
    if hr<0||count>128{
        crate::menu_controls_panel::fail_preview();original(callback);return;
    }
    for e in &events[..count as usize]{
        let scan=e.offset as usize;
        if scan>=256{continue;}
        crate::menu_controls_panel::keyboard_event(scan,e.data&0x80!=0);
        // Clear only the key's native state, never its adjacent callback.
        // Original GOS bounds the scan/modifier composite to <0x7de.
        for modifier in 0..8{
            let key=scan|(modifier<<8);
            if key<0x7de{write32(0x7582e4+key*8,0);}
        }
        if matches!(scan,0x1d|0x9d|0x2a|0x36|0x38|0xb8){write32(0x7582e0,0);}
    }
}
unsafe extern "C" fn cursor_hook(){
    if MODE.load(Ordering::Acquire)==ACTIVE&&OWNER.load(Ordering::Acquire)==GetCurrentThreadId(){
        if !capture_modal_mouse(){FLIGHT.with(|s|if let Ok(mut s)=s.try_borrow_mut(){if let Some(f)=s.as_mut(){f.menu_cursor();}});}
    }
    let original:unsafe extern "C" fn()=std::mem::transmute(CURSOR_INTEGRATE);
    original();
}
unsafe extern "C" fn script_key_hook(key:u32)->u32{
    let original:unsafe extern "C" fn(u32)->u32=std::mem::transmute(NATIVE_KEY);
    let native=original(key);
    if crate::menu_controls_panel::is_open(){return 0;}
    if native!=0||key!=1||MODE.load(Ordering::Acquire)!=ACTIVE
        ||OWNER.load(Ordering::Acquire)!=GetCurrentThreadId(){return native;}
    FLIGHT.with(|s|s.try_borrow().ok().and_then(|s|s.as_ref().and_then(|f|{
        (f.menu_armed&&f.menu_context(Instant::now())).then(||key_edge(f.buttons&2!=0,f.previous_buttons&2!=0))
    })).unwrap_or(0))
}
fn fresh_fmv_skip(buttons:u32,previous:u32,consumed:bool)->bool{
    !consumed && buttons&FMV_SKIP_MASK!=0 && previous&FMV_SKIP_MASK==0
}
unsafe extern "C" fn fmv_space_hook(key:u32)->u32{
    let original:unsafe extern "C" fn(u32)->u32=std::mem::transmute(NATIVE_KEY);
    let native=original(key);
    if native==1||key!=0x39||MODE.load(Ordering::Acquire)!=ACTIVE
        ||OWNER.load(Ordering::Acquire)!=GetCurrentThreadId(){return native;}
    let skip=FLIGHT.with(|s|s.try_borrow_mut().ok()
        .and_then(|mut s|s.as_mut().map(|f|f.cinematic_skip(Instant::now(),false))).unwrap_or(false));
    if skip{1}else{native}
}
unsafe extern "C" fn scene_skip_hook(wait:u32)->u32{
    let original:unsafe extern "C" fn(u32)->u32=std::mem::transmute(NATIVE_ANY_KEY);
    let native=original(wait);
    if native!=0||wait!=0||MODE.load(Ordering::Acquire)!=ACTIVE
        ||OWNER.load(Ordering::Acquire)!=GetCurrentThreadId()
        ||read32(ACTIVE_SCENE_ANIMATION)==0{return native;}
    let skip=FLIGHT.with(|s|s.try_borrow_mut().ok()
        .and_then(|mut s|s.as_mut().map(|f|f.cinematic_skip(Instant::now(),true))).unwrap_or(false));
    // The verified caller only tests zero/nonzero; no synthetic global
    // keyboard event or direct animation-state mutation is required.
    if skip{1}else{native}
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
            (SITE_CONSUME,call_bytes(SITE_CONSUME,CONSUME).to_vec(),call_bytes(SITE_CONSUME,consume_hook as usize).to_vec()),
            (SITE_SLOT,call_bytes(SITE_SLOT,NATIVE_SLOT).to_vec(),call_bytes(SITE_SLOT,slot_hook as usize).to_vec()),
            (SITE_ACTION,call_bytes(SITE_ACTION,NATIVE_ACTION).to_vec(),call_bytes(SITE_ACTION,action_hook as usize).to_vec()),
            (SITE_CURSOR,call_bytes(SITE_CURSOR,CURSOR_INTEGRATE).to_vec(),call_bytes(SITE_CURSOR,cursor_hook as usize).to_vec()),
            (SITE_SCRIPT_KEY,call_bytes(SITE_SCRIPT_KEY,NATIVE_KEY).to_vec(),call_bytes(SITE_SCRIPT_KEY,script_key_hook as usize).to_vec()),
            (SITE_KEYBOARD_POLL,call_bytes(SITE_KEYBOARD_POLL,KEYBOARD_POLL).to_vec(),call_bytes(SITE_KEYBOARD_POLL,keyboard_poll_hook as usize).to_vec()),
            (SITE_FMV_SPACE,call_bytes(SITE_FMV_SPACE,NATIVE_KEY).to_vec(),call_bytes(SITE_FMV_SPACE,fmv_space_hook as usize).to_vec()),
            (SITE_SCENE_SKIP,call_bytes(SITE_SCENE_SKIP,NATIVE_ANY_KEY).to_vec(),call_bytes(SITE_SCENE_SKIP,scene_skip_hook as usize).to_vec()),
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
        let pages=[0x535000usize,0x487000usize,0x489000usize,0x537000usize,0x538000usize,0x5d0000usize,0x44a000usize,0x4a0000usize];let mut prot=[0u32;8];
        for i in 0..pages.len(){
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
        for i in 0..pages.len(){let mut ignored=0;VirtualProtect(pages[i] as Obj,0x1000,prot[i],&mut ignored);}
        if flushed{MODE.store(WAIT,Ordering::Release);crate::log("SDL3-FLIGHT: twelve exact in-memory sites installed; fixed actions, flight-only throttle, menu bridge, FMV/3D skip, controls modal");}
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
    #[test]fn selection_requires_neutral_and_intent_edge(){
        let mut a=Activity::default();
        assert!(!a.observe([20000,0,0,0,0,0],0));
        assert!(!a.observe([0;6],1));
        assert!(!a.observe([0;6],0));
        assert!(!a.observe([9000,0,0,0,0,0],0));
        assert!(a.observe([10000,0,0,0,0,0],0));
        assert!(!a.observe([15000,0,0,0,0,0],0));
        assert!(!a.observe([0;6],0));
        assert!(a.observe([0;6],1));
        assert!(!a.observe([0;6],1));
    }
    #[test]fn inactive_guide_and_small_drift_do_not_select(){
        let mut a=Activity::default();
        assert!(!a.observe([128;6],0));
        assert!(!a.observe([0;6],1<<5));
        assert!(!a.observe([0;6],1<<20));
        assert!(a.observe([0,0,0,0,0,12000],0));
    }
    #[test]fn fixed_slots_are_bijective(){
        for (i,&action) in ACTIONS.iter().enumerate(){
            assert_eq!(slot_for_action(action),i as u32+1);
            assert_eq!(action_for_slot(i as u32+1),action);
        }
        assert_eq!(slot_for_action(0x7),0);
        assert_eq!(action_for_slot(0),0);assert_eq!(action_for_slot(11),0);
    }
    #[test]fn menu_click_edges_and_fractional_cursor_motion(){
        assert_eq!([key_edge(false,false),key_edge(true,false),key_edge(true,true),key_edge(false,true)],[0,1,2,4]);
        let (delta,rem)=cursor_delta(1000,0.01,2.0,0.0);
        assert_eq!(delta,2);assert!((rem-0.25).abs()<0.0001);
        assert_eq!(cursor_delta(-1000,0.05,1.0,0.0).0,-22);
        assert_eq!(axis(&neutral(START_THROTTLE),2),-1000);
    }
    #[test]fn fmv_skip_is_fresh_and_only_allowed_buttons(){
        for button in [1,2,1<<6]{
            assert!(fresh_fmv_skip(button,0,false));
            assert!(!fresh_fmv_skip(button,button,false));
            assert!(!fresh_fmv_skip(button,0,true));
        }
        assert!(!fresh_fmv_skip(4,0,false));
        assert!(!fresh_fmv_skip(0,1,false));
        assert!(!fresh_fmv_skip(2,1,false));
        assert_eq!(call_bytes(SITE_FMV_SPACE,NATIVE_KEY),[0xe8,0x5c,0xb0,0x0e,0]);
        assert_eq!(call_bytes(SITE_SCENE_SKIP,NATIVE_ANY_KEY),[0xe8,0x46,0x5d,0x09,0]);
        assert_eq!(call_bytes(SITE_KEYBOARD_POLL,KEYBOARD_POLL),[0xe8,0x6a,0x04,0,0]);
        assert_eq!(std::mem::size_of::<KeyEvent>(),16);
    }
    #[test]fn verified_call_encodings(){
        assert_eq!(call_bytes(SITE_INIT,INIT),[0xe8,0x8e,0x12,0,0]);
        assert_eq!(call_bytes(SITE_POLL,POLL),[0xe8,0x56,0x15,0,0]);
        assert_eq!(call_bytes(SITE_RELEASE,RELEASE),[0xe8,0xfc,0x16,0,0]);
        assert_eq!(call_bytes(SITE_CONSUME,CONSUME),[0xe8,0x37,0xdc,0xff,0xff]);
        assert_eq!(call_bytes(SITE_SLOT,NATIVE_SLOT),[0xe8,0xa3,0xfb,0xff,0xff]);
        assert_eq!(call_bytes(SITE_ACTION,NATIVE_ACTION),[0xe8,0x23,0x07,0,0]);
        assert_eq!(call_bytes(SITE_CURSOR,CURSOR_INTEGRATE),[0xe8,0x4c,0,0,0]);
        assert_eq!(call_bytes(SITE_SCRIPT_KEY,NATIVE_KEY),[0xe8,0x85,0x59,0xf6,0xff]);
    }
}
