//! Opt-in, read-only audit of the retail game's 32-bit HKLM installation
//! registration. The original installer records EXE Path, UIAssetPath
//! ("GOSData"), and Version ("1.02"). A standalone copy may not have the
//! same registry state as a conventional installation.
//!
//! This is a hypothesis discriminator, NOT evidence of GameOS's exact
//! ROF precedence. Never write registry keys or expose the user's paths.
use std::{path::Path,ptr};

const HKEY_LOCAL_MACHINE:isize=0x80000002u32 as isize;
const KEY_READ:u32=0x00020019;
const KEY_WOW64_32KEY:u32=0x0200;
const REG_SZ:u32=1;
const ERROR_SUCCESS:u32=0;
const KEY:&str=r"SOFTWARE\Microsoft\Microsoft Games\Crimson Skies\1.0";

#[link(name="advapi32")]
extern "system"{
    fn RegOpenKeyExW(key:isize,subkey:*const u16,options:u32,
                     access:u32,result:*mut isize)->u32;
    fn RegQueryValueExW(key:isize,name:*const u16,reserved:*mut u32,
                        kind:*mut u32,data:*mut u8,size:*mut u32)->u32;
    fn RegCloseKey(key:isize)->u32;
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

fn read_string(key:isize,name:&str)->Option<String>{
    let mut data=[0u16;2048];
    let mut kind=0u32;
    let mut size=(data.len()*2) as u32;
    let status=unsafe{RegQueryValueExW(
        key,wide(name).as_ptr(),ptr::null_mut(),&mut kind,
        data.as_mut_ptr().cast(),&mut size)};
    if status!=ERROR_SUCCESS || kind!=REG_SZ || size%2!=0 ||
       size<2 || size as usize>data.len()*2{
        return None;
    }
    let count=size as usize/2;
    let end=data[..count].iter().position(|&c|c==0)?;
    if end+1!=count{return None;}
    String::from_utf16(&data[..end]).ok()
}

fn canonical_path_key(raw:&str)->String{
    raw.trim().replace('/',"\\")
        .trim_end_matches('\\').to_lowercase()
}
fn install_path_matches(game:&Path,registry_value:&str)->bool{
    // Compare the ordinary spelling first. A full canonical OS path is
    // deliberately not logged, and any ambiguity is reported as mismatch.
    canonical_path_key(&game.to_string_lossy())==
    canonical_path_key(registry_value)
}
fn open_key()->Option<isize>{
    // 32-bit view is explicitly selected for a 32-bit game on Windows x64.
    // A native 32-bit Wine registry may implement it as the default view.
    for access in [KEY_READ|KEY_WOW64_32KEY,KEY_READ]{
        let mut key=0isize;
        if unsafe{RegOpenKeyExW(HKEY_LOCAL_MACHINE,wide(KEY).as_ptr(),0,access,&mut key)}
            ==ERROR_SUCCESS && key!=0 {
            return Some(key);
        }
    }
    None
}
pub fn report(){
    if !crate::cfg().log_enabled{return;}
    let Some(key)=open_key() else{
        crate::log("GOS-REGISTRY-DIAG: retail_hklm32=missing-or-inaccessible (no writes; patch-archive selection unproven)");
        return;
    };
    let exe=read_string(key,"EXE Path");
    let assets=read_string(key,"UIAssetPath");
    let version=read_string(key,"Version");
    unsafe{RegCloseKey(key);}
    let game=std::env::current_exe().ok()
        .and_then(|path|path.parent().map(Path::to_path_buf));
    let exe_match=match (game.as_ref(),exe.as_deref()){
        (Some(game),Some(exe))=>if install_path_matches(game,exe){
            "matches-current-game"
        }else{"other-or-unresolved-path"},
        _=>"missing-or-invalid"
    };
    let ui_assets=match assets.as_deref(){
        Some(name) if canonical_path_key(name)==canonical_path_key("GOSData")=>
            "original-relative-GOSData",
        Some(_)=>"other-location",
        None=>"missing-or-invalid",
    };
    let version=match version.as_deref(){
        Some("1.02")=>"original-1.02",
        Some(_)=>"other",
        None=>"missing-or-invalid",
    };
    crate::log(&format!(
        "GOS-REGISTRY-DIAG: retail_hklm32=present exe_path={} ui_asset_path={} version={} [read-only; never proof of archive loading]",
        exe_match,ui_assets,version
    ));
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn path_spelling_is_case_and_separator_independent(){
        assert!(install_path_matches(Path::new(r"C:\Games\CrimsonSkiesFix"),
                                    r"c:/games/crimsonskiesfix/"));
        assert!(!install_path_matches(Path::new(r"C:\Games\CrimsonSkiesFix"),
                                     r"C:\Other\Crimson Skies"));
    }
}
