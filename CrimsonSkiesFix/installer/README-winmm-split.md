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
