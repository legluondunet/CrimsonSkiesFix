# Experimental WinMM-only CrimsonSkiesFix runtime

This crate compiles the **original gameplay and DirectDraw hooks** in
`../src/lib.rs` as `winmm.dll`. It is different from the limited forwarding
prototype in `../winmm-prototype`.

**CrimsonSkiesFix's own `ddraw.dll` is not required by this build.** The
original game still requires a working DirectDraw implementation, whether
provided by Windows/Wine or an optional external graphics wrapper.

## Implementation

The SHA-256-approved original 1.02 `CRIMSON.EXE` imports both WinMM and
DirectDraw. During WinMM's minimal process-attach entry, the DLL checks the
exact expected in-memory **PE and import descriptor fingerprints**, ensures the
two DirectDraw IAT slots point to the loaded DirectDraw module, and changes
those two pointer entries to target the original CrimsonSkiesFix hook functions.
No disk EXE mutation, log, config, file I/O, library loading or Rust allocation
occurs in that loader-lock path. All original game verification and graphics
hook setup is **deferred until the game itself calls DirectDraw**.

The original `CrimsonSkiesFix` runtime and renderer interception code is
shared; the bootstrapping filename changes, not the game's gameplay fixes.
The two game-imported DirectDraw functions forward to the **original IAT
function pointers captured at bootstrap**, preserving the renderer chosen by
the Windows/Wine loader (including external wrappers). Additional DirectDraw
exports, if needed, resolve from the loaded backend outside `DllMain`.

## Current proof and limitations

- The exact owner's EXE was parsed and the fixed import RVA preimage verified.
- Windows x86 compilation, synthetic PE fingerprint tests, WinMM PE export checks, app-local static WinMM loading and native Windows multimedia smoke tests **passed** in [GitHub Actions run 43590864566](https://github.com/legluondunet/CrimsonSkiesFix/actions/runs/43590864566). These checks do not constitute a game session.
- WinMM's 14 game-imported named functions are forwarded by an experimental x86
  ABI-preserving tail-jump proxy. Its ordinals **are not canonical WinMM ordinals**.
- Unknown dependent modules that import other WinMM APIs or ordinal entries may
  not work yet; do not deploy automatically.
- For Windows, an app-local WinMM DLL must actually be loaded; for Wine,
  native/builtin DLL overrides must be tested.
- This code has **not yet been validated in an actual game mission**.
  Test the output in a disposable working game installation.
- `DllMain` performs only constrained PE checks and IAT writes; nevertheless
  these operations run under the Windows loader lock. Any kernel or Wine
  behavior difference must be resolved before production release.
- No fallback attempts to overwrite an unknown or user-supplied graphics DLL.
- Does not alter the production installer, `main`, or release `v1.0`.

## Build

```sh
cd CrimsonSkiesFix/runtime-winmm
cargo build --release --target i686-pc-windows-msvc
```

The output is `target/i686-pc-windows-msvc/release/winmm.dll`.
Use the isolated CI artifact when testing; do **not** copy the production
CrimsonSkiesFix `ddraw.dll` into the same test directory.

## Controlled first game test (on a COPY only)

**Do not execute the 1.0 `CrimsonSkiesFix.exe` installer in the copied test directory.**
It currently installs/verifies the old `ddraw.dll` and does not manage WinMM yet.
The standalone `CRIMSON.EXE` must be the exact supported decrypted 1.02 EXE.

1. Make a **separate copy of the full, working Crimson Skies game folder**.
   Keep the original installation intact, especially saves, patch DLLs and INI.
2. In the **copied** folder only, rename the existing CrimsonSkiesFix
   `ddraw.dll` to `ddraw.dll.disabled`. Do not rename or replace any unknown
   third-party wrapper in the original installation. The test folder must have
   **no local `ddraw.dll`** — Windows/Wine supplies system DirectDraw instead.
3. Copy this experimental `winmm.dll` alongside the copied `CRIMSON.EXE`.
   Keep the exact original `CRIMSON.EXE` and game data unchanged.
4. In the copied `CrimsonSkiesFix.ini`, temporarily choose `[Debug] Log=1`
   (preserving other settings), to record the bootstrap status.
5. Start **`CRIMSON.EXE` directly**, not `CrimsonSkiesFix.exe`.

Under Wine/Linux (use the same existing Wine prefix as the working game):

```sh
cd /path/to/copied/CrimsonSkiesFix
WINEPREFIX=/path/to/your/existing/prefix WINEDLLOVERRIDES='winmm=n,b' wine CRIMSON.EXE
```

The `winmm=n,b` override is **process-local** and does not permanently
change Wine settings. Do not use a global override. Under native Windows,
double-click `CRIMSON.EXE` in the **copied** game folder instead.

### What to report

Look for `CrimsonSkiesFix.ini` **next to `CRIMSON.EXE`**. The runtime creates it on its first intercepted DirectDraw call, without overwriting an existing file.

The `CrimsonSkiesFix.log` is written next to the DLL **unless** the game directory is named `CrimsonSkiesFix` and its parent contains `CrimsonSkiesFix.exe`; in that standalone layout the game log belongs to the **parent directory**, beside the launcher. With the current default windowed adapter, a baseline log is created even when `[Debug] Log=0`; `Log=1` enables extra diagnostics.

In that log, look for:

```text
WINMM-BOOTSTRAP: game DirectDraw API reached; iat_status=7
```

Status **7** indicates the two game IAT entries were redirected to the
CrimsonSkiesFix runtime. It does **not** prove that a menu or mission rendered
correctly. Other values mean the bootstrap was rejected or incomplete
(`1`=EXE shape mismatch, `4`=unresolved/already-hooked game IAT pointers, `5/6`=IAT protection failure). With the revised bootstrap, the two real DirectDraw function pointers are captured from the game's already-resolved import table; no `ddraw.dll` module lookup is required inside `DllMain`.

Check startup, menu, mission, video, audio, resolution/FOV, borderless
Alt+Tab and clean exit. Save the log and describe any missing features.
If the game fails to start, first stop the test — the original game folder
remains intact. Remove the experimental `winmm.dll` from the copied folder
and restore its backed-up `ddraw.dll` if needed.

**This prototype is not production-ready.** Runtime proof on both Wine and
Windows and a loader-lock/WinMM-ordinal safety review are mandatory before
the installer can migrate to `winmm.dll` or support third-party renderers.
