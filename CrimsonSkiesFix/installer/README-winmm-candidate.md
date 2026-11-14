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
- An unknown existing `winmm.dll` also blocks replacement; its bytes are
  preserved. The only accepted installed WinMM is a bit-identical copy of
  the embedded candidate runtime.
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
`CrimsonSkiesFix-WinMM-installer-x86-EXPERIMENTAL-NOT-RELEASE`.
Do not use `main`'s v1.0 installer to verify or automatically repair a WinMM
game installation; it is designed to restore the old `ddraw.dll`.

## Manual use on an isolated copy

Once a backed-up test folder has **no app-local `ddraw.dll`** and does not
contain an unknown `winmm.dll`:

```sh
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix.exe --install-only
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix.exe --launch-no-verify
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
