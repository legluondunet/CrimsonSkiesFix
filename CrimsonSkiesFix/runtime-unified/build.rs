use std::{env,process::Command};
fn main(){
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
