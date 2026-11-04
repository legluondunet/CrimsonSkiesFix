# CrimsonSkiesFix 0.1.89

**Windows x86 DirectDraw proxy and modern display compatibility patch for Crimson Skies (2000).**

CrimsonSkiesFix adapts one strictly verified, SafeDisc-free Crimson Skies 1.02 executable to modern screens. The original executable is never rewritten. The active project is a single 32-bit `ddraw.dll` written in Rust; it also includes optional, reversible GameOS (GOS) Video menu resource integration.

**Status:** experimental. Native 1920×1080 output, camera FOV/HUD adaptation, MainMenu signature, GameOS Resolution label, and the automatic disable/restore of the Video **Graphics** and **Resolution** dropdowns have been validated on Manjaro / Wine 11.0 / GE-Proton11-12. The Windows x86 DLL compiles and passes unit tests in GitHub Actions, but **real Windows runtime testing is postponed** until a Windows computer is available.

## What it does

- Provides a compatible 32-bit DirectDraw proxy, with a native 3D RGB565/Z32 presentation path, desktop-size rendering, adapted FOV/HUD, and 4:3 video/menu handling when the experimental adapter and widescreen options are enabled.
- Skips the legacy graphics adapter/resolution prompt for the authenticated executable.
- Shows the project version and build commit **only** on the MainMenu, including after returning from Preferences or gameplay; hides it in Video and other submenus.
- Displays the actual native display resolution in the Video Preferences **Resolution** label without changing the game's original graphics-device/mode tables.
- Optionally uses the game's native `@ctl@PM` disable message to grey out and make **Graphics** and **Resolution** unselectable. Other Video options are not deliberately modified.
- Preserves the player's INI file and offers optional diagnostic logging and the narrow rendering/focus recovery controls described below.

### Supported executable

Only the exact verified, unmodified SafeDisc-free **Crimson Skies 1.0.2.0** executable is accepted. Other versions, protected original launchers, or third-party modified executables are rejected before graphics hooks are installed.

| Property | Required value |
|---|---|
| Executable | Original validated 1.02 unwrapped binary (usually `CRIMSON.EXE`, sometimes named `CRIMSON-UNWRAPPED.EXE`) |
| Size | **2,580,480 bytes** |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

On a mismatch, the DLL reports the actual SHA-256 and refuses to launch; it never silently falls back to an unsupported executable.

## Installation

