# CrimsonSkiesFix 1.1 — organized split WinMM prototype

**TEST ONLY — not a release and not approved for existing game installations.**

The standalone installer installs an authenticated owner-supplied Crimson Skies
1.02 game, keeping the original directory structure under **`crimsonskies/`**.
It contains two separately SHA-verified binaries and no packaged game data:

```text
CrimsonSkiesFix-WinMM-SPLIT-TEST.exe  # test installer/launcher
CrimsonSkiesFix_installer.log        # installation/repair log
CrimsonSkiesFix.log                  # game log, created when [Debug] Log=1
crimsonskies/
  CRIMSON.EXE                       # owner supplied approved unwrapped 1.02
  winmm.dll                        # compact WinMM import proxy/DirectDraw IAT loader
  GOSDATA/                         # original authenticated game resources
  SavedGames/
  Planes/
  crimsonskiesfix/
    CrimsonSkiesFix.dll            # real game-fix runtime
    CrimsonSkiesFix.ini            # created/preserved by runtime on first launch
    CrimsonSkiesFix.ini.example
    README.md
    LICENSE
    build-info.txt
    CrimsonSkiesFix.install-receipt.txt  # for installs reconstructed from CD
```

Use a **disposable test installation** with exact approved game data and the
official 1.02 updater. The prototype retains the existing refusal to touch a
local unknown `ddraw.dll`. It must not overwrite the user's 1.0 or monolithic
1.1 installation. The new path is deliberately separate.

## Boot sequence

1. The original PE loader imports the app-local `crimsonskies/winmm.dll`.
2. Its minimal `DllMain` validates the exact game import layout and redirects
   only two original DirectDraw IAT imports. It does not load the nested DLL.
3. When the real game calls the intercepted DirectDraw entry, `winmm.dll`
   loads `crimsonskiesfix/CrimsonSkiesFix.dll` by its absolute path and
   hands over the captured DirectDraw provider addresses/status.
4. The runtime verifies the full approved executable hash **before making game
   changes** and uses the existing shared DirectDraw/game-fix code. It reads
   the INI from its own subdirectory and writes `CrimsonSkiesFix.log` beside
   the installer (at the grandparent of `crimsonskiesfix/`).

The original game EXE is not patched on disk.

## Why a game can start without actually applying the patch

`AUTO_INSTALL_RESULT=READY` and `GAME_EXIT_CODE=0` prove only that
installation checks passed and the child process returned normally.
They **do not prove** that the app-local WinMM DLL was selected by Windows,
that the original game's two DirectDraw imports were redirected, or that the
nested `CrimsonSkiesFix.dll` was actually loaded. If the MainMenu signature
is missing and Graphics displays its original 640×480 label, collect the
**installer log** for the startup probe.

New split-only test installers append a line such as:

```text
SPLIT-PROBE: snapshots=... winmm_provider=local core_dll_seen=true directdraw_iat=redirected snapshot_unavailable=false
```

`winmm_provider=local` means the observed `winmm.dll` was loaded
from `crimsonskies/`; `system` means a different system-side copy was
observed, `unobserved` means the read-only process sampler did not capture
one. `directdraw_iat=redirected` means **both** original DirectDraw
IAT pointers in the approved game process pointed inside the local WinMM
shim, `not-redirected` means the observed slots remained elsewhere,
`partial` means only one pointed inside the shim, and `unknown` means
the sampler could not establish a result. The probe never injects,
suspends or writes to process memory.

When the experimental INI does **not already exist**, the split installer
creates it with `[Debug] Log=1`, `AspectRatio=auto` and `FPSLimit=60`
for this diagnostic build only; it never overwrites an existing INI.
The ordinary 1.0 and 1.1 builds keep their original defaults.

## Native Win11 initial video initialization (640x480 RGB565)

