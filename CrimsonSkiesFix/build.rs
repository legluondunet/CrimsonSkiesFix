use std::{env,process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=CRIMSONSKIESFIX_COMMIT");
    println!("cargo:rerun-if-changed=../.git/HEAD");
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
