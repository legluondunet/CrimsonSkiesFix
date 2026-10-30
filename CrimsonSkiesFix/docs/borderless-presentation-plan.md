# Crimson Skies: borderless DirectDraw presentation design

**Status:** research / experimental design, 2026-10-30. No borderless
conversion is currently enabled on the renderer's DirectDraw4 path.

## Established by owner-supplied Wine runtime diagnostics

- `IDirectDraw` v1 setup in the loader is mostly auxiliary: it passes
  `SetCooperativeLevel(DDSCL_NORMAL, HWND=NULL)`, and the game then
  acquires DirectDraw2, DirectDraw4 and DirectDraw7 COM interfaces.
- A later `IDirectDraw4::SetCooperativeLevel` gets a valid, visible,
  game-owned top-level HWND, with requested flags `0x13`
  (`DDSCL_FULLSCREEN | DDSCL_ALLOWREBOOT | DDSCL_EXCLUSIVE`).
- `IDirectDraw4::SetDisplayMode` succeeds at `640x480x16` and
  `800x600x16`. Wine changes the actual game window from its initial
  `1920x1080` client bounds to those exclusive-mode sizes.
- The game creates a `DDSURFACEDESC2` (124 bytes) primary at
  `DDSD_CAPS | DDSD_BACKBUFFERCOUNT`, and invokes actual
  `IDirectDrawSurface4/7::Flip` calls on a surface with capabilities
  `0x1000e238`, including `DDSCAPS_PRIMARYSURFACE`, `FLIP`,
  `COMPLEX` and `FRONTBUFFER`.
- With `ModernFPSPacing=0`, steady `Flip` call rate reaches 120/s on
  the owner's 120 Hz Wine/GE-Proton session. With
  `ModernFPSPacing=1` and `FPSLimit=60`, three stable consecutive
  120-call windows took about 2.0s each. This verifies API call pacing,
  **not** the visible unique-frame rate or game-simulation correctness.
- The first Blt destination is not primary. Blt and Unlock have faster
  bursts than Flip, so never treat them as universal frame boundaries.

## Constraint: exclusive flip chain is not a normal borderless window

Windows DirectDraw documentation states that changing the mode and
flipping the primary surface generally requires exclusive cooperative
mode. Substituting `DDSCL_NORMAL`, then unconditionally claiming that
`SetDisplayMode` succeeded, leaves the game's required
`PRIMARYSURFACE | FLIP | COMPLEX` flipping chain unavailable or
semantically different. It can fail surface creation or break the 3D
render target. Merely resizing a window does not change the game's
internal video mode or perspective.

Microsoft references:
- https://learn.microsoft.com/en-us/previous-versions/ms785063(v=vs.85)
- https://learn.microsoft.com/en-us/windows/win32/api/ddraw/ns-ddraw-ddscaps
- https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-idirectdraw7-setcooperativelevel

## Stage A — observe attached buffers (in v0.1.9)

Without changing the game, add COM wrappers for:
- `GetAttachedSurface` (vtable slot 12) to learn whether a returned
  backbuffer has `DDSCAPS_BACKBUFFER`, how it relates to the primary,
  and whether the game requests it directly.
- `GetSurfaceDesc` (slot 22) to learn the resulting front/backbuffer
  dimensions and bpp/pixel format. Initially record only common
  descriptor prefix, never read the surface contents.
- `GetCaps` (slot 14) is already used read-only and can classify
  primary/front/back buffers.

Do not log raw pointer addresses, HWND values, filenames, screen
pixels, window titles or other-process windows. Bound instrumentation
and keep all logs opt-in under `[Debug] Log=1`.

## Stage A results — confirmed 2026-10-30

The owner's v0.1.9 diagnostic log, recorded on 2026-10-30,
provides the following repeatable positive matches on both 640x480
and 800x600 video modes:

