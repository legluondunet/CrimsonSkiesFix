# Standalone launcher bootstrap (experimental)

The Windows x86 release workflow now also builds a **single-file
CrimsonSkiesFix.exe**, separately from the existing five-file DirectDraw
proxy package. This is an early installer/launcher PR, not the 1.0 release.

## Usage and layout

Place `CrimsonSkiesFix.exe` beside **one complete, already installed**
Crimson Skies directory containing the **exact hash-approved unwrapped 1.02**
`CRIMSON.EXE` and `GOSDATA/ASSETS/` archives:

```text
CrimsonSkiesFix.exe
Crimson Skies/        # pre-existing game directory: copied, never altered
    CRIMSON.EXE
    GOSDATA/ASSETS/crimson.rof
    GOSDATA/ASSETS/crimptch.rof
```

On startup, the launcher verifies the original EXE SHA-256 and required ROF
files, copies everything through `CrimsonSkiesFix.installing/`, inserts its
embedded and SHA-256-verified `ddraw.dll`, and publishes the game in
`CrimsonSkiesFix/`. It then verifies and launches it with that directory as
its working directory. Subsequent starts verify/update the patch and launch
without copying game data again.

```text
CrimsonSkiesFix.exe
CrimsonSkiesFix.log        # beside launcher when logging is enabled
update1.02               # optional official updater; handling is future work
CrimsonSkiesFix/
    CRIMSON.EXE
    ddraw.dll               # extracted from launcher, not downloaded
    ddraw.dll.bak           # previous DLL, if upgraded
    GOSDATA/...
    CrimsonSkiesFix.ini  # generated directly in the game root, not overwritten
    CrimsonSkiesFix.ini.example
    README.md
    LICENSE
    build-info.txt
```

The alternative explicit command `CrimsonSkiesFix.exe --import "C:\\My Games\\Crimson Skies"`
imports the indicated complete installed game. `--verify` verifies the
supported EXE, required resource files and installed DLL **without writing**.
`--version` and `--help` are available. No `--launch` option is needed.

**Not supported in this PR:** ISO/CUE/BIN game-asset extraction, retail
installer automation, SafeDisc-protected executable transformation, a
compressed CAB layout, or automatic acquisition of an unwrapped EXE. These
require validated disc profiles / independently supplied compatible
executables; this launcher does not bundle or create game assets and does not
disable the existing EXE gate.

## Safety and release status

- Existing installed games are never overwritten by an import. Existing
  unrelated game files, the active INI, and the original ROF source files
  are retained. An already installed game is never reconstructed automatically.
- Symlinks/junction-like links observed as symlinks are refused during import.
  Any stale staging/upgrade path causes a clear failure for manual inspection.
- A proxy DLL upgrade is staged and hash-checked before publishing. The
  previous active DLL is retained as one rolling `.bak` version; there is no
  numbered backup accumulation.
- The original game directory is read-only to the importer; it copies data
  to an independent folder. Its source EXE is not rewritten.
- The runtime DLL still owns the small reversible GOS overlay
  `crimptch.rof` update. Follow its backup/restore instructions before
  uninstalling a fully installed patch.
- The standalone EXE embeds the exact DLL, current documentation, sample INI
  and GPL-3.0-or-later license text. No copyrighted game data is included.
- Default INI minimization lives in PR #25; this launcher will automatically
  inherit the resulting proxy-created defaults after that PR is merged.

The installer job tests the SHA-256 algorithm, the embedded DLL's presence,
architecture and basic refusal cases. Successful CI does **not** prove a
real Crimson Skies installation/gameplay session; test Windows 11 and
Wine/Proton with an actual accepted game before merging.

## Planned official 1.02 update handling

When an authenticated `update1.02` installer is available next to `CrimsonSkiesFix.exe`, future builds will offer to apply it as part of installation. **This bootstrap does not execute arbitrary nearby files.** We must first validate the updater's exact file format, identity and supported source editions from the owner's forthcoming disc/version scans, and ensure the update does not overwrite user data. The official retail updater may leave a SafeDisc-protected executable; it does not by itself satisfy the strict accepted unwrapped EXE SHA-256 check.

The intended final flat layout is exactly one `CrimsonSkiesFix/` game directory, with `CrimsonSkiesFix.ini` at that game's root and `CrimsonSkiesFix.log` beside the launcher. Existing nested INIs are migrated to the game root on first DLL load, without deletion of the original.
