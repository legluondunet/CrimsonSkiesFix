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

## Ordinal-usage evidence in the revised experimental runtime

When the player explicitly enables `[Debug] Log=1`, the DLL records
`WINMM-ORDINAL-BRIDGE: stage=DD4-SetDisplayMode resolved_unique=... ordinals=...`
at each authenticated DirectDraw mode transition, **outside DllMain**.

The resolver records only successful first uses in a lock-free atomic bitmap.
It never writes log files, allocates memory or invokes new user callbacks when
recording the ordinal. `ordinals=none` means no numeric thunk was successfully
resolved **before that snapshot**; it does **not** exclude future calls,
dynamic `GetProcAddress` calls to names or ordinals, or a second DLL bypassing
this proxy. `[Debug] Log=0` still disables the gameplay log entirely.

The previous GE-Proton game log from 15 November (`71345d9e`) predates
this read-only telemetry. It confirms the game ran but not which numeric
thunks (if any) were used. Only use a newer build with this feature to
inspect actual ordinal-use evidence under GE-Proton.

## Named WinMM call-order evidence (issue #32)

The updated opt-in prototype also records the **first attempted resolution**
among the game's 14 statically imported WinMM names. Resolution attempts
record an atomic bit **before** calling the native system-WinMM provider;
successes are counted separately, after `GetProcAddress` succeeds.

At the initial game DirectDraw hook and each subsequent DD4 mode change, an
ordinary `Log=1` game session prints a line such as:

```text
WINMM-NAMED-ORDER: stage=DD4-SetDisplayMode first=timeGetTime first_before_ddraw=true attempted_unique=1 resolved_unique=1 before_ddraw=timeGetTime
```

- `first_before_ddraw=true`: a named WinMM thunk attempted resolution
  before the **first observed CrimsonSkiesFix DirectDraw hook**.
- `before_ddraw=none`: no named WinMM first-resolution attempt was recorded
  before that hook; this is NOT proof that the OS loader lock was absent.
- `attempted_unique` and `resolved_unique`: unique game-import names
  attempted versus successfully mapped to the native provider, not call totals.
- This does **not** detect all loads, WinMM calls that bypass our proxy,
  anonymous ordinal thunks, or loader lock ownership. The Linux/Windows
  loader may call an imported function in another DLL's `DllMain` before
  our normal game initialization; that remains an explicit release blocker.

The probe performs **no logging, filesystem access, allocation, native API
resolution or new library loading** in the first-use marker. All string
formatting happens in the normal DirectDraw game path, outside our `DllMain`.
It is diagnostic only and is not enabled in the regular 1.0 or 1.1 builds.

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
