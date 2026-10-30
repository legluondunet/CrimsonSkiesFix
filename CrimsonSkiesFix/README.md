# CrimsonSkiesFix 0.1.16 — experimental DirectDraw proxy

**Status:** x86 DirectDraw proof of concept; the 60-calls/s limiter on
modern primary Flip was measured under Wine, but gameplay simulation
speed and borderless still require live validation. This PR does
not modify the original game executable.

## Installation — only one file at the game root

Extract the Windows x86 artifact into **a copy** of the game directory
containing the already-unwrapped Crimson Skies 1.0.2.0. The intended
layout is:

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log           # created ONLY when [Debug] Log=1, beside ddraw.dll
  CrimsonSkiesFix/
    CrimsonSkiesFix.ini.example # shipped with the ZIP; safe to overwrite
    README.md
    CrimsonSkiesFix.ini         # created once on first launch, NEVER shipped
```

**Only ddraw.dll lives beside the EXE**. Do not replace any DLL in the
Windows or Wine system directories. Both Windows and Wine use the same
32-bit Windows DLL, not a Linux .so. If Wine loads its own builtin
ddraw.dll, set `WINEDLLOVERRIDES="ddraw=n,b"`.

**Legacy INI migration:** The active configuration is now
`CrimsonSkiesFix/CrimsonSkiesFix.ini`. On first launch, the DLL
**copies** the existing `CrimsonFix/CrimsonFix.ini`, or earlier
`CrimsonFix.ini` in the game root, only if the new INI does not exist.
The old INI is never deleted. Existing `Log=1` and FPS settings
are preserved. The new `CrimsonSkiesFix.log` is created **beside the
loaded `ddraw.dll`**, not in the config folder; old logs are untouched.

**Updates preserve user settings.** The ZIP contains
`CrimsonSkiesFix/CrimsonSkiesFix.ini.example`, **not** `CrimsonSkiesFix/CrimsonSkiesFix.ini`.
Simply extract the ZIP over an existing game installation: only the
example and DLL may be replaced. Your existing
`CrimsonSkiesFix/CrimsonSkiesFix.ini`, including `[Debug] Log=1`, is untouched.

For a **new install**, the Rust DLL creates `CrimsonSkiesFix/CrimsonSkiesFix.ini`
with defaults the first time DirectDraw initializes; it uses an atomic
`create_new` file open, which never truncates or replaces an
existing INI. If the game directory is read-only, in-memory defaults
still work but no INI will be created.

If you used the old v0.1 proof of concept that placed
`CrimsonSkiesFix.ini` in the game root, it is migrated **only when the
nested INI is absent**. The original root file is preserved. If
migration fails, the old root configuration is still read rather
than silently dropping the user's settings.

To enable logs for debugging, edit the **active**
`CrimsonSkiesFix/CrimsonSkiesFix.ini`, not the `.example` template.
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

- `Borderless=1`: currently changes only the legacy DirectDraw v1
  initialization behavior. **It does not convert Crimson Skies' real
  DirectDraw4 exclusive 16-bit flip chain to borderless.** The actual
  borderless adapter is not yet active; this option is not proof that
  the game is windowed.
- `DesktopResolution=1`: preserve desktop mode, choose the display
  under the cursor at DirectDraw initialization. The game render target
  and FOV **are not yet automatically resized**.
- `FPSLimit=60`: select a pacing target of 1..60 calls/s; the game's
  **modern DirectDraw4 presentation path only uses this setting when
  `ModernFPSPacing=1`**. With `ModernFPSPacing=0`, 120 `Flip`
  calls/s on a 120Hz display are expected, as verified in gameplay.
- `ModernFPSPacing=0`: modern primary Flip pacing **disabled by default**;
  set `1` only for a controlled experimental FPS test. This does not
  change full-screen mode or the in-game rendering resolution.
- `[Debug] Log=0`: logging is **fully disabled by default**; no
  log file is created or modified. Set `Log=1` to create/overwrite
  `CrimsonSkiesFix.log` at the next game startup.

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
again. If more detail is needed, set `Log=1` and examine `CrimsonSkiesFix.log` beside `ddraw.dll`. Reset `Log=0` afterward.

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

## v0.1.16 — exact Direct3D3 scene/Flip correlation

The owner supplied three genuine v0.1.14 runtime logs:
`gamez.out`, `gamez.err`, and `CrimsonSkiesFix.log`.
The latter definitively confirms the IDirect3D3 device targets the
**RGB565 backbuffer** (DDSCAPS `0x1000601c`), with the
32-bit Z-buffer attached, in both 640x480 and 800x600 modes.
No successful `IDirect3DDevice3::SetRenderTarget` call is visible
in the bounded trace; the CreateDevice target is the only
confirmed render target in this session.

With opt-in `ModernFPSPacing=1`, the primary `Flip` API is around
60 calls/s in stable intervals, while successful
`D3D3.BeginScene` and `D3D3.EndScene` can each reach about 240
calls/s; `Blt` reaches 120 calls/s. **These earlier rates came from
independent time intervals and do not prove four scenes per Flip.**
The game's separate `gamez.out` reports a one-off
`dsh fps: 90.533806`, not a complete runtime FPS trace.
`gamez.err` reports a MCI CD-audio command error at
`zSound/zsnd_cd.cpp(413)`; its impact and relationship to the
wrapper are not yet established.

A new tested, **diagnostic-only** `scene_correlation.rs` module
groups exactly 120 **successful primary-surface Flip calls** and
measures the number of successful Direct3D3 BeginScene and EndScene
calls that happened between them. It resets when a
`SetDisplayMode` succeeds, including changes back to the same
resolution. Its maximum of 12 `SCENE-CORRELATION` log lines
reports `successful_primary_flips`, `successful_begin_scene`,
`successful_end_scene`, `begins_per_flip`, `ends_per_flip`
and the render-mode generation. This is the appropriate way to
find the number of D3D3 scene calls **per group of displayed
presentation API calls**, without assuming a scene means a frame.
The hook never locks any surface, sleeps beyond the owner's
existing pacing option, alters COM arguments, changes the game's
3D state, or modifies the original `Flip` result.

For the next test, enable `[Debug] Log=1` and keep the previous
`ModernFPSPacing` preference. Note that this build does **not**
activate borderless or native widescreen rendering.

## v0.1.15 — consistent runtime log identity

Runtime diagnostic headers and the DirectDraw recursion-guard message
use the canonical **CrimsonSkiesFix** name. A regression test ensures
these messages stay consistent. Legacy INI paths remain recognized
solely for one-time import of user settings.

## v0.1.14 — observe Direct3D3 scenes and render-target switching

The DLL source resides in `CrimsonSkiesFix/`. Its log file
`CrimsonSkiesFix.log` is written **beside the loaded `ddraw.dll`**,
while the persistent INI is
`CrimsonSkiesFix/CrimsonSkiesFix.ini`. Legacy INI paths are
considered only when the new settings file is absent.

This version extends the passive IDirect3D3::CreateDevice hook:
once the real `IDirect3DDevice3` has been created successfully, it
installs read-only monitoring on these *original* COM calls (indices
verified against the Microsoft/DirectX6 `d3d.h`):

- `BeginScene` (slot 9) and `EndScene` (slot 10):
  `RENDER-SAMPLE` and `RENDER-WINDOW` report bounded 120-call
  intervals for the **successful Direct3D scene API calls**
- `SetRenderTarget` (slot 14): a maximum of 16
  `D3D3-DEVICE` lines report whether the engine switches the
  graphics target from the backbuffer to some other surface
- `D3D3-DEVICE: hooks ...` reports the interception status;
  `D3D3-TRACE: CreateDevice ...` continues to describe the initial
  render target and the actual legacy device type

These are COM API call counts, **not guaranteed unique screen FPS**.
They do not introduce any additional frame pacing or alter render
targets, textures, the Z-buffer, COM ownership, fullscreen mode or
the game's window. If device creation fails, the invalid target is
not inspected. This instrumentation is opt-in with `[Debug] Log=1`.
The pre-existing `ModernFPSPacing` default remains 0, so a 120Hz
desktop can still have ~120 Flip calls/s; use `ModernFPSPacing=1`
only for a separate test of 60 calls/s.

The experimental BGRA output helper remains unused until a complete
windowed DDraw/D3D3 presentation backend is implemented.

## v0.1.12 — RGB565 image composer (backend foundation only)

The user's v0.1.11 Wine log confirmed that the prior pixel format and
per-mode diagnostics work as designed. The primary and backbuffer are
both **RGB565 16-bit**, with masks R=0xf800, G=0x07e0, B=0x001f,
while the attached depth surface is **Z32** (32-bit depth). Five
successive display-mode generations had their own complete
primary -> backbuffer -> Z-buffer evidence, with no reused stale
attachments. During this trace, `ModernFPSPacing=false` and the
user observed 120 FPS, which is expected; `FPSLimit=60` alone does
not enable modern DirectDraw4 pacing.

This update adds `src/rgb565_present.rs`: a self-contained, tested
CPU compositor for a **borrowed** RGB565 frame and an existing BGRA8
desktop output buffer. It supports explicit row pitches, safe
slice-bound checks, nearest-neighbor aspect-correct scaling, opaque
black pillarboxes and exact RGB565-to-8bit-channel conversion.
Its purpose is to supply the pixel conversion stage for the eventual
windowed presenter. The module has no Windows API, file IO, COM,
surface creation or game-hook side effects.

The new `src/win32_present.rs` implements a separate **inactive**
Win32 GDI output stage using a top-down 32-bit `BI_RGB` DIB and
`SetDIBitsToDevice`. It validates that the target is an actual
process-owned HWND with client bounds matching the composed picture,
checks the buffer size, and always releases the window DC.
Importantly, it does not alter the game window, DirectDraw mode, or
input focus. It has independent tests for the 40-byte DIB header,
negative top-down height, and invalid or short buffers.

**Neither stage is connected to the live game's DirectDraw Flip yet.**
The GDI presenter is only a future software-output option for a
compatible *windowed* renderer; drawing GDI over an exclusive
DirectDraw flip chain would not produce stable, real borderless 3D.
A later version must supply the necessary COM surface virtualization
or a compatible DirectDraw/Direct3D translation layer first.

A third discovery came from the game logs: IID
`bb223240-e72b-11d0-a9b4-00aa00c0993e` is **IDirect3D3**, and
`93281502-8cf8-11d0-89ab-00a0c9054129` is
**IDirect3DTexture2**. The wrapper now installs a passive hook on
`IDirect3D3::CreateDevice` (Wine d3d.h slot 8) when QueryInterface
returns a D3D3 object. `D3D3-TRACE` records the device CLSID,
the chosen DirectDrawSurface4 render-target caps, HRESULT and
whether an IDirect3DDevice3 was returned. It delegates unchanged,
does not acquire another device, and never modifies graphics state.
The game has not yet been tested with this new hook. A future
borderless engine must preserve these legacy D3D3 device interfaces
rather than assuming a 2D-only renderer.

**This compositor is intentionally not wired into the actual Flip
path yet.** Before presenting any game frame, a future backend must
virtualize the primary/backbuffer while preserving the Z-buffer and
the game's Direct3D device; it must also acquire a valid frame without
locking a live GPU render target in an unsafe phase. The 3D game
still runs at its original 640x480 or 800x600 exclusive DirectDraw
resolution. Native 1920x1080 rendering and widescreen FOV correction
remain separate tasks.

## v0.1.11 — correct per-mode 3D chain and detect actual pixel formats

The owner's v0.1.10 log confirms correct initial preflight at
`640x480x16`: the captured original monitor is `1920x1080`,
the primary -> backbuffer and backbuffer -> Z-buffer attachments are
both present and the calculated 4:3 centered viewport is
`x240,y0,1440x1080`, with `applied=false`.

A subtle diagnostic bug was also discovered: when the game changes
from `640x480` to `800x600`, the old global chain's state was still
marked complete. That meant the v0.1.10 log emitted a **premature**
`800x600` preflight immediately after the *first* backbuffer
attachment, before that mode's Z-buffer had been observed.

This is fixed: `presentation::Session` now tracks one **generation
per successful SetDisplayMode**, invalidates the previous front/back/Z
topology on every mode change (including repeated resolutions), and
only logs `BORDERLESS-PREFLIGHT` after both new attachments exist
during that generation. New `CHAIN-EPOCH` lines make the reset
explicit and the preflight line prints the generation.

The next unknown is actual surface pixel format. `SURFACE-FORMAT`
uses a bounded, read-only `GetSurfaceDesc` query on the real 124-byte
`DDSURFACEDESC2` **primary/backbuffer/Z surfaces only**, recording
size, backbuffer count, reported pixel flags/bit count, and RGB masks
where available. This distinguishes RGB555 from RGB565 and reports
the Z-buffer format without guessing. It never locks surfaces, accesses
pixel data, alters the display mode, or changes game return values.
The returned format data is treated as metadata, not proof that a
future borderless adapter already exists.

This new build has no new windowed presentation implementation.
For runtime validation keep `Log=1` and
`ModernFPSPacing=0` to isolate the diagnostics. An experimental
60-calls-per-second primary Flip limiter remains available separately.

## v0.1.10 — proven 3D flip chain and 4:3 desktop preflight

The user-supplied v0.1.9 Wine log verifies the full legacy chain
**without mutating any DirectDraw surface**:
`GetAttachedSurface(request=0x4)` on the primary
`0x1000e238` returns backbuffer `0x1000601c`;
`GetAttachedSurface(request=0x20000)` on that backbuffer returns
Z-buffer `0x10024000`. A separate request `0x00401000` accesses
texture mipmaps and **must never be treated as a screen buffer**.
The same front/back/depth relations recur after game mode changes
between 640x480x16 and 800x600x16.

The Rust module `src/presentation.rs` now provides:

- Classification of **actual successful** primary -> backbuffer,
  backbuffer -> Z-buffer, and texture -> mipmap attachments
- A conservative preflight requiring a complete 3D chain **and**
  either 640x480x16 or 800x600x16
- An integer aspect-ratio calculation for presenting a 4:3 game image
  on the monitor's initial desktop resolution
- A read-only `BORDERLESS-PREFLIGHT` journal line indicating the
  internal resolution, pre-exclusive monitor dimensions, planned
  centered viewport and `applied=false`
- Tests for the observed surface caps, invalid/partial chains,
  4:3 -> 16:9 output and unusual screen dimensions

For a 1920x1080 desktop, both observed game modes would be
**displayed within a centered 1440x1080 viewport** with 240px black
bars on each side. This is a *presentation plan*, **not** native
1920x1080 game geometry, increased internal resolution or a working
borderless mode. A real borderless backend must emulate the original
DirectDraw front/back/Z semantics and present the internal 16-bit
image using a compatible windowed output path.

v0.1.10 does not make the exclusive renderer windowed: it only
tests that the necessary conditions are recognized. Keep
`ModernFPSPacing=0` for the next diagnostic session to isolate
rendering behavior; the previously confirmed optional
60-call/s primary Flip limiter remains available.

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
the non-overwriting `CrimsonSkiesFix.ini.example`.

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
in the existing active `CrimsonSkiesFix.ini`. It applies `FPSLimit=60`
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
`32767.65535.65535.65535`; CrimsonSkiesFix now labels that
`Unavailable` rather than claiming a genuine driver version.

The standalone proxy still forwards `DirectDrawCreate`,
`DirectDrawEnumerateA` and `DirectDrawCreateEx` to the real Windows
system DLL. The DirectDraw v1 COM hooks are experimental; newer COM
interfaces reached via QueryInterface are not yet intercepted.
Runtime behavior requires testing under Wine/Windows.

## Build and PR policy

```powershell
rustup target add i686-pc-windows-msvc
cd CrimsonSkiesFix
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

The GitHub Actions artifact contains the game-ready layout. No
copyrighted game file, memory dump or original system DLL is included.
Keep this PR in draft until the owner validates a real gameplay session.
