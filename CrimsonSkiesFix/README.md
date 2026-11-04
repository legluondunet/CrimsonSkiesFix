# CrimsonSkiesFix 0.1.90

**Windows x86 DirectDraw proxy and modern display compatibility patch for Crimson Skies (2000).**

CrimsonSkiesFix adapts one strictly verified, SafeDisc-free Crimson Skies 1.02 executable to modern screens. The original executable is never rewritten. The active project is a single 32-bit `ddraw.dll` written in Rust; it also includes default-enabled, reversible GameOS (GOS) Video menu resource integration.

**Status:** experimental. Native 1920×1080 output, camera FOV/HUD adaptation, MainMenu signature, GameOS Resolution label, and the automatic disable/restore of the Video **Graphics** and **Resolution** dropdowns have been validated on Manjaro / Wine 11.0 / GE-Proton11-12. The Windows x86 DLL compiles, passes automated tests and loads/unloads successfully in a Windows GitHub Actions runner (including DirectDraw export checks). **Real Windows 11 gameplay testing is still postponed** until a suitable Windows computer is available.

## What it does

- Provides a compatible 32-bit DirectDraw proxy, with a native 3D RGB565/Z32 presentation path, desktop-size rendering, adapted FOV/HUD, and 4:3 video/menu handling. The previously experimental adapter and widescreen options are now enabled by default for the exact supported game executable.
- Suppresses the obsolete startup **driver/resolution selection dialog by default**, retaining the original HAL device selection. This verified in-memory bypass is gated by `Native3DWidescreen=1`.
- Shows the project version and build commit **only** on the MainMenu, including after returning from Preferences or gameplay; hides it in Video and other submenus.
- Displays the actual native display resolution in the Video Preferences **Resolution** label without changing the game's original graphics-device/mode tables.
- **By default**, uses the game's native `@ctl@PM` disable message to grey out and make **Graphics** and **Resolution** unselectable. Other Video options are not deliberately modified.
- **By default**, applies the bounded `LegacyOffscreenRGB565` correction for the game's first/legacy image when its exact 800×600 16-bit offscreen surface signature is recognized, avoiding the observed incorrect pink/magenta colors.
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
    ├── README.md
    ├── LICENSE                     # GPL-3.0 full license text
    └── build-info.txt              # source SHA and DLL SHA-256
