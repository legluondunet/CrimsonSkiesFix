# WinMM bootstrap and renderer independence — phase 1 audit

**Scope:** design and read-only import analysis only. No game runtime, installer or
release behavior changes. This work is intended for a future version;
CrimsonSkiesFix **1.0 (`v1.0`) remains immutable**.

## Goal

Make the filename `ddraw.dll` exclusively available to the chosen graphics
provider (Windows/Wine system DirectDraw, or a future D7VK installation).
Move CrimsonSkiesFix's initialization, game-resource fixes and unrelated hooks
behind another early-loading entry point such as `winmm.dll`, **without losing**
the current DirectDraw COM modifications until replacement or delegation is
proven to work.

## What current source code proves

1. `src/lib.rs` exports `DirectDrawCreate`, `DirectDrawEnumerateA` and
   `DirectDrawCreateEx`. It executes the executable SHA gate, configuration,
   GOS, startup-video and DirectDraw hook initialization from these entry
   points. The actual system DirectDraw is currently loaded by the
   proxy using an explicit `GetSystemDirectoryW()\ddraw.dll` path in
   `src/win32.rs`.
2. `src/ddraw_forward.rs` and `src/ddraw_exports.def` implement the complete
   **22-export native x86 DirectDraw proxy**. A `winmm.dll` proxy will have a
   different export surface: simply renaming the current DLL is invalid.
3. `src/modern_ddraw_hooks.rs`, `src/modern_surface_hooks.rs`,
   `src/com_surface4_forward.rs`, `src/in_game_adapter.rs` and
   `src/borderless_native_blt.rs` patch or translate graphics COM behavior;
   their compatibility with D7VK cannot be inferred from their current use
   with system DirectDraw.
4. `src/native_video_imports.rs` already has a diagnostic in-memory PE32
   import-table reader, but it reports only selected display-mode and DirectDraw
   functions. Its report does **not** establish whether WinMM is directly or
   indirectly imported by the game. `src/native_rof_load_trace.rs` already
   observes authenticated game `LoadLibrary*` IAT imports **after** initial
   DirectDraw entry — not early enough by itself to bootstrap the migration.
5. The installer (`installer/src/main.rs`, `installer/src/auto_install.rs`)
   embeds, installs, hashes and repairs a file named `ddraw.dll`. It must
   eventually be updated transactionally and must *never* silently replace an
   unknown wrapper. This audit does not alter the current installer path.

**Not established yet:** the validated decrypted 1.02 executable's static
WinMM imports, delay imports, or practical load order. The binary is not
included in this repository. We must not claim WinMM is viable based only on
other Windows games or search results.

## Verified original 1.02 PE32 — static import audit (14 November 2026)

The owner supplied `testme(1).exe` for **read-only** local static inspection.
The file **was not committed to the repository, executed, patched or redistributed**.

- SHA-256: `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`
- File size: **2,580,480 bytes**; format: **PE32 Intel i386**; matches the one hash-approved decrypted 1.02 executable.
- The PE import directory includes **21 distinct DLLs**.
- **`WINMM.dll` is a regular/static import**, descriptor RVA `0x61F014`, IAT RVA `0x620498` (14 named functions).
- **`DDRAW.dll` is also a regular/static import**, descriptor RVA `0x61F118`, IAT RVA `0x61FBA4` (2 named functions).
- The PE **delay-import directory is empty**, so these are ordinary loader imports rather than delay-loaded imports.
- The WinMM descriptor occurs earlier in the import directory than DirectDraw, **but descriptor order by itself is not a reliable cross-platform initialization or first-call guarantee**.

### Exact imports to preserve in a future WinMM proxy (14)

```text
mixerGetLineInfoA
mixerOpen
mciGetErrorStringA
mixerGetLineControlsA
mixerClose
auxSetVolume
auxGetVolume
mciSendCommandA
auxGetNumDevs
auxGetDevCapsA
mixerSetControlDetails
mixerGetControlDetailsA
mixerGetNumDevs
timeGetTime
```

DirectDraw functions imported from `DDRAW.dll`:

```text
DirectDrawCreate
DirectDrawEnumerateA
```

