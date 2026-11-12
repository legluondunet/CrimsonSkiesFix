# CrimsonSkiesFix

**Windows x86 DirectDraw compatibility proxy** for *Crimson Skies (2000)* on Windows and Wine/Proton.

The active project builds a 32-bit Rust `ddraw.dll` for one exact, verified Crimson Skies v1.02 SafeDisc-free executable. It does **not** modify the executable on disk.

**Status: experimental / release candidate under evaluation.** Native 1920×1080 rendering, FOV/HUD, 4:3 menu/video handling, startup driver dialog bypass, the GOS Video preferences lock, first-image RGB565 correction, menu signature, focus recovery and the Wine-only ROFFILE guard have been tested in gameplay under Wine/GE-Proton. The Windows x86 continuous-integration job also loads and unloads the built DLL on Windows and checks its DirectDraw exports. **Actual gameplay on Windows 11 has not yet been validated**; neither successful CI nor a Windows DLL loading smoke test proves it.

## Supported executable

Only the validated, unmodified, unwrapped **Crimson Skies 1.0.2.0** executable is accepted:

| Property | Required value |
|---|---|
| Size | `2,580,480` bytes |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

Other executables (including protected launchers and third-party patched versions) are rejected. The actual unsupported hash is logged when a file can be read and the log path is writable.

If the installation is older than 1.02, the [official 1.02 updater](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102) may be required. It does **not** remove SafeDisc.

## Install

The workflow now publishes **one downloadable artifact only**: `CrimsonSkiesFix-Windows-x86-Standalone-EXPERIMENTAL`, containing `CrimsonSkiesFix.exe`. Download it from a successful [Windows x86 GitHub Actions run](https://github.com/legluondunet/CrimsonSkiesFix/actions); the standalone installer includes the exact, verified `ddraw.dll` and deploys it automatically. The former `CrimsonSkiesFix-ddraw-Windows-x86-EXPERIMENTAL` archive is no longer published.

For a first installation, place one original supported ISO or CUE/BIN image, the official 1.02 updater, and your own SHA-256-approved decrypted 1.02 executable beside the launcher, then run `CrimsonSkiesFix.exe`. For an already verified installation, the launcher verifies and launches the game without requiring the original media.

The downloaded artifact contains **only the launcher**: it never distributes the original game, game assets or decrypted executable. The installed game receives its own `ddraw.dll`, documentation and `CrimsonSkiesFix.ini.example`; an active user INI is created on first game launch, never bundled. By default, `GosVideoLock=on` may modify the small original `GOSDATA/ASSETS/crimptch.rof` overlay after verifying and backing it up. Set `GosVideoLock=restore` to restore the original archive before removing the fix if desired.

For the exact supported media and the installation/repair flow, see the [detailed installation guide](CrimsonSkiesFix/README.md).

## Identify a build

`CrimsonSkiesFix/build-info.txt` in each newly packaged CI artifact records the full source commit, the expected eight-character build label, and the SHA-256 of the exact `ddraw.dll`. Compare `LogIdentity` with the first line of `CrimsonSkiesFix.log` while testing and compare `DllSha256` with the installed DLL. These values identify a build and help catch accidental version mismatches; they are **not** a digital signature.

The original game's and ROFFILE.DLL's expected hashes are separate from the **proxy DLL** hash in this manifest.

## Build and quality gates

```powershell
cd CrimsonSkiesFix
rustup target add i686-pc-windows-msvc
cargo test --locked --target i686-pc-windows-msvc
cargo build --locked --release --target i686-pc-windows-msvc
```

The [Windows x86 workflow](.github/workflows/crimsonskiesfix-x86.yml) compiles the PE32 DLL, runs Rust and Python regression checks, validates the clean distribution layout, and performs a native `LoadLibraryW` / `GetProcAddress` / `FreeLibrary` smoke test. The standalone `windowed_ddraw_probe.exe` remains in the repository for DirectDraw regression tests and its x86 build is checked by CI. It is **not** required to run the game and is no longer published as a downloadable artifact.

Release status and remaining checks are tracked in [release readiness audit](CrimsonSkiesFix/docs/release-readiness-2026-11-04.md). Native Windows 11 gameplay remains a distinct, manual acceptance criterion.

## License and contribution policy

This repository is licensed **GPL-3.0-or-later**; see [LICENSE](LICENSE). Experimental archives include an identical copy in the `CrimsonSkiesFix` subfolder. Changes are proposed via pull requests; merging and publication of a stable release require explicit owner approval.

legluondunet assisted by ChatGPT (OpenAI)
