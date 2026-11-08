//! Opt-in, narrowly scoped Win32 background-erase suppression experiment.
//!
//! Only the exact process-owned game HWND provided by the DD4 adapter can
//! be subclassed. We do not paint, call DirectDraw, allocate or log inside
//! the window procedure. All messages except WM_ERASEBKGND go untouched
//! to the original procedure. It is unhooked during WM_NCDESTROY.
//! This does not guarantee that WM_PAINT never paints white itself.
#![allow(dead_code)]
use std::{cell::Cell,ffi::c_void,sync::atomic::{AtomicIsize,AtomicUsize,Ordering}};

type Hwnd=*mut c_void;
const GWL_WNDPROC:i32=-4;
const WM_ERASEBKGND:u32=0x0014;
const WM_PAINT:u32=0x000f;
const WM_NCDESTROY:u32=0x0082;
const WM_KEYDOWN:u32=0x0100;
const WM_KEYUP:u32=0x0101;
const WM_CHAR:u32=0x0102;
const WM_SYSKEYDOWN:u32=0x0104;
const WM_SYSKEYUP:u32=0x0105;
const WM_SETFOCUS:u32=0x0007;
const WM_KILLFOCUS:u32=0x0008;
const WM_ACTIVATEAPP:u32=0x001c;
const WM_MOUSEMOVE:u32=0x0200;
const WM_SETCURSOR:u32=0x0020;
const WM_LBUTTONDOWN:u32=0x0201;
const WM_LBUTTONUP:u32=0x0202;
const WM_RBUTTONDOWN:u32=0x0204;
const WM_RBUTTONUP:u32=0x0205;
const WM_MBUTTONDOWN:u32=0x0207;
const WM_MBUTTONUP:u32=0x0208;
const CURSOR_SHOWING:u32=0x1;
#[repr(C)]
#[derive(Default,Clone,Copy)]
struct Point{x:i32,y:i32}
#[repr(C)]
struct CursorInfo{size:u32,flags:u32,hcursor:Hwnd,screen:Point}
#[repr(C)]
struct WinMessage{
    hwnd:Hwnd,message:u32,wparam:usize,lparam:isize,time:u32,
    point:Point,lprivate:u32
}
#[repr(C)]
#[derive(Default)]
struct MouseRect{left:i32,top:i32,right:i32,bottom:i32}
const PM_REMOVE:u32=1;
const NATIVE_MOVIE_PUMP_MAX_PER_FLIP:usize=4;
thread_local!{
    static IN_NATIVE_MOVIE_PUMP:Cell<bool>=const{Cell::new(false)};
    // ShowCursor display count is owned by this Win32 UI thread. Keep
    // exact increments for reversible cleanup on scene exit/mode change.
    static NATIVE_CURSOR_PROBE_SHOW_INCREMENTS:Cell<u32>=const{Cell::new(0)};
}

