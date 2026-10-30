# CrimsonSkiesFix 0.1.40 — experimental DirectDraw proxy

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

## v0.1.40 — round-trip COM identity across the native D3D3 boundary

The owner's v0.1.39 `--com-device-bridge` log is a full **PASS**: HAL `CreateDevice` accepts the borrowed original RGB565 Surface4, `GetRenderTarget` returns that exact physical target, BeginScene/EndScene succeed and 640x480 to centered 1440x1080 presentation succeeds in the unchanged 1920x1080 desktop.

Stage B4i adds an **inactive single-target identity map** in `src/com_surface4_roundtrip.rs`. It preserves one game-facing Surface4 wrapper reference, retrieves the real physical target only for native HAL calls, and maps the physical result of Direct3D3 `GetRenderTarget` back to the **original, same COM wrapper identity** with a new caller-owned AddRef. The original physical result still requires independent COM Release. It refuses foreign native results and does not manufacture or advertise other Surface4 interface versions. Dedicated x86 mock-COM unit tests cover these reference, QI, null and foreign pointer conditions.

The standalone program now offers **`--com-device-roundtrip`**. It creates the genuine RGB565+Z32 resources, wraps the target, registers the wrapper/native pair, calls native `IDirect3D3::CreateDevice(HAL, native)`, verifies physical `GetRenderTarget`, then remaps that returned physical surface to its original game-facing wrapper and checks `IUnknown` pointer identity before scene and presentation. It logs `COM-ROUNDTRIP-FINAL: PASS` only if the entire chain succeeds.

This is a narrow test for **one** D3D render target, not yet a virtual primary/backbuffer/Z-buffer chain, a texture/surface registry, or a hooked Crimson Skies implementation. The original DLL behavior is unchanged. Run the new opt-in test from the standalone Windows x86 artifact outside the game directory: `wine ./windowed_ddraw_probe.exe --com-device-roundtrip` and share the adjacent log.

## v0.1.39 — HAL device boundary must unwrap the COM Surface4 wrapper

The owner's v0.1.38 `--com-device` log shows `IDirect3D3::CreateDevice(HAL, forwarded RGB565 Surface4)` fails with **0x88760064 = DDERR_INVALIDCAPS**. All preparatory real DirectDraw/HAL Z32 allocations and Z attachments still succeed. Wine's `dlls/ddraw/ddraw.c::d3d3_CreateDevice` calls `unsafe_impl_from_IDirectDrawSurface4`, which assumes its own internal `struct ddraw_surface`. Wine's `dlls/ddraw/surface.c` implementation accepts the pointer only via native vtable or QueryInterface and then uses `CONTAINING_RECORD`: our independent COM wrapper cannot be treated as that internal surface.

To test a **safe future interception boundary**, v0.1.39 adds `com_surface4_forward::try_borrow_real_surface4` with an exact known-wrapper vtable check. This returns a **borrowed physical DirectDraw4 surface** only from our live forwarding object, never from an unrelated native surface. Callers must keep their original wrapper reference alive throughout the native operation. A new x86 test validates wrapper/foreign/null recognition, IUnknown aliasing and underlying COM ownership.

The standalone EXE adds explicit **`--com-device-bridge`** mode. It creates exactly the same full 45-method Surface4 wrapper, checks its own unwrapping API, then calls the genuine `IDirect3D3::CreateDevice(HAL)` with **the original physical RGB565 target**, not the wrapper. If creation works, it verifies `IDirect3DDevice3::GetRenderTarget` returns the original real target (not the wrapper), tests BeginScene/EndScene and the previous borderless Blt. Each gate reports an exact HRESULT and never silently retries.

Crucially, this is still a host-only demonstration. The **actual game's IDirect3D3::CreateDevice** needs a separate opt-in intercept to unwrap only our owned Surface4 COM handles. No replacement is installed yet. The old `--com-device` failing experiment remains available for diagnostics, while all previous successful test modes remain unchanged.

**Owner test:** download only `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL`, extract outside the game directory and run `wine ./windowed_ddraw_probe.exe --com-device-bridge` in the previously validated Wine/GE-Proton environment. Send `windowed_ddraw_probe.log` even if a specific stage fails. The experimental DLL should not be installed for this test.

## v0.1.38 — opt-in Direct3D3 HAL device creation using a real Surface4 forwarder

The owner's v0.1.37 `--com-forward` log reports **COM-FORWARD-RESULT: PASS** under Wine: independent `IUnknown` / `IDirectDrawSurface4` identity, QueryInterface/AddRef/Release, real RGB565 caps and descriptors, physical attached Z32, successful forwarded Blt, and a real underlying surface kept alive after release of its original owner. A separate native color/Blt presentation also passed with desktop 1920x1080 preserved.

