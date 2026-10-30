# CrimsonSkiesFix

Experimental **32-bit Windows DirectDraw compatibility proxy** for *Crimson Skies (2000)* on Windows and Wine/Proton. This repository contains the DirectDraw `ddraw.dll` project only.

## Status

- Windows x86 DirectDraw proxy with opt-in diagnostics and experimental primary-`Flip` pacing
- Direct3D3 scene/Flip instrumentation to study the game's rendering behavior
- **Not yet a working borderless / native-widescreen solution**; 3D rendering, game simulation speed, and focus behavior need gameplay validation

## Build

```sh
cd CrimsonSkiesFix
rustup target add i686-pc-windows-msvc
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

The [Windows x86 workflow](.github/workflows/crimsonskiesfix-x86.yml) compiles the DLL, runs tests, verifies its 32-bit PE format and packages the game-ready layout.

## Install

Test with a separate copy of the unwrapped game executable:

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log                  # created only with [Debug] Log=1
  CrimsonSkiesFix/
    CrimsonSkiesFix.ini.example        # shipped template
    CrimsonSkiesFix.ini                # persistent user settings (not shipped)
    README.md
```

The log is written beside the loaded DLL, **not** inside the configuration folder. The active INI is created only once and preserved across updates. Older configuration files are imported on first run when appropriate, without overwriting or deleting originals. Logging and modern frame pacing are disabled by default.

See [DLL installation, settings, diagnostics and limitations](CrimsonSkiesFix/README.md).

## License

GPL-3.0-or-later — see [LICENSE](LICENSE). Provenance notices for the diagnostics code remain in the source documentation.
