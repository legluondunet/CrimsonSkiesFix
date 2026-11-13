# CrimsonSkiesFix

**Windows x86 DirectDraw compatibility proxy** for *Crimson Skies (2000)* on Windows and Wine/Proton.

The active project builds a 32-bit Rust `ddraw.dll` for one exact, verified Crimson Skies v1.02 SafeDisc-free executable. It does **not** modify the executable on disk.

**Status: experimental release candidate, not yet published.** The owner validated original **French and German CD installation**, first and repeat launch, native Graphics/Resolution lock and the earlier unified-log build under Manjaro / GE-Proton11-12, with 1920×1080 rendering. The USA/Europe CD is recognized by strictly pinned file hashes and is covered by synthetic checks, but **no current original USA disc is available for an end-to-end test**. Native Windows 11 was exercised with an earlier DLL/opt-in profile; **the latest standalone installer, default settings and in-memory GOS VIDEO.SCRIPT substitution have not been game-tested on native Windows**. The Windows CI loads the x86 DLL and verifies 22 exports; that is not a gameplay test.

## Supported executable

Only the validated, unmodified, unwrapped **Crimson Skies 1.0.2.0** executable is accepted:

| Property | Required value |
|---|---|
| Size | `2,580,480` bytes |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

Other executables (including protected launchers and third-party patched versions) are rejected. The actual unsupported hash is logged when a file can be read and the log path is writable.

If the installation is older than 1.02, the [official 1.02 updater](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102) may be required. It does **not** remove SafeDisc.

## Install

The build workflow publishes **one experimental artifact**: `CrimsonSkiesFix-Windows-x86-Standalone-EXPERIMENTAL`, containing only `CrimsonSkiesFix.exe`. Obtain it from a successful [Windows x86 GitHub Actions run](https://github.com/legluondunet/CrimsonSkiesFix/actions). The launcher embeds the exact x86 DLL verified during the same build and installs it beside the approved game executable. The obsolete standalone DLL archive is not published.

For a first installation, place one original supported ISO or CUE/BIN image, the official 1.02 updater, and your own SHA-256-approved decrypted 1.02 executable beside the launcher, then run `CrimsonSkiesFix.exe`. For an already verified installation, the launcher verifies and launches the game without requiring the original media. The bundled, CI-checked DirectDraw patch is embedded in the one-file launcher.

To **install completely without launching the game**, run `CrimsonSkiesFix.exe --install-only`. The approved decrypted EXE, embedded DirectDraw patch and authentic 1.02 updater are all checked **before** any CD extraction begins, including with the manual `--install-cd` or `--extract-cd` commands. The read-only `--inspect-cd` remains available without these sources. For a verified installation, no original external sources are needed. To explicitly launch an existing game **without file-presence or SHA checks, repair, or extraction**, run `CrimsonSkiesFix.exe --launch`; this tries only `CrimsonSkiesFix/CRIMSON.EXE`. It is an unsafe-by-design opt-in that cannot repair missing/modified files and will fail at the OS level if the game cannot start. The ordinary no-argument path remains verified.

The downloaded artifact contains **only the launcher**: it never distributes the original game, game assets or decrypted executable. The installed game receives its own `ddraw.dll`, documentation and `CrimsonSkiesFix.ini.example`; an active user INI is created on first game launch, never bundled. By default, `GosVideoLock=on` may modify the small original `GOSDATA/ASSETS/crimptch.rof` overlay after verifying and backing it up. Set `GosVideoLock=restore` to restore the original archive before removing the fix if desired.

**The new first-run INI contains `[Graphics] AspectRatio=auto`, `FPSLimit=60` and `[Debug] Log=0`.** Choose `AspectRatio=4:3` for native-resolution 4:3 gameplay with centered black sidebars, 4:3 camera/HUD and the original 4:3 menus/videos, without changing the desktop resolution. Mandatory widescreen, fullscreen, GOS, Windows/Wine and first-image compatibility fixes are compiled in and applied automatically to authenticated games. Advanced diagnostic and `GosVideoLock=restore` options are accepted when explicitly added but are not cluttering the generated file. Existing personal INIs are preserved unchanged.

The current build keeps **two independent logs** beside the launcher: `CrimsonSkiesFix_installer.log` (last installation, repair or failure; unchanged on successful routine relaunches) and `CrimsonSkiesFix.log` (fresh DLL gameplay diagnostics for each game session). Explicit CD inspection commands also use the installer log. `--launch` does not touch that report; `--install-only` never creates a game process. See the [detailed installation guide](CrimsonSkiesFix/README.md).

## Identify a build

The launcher creates `CrimsonSkiesFix/build-info.txt` **inside the installed game folder** (not inside the one-file downloadable artifact). It records the full source commit, the expected eight-character build label, and the SHA-256 of the embedded `ddraw.dll`. Compare `LogIdentity` with the first game initialization line in `CrimsonSkiesFix.log`; separately consult `CrimsonSkiesFix_installer.log` for the last meaningful installation/repair report. Compare `DllSha256` with the installed DLL. These values identify a build and help catch accidental version mismatches; they are **not** a digital signature.

The original game's and ROFFILE.DLL's expected hashes are separate from the **proxy DLL** hash in this manifest.

## Build and quality gates

```powershell
cd CrimsonSkiesFix
rustup target add i686-pc-windows-msvc
cargo test --locked --target i686-pc-windows-msvc
cargo build --locked --release --target i686-pc-windows-msvc
```

The [Windows x86 workflow](.github/workflows/crimsonskiesfix-x86.yml) compiles the PE32 DLL, runs Rust and Python regression checks, validates the clean distribution layout, and performs a native `LoadLibraryW` / `GetProcAddress` / `FreeLibrary` smoke test. The standalone `windowed_ddraw_probe.exe` remains in the repository for DirectDraw regression tests and its x86 build is checked by CI. It is **not** required to run the game and is no longer published as a downloadable artifact.

Current release scope and remaining checks are tracked in the [12 November release preparation](CrimsonSkiesFix/docs/release-preparation-2026-11-12.md). The [4 November readiness audit](CrimsonSkiesFix/docs/release-readiness-2026-11-04.md) is kept as a historical checkpoint. Native Windows gameplay with this exact candidate remains a separate, later test.

## License and contribution policy

This repository is licensed **GPL-3.0-or-later**; see [LICENSE](LICENSE). Experimental archives include an identical copy in the `CrimsonSkiesFix` subfolder. Changes are proposed via pull requests; merging and publication of a stable release require explicit owner approval.

legluondunet assisted by ChatGPT (OpenAI)
