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
