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
- Dist artifact layout stays `ddraw.dll` at game root and
  `CrimsonFix/` subfolder for docs, INI and logs.
- Never ship the active `CrimsonFix.ini`; ship only the
  `CrimsonFix.ini.example` template to preserve user settings.
- x86 Windows CI, tests, and gameplay evidence required before enabling
  any behavior change by default.
