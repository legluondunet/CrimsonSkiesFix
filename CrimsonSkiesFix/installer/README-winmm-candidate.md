# Experimental CrimsonSkiesFix WinMM-only installer (1.1.0-rc.1)

This is an **opt-in**, standalone test-only build of the existing Rust installer
with `--features winmm-installer`. It embeds the complete shared
`runtime-winmm` game compatibility DLL, deploys **`winmm.dll` instead of
`ddraw.dll`**, and verifies its SHA-256. It does not ship or modify game data.

**Not a release.** The original v1.0 installer and `ddraw.dll` release
are unchanged. Full mission, Windows native, audio, input, Alt+Tab and
external DDraw wrapper compatibility tests have NOT been completed.

## Safety rules for this candidate

- Work on a **backup/copy** of the game, not the original.
- An app-local `ddraw.dll` of *any* provenance blocks experimental WinMM
  installation and auto-repair. Nothing deletes or renames that file.
  Migration of known v1.0 proxy hashes and external graphics wrappers is
  deferred until separately tested.
- An unknown existing `winmm.dll` still blocks replacement; its bytes are
  preserved. Exactly one earlier **verified experimental** WinMM build
  (735,232 bytes, SHA-256 `ffab4bfbc76b8aa9b22799b04242eb5f2ac18fc7016d534bf59fcdeb51c8f86a`,
  log identity `6d358ba9`), **or** the exact `7395cd3c` runtime
  (737,280 bytes, SHA-256
  `bec010d352aaf2dc16836d3f99fb8c104e61e1a98c3f5980736f933b112bb54e`,
  authenticated from the original CI artifact) may be upgraded in place.
  Its bytes are preserved as `winmm.dll.bak`. If that backup name already exists, the installer
  refuses to overwrite it. Other previous builds require manual inspection.
- Game EXE and assets retain the existing hash verification. The exact supported
  decrypted 1.02 executable is required; no unprotected game files ship here.
- `CrimsonSkiesFix.exe --launch-no-verify` remains the explicit **no repair**
  launch path; `--verify` checks `winmm.dll`, not `ddraw.dll`.
- Configuration remains `CrimsonSkiesFix/CrimsonSkiesFix.ini`. The DLL's
  gameplay log remains separate from `CrimsonSkiesFix_installer.log`.
- No known or unknown user DLL is removed by this test installer.

## Compile (Windows x86 CI)

Use [the isolated workflow](../../.github/workflows/winmm-installer-candidate.yml).
It builds and hashes `runtime-winmm/target/.../winmm.dll`, sets
`CRIMSONSKIESFIX_BUNDLED_DLL` to that exact binary, then builds
`installer/Cargo.toml --features winmm-installer` and runs tests.

The GitHub Actions artifact is labeled
`CrimsonSkiesFix-WinMM-1.1-TEST-ONLY-NOT-RELEASE`. Its executable is
**`CrimsonSkiesFix-WinMM-1.1-TEST.exe`**, not `CrimsonSkiesFix.exe`, so it
cannot be confused with the parallel v1.0 installer.
The `--version` output **must** contain both `1.1.0-rc.1` and
`runtime=winmm.dll EXPERIMENTAL`. If it prints `1.0.0` or mentions
`ddraw.dll`, stop: that is the old installer, not this candidate.
Do not use `main`'s v1.0 installer to verify or automatically repair a WinMM
game installation; it is designed to restore the old `ddraw.dll`.

## Identifying the correct file

**Two CI workflows can execute on the same commit:** one builds the unchanged
v1.0 DirectDraw installer, and the other builds this opt-in WinMM candidate.
The source commit suffix alone cannot identify which variant you downloaded.
Do not use a binary reporting `CrimsonSkiesFix 1.0.0-...` to validate WinMM;
it will fail `--verify` because it still requires `ddraw.dll`.

The new candidate has a different executable name and prints
`runtime=winmm.dll EXPERIMENTAL` with `--version`.
Do not rename it to the old v1.0 executable name or overwrite the original
installer; keep it beside the copied `CrimsonSkiesFix/` game directory.

## Existing experimental WinMM DLLs not recognized by SHA

The 1.1 test installer deliberately does not replace an unknown existing
`winmm.dll` even when a game log reports a familiar CrimsonSkiesFix commit:
different builds can share the **same log identity** but have **different
SHA-256 digests**. Always use the actual DLL digest, not the version in the
log, to decide whether a DLL is an approved upgrade source.

If `--install-only` returns
`existing winmm.dll SHA-256=... is not an approved experimental upgrade`,
**keep the file**. On a separate copy of the working game, move the existing
`winmm.dll` to an archival filename such as `winmm.dll.previous-test`,
first checking that destination does not already exist. Do not copy it back
as an active `winmm.dll` until returning to the previous test.

The installer also **refuses to rotate or delete an existing
`winmm.dll.bak`**. Archive that file separately if it is present. Avoid
using `winmm.dll.bak` as your manual archival filename. The `--verify`
command must always be run with **`CrimsonSkiesFix-WinMM-1.1-TEST.exe`**,
not with the unchanged v1.0 `CrimsonSkiesFix.exe`.

## Manual use on an isolated copy

Once a backed-up test folder has **no app-local `ddraw.dll`** and contains
either no `winmm.dll`, the embedded WinMM, or the exact SHA-approved earlier
experimental WinMM listed above, run the commands below. **If the installer
refuses an unrecognized DLL, leave it untouched** and perform a manual
inspection before renaming anything:

```sh
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --version
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --install-only
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --verify
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --launch-no-verify
```

The native/builtin Wine override is **per process** and is not persisted into
a Wine prefix. On Windows, the app-local WinMM DLL should load normally.

**Expected evidence:** game log identifies `WinMM IAT bootstrap`,
contains `WINMM-BOOTSTRAP: ... iat_status=7`, and reports stable menu,
mission, video, audio and Alt+Tab behavior. No `ddraw.dll` should be
installed by CrimsonSkiesFix in the game directory.

This build is a stepping stone toward making the standalone v1.1 installer
the default *after* validating and designing safe migration for old v1.0
installations and known external graphics wrappers.
