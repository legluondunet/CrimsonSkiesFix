# CrimsonSkiesFix

**Crimson Skies (2000) on modern Windows and Wine/Proton.**

CrimsonSkiesFix is a standalone Windows x86 installer, launcher and open-source compatibility patch for the original PC game. A single `CrimsonSkiesFix.exe` prepares the game, installs the integrated compatibility patch and launches Crimson Skies 1.02.

**Development documentation for 1.1 (not yet released).** The current stable release is [1.0](https://github.com/legluondunet/CrimsonSkiesFix/releases/tag/v1.0), which has different installation requirements; see its [version-specific README](https://github.com/legluondunet/CrimsonSkiesFix/blob/v1.0/README.md).

[Project downloads](https://github.com/legluondunet/CrimsonSkiesFix/releases) · [Report a problem](https://github.com/legluondunet/CrimsonSkiesFix/issues)

## Main features

- **Automatic installation and repair** from authenticated original CD images (USA/Europe, French and German editions) and the official 1.02 update.
- **No disc required to play** once installed. Verified installations launch again without re-extracting the original CD.
- **Borderless fullscreen** at desktop resolution, widescreen 3D with adapted field of view (FOV) and HUD.
- **Original 4:3 menus and videos**, with an optional 4:3 gameplay mode and black sidebars.
- **FPS limiting** (60 FPS by default), focus/Alt-Tab handling and display startup fixes.
- **Simplified video options**, removing the old graphics selector and preventing incompatible in-game Graphics/Resolution changes.
- **Integrated compatibility DLL**, version signature in the main menu and separate installer/game logs.
- **Common Windows/Wine implementation**, including a targeted Wine-only ROFFILE.DLL compatibility fix.

## Files you must provide

Put `CrimsonSkiesFix.exe` and **one supported original game CD image** in the same writable directory. Use an ISO, or a CUE together with all of its referenced BIN/IMG tracks.

### Supported CD editions

| Original CD | Internal profile | Official 1.02 update required? |
| --- | --- | --- |
| English (USA/Europe), 1.0 | `C_SKIES_DISC_001` | Yes |
| German, 1.0 | `C_SKIES_DE_001` | Yes |
| French, 1.0 | `C_SKIES_FR_002` | Yes |
| French, 1.02 | `C_SKIES_FR_001` | No |
| English (Europe), 1.02 | `C_SKIES_DISC_002` | No |

The installer identifies supported media by the exact paths, sizes and SHA-256 hashes of the files inside the CD, not the image filename. **You do not need to determine your CD version yourself.** If the update is required, the launcher identifies your edition and explains what to do in the terminal and installer log. No separately supplied decrypted game executable is required in 1.1.

### Official 1.02 update

CrimsonSkiesFix targets the final official PC game version, **1.02**, which includes important stability, performance and save-game fixes. An original version 1.0 CD is supported, but it does not contain all of the required 1.02 game data. Without the authentic update file, the launcher cannot complete installation or start the game.

If the terminal or desktop error dialog says that the 1.02 update is required:

1. Download the official Microsoft 1.02 update (`crmsk102.exe`) from **one** of these sources:
   - [Project Releases](https://github.com/legluondunet/CrimsonSkiesFix/releases) (if listed there).
   - [MyLittleLutrisScripts — crmsk102.7z](https://github.com/legluondunet/MyLittleLutrisScripts/raw/refs/heads/master/Crimson%20Skies/crmsk102.7z).
   - [PCGamingWiki — official Crimson Skies patch](https://community.pcgamingwiki.com/files/file/62-crimson-skies-patch/).
   - [ModDB — Crimson Skies Update v1.02](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102).
   - [My Abandonware](https://www.myabandonware.com/game/crimson-skies-be7) — under **Extras**, select **International patch v1.02**, not the game download or other fixes.
2. If the download is a 7z or ZIP archive, extract `crmsk102.exe` first. Place exactly one authentic copy of the extracted EXE **beside `CrimsonSkiesFix.exe`**, not inside `game/`.
3. **Do not run the Microsoft updater.** Run `CrimsonSkiesFix.exe` again; it verifies the file and finishes installation automatically.

The launcher accepts the update by content, even if its filename has changed. It does not download the update or run its historical setup program. Installation is offline once the required files are present.

- Expected size: `2,315,728` bytes
- SHA-256: `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af`

If a copy is already present but rejected, check the size and SHA-256; if multiple valid copies are detected, keep only one beside the launcher.

### If the launcher cannot start the game

CrimsonSkiesFix displays blocking launcher errors both in the terminal and in a native Windows error dialog, whether launched from the desktop or a terminal, with or without command-line arguments. For a recognized version 1.0 CD with a missing or invalid update, choose **Yes** to open the download instructions in your default browser. Choose **No** to close the message without opening anything. If multiple verified copies are present, keep only one. Missing or multiple CD images get short, actionable messages. Other blocking installer errors show an explanation. Successful installations show no dialog.

The full error is still written to the terminal when available, and installation errors are recorded in `CrimsonSkiesFix_installer.log` beside the launcher when logging is possible. Only CI and the explicit unattended mode suppress error dialogs. For scripts or unattended operation, set `CRIMSONSKIESFIX_NONINTERACTIVE=1` to keep the console diagnostics without blocking on a dialog.

### Example folder before installing

```text
CrimsonSkies/
├── CrimsonSkiesFix.exe
├── original-disc.cue
├── original-disc.bin
└── crmsk102.exe         # Only required for original version 1.0 CDs
```

A supported ISO can replace the CUE/BIN pair. The CUE must reference the actual track filenames. For the supported 1.02 CDs, omit the update file. Keep only one ISO or CUE beside the launcher.

## Install on Windows

1. Obtain the 1.1 launcher from the [project downloads page](https://github.com/legluondunet/CrimsonSkiesFix/releases) when released; these instructions do not apply to the existing 1.0 launcher.
2. Create a writable directory, such as `C:\Games\CrimsonSkies\`. Put the launcher and the required external files there.
3. Run `CrimsonSkiesFix.exe`. If an update is required, follow the instructions in the terminal. It authenticates the sources, installs the game and integrated patch, and launches Crimson Skies.
4. For future sessions, run **the same launcher**. The CD image and updater are no longer needed for an installation that verifies successfully; keep them available in case a full repair becomes necessary.

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
2. Put `CrimsonSkiesFix.exe`, the original CD image, and the official update if required, in a writable folder such as `~/Games/CrimsonSkies/`.
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

The game is installed in a new `game/` folder next to the launcher. Always start the game through the launcher, not directly through `game/CRIMSON.EXE`.

```text
CrimsonSkies/
├── CrimsonSkiesFix.exe
├── CrimsonSkiesFix.ini
├── CrimsonSkiesFix_installer.log
├── CrimsonSkiesFix.log
├── docs/
├── state/
└── game/
    ├── CRIMSON.EXE
    ├── fix.dll
    ├── GOSDATA/
    ├── Planes/
    └── SavedGames/
```

Use a separate directory for a fresh 1.1 installation rather than expecting automatic migration of an older layout.

The protected CD executable is preserved as `CRIMSON.EXE.protected.bak`. The repair workflow preserves player saves, custom planes and the active INI, and retains the former installation as a backup.

The generated `CrimsonSkiesFix.ini` contains these basic settings:

```ini
[Graphics]
AspectRatio=auto
FPSLimit=60

[Debug]
Log=0
```

Set `AspectRatio=4:3` for 4:3 gameplay with black sidebars. An existing custom INI is not overwritten.

The installer-managed `docs/build-info.txt` records the build version, source commit, bundled DLL SHA-256 and (in the five-edition candidate) the two Cargo lockfile SHA-256 hashes and third-party notice SHA-256. Verified launches restore missing or changed launcher-managed notices without requiring the CD. Existing player configuration and matching documents are left unchanged. Candidate packages include exact source and build-recipe references in `SOURCE-INFO.txt`.

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

CrimsonSkiesFix is licensed under **GPL-3.0-or-later** ([source license](https://github.com/legluondunet/CrimsonSkiesFix/blob/v1.0/LICENSE)). The GPL covers the project code, not Microsoft game files. Candidate CI collects dependency license texts into `THIRD-PARTY-NOTICES.txt`, also installed under `docs/`. Local developer builds without generated notices are not distribution-ready. The launcher does not bundle the original game, CD images, the official updater or a decrypted game executable. The official update remains a separate Microsoft file; hosting it separately requires applicable redistribution permission.

legluondunet assisted by ChatGPT (OpenAI)
