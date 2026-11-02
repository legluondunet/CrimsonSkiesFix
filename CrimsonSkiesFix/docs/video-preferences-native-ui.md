# Crimson Skies 1.02 — Video Preferences native UI ownership

## Evidence: user test of v0.1.80

The owner-provided `CrimsonSkiesFix(5).log` (2 Nov 2026, Wine 11 /
GE-Proton11-11) reports an authenticated 1920x1080 native D3D3 launch,
800x600 virtual menu and correct offscreen MainMenu watermark.

The experimental `SendMessageA` IAT replacement was **installed**,
but did **not** control the Graphics or Resolution dropdowns:

| Exact diagnostic | Observed |
|---|---:|
| `MENU-NATIVE-COMBO: hook_available` | true |
| `iat_calls` after entering Video Preferences | 1 |
| `iat_addstring` | 0 |
| Win32 native `ComboBox` child windows in each scan | 0 |
| Win32 Graphics or Resolution controls found | 0 |

Eight consecutive process-owned window/child enumerations returned
`windows=8 native_combos=0 recognized_graphics=0
recognized_resolution=0`. Even *after* `gosCallback` command 13
reported opening Preferences, the scan yielded no controls.

**Conclusion:** Neither the Win32 `ComboBox` class-enumeration route
nor the original EXE's `SendMessageA` import identifies the **visible
in-game Video Preferences** selectors. The game does have code using
`SendMessageA` with `CB_ADDSTRING`, but it is likely for the
**separate startup video-device dialog** (now bypassed), not the
DirectDraw-rendered in-game Preferences screen. The latter is a
logical 800x600 frame, not ordinary child Win32 controls. We cannot
prove all possible Win32 APIs are unused from this trace alone, but
the attempted hook demonstrably had no effect.

That disproved code was fully **removed**:
- `src/menu_native_combo.rs` deleted, including experimental window
  enumeration and IAT mutation.
- Removed its initialization in `src/lib.rs`.
- Removed its periodic 4:3 primary Flip scans from
  `src/com_surface4_forward.rs`.
- Removed the inert standalone probe stub.
- No replacement fake mouse-coordinate hit zones or edit of the
  game's resource files.

## Verified direct original-code path

The exact supported executable is
`CRIMSON-UNWRAPPED.EXE`, 2,580,480 bytes,
SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.
Preferred image base `0x00400000`.

### In-game GOS interpreter

The original game registers `gosCallback` at `0x00407670`
through the `0x004075D6` registration instruction. Its event
dispatcher at `0x00407670` indexes a jump table at `0x00408368`.
In the owner's Wine logs:

| GOS command | Original action | Test evidence |
|---|---|---|
| 13 | Prepare Preferences: `0x004077AC` calls `0x00418D30`, then copies `0x81` DWORDs from active to pending (`0x004077B6–0x004077C5`) | Observed when entering menu |
| 14 | Apply: `0x004077D9` copies the same `0x81` DWORDs pending to active, then calls `0x00419160` | Confirmed in earlier Wine trace |
| 15 | Mode request: branch at `0x00407806` calls `0x00419420` at exact `0x00407811`, then copies mode table and index | Verified guard blocks requests `0`, `1` |
| 16 | Additional menu event; **not proven** to exclusively mean cancel | Observed during opening/transitions |

The active video settings structure starts at `0x0064B450`:
- Active device: `0x0064B490`
- Active mode index: `0x0064B48C`

The menu's pending copy starts at `0x00648330`:
- Pending device: `0x00648370`
- Pending mode index: `0x0064836C`

The interpreter branch at `0x0040CCE1` reads both pending fields
to external value pointers; the `0x0040CD62` branch writes them
back (`0x0040CD64`: device; `0x0040CD6E`: mode). These **are
backend script property transfers**, not yet the widget object IDs
nor the displayed text renderer. Writing `1920` to a mode index
would corrupt mode-table indexing.

### Separate startup Win32 dialog

The exact x86 function at `0x00444900` reads imported
`SendMessageA` from `0x00A20488` at `0x004449AE`, then sends
`CB_ADDSTRING` and `CB_SETCURSEL`. Its helper `0x00444A50`
populates modes. **This is not proof that the in-game GOS Video
Preferences widgets use that window**, and the repeated failed tests
of v0.1.79/0.1.80 disprove its usefulness as an in-game hook.

