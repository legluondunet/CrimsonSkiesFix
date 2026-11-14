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
