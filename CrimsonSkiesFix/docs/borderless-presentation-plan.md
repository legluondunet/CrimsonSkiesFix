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

## Stage B3c — v0.1.22 owner-testable passive surface-method audit

The x86 proxy now has a **disabled-by-default**
`[Debug] SurfaceUsageTrace=1` switch that also requires `Log=1`.
It installs nine additional pass-through method hooks on observed
DirectDrawSurface2/3/4/7 interfaces, at the ABI-correct slots.
Each wrapper calls the original function **exactly once with
unmodified arguments** and forwards its real HRESULT; no surface
data or reference ownership is touched.

The `surface_usage.rs` counters classify the destination from
the existing read-only `GetCaps` result. They separately count
successful and failing `AddAttachedSurface`, `DeleteAttachedSurface`,
`GetDC`, `IsLost`, `Lock`, `ReleaseDC`, `Restore`,
`SetClipper` and `GetDDInterface` calls, then summarize per
120 successful primary Flips. There are at most 12 sample lines per
process and mode switches discard incomplete samples. These counts
can reveal which COM surface operations a full borderless virtualizer
must implement in game menus and 3D missions.

The default renderer, frame pacing, configuration persistence,
logging location and release archive layout are unchanged.
The opt-in audit is **not** a borderless implementation and should
not be confused with the standalone IUnknown and Surface4 ABI tests.

## Live v0.1.22 audit and v0.1.23 next measurement

The owner-provided v0.1.22 log uses Wine 11.0 and
GE-Proton11-11, `ModernFPSPacing=0`, `Log=1` and
`SurfaceUsageTrace=1`. Twelve bounded 120-primary-Flip
groups are recorded: six in generation 2 at 800x600, and
six in generation 3 at 640x480. Every captured method call
returned success (zero observed failures).

| Receiver.Method | 800x600 (gen 2) | 640x480 (gen 3) |
|---|---:|---:|
| primary.Lock / Restore | 1 / 1 | 1 / 1 |
| backbuffer.AddAttachedSurface | 1 | 1 |
| backbuffer.Lock / Restore | 147 / 1 | 1 / 1 |
| depth.Restore | 1 | 1 |
| texture.Lock / Restore | 1669 / 6 | 1477 / 1358 |
| other.Lock / Restore | 3 / 1 | 1 / 1 |
| other.GetDDInterface | 3 | 0 |

No `GetDC`, `ReleaseDC`, `SetClipper`, `IsLost`
or `DeleteAttachedSurface` was recorded in these limited
windows. This does not prove those methods are never used.
The first 640x480 group has 1358 successful texture Restores,
while subsequent stable 120-primary-Flip groups repeatedly
show ~240 texture Locks. A future windowed backend must
honor actual attachment and restoration semantics; it
cannot safely assume GetAttachedSurface or Restore are
mere no-ops.

Stable generation 3 still shows exactly 11 successful
BeginScene and EndScene calls per observed primary Flip
interval, with ~120 successful primary Flips/s when unpaced.
The current 16-bit RGB565 primary/backbuffer + 32-bit depth
chain remains unchanged, and every observed borderless
preflight says `applied=false`.

Version 0.1.23 adds only per-role accounting for the
already intercepted `Unlock`, `Blt`, `BltFast` and
`GetAttachedSurface` calls, without changing any live
DirectDraw object, surface data, COM ownership or GPU
presentation. Comparing texture Lock versus Unlock and
Blt receiver roles will inform the next real adapter design.

## v0.1.23 owner log: exact role/method totals

The user-supplied `CrimsonSkiesFix(4).log` from Wine 11.0
GE-Proton11-11 runs v0.1.23 with `ModernFPSPacing=0`
and optional `SurfaceUsageTrace=1`. There are **five
800x600 groups** in generation 2, and **seven 640x480
groups** in generation 3, each spanning 120 successful
primary Flips (12 groups; 1440 Flip COM completions).
There are no recorded HRESULT failures, and each
Lock/Unlock pair balances exactly in each measured group.

