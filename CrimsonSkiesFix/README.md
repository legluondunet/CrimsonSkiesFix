# CrimsonSkiesFix 1.0

**Windows x86 DirectDraw proxy and modern display compatibility patch for Crimson Skies (2000).**

CrimsonSkiesFix adapts one strictly verified, SafeDisc-free Crimson Skies 1.02 executable to modern screens. The original executable is never rewritten. The active project is a single 32-bit `ddraw.dll` written in Rust; it also includes default-enabled, reversible GameOS (GOS) Video menu resource integration.

**Status (13 November 2026): version 1.0 selected; release publication pending.** The owner validated original **FR and DE disc installs**, repeat launch without re-extracting, and non-editable native Graphics/Resolution options under **GE-Proton11-12 / Wine 11.0** on Manjaro (1920×1080). The final German run also verified the managed GOS overlay on the subsequent launch and a clean game exit. A real USA/Europe original CUE/BIN has now been inspected (`C_SKIES_DISC_001`, 112/112 internal-file SHA-256 matches) and installed using `--install-only` on candidate `0.1.90-59d81dd7` (45 original GAME files, all eight CABs expanded, verified data and approved EXE, no game launch). **USA/Europe gameplay still needs testing.** Earlier native Windows 11 gameplay used an opt-in profile on older code; the **current standalone installer and in-memory VIDEO.SCRIPT substitution are not yet validated in native Windows gameplay**. The 22 DirectDraw exports are verified in CI. See [current release scope](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/release-preparation-2026-11-12.md).

## What it does

- Provides a compatible 32-bit DirectDraw proxy, with a native 3D RGB565/Z32 presentation path, desktop-size rendering, adapted FOV/HUD, and 4:3 video/menu handling. The previously experimental adapter and widescreen options are now enabled by default for the exact supported game executable.
- Suppresses the obsolete startup **driver/resolution selection dialog by default**, retaining the original HAL device selection. This verified in-memory bypass is gated by `Native3DWidescreen=1`.
- Shows the project version and build commit **only** on the MainMenu, including after returning from Preferences or gameplay; hides it in Video and other submenus.
- Displays the actual native display resolution in the Video Preferences **Resolution** label without changing the game's original graphics-device/mode tables.
- **By default**, uses the game's native `@ctl@PM` disable message to grey out and make **Graphics** and **Resolution** unselectable. Other Video options are not deliberately modified.
- **By default**, applies the bounded `LegacyOffscreenRGB565` correction for the game's first/legacy image on exact 800×600 sources. The tested native Windows XRGB8888 movie pipeline additionally converts the complete original RGB565 opening image into a private 32-bit staging surface before same-format presentation; under Wine/Proton the regular RGB565 copy remains unchanged.
- Preserves the player's INI file and offers optional diagnostic logging and the narrow rendering/focus recovery controls described below.

## Platform-specific code after cleanup

The same Win32 x86 DLL is loaded on both hosts. Windowed DirectDraw4 surface forwarding, validated surface copy/presentation, widescreen FOV/HUD, FPS pacing, GOS menu lock and authenticated ROF archive routing use common code. The actual **surface format** is verified before the shared Blt presenter receives it; there is no separate operating-system implementation of that presenter.

| Runtime scope | Remaining reason for platform distinction |
|---|---|
| Native Windows | Early DD4 bootstrap to avoid exclusive display switching; physical-vs-DPI-logical monitor coordinates; the verified XRGB8888 800×600 movie/menu graph, 4:3 offscreen composition and RGB565 opening-image staging; native mouse/DirectInput compatibility repairs |
| Wine / Proton | Exact-hash-guarded ROFFILE.DLL EOF parser allocation repair, only when Wine is detected; normal RGB565 movie/menu graph remains selected |
| Both | Common native DirectDraw4 COM forwarding/Flip-to-Blt, game resource authentication, GOS script lock and menu version label |

The obsolete native GDI sidebar fallback, Windows exclusive-intro handoff, and temporary ColorProbe/ColorCapture BMP debugging modules were removed after owner validation. Existing INI files are preserved; retired debug keys are ignored. Other experimental native Windows paths are still opt-in and require testing before becoming defaults.