| Association | Requested capability | Parent DDSCAPS | Returned DDSCAPS |
|---|---|---|---|
| Primary -> backbuffer | `DDSCAPS_BACKBUFFER=0x4` | `0x1000e238` | `0x1000601c` |
| Backbuffer -> Z-buffer | `DDSCAPS_ZBUFFER=0x20000` | `0x1000601c` | `0x10024000` |
| Texture -> mipmap | `DDSCAPS_TEXTURE | DDSCAPS_MIPMAP = 0x401000` | `0x00401808` | `0x00401808` |

The first and second associations prove the primary flip chain's
required 3D depth attachment. Texture/mipmap attachments are separate
and must never be redirected as visible backbuffers. Existing texture
`GetSurfaceDesc` requests confirmed DDSURFACEDESC2=124 bytes; 8x8,
128x16 and 16x16 texture sizes were observed.

v0.1.10 includes a **tested, pure-Rust preflight** in
`src/presentation.rs`. It recognizes the successful front/back/Z
attachments and refuses incomplete or unsupported 3D chains. A
pre-exclusive monitor snapshot prevents exclusive-mode resolution
changes from corrupting the saved target desktop dimensions. A
centered aspect-fit plan for 640x480x16 or 800x600x16 on 1920x1080 is
1440x1080 at x=240, y=0, with 240-pixel side bars. The result is
reported to logs as `BORDERLESS-PREFLIGHT`, with
`requires_virtual_primary=true` and `applied=false`.

**Stage A's topology validation is complete. Stage B implementation
is not complete.** Do not convert to normal cooperative mode or
claim that the game renders at 1920x1080 yet.

## Stage B0 — frame format and CPU presentation foundation, v0.1.12

The owner's v0.1.11 Wine log confirms the following independent
`DDSURFACEDESC2` facts *on both 640x480 and 800x600*:

- Primary frontbuffer: RGB565, `DDPF_RGB=0x40`, 16 bits, masks
  `R=0xf800 G=0x07e0 B=0x001f`, and one backbuffer
- Attached backbuffer: the same RGB565 format
- Attached Z-buffer: `DDPF_ZBUFFER=0x400`, depth **32 bits**
- Five successive successful `SetDisplayMode` generations validate
  a new front/back/depth chain each time. No old Z-buffer evidence is
  reused in the current preflight.
- A 120Hz display continues to produce about 120 modern `Flip`
  calls/s when `ModernFPSPacing=0`; the prior 60-call/s cap requires
  explicit `ModernFPSPacing=1`.

The tested `src/rgb565_present.rs` compositor is the **pixel output
stage only**: it can convert an already-available RGB565 framebuffer
to BGRA8, aspect-fit into a separate desktop buffer, and fill black
pillarboxes. The inputs are *borrowed byte slices*, and all pitch,
geometry, and buffer-size bounds are checked before touching output.
It never calls `Lock` on the DirectDraw render target and never
replaces the game's exclusive flip chain. This avoids the false
impression that a simple screen-copy helper can safely capture
rendering from Direct3D VRAM.

The still-missing integration is significant: `DDSCL_NORMAL` cannot
retain the legacy exclusive primary Flip chain, while the application
has a Direct3D device rendering into its original RGB565 backbuffer
with an attached 32-bit Z-buffer. A correct COM virtualization or
external compatible DirectDraw3D translation layer must be selected
before any real windowed presentation or alt-tab behavior is enabled.
A 2D-only DDraw wrapper is not enough for the 3D device.

## Stage B1 — identify actual Direct3D3 device/render target

Cross-checking the captured `QueryInterface` GUIDs against Wine's
`d3d.h` established that the engine requests
**IID_IDirect3D3 = bb223240-e72b-11d0-a9b4-00aa00c0993e** and
**IID_IDirect3DTexture2 = 93281502-8cf8-11d0-89ab-00a0c9054129**.
This is not a 2D-only DirectDraw game and should not be passed to a
2D-only DDraw wrapper; the original fixed-function Direct3D3 device
requires genuine support. A DDraw->D3D9 translation layer may be a
possible future backend, but available third-party wrappers document
limited D3D compatibility; none is assumed working with Crimson Skies.

