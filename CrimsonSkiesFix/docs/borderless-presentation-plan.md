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

## Release gates

- Keep the PR in draft and never merge without owner validation.
- Dist artifact layout stays `ddraw.dll` at game root,
  `CrimsonSkiesFix/` for docs and INI, and optional
  `CrimsonSkiesFix.log` beside the loaded DLL.
- Never ship an active `CrimsonSkiesFix.ini` or log; ship only the
  `CrimsonSkiesFix.ini.example` template to preserve user settings.
- x86 Windows CI, tests, and gameplay evidence required before enabling
  any behavior change by default.
