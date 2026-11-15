use std::{env,fs,path::PathBuf,process::Command};

const MAX_ORDINAL:u32=1023;
const NAMED_START:u32=2048;

fn main(){
    let target=env::var("TARGET").expect("Cargo target");
    assert_eq!(target,"i686-pc-windows-msvc",
        "WinMM runtime is only built for i686-pc-windows-msvc");
    let root=PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo root"));
    let original_def=root.join("src/winmm_full_exports.def");
    println!("cargo:rerun-if-changed={}",original_def.display());
    let bridge=env::var_os("CARGO_FEATURE_WINMM_ORDINAL_BRIDGE").is_some();
    let def=if bridge {
        let out=PathBuf::from(env::var("OUT_DIR").expect("Cargo out"));
        let source=fs::read_to_string(&original_def).expect("WinMM named export definition");
        let mut generated=String::from(
            "; Experimental ordinal bridge: all numeric ordinals 1..1023 delegate\n\
             ; by the SAME ordinal to the native Windows/Wine WinMM provider.\n\
             ; Name-only entries reside at 2048+, avoiding numeric alias collision.\n\
             LIBRARY winmm.dll\nEXPORTS\n");
        let mut names=0u32;
        for line in source.lines(){
            let trimmed=line.split(';').next().unwrap_or("").trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("EXPORTS") ||
                trimmed.starts_with("LIBRARY ") {continue;}
            let token=trimmed.split_whitespace().next().expect("export token");
            let Some((name, internal))=token.split_once('=') else {
                panic!("unnamed or malformed prototype export: {trimmed}")
            };
            assert!(internal.starts_with("crimson_winmm_forward_") ||
                    internal.starts_with("crimson_winmm_extra_"),"unexpected symbol in .def");
            assert!(!name.is_empty() && name.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'_'));
            generated.push_str(&format!("    {name}={internal} @{}\n",NAMED_START+names));
            names+=1;
        }
        assert_eq!(names,185,"expected 185 original WinMM exported names");
        // The shared Rust runtime also exports these two no_mangle symbols.
        // Assign explicit high ordinals; otherwise the linker silently places
        // them in the numeric-only region/gap at the first free number.
        generated.push_str(
            "    CrimsonSkiesFixBootstrapStatus=CrimsonSkiesFixBootstrapStatus @2233\\n\\
             DllMain=DllMain @2234\\n");
        let mut assembly=String::from("    .text\n");
        for number in 1..=MAX_ORDINAL {
            generated.push_str(&format!(
                "    crimson_winmm_ordinal_{number}=crimson_winmm_ordinal_{number} @{number} NONAME\n"
            ));
            assembly.push_str(&format!(
                "    .globl _crimson_winmm_ordinal_{number}\n\
                 .p2align 4, 0x90\n\
                 _crimson_winmm_ordinal_{number}:\n\
                     pushal\n\
                     pushl ${number}\n\
                     call {{resolve}}\n\
                     addl $4, %esp\n\
                     movl %eax, 28(%esp)\n\
                     popal\n\
                     jmp *%eax\n"
            ));
        }
        let output_def=out.join("winmm-ordinal-bridge.def");
        fs::write(&output_def,generated).expect("write prototype WinMM .def");
        fs::write(out.join("winmm-ordinal-thunks.s"),assembly)
            .expect("write prototype ABI-preserving x86 thunks");
        println!("cargo:warning=Building EXPERIMENTAL ordinal-bridge WinMM, NOT a release runtime");
        output_def
    } else {original_def};
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
