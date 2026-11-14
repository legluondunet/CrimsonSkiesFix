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
- Unknown existing `winmm.dll` files are never overwritten. The installer
  recognizes **only** pinned SHA-256 digests of our own previously built
  experimental WinMM DLLs: `6d358ba9`, `7395cd3c`, `65304a3e`,
  `d2e45149`, `0bcc164b`, `39bb2e5d` and `f3ebbe51`, verified directly from their
  original GitHub Windows x86 CI artifacts. It preserves the prior active DLL
  as `winmm.dll.bak`; an existing backup name is never overwritten. All
  other WinMM files (including third-party wrappers) require manual review.
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

## Native Windows DD4 owner diagnostics after 1920x1200 flight screenshot

The real f3ebbe51 Windows 11 run reached in-flight gameplay, but two formerly
circular HUD gauges measured ~179x149 and ~180x152 pixels (1.20/1.18
horizontal stretch), matching 4:3 content spread across a 16:10 output.
The log showed native 1920x1200 FOV patch and DD4 owner recognition but
did not record the windowed DD4 cooperative/mode/preflight callbacks.

This version adds a **read-only, native-WinMM-only** owner-vtable check at
the already verified game mode-setup entry. The log may include:

```text
NATIVE-WINMM-DD4-SLOTS: verified_game_owner=true cooperative=active display_mode=active
```

Other possible values include `not-installed` and
`registered-but-displaced`. This distinguishes an actual
missing/overwritten DirectDraw4 callback from a missing *game call*.
No rendering, mode, hook installation policy, COM call or graphics menu is
modified by this diagnosis. The existing GOS text hook still needs actual
Preferences-page confirmation.

The exact f3ebbe51 WinMM DLL from runtime artifact `14123386745`
(745,984 bytes, SHA-256
`fca524dc8805ad439a755d81eca84e7ade95b6e57b10843ba4963bd61d74ac67`)
is an approved, previous experimental upgrade source. Unknown DLLs remain
protected; a pre-existing backup name is never overwritten.

## Prototype: native Windows Graphics Preferences label fallback

This prototype adds an **experimental text-only fix** for the confirmed
native Windows bug where Graphics Preferences still displays `640x480`
even though the authenticated physical monitor/3D target is `1920x1200`
at 125% Windows DPI scaling.

The existing byte-exact `uiData(0x845)` formatter remains intact. Because
the owner logs show that formatter hook installing without any observed
invocation, the WinMM-only native Windows build now also intercepts the
verified original EXE's GOS ANSI **text assignment** routine at
`0x00403350`. It substitutes **only** the exact `640x480` / `640 x 480`
string, after the original MainMenu has been observed, with the physical
screen dimensions captured and verified by the early in-memory widescreen
patch. For the known Windows host that is `1920x1200`. Other text is passed
to the original function verbatim.

This intervention **does not** alter GOS display-mode entries, graphics
indices, driver enumeration, input/disabled widget state, game resources
on disk, 4:3 menu surfaces, or real monitor modes. The COM/DirectDraw
backend fix is tracked independently in issue #39. The text issue is #40.

The x86 trampoline uses exact instructions from the SHA-approved EXE,
preserves the original CDECL 3-argument return and original allocator,
and installs only after the executable hash gate; native Windows
`winmm-bootstrap` is the only eligible build. It cannot install on Wine
or the v1.0 DirectDraw proxy. The output is **not yet accepted in real
Windows gameplay**: unit tests/CI verify the replacement contract and x86
build, not actual visibility of the GUI text on the owner's machine.

With `[Debug] Log=1` the game log should contain:

```
MENU-WINMM-TEXT: validated native GOS 0x403350 text writer hooked
MENU-WINMM-TEXT: original=640x480 shown=1920x1200 ... text-only
```

If the second line never appears, the GOS controls use yet another
rendering/text path, and the exact text source must be traced before any
more intrusive intervention. Do not assume a visual fix from a green CI
build alone.

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
