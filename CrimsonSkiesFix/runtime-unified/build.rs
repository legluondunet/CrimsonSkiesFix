#[allow(dead_code)]
#[path="../installer/src/sha256.rs"]
mod sha256;
use std::{env,fs,path::PathBuf,process::Command};
fn controls_art(){
    const REQUIRED:&str="CRIMSONSKIESFIX_REQUIRE_CONTROLS_ART";
    println!("cargo:rerun-if-env-changed={REQUIRED}");
    println!("cargo:rustc-check-cfg=cfg(crimson_controls_embedded)");
    let path=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../assets/controls/controls-art.pack");
    println!("cargo:rerun-if-changed={}",path.display());
    match fs::read(&path){
        Ok(bytes)=>{
            assert_eq!(bytes.len(),2555973,"wrong controls artwork length");
            assert_eq!(sha256::hex(&bytes),"79e6ac7529f3234dd364d752ff5405915d6daf537f5114cc4c1751813a0e6044","wrong controls artwork SHA-256");
            println!("cargo:rustc-cfg=crimson_controls_embedded");
        },
        Err(e) if e.kind()==std::io::ErrorKind::NotFound&&env::var(REQUIRED).as_deref()!=Ok("1")=>{
            println!("cargo:warning=Controls artwork not checked in; explicit external-overlay test build only");
        },
        Err(e)=>panic!("Cannot read required controls artwork {}: {e}",path.display()),
    }
}
fn main(){
    controls_art();
    println!("cargo:rerun-if-env-changed=CRIMSONSKIESFIX_SDL3_SHA256");
    assert_eq!(env::var("TARGET").as_deref(),Ok("i686-pc-windows-msvc"),
        "Organized runtime supports only 32-bit Windows ABI");
    let sha=env::var("CRIMSONSKIESFIX_COMMIT").ok()
        .filter(|s|s.len()>=7&&s.chars().all(|c|c.is_ascii_hexdigit()))
        .or_else(||{
            let out=Command::new("git").args(["rev-parse","HEAD"]).output().ok()?;
            if !out.status.success(){return None;}
            let x=String::from_utf8(out.stdout).ok()?.trim().to_owned();
            (x.len()>=7&&x.chars().all(|c|c.is_ascii_hexdigit())).then_some(x)
        }).unwrap_or_else(||"unknown".to_owned());
    println!("cargo:rustc-env=CRIMSONSKIESFIX_BUILD_COMMIT={}",&sha[..sha.len().min(8)]);
}
