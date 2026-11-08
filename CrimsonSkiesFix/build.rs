use std::{env,path::PathBuf,process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=CRIMSONSKIESFIX_COMMIT");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=src/ddraw_exports.def");
    // The working native Windows HAL test needed all 22 DirectDraw exports
    // with canonical ordinals. Only the x86 MSVC DLL uses this .def file.
    // The other 19 entries link to ABI-preserving assembly tailcalls.
    if env::var("TARGET").ok().as_deref()==Some("i686-pc-windows-msvc") {
        let manifest=PathBuf::from(env::var("CARGO_MANIFEST_DIR")
            .expect("Cargo always supplies CARGO_MANIFEST_DIR"));
        let def=manifest.join("src").join("ddraw_exports.def");
        assert!(def.is_file(),"missing canonical native DDRAW export definition");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}",def.display());
    }
    let sha=env::var("CRIMSONSKIESFIX_COMMIT").ok()
        .filter(|s|s.len()>=7&&s.chars().all(|c|c.is_ascii_hexdigit()))
        .or_else(||{
            let output=Command::new("git").args(["rev-parse","HEAD"]).output().ok()?;
            if !output.status.success(){return None;}
            let s=String::from_utf8(output.stdout).ok()?;
            let s=s.trim().to_string();
            (s.len()>=7&&s.chars().all(|c|c.is_ascii_hexdigit())).then_some(s)
        })
        .unwrap_or_else(||"unknown".to_string());
    println!("cargo:rustc-env=CRIMSONSKIESFIX_BUILD_COMMIT={}",&sha[..sha.len().min(8)]);
}
