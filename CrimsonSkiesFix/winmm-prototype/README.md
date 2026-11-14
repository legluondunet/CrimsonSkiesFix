# Experimental WinMM bootstrap for CrimsonSkiesFix

This is a standalone **Windows x86 WinMM proxy prototype**, **not** part of the
v1.0 release and **not** installed by `CrimsonSkiesFix.exe`.

The exact supported Crimson Skies 1.02 executable statically imports 14 WinMM
functions and 2 DirectDraw functions. This prototype tests forwarding those 14
WinMM functions to the real 32-bit system WinMM DLL, using x86 assembly shims
which preserve the caller's stack, arguments, registers and calling convention.

## Purpose

- Establish whether a local `winmm.dll` can be loaded and used on Windows.
- Record the **first WinMM export actually invoked** in a dedicated file
  `CrimsonSkiesFix_winmm_probe.log` next to the executing program.
- Observe the proxy path, system WinMM path and DirectDraw module path at that moment.
- Keep the current `ddraw.dll` and every graphics hook **unchanged**.

The probe runs on first forwarded export. Static imports do **not** establish
the first API call order: WinMM might be called after DirectDraw.

## Limitations and safety

**Do not put this prototype into a normal game installation or release.**
It exports only 14 *named* APIs imported by the approved game executable, not
all the WinMM functions potentially needed by other DLLs. Test ordinals 1–14
are not canonical system ordinals. An additional DLL may load WinMM by ordinal
or import other functions and fail.

Native WinMM resolution is performed lazily on the first API call, not in an
explicit `DllMain` routine. A first call could itself originate from another
DLL's initialization, so we **have not yet proven it is outside the loader
lock**. Production initialization must explicitly resolve that risk.

This prototype does not modify the game's PE, IAT, COM interfaces or memory.
It only writes an opt-in diagnostic file. The known-working `ddraw.dll` has
not been removed or changed.

Windows can prefer a system KnownDLL or previously loaded module over an
application-local DLL. Wine may require a per-process native override such as
`WINEDLLOVERRIDES=winmm=n,b`. Neither case is assumed to work automatically.

## Automated verification

The separate GitHub Actions workflow
[`winmm-bootstrap-prototype.yml`](../../.github/workflows/winmm-bootstrap-prototype.yml)
builds `i686-pc-windows-msvc`, audits 14 exact Windows x86 exports, and runs
an x86 smoke host against a real Windows system WinMM. It publishes a
**temporary, clearly labeled prototype artifact** only after passing.

Source-only verification:

```sh
python3 CrimsonSkiesFix/winmm-prototype/tools/verify_winmm_exports.py
```

## Later game validation (not part of this prototype)

When authorized, use a **disposable copy** of the verified game installation,
leave the existing CrimsonSkiesFix `ddraw.dll` in place and test whether the
experimental local `winmm.dll` logs its first call on Windows and Wine.
Avoid persistent global Wine overrides. A gameplay session is required to
verify multimedia behavior, call timing and any dependent DLL exports.

Only once a safe early initializer and renderer-independent DirectDraw hooking
are demonstrated should the installer be changed to free the `ddraw.dll`
filename for native DirectDraw or an optional D7VK wrapper.

[Audit PR #33](https://github.com/legluondunet/CrimsonSkiesFix/pull/33) ·
[Migration tracking issue #32](https://github.com/legluondunet/CrimsonSkiesFix/issues/32).