| Method / receiver | 800x600 (5 groups) | 640x480 (7 groups) |
|---|---:|---:|
| texture.Lock = texture.Unlock | 1697 | 1709 |
| backbuffer.Lock = backbuffer.Unlock | 114 | 1 |
| primary.Lock = primary.Unlock | 1 | 1 |
| backbuffer.Blt | 615 | 841 |
| depth.Blt | 442 | 1317 |
| texture.Restore | 6 | 1358 |
| backbuffer.AddAttachedSurface | 1 | 1 |
| primary.GetAttachedSurface | 1 | 1 |
| backbuffer.GetAttachedSurface | 1 | 1 |

Notably during stable 640x480 3D phases, 120 successful
primary Flips align with 120 successful backbuffer Blts,
240 successful depth Blts and exactly 11 BeginScene/EndScene
pairs per observed inter-Flip interval. These are API rates
and ratios, **not verified on-screen FPS**.

`depth.Blt` might be an explicit depth clear/fill,
but this cannot be inferred from the count alone.
Version 0.1.24 adds optional `SURFACE-BLT-SIGNATURE`
sampling for exact Blt flags, source-pointer NULL/provided
status, successful/failed HRESULTs and destination role,
without examining DDBLTFX or pixels. It has bounded
eight-signature-per-role storage and resets on mode changes.
The original exclusive DirectDraw/Direct3D3 paths remain
unchanged, borderless `applied=false`.

Original DirectDraw flags (Wine include/ddraw.h):
`DDBLT_COLORFILL=0x00000400`,
`DDBLT_DEPTHFILL=0x02000000`,
`DDBLT_WAIT=0x01000000`.

## v0.1.24 owner gameplay log — real fills confirmed

The owner-provided `CrimsonSkiesFix(5).log` is a successful
v0.1.24 run on Wine 11.0 / GE-Proton11-11 with
`SurfaceUsageTrace=1` and `ModernFPSPacing=0`.
It contains twelve complete groups of 120 successful primary
Flips (five 800x600; seven 640x480), with **zero failures**.

| Receiver | Exact Blt DWORD flags | src pointer | DX6 meaning |
|---|---|---|---|
| RGB565 backbuffer | 0x01000400 | NULL | DDBLT_WAIT \| DDBLT_COLORFILL |
| Z-buffer 32 bits | 0x02000000 | NULL | DDBLT_DEPTHFILL |
| backbuffer startup copy | 0x01000000 | provided | DDBLT_WAIT |
| backbuffer startup copy | 0x00000000 | provided | no extra flags |
| backbuffer startup copy | 0x01000200 | provided | DDBLT_WAIT \| DDBLT_ASYNC |

**Correction:** 0x00000200 is DDBLT_ASYNC, not a source
color-key override. Wine ddraw.h specifies
DDBLT_KEYSRCOVERRIDE=0x00010000.

In a stable 640x480 gameplay group the game makes exactly
120 RGB565 backbuffer COLORFILL Blts and **240 Z32 DEPTHFILL
Blts** per 120 successful primary Flips; these are real
depth fills, not Z-buffer image copies. Lock/Unlock call
counts remain paired by role in every measured group.
This establishes a useful requirement for a real windowed
DirectDraw3D adapter. It does not imply 120 displayed FPS
or independently validate simulated game timing.

## Stage B3f — v0.1.25 independent Windows x86 windowed capability probe

A separate `src/bin/windowed_ddraw_probe.rs` executable
now tests the real host DirectDraw4 and Direct3D3 APIs
**outside** the game and without injecting the proxy.
It uses a private test HWND, DDSCL_NORMAL, primary/clipper,
RGB565 DDSCAPS_3DDEVICE offscreen target, Z32 attachment,
color-fill and presentation Blt, and D3D3 HAL CreateDevice.
It prints real HRESULTs and verifies host desktop dimensions
have not been changed. No exclusive mode or SetDisplayMode
is called. Three x86 tests validate descriptor layout and
critical constants; GitHub Actions builds the probe into
an **independent artifact** while preserving the game
DLL's existing exact three-file release layout.

A successful result would validate native/Wine resource
creation and basic presentation only. It would **not**
provide a virtual primary/flip chain, COM identity,
game frame integration, or an actual borderless game.
A failed step will identify the backend capability to
address next before attempting runtime redirection.

## v0.1.26 — clean standalone probe file logging

The Windows x86 windowed DirectDraw resource probe opens
`windowed_ddraw_probe.log` beside its **own EXE**, not beside
the game or in Wine's current working directory. It starts
before any COM initialization and writes each HRESULT,
success/failure and final verdict immediately to disk.
Wine stderr/stdout remain separate, so extensive Wine
diagnostics cannot obscure the probe's output.

