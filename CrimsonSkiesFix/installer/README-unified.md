# CrimsonSkiesFix 1.1 — unified single-DLL architecture (EXPERIMENTAL)

**TEST BUILD — NOT A RELEASE.** The unified architecture has passed real
Crimson Skies gameplay tests on Windows 11 and GE-Proton, but no v1.1 release
is published yet.

Intended layout:

```text
crimsonskiesfix/
  CrimsonSkiesFix.exe      <- installer, verifier and mandatory launcher
  CrimsonSkiesFix.ini
  CrimsonSkiesFix.log
  CrimsonSkiesFix_installer.log
  state/
    CrimsonSkiesFix.install-receipt.txt  <- verified CD installation receipt
  docs/
    README.md
    LICENSE
    CrimsonSkiesFix.ini.example
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

**Runtime update:** if game/fix.dll is missing or does not match the exact
DLL embedded in this launcher, the installer stages and verifies its own
fix.dll, then replaces the older regular file. The displaced DLL is removed
after successful publication; no new fix.dll.bak is retained. If an update
cannot be completed, the previous DLL is restored where possible and the
installer returns an explicit error. Existing unrelated fix.dll.bak files
are never touched. For an already verified manual game import, this repair
does not require extracting the CD again.

## Testing this prototype (Windows 11 or Wine/GE-Proton)

Use a **separate test folder** named `crimsonskiesfix`. Do not overwrite an
existing working CrimsonSkiesFix/WinMM installation. The installer does **not**
migrate old folder layouts to this unified architecture.

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

**Movies under Wine / GE-Proton:** intro and cinematic playback can fail
independently of the DirectDraw widescreen patch. Quartz/DirectShow is a
**diagnostic hypothesis**, not a universally verified prerequisite and not
necessarily a complete decoder solution. For an isolated-prefix test,
runner-specific logs, and an optional *manual* `winetricks quartz` comparison,
see [the Wine video troubleshooting guide](https://github.com/legluondunet/CrimsonSkiesFix/blob/main/CrimsonSkiesFix/docs/wine-directshow-movie-troubleshooting.md).
The installer does not change codecs, DirectShow overrides, or the user's
Wine/Proton prefix.

**First-launch logging:** a freshly generated INI starts with `[Debug] Log=1`
so that the first game session produces `CrimsonSkiesFix.log`. After a normal
exit (code 0) with a nonempty game log, the installer changes only its own
**unmodified**, automatically created INI to `Log=0`. The first log is
preserved for support. Nonzero exits keep `Log=1` for a retry, and an INI
supplied or edited by the player is never changed automatically. A temporary
marker inside `state/` identifies the installer's first-run ownership and
is removed when logging is finalized or player customization is detected.
The separate `CrimsonSkiesFix_installer.log` remains available regardless
of this setting.

If a command fails, preserve `CrimsonSkiesFix_installer.log` and
`CrimsonSkiesFix.log` from the test root. For rendering regressions,
compare game launch, opening menu, preferences, first mission, movies,
pause, save/load and exit with the previously verified WinMM build.

The installer stores the verified installation receipt and its temporary
rollback files in `state/`, and generated documentation in `docs/`. The
active INI and both logs remain beside the launcher. `state/` is created
only when a CD receipt must be published; an imported game without a CD
receipt does not need it. The installer does **not create** `mods/` or
`third_party/` by default. Existing user content is left untouched.

**Important limits:**
- Start the game with CrimsonSkiesFix.exe, not directly via CRIMSON.EXE.
- Only the exact approved decrypted 1.02 game EXE is supported.
- No game, disc, updater, or copyrighted content is distributed.
- Synthetic PE/Win32/Wine checks cannot replace a real in-game test.
- DirectDraw invoked by a third-party DLL from its own DllMain could still
  be unsafe; this architectural risk requires separate examination.
- No automatic migration from older `CrimsonSkiesFix/` or `crimsonskies/`
  layouts is implemented. Use a separate new-layout installation.
- Rebuilding a broken **new-layout** installation from an authenticated disc
  is supported, with the former `game/` preserved for rollback.