**Feasibility decision:** WinMM is a **credible early-loading bootstrap candidate** for this exact game EXE. However, a DLL's static import only ensures the operating system resolves the dependency as part of normal process loader initialization. It does **not** justify installing game hooks under the Windows loader lock, and it does not show that any WinMM export is invoked before `DirectDrawCreate`. The next prototype must demonstrate **safe first-use or explicit launcher bootstrap initialization** and preserve all 14 WinMM functions under both Windows and Wine. No automatic migration should be enabled until that proof is available.

## Inspect the locally owned approved executable

On Linux or Windows (Python 3; no extra packages required):

```sh
python3 CrimsonSkiesFix/tools/audit_bootstrap_imports.py /path/to/CRIMSON.EXE
python3 CrimsonSkiesFix/tools/audit_bootstrap_imports.py /path/to/CRIMSON.EXE --json
```

On Windows, `python` or `py -3` can be used instead of `python3`.

The script prints the SHA-256, size, approved identity, every ordinary and
PE32 delay-import module/function, and highlighted `winmm.dll`/`ddraw.dll`
entries. It reads *only* the original file: no execution, injection, copying,
patching or Wine prefix is involved. All descriptors, thunk lists, imports and
filenames have bounds; malformed files fail closed. A non-approved hash is
reported as such with exit status 1 (a PE parse failure returns 2). The user
can provide the text/JSON report, **not** the copyrighted executable.

## Interpreting the result

| Result | Implication | Next step |
| --- | --- | --- |
| `static_winmm` nonempty | Direct PE import may load WinMM on process startup | Verify the *actual DLL path, load order and first safely callable export* under Windows and Wine; a static import is not proof of patch activation order |
| `delay_winmm` nonempty, no static WinMM | WinMM may load only on first use, potentially *after* DirectDraw | Measure load order; prefer a launcher-driven early load or a different import |
| Neither WinMM import present | No direct PE import of WinMM | Check indirect/dynamic loads and compare other candidate DLL imports; do not install a WinMM proxy without a bootstrap plan |
| `static_ddraw` nonempty | The game statically imports the DirectDraw API | Hooks must be installed before the game uses the API or intercepted by an explicitly verified DLL/function/IAT chain |

`GetProcAddress`, transitive DLL loads, KnownDLLs, Wine native/builtin overrides
and alternative library search strategies are outside this static analysis.
Import-table *order* alone is not a verified timing contract.

Windows DLL loading order:
https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-search-order

## Safe proposed migration — separate follow-up PRs

1. **Bootstrap proof:** authenticate the EXE and collect the static imports.
   Measure `winmm.dll` and `ddraw.dll` module origins and load/use timing on
   native Windows and Wine (no executable modification). Confirm app-local
   loading, or explicitly choose a launcher-controlled bootstrap instead.
2. **WinMM proxy:** implement only the game's required WinMM x86 exports with
   exact ABI forwarding to the real system library; verify under Windows/Wine.
   No actual graphics changes yet. **No nontrivial initialization inside
   `DllMain`**, under the loader lock.
3. **Core separation:** extract a backend-neutral, once-only game fix
   initializer; keep the exact executable hash check, guarded GOS patches,
   Windows/Wine logic and INI. Move DirectDraw interception to an explicitly
   tested IAT/function boundary rather than relying on the `ddraw.dll` filename.
4. **Transparent system backend:** use ordinary system DirectDraw *without*
   our own `ddraw.dll`, and prove parity on menu, missions, video,
   widescreen, borderless/Alt-Tab, FPS, input and clean exit. Preserve COM
   lifetime and threading protections.
5. **Installer migration:** install and hash `winmm.dll` transactionally;
   migrate only *authenticated* old CrimsonSkiesFix `ddraw.dll`. Unknown local
   graphics DLLs are left untouched; game data and user configuration are
   preserved. Do not bundle D7VK.
6. **External renderer experiment:** optionally allow user-installed D7VK
   `ddraw.dll`. Do not assume its COM objects or DirectDraw 2D support match
   the system backend. Detect failures and retain safe system fallback.

**Release policy:** keep all changes in feature branches until reviewed,
tested and approved; `v1.0` stays untouched. Do not advertise D7VK support
until it works in actual game sessions.

## Automated tests

`tools/test_audit_bootstrap_imports.py` constructs five synthetic PE32 cases:
static WinMM+DirectDraw imports, delay-only WinMM, no WinMM, malformed tables
and ordinal imports. It uses no game binaries.

```sh
python3 CrimsonSkiesFix/tools/test_audit_bootstrap_imports.py
```