v0.1.12 adds a **passive** `IDirect3D3::CreateDevice` COM hook
(vtable slot 8 per Wine `d3d.h`), installed only after successful
`QueryInterface(IID_IDirect3D3)`. It delegates to the original
method with the same CLSID, `IDirectDrawSurface4` render target,
device output and outer IUnknown; it logs the resulting 3D target
surface role, result and object-return status. The device remains
owned by the game; no new D3D device is created. This is the next
useful runtime question: whether the game renders into the
**backbuffer** (`0x1000601c`) or another intermediate surface,
including during menus and transitions.

The pure `rgb565_present.rs` and inactive `win32_present.rs`
together provide validated pixel conversion and top-down BGRA8
Win32 client-area output for *already available CPU image data*.
They are **not connected** to the exclusive D3D3 rendering path;
safe interception/translation of the 3D render target is still
missing. The actual borderless mode must remain off until then.

References:
- https://fossies.org/dox/wine-11.0/d3d_8h_source.html
- https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-setdibitstodevice
- https://github.com/elishacloud/dxwrapper/wiki/DirectDraw-to-Direct3D9-Conversion

## Stage B — prototype an explicit windowed presentation adapter

Only after Stage A is reproduced in gameplay, implement behind a
**new, default-off option**, *separate* from existing `Borderless=1`:

1. Own one selected game HWND after the first valid, process-owned
   `SetCooperativeLevel`; do not guess at null-HWND startup.
2. Move to `DDSCL_NORMAL` only if a complete offscreen render-and-present
   plan is available, otherwise keep original exclusive behavior.
3. Preserve the **internal 16-bit** game render dimensions and legacy
   Direct3D target semantics, including the attachment chain expected
   by the application. This may require a virtualized primary/backbuffer
   COM shim rather than modifying only `SetDisplayMode`.
4. Present via a real windowed primary `Blt`/stretch with the correct
   clipper or via an equivalent verified Direct3D backend. Default
   aspect-correct pillarboxing: e.g. a 4:3 image into 1920x1080 has a
   1440x1080 viewport centered horizontally. Do not call this native
   1920x1080 3D rendering; that is a separate engine-resolution/FOV job.
5. Preserve `QueryInterface`, `AddRef`, `Release`, `GetAttachedSurface`,
   `Lock`, `Unlock`, `Flip`, `Blt` and the game's surfaces'
   expected DDSCAPS/descriptor behavior. Do not violate COM ownership.
6. If any necessary COM operation or surface creation fails during
   preflight, automatically fall back to the original exclusive pipeline
   **before** any game surface is exposed. Never switch modes mid-frame.
7. Test menus, mission 3D, cutscenes, alt-tab, focus restoration,
   resolution changes, game exit, and native Windows as well as Wine.

The first borderless proof should focus on **aspect-correct presentation
at desktop output resolution**; native widescreen geometry, UI, FOV
and custom render resolution follow separately.

## Stage B1 follow-up — v0.1.17 same-group API timing

The passive `SCENE-CORRELATION` now includes monotonic elapsed
milliseconds between the first and 120th successful **primary**
Flip in each group. It divides **119 observed intervals** by that
elapsed time to report successful primary Flip *COM completion*
calls per second. This is better aligned with the corresponding
successful BeginScene/EndScene call counts than the previous
independent attempted-Flip and scene rate windows.

The first scene-counter baseline for a mode begins at successful
SetDisplayMode, so the initial group can also include scene events
during loading before its first Flip. Subsequent sample baselines
are the prior successful primary Flip group boundary. Neither
measurement proves on-screen frame rates or frame-to-scene identity.
The new counters do not enable windowed presentation.

## Stage B1 follow-up — v0.1.18 inter-Flip scene distribution

The v0.1.17 log shows five consecutive 120-Flip samples with
11 successful BeginScene and EndScene per Flip on average in
the 640x480 phase, contrasting with 1:1 in the 800x600 phase.
The diagnostic `SCENE-INTERFLIP` adds exact histogram bins
0..12 and 13+, per-interval min/max and Begin/End mismatches
over **119 within-group primary Flip intervals**. The result
will distinguish uniform counts from bursts. No game state
or presentation behavior changes.

## v0.1.18 Wine test — scene counts per successive primary Flip verified