```

**Default-on behavior in 0.1.90:** a fresh install suppresses the obsolete startup driver/resolution dialog, uses the native 3D adapter and GOS Graphics/Resolution lock, enables the first-image RGB565 color correction, activates focus repaint plus
the background-erase guard, and enables the Wine-only ROFFILE.DLL EOF guard
(subject to original DLL fingerprint verification). The **first game launch modifies only the small original** `GOSDATA/ASSETS/crimptch.rof` archive, after an exclusive backup. To opt out **before the first game launch**, create `CrimsonSkiesFix/CrimsonSkiesFix.ini` yourself and set `[Graphics] GosVideoLock=off` (and optionally `Native3DWidescreen=0` / `ExperimentalWindowedAdapter=0`). The `.ini.example` file alone is not the active configuration. Existing user INIs always take precedence and are never overwritten.

The DLL must be **next to the game executable**, not inside the `CrimsonSkiesFix` subfolder or a system directory. The supplied `.ini.example` is not loaded; edit the generated `CrimsonSkiesFix/CrimsonSkiesFix.ini` after launching once. Existing configurations are never overwritten by an update. Older `CrimsonFix` INI paths are imported only if the current INI does not already exist, without deleting the originals.

Wine users may need `WINEDLLOVERRIDES="ddraw=n,b"` to force the native proxy. The same Windows x86 DLL is used under Wine/Proton; it is **not** a Linux shared object.

## Verify the downloaded DLL

New CI artifacts include `CrimsonSkiesFix/build-info.txt` with the exact
Git source commit, the expected version/short-SHA text logged at startup,
and the SHA-256 and size of the corresponding `ddraw.dll`. This prevents
confusing an older DLL with a newer PR build that has the same download name.
It is **not** a cryptographic signature by the project author.

Under PowerShell, check the SHA-256 of the installed proxy:

```powershell
(Get-FileHash .\ddraw.dll -Algorithm SHA256).Hash
Get-Content .\CrimsonSkiesFix\build-info.txt
```

Compare the computed hash with `DllSha256` and check that the first line
of `CrimsonSkiesFix.log` begins with the `LogIdentity` value, followed
by ` - init`. The `Commit` and `Source` fields identify the exact
repository state. The distributed `LICENSE` is copied verbatim from
the repository and should be preserved when sharing the proxy DLL.
Older experimental archives may not contain these two files.

## Settings

All settings live in `CrimsonSkiesFix/CrimsonSkiesFix.ini`. Important graphics options:

| `[Graphics]` key | Default | Purpose |
|---|---|---|
| `Borderless` | `1` | Legacy DirectDraw window handling; by itself does not activate the modern windowed 3D adapter |
| `DesktopResolution` | `1` | Preserve the desktop mode and use monitor geometry |
| `FPSLimit` | `60` | Bounded frame-pacing target (1–60); API call counts are not a measured gameplay FPS |
| `ExperimentalWindowedAdapter` | `1` | Native DirectDraw4 3D adapter; enabled by default to support the locked Video settings and native display handling |
| `Native3DWidescreen` | `1` | Native monitor-sized FOV/HUD corrections **and suppression of startup driver/resolution chooser** |
| `FocusRepaint` | `1` | One-shot full repaint after focus return or exposed-window transition; set `0` to opt out |
| `WindowEraseGuard` | `1` | Prevent white background erasure over DirectDraw pixels on the verified game HWND; set `0` to opt out |
| `LegacyOffscreenRGB565` | `1` | First-image and legacy artwork RGB565 color correction; exact matching offscreen surfaces only; set `0` to disable |
| `ModernFPSPacing` | `0` | Optional modern Flip pacing experiment |
| `GosVideoLock` | `on` | **Default-on** native Graphics/Resolution lock; writes a verified, backed-up GOS patch overlay |

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

This is the **owner's previously validated configuration**. `LegacyOffscreenRGB565=1` is now enabled on a fresh install alongside `ExperimentalWindowedAdapter=1`, `Native3DWidescreen=1` and `GosVideoLock=on`. `FocusRepaint=1` and `WindowEraseGuard=1` are now enabled on fresh installs; only `Log=0` remains the default for ordinary diagnostics. The native GOS lock requires all three, including `DesktopResolution=1`; if any prerequisite is disabled by the user, installation is skipped safely.


**Window visibility fixes:** `FocusRepaint=1` requests a bounded one-shot full redraw when returning to the game or exposing a previously obscured window. `WindowEraseGuard=1` suppresses `WM_ERASEBKGND` only on the authenticated game window, preventing white background erasure of DirectDraw contents. The owner previously validated the guard's white-window improvement under Wine. Neither switch is a general crash handler or proven fix for the separately reported 0.1.90 gameplay crash.

### GOS Video dropdown lock, backup and restoration

On **first launch by default**, `GosVideoLock=on` makes the DLL verify the accepted executable and the original `crimson.rof` and `VIDEO.SCRIPT`, then create a small overlay containing the unchanged original `AIRFRAME.SCRIPT` patch and an edited, owner-derived `VIDEO.SCRIPT`. It modifies **only** the installed `GOSDATA/ASSETS/crimptch.rof`, saving a byte-for-byte original as:

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

## Wine ROFFILE.DLL INI parser guard (enabled by default)

A separate original-game issue can trigger an unhandled access violation at
`roffile.dll + 0x22E0` when the native GameOS INI-text parser reads beyond
the end of an exact-sized file allocation. This is tracked in
[issue #10](https://github.com/legluondunet/CrimsonSkiesFix/issues/10);
the original 32-bit ROFFILE.DLL was reverse-engineered, but the triggering
file's exact contents have **not** yet been confirmed.

The fix is **enabled by default for supported ROFFILE.DLL builds under Wine /
GE-Proton** and is always disabled at runtime on native Windows. It was
validated under Wine 11.0 / GE-Proton11-12 by the owner (two successful runs,
including four real padded allocations with zero allocation failures and
several menu/mission transitions). The precise mechanism of the historical
Windows/Wine difference is not established.

To disable the fix explicitly, change the persistent file
`CrimsonSkiesFix/CrimsonSkiesFix.ini`:

```ini
[Compatibility]
RoffileTerminatorFix=0
```

Setting `RoffileTerminatorFix=0` in an **existing** INI remains respected
after an upgrade; existing user settings are never overwritten. To enable
diagnostic messages temporarily, set `[Debug] Log=1`.

Once the game has loaded ROFFILE.DLL and reaches a hooked DirectDraw
render call, the proxy checks that the **on-disk DLL** exactly matches the
supported SHA-256
`1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a`
(135,168 bytes) and that the original code signatures are unchanged.
Only then does it replace an **in-memory** allocation call in
`InitIniFile`, using ROFFILE.DLL's **own allocator** for `file_size+2`
bytes and initializing both extra bytes to NUL. Original read, parse and
free functions remain unchanged. No game assets are modified on disk.

**Limits and fail-safe behavior:** unsupported binaries and failed
verifications are refused and logged as `ROFFILE-FIX: NOT installed`.
Installation of a supported binary is logged as `ROFFILE-FIX: installed`.
The diagnostic also tracks completed calls to the patched allocator using
lock-free counters. On the **next rendering callback after actual use** it logs,
for example:

```text
ROFFILE-FIX: allocation calls=1 padded_success=1 failures=0 last_input_size=1024; counted in cdecl hook, reported outside hook
```

`allocation calls` proves the game actually entered the patched allocation
path, unlike the installation marker alone. `padded_success` counts completed
`n+2` allocations, while `failures` counts unsuccessful/overflow requests.
The reported byte count is an example, not an expected file size. The logger
never runs from the native allocation hook, and reports are throttled.
If no allocation count appears after installation, then use of the hook has
**not been demonstrated** in that test. It does not mean the patch failed.

A DLL loaded and parsed entirely before the next graphics callback could
escape this opportunistic hook; an absent installation log means the fix
was never confirmed active. The hook remains restricted to the exact supported
ROFFILE.DLL SHA-256 and known instruction signatures. Unrecognized editions
are not modified. Tests under native Windows and additional long Wine sessions
remain desirable; no assumption of identical behavior on every computer
is made.

To turn the fix off on the next launch, set
`RoffileTerminatorFix=0`. This does not require restoring any original
game file because the patch never writes to disk.

## Logging and diagnostics

Set `[Debug] Log=1` to write `CrimsonSkiesFix.log` next to the loaded `ddraw.dll`; logging is normally off. Rejected executables are logged even when ordinary logging is disabled, if the directory is writable.

The initialization report provides privacy-filtered Windows/Wine, CPU, display, graphics and controller information. Optional `[Debug]` flags include `SurfaceUsageTrace`, `CrashBltTrace`, `ColorProbe` and `ColorCapture`, all disabled by default. Crash Blt tracing is intentionally verbose and writes a separate bounded rotating trace; disable it again after diagnosis.

Useful GOS log markers: `GOS-VIDEO-LOCK:`, `MENU-RESOLUTION-LABEL:`, `WINDOWED-WIDE:`, `MENU-BRANDING:`. If a new build fails, restore the previous `ddraw.dll` and relevant optional INI settings; for an already-installed GOS overlay use the explicit `restore` setting first.

## Validate that the game exits normally

A successful graphics session does not establish that Crimson Skies shuts
down cleanly. The DLL intentionally performs **no file I/O under the loader
lock in `DllMain`**, so the last line of `CrimsonSkiesFix.log` cannot be
used as an exit-status indicator.

The repository includes an optional read-only external process observer:
`tools/verify_game_exit.py`. On Linux, launch the **actual game process**
using the same working Wine/GE-Proton runner and prefix as your normal setup:

```sh
python3 /path/to/verify_game_exit.py \
  --cwd "/path/to/Crimson Skies" \
  -- /path/to/your/wine-runner CRIMSON.EXE
