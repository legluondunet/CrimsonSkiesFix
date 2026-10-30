# CrimsonFix 0.1.6 — experimental DirectDraw proxy

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
    CrimsonFix.ini.example # shipped with the ZIP; safe to overwrite
    README.md
    CrimsonFix.ini         # created once on first launch, NEVER shipped
    CrimsonFix.log         # created ONLY when [Debug] Log=1
```

**Only ddraw.dll lives beside the EXE**. Do not replace any DLL in the
Windows or Wine system directories. Both Windows and Wine use the same
32-bit Windows DLL, not a Linux .so. If Wine loads its own builtin
ddraw.dll, set `WINEDLLOVERRIDES="ddraw=n,b"`.

**Updates preserve user settings.** The ZIP contains
`CrimsonFix/CrimsonFix.ini.example`, **not** `CrimsonFix/CrimsonFix.ini`.
Simply extract the ZIP over an existing game installation: only the
example and DLL may be replaced. Your existing
`CrimsonFix/CrimsonFix.ini`, including `[Debug] Log=1`, is untouched.

For a **new install**, the Rust DLL creates `CrimsonFix/CrimsonFix.ini`
with defaults the first time DirectDraw initializes; it uses an atomic
`create_new` file open, which never truncates or replaces an
existing INI. If the game directory is read-only, in-memory defaults
still work but no INI will be created.

If you used the old v0.1 proof of concept that placed
`CrimsonFix.ini` in the game root, it is migrated **only when the
nested INI is absent**. The original root file is preserved. If
migration fails, the old root configuration is still read rather
than silently dropping the user's settings.

To enable logs for debugging, edit the **active**
`CrimsonFix/CrimsonFix.ini`, not the `.example` template.
Set `[Debug] Log=1` and relaunch. Future updates no longer reset
it to `Log=0`.

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

The inventory is now **100% Rust**:
`src/diagnostics.rs` gathers Windows/Wine, Proton hints, CPU/RAM and
monitor/session information; `src/diagnostics_graphics.rs` gathers
passive Direct3D9 GPU and XInput/DirectInput8 device information using
Win32/COM FFI. The old C file, C header, build script and `cc`
dependency have been removed; `ddraw.dll` remains a single x86 Rust
`cdylib` with no separate native diagnostics module.

This Rust implementation is adapted (with attribution, under
GPL-3.0-or-later) from the `system_diagnostics.c` source of
[SpiderMan2k1Fix](https://github.com/legluondunet/SpiderMan2k1Fix),
developed in PR #83 and included in the working branch used by PR #80.
The isolated installation layout follows SpiderMan2k1Fix PR #81.
The source contains Rust ABI-layout and privacy-oriented tests; actual
Wine/Windows reporting still needs owner testing.

## Troubleshooting

If the game displays incorrectly, change `Borderless=0` and test
again. If more detail is needed, set `Log=1` and examine the log in
the `CrimsonFix/` folder. Reset `Log=0` afterward.

After the first Wine test, a successful DirectDraw proxy installation
was confirmed but the initial borderless attempts reported an
unclassified `HWND adjustment failed`. New diagnostics now record
whether `SetCooperativeLevel` received a **NULL** window, a non-game
HWND, bad monitor geometry, or a precise Win32 error from
`GetWindowLongW`, `SetWindowLongW`, or `SetWindowPos`.
A NULL HWND leaves the window unchanged and the borderless adjustment
deferred until another cooperative-level call supplies a valid window.
The log intentionally avoids raw handle values.

A `Flip paced` or `Blt paced` line alone means the vtable hook was
**installed**; it does not prove that the game called it. The first
actual call generates a one-time `PRESENT:` marker. If no
`PRESENT:` marker appears during 3D gameplay, the 60 FPS limit remains
unverified and additional graphics paths must be investigated.

## Diagnostic follow-up — DirectDraw 2/4/7 and Surface 2/3/4

The owner-provided v0.1.5 log identifies the exact requested interfaces:
`IDirectDraw2`, `IDirectDraw4`, `IDirectDraw7`, and
`IDirectDrawSurface2`, `IDirectDrawSurface3`, `IDirectDrawSurface4`.
All queries succeeded. The game uses COM interface upgrades that can
bypass the v1-only presentation hooks. The initial `WIN-TRACE` captured
2 then 4 process windows, **none yet visible**, at `HWND=NULL` calls.
One v1 surface `Unlock` was observed; v1 `PRESENT:` was absent.

In v0.1.6, the diagnostic hooks **follow successful QueryInterface**
on these known versions and the `IDirectDraw7` returned directly by
`DirectDrawCreateEx`, preserving the original IID, output and HRESULT.
Modern `SetCooperativeLevel` and the five-argument
`SetDisplayMode` are traced with the correct ABI and forwarded
unchanged. Modern `CreateSurface` and surface `Flip`, `Blt`,
`BltFast`, `Unlock` are traced with exact COM signatures.
New messages start with `MODERN-DDRAW:`, `MODERN-SURFACE:`,
`RENDER-MODERN:` and are rate-limited or one-time-only. **No
FPS pacing, borderless adjustments, mode suppression or new
window selection are applied to these newer interfaces** until
gameplay confirms which calls represent actual presentation.
With `Log=0`, the extra modern-interface observation is disabled.

## Diagnostic follow-up after the first in-game Wine log

The user-supplied Wine log shows `SetCooperativeLevel(DDCSL_NORMAL)` is
called **twice with HWND=NULL**, so the current borderless change is
deferred. It also shows hooks installed on `Flip` and primary `Blt`,
but **no actual `PRESENT:` calls**, so 60 FPS pacing and native game
rendering resolution remain unverified.

In v0.1.5, when `[Debug] Log=1` is enabled, bounded read-only
`WIN-TRACE:` entries count visible/top-level windows owned by the
game process (without printing titles, HWNDs or other processes'
windows). They are collected only during DirectDraw initialization,
not every frame. Bounded `COM-TRACE:` entries capture requested
DirectDraw and DirectDrawSurface interface GUIDs from QueryInterface,
without altering the returned object. One-time `RENDER-TRACE:`
markers detect `BltFast` and `Unlock` on v1 surfaces; **these methods
are NOT FPS-paced** because their calls do not necessarily correspond
to displayed frames. The existing borderless/window behavior remains
unchanged: the proxy does not guess a window or force a new mode
without first establishing the renderer's actual object path.

The trace is fully disabled with `Log=0` and is deliberately bounded.

Some Wine/DXVK D3D9 drivers report the sentinel version
`32767.65535.65535.65535`; CrimsonFix now labels that
`Unavailable` rather than claiming a genuine driver version.

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