To test the next critical compatibility prerequisite, the standalone EXE adds an **explicitly opt-in `--com-device` mode**. Only in this mode, the probe passes a real RGB565 640x480 DirectDraw4 surface **through the 45-method forwarder** as the `pRenderTarget` parameter to **`IDirect3D3::CreateDevice(CLSID_D3D_HAL)`**. Its original physical RGB565 target and Z32 attachment remain alive, and the COM forwarder is kept alive throughout device use. The test then attempts real `IDirect3DDevice3::BeginScene/EndScene` and windowed Blt presentation, without switching desktop mode.

**A failed CreateDevice HRESULT is a valuable diagnostic:** it shows which obstacle remains between successful native COM delegation and Direct3D3 accepting the wrapper. The probe will log the exact HRESULT and **will not silently retry** with the original surface, or claim a successful virtual target. This mode is entirely separate from the already validated default, `--borderless`, `--borderless-loop` and `--com-forward` modes.

Use the separately built Windows x86 `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL` artifact, extract to a writable folder outside the game installation and run `wine ./windowed_ddraw_probe.exe --com-device` in the same Wine/GE-Proton environment. Attach its adjacent `windowed_ddraw_probe.log`. The actual game DLL's rendering and installed INI remain unchanged, and no virtual primary/Flip chain is exposed to the game.

## v0.1.37 — first native COM forwarder smoke test under Wine

Stage B4f connects the new **full 45-method Surface4 COM forwarder** to the *real DirectDraw4 runtime* in the separately distributed standalone `windowed_ddraw_probe.exe`. The new explicitly opt-in `--com-forward` mode launches a temporary 1920x1080 borderless diagnostic window with **normal/non-exclusive DirectDraw**, creates the real RGB565+D3D3 target and Z32 depth attachment, and then exercises the forwarding COM object against genuine Windows/Wine COM resources.

The test checks: two independent `QueryInterface` calls on the wrapper retaining the same physical `IUnknown`/`IDirectDrawSurface4` identity; AddRef/Release counts; `E_NOINTERFACE` and zeroed output for unsupported IIDs; real `GetCaps` and `GetSurfaceDesc` returning 640x480 RGB565; *physical* `GetAttachedSurface(Z32)` and the original 32-bit depth format; and `Blt` through the forwarding vtable. It also creates an **isolated second real RGB565 surface**, drops its original owner, confirms the wrapper's strong underlying COM reference keeps the surface usable, and finally releases that wrapper.

The legacy windowed, `--borderless` and `--borderless-loop` test modes remain unchanged; even `--com-forward` performs the existing native presentation Blt with the real target, **never** passes a virtual primary to Direct3D3, intercepts the game or changes display mode. All failures are logged in `windowed_ddraw_probe.log` beside the test EXE.

**Owner test:** extract the separate Windows x86 `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL` artifact to a writable directory outside the game folder and run `wine ./windowed_ddraw_probe.exe --com-forward` under the same Wine/GE-Proton environment. Send `windowed_ddraw_probe.log` afterward. The test is host-side only and the game DLL's actual rendering path remains unchanged.

## v0.1.36 — full 45-method Surface4 forwarding ABI and owned IUnknown

Stage B4e creates `src/com_surface4_forward.rs`, a real Win32 x86 `IDirectDrawSurface4` forwarding object **compiled but never instantiated in the game DLL**. It uses the previously verified *exact 45-slot typed vtable*, with all non-IUnknown methods forwarded to an original **real** Surface4 COM object, rather than returning misleading placeholder success.

The façade has its own stable physical `IUnknown` / `IDirectDrawSurface4` identity, returns `E_NOINTERFACE` for interfaces whose ABI it does not implement, maintains an independent atomic COM client-reference count, acquires exactly one strong reference to the original surface at construction, and releases that reference exactly once on final façade `Release`.

Four new x86 tests check all 45 vtable slots, stable QueryInterface identity and counts, real forwarding of Blt/Flip/GetAttachedSurface/Unlock with original HRESULT and pointer values, null pointer argument handling, and final ownership. The full real-method table is checked at compile time because it must initialize all 45 vtable fields.

**This is deliberately NOT the game borderless virtual primary.** Its GetAttachedSurface, Flip, GetSurfaceDesc, caps and all other graphics operations still describe the **physical original** surface. Real attachment returned pointers are not wrapped, and it is not yet integrated with the fail-closed borderless Flip bridge or active DirectDraw4 hooks. The game still runs its unchanged original rendering path. This step establishes ABI-complete forwarding and correct COM identity as a prerequisite for safely implementing selective virtual primary semantics.

