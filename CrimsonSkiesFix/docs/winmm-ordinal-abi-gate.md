# WinMM ordinal ABI gate for CrimsonSkiesFix 1.1

Status: **unresolved / release blocker**. This is a design and evidence note,
not approval to publish the WinMM candidate or install it over v1.0.

## Measured evidence (Windows x86 CI)

The mainline WinMM candidate exports 185 WinMM names with prototype
ordinals `1..185`. Our added native Windows x86 audit runs against the
runner's **real** `%WINDIR%\\SysWOW64\\winmm.dll` (PE32/i386), using
`runtime-winmm/tools/audit_system_winmm_abi.py`:

- 178 names present on both sides.
- 177 shared names have different numeric ordinals.
- 185 occupied ordinal slots conflict (including slots not published by name).
- The system DLL exposes `PlaySound` as ordinal **11** and has an
  **unnamed** ordinal 2. Our experiment assigning `PlaySound @2` was
  reverted rather than silently corrupting Windows ordinal consumers.

Reference: WinMM runtime CI run 43771119477, job `winmm-runtime-x86`.
Its `winmm-system-abi.json` is packaged alongside the experimental
WinMM DLL. The workflow is successful because the **audit ran**, not
because the WinMM ABI matched. The script can enforce the stricter
condition using `--strict`.

## Why a single hard-coded mapping cannot be assumed safe

Wine's `dlls/winmm/winmm.spec` declares ordinal **2** for PlaySoundA /
PlaySound and its `dlls/winmm/tests/wave.c` checks
`GetProcAddress(winmm, (LPCSTR)2) == GetProcAddress(winmm, "PlaySound")`.
That assumption does not match the Windows x86 DLL measured by our CI.

The game executable itself imports 14 WinMM APIs **by name**. Those calls
pass current smoke tests, which **do not** cover another game dependency
that might import WinMM by ordinal. A third-party ordinal import may
resolve to a **different, ABI-incompatible function**, causing silent stack
corruption instead of a predictable import failure.

Changing only the `@2` entry in the `.def` file cannot fix this.
Similarly, rearranging 185 names to one Windows build's numbered exports
would not establish Wine compatibility.

## Candidate designs to investigate, without shipping

1. **Provider-specific ordinal forwarding:** preserve name-based
   forwarding, and introduce separate numeric-only x86 tail-jump
   trampolines that resolve the *same ordinal* in the real WinMM provider
   selected by Windows or Wine. Test the full import surface, alias names,
   absent ordinals, exact DLL origin, loader lock and process exit before
   using this as a compatibility guarantee. Reserved numeric exports must
   not accidentally map to the 185 name-based trampolines.
2. **Launcher-controlled bootstrap:** avoid replacing `winmm.dll` at
   all; load an unambiguously named runtime from the authenticated
   launcher and install only game-scoped hooks outside unsafe loader
   callbacks. This is more invasive and requires real Windows/Proton
   prototype testing before changing the existing architecture.

Both options must retain our original game's hash gate, shared
Windows/Wine runtime, borderless 4:3 menus/films, upstream camera ratio
fix, stable audio, native DPI handling, Alt+Tab, and the user's INI.

## Release acceptance criteria

- No static ordinal-to-unrelated-function collisions against **both**
  actual Windows x86 and Wine providers.
- No recursion through the app-local proxy when resolving the real module.
- Native Windows and Proton full-mission tests including audio, menu,
  movie, Alt+Tab and exit; no dependence on `[Debug] Log=1`.
- Loader-lock safe bootstrap and safe migration of hash-recognized
  v1.0 `ddraw.dll`; unknown wrappers remain byte-for-byte untouched.
- The published v1.0 build and data stay unchanged until all gates pass.

A green CI audit step is **not** a release gate by itself.
