# 3D world + HUD: XYZRHW projected-vertex aspect A/B

**Status:** experimental, opt-in, not a released fix. Issue #41, related #37. Native Windows and Wine/Proton need in-game confirmation.

## New information from 15 November 2026

The user's `CrimsonSkiesFix(20261115-074614).log` identifies GE-Proton11-12 on Wine 11.0, host 1920×1080 / 120 Hz, RX 6600M and the verified original 1.02 widescreen patch at 1920×1080. The DD4/Direct3D HAL gameplay graph succeeds.

Both UI quads and indexed aircraft/world mesh draw calls are submitted to `IDirect3DDevice3` with **FVF=0x000001C4**:

```text
D3D3-VERTEX: DrawPrimitive primitive=6 fvf=0x1c4 position=XYZRHW ...
D3D3-VERTEX: DrawIndexedPrimitive primitive=4 fvf=0x1c4 position=XYZRHW ...
D3D3-PROJECTION: m11=1.0 m22=1.0 matrix_aspect=1.0 ...
```

The `XYZRHW` coordinates have already been projected into **screen pixels by the game**, before Direct3D receives them. Consequently the identity Device3 PROJECTION matrix is **not evidence of correct world projection**. This supports the owner's observation that **the aircraft as well as the HUD is horizontally elongated**. The previous #37 HUD-only emphasis was incomplete.

Earlier near-matched real 1920×1200 16:10 versus centered 1600×1200 true 4:3 screenshots (`image(20261114-193916).png` and `image(20261114-194052).png`) show gauge rings going from ≈147×125 to ≈147×150 px. The red aircraft also grows significantly in vertical extent, from ~306 to ~359 px in a roughly matched red-pixel crop; note that precise camera pose is not identical, so this is corroboration, not a calibrated 3D-model measurement.

The original owner-provided `CRIMSON(7).EXE` 1920×1200 horizontal FOV adjustment is **already faithfully reproduced** by the 38-byte-region in-memory patch in `native_widescreen_full.rs`. Its `atan(tan(angle/2)*(W/H)/(4/3))` adjustment is mathematically plausible, but a wider field of view is not sufficient if the original software projection still applies 4:3-specific scaling on its X/Y pixel coordinates.

## Falsifiable projected-vertex experiment

Commits on draft PR #36:

- `f1753dfa`: allow passive `HUD-RENDER-PATH` diagnostics under Wine and Proton as well as native Windows (earlier build wrongly gated to native Windows).
- `ccaea3a3`: new opt-in `[Debug] ExperimentalProjectedY43Probe=1`, **default OFF**.
- `258fb786`: fail-closed accessor of the **actual current verified virtual 3D epoch**, not simply the host monitor.
- `2f6c0404`: bounded, original-buffer-preserving copy of pretransformed x86 FVF=0x1C4 vertices.
- `a7f0df1e`: integrate it into original Direct3D3 DrawPrimitive / DrawIndexedPrimitive forwarding.

With the user opting in, only during a verified live 1920×1080 or 1920×1200 (or other native widescreen) gameplay epoch, only for exact FVF=0x1c4 and primitive types 4 (indexed triangle list) / 6 (fan quads), forward a **separately allocated temporary copy** of the original game data to the original Direct3D3 COM draw method. It edits **only screen-space Y** in that copy:

```text
factor = 3*W/(4*H)
y' = H/2 + (y-H/2) * factor
```

- 1920×1200 factor = 1.2
- 1920×1080 factor = 4/3
- Real 4:3, 800×600 menu/movie, unsupported/huge/unreadable/other-FVF buffers all pass through untouched.
- Strict pointer memory bounds `VirtualQuery`, count ≤2,048, buffer ≤64 KiB, finiteness/position sanity.
- Source game vertex memory, X/Z/RHW, color/UV, indices, FOV game globals, on-disk EXE, DirectDraw4/COM identity, and output presentation configuration are not changed.
- Mutually exclusive with `ExperimentalHudFov43Probe`, `ExperimentalHudEntityY43Probe`, `ExperimentalHudCanvas43Probe`.
- This **does change the effective final vertical geometry and vertical FOV**, may clip objects and may be slower (debug shadow copies). Any unwanted differences mean disable the option. A successful compilation is not visual approval.

Suggested test configuration in the **active game-root** `CrimsonSkiesFix.ini`:

```ini
[Graphics]
AspectRatio=auto

[Debug]
Log=1
ExperimentalHudFov43Probe=0
ExperimentalHudEntityY43Probe=0
ExperimentalHudCanvas43Probe=0
ExperimentalProjectedY43Probe=1
```

Expected log prefix `PROJECTED-Y43: enabled=1 draw=...` during actual flight. Capture **same flight attitude** for aircraft and gauges, compare baseline, inspect 3D world/camera, bottom screen clipping, performance and pause/cutscenes. Return to 0 immediately if any regression.

## Build verification

CI on SHA `a7f0df1efb95132f312af5af62a255d4d8f6f428` passed all three:
- WinMM installer [43692361534](https://github.com/legluondunet/CrimsonSkiesFix/actions/runs/43692361534)
- WinMM runtime [43692361497](https://github.com/legluondunet/CrimsonSkiesFix/actions/runs/43692361497)
- Legacy v1.0 [43692361512](https://github.com/legluondunet/CrimsonSkiesFix/actions/runs/43692361512).

ZIP artifact **14153260523** `CrimsonSkiesFix-WinMM-1.1-TEST-ONLY-NOT-RELEASE` verified CRC; PE32 EXE **1,480,704 bytes**, SHA256 **`5b7e4584e1f23762c7faa1ffd8458d6e70c4e158d836c019d40b25160ca669c1`**, matches the contained `SHA256=...` manifest.

**No conclusion about a finished fix is possible until real gameplay visual A/B on both graphics backends.**