## v0.1.35 — test-only physical Surface4 COM graph (ABI, references, Flip)

The user's v0.1.34 `--borderless-loop` log reports **120/120 successfully completed real DirectDraw4 presentation pairs** in the borderless 1920x1080 test window, with 1440x1080 centered RGB565 output and unchanged desktop. All HRESULT checks passed. The recorded 1512 ms includes test sleeps and is **not game FPS**.

**Stage B4d** adds an **x86 unit-test-only** physical COM graph inside `com_surface4_abi.rs`, reusing the previously verified 45-entry `Surface4VTable` type. Three separate, stable mock object addresses emulate logical primary, backbuffer and depth. The mock `QueryInterface(IUnknown/IDirectDrawSurface4)` preserves each physical identity, the typed `GetAttachedSurface` ABI returns the correct child pointer and consumes an independent client reference, and `GetCaps` / `GetSurfaceDesc` report the modeled RGB565/Z32 sizes and capabilities.

A test-only primary `Flip` vtable function calls the already fail-closed `FlipToBltBridge`: every successful logical Flip requires both black clear and stretch; failed presentation is returned to the caller and subsequent flips do not silently fall back to exclusive rendering. Tests check reference lifetimes after releasing the parent, unsupported interface rejection, real vtable slot signatures and 2-frame success followed by a partial-frame failure.

**This is not a production COM facade.** The graph is compiled only under `#[cfg(test)]`, its remaining Surface4 methods deliberately return `E_NOTIMPL`, the fixtures own their memory rather than COM Release freeing it, and the backend uses simulated presentation steps. The actual `ddraw.dll` never creates or exposes these mocks. The next phase must implement all game-used COM surface methods, real VRAM render-target ownership and proper dynamic reference-counted destruction before changing the game's `Flip`.

## v0.1.34 — 120/120 real presentations confirmed; native Blt backend shared

The owner-provided `windowed_ddraw_probe(8).log` from the **v0.1.33 --borderless-loop** test reports **120/120 successful real DirectDraw4 pairs** in a 1920x1080 borderless HWND, each pair comprising black primary clear and RGB565 stretch into x=240, y=0, 1440x1080. Direct3D3 HAL, Z32 and scene calls also pass. The elapsed 1508 ms includes test sleeps and **is not game FPS**. The log cannot alone prove pixel appearance.

Stage B4c creates `src/borderless_native_blt.rs`: a single **real physical DirectDraw4 windowed Blt backend** now compiled in both the standalone test and the DLL. It implements the shared `WindowedBlt` API, validates client/source/viewport rectangles and integer overflow, and calls the actual Surface4 vtable Blt slot 5 with proper DDBLTFX and HRESULT propagation. The code borrows live COM objects, does not AddRef/Release, and may be constructed only by a caller that owns a valid windowed primary with clipper and a real RGB565 offscreen target.

Three x86 tests use a fake 45-slot COM vtable to verify exact black-clear then stretch calls, desktop coordinate translation, flags, DWORD-20 fill and error/invalid-geometry handling.

The **standalone EXE v0.1.34 uses this exact same physical module** with its existing logging and 120-frame optional `--borderless-loop` test. No other standalone behavior is intended to change. The game DLL only compiles the new module; **it does not construct or use it in gameplay**. The full virtual primary/backbuffer/Z-buffer COM interface and safe Flip interception remain pending. The original game graphics, FPS behavior and INI remain unchanged.

To verify the refactor under Wine, run the separate standalone `windowed_ddraw_probe.exe --borderless-loop` (from a writable folder outside the game) and attach its adjacent `windowed_ddraw_probe.log`.
## v0.1.33 — connect Flip-to-Blt transaction to a real standalone backend

The owner confirmed a full API-level **PASS** for the
1920x1080 borderless DirectDraw test in v0.1.31,
including the native RGB565-to-1440x1080 4:3
stretched Blt with the monitor mode unchanged.