Each invocation truncates the previous probe-specific
log. A missing/unwritable directory causes an explicit
startup failure before any DirectDraw API call.
The game DLL and its existing `CrimsonSkiesFix.log`
remain unchanged, as do the user's INI and the
three-file game DLL distribution artifact. The standalone
probe artifact still contains only the executable.

## Stage B3g — v0.1.27 fix verified from the owner's probe log

The owner's first logged v0.1.26 standalone probe exited at:
`DirectDrawCreateEx(IDirectDraw4) HRESULT=0x80070057`.
This is the result of a **bug in our probe**, not a demonstrated
Wine windowed-backend limitation. The official
`DirectDrawCreateEx` contract mandates
`IID_IDirectDraw7`, and rejects all other requested IIDs.

In v0.1.27 the probe first creates `IDirectDraw7`, logs
the return status, then calls `IDirectDraw7::QueryInterface`
for the original needed `IDirectDraw4` interface and logs
that separate status. The original v7 COM reference is
released after the successful QueryInterface. The existing
normal-cooperative-level, primary+clipper, RGB565 offscreen,
Z32 attachment, Blt and Direct3D3 CreateDevice capability
checks remain intact. There is no modification to the
game DLL or to the active DirectDraw hook path.

Reference: https://learn.microsoft.com/en-us/windows/win32/api/ddraw/nf-ddraw-directdrawcreateex

## Stage B3h — v0.1.28 Z32 DDPIXELFORMAT mask correction

The owner ran the v0.1.27 stand-alone probe and provided a
complete log. DirectDraw7, QueryInterface(DirectDraw4),
windowed `DDSCL_NORMAL`, primary, clipper, RGB565
`DDSCAPS_3DDEVICE` offscreen target all return success.
The first failure is `CreateSurface(Z32)=0x88760091`,
`DDERR_INVALIDPIXELFORMAT`. This **does not yet establish
a Wine 32-bit depth limit**.

Comparing our descriptor with Wine's direct3d 32-bit depth
tests revealed that `DDPIXELFORMAT.dwZBitMask` was zero.
The v0.1.28 probe **incorrectly attempted** to set Win32 x86 word 22 to
`0xffffffff` for a 32-bit Z surface. It first tries the
requested video-memory Z32 caps and only if that fails
tries Z32 with driver-selected memory placement, logging
which path succeeded. After creation it queries the
*actual* descriptor, asserts 32-bit Z pixel format with a
non-zero depth mask, and tests the real depth-fill Blt.
If the second allocation works but lacks video memory,
this remains separate evidence and does NOT imply full
equivalence to the game's exact caps.

No 16-bit downgrade, SetDisplayMode, COM virtualization,
or game process interception is performed. All live DLL
graphics paths are still unchanged. The PR remains draft.

