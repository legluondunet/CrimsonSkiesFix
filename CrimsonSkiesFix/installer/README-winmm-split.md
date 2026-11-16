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

The new split-only experimental workaround uses two matching guards:

1. An exactly hashed, unmodified source archive and original managed
   overlay + immutable official backup must verify. When the current
   game lighting value is the initial Medium and the local INI has
   no completed-once marker, build an exact-size, **memory-only** VIDEO
   variant with the one proven script assignment changed to `HUA = 3`.
   Only the original authenticated `VIDEO.SCRIPT` is eligible.
2. The existing verified ROFFILE.DLL `ReadFile` observer substitutes
   the compressed bytes from this exact variant when the actual patched
   overlay's VIDEO payload is read (Windows observed route), or when
   the fallback original ROF source is read. Original bytes, file
   handles, reported sizes, return values, archive files, and
   compressed-member offsets remain unchanged.
3. Only **after the modified script bytes are observed being delivered**
   will the original GOS `uiData(0x855,0)` read be allowed to change the
   corresponding pending Lighting Quality field from Medium 2 to High 3.
   The original menu, selections, Accept and Cancel handlers remain in
   charge. Any other starting value is preserved.
4. Only a verified successful Apply appends
   `[Compatibility] CrimsonSkiesFixLightingHighDefaultApplied=1` to
   the existing INI. A later launch never reapplies the forced default
   and the player can subsequently change Lighting Quality normally.

The newly added log lines report script preflight, whether the actual
ROF payload was delivered, and whether the menu pending lighting value
was changed. They can help distinguish a wrong script routing result
from an incorrect visible dropdown.

**Not yet validated against real GUI / actual display option semantics.**
No user files should be modified in-place by the script patch.
This is experimental and does not apply to the legacy monolithic DLL
or the 1.0 release. PR remains draft.

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
