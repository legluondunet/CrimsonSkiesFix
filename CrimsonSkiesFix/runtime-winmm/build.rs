use std::{env,path::PathBuf,process::Command};
fn main(){
    let target=env::var("TARGET").expect("Cargo target");
    assert_eq!(target,"i686-pc-windows-msvc",
        "WinMM runtime is only built for i686-pc-windows-msvc");
    let root=PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo root"));
    let def=root.join("../winmm-prototype/src/winmm_exports.def");
    println!("cargo:rerun-if-changed={}",def.display());
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}",def.display());
    let sha=env::var("CRIMSONSKIESFIX_COMMIT").ok()
        .filter(|s|s.len()>=7 && s.chars().all(|c|c.is_ascii_hexdigit()))
        .or_else(||{
            let out=Command::new("git").args(["rev-parse","HEAD"]).output().ok()?;
            if !out.status.success(){return None;}
            let x=String::from_utf8(out.stdout).ok()?;
            let x=x.trim().to_string();
            (x.len()>=7&&x.chars().all(|c|c.is_ascii_hexdigit())).then_some(x)
        }).unwrap_or_else(||"unknown".to_string());
    println!("cargo:rustc-env=CRIMSONSKIESFIX_BUILD_COMMIT={}",&sha[..sha.len().min(8)]);
}
