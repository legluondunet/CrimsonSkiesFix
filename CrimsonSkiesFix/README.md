# CrimsonSkiesFix

**Crimson Skies (2000) on modern Windows and Wine/Proton.**

CrimsonSkiesFix 1.0 is a standalone Windows x86 installer, launcher and open-source DirectDraw compatibility patch for the original PC game. A single `CrimsonSkiesFix.exe` prepares the game, applies the official 1.02 data update, installs the integrated `ddraw.dll` patch and launches Crimson Skies.

**[Download CrimsonSkiesFix 1.0](https://github.com/legluondunet/CrimsonSkiesFix/releases/tag/v1.0)** · [Report a problem](https://github.com/legluondunet/CrimsonSkiesFix/issues)

## Main features

- **Automatic installation and repair** from authenticated original CD images (USA/Europe, French and German editions) and the official 1.02 update.
- **No disc required to play** once installed. Verified installations launch again without re-extracting the original CD.
- **Borderless fullscreen** at desktop resolution, widescreen 3D with adapted field of view (FOV) and HUD.
- **Original 4:3 menus and videos**, with an optional 4:3 gameplay mode and black sidebars.
- **FPS limiting** (60 FPS by default), focus/Alt-Tab handling and display startup fixes.
- **Simplified video options**, removing the old graphics selector and preventing incompatible in-game Graphics/Resolution changes.
- **Integrated DirectDraw proxy**, version signature in the main menu and separate installer/game logs.
- **Common Windows/Wine implementation**, including a targeted Wine-only ROFFILE.DLL compatibility fix.

## 1.1 development candidate (not released)

The experimental **WinMM-based 1.1** branch is being consolidated and tested
on Windows 11 and Wine/Proton. The **v1.0 download above remains the published
stable release**; the development candidate is not a v1.1 release yet.

- The 1.1 launcher installs its integrated **`winmm.dll`** compatibility
  runtime, rather than the `ddraw.dll` used by the 1.0 release.
- **Widescreen FOV/camera correction is automatic** for verified native
  3D gameplay. It preserves the horizontal field of view, uses the actual
  monitor height/width ratio before software projection, and returns to
  the game's original 4:3 camera values for menus and movies. No special
  debug option or logging is required.
- With `AspectRatio=4:3`, 3D is rendered at the largest suitable native
  4:3 raster (for example **1600×1200** on a **1920×1200** display),
  not upscaled from the game's original 640×480 mode.
- The **Graphics → Resolution** label reports the actual verified
  **3D rendering resolution** (for example 1600×1200 in forced 4:3,
  1920×1200 in 16:10, or 1920×1080 in 16:9). This display-only change
  does **not** alter the original GOS mode table or the 800×600
  menu/movie surfaces.
- Obsolete 1.1 test INI keys such as `ExperimentalProjectedY43Probe`,
  `ExperimentalY43ReticleGuard`, and `ExperimentalUpstreamAspectProbe`
  are ignored; **existing INI files are preserved**. Keep
  `[Graphics] AspectRatio=auto` (or `4:3`) and use
  `[Debug] Log=1` only when a diagnostic log is needed.

These features have undergone native Windows and Proton field testing,
but the current branch still requires a final regression check before
merging and publishing 1.1.

## Files you must provide (not included)

For a **new installation** or full repair, put all three items below **beside `CrimsonSkiesFix.exe`**:

| Required item | What you need |
| --- | --- |
| **Original game CD image** | Exactly one authentic supported `.iso`, or one `.cue` together with its referenced `.bin`/`.img` track file(s). Supported CD editions: USA/Europe, FR and DE. |
| **Official Crimson Skies 1.02 update** | The original `crmsk102.exe`, [archived on ModDB](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102). The installer verifies and extracts its game data without running the updater. |
| **Approved SafeDisc-free 1.02 executable** | Your own decrypted, unmodified `CRIMSON.EXE` version 1.0.2.0. **CrimsonSkiesFix does not decrypt, generate or supply this file.** |

Only the exact supported files are accepted. The updater and approved executable may be renamed, as they are identified by their contents, but keep just **one valid copy of each** beside the launcher.

**Required decrypted game executable:**
- Size: `2,580,480` bytes
- SHA-256: `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`

**Required official 1.02 updater:**
- Size: `2,315,728` bytes
- SHA-256: `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af`

The original CD's SafeDisc-protected executable, other game versions and third-party modified or widescreen executables are not accepted as the playable EXE. The official update itself does **not** remove SafeDisc.

### Example folder before installing

```text
CrimsonSkies/
├── CrimsonSkiesFix.exe          # Downloaded from the 1.0 release
├── original-disc.cue            # Or a supported .iso instead
├── original-disc.bin            # Required if referenced by the CUE
├── crmsk102.exe                 # Official 1.02 updater
└── CRIMSON-UNWRAPPED.EXE        # Your matching decrypted 1.02 EXE
```

Names are illustrative. A CUE file must reference the **actual filename** of its BIN/IMG track file. Keep only one ISO or CUE beside the launcher for automatic detection.

## Install on Windows

1. Download **[CrimsonSkiesFix.exe](https://github.com/legluondunet/CrimsonSkiesFix/releases/download/v1.0/CrimsonSkiesFix.exe)** from the version 1.0 release.
2. Create a writable directory, such as `C:\Games\CrimsonSkies\`. Put the launcher and the required external files there.
3. Double-click `CrimsonSkiesFix.exe`. It authenticates the original sources, installs the game and its DirectDraw patch, and launches Crimson Skies.
4. For future sessions, run **the same launcher**. The CD image, updater and external decrypted EXE are no longer needed for an installation that verifies successfully; keep them available in case a full repair becomes necessary.

To **install without launching** the game, open PowerShell in the launcher directory:

```powershell
.\CrimsonSkiesFix.exe --install-only
```

You do **not** need to run the original game `setup.exe` or download a separate `ddraw.dll`.

## Install on Linux with Wine or Proton

### Recommended compatibility layer

For Linux, use **one** of the following with **32-bit Windows application support**:

- **Wine Staging 11.19 or newer** (minimum recommended Wine Staging version).
- **GE-Proton** (through a compatible launcher such as Lutris).
- **Valve Proton** (through Steam or a compatible Proton launcher).

The older **Wine Staging 11.17** was observed to play the two introductory
movies but then stall with a black main menu when it tried to initialize
the `CRIMFLAG.MPG` menu-background video. In November 2026, the same
CrimsonSkiesFix development build under **Wine Staging 11.19** reached
`MainMenu`, continued presenting frames and exited cleanly with
the experimental `CRIMFLAG.MPG` bypass **disabled**. This is consistent
with an upstream AMStream synchronization fix added in Wine 11.18; it
does **not** establish that every later runtime or every machine is
automatically compatible.

GE-Proton has also been used for real-game tests. **Valve Proton is a
recommended option, not a claim that every Valve Proton version has
been individually tested with this build.** No particular Proton
version is pinned. Keep your selected Wine/Proton runner and prefix
consistent between installation and play.

**CrimsonSkiesFix does not bypass or disable the menu-background movie
by default, and does not require the experimental PR #51 CRIMFLAG hook.**
No additional Winetricks step is included in the standard instructions.
If you experience a black menu, confirm your actual Wine/Proton runner
version and check its 32-bit multimedia dependencies before reporting it.

1. Select **Wine Staging 11.19+**, **GE-Proton**, or **Valve Proton**, with 32-bit Windows application support.
2. Put the downloaded `CrimsonSkiesFix.exe` and the same three external items in a writable folder such as `~/Games/CrimsonSkies/`.
3. Open a terminal and run:

```bash
cd ~/Games/CrimsonSkies
wine CrimsonSkiesFix.exe
```

The same automatic installer runs under Wine. To install **without starting the game**:

```bash
wine CrimsonSkiesFix.exe --install-only
```

When using **Lutris, Proton or a custom Wine prefix**, launch `CrimsonSkiesFix.exe` in that environment and keep using **the same prefix** for subsequent launches. The launcher is a Windows x86 executable; there is no separate Linux binary.

## After installation

The game is installed in a new `CrimsonSkiesFix/` folder **next to the launcher**:

```text
CrimsonSkies/
├── CrimsonSkiesFix.exe
├── CrimsonSkiesFix_installer.log
├── CrimsonSkiesFix.log
└── CrimsonSkiesFix/
    ├── CRIMSON.EXE              # Approved game executable
    ├── ddraw.dll                # Integrated patch installed automatically
    ├── GOSDATA/                 # Original game assets
    └── CrimsonSkiesFix.ini      # Created on first game launch
```

The protected CD executable is preserved as `CRIMSON.EXE.protected.bak`. The repair workflow preserves player saves, custom planes and the active INI, and retains the former installation as a backup.

The generated `CrimsonSkiesFix.ini` contains these basic settings:

```ini
[Graphics]
AspectRatio=auto
FPSLimit=60

[Features]
SmoothMenuCursor=0

[Debug]
Log=0
```

Set `AspectRatio=4:3` for 4:3 gameplay with black sidebars.

**`[Features] SmoothMenuCursor=0`** (default) keeps the game's original
menu cursor and click handling. Set **`SmoothMenuCursor=1`** to opt into
the smoother **original gold menu cursor under Wine**. The fix derives a
Win32 `HCURSOR` from the installed game's own cursor artwork, hides only
duplicate game-drawn cursor sprites, and synchronizes GameOS hit-testing
coordinates so buttons still work after returning from gameplay.
The feature does **not** replace the gold cursor with a generic Windows
arrow and does not alter native Windows behavior. All dependent Wine
cursor hooks are enabled automatically; **`[Debug] Log=1` is not required**.

A user-selected `SmoothMenuCursor=0` or `1` overrides the older separate
Wine cursor `[Debug]` A/B switches regardless of section order. Existing
custom INIs are **never overwritten**: add the `[Features]` section manually
to an older INI to use this toggle. Restart the game to apply a change.

## Commands

| Command | Description |
| --- | --- |
| `CrimsonSkiesFix.exe` | Verify, install/repair if necessary, then launch |
| `--install-only` | Install/repair and verify without launching |
| `--verify` | Check the installed game and DirectDraw patch |
| `--launch-no-verify` | Launch directly, **without integrity checks or repair** |
| `--inspect image.iso` | Inspect an original or unsupported ISO |
| `--inspect image.cue [image.bin\|image.img]` | Inspect a CUE image; optional data-track override |
| `--verbose` | Extra diagnostics, combinable with a command |
| `--version`, `--help`, `-h` | Version and usage information |

**Logs** are written next to the launcher: `CrimsonSkiesFix_installer.log` (last meaningful installation/repair/error), `CrimsonSkiesFix.log` (new game log for each session), and `CrimsonSkiesFix_inspect.log` (generated by `--inspect`).

## Compatibility

CrimsonSkiesFix is designed for the **original Crimson Skies (2000) PC game** on Windows and Wine/Proton. The Xbox edition and unrelated re-releases are not supported.

## License

CrimsonSkiesFix is licensed under **GPL-3.0-or-later** ([source license](https://github.com/legluondunet/CrimsonSkiesFix/blob/v1.0/LICENSE)). It does **not** distribute the original game, CD images, official updater or decrypted game executable.

legluondunet assisted by ChatGPT (OpenAI)
