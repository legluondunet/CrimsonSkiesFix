//! Build-time opt-in for a strictly private/offline owner build.
//! The official Microsoft update is NOT included in this repository or CI.
//! This does not grant permission to redistribute the resulting executable.
#[path = "src/sha256.rs"]
mod sha256;

use std::{env,fs,path::PathBuf};

fn main() {
    // Candidate CI provides the notices generated from the same locked metadata.
    // Local developer builds without this input are explicitly not release-ready.
    const NOTICES:&str="CRIMSONSKIESFIX_NOTICES_FILE";
    println!("cargo:rerun-if-env-changed={NOTICES}");
    if env::var_os("CARGO_FEATURE_NATIVE_USDE_UPDATER").is_some(){
        let text=match env::var_os(NOTICES){
            Some(path)=>{
                let path=PathBuf::from(path);
                println!("cargo:rerun-if-changed={}",path.display());
                let text=fs::read_to_string(path).expect("cannot read generated dependency notices");
                assert!(text.starts_with("CrimsonSkiesFix - third-party dependency notices"),"invalid notices");
                text
            },
            None=>"Developer build: dependency notices not generated; not for distribution.\n".to_owned()
        };
        let out=PathBuf::from(env::var_os("OUT_DIR").expect("cargo OUT_DIR missing"));
        fs::write(out.join("third-party-notices.txt"),text).expect("cannot stage notices");
    }
    const ENV:&str="CRIMSONSKIESFIX_OWNER_PATCH102";
    println!("cargo:rerun-if-env-changed={ENV}");
    // Public/release builds retain the external-updater behavior.
    if env::var_os("CARGO_FEATURE_EMBED_OWNER_PATCH102").is_none(){
        return;
    }
    let file=PathBuf::from(env::var_os(ENV).unwrap_or_else(||{
        panic!("embed-owner-patch102 requires an explicit locally owned crmsk102.exe in {ENV}; no copyrighted update is included in the repository")
    }));
    let bytes=fs::read(&file).unwrap_or_else(|e|panic!("cannot read private official patch {}: {e}",file.display()));
    const EXPECTED_LEN:usize=2_315_728;
    const EXPECTED_SHA:&str="8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af";
    const CAB_OFFSET:usize=0x346f0;
    const CAB_BYTES:usize=2_091_693;
    assert_eq!(bytes.len(),EXPECTED_LEN,"private update102 size mismatch; refusing to embed");
    assert_eq!(sha256::hex(&bytes),EXPECTED_SHA,"private update102 SHA-256 mismatch; refusing to embed");
    assert_eq!(&bytes[..2],b"MZ","private update102 is not a Windows executable");
    assert_eq!(&bytes[CAB_OFFSET..CAB_OFFSET+4],b"MSCF","private update102 CAB header mismatch");
    assert!(CAB_OFFSET+CAB_BYTES<=bytes.len());

    let out=PathBuf::from(env::var_os("OUT_DIR").expect("cargo OUT_DIR missing"))
        .join("verified-private-crmsk102.bin");
    fs::write(&out,&bytes).unwrap_or_else(|e|panic!("cannot stage private patch in OUT_DIR: {e}"));
    println!("cargo:rerun-if-changed={}",file.display());
    println!("cargo:rustc-env=CRIMSONSKIESFIX_EMBEDDED_PATCH102={}",out.display());
}