```

After quitting from the game's own menu, inspect
`CrimsonSkiesFix-exitcheck.report.txt`, plus the matching stdout/stderr files
if a nonzero code or unhandled exception is reported. A detached Lutris or
Steam GUI launcher is **not** a valid command to monitor, because it may
return while the game is still running. Exit status `0` with no unhandled
exception is evidence for a normal exit, not a guarantee that every runtime
resource was freed. A nonzero status needs interpretation.

The exact protocol and optional Windows 11 instructions are in
[exit validation](docs/exit-validation-2026-11-04.md). A **real GE-Proton 11-12 gameplay session** on `0.1.90-f2b2ca42` has now been observed to end with `0` in Lutris/umu, with no unhandled exception reported; this successful session did **not** run the Python observer. Native Windows 11 game exit remains untested. Automated synthetic process tests do not replace actual gameplay verification.

## Build and tests

```powershell
rustup target add i686-pc-windows-msvc
cd CrimsonSkiesFix
cargo test --locked --target i686-pc-windows-msvc
cargo build --locked --release --target i686-pc-windows-msvc
```

The repository commits `Cargo.lock` to pin the tested `flate2` dependency tree.

The [x86 workflow](../.github/workflows/crimsonskiesfix-x86.yml) builds `ddraw.dll`, verifies the PE machine type, runs Rust and safe ROF/Python synthetic tests, performs a native Windows x86 DLL load/export/unload smoke check, and packages the game layout with the GPL license and build provenance manifest. `windowed_ddraw_probe.exe` is an internal experimental DirectDraw diagnostic; its build and tests remain part of CI, but it is no longer published as an artifact and must not be placed in the game directory.

For original GameOS Video UI research and owner test evidence, see [Video Preferences native UI](docs/video-preferences-native-ui.md) and [GOS archive inspector](tools/inspect_gos_rof.py). Earlier experiments and detailed changelogs remain available in the repository's Git history.

## Known limitations

This is an experimental, exact-executable build. The native 3D adapter, menu lock, startup chooser bypass, first-image RGB565 color fix, `FocusRepaint` and `WindowEraseGuard` are now **enabled by default**, but the combined fresh-install 0.1.90 defaults have not yet been runtime-validated as a whole. Wine/GE-Proton runtime results for 0.1.89 do not establish compatibility on other PCs. Windows 11 gameplay testing has been explicitly deferred until a Windows machine is available. The Wine-only ROFFILE.DLL guard is default-on only for an exact supported binary; it has been tested by the owner under GE-Proton but not under native Windows. FPS pacing hooks do not by themselves establish simulation speed or gameplay FPS. The GOS lock changes a user-owned archive on disk when enabled and is **not automatically undone** on exit.

## License and contribution policy

GPL-3.0-or-later. Changes are prepared through GitHub pull requests; merging remains at the owner's discretion.

legluondunet assisted by ChatGPT (OpenAI)