#[link(name="user32")]
extern "system"{
    fn IsWindow(hwnd:Hwnd)->i32;
    fn GetForegroundWindow()->Hwnd;
    fn GetInputState()->i32;
    fn GetCursorInfo(info:*mut CursorInfo)->i32;
    fn GetCursorPos(pos:*mut Point)->i32;
    fn ScreenToClient(hwnd:Hwnd,pos:*mut Point)->i32;
    fn PeekMessageW(msg:*mut WinMessage,hwnd:Hwnd,first:u32,last:u32,remove:u32)->i32;
    fn TranslateMessage(msg:*const WinMessage)->i32;
    fn DispatchMessageW(msg:*const WinMessage)->isize;
    fn GetWindowThreadProcessId(hwnd:Hwnd,pid:*mut u32)->u32;
    fn GetClientRect(hwnd:Hwnd,rect:*mut MouseRect)->i32;
    fn LoadCursorW(instance:Hwnd,name:*const u16)->Hwnd;
    fn SetCursor(cursor:Hwnd)->Hwnd;
    fn ShowCursor(show:i32)->i32;
    fn GetWindowLongW(hwnd:Hwnd,index:i32)->i32;
    fn SetWindowLongW(hwnd:Hwnd,index:i32,value:i32)->i32;
    fn CallWindowProcW(proc:isize,hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize;
    fn DefWindowProcW(hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize;
}
#[link(name="kernel32")]
extern "system"{
    fn GetCurrentProcessId()->u32;
    fn GetCurrentThreadId()->u32;
    fn GetLastError()->u32;
    fn SetLastError(code:u32);
}

static HOOK_HWND:AtomicUsize=AtomicUsize::new(0);
static PREVIOUS:AtomicIsize=AtomicIsize::new(0);
static ERASE_COUNT:AtomicUsize=AtomicUsize::new(0);
static PAINT_COUNT:AtomicUsize=AtomicUsize::new(0);
// Passive observations only. Never swallow, synthesize or dispatch input.
static KEYDOWN_COUNT:AtomicUsize=AtomicUsize::new(0);
static KEYUP_COUNT:AtomicUsize=AtomicUsize::new(0);
static CHAR_COUNT:AtomicUsize=AtomicUsize::new(0);
static FOCUS_COUNT:AtomicUsize=AtomicUsize::new(0);
static MOUSE_MOVE_COUNT:AtomicUsize=AtomicUsize::new(0);
static MOUSE_BUTTON_COUNT:AtomicUsize=AtomicUsize::new(0);
static MOUSE_LAST_X:AtomicIsize=AtomicIsize::new(0);
static MOUSE_LAST_Y:AtomicIsize=AtomicIsize::new(0);
// Armed only by native Windows RGB32 800x600 resource preflight while
// explicit NativeMovieMessagePump=1. Never used in Wine or ordinary graph.
static NATIVE_MOVIE_PUMP_HWND:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MOVIE_PUMP_CALLS:AtomicUsize=AtomicUsize::new(0);
static NATIVE_MOVIE_PUMP_MESSAGES:AtomicUsize=AtomicUsize::new(0);
// Strictly native Windows 800x600 movie/menu opt-in. The exact verified
// game HWND is armed only after DD4 preflight. The MainMenu/Preferences
// script observer enables translations solely while a menu is active.
static MENU_MOUSE_HWND:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_CLIENT_W:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_CLIENT_H:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_RENDER_W:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_RENDER_H:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_SCENE_ACTIVE:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_MAPPED:AtomicUsize=AtomicUsize::new(0);
static MENU_MOUSE_LAST_X:AtomicIsize=AtomicIsize::new(0);
static MENU_MOUSE_LAST_Y:AtomicIsize=AtomicIsize::new(0);
// Independent menu-only opt-in to make the native OS arrow visible,
// allowing a controlled distinction between hidden OS cursor and the
// game's missing cursor rendering. The normal/legacy cursor is not changed.
static MENU_SYSTEM_CURSOR_PROBE_HWND:AtomicUsize=AtomicUsize::new(0);
static MENU_SYSTEM_CURSOR_PROBE_EVENTS:AtomicUsize=AtomicUsize::new(0);
static MENU_SYSTEM_CURSOR_PROBE_RESTORES:AtomicUsize=AtomicUsize::new(0);
// Independent native menu GOS mouse bridge: DirectInput's original buttons
// remain untouched, only relative deltas are matched to actual viewport
// coordinates by the explicitly armed 800x600 input device hook.
static MENU_GOS_BRIDGE_HWND:AtomicUsize=AtomicUsize::new(0);
static MENU_GOS_BRIDGE_CLIENT_W:AtomicUsize=AtomicUsize::new(0);
static MENU_GOS_BRIDGE_CLIENT_H:AtomicUsize=AtomicUsize::new(0);
static MENU_GOS_BRIDGE_RENDER_W:AtomicUsize=AtomicUsize::new(0);
static MENU_GOS_BRIDGE_RENDER_H:AtomicUsize=AtomicUsize::new(0);

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowPaintStats{
    pub erased:usize,pub painted:usize,
    pub keydown:usize,pub keyup:usize,pub character:usize,pub focus_changes:usize,
    pub mouse_moves:usize,pub mouse_buttons:usize,
    pub menu_mouse_mapped:usize,
    pub system_cursor_probe_events:usize,
    pub system_cursor_probe_restores:usize,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WindowInputState{
    pub counts:WindowPaintStats,
    pub game_is_foreground:bool,
    // GetInputState reports the CALLING thread's queue, not all windows.
    pub queue_has_new_input:bool,
    pub window_thread:u32,
    pub current_thread:u32,
    pub cursor_visible:Option<bool>,
    pub cursor_screen:Option<(i32,i32)>,
    pub cursor_client_last_move:Option<(i32,i32)>,
    pub menu_mouse_logical_last_move:Option<(i32,i32)>,
}
fn input_event(msg:u32)->(bool,bool,bool,bool){
    (matches!(msg,WM_KEYDOWN|WM_SYSKEYDOWN),
     matches!(msg,WM_KEYUP|WM_SYSKEYUP),
     msg==WM_CHAR,
     matches!(msg,WM_SETFOCUS|WM_KILLFOCUS|WM_ACTIVATEAPP))
}

fn mouse_button_message(msg:u32)->bool{
    matches!(msg,WM_LBUTTONDOWN|WM_LBUTTONUP|
        WM_RBUTTONDOWN|WM_RBUTTONUP|WM_MBUTTONDOWN|WM_MBUTTONUP)
}
fn client_mouse_point(lparam:isize)->(i32,i32){
    let packed=lparam as u32;
    ((packed as u16 as i16) as i32,((packed>>16) as u16 as i16) as i32)
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct MenuRect{left:i32,top:i32,width:i32,height:i32}
fn menu_content_rect(
    client_w:i32,client_h:i32,render_w:i32,render_h:i32
)->Option<MenuRect>{
    if [client_w,client_h,render_w,render_h].iter()
        .any(|&v|v<=0 || v>16384){return None;}
    // Always use the SAME physical DD4 viewport + per-edge DPI scaling
    // as the actual NativeWindowedBlt presentation. Independently
    // fitting a 4:3 rectangle in Win32 coordinates can differ by a pixel
    // at fractional DPI or odd-sized monitors.
    let view=crate::presentation::output_viewport(
        (800,600),(render_w as u32,render_h as u32),
        (client_w as u32,client_h as u32)
    )?;
    Some(MenuRect{
        left:i32::try_from(view.x).ok()?,
        top:i32::try_from(view.y).ok()?,
        width:i32::try_from(view.width).ok()?,
        height:i32::try_from(view.height).ok()?
    })
}
fn menu_mouse_point(
    point:(i32,i32),client_w:i32,client_h:i32,
    render_w:i32,render_h:i32
)->Option<(i32,i32)>{
    let view=menu_content_rect(client_w,client_h,render_w,render_h)?;
    // Intentionally do not clamp to the viewport: negative/outside
    // coordinates tell the game that the pointer left its 4:3 canvas.
    let x=i64::from(point.0-view.left)*800/i64::from(view.width);
    let y=i64::from(point.1-view.top)*600/i64::from(view.height);
    let x=i16::try_from(x).ok()?;
    let y=i16::try_from(y).ok()?;
    Some((i32::from(x),i32::from(y)))
}
fn menu_mouse_message(msg:u32)->bool{
    msg==WM_MOUSEMOVE || mouse_button_message(msg)
}
fn menu_mouse_lparam(hwnd:Hwnd,msg:u32,lparam:isize)->isize{
    if !menu_mouse_message(msg) ||
        MENU_MOUSE_HWND.load(Ordering::Acquire)!=hwnd as usize ||
        MENU_MOUSE_SCENE_ACTIVE.load(Ordering::Acquire)==0{
        return lparam;
    }
    let width=MENU_MOUSE_CLIENT_W.load(Ordering::Acquire) as i32;
    let height=MENU_MOUSE_CLIENT_H.load(Ordering::Acquire) as i32;
    let render_w=MENU_MOUSE_RENDER_W.load(Ordering::Acquire) as i32;
    let render_h=MENU_MOUSE_RENDER_H.load(Ordering::Acquire) as i32;
    let Some((x,y))=menu_mouse_point(
        client_mouse_point(lparam),width,height,render_w,render_h
    ) else{
        return lparam;
    };
    MENU_MOUSE_LAST_X.store(x as isize,Ordering::Relaxed);
    MENU_MOUSE_LAST_Y.store(y as isize,Ordering::Relaxed);
    MENU_MOUSE_MAPPED.fetch_add(1,Ordering::Relaxed);
    let packed=(u32::from(x as i16 as u16)) |
        (u32::from(y as i16 as u16)<<16);
    packed as isize
}
/// Return to the exact ShowCursor count observed before our diagnostic
/// activated. Never use this while another thread owns the game HWND.
fn undo_native_system_cursor_probe(){
    NATIVE_CURSOR_PROBE_SHOW_INCREMENTS.with(|increment|{
        let count=increment.replace(0);
        for _ in 0..count{
            unsafe{ShowCursor(0);}
        }
        if count>0{
            MENU_SYSTEM_CURSOR_PROBE_RESTORES.fetch_add(1,Ordering::Relaxed);
        }
    });
}
fn native_system_cursor_probe_message(message:u32)->bool{
    matches!(message,WM_SETCURSOR|WM_MOUSEMOVE)
}
/// This is a temporary A/B diagnostic, NOT a final game cursor fix.
/// The game receives original Win32 events first; AFTER its WndProc
/// returns, show the Windows arrow in MainMenu/Preferences only. Never
/// change input coordinates, fake events or paint into DirectDraw.
fn after_game_wndproc_system_cursor_probe(hwnd:Hwnd,message:u32)->bool{
    if MENU_SYSTEM_CURSOR_PROBE_HWND.load(Ordering::Acquire)!=hwnd as usize ||
       MENU_MOUSE_SCENE_ACTIVE.load(Ordering::Acquire)==0 ||
       !native_system_cursor_probe_message(message){return false;}
    unsafe{
        // Win32 standard arrow, not a custom file or a global cursor change.
        let arrow=LoadCursorW(std::ptr::null_mut(),32512usize as *const u16);
        if arrow.is_null(){return false;}
        // ShowCursor is a per-thread reference count; keep any adjustments
        // bounded, one-time and reversible. Never repeatedly increment it
        // on each mousemove; the original game remains the cursor owner.
        let mut visible=NATIVE_CURSOR_PROBE_SHOW_INCREMENTS.with(|n|n.get()>0);
        if !visible{
            let mut count=0u32;
            for _ in 0..16{
                count+=1;
                if ShowCursor(1)>=0{visible=true;break;}
            }
            if !visible{
                for _ in 0..count{ShowCursor(0);}
                return false;
            }
            NATIVE_CURSOR_PROBE_SHOW_INCREMENTS.with(|n|n.set(count));
        }
        SetCursor(arrow);
        MENU_SYSTEM_CURSOR_PROBE_EVENTS.fetch_add(1,Ordering::Relaxed);
        true
    }
}
/// Must be called on the verified game window's UI thread after the
/// 800x600 native RGB32 DD4 graph and HWND borderless geometry succeed.
pub fn arm_native_system_cursor_probe(
    hwnd:usize,menu_mouse_remap_enabled:bool
)->Result<(),String>{
    if hwnd==0 || snapshot(hwnd).is_none(){
        return Err("verified game HWND/WndProc guard missing".into());
    }
    if menu_mouse_remap_enabled{
        return Err("NativeMenuMouseRemap=1; disable to isolate the OS arrow probe".into());
    }
    unsafe {
        let mut pid=0;
        let owner=GetWindowThreadProcessId(hwnd as Hwnd,&mut pid);
        if IsWindow(hwnd as Hwnd)==0 || owner==0 ||
           owner!=GetCurrentThreadId() || pid!=GetCurrentProcessId(){
            return Err("game HWND not owned by current UI thread/process".into());
        }
    }
    MENU_SYSTEM_CURSOR_PROBE_HWND.store(hwnd,Ordering::Release);
    Ok(())
}
pub fn disarm_native_system_cursor_probe(){
    let hwnd=MENU_SYSTEM_CURSOR_PROBE_HWND.swap(0,Ordering::AcqRel);
    if hwnd!=0{
        // Mode changes and script dispatch originate on game HWND/UI
        // owner in the validated native log. Refuse wrong-thread undo.
        let owner=unsafe{GetWindowThreadProcessId(hwnd as Hwnd,std::ptr::null_mut())};
        if owner==unsafe{GetCurrentThreadId()}{
            undo_native_system_cursor_probe();
        }
    }
}
fn intercept_erase(message:u32,matching:bool)->bool{
    message==WM_ERASEBKGND && matching
}

unsafe extern "system" fn guarded_proc(hwnd:Hwnd,msg:u32,wparam:usize,lparam:isize)->isize{
    let matching=HOOK_HWND.load(Ordering::Acquire)==hwnd as usize;
    let previous=PREVIOUS.load(Ordering::Acquire);

    if matching && intercept_erase(msg,true){
        ERASE_COUNT.fetch_add(1,Ordering::Relaxed);
        return 1; // WM_ERASEBKGND handled: leave DirectDraw pixels intact.
    }
    if matching && msg==WM_PAINT{
        PAINT_COUNT.fetch_add(1,Ordering::Relaxed);
    }
    if matching{
        let (down,up,character,focus)=input_event(msg);
        if down{KEYDOWN_COUNT.fetch_add(1,Ordering::Relaxed);}
        if up{KEYUP_COUNT.fetch_add(1,Ordering::Relaxed);}
        if character{CHAR_COUNT.fetch_add(1,Ordering::Relaxed);}
        if focus{FOCUS_COUNT.fetch_add(1,Ordering::Relaxed);}
        if msg==WM_MOUSEMOVE{
            let (x,y)=client_mouse_point(lparam);
            MOUSE_LAST_X.store(x as isize,Ordering::Relaxed);
            MOUSE_LAST_Y.store(y as isize,Ordering::Relaxed);
            MOUSE_MOVE_COUNT.fetch_add(1,Ordering::Relaxed);
        }
        if mouse_button_message(msg){
            MOUSE_BUTTON_COUNT.fetch_add(1,Ordering::Relaxed);
        }
    }
    if matching && msg==WM_NCDESTROY{
        // Keep the original chain in place for the final destroy callback.
        // Restore only when our hook is still the active window procedure.
        if previous!=0 && GetWindowLongW(hwnd,GWL_WNDPROC)==guarded_proc as *const () as usize as i32{
            let _=SetWindowLongW(hwnd,GWL_WNDPROC,previous as i32);
        }
        HOOK_HWND.store(0,Ordering::Release);
        PREVIOUS.store(0,Ordering::Release);
        disarm_native_menu_mouse_remap();
        disarm_native_gos_cursor_bridge();
        if MENU_SYSTEM_CURSOR_PROBE_HWND.load(Ordering::Acquire)==hwnd as usize{
            undo_native_system_cursor_probe(); // already on HWND owner thread
        }
        disarm_native_system_cursor_probe();
    }
    if previous==0{
        // Narrow installation race: do not recurse or dispatch via a NULL
        // callback. The original is captured immediately after SetWindowLongW.
        return DefWindowProcW(hwnd,msg,wparam,lparam);
    }
    // The original game receives the same messages and button flags;
    // only mouse client coordinates are scaled to the source 800x600
    // canvas when the exact native MainMenu/Preferences A/B is enabled.
    let forwarded=if matching{
        menu_mouse_lparam(hwnd,msg,lparam)
    }else{lparam};
    let result=CallWindowProcW(previous,hwnd,msg,wparam,forwarded);
    if matching && after_game_wndproc_system_cursor_probe(hwnd,msg) &&
        msg==WM_SETCURSOR {
        // Win32 cursor contract: after SetCursor, return TRUE to prevent
        // additional default cursor processing from overriding our probe.
        return 1;
    }
    result
}

/// Install once on the exact user-supplied, process-owned gameplay HWND.
/// Must only be invoked after borderless window setup and successful DD4
/// geometry validation. No process-wide hook and no class brush changes.
pub fn install(hwnd:usize)->Result<(),String>{
    if hwnd==0{return Err("NULL HWND".into());}
    let window=hwnd as Hwnd;
    unsafe{
        if IsWindow(window)==0{return Err("invalid game HWND".into());}
        let mut pid=0;
        if GetWindowThreadProcessId(window,&mut pid)==0 || pid!=GetCurrentProcessId(){
            return Err("HWND not owned by this process".into());
        }
        let old=HOOK_HWND.load(Ordering::Acquire);
        if old==hwnd{return if PREVIOUS.load(Ordering::Acquire)!=0{
            Ok(())
        }else{Err("window subclass still initializing".into())};}
        if old!=0{return Err("another HWND already guarded".into());}
        if HOOK_HWND.compare_exchange(0,hwnd,Ordering::AcqRel,Ordering::Acquire).is_err(){
            return Err("concurrent hook installation".into());
        }
        SetLastError(0);
        let previous=SetWindowLongW(window,GWL_WNDPROC,guarded_proc as *const () as usize as i32);
        let error=GetLastError();
        if previous==0 && error!=0{
            HOOK_HWND.store(0,Ordering::Release);
            return Err(format!("SetWindowLongW WNDPROC failed Win32={error}"));
        }
        if previous==0{
            HOOK_HWND.store(0,Ordering::Release);
            return Err("NULL previous game WndProc".into());
        }
        PREVIOUS.store(previous as isize,Ordering::Release);
        Ok(())
    }
}

pub fn snapshot(hwnd:usize)->Option<WindowPaintStats>{
    (hwnd!=0 && HOOK_HWND.load(Ordering::Acquire)==hwnd &&
        PREVIOUS.load(Ordering::Acquire)!=0).then_some(WindowPaintStats{
            erased:ERASE_COUNT.load(Ordering::Acquire),
            painted:PAINT_COUNT.load(Ordering::Acquire),
            keydown:KEYDOWN_COUNT.load(Ordering::Acquire),
            keyup:KEYUP_COUNT.load(Ordering::Acquire),
            character:CHAR_COUNT.load(Ordering::Acquire),
            focus_changes:FOCUS_COUNT.load(Ordering::Acquire),
            mouse_moves:MOUSE_MOVE_COUNT.load(Ordering::Acquire),
            mouse_buttons:MOUSE_BUTTON_COUNT.load(Ordering::Acquire),
            menu_mouse_mapped:MENU_MOUSE_MAPPED.load(Ordering::Acquire),
            system_cursor_probe_events:MENU_SYSTEM_CURSOR_PROBE_EVENTS.load(Ordering::Acquire),
            system_cursor_probe_restores:MENU_SYSTEM_CURSOR_PROBE_RESTORES.load(Ordering::Acquire),
        })
}

/// Read-only sample on the calling virtual Flip thread. This does not
/// call PeekMessage/DispatchMessage or modify DirectShow synchronization.
pub fn sample_input_state(hwnd:usize)->Option<WindowInputState>{
    let counts=snapshot(hwnd)?;
    let hwnd=hwnd as Hwnd;
    unsafe{
        if IsWindow(hwnd)==0{return None;}
        let window_thread=GetWindowThreadProcessId(hwnd,std::ptr::null_mut());
        if window_thread==0{return None;}
        let mut cursor=CursorInfo{
            size:std::mem::size_of::<CursorInfo>() as u32,flags:0,
            hcursor:std::ptr::null_mut(),screen:Point::default()
        };
        let cursor_visible=if GetCursorInfo(&mut cursor)!=0 {
            Some(cursor.flags&CURSOR_SHOWING!=0)
        }else{None};
        let mut pos=Point::default();
        let cursor_screen=if GetCursorPos(&mut pos)!=0{
            Some((pos.x,pos.y))
        }else{None};
        let cursor_client_last_move=(counts.mouse_moves>0).then_some((
            MOUSE_LAST_X.load(Ordering::Acquire) as i32,
            MOUSE_LAST_Y.load(Ordering::Acquire) as i32
        ));
        let menu_mouse_logical_last_move=
            (counts.menu_mouse_mapped>0).then_some((
                MENU_MOUSE_LAST_X.load(Ordering::Acquire) as i32,
                MENU_MOUSE_LAST_Y.load(Ordering::Acquire) as i32
            ));
        Some(WindowInputState{
            counts,
            game_is_foreground:GetForegroundWindow()==hwnd,
            queue_has_new_input:GetInputState()!=0,
            window_thread,
            current_thread:GetCurrentThreadId(),
            cursor_visible,cursor_screen,cursor_client_last_move,
            menu_mouse_logical_last_move,
        })
    }
}

/// Validate and arm the native menu-mouse A/B ONLY after the verified
/// 800x600 RGB32 graph and HWND borderless geometry were accepted.
/// The scene gate remains closed until the actual MainMenu/Preferences
/// script loads. This has no effect on DirectInput or the OS cursor.
pub fn arm_native_menu_mouse_remap(
    hwnd:usize,client:(u32,u32),render:(u32,u32)
)->Result<MenuMouseGeometry,String>{
    if hwnd==0{return Err("NULL menu HWND".into());}
    if snapshot(hwnd).is_none(){
        return Err("verified game WndProc guard is not installed".into());
    }
    let w=i32::try_from(client.0).map_err(|_|"invalid menu width")?;
    let h=i32::try_from(client.1).map_err(|_|"invalid menu height")?;
    let render_w=i32::try_from(render.0).map_err(|_|"invalid render width")?;
    let render_h=i32::try_from(render.1).map_err(|_|"invalid render height")?;
    let view=menu_content_rect(w,h,render_w,render_h)
        .ok_or("unsupported physical-to-client 4:3 menu viewport")?;
    unsafe{
        if IsWindow(hwnd as Hwnd)==0{return Err("invalid menu HWND".into());}
        let mut pid=0;
        if GetWindowThreadProcessId(hwnd as Hwnd,&mut pid)!=GetCurrentThreadId() ||
            pid!=GetCurrentProcessId(){
            return Err("menu HWND is not owned by current process/thread".into());
        }
        let mut actual=MouseRect::default();
        if GetClientRect(hwnd as Hwnd,&mut actual)==0 ||
            actual.right-actual.left!=w || actual.bottom-actual.top!=h{
            return Err("menu HWND client geometry mismatch".into());
        }
    }
    MENU_MOUSE_SCENE_ACTIVE.store(0,Ordering::Release);
    MENU_MOUSE_CLIENT_W.store(client.0 as usize,Ordering::Release);
    MENU_MOUSE_CLIENT_H.store(client.1 as usize,Ordering::Release);
    MENU_MOUSE_RENDER_W.store(render.0 as usize,Ordering::Release);
    MENU_MOUSE_RENDER_H.store(render.1 as usize,Ordering::Release);
    MENU_MOUSE_HWND.store(hwnd,Ordering::Release);
    Ok(MenuMouseGeometry{left:view.left,top:view.top,width:view.width,height:view.height})
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct MenuMouseGeometry{pub left:i32,pub top:i32,pub width:i32,pub height:i32}
pub fn disarm_native_menu_mouse_remap(){
    MENU_MOUSE_HWND.store(0,Ordering::Release);
    MENU_MOUSE_SCENE_ACTIVE.store(0,Ordering::Release);
}
/// The original Win32 window remains full-size borderless. This opt-in
/// bridge is allowed only for the native Windows DD4 virtual 800x600 movie
/// epoch, AFTER its actual HWND and physical 4:3 viewport are verified.
/// WndProc WM_* messages stay exactly as originally dispatched.
pub fn arm_native_gos_cursor_bridge(
    hwnd:usize,client:(u32,u32),render:(u32,u32)
)->Result<MenuMouseGeometry,String>{
    if hwnd==0 || snapshot(hwnd).is_none(){
        return Err("verified game HWND/WndProc guard missing".into());
    }
    if crate::cfg().native_menu_mouse_remap{
        return Err("NativeMenuMouseRemap=1; disable before native GOS cursor bridge".into());
    }
    let w=i32::try_from(client.0).map_err(|_|"bad client width")?;
    let h=i32::try_from(client.1).map_err(|_|"bad client height")?;
    let rw=i32::try_from(render.0).map_err(|_|"bad render width")?;
    let rh=i32::try_from(render.1).map_err(|_|"bad render height")?;
    let viewport=menu_content_rect(w,h,rw,rh)
        .ok_or("unsupported 4:3 virtual menu viewport")?;
    unsafe {
        let hwnd=hwnd as Hwnd;
        let mut pid=0u32;
        let owner=GetWindowThreadProcessId(hwnd,&mut pid);
        if IsWindow(hwnd)==0 || owner!=GetCurrentThreadId() ||
           pid!=GetCurrentProcessId(){
            return Err("game window not on current UI thread".into());
        }
        let mut actual=MouseRect::default();
        if GetClientRect(hwnd,&mut actual)==0 ||
           actual.right-actual.left!=w ||
           actual.bottom-actual.top!=h {
            return Err("borderless client dimensions changed".into());
        }
    }
    MENU_GOS_BRIDGE_CLIENT_W.store(client.0 as usize,Ordering::Release);
    MENU_GOS_BRIDGE_CLIENT_H.store(client.1 as usize,Ordering::Release);
    MENU_GOS_BRIDGE_RENDER_W.store(render.0 as usize,Ordering::Release);
    MENU_GOS_BRIDGE_RENDER_H.store(render.1 as usize,Ordering::Release);
    MENU_GOS_BRIDGE_HWND.store(hwnd,Ordering::Release);
    Ok(MenuMouseGeometry{
        left:viewport.left,top:viewport.top,
        width:viewport.width,height:viewport.height
    })
}
pub fn disarm_native_gos_cursor_bridge(){
    MENU_GOS_BRIDGE_HWND.store(0,Ordering::Release);
}
/// Snapshot the *actual* Windows pointer and transform through the SAME
/// viewport as presentation. The game cursor still belongs to GOS; this
/// function itself does not modify the game or the OS. Gated by exact
/// MainMenu/Preferences script plus owner HWND, focus, UI thread and
/// a verified physical movie viewport. Non-menu / gutter input is ignored.
pub fn native_gos_cursor_bridge_target()->Option<(i32,i32)>{
    let hwnd=MENU_GOS_BRIDGE_HWND.load(Ordering::Acquire);
    if hwnd==0 || MENU_MOUSE_SCENE_ACTIVE.load(Ordering::Acquire)==0 ||
       PREVIOUS.load(Ordering::Acquire)==0 ||
       HOOK_HWND.load(Ordering::Acquire)!=hwnd{return None;}
    let w=MENU_GOS_BRIDGE_CLIENT_W.load(Ordering::Acquire) as i32;
    let h=MENU_GOS_BRIDGE_CLIENT_H.load(Ordering::Acquire) as i32;
    let rw=MENU_GOS_BRIDGE_RENDER_W.load(Ordering::Acquire) as i32;
    let rh=MENU_GOS_BRIDGE_RENDER_H.load(Ordering::Acquire) as i32;
    unsafe {
        let game=hwnd as Hwnd;
        if GetForegroundWindow()!=game ||
           GetWindowThreadProcessId(game,std::ptr::null_mut())!=GetCurrentThreadId(){
            return None;
        }
        let mut pointer=Point::default();
        if GetCursorPos(&mut pointer)==0 || ScreenToClient(game,&mut pointer)==0{
            return None;
        }
        let viewport=menu_content_rect(w,h,rw,rh)?;
        if pointer.x<viewport.left || pointer.y<viewport.top ||
           pointer.x>=viewport.left+viewport.width ||
           pointer.y>=viewport.top+viewport.height{return None;}
        let target=menu_mouse_point(
            (pointer.x,pointer.y),w,h,rw,rh
        )?;
        if !(0..800).contains(&target.0) || !(0..600).contains(&target.1){
            return None;
        }
        Some(target)
    }
}
pub fn set_native_menu_mouse_script_active(active:bool){
    MENU_MOUSE_SCENE_ACTIVE.store(usize::from(active),Ordering::Release);
    if !active && MENU_SYSTEM_CURSOR_PROBE_HWND.load(Ordering::Acquire)!=0{
        // The exact game script loader has left MainMenu/Preferences.
        // Reset only our own ShowCursor increments, no global cursor
        // restore, no changes to the game's original callback.
        let hwnd=MENU_SYSTEM_CURSOR_PROBE_HWND.load(Ordering::Acquire) as Hwnd;
        if unsafe{GetWindowThreadProcessId(hwnd,std::ptr::null_mut())}
            ==unsafe{GetCurrentThreadId()}{
            undo_native_system_cursor_probe();
        }
    }
}

/// Arm for the exact process-owned game HWND; only the physically verified
/// native RGB32 movie epoch may call this (guarded by the caller's config).
/// This does not post, fetch, or dispatch messages during setup.
pub fn arm_native_movie_pump(hwnd:usize)->Result<(),String>{
    if hwnd==0{return Err("NULL game HWND".into());}
    unsafe{
        let window=hwnd as Hwnd;
        if IsWindow(window)==0{return Err("invalid game HWND".into());}
        let mut pid=0;
        let tid=GetWindowThreadProcessId(window,&mut pid);
        if tid==0 || pid!=GetCurrentProcessId() || tid!=GetCurrentThreadId(){
            return Err("game HWND not owned by current process/thread".into());
        }
        let old=NATIVE_MOVIE_PUMP_HWND.load(Ordering::Acquire);
        if old!=0 && old!=hwnd{return Err("another game HWND already armed".into());}
        NATIVE_MOVIE_PUMP_HWND.store(hwnd,Ordering::Release);
        Ok(())
    }
}

/// Dispatch at most four messages per native RGB32 movie Flip, for exactly
/// the same game HWND on its owning thread. Never dispatch inside WndProc,
/// never enter with the virtual Flip mutex held (caller responsibility).
/// A per-thread guard suppresses nested pumps during WndProc reentrancy.
/// Filtering by HWND avoids removing the thread's WM_QUIT and unrelated
/// window messages. This is EXPERIMENTAL and opt-in; it can affect the
/// legacy game's own keyboard/input processing.
pub fn pump_native_movie_window(hwnd:usize)->Option<(usize,usize,usize)>{
    if hwnd==0 || NATIVE_MOVIE_PUMP_HWND.load(Ordering::Acquire)!=hwnd{
        return None;
    }
    unsafe{
        let window=hwnd as Hwnd;
        if IsWindow(window)==0 ||
            GetWindowThreadProcessId(window,std::ptr::null_mut())!=GetCurrentThreadId(){
            return None;
        }
    }
    let entered=IN_NATIVE_MOVIE_PUMP.with(|flag|{
        if flag.get(){false}else{flag.set(true);true}
    });
    if !entered{return None;}
    struct Reset;
    impl Drop for Reset{
        fn drop(&mut self){IN_NATIVE_MOVIE_PUMP.with(|f|f.set(false));}
    }
    let _reset=Reset;
    let mut dispatched=0usize;
    unsafe{
        for _ in 0..NATIVE_MOVIE_PUMP_MAX_PER_FLIP{
            if IsWindow(hwnd as Hwnd)==0{break;}
            let mut msg:WinMessage=std::mem::zeroed();
            // Inspect without removing first: preserve WM_QUIT and
            // unexpected foreign messages intact even if filtering is
            // changed by a future Windows/runtime implementation.
            if PeekMessageW(&mut msg,hwnd as Hwnd,0,0,0)==0{break;}
            let allowed=msg.hwnd==hwnd as Hwnd ||
                (!msg.hwnd.is_null() &&
                    IsWindow(msg.hwnd)!=0 &&
                    GetWindowThreadProcessId(msg.hwnd,std::ptr::null_mut())==
                        GetCurrentThreadId());
            if !allowed{break;}
            if PeekMessageW(&mut msg,hwnd as Hwnd,0,0,PM_REMOVE)==0{break;}
            // The window filter only returns game HWND and descendant
            // messages. Never silently discard an unrelated message.
            if msg.hwnd.is_null(){break;}
            let _=TranslateMessage(&msg);
            let _=DispatchMessageW(&msg);
            dispatched+=1;
        }
    }
    let calls=NATIVE_MOVIE_PUMP_CALLS.fetch_add(1,Ordering::Relaxed)+1;
    let total=NATIVE_MOVIE_PUMP_MESSAGES.fetch_add(dispatched,Ordering::Relaxed)+dispatched;
    Some((dispatched,calls,total))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn only_the_guarded_window_erase_message_is_suppressed(){
        assert!(intercept_erase(WM_ERASEBKGND,true));
        assert!(!intercept_erase(WM_ERASEBKGND,false));
        for m in [WM_PAINT,WM_NCDESTROY,0,0x0006,0x0007,0x0010]{
            assert!(!intercept_erase(m,true));
        }
    }
    #[test]
    fn passive_input_message_classification_forwards_every_message(){
        assert_eq!(input_event(WM_KEYDOWN),(true,false,false,false));
        assert_eq!(input_event(WM_SYSKEYDOWN),(true,false,false,false));
        assert_eq!(input_event(WM_KEYUP),(false,true,false,false));
        assert_eq!(input_event(WM_SYSKEYUP),(false,true,false,false));
        assert_eq!(input_event(WM_CHAR),(false,false,true,false));
        assert_eq!(input_event(WM_KILLFOCUS),(false,false,false,true));
        assert_eq!(input_event(WM_ACTIVATEAPP),(false,false,false,true));
        assert_eq!(input_event(WM_PAINT),(false,false,false,false));
    }
    #[test]
    fn native_gos_bridge_target_follows_same_letterboxed_viewport(){
        let viewport=menu_content_rect(1536,960,1920,1200).unwrap();
        assert_eq!((viewport.left,viewport.top,viewport.width,viewport.height),
                   (128,0,1280,960));
        assert_eq!(menu_mouse_point((128,0),1536,960,1920,1200),Some((0,0)));
        assert_eq!(menu_mouse_point((768,480),1536,960,1920,1200),Some((400,300)));
        assert_eq!(menu_mouse_point((1407,959),1536,960,1920,1200),Some((799,599)));
    }
    #[test]
    fn mouse_message_and_coordinate_readout_is_passive(){
        assert!(!mouse_button_message(WM_MOUSEMOVE));
        assert!(mouse_button_message(WM_LBUTTONDOWN));
        assert!(mouse_button_message(WM_RBUTTONUP));
        assert!(!mouse_button_message(WM_KEYDOWN));
        assert_eq!(client_mouse_point(0x0258_0320isize),(800,600));
        assert_eq!(client_mouse_point(0xffff_ffffu32 as isize),(-1,-1));
        assert_eq!(std::mem::size_of::<Point>(),8);
        assert_eq!(std::mem::size_of::<CursorInfo>(),20); // x86 only
        assert_eq!(std::mem::size_of::<WinMessage>(),32); // x86 only
        assert_eq!(std::mem::size_of::<MouseRect>(),16);
        assert_eq!(NATIVE_MOVIE_PUMP_MAX_PER_FLIP,4);
    }
    #[test]
    fn native_system_cursor_probe_message_scope_is_menu_mouse_only(){
        assert!(native_system_cursor_probe_message(WM_SETCURSOR));
        assert!(native_system_cursor_probe_message(WM_MOUSEMOVE));
        assert!(!native_system_cursor_probe_message(WM_KEYDOWN));
        assert!(!native_system_cursor_probe_message(WM_LBUTTONDOWN));
        assert!(!native_system_cursor_probe_message(WM_NCDESTROY));
        assert_eq!(MENU_SYSTEM_CURSOR_PROBE_HWND.load(Ordering::Acquire),0);
    }
    #[test]
    fn native_menu_4_by_3_mouse_mapping_is_exact_and_reversible(){
        assert_eq!(menu_content_rect(1536,960,1920,1200),Some(MenuRect{
            left:128,top:0,width:1280,height:960
        }));
        assert_eq!(menu_mouse_point((128,0),1536,960,1920,1200),Some((0,0)));
        assert_eq!(menu_mouse_point((1407,959),1536,960,1920,1200),Some((799,599)));
        assert_eq!(menu_mouse_point((1408,960),1536,960,1920,1200),Some((800,600)));
        assert_eq!(menu_mouse_point((636,542),1536,960,1920,1200),Some((317,338)));
        assert_eq!(menu_mouse_point((64,480),1536,960,1920,1200),Some((-40,300)));
        assert_eq!(menu_mouse_point((640,360),1280,720,1280,720),Some((400,300)));
        assert_eq!(menu_mouse_point((400,300),800,600,800,600),Some((400,300)));
        assert_eq!(menu_content_rect(640,480,640,480),Some(MenuRect{
            left:0,top:0,width:640,height:480
        }));
        let odd=menu_content_rect(1093,614,1366,768).unwrap();
        let backend=crate::presentation::output_viewport(
            (800,600),(1366,768),(1093,614)).unwrap();
        assert_eq!((odd.left,odd.top,odd.width,odd.height),
            (backend.x as i32,backend.y as i32,backend.width as i32,backend.height as i32));
        assert_eq!(menu_mouse_point((0,0),0,0,0,0),None);
        assert!(!menu_mouse_message(WM_CHAR));
        assert!(!menu_mouse_message(WM_KEYDOWN));
        assert!(menu_mouse_message(WM_MOUSEMOVE));
        assert!(menu_mouse_message(WM_LBUTTONUP));
        assert_eq!(menu_mouse_lparam(std::ptr::null_mut(),WM_MOUSEMOVE,0x1234),0x1234);
    }
    #[test]
    fn native_movie_pump_requires_a_strictly_armed_window(){
        assert_eq!(pump_native_movie_window(0),None);
        // No OS call is made for non-armed identifiers.
        assert_eq!(pump_native_movie_window(0x98765),None);
    }
    #[test]
    fn guarded_window_stats_require_installed_window(){
        assert!(snapshot(0).is_none());
    }
}
