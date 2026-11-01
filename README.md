# CrimsonSkiesFix

Experimental **32-bit Windows DirectDraw compatibility proxy** for *Crimson Skies (2000)* on Windows and Wine/Proton. This repository contains the DirectDraw `ddraw.dll` project only.

## Status

- Windows x86 DirectDraw proxy with opt-in diagnostics and experimental primary-`Flip` pacing
- Direct3D3 scene/Flip instrumentation to study the game's rendering behavior
- **Experimental borderless and native 3D widescreen/FOV adaptation**; full gameplay stability, transitions, HUD and Windows/Wine parity still require validation

## Supported game executable

Only the unmodified Crimson Skies **v1.02** executable is accepted.
The DirectDraw proxy checks the on-disk SHA-256 **before installing graphics hooks**.
Third-party widescreen-patched executables, protected EXEs and other game
versions are refused with an error dialog. The detected SHA-256 is recorded in
`CrimsonSkiesFix.log` even when normal logging is disabled (if writable).

- **EXE size:** 2,580,480 bytes
- **Required SHA-256:** `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`

The DLL applies resolution, FOV and HUD adjustments **in memory**;
it does not alter the original executable on disk. These graphics
adjustments are still experimental and require game testing.

For installations older than v1.02, the
[official v1.02 updater (archived)](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102)
may be required, depending on the game's language/edition. That updater
does **not** remove SafeDisc.

## Build

```sh
cd CrimsonSkiesFix
rustup target add i686-pc-windows-msvc
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

The [Windows x86 workflow](.github/workflows/crimsonskiesfix-x86.yml) compiles the DLL, runs tests, verifies its 32-bit PE format and packages the game-ready layout.

## Install

Test with the exact unmodified SafeDisc-free Crimson Skies v1.02 executable produced by our private research tool (other EXE hashes are rejected):

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log                  # diagnostics or rejected game EXE
  CrimsonSkiesFix/
    CrimsonSkiesFix.ini.example        # shipped template
    CrimsonSkiesFix.ini                # persistent user settings (not shipped)
    README.md
```

The log is written beside the loaded DLL, **not** inside the configuration folder. The active INI is created only once and preserved across updates. Older configuration files are imported on first run when appropriate, without overwriting or deleting originals. Logging and modern frame pacing are disabled by default.

See [DLL installation, settings, diagnostics and limitations](CrimsonSkiesFix/README.md).

## License

GPL-3.0-or-later — see [LICENSE](LICENSE). Provenance notices for the diagnostics code remain in the source documentation.
