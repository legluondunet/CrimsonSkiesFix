//! Read-only registry audit for the original Crimson Skies installation.
//! Distinguishes genuinely missing values, wrong value type, invalid data,
//! and a different 32-/64-bit registry view. Neither registry paths nor
//! player-controlled values are copied into the diagnostic log.
//!
//! This is evidence about installation registration, NOT proof that GameOS
//! selects ROF resources from these values. Never write to the registry.
use std::{path::Path,ptr};

const HKEY_LOCAL_MACHINE:isize=0x80000002u32 as isize;
const HKEY_CURRENT_USER:isize=0x80000001u32 as isize;
const KEY_READ:u32=0x00020019;
const KEY_WOW64_32KEY:u32=0x0200;
const KEY_WOW64_64KEY:u32=0x0100;
const REG_SZ:u32=1;
const REG_EXPAND_SZ:u32=2;
const ERROR_SUCCESS:u32=0;
const ERROR_FILE_NOT_FOUND:u32=2;
const ERROR_MORE_DATA:u32=234;
const RETAIL_KEY:&str=r"SOFTWARE\Microsoft\Microsoft Games\Crimson Skies\1.0";
const WOW6432_KEY:&str=r"SOFTWARE\Wow6432Node\Microsoft\Microsoft Games\Crimson Skies\1.0";

