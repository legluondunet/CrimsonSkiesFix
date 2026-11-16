use std::{env,path::PathBuf};
fn main(){
    assert_eq!(env::var("TARGET").as_deref(),Ok("i686-pc-windows-msvc"),
        "WinMM loader supports only 32-bit native Windows ABI");
    let root=PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest"));
    let def=root.join("../runtime-winmm/src/winmm_full_exports.def");
    println!("cargo:rerun-if-changed={}",def.display());
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}",def.display());
}
