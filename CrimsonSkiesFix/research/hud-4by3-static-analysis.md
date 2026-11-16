# HUD 4:3 investigation — original decrypted Crimson Skies 1.02

Status: **static reverse engineering only; no new runtime graphics patch**.

## Provenance and guard rails

Owner-supplied `CRIMSON(20261114-203348).EXE` is PE32/i386 with six sections,
image base `0x00400000`, image size `0x00652000` and SHA-256:

```text
43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75
```

Original executable is **not** in this repository, and must not be added.
The existing `native_widescreen_full.rs` has a 38-region, preimage-verified
in-memory delta against this exact original. Reverse engineering and any
future patch must preserve the validated WinMM DD4 + movie Surface1 renderer,
3D field of view, movie/menu compositing, Windows/Wine shared implementation,
and zero on-disk EXE modification.

## Verified 4:3 vs 16:10 A/B result

On the same 1920×1200 physical screen:

| HUD instrument | 16:10 pixel bounds | 4:3 pixel bounds |
| --- | --- | --- |
| Left top | 147×124 | 147×149 |
| Left middle | 147×125 | 147×150 |
| Right top | 146×124 | 146×149 |
| Right middle | 148×125 | 148×150 |

Instrument **width remains constant**; height and displacement from screen
center scale by approximately 0.8333 in widescreen. At 1920×1200 this equals
`(4/3)/(1920/1200)`. The desired correction is local to HUD and anchored
around screen center, never a global final-frame transformation.

## The `0x0071B3F8` global is a camera projection aspect — not proved to be HUD

The original binary contains only two *direct absolute references* to this
global in executable sections (the third comes from our 38-region patch):

```asm
; Original 0x004603F0 — initializer
fld  dword ptr [0x006032DC]  ; 1.0
fdiv dword ptr [0x00607BD0]  ; 4/3
fstp dword ptr [0x0071B3F8]  ; 0.75

; Original 0x0042B570 — FOV conversion, reader at 0x0042B58F
fld   dword ptr [esp+0x28]  ; horizontal field of view
fmul  dword ptr [0x006032E0] ; 0.5
fptan
fstp  st(0)
fstp  qword ptr [esp+0x14] ; tan(hFOV/2)
fld   dword ptr [0x0071B3F8] ; aspect ratio = height/width
fmul  qword ptr [esp+0x14]
fld1
fpatan
fadd  st(0),st(0)         ; vFOV = 2*atan(aspect*tan(hFOV/2))
```

The current patch adds an initializer at **RVA `0x2024BD`**, called from
RVA `0x1AA08B`, that replaces the global with `height/width`
(`0.625` at 1920×1200). It also widens the horizontal FOV so the vertical
world FOV remains consistent. **Do not freeze this global to 0.75**: it is
directly consumed by the 3D camera projection. No direct executable xref
has established a HUD-specific consumer; indirect uses remain possible.

The camera FOV reader at VA `0x0042B570` is called from
`0x0042B6A5`, `0x0049DB8C`, `0x004A058E` and `0x004A0960`.
These are camera paths, not evidence of HUD draw batching.

## We identified HUD-specific code and its ZRD layout

The original executable includes explicit named resources:

```text
hud_v2.zrd        VA 0x00624F08 — referenced from 0x00454EE5
SW_GAUGES         VA 0x00624F14 — referenced from 0x00454EFD
POSITION_1ST      VA 0x00624F28 — referenced from 0x00454F14
POSITION_3RD      VA 0x00624F38 — referenced from 0x00454F7B
speedometer       VA 0x00624D74 — referenced from 0x0045394D
altimeter         VA 0x00624DAC — referenced from 0x00453BB1
gungauge          VA 0x00624E18 — referenced from 0x00454910
missilegauge      VA 0x00624E98 — referenced from 0x00454C31
hud_hud.cpp       VA 0x00624FE4 — referenced from 0x004553D8
```

**HUD layout constructor** at VA `0x00454E70` loads `hud_v2.zrd`,
`SW_GAUGES`, `POSITION_1ST` and `POSITION_3RD`, and writes four
coordinate multipliers to object offsets `+0x20BC, +0x20C0, +0x20C4,
+0x20C8`.

**HUD layout update** at VA `0x00454FD0` uses native display dimension
getters `0x00441090` and `0x004410A0`, multiplies the values by the
respective configured `POSITION_1ST` or `POSITION_3RD` multipliers,
and calls HUD object vtable slot `+0x0C`.