### Supported executable

Only the exact verified, unmodified SafeDisc-free **Crimson Skies 1.0.2.0** executable is accepted. Other versions, protected original launchers, or third-party modified executables are rejected before graphics hooks are installed.

| Property | Required value |
|---|---|
| Executable | Original validated 1.02 unwrapped binary (usually `CRIMSON.EXE`, sometimes named `CRIMSON-UNWRAPPED.EXE`) |
| Size | **2,580,480 bytes** |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

On a mismatch, the DLL reports the actual SHA-256 and refuses to launch; it never silently falls back to an unsupported executable.

## Standalone automatic installation and repair (integrated into main)

The separate Windows-x86 standalone launcher, **CrimsonSkiesFix.exe**, now
automatically verifies the existing game and launches it when all pinned
hashes match. When no game exists, it installs from an owner-provided
authentic USA/Europe, FR or DE ISO/CUE and the official 1.02 update, then
installs an exact-hash-approved, user-provided decrypted 1.02 game EXE and
launches the game. The original protected EXE is preserved as
CRIMSON.EXE.protected.bak.

Place one original ISO or CUE with its BIN/IMG, the official 1.02
updater, and your own **2,580,480-byte** decrypted EXE with SHA-256
43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75
beside the launcher; then run it without arguments. To install/repair and verify **without launching**, use `CrimsonSkiesFix.exe --install-only`. Before any disc extraction the installer requires (1) an exact approved owner-provided 1.02 decrypted executable, (2) the integrated x86 DirectDraw patch, and (3) the authenticated official 1.02 updater. If any is absent or invalid, extraction is refused and any existing installation is preserved. All CD detection, extraction and official-update handling is now internal to the normal and `--install-only` installation workflows; there are no separate CD command-line options. The ordinary no-argument path continues to verify, repair and launch.

External originals
are needed only for a fresh installation or repair. A fully verified
installed game launches without media. The verifier recognizes the exact
official 1.02 CRIMPTCH.ROF **or** CrimsonSkiesFix's authenticated GOS Video
overlay with its independently verified official-original backup; a managed
overlay is not CD damage and must not trigger extraction. A bundled-DLL-only
version change is repaired by replacing ddraw.dll, without rebuilding the game.
The installer log reports `GosOverlayState=OFFICIAL_102` or
`GosOverlayState=VERIFIED_MANAGED` after a successful verification.

