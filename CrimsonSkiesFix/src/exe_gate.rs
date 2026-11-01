//! Fail-closed acceptance gate for the single owner-verified Crimson Skies
//! 1.02 executable. Runs at the first DirectDraw export,
//! never from DllMain or under the Windows loader lock.
//! Reads the EXE on disk, does not modify it, and does not inspect game content.
use std::{
    fs::{self, OpenOptions}, io::Write, ptr,
    sync::OnceLock,
};
use crate::{log,win32,LOG};

pub const EXPECTED_SHA256:&str="43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75";
pub const EXPECTED_SIZE:usize=2_580_480;
static ACCEPTED:OnceLock<bool>=OnceLock::new();

#[link(name="user32")]
extern "system" {
    fn MessageBoxW(hwnd:*mut std::ffi::c_void,text:*const u16,caption:*const u16,kind:u32)->i32;
}

fn sha256(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let bits = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 { padded.push(0); }
    padded.extend_from_slice(&bits.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes(chunk[4*i..4*i+4].try_into().unwrap()); }
        for i in 16..64 {
            let x = w[i-15]; let y = w[i-2];
            let s0 = x.rotate_right(7)^x.rotate_right(18)^(x>>3);
            let s1 = y.rotate_right(17)^y.rotate_right(19)^(y>>10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut z] = h;
        for i in 0..64 {
            let s1=e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
            let ch=(e&f)^((!e)&g);
            let t1=z.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0=a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
            let maj=(a&b)^(a&c)^(b&c);
            let t2=s0.wrapping_add(maj);
            z=g;g=f;f=e;e=d.wrapping_add(t1);d=c;c=b;b=a;a=t1.wrapping_add(t2);
        }
        for (x,y) in h.iter_mut().zip([a,b,c,d,e,f,g,z]) { *x=x.wrapping_add(y); }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

fn accepted_sha(size:usize,hash:&str)->bool{
    size==EXPECTED_SIZE && hash==EXPECTED_SHA256
}
fn observed_exe()->Result<(usize,String),String>{
    let path=std::env::current_exe().map_err(|e|format!("cannot resolve game EXE: {e}"))?;
    let metadata=fs::symlink_metadata(&path)
        .map_err(|e|format!("cannot stat game EXE: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink(){
        return Err("game EXE is not a regular file".into());
    }
    // Hash the actual executable path, not the modified in-memory image:
    // widescreen/FOV changes made later by this DLL must not affect admission.
    let data=fs::read(&path).map_err(|e|format!("cannot read game EXE: {e}"))?;
    Ok((data.len(),sha256(&data)))
}

fn write_rejection(line:&str)->bool{
    if LOG.get().is_some(){
        log(line);
        return true;
    }
    // Rejections are always logged, including when the user's [Debug] Log=0.
    // Never truncate an existing game log or follow a non-file/symlink path.
    let Some(path)=win32::log_path() else {return false};
    match OpenOptions::new().create(true).append(true).open(path){
        Ok(mut f)=>writeln!(f,"{line}").is_ok(),
        Err(_)=>false,
    }
}

fn rejection_message(actual:&str)->String {
    format!(
        "Only the executable with the following SHA-256 is supported:\n{EXPECTED_SHA256}\n\nYour executable:\n{actual}"
    )
}

fn dialog(text:&str){
    let caption:Vec<u16>="CrimsonSkiesFix - Unsupported executable".encode_utf16().chain([0]).collect();
    let message:Vec<u16>=text.encode_utf16().chain([0]).collect();
    unsafe{MessageBoxW(ptr::null_mut(),message.as_ptr(),caption.as_ptr(),0x10 /* MB_ICONERROR | MB_OK */);}
}

pub fn verify_once()->bool{
    *ACCEPTED.get_or_init(||{
        let observed=observed_exe();
        if let Ok((size,ref hash))=observed{
            if accepted_sha(size,hash){return true;}
        }
        let (size,actual,reason)=match observed {
            Ok((size,sha))=>(size.to_string(),sha,"unrecognized executable".to_string()),
            Err(error)=>("unavailable".to_string(),"UNAVAILABLE".to_string(),error),
        };
        let line=format!(
            "EXE-GATE: REJECTED reason={reason}; expected_sha256={EXPECTED_SHA256}; actual_sha256={actual}; expected_size={EXPECTED_SIZE}; actual_size={size}; no DirectDraw hooks installed"
        );
        // Preserve the rejection hash in the diagnostic log, without exposing
        // log internals or executable provenance in the user-facing dialog.
        let _=write_rejection(&line);
        dialog(&rejection_message(&actual));
        // Returning DDERR_GENERIC lets the game display a misleading
        // "Reinstall DirectX" dialog. A rejected EXE must never resume.
        // This code runs on the first DirectDraw export, not inside DllMain.
        std::process::exit(1);
    })
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn sha256_known_vectors(){
        assert_eq!(sha256(b""),"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256(b"abc"),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    #[test]
    fn rejection_dialog_has_no_provenance_or_logging_details(){
        let actual="5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67";
        let message=rejection_message(actual);
        assert_eq!(message,format!(
            "Only the executable with the following SHA-256 is supported:\n{EXPECTED_SHA256}\n\nYour executable:\n{actual}"
        ));
        assert!(!message.contains("unsafedisk"));
        assert!(!message.contains("log"));
        assert!(!message.contains("rejection"));
    }
    #[test]
    fn accepts_exact_owner_exe_only(){
        assert!(accepted_sha(EXPECTED_SIZE,EXPECTED_SHA256));
        assert!(!accepted_sha(EXPECTED_SIZE-1,EXPECTED_SHA256));
        assert!(!accepted_sha(EXPECTED_SIZE,&"0".repeat(64)));
        assert!(!accepted_sha(EXPECTED_SIZE,"5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67"));
        assert!(!accepted_sha(EXPECTED_SIZE,"612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d"));
    }
}
