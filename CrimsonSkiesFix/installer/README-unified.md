# CrimsonSkiesFix 1.1 — unified single-DLL architecture (EXPERIMENTAL)

**OWNER TEST ONLY — NOT A RELEASE.** This build is a new architecture; it
has not yet passed real Crimson Skies Windows/GE-Proton gameplay tests.

Intended layout:

```text
crimsonskiesfix/
  CrimsonSkiesFix.exe      <- installer, verifier and mandatory launcher
  CrimsonSkiesFix.ini
  CrimsonSkiesFix.log
  CrimsonSkiesFix_installer.log
  CrimsonSkiesFix.install-receipt.txt
  README.md
  LICENSE
  build-info.txt
  game/
    CRIMSON.EXE
    fix.dll                <- single combined loader + CrimsonSkiesFix code
    GOSDATA/
    Planes/
    SavedGames/
```

The launcher accepts only the approved decrypted Crimson Skies 1.02 executable
and patches the *suspended child process memory* PE32 import name:
`DDRAW.dll\0` -> `fix.dll\0\0\0`, exactly ten bytes in both cases.
The original on-disk CRIMSON.EXE is **not modified**. Native system WinMM is
not replaced by any project DLL. Third-party ddraw.dll wrappers are not
overwritten; the actual backend is resolved on the first DirectDraw call.

The single **fix.dll** contains all CrimsonSkiesFix patches, exports the
game's two imported DirectDraw functions, and does not dynamically load a
second CrimsonSkiesFix.dll. Its DllMain remains inert.

## Testing this prototype (Windows 11 or Wine/GE-Proton)

Use a **separate test folder** named `crimsonskiesfix`. Do not overwrite an
existing working CrimsonSkiesFix/WinMM installation.

**Existing game files (quick smoke):** put the packaged `CrimsonSkiesFix.exe`
at the test-folder root, and copy a clean, already installed Crimson Skies
game into `game/`. The `game/` directory must contain the *approved decrypted
1.02* `CRIMSON.EXE` and the original `GOSDATA/` assets. Do **not** copy an
old project `winmm.dll` or `CSFIX.dll` into this new test installation.
The launcher installs the embedded `fix.dll` if the game passes the strict
SHA-256 checks; it does not need the CD just to launch a verified legacy
import. A third-party DirectDraw wrapper is not overwritten.

**Fresh install from original disc:** beside `CrimsonSkiesFix.exe`, place
exactly one original `.iso` or `.cue` with its referenced `.bin`, one
authentic official 1.02 updater, and the approved decrypted 1.02 executable.
The launcher verifies all prerequisites before extraction. It never
redistributes the game's files.

Run these commands **from the test folder**:

```text
CrimsonSkiesFix.exe --version
CrimsonSkiesFix.exe --install-only
CrimsonSkiesFix.exe --verify
CrimsonSkiesFix.exe
```

With Wine, prefix each command with `wine` or launch through your normal
Lutris/Proton environment. `--install-only` checks and prepares the
installation without starting the game. `--verify` checks the approved
binary, data and bundled `fix.dll`. The final command launches the game
through the required suspended-process import alias. Launching
`game/CRIMSON.EXE` directly **does not activate** our corrections.

If a command fails, preserve `CrimsonSkiesFix_installer.log` and
`CrimsonSkiesFix.log` from the test root. For rendering regressions,
compare game launch, opening menu, preferences, first mission, movies,
pause, save/load and exit with the previously verified WinMM build.

The installer **does not create** `mods/` or `third_party/` by default.
These folders will only be needed if future features actually use them.
Any existing user content at those paths is left untouched.

**Important limits:**
- Start the game with CrimsonSkiesFix.exe, not directly via CRIMSON.EXE.
- Only the exact approved decrypted 1.02 game EXE is supported.
- No game, disc, updater, or copyrighted content is distributed.
- Synthetic PE/Win32/Wine checks cannot replace a real in-game test.
- DirectDraw invoked by a third-party DLL from its own DllMain could still
  be unsafe; this architectural risk requires separate examination.
- This opt-in prototype must not be merged or released before complete
  extraction, repair, migration and real gameplay validation.