On a two-monitor Windows 11 test session the split loader successfully
redirected DirectDraw (`iat_status=7`), authenticated the game EXE and
prepared the first windowed D3D3 640x480x16 RGB565/Z32 graph. The game then
requested `IID_IDirectDrawSurface` (COM v1) on the graph's backbuffer,
which the previous bridge refused with `E_NOINTERFACE`; the user saw
**"Failed to initialize video"**. This failure happened **after** successful
HAL preflight, not because WinMM failed to load.

The split-only bridge now forwards this *exact* legacy 640x480 RGB565
backbuffer COM query to the **native backing surface**, returning the
provider's real v1 interface with the provider's own AddRef. It never
returns the virtual Surface4 pointer under a Surface1 IID. Primary,
Depth, unknown formats and unknown resolutions remain excluded.
The original monolithic WinMM and 1.0 variants are unchanged.

With `[Debug] Log=1`, a successful native COM handoff should produce
`NATIVE-WINMM-SURFACE1: ... mode=640x480x16 ... hr=0x00000000 valid_interface=true`.
The 640x480 logical game startup mode is expected: it must not be confused
with the later verified **1920x1200** physical 3D output. The next real
native-Windows test must confirm the game proceeds beyond this QI, shows
the MainMenu signature, enters gameplay and displays the corrected 3D
resolution label after returning to VIDEO Preferences. A green CI
alone does not validate rendering or remove WinMM loader-lock risks.

## One-time Lighting Quality High — experimental, exact GOS fix

The owner's real Windows log `CrimsonSkiesFix(20261116-155414).log`
proves the first attempt **did run**: `MENU-LIGHTING-DEFAULT` changed
the pending Video scalar from 2 to 1. However, the dropdown **remained
Medium**. This was **not** a WinMM/DirectDraw failure.

The original authenticated `VIDEO.SCRIPT` was independently examined.
It reads nine pending Video fields via `callback(...,2133,0,...)`, then
sends message `20001` to itself. That handler assigns Lighting Quality
`HUA` again from the GOS hardware-selector symbol `NVA`, overwriting
the value just read before it initializes dropdown `LVA`. Also, the
game's native lighting hardware flags (index 1) are **not** the five-item
dropdown's visible quality index: Medium is index 2 and the next higher
selection, High, is proposed as index 3.

The first GUI test of the initial resource workaround (build e47c2919)
still displayed Medium. Its startup log established the decisive cause:
the script preflight executed **before GameOS initialized preferences**
(`active_index=0`) and cached an ineligible result; later menu loading
could no longer change it.

The revised split-only experiment now:

1. Checks exact supported 1.02 EXE and authenticated original+managed ROF
   sources. Before preferences initialize, only the existing INI's
   completed-once marker determines whether an override may be prepared.
2. Builds an authenticated **4443-byte** VIDEO script (the managed source
   remains 4413 bytes on disk) with a conditional message-20001 override:
   read the original selection from NVA, **if and only if the current
   dropdown index is Medium (2), choose High (3)**. Existing Low, High
   and other selections remain unchanged.
3. Substitutes the stream only in the exact verified ROF `ReadFile`
   buffers, including the Windows-observed `CRIMPTCH.ROF` route.
   The decompressed-size descriptor is adjusted in the read buffer
   **before** delivering script bytes. Compressed length and all file
   offsets, Win32 return values and original files stay identical.
4. Leaves the original uiData pending fields untouched. Once the
   authenticated modified VIDEO resource has actually been delivered,
   the GOS UI Get callback may record that the first-use option was
   offered. Only after a successful original Apply does the runtime
   append `[Compatibility] CrimsonSkiesFixLightingHighDefaultApplied=1`
   to the existing INI. Subsequent player changes remain authoritative.

The new logs distinguish pre-init script preparation, managed-overlay
header/payload delivery and VIDEO GET. A green CI validates only the
build and bounded substitutions, **not yet the visible High dropdown**.

**Not yet validated against real GUI / actual display option semantics.**
No user files should be modified in-place by the script patch.
This is experimental and does not apply to the legacy monolithic DLL
or the 1.0 release. PR remains draft.

