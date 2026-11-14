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
- Any **regular existing** `winmm.dll` is replaced by the bundled runtime,
  **even if its SHA-256 is unknown**. Its exact previous bytes become
  `winmm.dll.bak`. When `.bak` already exists, it is replaced by that
  previous active DLL, irrespective of the old backup's origin.
- The old backup is temporarily parked as `winmm.dll.bak.previous` while
  publishing the new DLL, permitting rollback if installation fails. It is
  deleted after successful publication; never retain multiple backups.
  An orphaned `.installing` or `.bak.previous` indicates an interrupted
  transaction and blocks a new install for inspection.
- If the current `winmm.dll` already matches the embedded DLL, launching
  again is a no-op: neither the DLL nor `.bak` is touched. If the active
  DLL is absent, an existing backup is kept rather than pointlessly deleted.
  Directories and unsafe filesystem objects are still rejected.
- Game EXE and assets retain the existing hash verification. The exact supported
  decrypted 1.02 executable is required; no unprotected game files ship here.
- `CrimsonSkiesFix-WinMM-1.1-TEST.exe --launch-no-verify` remains the
  explicit **no repair** path; normal launch and `--install-only` perform
  the automatic upgrade where safe. `--verify` checks `winmm.dll`, not
  `ddraw.dll`.
- Configuration remains `CrimsonSkiesFix/CrimsonSkiesFix.ini`. The DLL's
  gameplay log remains separate from `CrimsonSkiesFix_installer.log`.
- An older `winmm.dll.bak` **is intentionally overwritten** during upgrades; preserve an independent copy first if you want a longer history.

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

## Previously unknown WinMM DLLs

SHA-256 authentication is no longer a prerequisite for updating an already
installed `winmm.dll`. When the embedded DLL differs, the user-approved
policy is to move the existing regular file to `winmm.dll.bak`, overwriting
the previous backup transactionally. Thus older experimental builds and
third-party WinMM files are treated identically for this **test installer**.
A regular file with unexpected content is not executed by the installer.
The installed game EXE and game assets remain strictly verified.

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

## Automatic one-backup WinMM upgrade behavior

With the exact approved game and no app-local `ddraw.dll`, replacing the
standalone TEST executable and **launching it normally** is sufficient.
The installer hashes the existing `winmm.dll` and stages the embedded
new DLL if the hashes differ, regardless of the old DLL's origin. It
temporarily moves an existing `.bak` out of the way for rollback,
renames the previous active DLL to `winmm.dll.bak`, and installs the
verified replacement. On success it deletes the older backup.

Only **one** `winmm.dll.bak` remains; it always contains the previous
active DLL, even if that DLL was unknown. If the installed DLL already
matches the embedded hash, nothing changes. If there is no active DLL,
a pre-existing `.bak` is preserved. If a rename or publication fails,
the transaction attempts to restore the original active and backup paths;
an interrupted transaction requires inspection before retrying.
The original 1.0 `ddraw.dll` installer behavior is unchanged.

The launcher **does not** perform updates in `--launch-no-verify` mode.

## Manual use on an isolated copy

On an isolated test copy with **no app-local `ddraw.dll`**, the launcher
will upgrade an existing regular `winmm.dll` regardless of its hash.
The following optional commands allow manual verification, but a normal
launch performs the installation and backup rotation automatically:

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