If files are damaged, the launcher constructs a separate verified
installation, migrates the player's SavedGames, Planes and active INI,
and retains the old installation in CrimsonSkiesFix.previous rather
than deleting it. If a required original source is absent, it refuses
to overwrite the current directory. Two independent logs sit beside `CrimsonSkiesFix.exe`:
`CrimsonSkiesFix_installer.log` records the **most recent meaningful installation,
repair or installer error**, while `CrimsonSkiesFix.log` is
created afresh by the DirectDraw DLL at **every game launch**. A successful
normal verification/relaunch leaves the previous installer report untouched.
**Unified installer CLI (release 1.0 candidate, PR #28):** `--install-only` installs,
verifies and repairs known 1.02 components without launching; `--verify`
checks read-only; `--launch-no-verify` starts directly. For an unsupported
disc, `--inspect image.iso`, `--inspect image.cue` or
`--inspect image.cue image.bin/img` produces a separate
`CrimsonSkiesFix_inspect.log` to send in a support report.
`--verbose` can accompany a command; `--help` and `-h` are equivalent.
Images passed directly without `--inspect` are no longer accepted.

A successful `--verify` is read-only and does not replace the installer report; an installation or repair failure records the actionable error. The
launcher can safely migrate a recognized older `CrimsonSkiesFix-installer.log` to the underscore name if the new file is absent, and removes the older `CrimsonSkiesFix.installer.log` only after writing a
new real installer report, and only when its small, regular-file contents
prove it was generated by the old launcher. Other files are left untouched.

**Owner gameplay tests cover FR and DE under GE-Proton.** USA/Europe now has a real CD/CUE inspection and verified `--install-only` installation under Wine/GE-Proton, but a real USA/Europe gameplay session is outstanding. Native Windows tests of this exact build are outstanding. The existing manual DLL installation described below remains available. The original game, decrypted executable and protected installer are **never bundled**. A deliberate `CrimsonSkiesFix.exe --launch-no-verify` instead tries the existing `CrimsonSkiesFix/CRIMSON.EXE` **without checking its presence, SHA-256, GOS assets or the bundled proxy**; no installation, verification or repair is attempted, and the OS may refuse to start it. This option does not alter the safety checks of the default launcher.

Detailed behavior and testing checklist:
[automatic installer workflow](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/automatic-installer-workflow.md).

## Installation

Download **CrimsonSkiesFix-Windows-x86-1.0** from a successful [Windows x86 GitHub Actions build](https://github.com/legluondunet/CrimsonSkiesFix/actions). The archive contains **one file**, `CrimsonSkiesFix.exe`. This launcher embeds the already verified Windows x86 `ddraw.dll`; no separate DLL download is published.

For a fresh install, put an authentic supported ISO or CUE/BIN, the official 1.02 update and your own approved decrypted 1.02 EXE beside `CrimsonSkiesFix.exe`, then run it. Existing verified installations can launch without original media. The launcher installs the proxy beside `CRIMSON.EXE` and writes its documentation and template into the game directory. It never packages copyrighted original game files.

After installation, the relevant files have this layout:

```text
CrimsonSkiesFix.exe                 # standalone installer and launcher
CrimsonSkiesFix/
├── CRIMSON.EXE                     # validated user-provided 1.02 EXE
├── ddraw.dll                       # extracted from the launcher
├── GOSDATA/ASSETS/                 # user's original ROF assets
├── CrimsonSkiesFix.ini.example    # generated template, not active
├── CrimsonSkiesFix.ini            # generated on first game launch
├── README.md
├── LICENSE
└── build-info.txt                 # exact source and installed DLL digest
```

**Default-on behavior in 1.0:** a fresh install suppresses the obsolete startup driver/resolution dialog, uses the native 3D adapter and GOS Graphics/Resolution lock, enables the first-image RGB565 color correction, activates focus repaint plus
the background-erase guard, and enables the Wine-only ROFFILE.DLL EOF guard
(subject to original DLL fingerprint verification). The **first game launch modifies only the small original** `GOSDATA/ASSETS/crimptch.rof` archive, after an exclusive backup. To opt out of the backed-up GOS overlay **before first launch**, create `CrimsonSkiesFix.ini` in the installed game directory yourself and set `[Graphics] GosVideoLock=off`. All other required graphics fixes are enabled without any INI switches. The `.ini.example` file alone is not the active configuration. Existing user INIs always take precedence and are never overwritten.

The DLL must be **next to the game executable**, not inside a separate configuration subfolder or a system directory. The supplied `.ini.example` is not loaded; edit the generated `CrimsonSkiesFix.ini` in the installed game directory after launching once. Existing configurations are never overwritten by an update. Older `CrimsonFix` INI paths are imported only if the current INI does not already exist, without deleting the originals.

Wine users may need `WINEDLLOVERRIDES="ddraw=n,b"` to force the native proxy. The same Windows x86 DLL is used under Wine/Proton; it is **not** a Linux shared object.

## Verify the DLL installed by the launcher

The standalone launcher installs `build-info.txt` beside the game EXE with the exact
Git source commit, the expected version/short-SHA text logged at startup,
and the SHA-256 and size of the corresponding `ddraw.dll`. This prevents
confusing an older installed DLL with the one embedded in the current launcher.
It is **not** a cryptographic signature by the project author.

Under PowerShell, check the SHA-256 of the installed proxy:

```powershell
(Get-FileHash .\ddraw.dll -Algorithm SHA256).Hash
Get-Content .\build-info.txt
```

Compare the computed hash with `DllSha256` and find the game's
initialization line in the **DLL-only** `CrimsonSkiesFix.log`; it begins
with `LogIdentity` and ends with ` - init; ...`. The standalone installer
writes its independent report to `CrimsonSkiesFix_installer.log`, not to
the beginning of the game log. The `Commit` and `Source` fields identify the exact
repository state. The distributed `LICENSE` is copied verbatim from
the repository and should be preserved when sharing the proxy DLL.
The standalone artifact itself contains one EXE; these files are created inside the game directory.

## Settings

CrimsonSkiesFix automatically applies its verified compatibility fixes to
supported game files. **The generated INI is intentionally minimal**:
borderless fullscreen, desktop resolution, 3D widescreen FOV/HUD, the startup
chooser bypass, first-image color correction, focus recovery, Wine-only ROFFILE
protection, Windows-native compatibility fixes and GOS Graphics/Resolution
lock are already active when the exact game/platform preflight succeeds.
There are **no mandatory fix switches to enable**.

The first-run `CrimsonSkiesFix.ini` and shipped `.ini.example` now contain three user-facing settings:

```ini
; CrimsonSkiesFix — optional user preferences (created once).
; Verified game compatibility fixes are built in and enabled automatically.
; Advanced diagnostics and GOS restoration are documented in README.md.
[Graphics]
; Game aspect ratio: auto (desktop) or 4:3 (centered pillarbox).
AspectRatio=auto
; Frame pacing target, 1-60 frames per second.
FPSLimit=60

[Debug]
; Additional game diagnostics (0=off, 1=on).
Log=0
```

`AspectRatio=auto` fills the desktop with native-resolution 3D at the monitor's
aspect ratio (the existing behavior). Set `AspectRatio=4:3` to render the **entire
game in true 4:3**: widescreen displays show centered 3D with black bars on
either side, matching the existing 4:3 menus/videos without stretching.
At 1920×1080, 3D renders at **1440×1080** with **240-pixel sidebars**.
The 3D camera, FOV and HUD are patched to that 4:3 internal resolution,
not squeezed from an already-wide scene. The game remains in borderless
fullscreen, and the system display mode is unchanged. On monitors too small
to support the verified 3D source, the option fails closed rather than
silently switching the physical display mode.

`FPSLimit` can be set from 1 to 60; `Log=1` enables detailed diagnostics.
For specialized investigations, advanced optional legacy/diagnostic switches
(for example `[Graphics] ModernFPSPacing=1`, `[Debug]
FrameTimeTrace=1`, or `[Debug] CrashBltTrace=1`) remain accepted, but are
not included in a normal first-run INI. They are not prerequisites for
correct operation. Existing INIs are **never silently rewritten** and known
legacy overrides are still read for compatibility. Delete or edit a
previously disabled override deliberately if you want its default-on fix.
A future cleanup can retire legacy overrides separately, after compatibility
testing.

The native GOS lock is default-on, but a deliberate `[Graphics]
GosVideoLock=restore` operation remains supported so an authenticated
original overlay can be restored safely. For an initial opt-out or to keep
the restored original behavior, `GosVideoLock=off` remains supported.
These maintenance values are intentionally **not written into the generated
INI**.

**Built-in window visibility repairs:** focus return requests a bounded
full repaint, and the verified game HWND ignores `WM_ERASEBKGND` erasure,
avoiding white backgrounds. Neither is a general crash handler.

### GOS Video dropdown lock, backup and restoration

The experimental `GosVideoLock=on` path identifies exactly three complete `crimson.rof` archives using the installer's original-CD SHA-256 profiles: USA (60,236,221 bytes, SHA-256 `acc9946874110e9741183384010bc02fd48923ae3d608fcf02a96498c3731174`), FR (62,745,707 bytes, SHA-256 `99074dbc39e27e65b6b49ea338fc501d747caa0ac97062f931ee6abdc9e7350f`), and DE (62,678,770 bytes, SHA-256 `4ccc5d25c2635ea9ad042ff1e356408d504e4c616389084cb087b49ddb28cbc7`). The USA and FR copies contain the same independently verified `VIDEO.SCRIPT` (SHA-256 `f0b0ae62dc582a55dfc6eaf4a8b2937ef2084a03596b8fa52542a5de4512afa9`). **DE was subsequently validated in-game by the owner under GE-Proton**, including the authenticated in-memory GOS lock. Accepting the whole-archive hash alone is not considered sufficient: The runtime must also authenticate the extracted script's exact hash and length. If DE has a different script, the lock fails closed; other editions or modified archives are never guessed from their country names.


On **first launch by default**, `GosVideoLock=on` makes the DLL verify the accepted executable and the original `crimson.rof` and `VIDEO.SCRIPT`, then create a small overlay containing the unchanged original `AIRFRAME.SCRIPT` patch and an edited, owner-derived `VIDEO.SCRIPT`. It modifies **only** the installed `GOSDATA/ASSETS/crimptch.rof`, saving a byte-for-byte original as:

```text
GOSDATA/ASSETS/crimptch.rof.CrimsonSkiesFix.original.bak
```

The game engine may ignore the `VIDEO.SCRIPT` member placed in the small overlay archive. In the current experimental workaround, the SHA-256-verified `ROFFILE.DLL` read of the **original `CRIMSON.ROF`** receives the corrected, user-derived script **only in memory**. The parsed ROF member descriptor supplies its exact offset without a locale-specific constant; a standards-compliant zlib stream is regenerated to match that member's original compressed byte length, preserving every other member and file position. This needs the authentic original archive, the exact 797-byte official patch backup, and the managed modified overlay; any mismatch disables virtualization. The actual original `CRIMSON.ROF`, executable and DLL are not modified. A repeat launch recognizes the existing overlay and does not rewrite it. No original game script is distributed in the project. This method is **owner-validated under GE-Proton for FR and DE**, including FR with `Log=1` and `Log=0`; a real USA/Europe CUE/BIN has passed strict manifest inspection and installation, although USA game behavior and native Windows gameplay remain unvalidated.

| Setting | Effect |
|---|---|
| `GosVideoLock=off` | Do nothing to game archives; **does not remove** an existing patch |
| `GosVideoLock=on` | Install or recognize the native disabled-state overlay, with a verified backup |
| `GosVideoLock=restore` | Restore the original `crimptch.rof` from backup at the next launch, then consume the backup |

After restoring, change the setting back to `off` (or `on` to install it again). **Always use `restore` before removing the DLL if you want the original unmodified GOS Video screen.** The override is persistent across game exits and DLL removal.

Historical GOS overlay installation, idempotence and restoration were validated under Wine/GE-Proton in early November 2026. Earlier native Windows tests used a different overlay-routing path; those tests do not validate this new read-buffer virtualization. The owner confirmed the native disabled **Graphics**/**Resolution** controls with the in-memory method on the original FR edition under GE-Proton on 12 November 2026, with and without diagnostic logging. USA media can now be tested in-game (inspection and installation have passed); DE owner tests succeeded. Native Windows acceptance of the current candidate remains outstanding. For other media editions, the installer must supply only the exactly hashed original assets and official 1.02 update; fail-closed checks must not be bypassed.

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

When ROFFILE.DLL is loaded through the authenticated game's observed
`LoadLibrary` import, the Wine EOF repair is now attempted immediately
**after** the loader returns and **before** original ROF archive observation.
Graphics-frame probing remains a fallback for other loading paths. A DLL
loaded and parsed through an unobserved path can still escape the repair;
an absent `ROFFILE-FIX: installed` log means activation was not confirmed.
The hook remains restricted to the exact supported ROFFILE.DLL SHA-256
and known instruction signatures. Unrecognized editions
are not modified. Tests under native Windows and additional long Wine sessions
remain desirable; no assumption of identical behavior on every computer
is made.

To turn the fix off on the next launch, set
`RoffileTerminatorFix=0`. This does not require restoring any original
game file because the patch never writes to disk.

## Native Windows automatic compatibility

For a **new installation**, seven native-Windows-only
compatibility features are **enabled in the compiled defaults**, not listed
in the generated INI: `NativeDD4Bootstrap`,
`NativeMovieRGB32`, `NativeMovieMessagePump`, `NativeFullFrameCompose`,
`NativeMouseClipRepair`, `NativeDirectInputMouseCoopRepair` and
`NativeGosCursorBridge`. These keep their strict exact-game and runtime
preflight guards and do **not** run under Wine/Proton. They no longer require
`[Debug] Log=1` just to work. Optional mouse tracing, screen cursor
probes, frame-time profiling and other debug diagnostics stay disabled.

Existing user INI files are **not overwritten or silently migrated**; previous
explicit `=0` legacy overrides remain respected until they are deliberately
removed. Under native Windows, advanced debug overrides can still isolate
hardware/driver regressions, but should never be required for first-run
operation. The initial Windows owner validation used an explicit test
configuration; a **new default-install Windows 11 runtime smoke test** is still
needed before a public stable release. The Wine/GE-Proton path continues to use RGB565 game presentation.
**Wine-only native Blt safety** from v0.1.55 now installs its minimal
Surface4 QueryInterface/GetAttachedSurface/Blt guards whenever the verified
virtual windowed adapter is enabled, even with `[Debug] Log=0` and
`[Graphics] ModernFPSPacing=0`. It unwraps authenticated synthetic
surface sources before passing them to Wine's native DirectDraw Blt; it does
not enable opt-in tracing, extra Flip interception, or FPS pacing. Previously
this guard was accidentally tied to the logging/pacing switches, so the
default INI could leave it inactive. A native Windows run with `Log=0`
retains its previously validated hooks. A legacy `ExperimentalWindowedAdapter=0` override
intentionally disables the virtual graph and therefore does not exercise
this specific source-unwrapping protection. This change requires owner
runtime verification at the original reproducible Wine crash location.

## Logging and diagnostics

**Two standard operational logs plus a dedicated inspection report:** `CrimsonSkiesFix_installer.log` beside the launcher holds the latest installation, repair or error report; verified launches leave it unchanged, avoiding the loss of a previous error. `--inspect` writes `CrimsonSkiesFix_inspect.log` independently, preserving the installer report. `CrimsonSkiesFix.log`, also beside the launcher, belongs to the game's DirectDraw DLL and is **replaced at each game session** regardless of whether the game is launched by the standalone EXE or directly. A standalone-independent DLL installation writes the game log beside its own `ddraw.dll`. The launcher no longer supplies a process handoff marker or appends DLL diagnostics into the installer report. Set `[Debug] Log=1` for additional game diagnostics. Rejected executables can be logged even with `Log=0` when the directory is writable. An optional `CrashBltTrace=1` creates a separate bounded rolling crash trace only when explicitly enabled.

The initialization report provides privacy-filtered Windows/Wine, CPU, display, graphics and controller information. Optional `[Debug]` flags include `SurfaceUsageTrace`, `CrashBltTrace` and `FrameTimeTrace`, all disabled by default. Retired `NativeIntroExclusive`, `NativeGdiSidebars`, `ColorProbe` and `ColorCapture` INI keys are ignored by this branch; no BMP files are created. Crash Blt tracing is intentionally verbose and writes a separate bounded rotating trace; disable it again after diagnosis.

For an opt-in check of frame cadence, enable **both** `[Debug] Log=1` and `[Debug] FrameTimeTrace=1`, then fly for at least one minute. Each `FRAMETIME:` record summarizes 120 successful virtual Surface4 Flip completion intervals (average, median, p95, p99, worst, >2×/>3× target, pauses and presentation-call latency). It does **not** measure when the GPU or desktop compositor physically displays a frame; use an external display-time capture to investigate perceived stutter. The GE-Proton11-12 owner test at 1920×1080 / 60 FPS found only one >33.3ms interval in 5,520 sampled gameplay intervals; menu/transition samples were more variable. This measurement alone does not justify changing the default FPS limiter. Disable `FrameTimeTrace` after diagnosis.

Useful GOS log markers: `GOS-VIDEO-LOCK:`, `MENU-RESOLUTION-LABEL:`, `WINDOWED-WIDE:`, `MENU-BRANDING:`. If a new build fails, restore the previous `ddraw.dll` and relevant optional INI settings; for an already-installed GOS overlay use the explicit `restore` setting first.

## Validate that the game exits normally

A successful graphics session does not establish that Crimson Skies shuts
down cleanly. The DLL intentionally performs **no file I/O under the loader
lock in `DllMain`**. The last DirectDraw DLL line alone is not an exit
status indicator. On a launch that performs installation or repair, the
parent records `GAME_EXIT_CODE=...` in `CrimsonSkiesFix_installer.log` after
waiting for the game to exit; on subsequent verified-only launches, it
prints the exit code to the console without overwriting the retained
installer report. Use the launcher's exit status or an external observer
for a definitive clean-exit check.

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
[exit validation](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/exit-validation-2026-11-04.md). A **real GE-Proton 11-12 gameplay session** on `0.1.90-f2b2ca42` has now been observed to end with `0` in Lutris/umu, with no unhandled exception reported; this successful session did **not** run the Python observer. Native Windows 11 game exit remains untested. Automated synthetic process tests do not replace actual gameplay verification.

## Local Windows x86 build without GitHub Actions

**Manjaro alternative:** [build the exact Windows x86 MSVC DLL from Linux](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/manjaro-cross-build.md)
with `tools/build_linux_windows_x86.sh` and `cargo-xwin`. The script verifies
all 22 native DDRAW exports and packages a source-hashed candidate without
installing Visual Studio or modifying the native Windows 11 test machine.
Successful cross-compilation alone is **not** Windows gameplay validation.

As of 7 November 2026, the owner's **private-repository** GitHub Actions allowance is exhausted, while public SpiderMan2k1Fix still runs free standard hosted runners:
the jobs are blocked **before any compilation step**. Do not interpret the
failed Actions status as evidence that the proposed Rust 22-export fix failed
to compile. Use the [local Windows build and acceptance protocol](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/windows-local-build.md)
on a Windows MSVC developer environment. The source includes
`tools/build_windows_local.ps1`, which builds the 32-bit DLL, runs tests and
both 22-export name/ordinal and loader checks, and creates a hash/provenance
verified candidate ZIP without touching game files. The resulting DLL is
**not Windows-gameplay-validated** until the owner tests that exact candidate.

## Build and tests

```powershell
rustup target add i686-pc-windows-msvc
cd CrimsonSkiesFix
cargo test --locked --target i686-pc-windows-msvc
cargo build --locked --release --target i686-pc-windows-msvc
```

The repository commits `Cargo.lock` to pin the tested `flate2` dependency tree.

The [x86 workflow](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/.github/workflows/crimsonskiesfix-x86.yml) builds `ddraw.dll`, verifies the PE machine type and all 22 canonical DirectDraw names/ordinals, runs Rust and safe ROF/Python synthetic tests, performs a native Windows x86 DLL load/export/unload smoke check, and packages the game layout with the GPL license and build provenance manifest. `windowed_ddraw_probe.exe` is an internal experimental DirectDraw diagnostic; its build and tests remain part of CI, but it is no longer published as an artifact and must not be placed in the game directory.

For original GameOS Video UI research and owner test evidence, see [Video Preferences native UI](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/video-preferences-native-ui.md) and [GOS archive inspector](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/tools/inspect_gos_rof.py). Earlier experiments and detailed changelogs remain available in the repository's Git history.

## Known limitations

This is still an experimental, **exact-executable** build, not a validated public release. Native Windows 11 was runtime-tested with explicit opt-in settings (including `NativeDD4Bootstrap`, `NativeMovieRGB32`, `NativeFullFrameCompose`, DirectInput mouse handling and the window message pump); the combined **fresh-install defaults have not yet been validated under native Windows**. GE-Proton11-12 with Wine 11.0 has been user-tested, but a distinct Wine-staging 11.17 run and additional hardware/driver combinations remain untested in PR #23. Automatic test success does not prove correct real-game start, focus/Alt+Tab, or clean process exit. The Wine-only ROFFILE.DLL parser guard is intentionally disabled on native Windows. FPS pacing hooks alone do not establish gameplay FPS. Enabling GOS Video lock modifies the user-owned `crimptch.rof` and is **not automatically undone** when the game exits.

## License and contribution policy

GPL-3.0-or-later. Changes are prepared through GitHub pull requests; merging remains at the owner's discretion.

legluondunet assisted by ChatGPT (OpenAI)