The exact method pointer at `0x00607718+0x0C` resolves to **VA
`0x004559E0`**, which only stores the two resulting values into the HUD
object at `+0x14` and `+0x18`:

```asm
004559E0: mov eax,[esp+4]
004559E4: mov edx,[esp+8]
004559E8: mov [ecx+0x14],eax
004559EB: mov [ecx+0x18],edx
004559EE: ret 8
```

**Important**: The config keys are named `POSITION_1ST/3RD`. Despite
being multiplied by display dimensions, these values should be treated
as **HUD placement coordinates, not proven gauge size/scaling**. Arbitrarily
multiplying them by 1.2 could move instruments without fixing their
elliptical outlines. The exact HUD geometry/projection drawing step
remains to be identified downstream of this object, or in the ZRender
2D pipeline. This refines the research plan and rejects a premature
"change HUD container height" patch.

## FOV-related patch sites — do not revert blindly

The 38-region delta also changes two operands near VA `0x00455C01`
and `0x00455C0E`, from original constant 60° and 30° values to
dynamic widened-FOV variables `0x00A1EFF0` and `0x00A1EFF4`.
These feed a `tan()/tan()` normalization in a camera-related helper
at `0x00455BE0`. They are not proven to draw the gauge rings and
must not be reverted without demonstrating their layer/caller context.

## Next autonomous work

1. Continue **offline**, following the HUD module/ZRD references above
   to the 2D instrument draw calls and the ZRender 2D projection.
2. Correlate caller/callee and object ownership to prove the code affects
   the gauge **vertical radius** (not merely its screen-center placement).
3. Make an independently controlled, byte-gated and reversible experimental
   patch for **only the HUD draw state**, not the shared FOV projection.
4. Check Windows 16:10, 16:9 and authentic 4:3; no claim of success until
   a user-supplied gameplay screenshot validates round gauges and unchanged
   3D world/cinematics.

Do not merge/release PR #36 on the basis of static inspection alone.


## Native WinMM debug camera normalization A/B

The original 1.02 EXE confirms two patched x87 operand references in the
camera-entity normalization helper at VA 0x00455BE0. The game's scene
loop calls this helper from VA 0x0049FCC3.

- RVA 0x55C01 / VA 0x455C01: original 3-byte memory address
  9C 40 60 (0x0060409C, 60-degree reference) was replaced by
  F0 EF A1 (0x00A1EFF0, dynamic wide-FOV reference).
- RVA 0x55C0E / VA 0x455C0E: original 90 40 60 (0x00604090,
  original 30-degree half-angle reference) was replaced by
  F4 EF A1 (0x00A1EFF4, dynamic wide-FOV half-angle reference).

This code is part of a scene-camera/entity pass. **It is not yet proven
that the call exclusively affects the HUD**; changes to scene scale/FOV
must be checked and not accepted merely because gauges become circular.

Experimental WinMM build has a disabled-by-default, exact two-site
reversible startup A/B test, enabled ONLY by the debug switch below:

    [Graphics]
    AspectRatio=auto

    [Debug]
    Log=1
    ExperimentalHudFov43Probe=1

The normal baseline is ExperimentalHudFov43Probe=0. On native Windows
widescreen only, AFTER the complete byte-verified EXE patch succeeds,
the prototype changes only those 2 operand addresses back to the
verified original 4:3 references. It never changes the game EXE on disk,
and does not patch DirectDraw, WinMM, movie compositing, or render
surfaces. It refuses unexpected byte preimages and attempts rollback
on any write/protection failure.

Look for the log line beginning HUD-FOV-A-B: enabled=1. Compare
same-mission gauge circularity, HUD edge anchors, plane and world FOV,
menus and cinematics. A failure or unrelated 3D distortion means
restore ExperimentalHudFov43Probe=0 and return to the proven baseline.

**Experimental only:** without native visual proof this is a hypothesis
test, not a completed HUD-only correction.


## 2026-11-14 camera-FOV A/B result and entity-Y prototype