Owner-supplied v0.1.18 log (`CrimsonSkiesFix(1).log`; Wine 11.0,
GE-Proton11-11, AMD Radeon RX 6600M, 1920x1080 at 120Hz,
`ModernFPSPacing=1`, `FPSLimit=60`, `Log=1`) validates the new
`SCENE-INTERFLIP` histogram in the real Direct3D3 path:

| Mode / stage | Complete groups | 119-interval within-group distribution | Begin/End mismatches | Counter regressions |
|---|---:|---|---:|---:|
| Generation 2, 800x600 | 4 | 476 intervals with exactly 1 BeginScene and EndScene each | 0 | 0 |
| Generation 3, 640x480 entry | 1 | 110 intervals with 4, 8 with 11, 1 with 22 | 0 | 0 |
| Generation 3, 640x480 stable | 5 | **595 observed intervals with exactly 11** successful BeginScene/EndScene calls each | 0 | 0 |
| Generation 3, transition out | 1 | 95 intervals with 4, 24 with 11 | 0 | 0 |
| Generation 3, later transition | 1 | 31 intervals with 1, 87 with 4, 1 with 5 | 0 | 0 |

There are 12 complete groups in the file. Five consecutive
stable 120-primary-Flip groups each contain 1320 successful
BeginScene and 1320 successful EndScene events, with
119 within-group intervals each at 11. Four *unmeasured*
intervals cross the boundaries between those five groups;
therefore 595 **observed** intervals must not be described
as 595 uninterrupted adjacent intervals.

Stable successful primary Flip COM completion rates were
approximately 59.89–60.01 calls/s in these five groups.
In the 800x600 generation, the four complete groups all
show 1:1 inter-Flip scene activity, despite startup/transition
rate variation (13.07–58.05 calls/s, then 36.04).
The scene count varies between phases, including within
generation 3: this is not evidence that resolution alone
determines the number of scenes.

The observed 11 calls per primary-Flip interval is **real at
the COM API level**, not a window-average artifact.
It is **not evidence of 11 presented images** or 11 engine
simulation steps per interval. There is no proof that each
scene contains geometry draw calls. The log's successful
`CreateDevice` still targets RGB565 backbuffer with attached
32-bit Z-buffer; the borderless preflight reports
`applied=false`.

### Completed A/B comparison — v0.1.18, ModernFPSPacing=1 versus 0

The owner supplied the requested same-binary test with modern primary
Flip pacing disabled (`CrimsonSkiesFix(2).log`). Both recordings use
v0.1.18, the same Wine 11.0/GE-Proton11-11 stack and 120Hz desktop,
`[Graphics] FPSLimit=60` and `[Debug] Log=1`. The **only recorded
configuration difference** is `ModernFPSPacing=1` for the first
recording versus `ModernFPSPacing=0` for the second. We cannot
verify identical game movements/scenes or timing from the logs alone.

| Property | Paced: `ModernFPSPacing=1` | Unpaced: `ModernFPSPacing=0` |
|---|---|---|
| Samples logged | 12 groups × 120 successful primary Flips | 12 groups × 120 successful primary Flips |
| 800x600 generation 2 | 4 groups; 476/476 intervals exactly 1 BeginScene and EndScene | 7 groups; 833/833 intervals exactly 1 BeginScene and EndScene |
| 640x480 generation 3, stabilized | 5 groups; 595/595 sampled intervals exactly 11 successful BeginScene and EndScene | 3 groups; 357/357 sampled intervals exactly 11 successful BeginScene and EndScene |
| Successful primary Flip completion cadence, stable 640x480 | 59.89–60.01 calls/s | 119.96–120.08 calls/s |
| Successful primary Flip completion cadence, steady 800x600 | 58.05 calls/s in one group; other groups include startup/stalls | 120.08 calls/s in three groups; transient groups vary |
| BeginScene vs EndScene interval mismatches | 0 | 0 |
| Counter regressions | 0 | 0 |

