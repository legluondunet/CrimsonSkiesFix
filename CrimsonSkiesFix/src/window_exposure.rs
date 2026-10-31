//! Read-only Win32 visibility hints for the opt-in focus repaint experiment.
//!
//! This module can be shared by the Windows DLL and the standalone probe.
//! It never subclasses the game WndProc, sends messages, invalidates a
//! window, or changes the desktop / DirectDraw cooperative level.
#![allow(dead_code)]
use std::ffi::c_void;

pub type Hwnd=*mut c_void;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct GameVisibility {
    pub foreground:bool,
    pub pending_paint:bool,
}

#[link(name="kernel32")]
extern "system" {
    fn GetCurrentProcessId()->u32;
}

#[link(name="user32")]
extern "system" {
    fn IsWindow(hwnd:Hwnd)->i32;
    fn IsWindowVisible(hwnd:Hwnd)->i32;
    fn IsIconic(hwnd:Hwnd)->i32;
    fn IsChild(parent:Hwnd,child:Hwnd)->i32;
    fn GetForegroundWindow()->Hwnd;
    fn GetUpdateRect(hwnd:Hwnd,rect:*mut c_void,erase:i32)->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,process_id:*mut u32)->u32;
}

/// Query only the exact HWND already verified by the game DD4 adapter.
///
/// Returns None if it is stale, foreign or not supplied. A NULL foreground
/// HWND is an unknown focus state, not proof that the game lost focus.
/// GetUpdateRect(erase=FALSE) does NOT consume the invalidation.
pub fn sample_game_window(hwnd:usize)->Option<GameVisibility>{
    if hwnd==0{return None;}
    let hwnd=hwnd as Hwnd;
    unsafe{
        if IsWindow(hwnd)==0{return None;}
        let mut pid=0;
        if GetWindowThreadProcessId(hwnd,&mut pid)==0 || pid!=GetCurrentProcessId(){
            return None;
        }
        let active=GetForegroundWindow();
        if active.is_null(){return None;}
        Some(GameVisibility{
            foreground:IsIconic(hwnd)==0 && IsWindowVisible(hwnd)!=0 &&
                (active==hwnd || IsChild(hwnd,active)!=0),
            pending_paint:GetUpdateRect(hwnd,std::ptr::null_mut(),0)!=0,
        })
    }
}