Obtain the **CrimsonSkiesFix-ddraw-Windows-x86-EXPERIMENTAL** artifact from a successful [Windows x86 GitHub Actions build](https://github.com/legluondunet/CrimsonSkiesFix/actions). Copy the contents alongside your supported executable (back up any previous `ddraw.dll` first):

```text
Crimson Skies/
├── CRIMSON.EXE                     # supported SHA-256 (name may differ)
├── ddraw.dll                       # CrimsonSkiesFix Windows x86 DLL
├── GOSDATA/
│   └── ASSETS/
│       ├── crimson.rof             # original game content: DO NOT replace
│       └── crimptch.rof            # small original GOS overlay
└── CrimsonSkiesFix/
    ├── CrimsonSkiesFix.ini.example # shipped with the build
    ├── CrimsonSkiesFix.ini         # generated once; never shipped
    └── README.md
```

The DLL must be **next to the game executable**, not inside the `CrimsonSkiesFix` subfolder or a system directory. The supplied `.ini.example` is not loaded; edit the generated `CrimsonSkiesFix/CrimsonSkiesFix.ini` after launching once. Existing configurations are never overwritten by an update. Older `CrimsonFix` INI paths are imported only if the current INI does not already exist, without deleting the originals.

Wine users may need `WINEDLLOVERRIDES="ddraw=n,b"` to force the native proxy. The same Windows x86 DLL is used under Wine/Proton; it is **not** a Linux shared object.

## Settings

All settings live in `CrimsonSkiesFix/CrimsonSkiesFix.ini`. Important graphics options:

| `[Graphics]` key | Default | Purpose |
|---|---|---|
| `Borderless` | `1` | Legacy DirectDraw window handling; by itself does not activate the modern windowed 3D adapter |
| `DesktopResolution` | `1` | Preserve the desktop mode and use monitor geometry |
| `FPSLimit` | `60` | Bounded frame-pacing target (1–60); API call counts are not a measured gameplay FPS |
| `ExperimentalWindowedAdapter` | `0` | Opt-in DirectDraw4 3D adapter for modern borderless/native rendering |
| `Native3DWidescreen` | `0` | Opt-in monitor-sized 3D resolution, FOV and HUD corrections |
| `FocusRepaint` | `0` | Optional focus/foreground repaint |
| `WindowEraseGuard` | `0` | Optional suppression of `WM_ERASEBKGND` for the verified game window |
| `LegacyOffscreenRGB565` | `0` | Optional RGB565 correction for legacy menu/video images |
| `ModernFPSPacing` | `0` | Optional modern Flip pacing experiment |
| `GosVideoLock` | `off` | Native GOS Graphics/Resolution dropdown override: `off`, `on` or `restore` |

The Wine-validated native widescreen + GOS dropdown configuration is:

```ini
[Graphics]
Borderless=1
DesktopResolution=1
ExperimentalWindowedAdapter=1
Native3DWidescreen=1
FocusRepaint=1
WindowEraseGuard=1
LegacyOffscreenRGB565=1
GosVideoLock=on

[Debug]
Log=1
```

This is a **tested example**, not the generated defaults. `GosVideoLock=on` requires `ExperimentalWindowedAdapter=1`, `Native3DWidescreen=1` and `DesktopResolution=1`; the DLL skips the operation when these requirements are not satisfied.

### GOS Video dropdown lock, backup and restoration

With `GosVideoLock=on`, the DLL verifies the accepted executable and the original `crimson.rof` and `VIDEO.SCRIPT`, then creates a small overlay containing the unchanged original `AIRFRAME.SCRIPT` patch and an edited, owner-derived `VIDEO.SCRIPT`. It modifies **only** the installed `GOSDATA/ASSETS/crimptch.rof`, saving a byte-for-byte original as:

```text
GOSDATA/ASSETS/crimptch.rof.CrimsonSkiesFix.original.bak
```

The original `crimson.rof`, executable and DLL are not modified. A repeat launch recognizes the existing override and does not rewrite it. Unsupported/modified archives, missing or inconsistent backups and errors are handled conservatively. No original game script is distributed in the repository or DLL.

| Setting | Effect |
|---|---|
| `GosVideoLock=off` | Do nothing to game archives; **does not remove** an existing patch |
| `GosVideoLock=on` | Install or recognize the native disabled-state overlay, with a verified backup |
| `GosVideoLock=restore` | Restore the original `crimptch.rof` from backup at the next launch, then consume the backup |

After restoring, change the setting back to `off` (or `on` to install it again). **Always use `restore` before removing the DLL if you want the original unmodified GOS Video screen.** The override is persistent across game exits and DLL removal.

The native install, repeat-launch idempotence and native restore have been **confirmed by the owner under Wine/GE-Proton** on 4 November 2026, with installation and restoration success messages captured in logs. This is **not** a claim of runtime validation under Windows 11.

## Logging and diagnostics

Set `[Debug] Log=1` to write `CrimsonSkiesFix.log` next to the loaded `ddraw.dll`; logging is normally off. Rejected executables are logged even when ordinary logging is disabled, if the directory is writable.

The initialization report provides privacy-filtered Windows/Wine, CPU, display, graphics and controller information. Optional `[Debug]` flags include `SurfaceUsageTrace`, `CrashBltTrace`, `ColorProbe` and `ColorCapture`, all disabled by default. Crash Blt tracing is intentionally verbose and writes a separate bounded rotating trace; disable it again after diagnosis.

Useful GOS log markers: `GOS-VIDEO-LOCK:`, `MENU-RESOLUTION-LABEL:`, `WINDOWED-WIDE:`, `MENU-BRANDING:`. If a new build fails, restore the previous `ddraw.dll` and relevant optional INI settings; for an already-installed GOS overlay use the explicit `restore` setting first.

## Build and tests

```powershell
rustup target add i686-pc-windows-msvc
cd CrimsonSkiesFix
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

The [x86 workflow](../.github/workflows/crimsonskiesfix-x86.yml) builds `ddraw.dll`, verifies the PE machine type, runs Rust and safe ROF/Python synthetic tests, and packages the user installation layout. `windowed_ddraw_probe.exe` is a **separate experimental diagnostic tool**, not required in the game directory.

For original GameOS Video UI research and owner test evidence, see [Video Preferences native UI](docs/video-preferences-native-ui.md) and [GOS archive inspector](tools/inspect_gos_rof.py). Earlier experiments and detailed changelogs remain available in the repository's Git history.

## Known limitations

This is an experimental, exact-executable build. Modern DirectDraw adapter settings are opt-in and require more gameplay coverage; Wine/GE-Proton runtime results do not establish compatibility on other PCs. Windows 11 gameplay testing has been explicitly deferred until a Windows machine is available. FPS pacing hooks do not by themselves establish simulation speed or gameplay FPS. The GOS lock changes a user-owned archive on disk when enabled and is **not automatically undone** on exit.

## License and contribution policy

GPL-3.0-or-later. Changes are prepared through GitHub pull requests; merging remains at the owner's discretion.

legluondunet assisted by ChatGPT (OpenAI)
