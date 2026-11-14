use std::{env, path::PathBuf};
fn main() {
    let target = env::var("TARGET").expect("Rust target");
    if target != "i686-pc-windows-msvc" {
        panic!("WinMM prototype supports only i686-pc-windows-msvc");
    }
    let def = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("src").join("winmm_exports.def");
    println!("cargo:rerun-if-changed=src/winmm_exports.def");
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def.display());
}
