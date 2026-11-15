# CrimsonSkiesFix WinMM ordinal-bridge experiment

**TEST ONLY. NOT THE v1.1 RELEASE.**

The executable in this artifact is
`CrimsonSkiesFix-WinMM-Ordinal-Bridge-TEST.exe`. It packages the experimental
WinMM runtime with a separate table for ordinal-only entries **1..1023** and
185 name-addressable imports at **2048..2232**. It is not the standard WinMM
1.1 test launcher and it is not the published 1.0 installer.

The ordinary 1.0 release and the default 1.1 candidate build remain unchanged.

## What the Windows CI has established

A standalone Windows **32-bit** smoke host loaded both:
- the actual Windows system WinMM provider; and
- the app-local experimental WinMM DLL by its verified full path.

It called `timeGetTime` through both exported name and the same ordinal found
in the actual system provider. The resulting values were consistent. A second
host exercised app-local WinMM named imports and synthetic bootstrap rejection.
This does **not** prove the game is compatible or that Wine ordinal forwarding
is correct. Dynamic third-party modules, loader lock, crashes and UI are
not validated by the small smoke program.

## If conducting an owner A/B gameplay test later

**Only on a disposable complete copy** of the *approved* Crimson Skies 1.02
installation. Do not run this candidate over your working installation.

- The copied game directory must have **no local `ddraw.dll`**; that slot
  remains deliberately reserved for the actual Windows/Wine backend.
- The installer may replace an existing `winmm.dll` and preserve it as the
  single `winmm.dll.bak`, according to the existing experimental migration
  rules. Use a disposable copy rather than relying on backup rotation.
- Confirm `--version` prints **`ORDINAL-BRIDGE-TEST`**. Stop if it does
  not. The game EXE and original CD data must be supplied by the owner.
- Under Linux, use a process-local `WINEDLLOVERRIDES='winmm=n,b'` (never
  a global override), and keep the existing Wine/Proton prefix.
- Do not merge the prototype solely on a successful launch: verify game
  sound, full mission, cinematics, 4:3 menus, FOV, DPI, Alt+Tab, shutdown,
  and both Log=0 and Log=1.

All graphical corrections remain those of the merged WinMM 1.1 candidate.
The experimental change is only in the WinMM import/export ABI path.

Remaining blockers: actual Wine/Proton functional validation, ordinal coverage
beyond 1023, loader-lock auditing, safe existing-install migration, and
compatibility of dependent DLLs. Release 1.1 is still blocked.
