# CrimsonFix 0.1.9 — experimental DirectDraw proxy

**Status:** x86 DirectDraw proof of concept; the 60-calls/s limiter on
modern primary Flip was measured under Wine, but gameplay simulation
speed and borderless still require live validation. This PR does
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
ModernFPSPacing=0

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
- `ModernFPSPacing=0`: modern primary Flip pacing **disabled by default**;
  set `1` only for a controlled experimental FPS test. This does not
  change full-screen mode or the in-game rendering resolution.
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

## v0.1.9 — first primary/backbuffer chain inspection

The v0.1.8 game test with `ModernFPSPacing=1` demonstrated
actual DirectDraw4 primary-surface `Flip` pacing: three consecutive
120-call samples took about 2000 ms each (~60 calls/sec), versus
~1000 ms each (~120 calls/sec) when disabled. This confirms that the
**COM API call limiter** works in the user's Wine/GE-Proton session.
Normal gameplay speed and animation timing remain unverified; this
does **not** establish 60 unique 3D frames on screen.

The primary surface's capabilities include
`DDSCAPS_PRIMARYSURFACE`, `DDSCAPS_FLIP` and
`DDSCAPS_COMPLEX`. These signify a hardware-style front/backbuffer
chain which cannot be preserved by a naive switch to `DDSCL_NORMAL`
and skipping `SetDisplayMode`. The actual exclusive HWND resizes to
640x480 or 800x600, 16 bpp. A borderless 1920x1080 *window* alone
would not yield 1920x1080 3D rendering.

This new version instruments the next two critical COM operations
**without changing their behavior**:

- `SURFACE-ATTACH` records a successful GetAttachedSurface request,
  parent and returned surface DDSCAPS roles, HRESULT and whether a
  backbuffer object was returned, without writing raw COM pointers
- `SURFACE-CHAIN` marks a proven primary -> attached backbuffer
  association, if the game requests it
- `SURFACE-DESC` records the descriptor dimensions and DDSCAPS when
  the game itself invokes GetSurfaceDesc (the hook does not allocate
  pixels or call Lock)
- The returned attached object receives the same bounded modern
  diagnostic hooks for subsequent Flip/Blt calls

Tracing is bounded (first 24 attachments and 20 descriptions), is
opt-in via `[Debug] Log=1`, and does not interfere with the existing
experimental `ModernFPSPacing` choice. The package still ships only
the non-overwriting `CrimsonFix.ini.example`.

For the next run, **keep `ModernFPSPacing=0` first** to isolate the
instrumentation. If the game runs normally and the chain is
identified, the next branch can design a separate, explicitly opt-in
windowed presentation adapter with original rendering resolution
retained internally.

## v0.1.8 — interval call rates and surface/viewport observation

The user's in-game v0.1.7 log confirms the game supplies a visible,
game-owned `HWND` at 1920x1080 **before** entering exclusive DirectDraw
mode. It later switches between 640x480 and 800x600 at 16 bpp.
The new sampling data shows `Flip` and `Blt` call rates near 30/s
in some intervals, while a burst of `Unlock` calls reaches more than
1,000/s; the whole-session averages are misleading. These call rates
are **not** validated display FPS.

v0.1.8 adds:
- Bounded `RENDER-WINDOW:` entries every 120 calls, showing calls/s
  over each short interval rather than since the program started
- `SURFACE-CREATE:` details (descriptor size, explicitly specified
  width/height and post-creation capability bits) for up to 24 surfaces;
  read-only, no pixel readback or changes to surfaces
- `SURFACE-TRACE:` with the capability bits on the first modern
  `Flip` and `Blt` destinations, allowing confirmation of
  `DDSCAPS_PRIMARYSURFACE` and flip-chain usage
- `WIN-TRACE:` dimensions/style of the actual game HWND **after**
  successful or failed modern SetDisplayMode calls (up to 12 samples)

All these diagnostics are opt-in behind `[Debug] Log=1`. The game
is still launched with its original DirectDraw display-mode behavior.
`ModernFPSPacing=0` remains the safest default; modern primary Flip
pacing is still an optional experiment pending classification of the
actual first Flip surface.

**Important:** DirectDraw's exclusive/fullscreen flip chains cannot
be made truly borderless just by removing a window border or replacing
`DDSCL_EXCLUSIVE` with `DDSCL_NORMAL`. A successful future patch must
maintain a compatible presentation chain and decouple game resolution
from desktop resolution. Nothing in v0.1.8 changes this yet.

## v0.1.7 — confirmed modern render calls and guarded FPS experiment

The v0.1.6 Wine log confirms that the game **really calls newer-surface
Flip and Blt**, while DirectDraw4 changes display modes between
`640x480x16` and `800x600x16`. The game passes a valid HWND and requests
flags `0x13`, corresponding to exclusive full-screen mode
(`DDSCL_FULLSCREEN | DDSCL_ALLOWREBOOT | DDSCL_EXCLUSIVE`).
Previously invisible windows are **not** evidence that there is no
game window: those snapshots were captured earlier in initialization.

This build logs one bounded `WIN-TRACE` snapshot of the **first
non-NULL HWND actually passed by the game**, including visibility,
outer/client dimensions and style flags (never the window title or raw
HWND). Modern `Flip`, `Blt`, `BltFast` and `Unlock` hooks collect
cumulative call counts. At 10, 60, 300 and 1200 calls they emit
`RENDER-SAMPLE` messages with elapsed time and approximate calls/sec.
**These are API-call rates, not verified rendered FPS**, particularly
for Blt and Unlock.

An **optional, experimental** real FPS limiter can now be tried on
newer DirectDraw interfaces. Add `ModernFPSPacing=1` under `[Graphics]`
in the existing active `CrimsonFix.ini`. It applies `FPSLimit=60`
(or another existing 1–60 setting) only to newer-surface **Flip**
calls whose `GetCaps` confirms `DDSCAPS_PRIMARYSURFACE`. It does not
throttle offscreen surfaces, `Blt`, `BltFast` or `Unlock`, which
can occur multiple times per frame. Set `ModernFPSPacing=0` to
disable immediately on the next game launch. **Default is 0 to
preserve the current working game behavior**. This experiment has
passed compilation/tests but has NOT yet been validated in-game.

The `Borderless` and `DesktopResolution` settings still affect
only the old DirectDraw v1 interception; no borderless or resolution
patch has yet been applied to DirectDraw4/7. Merely resizing a window
while the game is in exclusive 16-bit mode would not constitute
native 1920x1080 rendering or widescreen FOV correction.

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
