fn main() {
    println!("cargo:rerun-if-changed=native/system_diagnostics.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Adapted from SpiderMan2k1Fix's GPL-3.0-or-later passive diagnostics.
        // Compile into the existing x86 Rust cdylib, no external diagnostics DLL.
        cc::Build::new()
            .file("native/system_diagnostics.c")
            .warnings(false)
            .compile("crimson_system_diagnostics");
        println!("cargo:rustc-link-lib=advapi32");
        println!("cargo:rustc-link-lib=dxguid");
        println!("cargo:rustc-link-lib=user32");
    }
}