Reference: Wine's ddraw depth tests and original
`DDPIXELFORMAT` layout
(https://github.com/wine-mirror/wine/blob/master/include/ddraw.h).

## Stage B3i — v0.1.29 exact DDPIXELFORMAT union layout and HAL enumeration

The v0.1.28 owner log still fails Z32 allocation with
`DDERR_INVALIDPIXELFORMAT` after both attempts. The
diagnostic's Win32 descriptor was incorrect in **both**
v0.1.27 and v0.1.28: the depth mask is the third union,
`DDPIXELFORMAT.dwZBitMask` at relative DWORD 5.
The 32-bit x86 DDSURFACEDESC2 DDPIXELFORMAT begins at
DWORD 18, so **actual Z mask is DWORD 23**, whereas
the previous code wrote 0xffffffff to DWORD 22
(the `dwStencilBitDepth` alias). This is a concrete
ABI bug in the standalone test, not proof that Wine
cannot create a windowed Z32 resource.

The v0.1.29 probe fixes both the requested and the
returned `dwZBitMask` offset. It also queries
`IDirect3D3::EnumZBufferFormats(HAL)` (COM slot 10,
callback HRESULT=DDENUMRET_OK=1) and logs the
hardware-advertised Z formats *before* allocation.
If HAL advertises a **pure 32-bit Z format**, the
probe requests its exact 32-byte DDPIXELFORMAT,
otherwise it requests a correctly packed Z32
with 0xffffffff mask as an explicit test.

The program never silently downgrades to 16-bit
depth, never alters the desktop display mode, and
leaves the game proxy untouched. New unit tests
check the correct Z/stencil word offsets and
selection of a genuine 32-bit depth format.

Reference: https://github.com/wine-mirror/wine/blob/master/include/ddraw.h
and https://github.com/wine-mirror/wine/blob/master/include/d3d.h.

## Stage B3j — v0.1.29 live PASS and v0.1.30 presentation-scene probe

The owner's `windowed_ddraw_probe(4).log` confirms
a complete **PASS** on v0.1.29 under Wine. Relevant results:

- DirectDraw7, QueryInterface(DirectDraw4), DDSCL_NORMAL,
  windowed primary and clipper: all S_OK.
- `RGB565 offscreen 3D target`: S_OK.
- `IDirect3D3::EnumZBufferFormats(HAL)`: S_OK, four
  returned formats, including one pure 32-bit
  DDPF_ZBUFFER with 24 active Z bits and no declared
  stencil: `dwZBitMask=0x00ffffff`.
- Z32 allocation in **explicit video memory**: S_OK.
  Its `GetSurfaceDesc` reports caps `0x10024000`,
  32-bit depth and `dwZBitMask=0x00ffffff`.
- Z attachment to render target, Z DEPTHFILL,
  RGB565 COLORFILL, primary windowed Blt and D3D3 HAL
  CreateDevice: all S_OK. Desktop 1920x1080 preserved.

Therefore the required **real** windowed DirectDraw
resource architecture is available under this owner's
Wine configuration. This does not prove the actual
Crimson Skies COM flip chain can be virtualized safely.

Next v0.1.30 standalone proof uses the same resources
to call `IDirect3DDevice3::BeginScene/EndScene`, then
clear a temporary 960x540 16:9 window client to black
via clipped primary COLORFILL, and stretch the 640x480
offscreen RGB565 surface into a centered 720x540 4:3
viewport (x=120) using a *real* DirectDraw4 Blt.
All HRESULTs and the actual geometry are logged.
A real game borderless/Flip adapter is not yet
implemented; no game DLL behavior changes.

## Stage B3k — owner confirms v0.1.30 PASS; v0.1.31 opt-in borderless diagnostic

The owner's \`windowed_ddraw_probe(5).log\` confirms all
previous v0.1.30 HRESULTs are S_OK, including
IDirect3DDevice3 BeginScene and EndScene and both
windowed primary Blts (black full-client fill,
640x480->720x540 4:3 stretched RGB565).
The client is exactly 960x540 with aspect viewport
x=120,y=0,w=720,h=540. The 1920x1080 desktop is
preserved. The result is API success, not independent
pixel verification.

Version v0.1.31 introduces an **explicitly opt-in**
standalone \`--borderless\` test. It creates a separate
\`WS_POPUP|WS_VISIBLE\` temporary HWND at primary
desktop 0,0, matching current display dimensions.
The same real normal-mode DirectDraw4 clipper, RGB565
D3D3 render target and HAL device are used to
present a three-band red/green/blue RGB565 test
pattern stretched into a centered 4:3 viewport.
For 1920x1080 the planned image is x=240,
y=0,w=1440,h=1080; the remaining left/right
strips are black. Runtime checks reject
wrong client geometry or a repositioned HWND.
No fullscreen display switch, topmost setting,
game window hook, new COM surface object or
input interception occurs. The test window closes
automatically and its log stays alongside the EXE.

This establishes an additional host-level
presentation prerequisite, but it is NOT
in-game borderless and does not implement
a virtual DirectDraw primary/flip chain.
Retain PR #6 as draft until game validation.

## Stage B4a — v0.1.31 borderless probe PASS and inactive Flip-to-Blt transaction

The owner supplied \`windowed_ddraw_probe(7).log\`
from the *correct* \`--borderless\` run of the
v0.1.31 probe. Every recorded HRESULT is S_OK:
normal DirectDraw4, HWND clipper, RGB565+Z32
Direct3D3 HAL with BeginScene/EndScene, three
RGB565 color-bar fills, and both primary Blts.
The test window was exactly 1920x1080 at 0,0;
the 640x480 source was presented into x=240,
y=0,w=1440,h=1080 with black sidebars.
Host desktop remained unchanged. Pixel-level
visual correctness still requires owner feedback.

The next engineering gap is genuine in-game
IDirectDrawSurface4::Flip virtualization.
The new \`borderless_flip.rs\` implements an
**inert pure contract** for a deterministic
Flip-to-Blt sequence and catches stale mode
generations, mixed 640x480/800x600 calls,
partial-presentation failures, and false
success or implicit post-exposure fallback.
It tests both black full-client clear and
RGB565 viewport stretch, successful only as
a paired transaction.

This module has no real COM object, HWND
or DirectDraw method call: using it alone
cannot activate borderless in the game. A
future opt-in runtime must first create
a **complete virtual IDirectDrawSurface4**
primary/backbuffer/Z interface, preserve
IUnknown physical identity and attachment
reference counts, provide the real RGB565
3D-capable target to IDirect3D3::CreateDevice,
and only then route Flip through this
transaction backed by actual windowed
DirectDraw4 Blts.

No existing game rendering path or INI
setting is changed by v0.1.32.

## Stage B4b — v0.1.33 shared real Win32 backend seam

The pure \`borderless_flip.rs\` now has a reusable
\`execute_windowed_blt_pair\` API that checks the exact
internal 640x480 or 800x600 16bpp source geometry
and integer 4:3 destination viewport, then invokes
backend black-clear and RGB565-stretch in order.
The fail-closed \`FlipToBltBridge\` shares this
same transaction and additionally enforces actual
virtual COM exposure, display-mode epoch and
no fallback after a partial frame.

This enables a separate real Win32 DirectDraw4
standalone probe to implement \`WindowedBlt\`
and exercise the same two actual Blts, without
misrepresenting its test objects as a fully
virtualized game primary/backbuffer/Z chain.

No game-visible new COM objects are exposed
and no active DirectDraw4 hooks are changed.
The production virtual COM surface chain
still requires engineering and verification.

## Stage B4b runtime seam — v0.1.33 host-owned COM surfaces

The standalone Win32 x86 probe now imports the same
\`borderless_flip::execute_windowed_blt_pair\` shared
by the game-side fail-closed transaction model.
A real \`NativeWindowedBlt\` backend borrows the
validated windowed primary/clipper and real 640x480
RGB565 offscreen \`IDirectDrawSurface4\` to perform
the actual black-clear and stretch. The game-side
COM adapter remains unexposed.

The optional **\`--borderless-loop\`** flag runs
120 physical source color fills, D3D3 scene pairs
and 120 clipped black-clear + RGB565 scaled Blt
pairs, logging progress in groups of 30 and the
first HRESULT failure. The operation count is
not an actual gameplay frame-rate measurement.
The existing one-frame windowed and borderless
tests are unchanged. The host desktop remains
in normal DDSCL mode, with no display switch.

A future in-game bridge still requires a complete
physical COM virtual primary/backbuffer/depth
surface chain and verified IUnknown identities,
not just this working real backend.

## Stage B4c — v0.1.34 shared real DirectDraw4 physical presenter

The owner's v0.1.33 `--borderless-loop` log confirms exactly `FLIP-BLT-RESULT: requested=120 successful=120 elapsed_ms=1508 (NOT gameplay FPS)` and `PROBE COMPLETED: PASS`. Every real DirectDraw4 windowed primary presentation succeeded. Test window 1920x1080 at 0,0, 4:3 region 1440x1080 at x=240; the desktop mode remained unchanged.

The NativeWindowedBlt implementation has been extracted to an identical shared source module used by the standalone test and **compiled but inactive** inside `ddraw.dll`. The module's unsafe constructor requires physical IDirectDrawSurface4 pointers and caller-maintained COM ownership; geometry and HRESULTs are explicitly checked. Three isolated mock-vtable tests cover flags, actual screen-space viewport rectangles, color fill, propagation of a failing Blt and refusal to dispatch invalid geometry.

No virtual game COM interface, GetAttachedSurface handler or active in-game Flip hook was added; the game path is still exclusive/unchanged. This is a safe engineering prerequisite, not a borderless game feature. Owner follow-up may use the unchanged `--borderless-loop` option with the v0.1.34 separate EXE.
## Stage B4d — typed COM graph integration tests, v0.1.35

The owner's v0.1.34 standalone log once again confirms borderless `FLIP-BLT-RESULT: requested=120 successful=120 elapsed_ms=1512 (NOT gameplay FPS)` and preserved 1920x1080 host display. The standalone physical NativeWindowedBlt implementation is considered validated at the DirectDraw HRESULT level.

The new test-only `surface_graph_tests` within `com_surface4_abi.rs` combines the *exact 45-slot native Win32 x86 Surface4 vtable*, the existing `SurfaceContract` ownership state machine, and the fail-closed `FlipToBltBridge` into one in-memory mock COM graph. These tests cover real pointer dispatch of IUnknown and IDirectDrawSurface4, primary/backbuffer/Z attachments, reference lifetimes, descriptor/caps reads, and a simulated Flip success/failure latch.

All mock object allocations remain owned by the Rust test Rig; Release to zero changes logical graph lifetime but does not deallocate that test memory. The fixture advertises no unimplemented interface versions and does not exercise the actual game process. The full game-used Surface4 COM implementation and VRAM backing remain prerequisites for a safe production bridge. No game hook changes.
## Stage B4e — v0.1.36 real COM forwarding object, still inactive

The 45-slot typed Surface4 ABI from Stage B3b is now implemented by a real, compilable COM forwarding façade in `com_surface4_forward.rs`. It owns an independent IUnknown identity and one real strong underlying Surface4 reference. QI advertises only IUnknown and IDirectDrawSurface4; AddRef/Release are atomic; the last client Release drops the façade and exactly one underlying COM reference. All other 42 methods delegate through exact typed original COM function pointers.

This solves **ABI-complete forwarding and object reference lifetime**, not the game primary/backbuffer/Z topology: forwarding `Flip` remains physical Flip and forwarding GetAttachedSurface returns an underlying physical attached surface. Therefore **the module has no runtime caller** and is forbidden from being exposed as a game windowed virtual surface. The next stage will make role-specific virtual primary methods provide the observed chain and route Flip to the already validated NativeWindowedBlt backend, without disturbing the renderer's real RGB565 Direct3D3 render target. Additional interface versions must not be advertised until implemented.

The owner has separately confirmed v0.1.34 standalone borderless loop 120/120 successful; no new Wine test is needed for this COM-only step.
## Stage B4f — v0.1.37 standalone real native Surface4 COM forwarding test

Following the 45-method, refcounted COM forwarder implementation (B4e), the next validation uses **genuine Windows x86 DirectDraw4 COM surfaces under Wine** rather than mocked vtables. The standalone `--com-forward` mode wraps the real RGB565 640x480 3D target with its genuine Z32 attachment and verifies COM QI identity, unsupported IID behavior, counts, real GetCaps/GetSurfaceDesc, physical GetAttachedSurface, real Z descriptor, and actual RGB565 green ColorFill through the wrapper.

Critically, the returned attachment remains the original **unwrapped** physical Z buffer; this must not be advertised as virtual game attachment topology. An independent temporary RGB565 surface then tests underlying AddRef lifetime by dropping the original COM owner while the wrapper is still live. The test never passes the wrapper to IDirect3D3::CreateDevice, which still uses the original physical target; thus this is an isolated live-COM smoke test rather than an in-game 3D rendering test through a virtual surface.

All steps report detailed HRESULTs in the adjacent log and the normal borderless presentation checks still run. No game mode switch, COM hook installation or active game DLL changes.
## Stage B4g — v0.1.38 test Direct3D3 device creation on a forwarded Surface4

The owner verifies v0.1.37 live COM-forward behavior under Wine with native IUnknown/Surface4 identity, physical attached Z32 and actual RGB565 Blt. All expected HRESULTs are S_OK, the wrapper retains an independent original Surface4 reference across owner destruction, and the 1920x1080 desktop remains unchanged. Its COM wrapper is nevertheless not yet used as the render target of a D3D3 device.

Version 0.1.38 adds **only** an opt-in `--com-device` standalone experiment: preserve the real normal-mode primary, real 640x480 RGB565 surface with true Z32 attachment, wrap the RGB565 surface in the full typed 45-slot COM forwarding façade and pass **the wrapper** into `IDirect3D3::CreateDevice(HAL, wrapper)`. The returned real HRESULT is logged, with no fallback to the original target if the test fails. If it succeeds, the test runs BeginScene/EndScene and the existing windowed Blt presentation and records `COM-DEVICE-RESULT: PASS`.

This test intentionally probes an implementation-defined boundary: Direct3D3 may require additional COM identity/interface information that the forwarding wrapper does not advertise. A failed HRESULT or crash will be used to plan the next integration step; it does not imply any regression in the native physical DirectDraw renderer. No game DLL hook calls this new code.
## Stage B4h — v0.1.39 Wine HAL boundary requires original internal surface

The owner's v0.1.38 standalone log proves the complete native resource chain still works but `IDirect3D3::CreateDevice` rejects the *forwarded* Surface4 with `DDERR_INVALIDCAPS` (0x88760064). Inspection of current Wine `ddraw.c::d3d3_CreateDevice` and `surface.c::unsafe_impl_from_IDirectDrawSurface4` shows an implementation-specific dependency: Wine converts the Surface4 interface pointer to its internal `ddraw_surface` structure. Calling QI for Surface4 on our wrapper does not change its physical address, and thus cannot supply the expected native internal layout.

Stage B4h introduces `try_borrow_real_surface4` to resolve only our own live forwarding object to its borrowed original Surface4 pointer. The optional standalone `--com-device-bridge` mode checks this boundary, calls D3D3 HAL CreateDevice with the real target, reads `IDirect3DDevice3::GetRenderTarget` to ensure the result belongs to the original real target, then confirms scene and scaled presentation. This does **not** install a D3D3 hook in Crimson Skies, replace a real object with a virtual primary or make in-game borderless work. It is a host-level foundation for a future controlled D3D3 intercept.
## Stage B4i — v0.1.40 real D3D3 render target COM identity roundtrip

After the owner confirmed v0.1.39 `--com-device-bridge` PASS, the next unresolved requirement is not merely successful CreateDevice but returning **the same game-facing COM identity** when the game asks the device for its render target. `com_surface4_roundtrip.rs` implements a deliberately small, inactive one-target map that owns a real AddRef to the wrapper, exposes the borrowed native Surface4 to the HAL only, and remaps a matching physical GetRenderTarget result back to that original wrapper with its own AddRef. It rejects unrelated native pointers and preserves independent ownership of physical GetRenderTarget results.

The standalone v0.1.40 `--com-device-roundtrip` performs the real native HAL CreateDevice, GetRenderTarget and IUnknown round trip before the already validated scene and borderless Blt. This is still not a full game-facing primary/backbuffer/Z graph or active D3D3 intercept; those require explicit mapping and lifecycle management for each logical surface role.
## Stage B4j — v0.1.41 graph-specific COM attachment ABI

Confirmed v0.1.40 owner log: 1920x1080 borderless, native RGB565 D3D3 HAL, `GetRenderTarget` mapped from genuine Surface4 to identical game-facing forwarding object, `COM-ROUNDTRIP-FINAL: PASS`.

Version 0.1.41 adds graph-specific `IDirectDrawSurface4` forwarding mode with three distinct native-backed wrapper identities, strong acyclic parent->child attachment refs, valid virtual 640x480/800x600 RGB565/Z32 descriptors, caps, pixel formats and actual typed GetAttachedSurface vtable behavior. Native/unwrapped surfaces remain solely for Wine's HAL. Unsafe virtual `Flip`/`Blt`/`Lock` and unimplemented attachment mutation paths return `E_NOTIMPL` instead of accidentally forwarding to physical exclusive operations. The standalone `--com-surface-chain` tests graph construction, client-ref lifetimes, parent/back/depth identity, native renderer creation and existing physical windowed presentation. This is a separate host validation, not an active runtime game hook.
## Stage B4k — v0.1.42 virtual COM Flip now reaches real windowed DirectDraw Blts, probe only

The owner-confirmed 0.1.41 `--com-surface-chain` log verifies all native DirectDraw4 COM ABI roles, attached surfaces, real HAL RGB565/Z32 render target and 4:3 physical presentation. The isolated 0.1.42 experiment introduces `arm_graph_primary_flip` on the typed 45-slot Surface4 forwarding facade, with a complete graph + exposed physical-backend contract and a one-time opt-in. Unarmed, wrong-target and unsupported-flag Flip calls remain fail closed; armed valid Flip invokes the existing `FlipToBltBridge` and NativeWindowedBlt pair, acknowledging success only after black clear and stretched RGB565 Blt.

`--com-virtual-flip` tests 120 actual COM-vtable Flip invocations against genuine live DirectDraw4 primary/backbuffer/Z objects while keeping D3D3 HAL bound to the native RGB565 surface. It records successful frames, checks frame counts, propagates real HRESULT failures, preserves the physical display resolution, and distinguishes its timing from gameplay FPS. This test does not install a game hook; the DLL still does not expose the graph or arm the presenter. The remaining production task is intercepting real game DirectDraw4 CreateSurface/Flip/D3D3 CreateDevice with full method coverage, lifecycle and safe fallback before exposure.
## Release gates

- Keep the PR in draft and never merge without owner validation.
- Dist artifact layout stays `ddraw.dll` at game root,
  `CrimsonSkiesFix/` for docs and INI, and optional
  `CrimsonSkiesFix.log` beside the loaded DLL.
- Never ship an active `CrimsonSkiesFix.ini` or log; ship only the
  `CrimsonSkiesFix.ini.example` template to preserve user settings.
- x86 Windows CI, tests, and gameplay evidence required before enabling
  any behavior change by default.


## Stage B4l — v0.1.43 native 800×600 virtual-Flip parity probe (inactive in the game)

The owner-provided v0.1.42 `--com-virtual-flip` log (`windowed_ddraw_probe(20261030-165232).log`) confirms on the owner's Wine setup: DirectDraw7→DirectDraw4, `DDSCL_NORMAL`, primary+HWND clipper, 640×480 RGB565 target, enumerated pure 32-bit HAL depth format in video memory, native D3D3 HAL scene, physical→virtual backbuffer render-target identity, correct 640×480→1440×1080 centered letterboxing, and **120/120 successful** `IDirectDrawSurface4::Flip`→real two-Blt presentations. Elapsed 1507 ms is a synthetic test timing, **not gameplay FPS**. This evidence does not prove that the real game or 800×600 works.

The opt-in `--com-virtual-flip-800` test now creates a genuine **800×600** RGB565 3D target and same-sized 32-bit Z surface, advertises **800×600** virtual descriptors on three separate Surface4 COM identities, validates attachments and QI, maps the HAL render-target pointer back to the correct game-facing backbuffer identity, and presents through the existing guarded/armed virtual Flip implementation. The 640×480 flag remains available and unchanged. The standalone test does not hook or enable the actual game's exclusive fullscreen path. Once both modes have live owner evidence, the next engineering challenge remains in-game interception, correct surface lifetimes and the transition between 800×600 and 640×480, without silently falling back to native exclusive Flip.


## Stage B4m — v0.1.44 800×600 → 640×480 → 800×600 surface-epoch lifetime probe

The owner-provided v0.1.43 `windowed_ddraw_probe(20261030-171410).log` confirms **PASS** for the 800×600 isolated real windowed primary+clipper, RGB565+Z32, 3-object virtual COM graph, native D3D3 HAL/round-trip identity and **120/120** virtual Flip→windowed two-Blt operations. Letterboxed 800×600 to 1440×1080 at x=240 in 1920×1080; desktop preserved. 1509 ms is synthetic probe time, not gameplay FPS. Together with earlier 640×480 PASS, this proves independent resolutions only; not safe resource recreation or in-game transitions.

The standalone v0.1.44 `--com-virtual-flip-transition` performs three **entirely separate render-target/Z32/virtual Surface4/D3D3-device epochs** while preserving **one HWND/DirectDraw4/primary+clipper in DDSCL_NORMAL**. Each epoch uses its own checked source resolution, virtual descriptors, Z surface, HAL device, attached graph, one-time graph arm and 120 presentations. Per-epoch owners are destroyed by Rust scope before the next allocation; virtual flip frame counter starts at zero each time and generation increments. The log records each epoch. The old single-resolution modes are unchanged, and no game hooks are added.

**Limit:** the existing real windowed *primary* and clipper are reused rather than rebuilt; the real game still uses native fullscreen `SetDisplayMode` and its own original COM construction. Thus even a three-epoch PASS cannot certify the actual game's dynamic mode switch. Do not enable the game borderless feature until the real COM interception and lifetime contracts are implemented and tested under Wine.