## Next safe engineering step (not implemented)

Identify the *actual DirectDraw/GOS menu widget instances*, their
item selection callback and their displayed-text owner. Only after
verifying those bindings should we:
1. Disable the actual `Graphics` and `Resolution` widgets,
   preserving all other Video options.
2. Show the current launch monitor width/height as **UI text** while
   preserving internal 800x600 4:3 menu surfaces and original
   mode-table index.
3. Validate on Wine and Windows with Menu → Video → MainMenu, a second
   monitor and in-game native FOV/HUD/3D.

Already validated and intentionally preserved:
- Backend resolution mode-request guard (v0.1.73).
- Backend graphics-device commit guard (introduced v0.1.76), though
  a different pending Graphics selection has **not** yet been tested.
- Offscreen MainMenu signature and native scene-art gating (v0.1.78).
- Native 3D resolution/FOV/HUD and 4:3 menu/video presentation.

This document records read-only inspection and the rollback of the
failed Win32 approach; it does not claim the two menus are disabled.
PR #9 remains draft and unmerged.

legluondunet assisted by ChatGPT (OpenAI)

## New verified GOS UI entry points (2 November 2026, v0.1.81 review)

The uploaded v0.1.81 `CrimsonSkiesFix(6).log` confirms the removed
Win32 interception is no longer installed. It also establishes native
1920×1080 DirectDraw/D3D3 with 800×600 virtual RGB565 menu, successful
MainMenu offscreen branding, suppression in Video Preferences and later
recognition of MainMenu artwork. The active and pending video fields
stay `device=1 / mode_index=0` in this session; it does **not**
exercise a changed graphics selection or visibly fix the lists.

