# Install original Crimson Skies CD files (experimental, SafeDisc handled last)

**Scope:** This is an **installation of the original game files**, not
a demonstration that the game can already launch. The historical
`CRIMSON.EXE` from the CD remains protected and is intentionally untouched.
SafeDisc / approval of the final 1.02 executable is a **separate, final stage**.

This stage differs from `--extract-cd`: it prepares and publishes the
permanent `CrimsonSkiesFix/` installation directory, adds the official
1.02 gameplay asset, installs the bundled DirectDraw compatibility proxy,
creates the writable game folders, verifies the result, and records provenance.

## First installation test under Wine

Place the **latest experimental** `CrimsonSkiesFix.exe`, exactly one of the
owner's recognized original CD images (`.iso` or `.cue` with its `.bin`),
and the exact validated official `Crimson_Skies_Patch_102.exe` in the same
directory. If multiple images are adjacent, specify the desired one.

```sh
# French ISO (explicit input selection)
WINEDEBUG=-all wine CrimsonSkiesFix.exe --install-cd "MS-CrimsonSky-FrenchVersion.iso" "Crimson_Skies_Patch_102.exe"
# Alternative, with exactly one original disc and updater adjacent:
# WINEDEBUG=-all wine CrimsonSkiesFix.exe --install-cd

WINEDEBUG=-all wine CrimsonSkiesFix.exe --verify-install
```

The program first confirms the exact official patch checksum and the
entire CD's **internal** per-file SHA-256 manifest. Only supported USA,
FR and DE editions are accepted; a renamed image does not bypass
per-file checks.

**Transaction layout:**

```text
CrimsonSkiesFix.exe                 # launcher, outside installed game
CrimsonSkiesFix.log                 # installation report
My-Disc.iso or My-Disc.cue + BIN    # original media, never modified
Crimson_Skies_Patch_102.exe         # user's verified official update
CrimsonSkiesFix/                   # created ONLY after full preflight
    CRIMSON.EXE                     # original protected CD EXE, unchanged
    CRIMSON.ICD                     # original protected CD payload, unchanged
    ddraw.dll                       # embedded, SHA-256 verified
    GOSDATA/
        ASSETS/
            CRIMSON.ROF             # original language-specific game archive
            CRIMPTCH.ROF            # same verified official 1.02 ROF
            GRAPHICS/...
            BINARIES/...
    ZBD/                            # chapter CABs + sound/geometry archives
    SavedGames/                     # newly created, writable
    Planes/                         # newly created, writable
    CrimsonSkiesFix.ini.example     # active INI generated at first game start
    build-info.txt
    CrimsonSkiesFix.install-receipt.txt
    README.md
    LICENSE
```

The CD provides the edition-specific localization, videos, CABs and ZBDs.
The original 1.02 patch is never run as a Windows executable; its
**audited LZX CAB** is read and only the 797-byte `CRIMPTCH.ROF` is
added or preserved (the FR CD already contains the exact same asset).
No Microsoft legacy DirectX dependencies, setup executable, fonts,
or system services are installed.

The installer creates `CrimsonSkiesFix.installing/` first. Every
original GAME/ file is checked against the CD's authenticated per-file
manifest **both during extraction and before the final rename**.
The official ROF and our embedded `ddraw.dll` are hash verified.
An existing `CrimsonSkiesFix/` or abandoned
`CrimsonSkiesFix.installing/` is never overwritten or deleted.
A failure leaves staging for manual investigation without publishing an
incomplete game directory.

After success the log includes:

```text
InstalledProxy=VERIFIED
SavedGamesDirectory=CREATED
PlanesDirectory=CREATED
GameFilesVerified=YES
ExecutableState=PROTECTED_PENDING
INSTALL_RESULT=GAME_FILES_READY_EXECUTABLE_PENDING
```

The separate `--verify-install` command independently rehashes
all installed original GAME/ files (including the original protected
EXE), plus the official patch asset and the embedded proxy. It does
**not** attempt to execute the game or mistake a protected executable
for the project's required **exact unwrapped 1.02** game EXE.

On a subsequent ordinary launch, a verified CD install with its
unchanged protected executable reports that the data is ready
and exits cleanly. Only after a future final executable stage satisfies
the existing strict 1.02 SHA-256 gate will the normal launcher
attempt to start the game.

## Not yet established by the available test data

We have not compared an installed game directory produced by Microsoft's
original installer to this result. Any installation-time registry entries,
empty directories other than SavedGames/Planes, or chapter-CAB expansion
still need to be audited before declaring a *playable* installation fully
equivalent to the original one. The current chapter CAB files are retained
as original files under `ZBD/` rather than indiscriminately unpacked.
The original patch's protected 1.02 executable pair is **not** currently
applied; it belongs to the deferred executable stage.

Further validations: real ISO/CUE installations under Wine and native
Windows 11, `--verify-install`, refusal to overwrite an installation,
and eventual supported 1.02 executable launch. Keep PR #26 **draft**.