The stable unpaced 640x480 groups each contain exactly 1,320
successful BeginScene and 1,320 EndScene calls per 120 successful
primary Flips, with elapsed first-to-last-Flip times of 991, 991
and 992 ms. The corresponding paced stable groups each have the
same 1,320/120 counts but require 1983–1987 ms.
Therefore the *rate of COM scene calls* roughly doubles with the
successful Flip completion rate, while the observed 11-scene/Flip
structure stays unchanged.

For 800x600, both runs confirm an exact 1-scene-per-inter-Flip
interval during all measured groups; unpaced groups achieve
120.08 calls/s once stabilized. During an unpaced 640x480
transition, one group shows 52 intervals with 4 scene calls,
66 with 11, and one with 22; its Flip call rate is only 59.77/s.
Thus **disabling the limiter does not guarantee 120 calls/s at
every moment**. Phases with slower workload or transition timing
must not be mistaken for a persistent 60-FPS cap.

Neither observation establishes unique on-screen frame counts,
simulation ticks, correctness of animation speed or the number
of actual draw calls in each BeginScene/EndScene section.
The original 3D device/backbuffer/Z-buffer topology persists,
and borderless remains unimplemented.

### Next validation: game simulation timing, without another DLL hook

The A/B experiment is complete. Before changing FPS pacing defaults,
the game must be assessed for **actual simulation/animation speed**
with pacing enabled and disabled. Compare equivalent reproducible
gameplay actions (e.g., a fixed-length timed maneuver, movement
over a known route or scripted event) using an external stopwatch
and screenshots/video if needed; note any fast-forward symptoms.
The existing log alone cannot establish whether doubling Flip and
scene-call rates also doubles simulation speed.

Preserve `ModernFPSPacing=0` as the code default and `FPSLimit=60`
as a setting that matters only when modern pacing is enabled.
Do not change the FPS limiter or the COM graphics pipeline solely
from these API-call observations.

## Owner observation: perceived simulation speed in the A/B test

After comparing v0.1.18 with `ModernFPSPacing=1` (~60 successful
primary Flip COM completions/s) and `ModernFPSPacing=0` (~120/s),
the owner reports **no perceptible speed-up in gameplay**, including
aircraft movement and animation. This is a *qualitative* observation,
not a stopwatch-based proof of simulation-timing independence.

Decision: **retain current pacing defaults and behavior**. Do not
force 60 FPS or deduce simulation ticks from BeginScene/EndScene.
Prioritize the actual DirectDraw3D-compatible borderless presentation
architecture rather than additional pacing hooks.

## Borderless development decision — next implementation stages

**Do not activate a partial borderless implementation.** DirectDraw
requires exclusive level for the game's real primary-surface flipping
chain. Passing `DDSCL_NORMAL` to the real API and pretending that
`SetDisplayMode` and `Flip` succeeded would strand the Direct3D3
render target or the app's expected COM surface topology.

The observed Windows/Wine objects are:

1. `IDirectDraw4::SetCooperativeLevel` receives a valid game-owned
   HWND and `0x13` (`DDSCL_FULLSCREEN|DDSCL_EXCLUSIVE|DDSCL_ALLOWREBOOT`).
2. `IDirectDraw4::SetDisplayMode` selects 640x480x16 / 800x600x16.
3. `CreateSurface` creates a primary with one backbuffer, then
   `GetAttachedSurface` returns an RGB565 backbuffer and a 32-bit
   Z-buffer; the game also creates many texture/mipmap surfaces.
4. `IDirect3D3::CreateDevice` gets that **real DirectDrawSurface4
   backbuffer**, and the scene hooks observe 1, 4 or 11 complete
   BeginScene/EndScene pairs between successful primary Flips.

### Stage B2: pure Rust COM-interface / state-machine contract

Design and unit-test a dedicated façade that distinguishes a
**game-visible virtual primary/backbuffer** from the real system
windowed primary and the real Direct3D-capable offscreen RGB565
render target. Test method identity across QueryInterface versions,
AddRef/Release and attachment ownership, expected flags/descriptors,
and error propagation before introducing any active runtime hook.

The actual Direct3D3 device MUST receive a compatible real
DirectDrawSurface4 target; do not substitute an arbitrary memory
buffer or fake a successful `CreateDevice` HRESULT. A fake `Flip`
cannot simply return DD_OK unless the corresponding pixels have
actually been presented.