The shared \`src/borderless_flip.rs\` now exposes an
\`execute_windowed_blt_pair\` function: it strictly
validates internal mode, destination geometry and
the ordering of black clear followed by RGB565 stretch.
The existing fail-closed game-side
\`FlipToBltBridge::present\` calls this same function,
rather than duplicating its logic. New tests ensure
a failed clear does not attempt presentation and
a failed stretch is not counted as a completed frame.

The standalone x86 test EXE now imports this **same
two-Blt transaction** and provides a \`WindowedBlt\`
implementation backed by *real DirectDraw4 COM
surfaces*, not simulated buffers. The existing
default and \`--borderless\` options still execute
a single frame, as in the previously validated
standalone tests.

A new explicit \`--borderless-loop\` option attempts
**120 RGB565 color fills, 120 additional D3D3 scene
pairs and 120 black-clear + scaled viewport Blt
pairs** in its own 1920x1080 window. It reports
per-stage HRESULT failures, bounded successful-frame
progress, and a final summary. The loop's timing is
**not game FPS**, and the test does not intercept
Crimson Skies or call its exclusive \`Flip\`.

Run from a writable folder **outside the game
installation**, under the same Wine/GE-Proton:
\`wine windowed_ddraw_probe.exe --borderless-loop\`.
Send \`windowed_ddraw_probe.log\` afterward.
The test window closes automatically.

The actual game still uses its original exclusive
DirectDraw4 Flip chain.

Only a standalone host-level test is presently available.
A complete game-facing virtual primary/backbuffer/Z COM
implementation, proper reference ownership and compatible
GetAttachedSurface/Flip behavior remain required before
the game's \`Borderless=1\` can be made active.

## v0.1.32 — first inactive Flip-to-Blt bridge contract, owner borderless PASS

The owner-provided **v0.1.31** standalone log confirms
\`--borderless\` ran successfully under Wine: private
\`WS_POPUP\` test HWND exactly \`1920x1080\` at screen
origin \`x=0,y=0\`; genuine RGB565 640x480 and video-
memory Z32 surfaces; real Direct3D3 HAL BeginScene/
EndScene; red/green/blue RGB565 fills; and successful
clipped windowed primary black COLORFILL followed by
scaled Blt to \`x=240,y=0,1440x1080\`. The desktop
mode was preserved. This is API-level confirmation,
not captured-pixel verification.

**Stage B4a** introduces \`src/borderless_flip.rs\`,
a *pure, inactive and test-driven* model of converting
one logical game's Flip into precisely **two** required
windowed DirectDraw operations: first clear black,
then stretch the real RGB565 backbuffer into the
centered 4:3 viewport. The new
\`WindowedBlt\` trait defines the eventual backend
interface; no Win32, COM, game DLL hook, or real
GPU call is made by the new module.

Crucially, the bridge can be created only after
\`AdapterContract\` has reached \`Phase::Exposed\`
(which will itself require a complete COM
implementation in the future). It rejects stale
display-mode generations and wrong source sizes,
reports a successful logical Flip only after
**both** Blts complete, and latches any failed
step without silently switching an already
exposed virtual primary back to exclusive mode.

Eight regression tests cover 640x480 and 800x600
at 1920x1080, 120 back-to-back logical frames,
black-clear or stretch failure, wrong generation,
changed mode and counter overflow.

This is a necessary bridge contract but **not yet
the in-game borderless feature**. The runtime's
real virtual primary/backbuffer/Z chain and
physical COM ownership/attachments must be
implemented and tested before any opt-in game
activation. The existing \`ddraw.dll\` rendering,
modern Flip pacing, configuration and probe
behavior remain exactly as before.

## v0.1.31 — optional real-size borderless DirectDraw presentation test

The owner's v0.1.30 standalone log is a full PASS:
normal-mode DirectDraw, RGB565 640x480 D3D3 target,
video-memory Z32 (24 useful depth bits), D3D3 HAL
BeginScene/EndScene, windowed primary COLORFILL,
and a real DirectDraw stretch Blt to a **720x540 4:3**
centered region in a 960x540 client. The host desktop
remained **1920x1080**.

The **separate** standalone test EXE now accepts an
optional **\`--borderless\`** switch. With no arguments,
the already-validated 960x540 framed-window test
remains the default. With the switch, the EXE creates
its **own temporary WS_POPUP (no border) window**,
sized to the *primary desktop* (1920x1080 in the
owner's configuration); no exclusive cooperative
mode, SetDisplayMode, game HWND hook or topmost
setting is used.

The EXE checks actual client size and screen origin,
draws red/green/blue RGB565 stripes on the offscreen
test surface, black-fills the window through a real
DirectDraw clipper, and scales the 640x480 surface
to a centered **1440x1080 4:3 viewport at x=240**.
It logs the real HWND/client geometry and HRESULTs
before closing its test window automatically.
The monitor's display resolution must remain unchanged.
The opt-in window appears for roughly three seconds;
it cannot trap the user in permanent fullscreen.

For the next test, keep the standalone EXE **outside
the game directory** and run it under the same Wine/
GE-Proton environment as before, with the argument:
\`windowed_ddraw_probe.exe --borderless\`.
Attach \`windowed_ddraw_probe.log\` and report whether
three colored vertical bands appear in the center
with black pillarboxes left/right.

**Scope:** this validates a temporary test window,
not the game itself. No COM virtual primary/Flip
implementation exists yet. The actual game
\`ddraw.dll\`, EXE, settings and FPS behavior are
unchanged; the test EXE is distributed separately.

## v0.1.30 — verify real windowed D3D scenes and 4:3 stretched presentation

The owner's standalone v0.1.29 log reports **PASS** for
the complete windowed resource chain on Wine 11.0,
GE-Proton11-11: IDirectDraw7/4, DDSCL_NORMAL, primary
and HWND clipper, RGB565 offscreen `DDSCAPS_3DDEVICE`,
32-bit video-memory Z buffer with **24 active Z bits**
(`dwZBitMask=0x00ffffff`), Z attachment, Z DEPTHFILL,
RGB COLORFILL, windowed primary Blt, and
`IDirect3D3::CreateDevice(HAL, RGB565 target)`.
The desktop remains 1920x1080. This validates resources
and API call completion, not actual game borderless.

To test the next prerequisites, the **standalone probe only**
now creates a private temporary window with a requested
**960x540 client area** (16:9), exercises real
`IDirect3DDevice3::BeginScene/EndScene`, clears the
window to black via window-clipper-constrained
`primary.Blt(COLORFILL)`, and attempts to **stretch**
the 640x480 RGB565 offscreen surface into a centered
720x540 (4:3) viewport: x=120, y=0, black pillarboxes.
Logs report the *actual* client dimensions, integer
aspect-fit viewport and all DirectDraw/Direct3D HRESULTs.

This is a temporary diagnostic window, **not fullscreen
and not the actual game's HWND**. A successful Blt HRESULT
does not independently verify displayed pixels or user
input behavior. The production COM virtual primary/Flip
adapter is still pending.

Download the separate `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL`
artifact, extract it **outside the game directory**, run
under Wine and attach `windowed_ddraw_probe.log`.
No change to the game DLL, INI, graphics paths or original
game executable.

## v0.1.29 — correct the actual Z mask field and enumerate HAL Z formats

The v0.1.28 owner test still returns
`DDERR_INVALIDPIXELFORMAT (0x88760091)` with both
forced video memory and driver-selected Z32 allocation.

A **real ABI bug** remained: the probe wrote its proposed
Z32 mask to `DDSURFACEDESC2[22]` (the stencil-bit-depth
field), while Wine's `DDPIXELFORMAT.dwZBitMask` actually
occupies **DDSURFACEDESC2[23]**. The field layout is
documented in Wine's `include/ddraw.h`; the previous
0.1.28 claim of having fully corrected the descriptor
was therefore premature.

Version 0.1.29 writes `dwZBitMask=0xffffffff` at word
23, leaving the stencil-bit-depth word 22 zero. Before
allocating Z32, it now uses **real
`IDirect3D3::EnumZBufferFormats(HAL)`** and logs all
returned pixel-format flags, bits, stencil bits, Z masks
and stencil masks. If a pure 32-bit Z format is advertised,
the exact reported `DDPIXELFORMAT` is copied into
the Z32 creation descriptor. Otherwise, the probe
attempts the corrected standard pure-Z32 descriptor and
reports the result instead of claiming support.

The previous strict video-memory and optional driver-
selected-memory **32-bit only** tests, real `GetSurfaceDesc`
verification, depth fill, clipper-constrained window Blt,
and HAL `CreateDevice` remain. The log now also reports
the **actual** returned depth mask at word 23.

Only the independent `windowed_ddraw_probe.exe` is
changed. Install it outside the game directory, run
it under Wine and send `windowed_ddraw_probe.log`.
No game DLL code or in-game DirectDraw behavior changed.

## v0.1.28 — correct Z32 pixel mask and probe real depth fill

The owner-provided `windowed_ddraw_probe(1).log` from v0.1.27
confirms successful `DirectDrawCreateEx(IDirectDraw7)`,
`QueryInterface(IDirectDraw4)`, `DDSCL_NORMAL`,
the windowed primary/clipper and the **RGB565 offscreen 3D
target**. It stops at `CreateSurface(Z32)` with
`0x88760091 = DDERR_INVALIDPIXELFORMAT`.

The standalone test's Z32 descriptor incorrectly had a
**zero `dwZBitMask`**. The corrected pixel format uses
`DDPF_ZBUFFER`, 32-bit depth and mask `0xffffffff`,
as in Wine's 32-bit depth-format tests. It first tries
`DDSCAPS_ZBUFFER | DDSCAPS_VIDEOMEMORY` and, if refused,
retries another **32-bit** Z surface with driver-selected
memory placement. Every real HRESULT is logged, with the
selected memory policy; the probe **never silently falls
back to 16-bit depth**.

On successful creation, the probe reads the actual Z32
`GetSurfaceDesc` (bits and mask), tests the game's real
`DDBLT_DEPTHFILL` operation, then continues with the
original Z attachment, RGB565 fill, windowed Blt and
Direct3D3 HAL CreateDevice. If the backend rejects a
step the log preserves its exact HRESULT.

Only `windowed_ddraw_probe.exe` is changed. Install the
standalone probe artifact outside the game folder and
return its `windowed_ddraw_probe.log`. The active game
`ddraw.dll`, its INI and FPS behavior remain unchanged;
no in-game borderless feature is claimed.

## v0.1.27 — fix standalone probe DirectDraw initialization

The owner-provided v0.1.26 `windowed_ddraw_probe.log`
shows `DirectDrawCreateEx(IDirectDraw4): HRESULT=0x80070057`.
That error is **not evidence that windowed DirectDraw or
Direct3D3 cannot work**. It comes from an invalid interface
identifier: Microsoft documents that `DirectDrawCreateEx`
accepts **only `IID_IDirectDraw7`**.

The probe now uses `DirectDrawCreateEx(IDirectDraw7)`,
then `IDirectDraw7::QueryInterface(IDirectDraw4)`. Both
stages are recorded in the same adjacent
`windowed_ddraw_probe.log`, with their actual HRESULTs.
The initial IDirectDraw7 reference is safely released
after IDirectDraw4 is obtained. A new unit test asserts
the exact v7 and v4 interface identifiers.

Only the standalone test program has changed. The game
`ddraw.dll`, its INI and rendering path are untouched.
Download the newer **windowed probe artifact** into the
separate test directory, run it under Wine, and attach the
new `windowed_ddraw_probe.log`.

## v0.1.26 — standalone probe writes a clean diagnostic log

The standalone Windows x86 `windowed_ddraw_probe.exe` now
creates **`windowed_ddraw_probe.log` next to the executable**
at the start of each run. The file is overwritten on the next
run so that old results cannot be confused with new ones.

Every probe step, DirectDraw/Direct3D **HRESULT (including
failures)**, desktop-size check and final PASS/FAIL status is
written and flushed immediately. Wine's noisy messages
to the terminal are **not** copied to this log, while the
probe's own messages remain visible in the terminal.

Extract the **separate standalone probe artifact** outside
the game folder, run the EXE normally under Wine or
GE-Proton and send `windowed_ddraw_probe.log`.
There is **no need to redirect the terminal output**.

If log creation fails (for example, the EXE directory is
read-only), the probe prints a specific startup error and
exits *before* running any DirectDraw test. The game
`ddraw.dll` and its `CrimsonSkiesFix.log` are unaffected.

## v0.1.25 — standalone real windowed DirectDraw3D resource probe

A new separate **Windows x86 console executable**
`windowed_ddraw_probe.exe` tests whether the host's real
DirectDraw backend can build the resource chain needed for
future Crimson Skies borderless support. It is **NOT** injected
into the game and is packaged as a **separate GitHub artifact**.

It creates its **own** Win32 test window, requests
`DDSCL_NORMAL` (never exclusive), creates a real windowed
primary plus attached clipper, and attempts to create a
640x480 RGB565 video-memory offscreen surface with
`DDSCAPS_3DDEVICE`, a 32-bit Z-buffer, their attachment,
a color-fill Blt, a clipper-constrained Blt to the visible
window, and an `IDirect3D3::CreateDevice` targeting that
offscreen surface. It checks the desktop resolution remains
unchanged. All actual COM HRESULTs are printed to the console.

**How to test** (under Wine/GE-Proton or native Windows):
extract the `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL`
artifact into a **separate directory**, *not* the Crimson Skies
game directory, to avoid loading the game's `ddraw.dll` proxy.
Run `windowed_ddraw_probe.exe` directly from that directory
and send its full console output. A failed HRESULT is useful
compatibility evidence, not necessarily a bug in the game DLL.

The existing three-file game DLL release artifact remains
unchanged; `Borderless=1` is **still a preflight only**.
This proof of actual normal-mode DirectDraw support does
not yet virtualize COM surfaces or implement the final
game-window borderless presentation.

## v0.1.24 — inspect Z-buffer Blt flags (opt-in)

The user's v0.1.23 Wine log shows five 120-Flip groups in 800x600
and seven in 640x480. **Lock=Unlock for every observed role and
group**, and every audited HRESULT is successful. In stable 640x480
there are 120 backbuffer Blts and 240 Z-buffer Blts for 120
successful primary Flips, while stable menu groups often have
120 Blts on each buffer.

The existing opt-in `[Debug] SurfaceUsageTrace=1` now also
produces a bounded `SURFACE-BLT-SIGNATURE` summary for
each group: exact hexadecimal Blt flags, whether the source
surface pointer was NULL, success/failure counts and the
receiving surface's role. Eight distinct signatures per
role are kept per group and extras are counted as overflow.

This may distinguish `DDBLT_DEPTHFILL=0x02000000` from
`DDBLT_COLORFILL=0x00000400` or copied surfaces, but
**does not access DDBLTFX, pixels, RECTs or surface memory**.
No additional COM hooks are installed, and no game call is
changed. The borderless presenter is still not implemented.

For the next Wine test, leave the current graphics settings
and enable `Log=1` and `SurfaceUsageTrace=1` under
`[Debug]`, then provide the latest `CrimsonSkiesFix.log`.

## v0.1.23 — per-role Lock/Unlock, Blt and attachment diagnostics

After the owner supplied v0.1.22 logs, the opt-in
`[Debug] SurfaceUsageTrace=1` audit additionally records
`Unlock`, `Blt`, `BltFast` and `GetAttachedSurface`
successes/failures **by receiver surface role**. These
methods already have forwarding hooks; no new vtable entries
are patched and original arguments, HRESULTs and gameplay
behavior are unchanged. Two tests cover the new counters.

The previous v0.1.22 test confirmed explicit
`backbuffer.AddAttachedSurface` during both 800x600/640x480
initializations, frequent texture Lock and Restore operations,
and three `GetDDInterface` queries on other offscreen
surfaces during the measured 800x600 phase. No captured hook
returned an error; borderless remains inactive.

For the v0.1.23 test, keep `Log=1` and
`SurfaceUsageTrace=1` under `[Debug]`, preserve your
existing `ModernFPSPacing`, visit menus and play 3D,
then provide `CrimsonSkiesFix.log`. New lines can compare
per-role Lock/Unlock counts with Blt destinations.

## v0.1.22 — Stage B3c: opt-in passive DirectDraw surface audit

The new `[Debug] SurfaceUsageTrace=1` option (with `Log=1`)
activates **nine additional original-call forwarding hooks** on the
actual DirectDrawSurface2/3/4/7 objects: `AddAttachedSurface`,
`DeleteAttachedSurface`, `GetDC`, `IsLost`, `Lock`, `ReleaseDC`,
`Restore`, `SetClipper`, `GetDDInterface`.

Each hook forwards the exact original arguments once and records
only **successful/failed HRESULT counts per surface role** (primary,
backbuffer, Z-buffer, texture, other). After each group of 120
successful primary Flips, the log emits one bounded
`SURFACE-USAGE` summary; samples reset after successful display-mode
changes. Counters never include raw COM addresses or surface pixels.
An optional trace does **not** constitute a functioning borderless
adapter. The current exclusive rendering path remains unchanged.

This audit is **off by default**, including on existing user INI
files. For a Windows/Wine gameplay experiment, enable it in the
existing `CrimsonSkiesFix/CrimsonSkiesFix.ini`:

```ini
[Debug]
Log=1
SurfaceUsageTrace=1
```

Keep `ModernFPSPacing` at your established working value. Run a
menu and a 3D scene, then attach `CrimsonSkiesFix.log`. Disable the
audit after collecting evidence; its extra vtable hooks are a
diagnostic experiment only, not an FPS optimization.

## v0.1.21 — complete Surface4 COM ABI layout (inactive)

The new `src/com_surface4_abi.rs` defines the **45-entry
IDirectDrawSurface4 vtable** with explicit, typed Win32 x86 calling
signatures, verified against Wine's DirectDraw header. Four Windows
x86 unit tests verify every slot offset, the 180-byte table size,
critical observed methods (Flip, Blt, GetAttachedSurface, Lock,
Unlock, GetSurfaceDesc) and typed mock dispatch.

This is **ABI groundwork, not a functional Surface4 implementation**.
Only a test-only mock uses the table. The mock does not advertise
a Surface4 IID and no new object is exposed to the game. Surface1/2/3/7
must have separate, correct ABI definitions. The current graphics
hooks and windowing remain unchanged.

## v0.1.20 — Stage B3a: native IUnknown ABI scaffold (inactive)

`src/com_unknown.rs` now defines a **real, heap-allocated Win32
COM IUnknown object** with a three-method C-layout vtable using
the x86 `extern "system"` calling convention. It implements:

- `QueryInterface(IID_IUnknown)` returning the **same physical
  pointer**, with one reference acquired per successful query.
- `AddRef` and `Release` backed by an atomic 32-bit
  counter. The final Release frees the heap object exactly once.
- Proper null output / invalid interface handling; failing queries
  clear the output pointer and never acquire a reference.
- A guarded counter that does not wrap at `u32::MAX`, and
  distinct identities for distinct primary/backbuffer/depth objects.

Seven Windows x86 unit tests exercise the **actual function pointers**
and object destruction; the prior logical contract remains separate.

**Crucial:** this scaffold advertises **only IID_IUnknown**. It
must **not** be used as an IDirectDrawSurface1/2/3/4/7, since those
interfaces require complete, ABI-correct method tables and real
Direct3D-compatible resources. There is no runtime object creation
or installation, no windowed adapter, no new configuration switch,
and no change to current rendering, FPS pacing or input.

## v0.1.19 — Stage B2: isolated COM surface/lifecycle model

The new **inactive** `src/borderless_contract.rs` module models a
future DirectDraw3D-compatible borderless adaptor without modifying
the game's exclusive DirectDraw or Direct3D runtime calls.

- Declares an explicit opt-in backend readiness contract: game-owned
  HWND, normal DirectDraw mode, real RGB565 DirectDrawSurface4 target
  with mandatory `DDSCAPS_3DDEVICE`, real 32-bit Z-buffer, primary/clipper,
  functioning Blt and complete COM interface handling.
- Refuses incomplete backend evidence **before** any replacement
  surface is exposed. Once exposed, the model forbids silently
  falling back to exclusive mode.
- Models stable logical COM object identity across DirectDrawSurface
  versions, reference increments on QueryInterface/GetAttachedSurface,
  AddRef/Release and attachment ownership on primary/backbuffer/depth.
- Provides the observed 640x480/800x600 descriptor/caps contract and
  rejects unsupported attachment requests, dead or stale-epoch objects.

Eleven new Rust unit tests validate these invariants, including
combined COM reference-count overflow protection and Direct3D3 target caps; the Windows x86
CI still builds the same unmodified graphic hooks and deploys the
same three-file ZIP layout. **There is no actual virtual COM object
or rendering adapter yet.** Real IUnknown pointer identity, ABI
dispatch, GPU surface creation and windowed presentation are still
Stage B3 engineering work, not achievements of this release.

## v0.1.18 — individual inter-Flip Direct3D3 scene distribution

The owner's v0.1.17 gameplay log establishes a stable mean of
**11 successful BeginScene and EndScene calls per primary Flip** in
five complete 120-Flip groups at 640x480, compared with exactly 1:1
in five groups at 800x600. This is not the on-screen FPS rate.

The optional `SCENE-INTERFLIP` log now distinguishes 11 calls on
*each* consecutive successful primary Flip interval from a burst
that averages 11 over a group. For each of the existing 12 bounded
groups, it counts **119 intervals** between 120 successful primary
Flips, recording exact BeginScene bins 0 through 12, a 13+ bin,
min/max of BeginScene and EndScene, mismatched counts and counter
regressions. Failed and secondary Flips are not fed into the
sample; mode switches reset all state. Group totals and interval
histograms use different initial baselines and need not sum to
the same values.

There are no alterations to graphics, the FPS limiter, Windows
focus, renderer state, INI defaults, audio or game files.

## v0.1.17 — same-group successful Flip timing (diagnostic only)

The v0.1.16 scene/Flip correlation now also times **the same set
of successful primary Flip calls**, using a monotonic clock and a
snapshot taken after each successful original COM operation. Each
120-call group contains **119 intervals from the first successful
Flip to the last**. Its bounded (12 lines per process) opt-in
`SCENE-CORRELATION` output gains:

- `first_to_last_flip_ms`: elapsed time inside the group, not since
  the game started or a display mode was configured.
- `flip_intervals=119`: the correct interval count for 120 events.
- `successful_primary_flip_calls_per_s`: API completion rate from those
  same 119 intervals, or `Unavailable` when the duration
  rounds to zero milliseconds.

Scene/Flip ratios still use **120 successful Flip calls**, and do
not establish unique presented frames, simulation rate or a precise
one-to-one scene/Flip relationship. Failed/secondary Flip attempts
are excluded. Any successful SetDisplayMode resets both the partial
sample and its clock, even for a repeated resolution. No rendering
mode, COM arguments, FPS limiter, game input, audio, INI defaults
or log placement were changed.

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