## Split loader WinMM export-ABI hardening — fail closed (experimental)

The native WinMM ABI audit from PR #42 identified 177 ordinal mismatches
among 178 matching exported names. The earlier split loader reused the
monolithic `winmm_full_exports.def`, which mapped 185 named functions
to arbitrary slots @1..@185; a third-party module importing e.g.
`winmm.dll` **by ordinal** could therefore silently jump to the
wrong function with an incompatible ABI.

The split loader now uses an **independent** `winmm_loader_exports.def`:

- All **185 named** WinMM exports still exist; the approved game imports
  its 14 WinMM APIs **by name** and therefore does not depend on their
  numeric ordinals.
- Their numeric aliases now occupy **@2048..@2232**. The common OS
  ordinal interval **@1..@1023** remains unoccupied. A dependent module
  importing an ordinal in this range will explicitly fail to resolve
  rather than call another, unrelated multimedia function.
- `loader-winmm/tools/verify_split_exports.py` audits the actual PE32
  export table, rejects any occupied slot @1..@1023, missing/wrong named
  aliases or unexpected named exports. The synthetic x86 load host also
  requires GetProcAddress by representative low ordinals to fail on
  **real Windows and Wine32**.
- This does **not** implement OS-accurate forwarding by ordinal, nor
  promise correct execution for arbitrary third-party DLLs. A module
  requiring the system's ordinal ABI may now fail to load **explicitly**,
  which is preferable to silently corrupting memory. Ordinal
  compatibility remains a **release blocker** under issue #32.

### Remaining loader-lock hazard — NOT solved by this change

`winmm.dll!DllMain` currently authenticates a fixed PE import structure
and modifies exactly two DirectDraw IAT pointers using VirtualProtect.
The full executable hash and the full runtime installation wait until
the first intercepted DirectDraw function. However, **named WinMM
exports** can be invoked by other DLLs during their own DllMain, at which
point the proxy's first-use `system_winmm()` resolver may call
`LoadLibraryW` under the Windows loader lock. The split loader only
moves the game-fix runtime LoadLibrary out of its **own** DllMain; it
does **not** prove all export forwarding is loader-lock safe.

The automated **Win32 dependent-DllMain regression** now builds a
standalone third-party x86 DLL that links against `winmm.dll` and calls
`timeGetTime` directly from its own `DLL_PROCESS_ATTACH`. A separate
fixture variant calls the **171-name extra-export forwarding** path
(`joyGetNumDevs`) *first*, then `timeGetTime`. Each variant is loaded
in a **fresh process** with no prior WinMM module, using a watchdog
(30 seconds on native Windows, 45 seconds on Wine32). The host verifies
the callback ran, the exact local proxy export is present, and the
synthetic non-game image remained unhooked.

A passing result covers these specific first-use calls, loader-order
conditions and CI platforms only. **It does not prove loader-lock
safety in general**. Microsoft explicitly forbids `LoadLibraryW` from
`DllMain` (directly or indirectly), and our lazy forwarding still
may execute it while a dependent DLL holds that lock:
https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices

Keep this prototype **DRAFT / TEST ONLY** pending a provider strategy
that is safe even under loader lock, a full dependency inventory, and
repeated actual game tests. Neither a green synthetic DLL test nor a
green ordinal fail-closed export gate authorizes a 1.1 release.

## Known limitations

- First-use WinMM named-export forwarding can still call system
  `LoadLibraryW` during dependent-module initialization, possibly under
  the Windows loader lock. **Not production ready.**
- The nonexperimental 1.1 WinMM export ordinals remain noncanonical.
  Do not deploy to unknown dependent DLL sets.
- Windows-native gameplay, Wine/Proton mission tests, 3D aspect/HUD, audio,
  cinematics, Alt+Tab, shutdown and save persistence still need owner validation.
- No automatic migration of a pre-existing flat installation is performed.
  On failures the old install should be left intact.

No release or merge should be made on CI-only results.