### Stage B3: fully isolated, default-off runtime experiment

Use a **separate** `ExperimentalWindowedAdapter=0` configuration
option *only once its implementation exists*. The original
`Borderless=1` preflight is not a guarantee of windowed rendering
and must not silently enable this experiment.

On opt-in only, before the game is exposed to any replacement surface:
- Validate a game-owned HWND and complete windowed DirectDraw3D
  initialization support in the chosen Windows/Wine backend.
- Keep the desktop mode intact, retain the 640x480/800x600
  RGB565 offscreen 3D render target and 32-bit Z-buffer;
  retain texture/mipmap semantics.
- Create a real windowed primary with an appropriate clipper; use
  a tested real DirectDraw `Blt` (or a validated equivalent)
  from the 3D render target to an aspect-fitted desktop viewport.
  Account for real desktop format conversion, scaling, GDI/Win32
  interop, and Windows/Wine focus changes.
- Emulate game-visible surface capabilities and Flip semantics
  consistently for all QueryInterface versions and involved COM
  methods. No host resolution change or guessed dimensions.
- If any capability or resource initialization fails, **fall back
  before exposing objects to the game**. Once active, do not
  silently switch to exclusive mode mid-frame.
- Validate menus/3D/cutscenes/Alt+Tab/exit/mode switches under Wine
  *and* native Windows before considering an enabled-by-default
  behavior.

The existing RGB565-to-BGRA8 compositor and GDI presenter remain
**inactive fallback building blocks**: they only process already
available CPU pixels and do not solve GPU backbuffer capture.
No 3D surface locks or synchronous per-frame CPU readbacks should be
introduced solely to make a nominal borderless screenshot appear.

### Why not substitute an off-the-shelf 2D wrapper?

`cnc-ddraw` explicitly excludes Direct3D games.
`dxwrapper`'s `Dd7to9` documents limited Direct3D support, so it
cannot be assumed compatible with this Direct3D3 renderer without
independent validation. `DDrawCompat` supports sophisticated
borderless presentation on Windows but explicitly does not support
Wine or stacking with other graphics wrappers; it cannot directly
serve as a validated component inside our Wine-targeted DLL.

References:
- https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-idirectdraw7-setcooperativelevel
- https://learn.microsoft.com/en-us/previous-versions/ms785063(v=vs.85)
- https://github.com/FunkyFr3sh/cnc-ddraw/blob/master/README.md
- https://github.com/elishacloud/dxwrapper/wiki/DirectDraw-to-Direct3D9-Conversion
- https://github.com/narzoul/DDrawCompat/blob/master/README.md

## Stage B2 implementation — v0.1.19 pure COM contract model

A new isolated `src/borderless_contract.rs` file contains a
testable model of the planned façade. It does not install COM hooks or
allocate resources and is not reachable from active gameplay.

The model enforces:
- All required windowed backend capabilities must be validated prior
  to surface exposure. CPU copies of pixels cannot substitute for
  the **real RGB565 IDirectDrawSurface4** that Direct3D3 requires.
  The real target also must have the `DDSCAPS_3DDEVICE` capability,
  because `IDirect3D3::CreateDevice` requires a 3D-device surface.
- Failures during preparation abort without altering the original
  exclusive path; once the façade has exposed a replacement object,
  switching silently back to exclusive is forbidden.
- Logical COM IUnknown identity survives QueryInterface across
  Surface1/2/3/4/7 interface versions, with explicit error handling
  for unsupported interfaces and stale mode epochs.
- Queries and GetAttachedSurface acquire client references; attachment
  ownership keeps backbuffer/depth alive while the parent exists.
  Combined client-plus-attachment counts must not overflow `u32`.
  Release cascades when final owner and client references disappear.
- Per-surface descriptors mirror *observed* primary (RGB565 +
  backbuffer count 1), RGB565 backbuffer and 32-bit Z-buffer. A
  missing or mismatched topology cannot satisfy preflight.