**Further direct analysis of the owner's original clean 1.02 EXE**
(SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`)
revealed that the game registers multiple **native GOS script functions**
right beside `gosCallback`:

| Original registration instruction | GOS symbol (string VA) | Original handler VA | Opcode bytes at registration |
|---|---|---|---|
| `0x004075D6` | `gosCallback` (`0x0061F150`) | `0x00407670` | `68 70 76 40 00` |
| `0x004075F4` | `uiData` (`0x0061F15C`) | `0x004093A0` | `68 A0 93 40 00` |
| `0x00407612` | `uiControl` (`0x0061F164`) | `0x00404960` | `68 60 49 40 00` |
| `0x00407630` | `gosPageInit` (`0x0061F170`) | `0x00403680` | `68 80 36 40 00` |
| `0x0040764E` | `gosLocalize` (`0x0061F17C`) | `0x00404630` | `68 30 46 40 00` |

The registration code at `0x004075E0`, `0x004075FE`,
`0x0040761C`, `0x0040763A`, and `0x00407658` calls the
same script function registration helper `0x00406FF0`, then
registers the returned descriptor through `0x005D3AD0`.
The function names, addresses, original registration bytes and
dispatch into handlers were verified from the exact EXE's binary,
not inferred from screenshots or a search result.

Disassembly of `uiControl` at `0x00404960` shows it reads the first
DWORD of its third argument as a script operation code and dispatches
on values including `6`, `0x85D`, `0x85E`, `0x868`, `0x9C6`;
it uses a jump table at `0x00404EA0` for `0x85D` subcommands.
Likewise, `uiData` at `0x004093A0` dispatches a family of script
opcodes beginning at `0x7D0` through a table at `0x0040F518`.
These functions end in `ret 0xC` and use the three-argument GOS
callback convention, making a **pass-through, read-only registration
observer** a feasible next diagnostic experiment.

**Interpretation boundary:** Identifying these GOS script API handlers
does not yet identify the two specific Video dropdown instances or
their display-string owner. A future trace should narrowly and
separately associate `uiControl` and `uiData` opcodes with entering
the Video tab, opening each dropdown and opening an unrelated option,
without guessing pointer types or swallowing any call. Only then
should a widget-specific disabling or label-setting implementation be
attempted. The verified `gosLocalize` script entry point is another
possible text ownership route, but no causal link to the Resolution
field is established yet.

No user-facing DLL behavior was changed based on this finding.
The present v0.1.81 release remains the validated cleanup baseline.


## v0.1.82 diagnostic implementation — script opcode boundaries

Byte-for-byte disassembly of the owner-supplied clean 1.02 EXE
established the following exact dispatch ranges; these are
**native GOS interpreter opcodes**, not widget IDs:

- `uiControl` at `0x00404960` reads its script operation at
  `[arg3+0]`, then uses the jump table `0x00404EA0` for
  `0x85D` subcommands 0–8 at `[arg3+4]`; other direct operations
  include `0x85E`, `0x860`, `0x868`, `0x899`,
  `0x96E`, `0x9C5..0x9C8` and `0xC1C..0xC24`.
- `uiData` at `0x004093A0` first reads its operation and
  next parameter from `[arg3+0]` and `[arg3+4]`.
  The 39-entry direct table at `0x0040F518` covers
  `0x7D0..0x7F6`; `0x833` has an additional direct
  branch. A **second dispatch stage** (previously overlooked)
  covers `0x834..0x96D` using an indexed byte table at
  `0x0040F788` followed by a common target table at
  `0x0040F5B4`. That compressed stage maps 314 possible
  opcodes onto 117 distinct handlers; many code values map to
  the common default `0x0040F507`. The higher `0x9C9`
  has a distinct special case.
- Both handlers return with `RET 0xC`, so a strict
  three-argument stdcall pass-through wrapper preserves the original
  calling convention. The new observer also retains the original
  function arguments and return value, logging only numeric opcode,
  small/scalar second-parameter category and return *class*.

The new `CrimsonSkiesFix/src/menu_ui_trace.rs` registers these
read-only observers at the original `PUSH imm32` sites after
successful executable SHA-256 verification, when `[Debug] Log=1`.
The log gate additionally requires a 640×480/800×600 virtual frame
and an existing GOS Preferences transaction (command 13). Entries
are capped to 96 `uiData` and 72 `uiControl` changed signatures.
Unknown pointer values are **not** logged as addresses. The
observer is disabled completely when logging is off.

**Experiment status:** Static instruction-byte checks and Windows
x86 compilation tests do not prove the relevant callbacks are
invoked by the Video tab. Owner Wine runtime traces are required
to associate a particular command with opening Graphics, opening
Resolution and using Viewing Range. No input blocking or text
overrides are implemented by this observer; preserving existing
backend guards is intentional.


### Exact Video script transfer IDs recovered from `uiData`

The compressed `uiData` dispatcher (tables `0x0040F788` and
`0x0040F5B4`) can be inverted against the original executable:

| `uiData` opcode | Handler VA | Confirmed action |
|---|---|---|
| `0x83E` | `0x0040CDE2` | Copies `0x81` DWORDs from active `0x0064B450` to pending `0x00648330` video settings |
| `0x855` | `0x0040CCE1` | Reads or writes **nine** pending video settings, including Graphics `0x00648370` and Resolution mode index `0x0064836C` |
| `0x850` | `0x0040CC71` | Reads or writes three additional settings at `0x00648528..0x00648530`; not proven to be Graphics/Resolution |
| `0x83F` | `0x0040D018` | Another script operation; widget ownership not yet established |

In the `0x855` handler, a zero second descriptor DWORD goes to
`0x0040CCE8`, where pending values are read into output
parameters. A nonzero second DWORD branches to `0x0040CD62`,
where the values supplied by the script are assigned to the
pending structure; individual assignments occur at
`0x0040CD64` (device) and `0x0040CD6E` (mode index),
alongside seven other video option fields. This is the **first
exact, independently decoded GOS opcode-level link** to the
Video Preferences data structure.

The v0.1.82 trace now reports a separate bounded event
`MENU-GOS-UI-VIDEO: uiData opcode=0x855 operation=get|set
pending_device=A->B pending_mode=C->D` when these values first
appear or change. It calls the genuine original handler with
the untouched three arguments. No script pointers are printed,
no selection is blocked, and no UI text is replaced.

This does **not** identify the original widget's input or
text render component: one `uiData 0x855` call transports
**nine settings together**, so refusing that entire opcode
would also break Viewing Range, Effects Level, Lighting, etc.
The final fix must target the actual two GUI selections/labels,
not blindly suppress opcode `0x855`.