#[link(name="advapi32")]
extern "system"{
    fn RegOpenKeyExW(key:isize,subkey:*const u16,options:u32,
                     access:u32,result:*mut isize)->u32;
    fn RegQueryValueExW(key:isize,name:*const u16,reserved:*mut u32,
                        kind:*mut u32,data:*mut u8,size:*mut u32)->u32;
    fn RegCloseKey(key:isize)->u32;
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

#[derive(Debug,PartialEq,Eq)]
enum Value {
    Text(String),
    Missing,
    WrongType,
    InvalidEncoding,
    TooLarge,
    QueryFailed,
}
impl Value{
    fn label(&self)->&'static str{
        match self{
            Self::Text(_)=>"present",
            Self::Missing=>"missing",
            Self::WrongType=>"wrong-type",
            Self::InvalidEncoding=>"invalid-encoding",
            Self::TooLarge=>"oversized",
            Self::QueryFailed=>"query-failed",
        }
    }
}
// REG_SZ in old retail installers may be encoded without the final NUL.
// Accept missing or multiple trailing NULs, never an embedded NUL.
// This decoding has no effect on game behavior.
fn decode_string(data:&[u16])->Result<String,()>{
    let end=data.iter().rposition(|&c|c!=0).map_or(0,|i|i+1);
    if data[..end].contains(&0){return Err(());}
    String::from_utf16(&data[..end]).map_err(|_|())
}
fn read_value(key:isize,name:&str)->Value{
    let mut data=[0u16;2048];
    let mut kind=0u32;
    let mut size=(data.len()*2) as u32;
    let status=unsafe{RegQueryValueExW(
        key,wide(name).as_ptr(),ptr::null_mut(),&mut kind,
        data.as_mut_ptr().cast(),&mut size)};
    match status{
        ERROR_FILE_NOT_FOUND=>return Value::Missing,
        ERROR_MORE_DATA=>return Value::TooLarge,
        ERROR_SUCCESS=>{},
        _=>return Value::QueryFailed,
    }
    if kind!=REG_SZ && kind!=REG_EXPAND_SZ{return Value::WrongType;}
    if size%2!=0 || size as usize>data.len()*2{
        return Value::InvalidEncoding;
    }
    match decode_string(&data[..size as usize/2]){
        Ok(value)=>Value::Text(value),
        Err(())=>Value::InvalidEncoding,
    }
}
fn canonical_path_key(raw:&str)->String{
    raw.trim().replace('/',"\\")
        .trim_end_matches('\\').to_lowercase()
}
fn install_path_matches(game:&Path,value:&str)->bool{
    canonical_path_key(&game.to_string_lossy())==canonical_path_key(value)
}
fn classify_exe(value:&Value,game:Option<&Path>)->&'static str{
    match (value,game){
        (Value::Text(path),Some(game)) if install_path_matches(game,path)=>
            "matches-current-game",
        (Value::Text(_),Some(_))=>"other-or-unresolved-path",
        (Value::Text(_),None)=>"no-current-game-path",
        _=>value.label(),
    }
}
fn classify_assets(value:&Value)->&'static str{
    match value{
        Value::Text(x) if canonical_path_key(x)==canonical_path_key("GOSData")=>
            "original-relative-GOSData",
        Value::Text(_)=>"other-path",
        _=>value.label(),
    }
}
fn classify_version(value:&Value)->&'static str{
    match value{
        Value::Text(x) if x=="1.02"=>"original-1.02",
        Value::Text(_)=>"other-version",
        _=>value.label(),
    }
}
fn report_view(label:&str,root:isize,path:&str,flags:u32,game:Option<&Path>){
    let mut key=0isize;
    let status=unsafe{RegOpenKeyExW(root,wide(path).as_ptr(),0,KEY_READ|flags,&mut key)};
    if status!=ERROR_SUCCESS || key==0 {
        let state=if status==ERROR_FILE_NOT_FOUND{"missing"}else{"inaccessible-or-unsupported"};
        crate::log(&format!("GOS-REGISTRY-DIAG: view={label} key={state} [read-only; no registry writes]"));
        return;
    }
    let exe=read_value(key,"EXE Path");
    let assets=read_value(key,"UIAssetPath");
    let version=read_value(key,"Version");
    unsafe{RegCloseKey(key);}
    crate::log(&format!(
        "GOS-REGISTRY-DIAG: view={label} key=present exe_path={} ui_asset_path={} version={} [read-only; no values or paths logged; archive priority unproven]",
        classify_exe(&exe,game),classify_assets(&assets),classify_version(&version)
    ));
}
pub fn report(){
    if !crate::cfg().log_enabled{return;}
    let game=std::env::current_exe().ok()
        .and_then(|path|path.parent().map(Path::to_path_buf));
    // Both the explicit 32-bit view and native view matter under Wine.
    // The original retail Windows registry export uses Wow6432Node;
    // we check it as a separate hypothesis, not as a new target for writes.
    report_view("hklm32",HKEY_LOCAL_MACHINE,RETAIL_KEY,KEY_WOW64_32KEY,game.as_deref());
    report_view("hklm-native",HKEY_LOCAL_MACHINE,RETAIL_KEY,0,game.as_deref());
    report_view("hklm64",HKEY_LOCAL_MACHINE,RETAIL_KEY,KEY_WOW64_64KEY,game.as_deref());
    report_view("hklm-explicit-wow6432",HKEY_LOCAL_MACHINE,WOW6432_KEY,0,game.as_deref());
    // HKCU is not assumed to substitute for the retail HKLM record;
    // the owner's original export contains a separate preferences key.
    report_view("hkcu",HKEY_CURRENT_USER,RETAIL_KEY,0,game.as_deref());
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
    #[test] fn legacy_reg_sz_trailing_nuls_or_missing_terminator(){
        assert_eq!(decode_string(&['G' as u16,'O' as u16,'S' as u16,0,0]),
                   Ok("GOS".to_string()));
        assert_eq!(decode_string(&['G' as u16,'O' as u16,'S' as u16]),
                   Ok("GOS".to_string()));
        assert!(decode_string(&['G' as u16,0,'S' as u16]).is_err());
        assert!(decode_string(&[0xD800]).is_err());
    }
    #[test] fn missing_and_wrong_type_cannot_masquerade_as_valid(){
        assert_eq!(classify_exe(&Value::Missing,Some(Path::new(r"C:\Games"))),"missing");
        assert_eq!(classify_assets(&Value::WrongType),"wrong-type");
        assert_eq!(classify_version(&Value::InvalidEncoding),"invalid-encoding");
        assert_eq!(classify_assets(&Value::Text("GOSData".into())),"original-relative-GOSData");
        assert_eq!(classify_version(&Value::Text("1.02".into())),"original-1.02");
    }
}
