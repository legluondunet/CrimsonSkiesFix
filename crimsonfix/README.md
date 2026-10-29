# CrimsonFix 0.1.1 — experimental DirectDraw proxy

**Status:** x86 DirectDraw proof of concept; game rendering, borderless
and FPS pacing still require live Windows/Wine validation. This PR does
not modify the original EXE, the main Rust patcher or unSafeDisc.

## Installation — only one file at the game root

Extract the Windows x86 artifact into **a copy** of the game directory
containing the already-unwrapped Crimson Skies 1.0.2.0. The intended
layout is:

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonFix/
    CrimsonFix.ini
    README.md
    CrimsonFix.log         # created ONLY when [Debug] Log=1
```

**Only ddraw.dll lives beside the EXE**. Do not replace any DLL in the
Windows or Wine system directories. Both Windows and Wine use the same
32-bit Windows DLL, not a Linux .so. If Wine loads its own builtin
ddraw.dll, set `WINEDLLOVERRIDES="ddraw=n,b"`.

If you used the old v0.1 proof of concept that placed CrimsonFix.ini
at the game root, it is copied **once** into CrimsonFix/ when the new
nested INI does not already exist. The old root INI is preserved and
the nested INI takes priority. New archives already contain the nested
file and do not require migration.

## Configuration

```ini
[Graphics]
Borderless=1
DesktopResolution=1
FPSLimit=60

[Debug]
Log=0
```

- `Borderless=1`: experimental DDSCL_NORMAL cooperative mode plus a
  borderless window. Some DirectDraw flip chains may not support this.
- `DesktopResolution=1`: preserve desktop mode, choose the display
  under the cursor at DirectDraw initialization. The game render target
  and FOV **are not yet automatically resized**.
- `FPSLimit=60`: clamp frame pacing to 1..60 FPS. Currently covers
  `Flip` and primary `Blt` paths only, so the entire game may not
  actually be limited until additional rendering paths are captured.
- `[Debug] Log=0`: logging is **fully disabled by default**; no
  log file is created or modified. Set `Log=1` to create/overwrite
  `CrimsonFix/CrimsonFix.log` at the next game startup.

## Private-by-default diagnostics (reused from Spider-Man)

When the user explicitly enables `Log=1`, initialization logs one
privacy-filtered `SYS-INFO: BEGIN ... END elapsed_ms=...` snapshot:

- Native Windows vs Wine, Win32 version/build (emulated under Wine),
  Wine version and host OS/kernel if exported by Wine.
- Proton/Steam compatibility **hints** if exported; never print raw
  Steam paths or claim the inferred Proton version is authoritative.
- CPU model, physical/logical processors and Win32-reported RAM.
- Active monitors, resolution, refresh and adapter names.
- D3D9-reported GPU model, vendor/device IDs, graphics driver
  module/version, never misrepresent Wine/DXVK as the host Linux
  Mesa/AMDGPU/NVIDIA kernel driver.
- Passive XInput and DirectInput8 gamepad enumeration: subtype or
  device model/VID:PID, no device acquisition or input state reads.
- Desktop/session environment hints if available.

No usernames, hostnames, home paths, process command lines, IPs, MACs,
serials, EDID, Steam account identifiers or unrestricted environment
values are recorded. Missing APIs yield `Unavailable`; no WMI worker,
shell command, gamepad `Acquire` or D3D device creation is used.

Inventory runs at most once, after DirectDraw first initializes,
**never inside DllMain**. Log files remain local and are not uploaded.

This is adapted (with attribution, under GPL-3.0-or-later) from the
`system_diagnostics.c` module of
[SpiderMan2k1Fix](https://github.com/legluondunet/SpiderMan2k1Fix),
developed in PR #83 and included in the working branch used by PR #80.
The isolated installation layout follows SpiderMan2k1Fix PR #81.

## Troubleshooting

If the game displays incorrectly, change `Borderless=0` and test
again. If more detail is needed, set `Log=1` and examine the log in
the `CrimsonFix/` folder. Reset `Log=0` afterward.

The standalone proxy still forwards `DirectDrawCreate`,
`DirectDrawEnumerateA` and `DirectDrawCreateEx` to the real Windows
system DLL. The DirectDraw v1 COM hooks are experimental; newer COM
interfaces reached via QueryInterface are not yet intercepted.
Runtime behavior requires testing under Wine/Windows.

## Build and PR policy

```powershell
rustup target add i686-pc-windows-msvc
cd crimsonfix
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

The GitHub Actions artifact contains the game-ready layout. No
copyrighted game file, memory dump or original system DLL is included.
Keep this PR in draft until the owner validates a real gameplay session.