Owner's Windows 11 A/B: \`image(20261114-213421).png\` (test off,
\`CrimsonSkiesFix(20261114-213454).log\`) versus
\`image(20261114-214205).png\` (test on,
\`CrimsonSkiesFix(20261114-214239).log\`). Both use authenticated
\`1.1.0-rc.1-5a4fd204\`, 1920x1200, WinMM. The latter log confirms
\`experimental_hud_fov43_probe: true\` and
\`HUD-FOV-A-B: enabled=1 ... sites=2\`.

Threshold-based connected-component outer-bezel bounds on original
1920x1200 PNGs:

| Instrument | FOV A/B off | FOV A/B on |
|---|---|---|
| left upper | x=175,y=677,147x125 | x=17,y=692,177x151 |
| left middle | x=175,y=806,147x125 | x=17,y=847,179x150 |
| right upper | x=1600,y=677,148x126 | x=1727,y=692,178x151 |
| right middle | x=1599,y=806,149x125 | x=1727,y=847,179x151 |

The FOV A/B **increases both axes by ~1.2** and pushes gauges outwards;
their horizontal/vertical ratio stays ~1.18. **Reject this A/B as a HUD
aspect fix**, keep \`ExperimentalHudFov43Probe=0\` by default. It may also
change other scene geometry. Runtime's DD4/Surface1 and 3D transitions
continue to work in both screenshots/logs.

Further original-EXE static disassembly established that the same
game scene-camera entity loop (VA \`0x0049FCC3\`) calls \`0x00455BE0\`,
whose two CDECL calls separately set the X/Y/Z scale and position of
the entity:

- \`0x00455C40\` (\`e8 0b c1 07 00\`): calls \`0x004D1D50\` with
  object pointer and scale \`(x,y,z)\` from offsets \`+0x20,+0x24,+0x28\`.
- \`0x00455C54\` (\`e8 d7 bd 07 00\`): calls \`0x004D1A30\` with
  object pointer and position \`(x,y,z)\` from offsets \`+0x2c,+0x30,+0x34\`.

These two exact call sites offer a narrow falsifiable hypothesis: the
2D overlay entity can recover its genuine 4:3 vertical size and vertical
anchor without altering X geometry or the world-camera FOV. The required
Y-only factor at the physical widescreen raster is
\`factor = 3*width/(4*height)\` (1.2 at 1920x1200, 4/3 at
1920x1080). The original A/B with actual 1600x1200 4:3 proved the
required vertical pixel difference, with X positions unchanged.

**Opt-in debug-only experimental WinMM site:** new switch

\`\`\`ini
[Graphics]
AspectRatio=auto

[Debug]
Log=1
ExperimentalHudFov43Probe=0
ExperimentalHudEntityY43Probe=1
\`\`\`

This prototype patches **only the two exact 5-byte call operands**
after the original hash/preimage-gated 38-region widescreen patch
succeeds. The new CDECL forwarding wrappers apply Y*factor to **the
call arguments only**, preserving all X/Z arguments, the underlying
object fields, the 3D FOV variable and on-disk EXE. Exact original
call-site byte checks are enforced; partial write failures attempt
rollback. Native Windows and widescreen only, **OFF by default**.
The old FOV A/B switch must be off; simultaneous activation refused.

Log markers:

\`\`\`text
HUD-ENTITY-Y43: enabled=1 sites=2 vertical_factor=1.200000 ...
HUD-ENTITY-Y43: call=SetScale original_y=... new_y=...
HUD-ENTITY-Y43: call=SetPosition original_y=... new_y=...
\`\`\`

**Risk:** the scene-camera entity list at 0x49FCC3 may contain 3D
world objects in addition to overlays. This is **still a controlled
hypothesis test, not a confirmed HUD-only fix**. Native test must
visually verify circular gauge rings, unchanged horizontal anchors,
correct vertical edge placement AND no camera, aircraft, terrain,
cinematic or menu changes. Restore option=0 immediately if wrong.
Do not merge or release the prototype based only on CI.


## Audit of the owner-supplied prepatched 1920x1200 FOV/HUD executable

**2026-11-14**, after the owner reiterated that an already patched 1920x1200
FOV executable was available, we recovered the actual old uploads from their
library and compared them *byte for byte*, rather than relying on the names
or historical summaries alone.

| Binary supplied by owner | SHA-256 | Size |
| --- | --- | --- |
| \`CRIMSON-UNWRAPPED.EXE\`, unprotected 1.02 | \`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75\` | 2,580,480 |
| \`CRIMSON(7).EXE\`, 1920x1200 FOV-fixed reference | \`612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d\` | 2,580,480 |
| \`CRIMSON_1920x1080_FOVFIX.EXE\` | \`5578a15ce072d09655be43419829a7530ed6c71fa5a8770b39d67\` | 2,580,480 |
| \`CRIMSON_2560x1080_FOVFIX.EXE\` | \`694863c23282d295382ba12f1a2e2c64e5c4b8e7eb766f7b7a6ebc1d936acb92\` | 2,580,480 |

Bytewise comparisons prove:

1. \`CRIMSON(7).EXE\` differs from the owner-supplied decrypted 1.02
   executable at **exactly 314 individual bytes in 38 disjoint groups**.
   These are the **same 38 regions** encoded in
   \`native_widescreen_full.rs\`, which already says it was derived from
   this owner-provided FOV/HUD reference. Thus the original reference
   was **not ignored or missing from the in-memory widescreen patch**.
2. Comparing the 1920x1080 patched executable against \`CRIMSON(7).EXE\`
   yields **one changed byte at file offset \`0x20254F\`**, the low
   bytes of the injected height. Comparing 2560x1080 against
   \`CRIMSON(7).EXE\` yields **three changed bytes at \`0x20254B\`,
   \`0x20254C\` and \`0x20254F\`**, the injected width and height. The
   code of the FOV patch is otherwise the same across these supplied
   resolutions.
3. Both HUD canvas calls at VA \`0x00455CA5\` and \`0x00455CF2\` are
   **byte-for-byte unchanged** between original \`CRIMSON-UNWRAPPED.EXE\`
   and 1920x1200 patched \`CRIMSON(7).EXE\`:
   \`640x480\` and \`320x240\` remain their original values.
   Therefore, \`ExperimentalHudCanvas43Probe\`, which experiments
   with those two heights, is **a new unproven change**—it does *not*
   reproduce an extra fix found in the owner-supplied prepatched EXE.
4. References to the 60° / 30° FOV constants are redirected by the
   supplied 1920x1200 patched executable (three-byte operand changes
   at RVA \`0x55C01\` and \`0x55C0E\`). Our old camera-FOV A/B deliberately
   reversed those references and failed visual validation; this does
   not imply the original reference missed an additional HUD patch.

**Interpretation and important limitation:** the reference archive's label
"Resolution Hack and FOV Fix - No HUD Stretch" is useful evidence of the
**intended** rendering behavior, but it is not a substitute for a native
screenshot of that original executable running with an original/compatible
rendering backend. We currently know our running WinMM build uses the exact
38-region reference widescreen delta but shows elliptical HUD gauges
(~147x125 vs native true-4:3 ~147x150). That discrepancy could involve
the original reference patch's own limitations or the **WinMM virtual
rendering/presentation integration**. Do not assert which until a
matched native-render comparison is available. Do not add a fourth
speculative EXE patch as though the reference contains some unseen bytes.

**Additional owner-binary audit:** `CRIMSON(1).EXE` and `CRIMSON(6).EXE`
are **byte-for-byte identical** to `CRIMSON(7).EXE` (1920x1200
reference). `CRIMSON(5).EXE`, `crimson(8).exe` and the separately
named `CRIMSON_1920x1080_FOVFIX.EXE` are the **same** 1920x1080
reference SHA-256; `Crimson(9).exe` and `Crimson(10).exe` are
identical to the decrypted original. None contains a **second
alternate HUD scale correction**. There is also an early `crimson.exe`
that differs from the decrypted original by only ten bytes in two
locations, unrelated to this 38-region 1920x1200 patch.

Public game-compatibility documentation distinguishes a
**stretched/enlarged HUD variant** from a regular variant where HUD
elements stay **small**:
https://www.myabandonware.com/game/crimson-skies-be7 .
Consequently the label `No HUD Stretch` does not, by itself, prove
that the round gauges retain geometric 4:3 proportions. A matched
screenshot of the original reference renderer is still needed to
establish that narrower visual claim.

**Next investigation**: compare the 1920x1200 reference executable's HUD
and FOV under a separate pristine rendering setup to the exact same
mission under the WinMM adapter, or isolate the HUD coordinate mapping
from static drawing code. Leave the already functioning DD4/Surface1
runtime unchanged. Retain the canvas probe OFF until that cause is
established; do not merge it as a production feature without A/B
confirmation. These proprietary executables were inspected locally
only and are **not** added to this repository.


## 15 Nov 2026, 09:30 Proton Y43 trace: reticle regression proven, edge diagnosis

Owner-supplied complete log `CrimsonSkiesFix(20261115-093031).log` is
from WinMM **1.1.0-rc.1-fad24b14**, GE-Proton11-12 (Wine 11.0),
1920x1080, `ExperimentalProjectedY43Probe=true`. An extended mission and
mode transitions produce no `ERROR` or `CRITICAL` line, but the user still
observes an oval aiming reticle, incomplete objective bubbles, and black
triangles under extreme banking. The verified world's 3D airplane, round
instruments and compass should remain unchanged.

**Positive reticle geometry evidence, not just visual inference:**
- `PROJECTED-Y43-CENTER` examples 1..10 at log lines 777–787 show a
  repeatedly drawn `DrawPrimitive(type 6, FVF 0x1C4, count 4)` quad
  near screen center with exact **32.00x32.00** original bbox and
  **32.00x42.67** after Y43 (source `original_ratio=1.0000`, destination
  `new_ratio=0.7500`). This exactly matches the user's oval aim symbol.
- This quad is a strong *reticle candidate*, but bounding boxes do not
  uniquely prove a sprite's identity; avoid any generic exemption of
  all center-screen or all 2D draws.
- **Opt-in A/B** is now implemented with `[Debug]
  ExperimentalY43ReticleGuard=1`, OFF by default, and only effective
  when `ExperimentalProjectedY43Probe=1`. It exempts a primitive from Y43
  **only** if non-indexed, primitive type 6, exact FVF 0x1C4, count=4,
  four unique axis-aligned corners, bbox widths **31.5–32.5** on both axes,
  bbox center within 32px horizontally and 40px vertically of raster
  center, in a verified native widescreen gameplay epoch. An exempt
  quad uses original caller vertex data unchanged. Neither the 6-category
  global Y43 factor, nor world/HUD vertex selection, nor on-disk EXE,
  nor DD4/Surface4 is modified. `PROJECTED-Y43-RETICLE-GUARD` logs
  bounded samples and `reticle_kept_round` cumulative count. User
  verification must establish that the *actual reticle* becomes round
  without breaking any other HUD element.

**Clipping/cutout evidence and limitations:**
- `PROJECTED-Y43-EDGE` sample 1 logs original Y bbox `[0,1237.60]`
  moving to `[-180,1470.13]`; sample 4 `[704.37,1080]` moving to
  `[759.16,1260]`, both in indexed TEX1 triangle lists. At the last
  periodic checkpoint, **258,094 draw calls** crossed the upper viewport
  boundary, and **221,618** crossed the lower, from a formerly in-range
  vertex. These counters count calls, **not unique vertices, triangles,
  artifacts or visible pixels**. A large number is not evidence that all
  are bad — clipping outside viewport is routine 3D behavior.
- `PROJECTED-Y43-SKIP` reveals a separate issue: source projected
  coordinates **+1,215,425.9** and **-1,498,662.1** on valid indexed
  triangle lists; the y-adjuster rejected these by its absolute bound
  and forwarded the **entire draw unmodified**. Latest cumulative
  `skip_invalid=161`. Such oversized values may be natural intermediate
  outputs for geometry behind/near the camera; do not clamp or delete them.
- All three other skip counters `skip_format=0`, `skip_large=0`,
  `skip_unreadable=0`, and `reserve_failures=0` at last checkpoint.
  No error code from Direct3D is seen in the trace.
- These facts make late post-projection Y43 vs the original game's
  precomputed clipping a serious hypothesis for incomplete objective
  bubbles and black void triangles. **The log alone does not identify
  an exact bad triangle.** A correct fix should align the **actual
  projection + clipping stage**; avoid simply discarding or clamping
  out-of-bounds geometry or broad exclusions of indexed triangles.

**Worktree provenance:** commits `2699955c` (INI toggle) and
`2564ac76` (precise A/B guard). Both are experimental on draft PR #36.
Reticle guard is disabled by default; absence of regression requires
actual Proton/Windows screenshots, not only Windows CI.


## Proton reticle-guard runtime result and camera setup static analysis (15 Nov 2026)

Uploaded log CrimsonSkiesFix(20261115-094834).log: WinMM 1.1.0-rc.1-2564ac76; GE-Proton11-12, Wine11, Manjaro, 1920x1080. Both ExperimentalProjectedY43Probe and ExperimentalY43ReticleGuard enabled. No screenshot of this exact build was supplied, so the visual effect of the guard has NOT yet been independently validated.

- PROJECTED-Y43-RETICLE-GUARD: first 8 traces show original, square 32x32-pixel center quads, e.g. bbox x=[942.5,974.5] y=[531.5,563.5], forwarded unmodified. Last cumulative reticle_kept_round=16258. This definitively validates invocation of the *experimental selector*, not full visible reticle shape.
- PROJECTED-Y43-CENTER still reports 12130 other centered initially square quads, with original_ratio=1 and new_ratio=0.75. First candidate bbox is about 79.73x79.73. Do NOT broadly exclude all these from Y43: some could be world sprites/target indicators, not the reticle. Identify by texture or verified caller if a new selector is necessary.
- Last sampled global Y43 counters: corrected_categories=0x3f; scratch_reuses=15298838, scratch_grows=49, reserve_failures=0; skip_format=skip_large=skip_unreadable=0; skip_invalid=1066, moved_top=1605863, moved_bottom=1426356. These boundary counters count draw calls, not visible black triangles or pixels. Do not infer black-triangle causal identity.
- Four successful epochs (1920x1080 startup, 800x600 menu, 1920x1080 flight, 800x600 menu), no ERROR/CRITICAL. Accelerated HUD renderer_mode=Some(1) replicate=Some(0), old canvas branch inactive. Zero calls to extra Strided and VB paths.

Independent static disassembly of the user-supplied *decrypted official 1.02* PE32 (SHA256 43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75) using objdump:
- VA 0x0042B570 is x87 FOV/camera conversion. It evaluates fptan on an angle converted with the coefficient at 0x006032E0; multiplies by global aspect H/W at 0x0071B3F8; applies fpatan and double-angle, then calls 0x004D2C20 to set a camera/render device property. It also invokes 0x004D2E30, which stores a camera parameter into renderer object field +0xD0 for expected state 1.
- The owner-supplied 1920x1200 prepatched EXE contains injected code at 0x006024D9 and 0x00602508. This computes fptan(angle)*((width/height)/(4/3)) before fpatan, i.e. camera FOV math was already modified in the exact 38-region patch we ported. Our Y43 raster modification is an additional *post-projection* stretch.
- 0x004D70A0 validates a type-3 rendering object and reads two fields from its internal +0x38 pointer; its precise relation to clip-space is unproven. Do not claim the true clipping entrypoint is identified.

Next research: find the viewport projection and polygon clipping agreement *before* the XYZ/RHW screen-coordinate stage using the verified camera properties. Keep the working 38-region FOV patch and optional reticle guard as-is; avoid ad hoc clipping, huge-coordinate clamps, or unverified changes to the game executable. Draft PR #36 remains unreleased.


## 16 November 2026: original 4:3 versus corrected widescreen HUD rings

After the split WinMM pipeline was repaired, the owner asked to eliminate
the remaining **~2% ellipse** in five instrument bezels. We recovered
two original user screenshots at the *same physical* 1920×1200 desktop
resolution:

- `image(20261114-194052).png` — native centered **1600×1200 4:3**
  gameplay (original ring aspect reference).
- `image(20261116-162001).png` — native **1920×1200 16:10**
  gameplay with the verified in-memory FOV/HUD delta and split WinMM DD4
  backend (same aircraft/HUD style but scene state not pixel-identical).

Measured instrument bezel extents in the 16:10 capture are ~145×149,
145×148, 147×150, 145×148 and 145×148 pixels, giving width/height
ratios **0.973–0.980**. The previous true 4:3 A/B had approximately
147×149/150 pixels (**0.980–0.987**). The raw images have notably similar
left/right ring details at the same screen locations: dark bezel pixels
overlap within a 10/255 channel tolerance for roughly **84–92%**
of aligned dark bezel pixels for the five instruments (background,
pointer positions, animation and JPEG/PNG sampling differ).

**Interpretation:** the large native widescreen distortion from November
14 (≈147×125 instead of ≈147×150) is gone; the residual 1–3% discrepancy
is essentially already present in the true 4:3 source/reference. This
comparison alone cannot distinguish minor artist-painted noncircularity
from rendering quantization or a genuine remaining 1–2 px sprite scaling
error. Both the 3D aircraft and the player's reticle are rendered through
preprojected FVF=0x1C4 pathways, so a generic Y or X vertex rescale risks
reintroducing the earlier 3D FOV, aircraft, reticle and clipped-screen
regressions.

**Do NOT** silently add a second global projection, DD4 Blt, postprocess,
or HUD-canvas scale to "fix" these few pixels. If geometrically perfect
circles are desired (beyond fidelity to the original 4:3 rendition),
first prove which **SW_GAUGES instrument sprite/geometry draw calls** own
the bezel vertices and isolate those assets. Limit any correction to
the bezel, not its background, labels, airplane, whole HUD, menu or
reticle. Keep this as an independent opt-in experiment before promotion;
there is no supported gauge-specific correction in the present PR #44.

This result **does not close issue #41's world-aircraft aspect investigation**.