This model intentionally **does not implement real COM vtables or
claim to validate any GPU surfaces**. A future Stage B3 must give
every logical identity a stable physical IUnknown pointer, conform
to DirectDraw/Direct3D interface method ABI, obey actual implicit
vs explicit attachment lifetimes, and verify resource handles
before committing. It must route all unsupported cases to a
safe *pre-exposure* fallback. Do not enable Borderless based on
this module alone.

Microsoft DirectX 6 `IDirect3D3::CreateDevice` ABI/caps reference:
- https://library.thedatadungeon.com/directx6/devdoc/live/directx/imref_992c.content.htm

Microsoft COM and DirectDraw contracts:
- https://learn.microsoft.com/en-us/windows/win32/com/rules-for-implementing-queryinterface
- https://learn.microsoft.com/en-us/windows/win32/com/implementing-reference-counting
- https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-idirectdrawsurface7-getattachedsurface
- https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-idirectdrawsurface7-addattachedsurface

## Stage B3a — v0.1.20 physical IUnknown prototype (still inactive)

The new standalone `src/com_unknown.rs` is a real native COM
ABI scaffold: a `#[repr(C)]` object with a vtable pointer at
offset zero, an `extern "system"` three-method IUnknown
vtable, real physical pointer identity, atomic AddRef/Release and
deallocation on the final Release. Seven x86 unit tests call the
vtable function pointers, verify QueryInterface's reference contract
and object identity, reject unsupported Surface4 and NULL arguments,
test reference overflow, and prove single final destruction.

For deliberate correctness, the static set of supported interfaces
is **only IID_IUnknown**. Exposing an IDirectDrawSurface IID without
a complete compatible vtable would be a COM contract violation and
could crash the game. Neither the logical `borderless_contract`
nor the physical `com_unknown` module is called by a
DirectDraw hook. They also do not implement COM aggregation or
thread/apartment marshalling. The release archive and runtime graphics
pipeline are unchanged.

Next stages:
1. Enumerate **all used DirectDrawSurface4 and QueryInterface methods**
   from real game logs/ABI evidence before claiming a surface interface.
2. Implement a complete method dispatch/facade for each advertised
   interface (including Release, real attachments, texture/lock/Blts,
   device-target compatibility) with identity and HRESULT tests.
3. Construct the actual windowed backend *before* returning a
   façade to the game. Only then expose an opt-in flag and attempt
   aspect-correct 4:3 presentation without changing desktop mode.
4. Validate both Wine and native Windows. Abort safely before
   exposure if any capability is unsupported.

Reference: https://learn.microsoft.com/en-us/windows/win32/com/rules-for-implementing-queryinterface

## Stage B3b — v0.1.21 exact 45-method Surface4 ABI

A separate inactive Rust module `src/com_surface4_abi.rs`
contains the complete, order-verified IDirectDrawSurface4 vtable
based on Wine's `include/ddraw.h`, with all 45 signatures and
the Win32 x86 calling convention.

The four x86 tests cover offsets of **all 45 methods**, the
180-byte struct, fixed method slot identities observed in the
owner's logs and typed dispatch on a fully initialized mock.
The mock deliberately returns E_NOINTERFACE for Surface4,
and it is never used as a real DirectDraw or Direct3D target.

This module fixes only the ABI contract. A real borderless adapter
still requires complete COM dispatch for every advertised IID,
genuine Direct3D3-compatible RGB565 offscreen resources and
32-bit Z buffer, windowed primary/clipper and actual presentation
without changing the host desktop resolution. No live hook or
configuration option is enabled by this commit.

Source: https://github.com/wine-mirror/wine/blob/master/include/ddraw.h

## Release gates

- Keep the PR in draft and never merge without owner validation.
- Dist artifact layout stays `ddraw.dll` at game root,
  `CrimsonSkiesFix/` for docs and INI, and optional
  `CrimsonSkiesFix.log` beside the loaded DLL.
- Never ship an active `CrimsonSkiesFix.ini` or log; ship only the
  `CrimsonSkiesFix.ini.example` template to preserve user settings.
- x86 Windows CI, tests, and gameplay evidence required before enabling
  any behavior change by default.
