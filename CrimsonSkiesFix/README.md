# CrimsonSkiesFix 0.1.85 — experimental DirectDraw proxy

**Status:** x86 DirectDraw proof of concept; the 60-calls/s limiter on
modern primary Flip was measured under Wine, but gameplay simulation
speed and borderless still require live validation. This PR does
not modify the original game executable.


## v0.1.85 — exact original GOS resolution-label builder identified

The owner-tested `CrimsonSkiesFix(9).log` from v0.1.84 reports
that the new read-only page and localization hooks installed, and
provides a strong correlation between the in-game Preferences > Video
screen and its native GOS text/resources:

- Preferences page startup localized 1078..1082 (five distinct IDs).
- A later page requested 1099..1117 (19 consecutive localized IDs).
- `uiData(0x855)` was called to **read** nine pending video fields:
  active/pending device=1, mode index=0, seven-option change mask=0.
- `uiData` calls 0x844, 0x845, 0x83F, 0x840, 0x841, 0x842 and
  0x847 occur near the resolution widgets.
- `gosCallback` mode requests 0 and 1 were both
  **blocked=true, changed=false** as intended.
- The apply guard command 14 ran with unchanged pending
  Graphics=1 and Resolution index=0. No changed pending device
  was tested in this file.
- The RGB565 MainMenu watermark still draws and presents
  successfully, including after returning from Preferences.

**New precise disassembly discovery** from the one SHA-256-verified
unwrapped 1.02 EXE (full original 2,580,480 bytes):

`uiData` opcode **0x845** dispatches to `0x0040CEF7`:
- If the supplied index equals `0xFFFFFFFF`, it returns the
  original resolution-mode count stored at `0x0064B498`.
- Otherwise it reads a packed 32-bit `height<<16|width`
  value from `0x0064B49C + index*4`, splits its low/high
  halves and calls formatter `0x004054B0` with string-resource
  template **0x206**, then passes the resulting string through
  `0x00403350` to the script output.
- This is a **specific confirmed original resolution-label
  construction path**, unlike the failed static-string patch
  in v0.1.77 or the non-existent Win32 ComboBoxes of v0.1.80.
  The backend mode index is not itself a pixel width.

Opcode **0x847** dispatches to `0x0040CFE4`, returns a
fixed count of **five** options, and localizes item IDs
`0x72 + index`. **Its exact visual widget is not yet
verified**; do not label it as Graphics merely because it is
called near the Video page.

**v0.1.85 instrumentation:** Added read-only
`src/menu_ui_list_probe.rs` to record original 0x845/0x847
list requests, including count/index and validated packed
resolution dimensions, capped at 32 events per list type.
Both original opcode instruction preimages are byte-checked.
The newly separated diagnostic Preferences data-session state
is not reset by the independent Campaign-art watermark detector;
only native command 14/16 or actual MainMenu script reload
end that trace. No menu values, list counts, text, pointers,
screen geometry, mode table, game files or input are changed.

**Still outstanding:** visually disable Graphics and Resolution
and display the actual launch-monitor dimensions in the
Resolution *widget*, while preserving all unrelated settings.
This is still a diagnostic build, not a final UI patch.
Do not merge PR #9 before Wine and Windows validation.

## v0.1.84 — identify the real GOS Video UI page and text owner

The owner's v0.1.83 Wine/GE-Proton tests explicitly validated
MainMenu watermark at launch and after returning from gameplay.
This diagnostic build **retains all v0.1.83 gameplay/rendering
features without modifying the on-screen Video dropdowns yet**.

Our previous read-only `uiData(0x855)` trace was overly restrictive:
after gameplay, the original script dispatcher can report
`scene=OTHER` while the game displays the MainMenu family UI
(the exact regression repaired in v0.1.83). Therefore
`menu_ui_trace.rs` now requires the original MainMenu script to
**have been observed at least once**, a real 640x480/800x600 menu
frame and the GOS Preferences transaction hint. It does not
require the stale script scene to still equal `MAIN`.

Static x86 disassembly verifies that `uiData(0x855)` reads or
writes **nine** separate 32-bit Video Preferences fields:
`0x648370` (Graphics device), `0x64836c` (Resolution mode
index), and seven others at `0x648354`, `0x648358`,
`0x648350`, `0x648364`, `0x648368`, `0x64835c`,
`0x648360`. On every sampled setter (max 24), it reports
the exact Graphics and Resolution before/after values plus a
**seven-bit other-settings change mask**. No preference value,
input buffer, option, or original callback output is changed.
This allows comparisons with unrelated settings such as
Viewing Range without ever blocking their shared opcode.

The exact original 1.02 executable also registers:
- `gosPageInit`: `0x00407630: PUSH 0x00403680`
- `gosLocalize`: `0x0040764E: PUSH 0x00404630`

The new `src/menu_ui_pages.rs` provides separate, byte-verified,
strictly pass-through observers of these two GOS APIs:
`MENU-GOS-PAGE: gosPageInit invoked` reports bounded
page-initialization occurrences after the MainMenu was first
observed. `MENU-GOS-LOCALIZE: localized resource_id=N`
records at most 80 small original numeric text-resource IDs
inside the Preferences transaction. It does **not** read
or replace localized text strings, display buffers, user input,
button properties or game settings. No arbitrary pointer value
is logged.

These traces help distinguish page initialization/text requests
from the nine-setting transport call, and may identify which
resource must ultimately own the `640 x 480` visual label and
the custom dropdown disabled state. The previously attempted
Win32 ComboBox interception remains **removed**.

**Owner test:** launch with `[Debug] Log=1`, open
Preferences > Video, expand Graphics, select another entry
but do not apply; expand Resolution, select another entry;
change Viewing Range as a comparison, then apply and return
to MainMenu. Send the new `CrimsonSkiesFix.log`.
Do not rely on this build for the final appearance: **Graphics
and Resolution still open and the UI still shows a legacy
resolution label**. Existing commit-time and mode-request
backend guards are unchanged. PR #9 remains draft/unmerged.

## v0.1.83 — MainMenu signature return validated under Wine

**Owner validation (2 November 2026):** The owner explicitly confirmed
that the signature stays present on MainMenu after returning from a
game. Their `CrimsonSkiesFix(8).log`, using
`CrimsonSkiesFix 0.1.83-9d5ea7d4` under Wine 11.0 /
GE-Proton11-11 on a 1920×1080 monitor, independently verifies
initial composition and two later reentries:
- Initial `MENU-BRANDING: offscreen RGB565 menu text composited
  before primary Blt` followed by
  `signature frame actually presented by native RGB565 primary Blt`.
- After returning from gameplay, `MENU-SCENE: other script`
  occurs, but `MENU-BRANDING: confirmed native MainMenu Campaign
  artwork`, `MainMenu signature COMPOSITED after return;
  reentry=2 script_main=false`, and successful primary Blt confirm
  reappearance despite stale script identity.
- Another return produces `reentry=3 script_main=false`,
  accompanied by `Campaign artwork verified; stale GOS Preferences
  visibility hint cleared` and successful primary Blt.

**Status:** MainMenu signature at startup and after a game, and its
absence when the Campaign artwork is not present, are **validated on
the owner's Wine/GE-Proton configuration**. Do not interpret this as
validation on every Windows setup or monitor arrangement. No new
branding-related code is needed for this particular reported issue.

The separate **Preferences > Video** UI work is still open:
Graphics and Resolution dropdowns remain visually interactive,
and the Resolution label still shows the legacy mode instead of
the desktop dimensions. Keep draft PR #9 unmerged until that work
is completed and tested.

## v0.1.83 — restore signature after gameplay → MainMenu (experimental)

The owner reports that after finishing gameplay and returning to the
main menu the build label disappears. In the owner's v0.1.82 Wine
journal, the first menu is branded, but after subsequent Preferences
transactions the original GOS command **13** marks the internal
`Video Preferences opened` hint again while **the actual Campaign
menu artwork is later recognized**. The last line of the 343-line log
is `MENU-BRANDING: confirmed native MainMenu Campaign artwork;
watermark eligible`, not a second verified primary presentation.
The original code only reported the first successful offscreen
signature composition per process, so a missing later log cannot
indicate whether the second attempt drew or presented successfully.

The original menu-scene observer also watches just four verified
script-loader CALL instructions. It can miss a return from gameplay
which restores MainMenu without calling any of those sites.
`menu_scene` therefore now remembers whether a **genuine MainMenu
script was ever observed**. On later 640x480/800x600 RGB565 frames,
the renderer is allowed to probe the unchanged private scratch
even if the last observed script identity is stale. The build
label is composed **only if the same 8-point Campaign-button artwork
test confirms the actual MainMenu**. It is not shown in gameplay
at native desktop dimensions, Video Preferences or unrelated menus
where those art points are absent. If in doubt, present the original
game frame unmodified. It does not change input interception, game
resources, DirectDraw geometry or the backend video guards.

When actual MainMenu artwork is verified, any stale internal GOS
Preferences-visibility hint from command 13 is cleared. This
affects only the DLL's own scene diagnostics, **not** the game's
Preferences variables.

To distinguish offscreen drawing from actual presentation, the
logging now reports transitions on **each** successful MainMenu
return, rather than only the very first frame:
`MENU-BRANDING: MainMenu signature COMPOSITED after return;`
and, after successful native primary Blt,
`MENU-BRANDING: signature frame actually presented by native RGB565
primary Blt`. Only these after-return diagnostics can help
distinguish successful drawing from frame delivery; the in-game
visual result still requires confirmation under Wine.

**Validation requested:** start on MainMenu, open Preferences and
return, start a game and return to MainMenu, repeat and inspect
the new messages along with a screenshot. No PR merge until
this behavior is confirmed. The custom Graphics/Resolution
dropdowns and current-monitor UI text are still unfinished.

## v0.1.82 — observe native GOS uiData / uiControl callbacks (read-only)

New static analysis of the *original owner-supplied* unwrapped
Crimson Skies 1.02 executable (SHA-256
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`)
confirms the GOS script function registration sequence:

- `gosCallback` (existing proven observer): `0x004075D6`,
  handler `0x00407670`.
- `uiData`: `0x004075F4: PUSH 0x004093A0`, original handler
  at `0x004093A0`.
- `uiControl`: `0x00407612: PUSH 0x00404960`, original handler
  at `0x00404960`.

The two newly investigated functions accept **three stack arguments**
and return using `RET 0xC`. `uiData` reads an operation code
from its script descriptor and uses a jump table at
`0x0040F518` for `0x7D0..0x7F6`, plus additional cases.
`uiControl` dispatches `0x85D` subcommands `0..8`
using table `0x00404EA0` and other discrete opcodes.
This establishes concrete script entry points but **not yet**
that either handles the Video Graphics or Resolution widget.

A new `src/menu_ui_trace.rs` provides *bounded, strictly
pass-through* registration observers for these two functions.
They install only with `ExperimentalWindowedAdapter=1` and
`[Debug] Log=1` after the same mandatory EXE SHA gate.
The original in-memory registration `PUSH imm32` bytes are
verified and page protections are restored; on any mismatch
the original handler is retained. Both wrappers forward the
exact 3 arguments and native DWORD return. Neither handler's
input/output or game preference state is changed.

Only after original `gosCallback` command 13 has started the
Preferences transaction, while the game is presenting its
640×480/800×600 menu with the original MainMenu script loaded,
are diagnostic entries eligible:

```text
MENU-GOS-UI: function=uiControl opcode=0x85d arg_class=...
MENU-GOS-UI: function=uiData opcode=0x7d0 arg_class=...
```

Logging is deduplicated and limited to 72 `uiControl`
and 96 `uiData` opcode/argument-signature transitions.
Unknown pointers and large native return values are **never
dereferenced or printed as addresses**; logs classify them
only as scalar/non-scalar or result category.

**Testing goal:** first open Video Preferences; separately
open Graphics and Resolution, interact with each list, then
change *Viewing Range* as a control comparison. Inspect
`MENU-GOS-UI` entries and the existing
`MENU-GOS-EVENT`/ `MENU-VIDEO-PROBE` values to identify
which exact script command belongs to which widget.
Only then should a per-widget disable or visible desktop
resolution override be implemented.

For independent, read-only verification on the one accepted clean EXE,
use `python3 tools/inspect_gos_ui.py /path/to/CRIMSON.EXE`.
The script verifies the full SHA-256, exact original GOS callback
registration bytes and the `uiData` opcode-to-handler tables, without
running the game or requiring Wine.

The v0.1.73 mode-request and v0.1.76 apply guards, the
owner-confirmed 0.1.78 MainMenu signature, native resolution
and aspect-correct 4:3 menu remain unchanged. **The visual
Graphics and Resolution dropdowns still open, and Resolution
still displays 640 x 480.** This is an instrumentation build,
not the final requested UI fix. PR #9 remains draft.

## v0.1.81 — remove nonfunctional Windows ComboBox experiment

The owner supplied the v0.1.80 Wine trace, which conclusively
shows that while the `SendMessageA` import hook installed
(`hook_available=true`), it captured only **1** generic call and
**0** `CB_ADDSTRING` messages. Eight complete process-owned window
scans found **0** native Win32 ComboBox controls, both before and
during Video Preferences. No Graphics/Resolution control was disabled
and `Resolution` still showed the legacy `640 x 480`.

That experimental Win32-only approach was **not working**. This
revision removes its IAT patch, its periodic process-window enumeration,
the complete `menu_native_combo.rs` source module and the standalone
probe stub. No desktop monitor or game window enumeration occurs on
each menu Flip anymore.

The accepted 1.02 EXE does have Win32 `CB_ADDSTRING` code at
`0x00444900`, apparently for a **different, startup video-device
chooser**, not the game's DirectDraw-rendered in-game Preferences
panel. The verified GOS callback path is command 13 to prepare,
14 to apply and 15 to request a resolution mode, while the interpreter
reads/writes pending device/mode values at `0x0040CCE8` and
`0x0040CCF2` / `0x0040CD64` and `0x0040CD6E`.
See `docs/video-preferences-native-ui.md` for reproducible
binary evidence, the precise original executable hash and the
remaining widget-level research needed.

**Known incompleteness:** the on-screen Graphics/Resolution lists are
still interactive and Resolution still shows the game's legacy
mode text instead of the current desktop dimensions. The original
v0.1.73 resolution backend guard and v0.1.76 Graphics commit guard
remain, as do the owner-validated v0.1.78 watermark behavior and
native 3D 1920×1080 FOV/HUD. Do not merge draft PR #9 until the
original GOS Video UI widgets, their input callbacks and displayed
label owner have been established and tested in-game. This is a
cleanup / reverse-engineering release, **not** a visual Video menu fix.

## v0.1.80 — discover already-created native Video ComboBoxes (experimental)

The owner-tested v0.1.79 log confirms `MENU-NATIVE-COMBO:
hook_available=true` and the game continues running at native
1920×1080 with the Menu signature, **but neither the Graphics nor
Resolution list became disabled**. No
`MENU-NATIVE-COMBO: Graphics native ComboBox disabled` nor the
corresponding Resolution record appeared. That means the import
hook installation itself succeeded but **does not prove any actual
`CB_ADDSTRING` messages were observed**. The prior code logged only
matched controls, not whether it was ever invoked.

The exact supported EXE disassembly confirms the `0x00444900`
graphics control setup and `0x00444A50` resolution helper use
`SendMessageA` at the verified import `0x00A20488`.
One plausible explanation is that some Win32 ComboBoxes were
constructed *before* the first DirectDrawCreate and thus before
the IAT hook. Another possibility is that those messages reach a
different thunk, or the live GUI uses an unexpected window class.
None is proven by the v0.1.79 log.

**New bounded fallback:** During a logical 640×480/800×600
MainMenu-family frame, at most once per 90 frames, enumerate only
top-level windows owned by the **current game process**, their native
Win32 child windows and ComboBox item texts. Identify Graphics only
when the *same native ComboBox* contains both exact original
`DirectDraw HAL` and `Software Render` options; identify
Resolution only when all its entries are valid `WIDTH x HEIGHT`
mode strings. Never infer controls from screen coordinates, owner
drawn item data, window titles or unrelated processes. A matched
Graphics box is disabled by `EnableWindow(FALSE)`. On a matched
Resolution box, replace the **selected item** with the actual
monitor dimensions using `CB_DELETESTRING`/`CB_INSERTSTRING`,
and preserve its original `CB_GETITEMDATA` and selected index
before disabling. Skip sorted lists and owner-draw no-string
controls, since updating their labels without their native
handlers would be unsafe.

The existing IAT hook now also counts all intercepted
`SendMessageA` calls and `CB_ADDSTRING` calls and reports their
class for at most eight calls, without logging arbitrary user
window text. The bounded scan inventory logs:
`MENU-NATIVE-COMBO: owned-window-scan=N windows=...
native_combos=... recognized_graphics=...
recognized_resolution=... uninspectable=...
iat_calls=... iat_addstring=...`.
It distinguishes absent callbacks from native-class mismatch
and an already-created control.

**This is a new experiment, not a verified fix.** If the UI is
not an inspectable Win32 ComboBox or its strings are represented
by owner-draw item data, no external controls are changed. The
v0.1.73/0.1.76 backend mode and graphics apply guards remain in
place. The v0.1.69 offscreen watermark and validated v0.1.78
menu-detection logic are unchanged. Check the new scan and
interception diagnostics together with screenshots before
considering PR #9 for merging.

## v0.1.79 — target actual Win32 Video ComboBoxes (experimental)

The owner confirms v0.1.78 appears to fix the MainMenu version/commit
watermark returning after visiting Preferences, with no watermark on
the Video panel. Wine log independently records the new
`MENU-BRANDING: confirmed native MainMenu Campaign artwork` and
`native MainMenu artwork absent` transitions in the same session.
The exact original v0.1.69 offscreen RGB565 composition is unchanged.
This is validation on the owner's tested configuration, not a claim of
exhaustive multi-monitor or Windows compatibility.

**Important x86 discovery from the exact accepted v1.02 EXE:**
The original `0x00444900` Video control constructor and
`0x00444A50` mode-list helper call the game's imported **Win32
`SendMessageA`** at `0x00A20488`, including the actual native
ComboBox messages `CB_ADDSTRING=0x143`, `CB_SETITEMDATA=0x151`
and `CB_SETCURSEL=0x14E`. This disproves the earlier assumption
that these dropdowns must be non-Win32 GOS-only widgets.
The static `0x00623A1C` `640 x 480` string modified by v0.1.77
is an optional fallback item, not the guaranteed current visible
label: the constructor also builds dynamically generated mode names.
That is why changing only that static source string could not fix the
real UI in the owner's screenshot.

**New experiment:** The v0.1.79 DLL removes that ineffective
source-string mutation and instead hooks only the exact supported
EXE's **SendMessageA IAT slot**, after SHA validation and verifying
the original x86 IAT-load instruction. The hook forwards all
unrelated messages byte-for-byte to original `SendMessageA`. For
native `ComboBox` windows, when `CB_ADDSTRING` inserts an original
legacy resolution option (e.g. `640 x 480`, `800 x 600`),
its **visible list-item text** is substituted with the actual launch
monitor dimensions, e.g. `1920x1080`, without altering the
original item index or `CB_SETITEMDATA` numeric mode mapping.
On seeing exact original Graphics options `Software Render` or
`DirectDraw HAL`, or any resolution option, the corresponding
**native ComboBox** is disabled via `EnableWindow(FALSE)`.
Other Video controls are not disabled or relabeled.

This approach does **not** block arbitrary mouse coordinates or
change DirectDraw display sizes, FOV/HUD, game mode tables or the
original accepted EXE on disk. The pre-existing command-15 resolution
guard and command-14 Graphics commit guard remain as a fallback.

**Caveats:** This is not yet validated in Wine. The original EXE may
resolve `SendMessageA` to a different import thunk, or custom
subclasses may expose a nonstandard class name; if so, the new hook
fails closed or does not match those controls, and original behavior
is retained. Success requires owner screenshots showing the
**Resolution field** with native dimensions **and** both disabled
dropdown arrows, plus logging:
`MENU-NATIVE-COMBO: ... Resolution native ComboBox disabled`
and `... Graphics native ComboBox disabled`.
Check that the other Video options and `Accept Changes` remain
usable, gameplay remains at the native desktop resolution, and the
MainMenu watermark is not displayed over nested panels.

Do not merge PR #9 until these actual menu behaviors are tested.

## v0.1.78 — MainMenu return experiment; native Resolution text still unresolved

The v0.1.77 owner screenshot/log verifies the Video Preferences watermark
is hidden correctly and the real desktop remains 1920x1080, but reports
two unresolved bugs:

- After returning from Preferences to the original MainMenu, the project
  version/commit label **does not come back**.
- Video > Resolution **still displays `640 x 480`**, including its open
  dropdown, despite the log saying that the source bytes
  `0x00623A1C: "640 x 480"` were changed to `1920x1080`.

The original 1.02 x86 game registers a source string used when populating
a legacy video list via `0x00444ADE: PUSH 0x00623A1C`. The successful
memory patch confirms that string's bytes changed; it does **not**
demonstrate that the UI widget updates or even uses the same live copy.
The log was corrected to report only the memory-source mutation, not
a visually changed dropdown. **This remains unfinished**; do not claim
that `1920x1080` is shown until actual UI validation.

The existing script observer compared scene *identity* and logged only
when it changed. It therefore cannot detect consecutive `MainMenu`
loader calls with the same script pointer. The v0.1.78 experimental
change additionally resets the Video-dialog branding suppression when
the exact original `MainMenu` script loader is **called again**, even
if scene identity was already `MainMenu`. A new diagnostic
`MENU-SCENE: MainMenu script requested again without scene identity change`
confirms this return path, if present. The offscreen RGB565 rendering
and confirmed hiding inside Video are unchanged. If no second original
script load occurs, this alone **will not restore** the label: the
actual native UI transition signal must then be identified.

**Additional frame-level validation:** Because original GOS command 13
is reused for multiple nested GUI transitions (and the original
MainMenu script may not be reloaded on Return to Menu), the version
overlay now verifies that the game's stationary **Campaign button**
actually appears in the 800×600 RGB565 menu image. Eight fixed
normalized sample points inside the button's gold fill were checked
against owner screenshots from two separate animated MainMenu frames
(all eight matched) and three Video/Preferences frames (at most two
matched). Only when **at least six** pass the conservative gold-color
test is the watermark composed on the offscreen frame. The test is
rendering-only: it never changes UI input, game values, menu content
or desktop geometry. Every transition in this check is logged as
`MENU-BRANDING: confirmed native MainMenu Campaign artwork` or
`MENU-BRANDING: native MainMenu artwork absent`.

A Wine implementation that cannot read RGB pixels through this
offscreen GDI DC falls back to the previous strict GOS dialog-state
guard (and logs still help identify the issue). The visual classifier
is an **experiment on the exact supported menu artwork**, not proof
that all game screens and display setups behave identically. Runtime
testing must confirm the label is visible on the genuine menu
**both before and after** visiting Preferences, and absent in the
Video/Preferences panels. If a different menu has similar artwork,
the classifier must be refined before merging.

The two custom Video dropdown arrows still open, even though the
back-end applies native Resolution/Graphics guards. Further widget-level
analysis is needed to disable them and show the desktop dimensions.
No screen-coordinate hit boxes, broad input interception or game-file
patches were introduced. Leave PR #9 as draft until complete.

## v0.1.77 — prevent watermark overlap in Video dialog; monitor-resolution label experiment

The owner's side-by-side screenshots of Preferences > Video and its open
Graphics dropdown prove three remaining UI problems in v0.1.76:

1. Graphics still opens and offers `Software Render` and `DirectDraw HAL`.
2. Resolution still displays `640 x 480`, not the physical desktop
   `1920x1080`.
3. The offscreen version signature is drawn over the **Cancel Changes**
   button in Video Preferences, despite having been requested on
   **MainMenu only**.

The script observer tracks `MainMenu`, but the game opens Video
Preferences **within that same script**. The existing GOS event observer
now tracks the exact original transaction lifecycle: command 13 enters
the Video Preferences editing transaction, and command 14/16 applies
or cancels it. Only after forwarding the original callback, the
renderer updates a state bit. The offscreen version label is now
rendered only for an unobstructed MainMenu, not when this Video dialog
is active. No changes to the owner-validated flicker-free RGB565
composition. If the callback observer cannot be installed, watermark
drawing fails closed instead of covering another menu.

The GOS registration observer is also now installed independently
of `Log=1` whenever all three native video options are enabled:
the pre-existing command-14 Graphics commit guard must **not**
silently stop working when diagnostic logging is disabled.

**New exact-resolution-label experiment:** The validated 1.02 EXE
contains a `PUSH 0x00623A1C` instruction at `0x00444ADE`
in the original **Video Preferences mode-list constructor**.
That data pointer contains the literal `640 x 480\0\0\0`.
The DLL checks both original byte sequences and the authenticated
game image, then replaces **only that UI text slot** with
`WIDTHxHEIGHT`, using the actual launch monitor size (for example
`1920x1080`) in the original 12-byte storage. The backend
mode index, DirectDraw geometry, 4:3 menus, HAL selection and
all other list options are untouched. On preimage/size mismatch, the
original text is kept. This is an early experiment: the native
control may use an independent cached label, so visual confirmation
is essential. Other legacy entries remain as originally displayed.

**Not complete:** The arrow widgets for `Graphics` and `Resolution`
can still be opened. The backend mode guard and commit-time Graphics
guard are retained, but neither custom dropdown has yet been natively
disabled. Do not mistake changing a label for disabling an input
widget. No guessed click-coordinate overlay is used.

Under Wine, verify (1) the watermark disappears while Video is open,
(2) `Resolution` displays native monitor dimensions,
(3) the watermark reappears only upon leaving Video for the genuine
MainMenu, and (4) all other video options remain usable. If the
label does not change, look for `MENU-RESOLUTION-LABEL` diagnostics.
Do not merge draft PR #9 until all requested native UI controls
and tests have been validated.

## v0.1.76 — protected Video Preferences Apply (experimental)

The owner-supplied v0.1.75 log verifies exact active/pending traces:
`active_device=1, active_mode_index=0, pending_device=1,
pending_mode_index=0` throughout the observed menu interaction.
The GOS sequence is `command=13 (prepare)`, `15 (mode request)`
twice, then `14 (apply)` and `13 (prepare)` again. Both
`request=0` and `request=1` were already blocked by the v0.1.73
mode-backend guard. No different pending Graphics device was observed;
this is **not** evidence that its drop-down was inaccessible.

The v0.1.76 experiment adds a separate safety check at the **exact
GOS Preferences Apply transaction** (`command=14`). Only after all
existing native-widescreen/desktop/menu/800×600 strict conditions
and original x86 device/mode read preimages pass, it restores
**the two pending fields only** from active values just before the
game's original `gosCallback` applies the 0x81-DWORD preferences.
Other options in the pending structure are untouched and remain
handled by the original game code. The existing resolution backend
guard is kept.

Expected when pressing Apply in the verified menu:
`MENU-VIDEO-LOCK: command=14 reset pending Graphics X->Y and
Resolution index A->B; other preferences unchanged`.

This should prevent an edit to Graphics from being **committed to
the active game state** even if the UI's custom list still allows
clicking and highlighting other entries. It does **not** lock the
visible dropdown widgets, grey them out, or replace their text with
the actual monitor resolution: those remaining native UI changes
are still pending exact widget control/render identification.
No arbitrary mouse-zone blocks, mutation of the on-disk EXE, or
replacement of the game's full settings table are used.

Please test under Wine (1) trying Graphics alternatives then applying,
(2) Resolution alternatives then applying, (3) a different Video
setting such as brightness/detail to confirm other options still
commit, and (4) entering gameplay. Continue using the validated
v0.1.69 offscreen version-label baseline. PR #9 remains draft
and unmerged.

## v0.1.75 — discover the **pending** Graphics / Resolution values (read-only)

The owner-supplied v0.1.74 Wine/GE-Proton log verifies that our
pass-through `gosCallback` observer activates and records commands
**13, 14 and 15**. Command 15 is the two user-tested Resolution
requests; both remain blocked by the already-validated strict backend
mode guard. Command 13 runs before the requests, and 14 follows them.

Re-examination of the accepted unmodified v1.02 x86 image exposes a
**second, UI-editable video preferences structure**:

- **Active** settings start at `0x0064B450`. Within them,
  `active_device=0x0064B490` and `active_mode=0x0064B48C`.
- The **pending** menu settings start at `0x00648330`. Within them,
  `pending_device=0x00648370` and `pending_mode=0x0064836C`.
- `gosCallback` command **13** calls `0x00418D30`, then copies
  `0x81` DWORDs from active to pending (`0x004077B6–0x004077C5`).
- Command **14** copies `0x81` DWORDs from pending to active,
  then calls `0x00419160` to apply preferences
  (`0x004077D9–0x004077EF`).
- A separate interpreter function **reads and writes** both pending
  indices: `0x0040CCE8 / 0x0040CD64` access the pending device,
  and `0x0040CCF2 / 0x0040CD6E` the pending mode.

This explains why a `Graphics` selection could affect the editable
menu list without changing `active_device` until Apply.

The new v0.1.75 trace checks the **exact original pending-variable read
instruction bytes** before accessing these globals, then logs only
distinct `active_device/active_mode/pending_device/pending_mode`
tuples during the 800×600 (or 640×480) virtual UI. The existing GOS
event trace also records those pairs before/after commands 13, 14, 15,
limited to 80 events. All reads remain **strictly passive**: no
writes to the executable, mode tables or pending settings.

Expected lines:
`MENU-VIDEO-PROBE: ... active_device=N active_mode_index=N pending_device=N pending_mode_index=N`
and
`MENU-GOS-EVENT: command=13 stage=prepare-preferences ...`,
`command=14 stage=apply-preferences ...`.

**Still pending:** the actual visually disabled `Graphics` and
`Resolution` GUI controls, and showing the current monitor's desktop
dimensions in the Resolution field. Neither is implemented by this
read-only build. This new evidence is needed before considering a
widget-specific setter hook: simply overriding the active device
would not guard the pending menu state, and a legacy mode index is
not an actual desktop width/height.

Keep the owner-validated v0.1.69 flicker-free version label and
the confirmed v0.1.73 resolution backend lock. PR #9 remains draft;
no merges before the Video GUI integration is complete.

## v0.1.74 — original Video UI callback research (read-only)

The owner-provided v0.1.73 Wine log **validates** the narrow backend
resolution-mode guard: commands `request=0` and `request=1` both
report `blocked=true changed=false index_before=0 index_after=0`,
while 1920×1080 3D and the validated offscreen MainMenu version label
continue to function.

The remaining requests concern the game's **custom Video GUI widgets**,
not the internal mode table: visually disabling `Graphics` and
`Resolution`, and displaying the active monitor's desktop dimensions
inside the latter. The exact widget IDs/events are not yet known, so
we must not infer them from generic mouse coordinates.

This experimental build adds a separate, **read-only and bounded**
observer of the original GOS script callback registration at
`0x004075D6`. This location contains the exact instruction
`PUSH 0x00407670`, where `0x00407670` is the original
`gosCallback` dispatcher. After the strict EXE hash gate, and only
when `ExperimentalWindowedAdapter=1` plus `[Debug] Log=1`,
the DLL redirects the *registration pointer* to a pass-through
wrapper that always calls the original dispatcher, preserving its
arguments, return value and side effects. There are **no new
selection changes, input filters, game-file edits or GUI overlays**.

When registered early enough and the virtual 4:3 menu is active,
relevant commands 8–30 can produce up to 80 diagnostic lines:
`MENU-GOS-EVENT: command=N result=R device=A->B
mode_index=C->D ui_4by3=true [pass-through]`, including both
read-only variables before and after the original callback. The observer explicitly
cannot affect already-registered callbacks, so if no such lines
appear after Video menu interaction, we should not assume no commands
were sent; that means this registration timing approach needs review.
Exact original instruction bytes and page protections are checked,
and installation failure is logged with a recovery attempt.

**Testing:** with `Log=1`, visit Preferences > Video. Open and
operate the `Graphics` and `Resolution` selectors separately,
then adjust another Video option for a control comparison. Share
the resulting log. The proven v0.1.73 resolution *backend lock*
remains enabled. Neither dropdown is yet visually disabled and
`Resolution` still shows the game's old label. This is diagnostic,
not a claim that those UI requests are complete.

PR #9 stays draft. The 0.1.69 validated menu branding and native
FOV/HUD widescreen remain unchanged.

## v0.1.73 — narrow in-menu resolution backend guard (experimental)

The owner-tested v0.1.72 log proves that two `gosCallback`
command-15 selections reach our authenticated call-site observer:
`request=0` changes the game's legacy mode-table index **0 → 1**,
then `request=1` changes it **1 → 0**, while the 1920×1080 desktop
and 800×600 virtual UI remain unchanged. The exact original event
interception is therefore verified under GE-Proton.

In v0.1.73, the hook now **returns without invoking the original mode
selection handler** only if *all* of the following are true:

- The strict SHA-256 gate accepted the sole supported clean v1.02 EXE
  and the original `CALL rel32` preimage at `0x00407811` matched.
- `ExperimentalWindowedAdapter=1`,
  `Native3DWidescreen=1` and `DesktopResolution=1`.
- The verified MainMenu script has already been entered.
- The live virtual primary Flip is rendering a 640×480 or 800×600
  logical UI frame, not native-resolution gameplay.

In this specific menu context, command 15 leaves the selected
legacy mode index unchanged and the caller still copies its existing
valid resolution table. Otherwise, the original handler and all of
its effects run unmodified. **No game EXE or preference files are
written on disk**, no arbitrary keyboard/mouse interception is used.

A successful menu-only blocked request is logged as
`MENU-VIDEO-EVENT: request=N index_before=X index_after=X
blocked=true changed=false`. The original handler should still
run in non-menu contexts. An additional unit test enforces the
all-conditions-required logic.

**Important limitation:** this is a **backend value lock only**. The
game's custom `Resolution` dropdown can still react visually to
clicks, and its displayed text is not yet replaced with the actual
monitor's resolution. The separate `Graphics` dropdown is likewise
not disabled. Do not claim the full Video menu UX is fixed until
these original custom widget controls and labels are verified.

Compare with the owner-validated v0.1.69 menu signature and 3D/HUD
baseline. Keep PR #9 as draft and do not merge before testing
under Wine and Windows.

## v0.1.72 — exact Video selection event trace (read-only)

The owner's v0.1.71 log demonstrates that the video-state probe is
working, but only reports `selected_device=0, selected_mode_index=0`
before the menu loads, then `selected_device=1,
selected_mode_index=0` after MainMenu loads. No distinct mode-index
change occurs in the recorded period. Without the actual callback
invocation, this does **not** establish whether the user clicked or
opened a Resolution selector, requested its current value, or never
issued a mode-changing selection.

The supported v1.02 EXE has an original, byte-exact `CALL rel32`
at `0x00407811` targeting the game `0x00419420` callback; the
dispatcher maps original `gosCallback` command 15 to this branch.
This build installs an **opt-in, pass-through** observer for only
that call, **after SHA-256 authentication** and only with
`ExperimentalWindowedAdapter=1` and `[Debug] Log=1`.

The wrapper forwards every request to the real legacy mode handler
with exactly the same argument and result, then logs requested mode
and the original mode-table index before/after:
`MENU-VIDEO-EVENT: gosCallback video-mode request=...
index_before=... index_after=... changed=...`. There is a bounded
48-event ceiling, strict original x86 preimage validation,
memory-protection restoration and error logging. The event monitor
does **not** disable either dropdown or change resolution modes;
it can establish whether the resolution selection event actually
fires even if the recorded index remains unchanged. The prior
`MENU-VIDEO-PROBE` on 4:3 menu frames also remains in place.

**Test:** enter Preferences > Video, open and select entries from
`Graphics` and `Resolution`, then return to MainMenu. Compare
`MENU-VIDEO-EVENT` with `MENU-VIDEO-PROBE` lines. Do not confuse
a mode-table index (0, 1, etc.) with the selected monitor's
1920x1080 physical dimensions. No click-zone guessing or game
resource modifications are introduced.

**Still pending:** truly disabled game UI widget behavior and visible
native-monitor `Resolution` label, until the original event/widget
identities are verified. All validated v0.1.69 menu-branding and
native 3D/FOV/HUD rendering is intentionally unchanged.

## v0.1.71 — fix the silent Preferences video diagnostic (read-only)

The owner-supplied v0.1.70 log shows the game rendering the correct
1920x1080 desktop and virtual 800x600 4:3 MainMenu, with a successful
flicker-free offscreen version label. **No `entered Preferences` or
`MENU-VIDEO-PROBE` entries appear in that log.** The probe was
incorrectly conditional on our narrow four-call script observer
recognizing `Preferences`; not every nested menu has that transition.
The lack of probe lines does *not* establish that the user did not
interact with video settings.

In v0.1.71, the exact authenticated game image is still required and
its original device/index load instructions must still match their
byte-exact preimages before any memory read. However, the read-only
probe now samples only the game's **640x480 or 800x600 logical UI
Flip** (when `[Debug] Log=1`), without requiring any particular script
scene tag. It logs the *first sample and only distinct device/index
changes*, capped at 32 events; it never writes to the game memory.
`scene=MainMenu-or-unobserved-submenu` is a diagnostic *hint*, not
proof that the MainMenu is currently visible.

Search the log for
`MENU-VIDEO-PROBE: ui=800x600 scene=... selected_device=...
selected_mode_index=...`. Compare before/after choosing the `Graphics`
and `Resolution` lists. If no variable changes appear, it may be
necessary to trace the verified `0x00407811` resolution callback in
the UI interpreter rather than inferring anything from snapshots.

**This is a diagnostic build, not the promised disabled dropdowns or
desktop resolution label.** The validated 0.1.69 MainMenu version
watermark, aspect-correct 4:3 movie/UI and native 3D resolution/FOV/HUD
are unchanged. PR #9 remains draft and should not be merged until
the actual menu Video widgets are implemented and visually verified.

## v0.1.70 — Video Preferences controls: exact-state research (read-only)

The *working v0.1.69 offscreen MainMenu signature* is preserved as the
visual reference. The video controls are **not yet locked** in this
build, nor does the Resolution dropdown show desktop dimensions.

Static analysis of the **single accepted clean v1.02 EXE** identifies
two independently stored GUI backing values:

- `nSelectedVideoDevice` is registered as a GOS script variable at
  `0x64B490`, with an exact x86 read instruction at `0x419208`.
- The game-selected resolution/mode **index** is at `0x64B48C`, read
  from `0x419235`. This is a legacy mode-table index, **not** the
  selected monitor's width/height. The available list still comes
  from the original game UI.

With `ExperimentalWindowedAdapter=1` and `[Debug] Log=1`,
`menu_video_probe.rs` observes these values in `Preferences`
during a game-owned virtual Flip. It validates both original x86
instruction preimages, limits transition logging, and never writes to
those values. Look for:
`MENU-VIDEO-PROBE: Preferences selected_device=... selected_mode_index=...`
when entering Video and changing either dropdown.

A companion script (`tools/audit_video_menu.py`) searches the user's
own installed game folder for relevant GOS UI filenames and bounded
text/resource markers. It refuses an unrecognized EXE, reports only
**relative** paths plus hashes/short text contexts, and **does not**
copy, redistribute or change game files. On the user's own PC:

```sh
python CrimsonSkiesFix/tools/audit_video_menu.py "/path/to/Crimson Skies" \
  -o video-ui-audit.json
```

Use both the diagnostic log and resource audit to locate **exact**
`Graphics` and `Resolution` control IDs and their callback path.
Then implement a widget-specific disabled state, current-monitor
resolution label and fail-closed tests. No global mouse blocking,
screen-coordinate guessing, or alteration of game-owned mode lists
is permitted. Do not merge until all controls are functional and
verified under Wine and Windows.

## v0.1.69 — atomic offscreen menu signature composition (experimental)

The owner confirmed the v0.1.68 signature **is visible but flickers slightly**
while an animated background video plays behind the game's MainMenu.
The runtime log confirms MainMenu detection, successful primary-surface
`DrawTextW` and continuing virtual primary Flip calls. This is
consistent with interleaved movie updates and drawing directly on the
already-visible DirectDraw primary.

In v0.1.69 the DLL owns a fresh **private, RGB565 system-memory offscreen
scratch** for the game's 640×480/800×600 menu epoch. On every MainMenu
Flip, it copies the complete unchanged native backbuffer to the scratch,
draws `CrimsonSkiesFix VERSION-SHORTCOMMIT` via the scratch's DirectDraw4
`GetDC`/`ReleaseDC`, and then displays the **composited picture in a
single normal primary Blt**. In all other scenes, or whenever the scratch
copy or text draw fails, the original real backbuffer is presented
unchanged. The game's own 3D render target and video decoder are never
overwritten; no independent HWND or visible-primary GDI overlay is used.
The menu signature stays in the lower right of the aspect-correct 4:3
viewport.

The new diagnostic is
`MENU-BRANDING: offscreen RGB565 menu text composited before primary Blt`.
Any scratch-copy/GetDC/ReleaseDC error is logged once and bypasses
the overlay. **No runtime flicker-free claim is made until owner
screenshots/video under Wine confirm it.** The 60 FPS Flip pacing,
resolution/FOV and strict EXE SHA-256 policy remain unchanged.

The separate requests to disable `Graphics` and `Resolution` in
Preferences > Video and to display the selected desktop resolution in
that menu remain pending. Do not merge this experimental PR yet.

## v0.1.68 — target the DirectDraw primary (menu-label visibility regression)

On Wine/GE-Proton, v0.1.67 logged `MENU-SCENE: entered MainMenu` and
`MENU-BRANDING: draw requested on active MainMenu`, but the screenshot
showed **no project label**. `DrawTextW(GetDC(HWND))` succeeded according
to Win32, yet its pixels were not necessarily composited with the DirectDraw
primary. The display framebuffer is still owned by DirectDraw.

The new experiment draws the signature into the actual live
`IDirectDrawSurface4` primary surface via its `GetDC` and `ReleaseDC`
COM methods **after** the real windowed primary Blt completes. The
label is placed inside the aspect-correct viewport (the 4:3 game image,
not the black side pillars), near the lower-right margin. The label
is attempted only when `MainMenu` is active, and the game EXE and
original rendering pipeline are unchanged. Errors return without
interrupting normal presentation or game input.

The log now distinguishes `MENU-BRANDING: DirectDraw4 primary Surface4
text submitted` from `MENU-BRANDING: primary Surface4 drawing unavailable:
IDirectDrawSurface4::GetDC hr=...`. Even success is only proof of a DC
draw submission; **test visually under Wine**. The verified transition
to MainMenu was already working in v0.1.67.

The `Graphics` and `Resolution` widgets of Preferences > Video
remain separate, unfinished work.

## v0.1.67 — main-menu-only version label experiment

The exact verified v1.02 EXE has four original `CALL rel32` instructions
that load game menus. Only after the executable passes the strict SHA-256
gate, a byte-exact in-memory hook observes those calls and recognizes the
`MainMenu` and `Preferences` script pointers. A mismatch disables the
menu observer entirely. The original script loader is always called
unchanged, and the game EXE on disk is never modified.

With `ExperimentalWindowedAdapter=1`, after a successful windowed
DirectDraw primary Flip, the DLL requests a transparent Win32 text draw
in the lower-right corner of the game client area **only if the MainMenu
script is active**: `CrimsonSkiesFix VERSION-SHORTCOMMIT`. The version
and eight-character commit are embedded from the actual CI source head.
This is an experiment: GDI/DirectDraw compositing varies between Wine and
Windows, and a successful draw call does not prove the label is visible.
The game UI, textures, mouse input and frame pacing are not modified.

Look for `MENU-SCENE: hook_available=true`, `MENU-SCENE: entered MainMenu`
and `MENU-BRANDING: draw requested on active MainMenu` in the log.
If the label is absent, flickers, or appears in a different game screen,
send that log along with a screenshot. If the script-hook checks fail,
the label remains disabled without affecting the game's menu.

**Still pending:** prevent selection of the `Graphics` and
`Resolution` custom drop-down lists and replace the visible Resolution
value with the selected monitor's physical dimensions. The original game
`Preferences` menu loads external UI resources, rather than standard
Win32 combo boxes; the project does not yet identify those exact widget
structures/events. The monitor dimensions themselves are already
available (logged at startup). Do not claim that the in-menu settings
are currently locked or rewritten. Both changes need a separate verified
widget-specific implementation before merging.

## v0.1.66 — monitor launch selection and build identity (experimental)

The DLL now captures the monitor under the pointer on its **first DirectDraw export** and freezes that selection; it does not follow later cursor movement. For the existing opt-in windowed adapter, the game-owned HWND is validated before being moved to that monitor's **actual virtual desktop origin**, including nonzero and negative coordinates. Render-buffer sizes and client geometry use `right-left` and `bottom-top`, not absolute `right` and `bottom`. The early FOV/HUD patch reads the same frozen monitor's dimensions, and later DirectDraw4 negotiation reuses them. This replaces the previous primary-screen (0,0) assumption.

This captures the pointer's monitor at DirectDraw startup, **not an OS-provided "monitor that launched the process"** (Windows has no general equivalent). Test with displays on either side of the primary, including negative coordinates. The windowed 4:3 menus and videos still retain their logical 640x480/800x600 rendering and centered aspect-correct presentation.

The build now embeds `CrimsonSkiesFix VERSION-SHORTCOMMIT` (actual source head commit in CI) and logs it during initialization. **Drawing this text only on the main menu is not implemented yet.** The game uses its own script-driven custom widgets, and the native video options for `Graphics` and `Resolution` likewise still need a verified GUI event hook to safely lock their lists and replace the visible resolution with the selected monitor dimensions. Their existing settings remain untouched in this experimental build; do not claim them disabled.

**Status:** Windows x86 compile/unit tests and real multi-monitor Wine/Windows runtime tests are separate gates. No merge until the user validates the new monitor placement and no regression in 4:3 videos or gameplay.

## v0.1.65 — experimental early native FOV/HUD initialization

**Motivation:** owner screenshots on v0.1.64 show visibly horizontal-oval round HUD gauges and disproportionate cinematic bars at native 1920x1080, despite a successful exact in-memory reference delta and a 1:1 1920x1080 RGB565/Z32 rendering epoch. This is a visual regression requiring validation, not evidence of success.

**Hypothesis:** previously, the supported prepatched reference EXE had all widescreen instructions active from process startup. The new exact clean-EXE memory patch was only applied during the later game-owned DirectDraw4 exclusive cooperative request, after earlier game initialization could have cached 4:3 UI/projection values.

**Controlled experiment:** with both `ExperimentalWindowedAdapter=1` and `Native3DWidescreen=1`, the first exported DirectDraw call now applies the *same strict, preimage-checked, full FOV/HUD memory delta* before entering the system DirectDraw code, but **only when Windows reports exactly one monitor with valid primary geometry**. No file is modified, no patch runs in `DllMain`, and all existing byte/PE checks, protection restoration and rollback remain in place. The later game HWND-specific stage reuses the result if its monitor matches. A mismatch fails closed. Multi-monitor setups retain the old deferred HWND path; no guessing based on cursor location.

The test log should show `WINDOWED-WIDE: early DirectDraw native FOV/HUD patch monitor=1920x1080 result=Patched`, followed later by `reused early native FOV/HUD patch`. Compare **the same cutscene**, three circular HUD gauges, and the 3D aircraft proportions against v0.1.64. This is a timing hypothesis, not a proven proportions fix. Return to v0.1.64 if the regression persists or worsens. Do not merge before owner runtime validation.

## Supported game executable — strict check (v0.1.65)

**Only the exact verified, unmodified SafeDisc-free Crimson Skies v1.02 executable is accepted.** The original
SafeDisc-wrapped retail launcher, older versions, and third-party
already-widescreen/FOV-patched EXEs are **not supported**.

| Required file | Exact value |
|---|---|
| Game EXE | `CRIMSON.EXE` |
| Size | `2,580,480` bytes |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

Before installing any graphics hooks or changing game memory, the DLL
calculates the **raw SHA-256 of the executable on disk**. Any different
hash (or an unreadable file) is **refused**: the DLL shows a concise
Windows error dialog with the expected and detected SHA-256 values.
Once the user clicks OK, the game process exits immediately rather than
continuing into its misleading DirectX initialization error. The error is written beside `ddraw.dll` to
`CrimsonSkiesFix.log` **even with `[Debug] Log=0`** (as long as
the directory is writable). The refusal never overwrites or modifies
the game EXE. Unknown executables do not fall back to the
normal/unmodified DirectDraw implementation through this proxy.

Our DLL is intended to **adapt the verified executable in memory** to
the active monitor's resolution, camera FOV and HUD, retaining 4:3 for
menus and videos. This is experimental, controlled by
`ExperimentalWindowedAdapter=1` and `Native3DWidescreen=1`;
**native 3D gameplay with the full automatic patch is not yet validated**.
The EXE on disk keeps the exact SHA-256 above.

To check the game executable yourself:

```sh
sha256sum CRIMSON.EXE
```

Windows PowerShell:

```powershell
Get-FileHash .\CRIMSON.EXE -Algorithm SHA256
```

## Installation — only one file at the game root

Extract the Windows x86 artifact into **a copy** of the game directory
containing the exact verified unmodified Crimson Skies 1.0.2.0 EXE above. The intended
layout is:

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log           # created for Log=1, adapter mode, or EXE rejection
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
ExperimentalWindowedAdapter=0
FocusRepaint=0
WindowEraseGuard=0
LegacyOffscreenRGB565=0
Native3DWidescreen=0

[Debug]
Log=0
```

- `Borderless=1`: legacy DirectDraw v1 behavior. On its own, this does
  **not** convert the game's DirectDraw4 flip chain to borderless.
  The experimental DirectDraw4 integration is controlled separately by
  `ExperimentalWindowedAdapter` (off by default).
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
- `ExperimentalWindowedAdapter=0`: experimental DirectDraw4 windowed
  3D surface adapter; enabling `1` keeps the actual 1920x1080 desktop
  mode while the game sees its own 640x480/800x600 RGB565 rendering
  mode. In v0.1.48, `GetDisplayMode` also reports that logical mode
  and RGB565 backbuffer/Z32 `Lock` forwards to its equivalent native
  surface. This remains **not gameplay-validated**: the first real
  v0.1.47 run reached successful `D3D3::CreateDevice` but the game
  reported `Failed to initialize video` before its first 3D scene.
  v0.1.48 adds bounded diagnostics on rejected virtual COM operations.\n  In v0.1.49, the virtual Primary surface additionally uses a separate,\n  preflight-verified RGB565 staging surface for its CPU `Lock`/`Unlock`: the\n  1920x1080 desktop primary is **never exposed** as a lockable 640x480 buffer.\n  This specifically targets the observed v0.1.48 game error\n  `WINDOWED-GAME: virtual Surface4 Lock physical desktop primary refused`.\n  v0.1.50 additionally acknowledges an identical repeated game DD4\n  exclusive cooperative request after the graph was exposed, without\n  changing native DDSCL_NORMAL or the live surfaces. Altered HWND,\n  interface, flags or window geometry remain rejected. This addresses\n  the next v0.1.49 log failure (`0x80004005` on repeated 0x13).\n  v0.1.51 permits game-supplied native source-copy Blts into the\n  same-size real RGB565 backbuffer (with COM facade source unwrapping,\n  epoch/role/flag validation); this addresses repeated `0x80004001`\n  backbuffer source-copy failures in the v0.1.50 game log. The Flip\n  bridge now clears the black pillarbox once per resolution epoch, not\n  ahead of every frame, to reduce compositor-visible black flicker.\n  Repeated 640x480/800x600 game-requested reinitializations still occur\n  and actual movie decoding/presentation remains unverified.\n  v0.1.52 paces the actual virtual Primary Flip using FPSLimit\n  (the previous passive ModernFPSPacing hook did not intercept it).\n  It also logs the first four real Blt source pixel formats and\n  otherwise invisible Flip presentation failures. With v0.1.51,\n  videos returned and flicker ceased, but the first logo screen showed\n  magenta/green color corruption, the window could turn white after\n  another window covered it, and a later crash was reported with no\n  identifiable error in the final log. Their causes remain under investigation;\n  do not treat v0.1.52 as a verified fix for color, focus, or crashes.\n  v0.1.53 adds optional `CrashBltTrace=1` (requires `Log=1`):\n  each Wine native Surface::Blt entry/exit is logged with sequential ID,\n  thread, COM source/destination addresses, flags and HRESULT (up to 20,000\n  operations). This traces the v0.1.52 reproducible native crash\n  `0xc0000005` at Wine builtin `ddraw+0x3B619` (null+0x280),\n  where the actual proxy frame is the native `Blt` forwarding hook.\n  v0.1.54 fixes the v0.1.53 fixed 20,000-call diagnostic cutoff:\n  the latest 2,048 native Blt ENTER/EXIT pairs are kept continuously\n  in the separate `CrimsonSkiesFix-BltTrace.log` beside ddraw.dll.\n  The original `CrimsonSkiesFix.log` remains small, including mode epochs.\n  No pixel-format/graphics semantics were modified in v0.1.54.\n  v0.1.55 addresses a possible COM boundary violation discovered after\n  the next live crash: native Wine ddraw Blt was called with\n  dest=0xc803a6c and src=0x132f820 (wait-only), then crashed at\n  builtin ddraw+0x3b619 on NULL+0x280 before returning.\n  A synthetic Surface4 from our experimental graph must not be passed\n  to a native Wine surface's Blt: Wine casts its source as its own\n  internal DirectDraw surface. Native-destination Blt now unwraps\n  recognized, non-retired virtual Surface4 sources: virtual primary\n  uses logical RGB565 Lock/Unlock staging, backbuffer uses real RGB565,\n  depth/retired/unready virtual surfaces fail closed, foreign native\n  sources are forwarded unchanged. The separate rolling trace is kept\n  to confirm whether this specific crash is fixed; the fatal call's\n  source has not yet been independently proven synthetic, so this is\n  a candidate fix requiring live validation. Disable CrashBltTrace\n  after recording a failing run.\n  Direct primary writes are not yet mirrored to the display without a Flip,\n  and full gameplay remains unverified.
  Turn this option back to `0` to return to the previous exclusive
  rendering path if initialization fails.
- `[Debug] CrashBltTrace=0`: default-off verbose trace of each native DirectDraw Blt\n  just before and after the call. Set `1` only for a controlled crash\n  reproduction with `Log=1`, then reset to `0`. v0.1.54 stores\n  the most recent 2,048 native Blt calls in a rolling separate file\n  `CrimsonSkiesFix-BltTrace.log` next to `ddraw.dll` (it is overwritten\n  on each launch, rotated in-place every 2,048 Blts, and remains small).\n  It does not stop recording during lengthy gameplay but may slow the\n  game. An ENTER without its matching EXIT on the crashing Wine TID\n  identifies the last in-flight Blt (not necessarily the invalidation).\n- `[Debug] Log=0`: logging is **fully disabled by default**; no
  log file is created or modified. Set `Log=1` to create/overwrite
  `CrimsonSkiesFix.log` at the next game startup.

## Experimental Win32 background erase guard — v0.1.57

A real v0.1.56 run still showed a **completely white 1920x1080 game
window while the terminal was foreground**, even though `Flip` reached
960 and Wine returned successful DirectDraw HRESULTs. FocusRepaint
logged just one full refresh on frame 1 of graphics epoch 2 and no
later refresh: success at the DirectDraw API does not demonstrate that
the displayed HWND retained the frame. Wine's
`WINED3D_TEXF_LINEAR not supported in software blit` FIXME reports
an unsupported *filter* in a CPU blit, not evidence of the white-window
cause.

A separate, default-off `[Graphics] WindowEraseGuard=0` experiment
can be enabled with `WindowEraseGuard=1` and
`ExperimentalWindowedAdapter=1`. It installs a restricted Win32
WndProc subclass ONLY on the verified process-owned game HWND once
the complete DD4 adapter has initialized. It returns 1 only for
`WM_ERASEBKGND` to prevent Windows/Wine from filling the game
background over DirectDraw pixels. It transparently forwards all
other messages (including WM_PAINT, WM_ACTIVATE and input) to the
original window procedure; on WM_NCDESTROY it restores the original
procedure when still installed. No DirectDraw or logging calls occur
inside the WndProc callback. Debug counters (suppressed WM_ERASEBKGND,
observed WM_PAINT) are reported by the ordinary render-thread
`Flip` code when `Log=1`.

**Wine/GE-Proton validation (31 Oct 2026):** Two screenshots show the
Crimson Skies main menu and hangar/mission menu staying visible behind
an overlapping terminal, unlike the entirely white v0.1.56 window.
The attached v0.1.57 game log confirms that the guard was installed,
that **7 WM_ERASEBKGND messages were suppressed** and 7 WM_PAINT
messages observed, with at least 720 successful virtual-primary Flips
and no reported crash. This is strong practical evidence that the
Win32 background erase contributed to the white-window symptom, but
it does not exclude other contributing presentation issues or verify
native-Windows behavior. The menu background video appears paused
while the game lacks focus; the guard intentionally preserves
WM_ACTIVATE and all other normal messages, so game-controlled focus
suspension remains plausible, not an established regression. If no WM_ERASEBKGND events are
reported, or the screen remains white with the guard enabled, the
investigation must shift to Wine frontbuffer presentation, compositor
or game WM_PAINT. The optional guard does not change pixel format,
native COM source validation, crash protection or the anti-flicker
one-time clear.

For a controlled A/B run: with exactly the same settings, first
reproduce with `WindowEraseGuard=0`, then set `WindowEraseGuard=1`
(leave `FocusRepaint=1`, `[Debug] Log=1`, and
`CrashBltTrace=0`). Cover/uncover, Alt+Tab, minimize/restore from
menu and 3D mission. Record visual results and supply
`CrimsonSkiesFix.log`. The two repaint/guard switches are independent.
If any regression occurs, set `WindowEraseGuard=0` immediately.
Do not classify repeated `surface_cpu_blt` filter FIXME as a crash.

## Experimental visibility restoration — v0.1.56

The v0.1.55 COM source guard successfully ran during gameplay and the former
Wine native `Blt` access violation has **not** been reproduced; it is not
proven impossible. Keep that stability fix active.

v0.1.56 adds an independent **default-off** `[Graphics] FocusRepaint=0`
experiment, effective only while `ExperimentalWindowedAdapter=1`. Set
`FocusRepaint=1` to test a white or blank game surface after Alt+Tab,
restoring the game window, or exposure/invalidation. On each virtual
primary `Flip` it makes read-only Win32 queries of the verified game HWND
(foreground status and pending paint region). After a witnessed
foreground loss/return, or the **leading edge** of a new pending Win32
paint region, the next Flip performs exactly one complete black-clear
and RGB565 4:3 viewport stretch. Subsequent Flips remain viewport-only,
preserving v0.1.51's no-per-frame-clear anti-flicker change.

No WndProc subclassing, Win32 painting/erasing, background thread,
DD4 resource reconstruction, or game executable patch is involved. It
is deliberately not claimed as a verified fix: if the game stops
flipping while occluded and has no pending paint region when resumed,
the transition may not be observed. Native Windows and Wine results
must be tested. On regression set `FocusRepaint=0` to return to the
v0.1.55 presentation behavior without changing any other setting.

**Test plan:** with the same game and settings, first test
`FocusRepaint=0`, then `FocusRepaint=1`. During both menu and a 3D
mission, Alt+Tab to an overlapping window, cover/uncover the game,
minimize/restore and return to the game. Check for recovered pixels,
unwanted full-screen black flashes, sustained white, lost focus and
crashes. When enabled with `[Debug] Log=1`, successful one-shot events
are logged as `WINDOWED-GAME: FocusRepaint one-shot full repaint succeeded`.
Leave `CrashBltTrace=0` for ordinary focus testing unless a native
crash returns.

The separate pink/green 32-bit source-to-RGB565 conversion problem is
**not changed** by v0.1.56.

## Historical v0.1.62 experiment — superseded by v0.1.63

The v0.1.62 experiment used a separately supplied, already-widescreen-patched
EXE to identify the shared width/height fields and FOV/HUD transformations.
**That executable is no longer supported. Do not install or test the old
prepatched EXE with the current DLL.**

Since v0.1.63, the only accepted on-disk executable is the **clean, unmodified
SafeDisc-free v1.02 EXE** (2,580,480 bytes; SHA-256
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`).
The DLL checks this hash before any graphics hooking and, if opted in, attempts
the resolution/FOV/HUD changes **in memory** on that exact image. Use the
[current executable requirements](#supported-game-executable--strict-check-v0163)
and current experimental settings above; in-game widescreen/FOV behavior remains
subject to user validation. The full historical v0.1.62 investigation remains
available in Git history, but its installation instructions are obsolete.

## v0.1.61 — targeted correction of the actual pink/green source image

**Wine gameplay validation — 31 October 2026:** The owner tested v0.1.61
with `LegacyOffscreenRGB565=1` under Wine 11.0/GE-Proton11-11,
and supplied `image(20261031-035251).png` and matching
`CrimsonSkiesFix(20261031-035259).log`. The screenshot now shows
the FULL-WIDTH original Crimson Skies splash artwork, accurately red/gold
logo, correctly colored gray dirigible, intact text, no pink/green
speckling, and the correct 4:3 picture within a 1920x1080 desktop.
The DLL log independently confirms the exact native game-owned
800x600 RGB565 source creation succeeded (`hr=0x00000000`), the
subsequent source has 16-bit RGB565 masks, while the 320x240 video
source remains 32-bit XRGB; the virtual Primary Flip succeeds through
at least frame 600 with no fatal DirectDraw errors reported.
**This validates the specific intro-logo color correction on Wine/
GE-Proton**, not indefinite game stability, native Windows parity, or
every later game/movie surface. Retain the default-off switch until
broader gameplay and native Windows testing. No further color-channel
transformation is required for this tested image.



**Root cause strongly supported by the three owner-supplied v0.1.60 BMP
captures:** `Copy1-Source32.bmp` shows the Crimson Skies intro logo,
dirigible and surrounding artwork with neon pink/green corruption,
occupying ONLY x=0..399 of its nominal 800-pixel wide 32-bit surface,
while the bottom ends at scanline 500. The preexisting error is
**already present in the native XRGB8888 source before any Blt**.
Reinterpreting each XRGB32 DWORD's **low 16 bits as RGB565** recovers
the correct original red/gold logo and grayscale airship, squeezed
to half horizontal width because each DWORD contains two packed
RGB565 pixels. The BMP24 capture discards the DWORD's top byte, so
this reconstruction recovers only one of the two source pixels.
The observed first-source image appears to contain 800 native
16-bit horizontal pixels stored on each 800x600 native
XRGB32 row, hence occupying 400 of 800 DWORD slots.

The game creates this specific native DD4 surface with
`dwFlags=0x00110007` (CAPS|HEIGHT|WIDTH|CKSRCBLT|TEXTURESTAGE),
`dwCaps=0x00000840` (OFFSCREENPLAIN|SYSTEMMEMORY),
logical dimensions 800x600 and **NO DDSD_PIXELFORMAT**. The physical
desktop remains 32-bit under `DDSCL_NORMAL`, so Wine assigns
XRGB8888 to the game-owned system-memory surface even though the
classic 16-bit game populates it with RGB565 pixel pairs. Wine
subsequently faithfully converts those *incorrectly described* RGB32
pixels, explaining the visually corrupted intro. Independently,
the 320x240 video source is correctly described as 32-bit and
matches the RGB565 video destination in all sampled points; DO NOT
change its format.

**Isolated, default-off candidate fix `[Graphics]
LegacyOffscreenRGB565=0`:** With
`ExperimentalWindowedAdapter=1`, opt into `LegacyOffscreenRGB565=1`
to make the original DD4 CreateSurface allocate the *exactly matching*
800x600 game system-memory surface in native RGB565 format rather
than inheriting the physical desktop's 32-bit format. It:
- applies only on the verified game DD4 and active 800x600x16 graph;
- matches EXACT descriptor flags=0x00110007 and caps=0x00000840;
- clones the 124-byte DDSURFACEDESC2, preserving all caller data
  except supplying the missing native DDPIXELFORMAT masks;
- leaves the original caller descriptor, physical desktop, HAL/Z32
  graph, 320x240 movie source, 3D textures and COM boundary untouched;
- logs `WINDOWED-COLOR: LegacyOffscreenRGB565 native image source
  800x600 RGB565 creation hr=0x00000000` on successful creation.

**Not yet validated in game.** Test v0.1.61 with existing verified
`FocusRepaint=1` and `WindowEraseGuard=1`; set
`LegacyOffscreenRGB565=1`, `[Debug] Log=1`,
`ColorProbe=0`, `ColorCapture=0` and view the original
pink/green intro. Collect a new screenshot and log to confirm the
native 800x600 source is now **16 bpp** and the logo colors/placement
are restored. If a new startup error, video regression or crash
appears, immediately set `LegacyOffscreenRGB565=0` to return to the
old, stable v0.1.60 behavior. Do NOT merge without live validation
and native Windows tests.

## v0.1.60 — actual color conversion verified; locate original pixels

The supplied `CrimsonSkiesFix(20261031-033532).log` (v0.1.59,
Wine 11.0/GE-Proton11-11) contains 45 sampled XRGB8888 source / RGB565
destination pixel pairs: 18 from two 1:1 copies and 27 from 3 scaled
320x240 -> 800x600 movie copies (#3, #60, #120). The old color
diagnostic expected simple **truncation** (`r>>3,g>>2,b>>3`) and
reported 13 apparently bad pixels. Recomputed against **normalized
rounding** (`r5=round(r*31/255), g6=round(g*63/255),
b5=round(b*31/255)`): **45/45 match exactly**, including ALL 27
samples of scaled video. Thus the old `mismatches` were a **false
diagnostic alarm** caused by the tool's expected formula, not evidence
of bad Wine conversion. The underlying video/intro pink-green problem
remains unproven and unresolved; a correct source/dest pixel match
cannot prove video source artwork is valid.

The `ColorProbe` expectation has now been corrected to normalized
rounding, with regression tests from the real log. To find where the
visible corruption originates, a separate **default-off** `[Debug]
ColorCapture=0` (requires `Log=1`) creates just THREE snapshots
after the selected successful native Blts:

- `CrimsonSkiesFix-Color-<PID>-Copy1-Source32.bmp`: first full
  800x600 XRGB32 source before its 1:1 copy;
- `CrimsonSkiesFix-Color-<PID>-Copy3-Source32.bmp`: first 320x240
  XRGB32 video source before its stretch to RGB565;
- `CrimsonSkiesFix-Color-<PID>-Copy3-Dest565.bmp`: 800x600 RGB565
  backbuffer after that stretch, converted to 24-bit BMP for viewing.

Capture takes read-only native DirectDraw locks and copies data into
owned memory. File output uses a bounded standard BMP encoder,
allowlisted deterministic labels, a PID suffix and create-new
semantics beside the **loaded** game-local `ddraw.dll`: no overwriting
other files or following preexisting symlinks. The game pixel buffer
is not written. This is a *temporary and more intrusive*
diagnostic—GPU readback and disk I/O may stall the game briefly; do
not enable it for normal playing.

Run once with existing graphics options (WindowEraseGuard=1,
FocusRepaint=1, ExperimentalWindowedAdapter=1), `Log=1`,
`ColorProbe=1`, `ColorCapture=1`, `CrashBltTrace=0`. After the
suspect intro/splash/movie, upload the resulting BMP files and
`CrimsonSkiesFix.log`. If the raw XRGB32 image is already
pink/green, look upstream at its content/decoder or earlier
lock/writes. If raw source is correct but stretched target is not,
inspect which native transfer/graphics path wrote the target;
if both are correct but screen corrupts, examine primary staging,
presentation or compositor. Do not assume these first captures
necessarily coincide with the exact affected logo/frame; if not,
the diagnostic selection needs adapting. Return both opt-in debug
flags to 0 after capture. **No game rendering changes** in v0.1.60.

## v0.1.59 — check the scaled movie pixels, not just the 1:1 images

The supplied v0.1.58 game log `CrimsonSkiesFix(20261031-031702).log`
proves two native 800x600 XRGB8888→800x600 RGB565 1:1 transfers
produced **9/9 correct sampled pixels each** (`COLOR-PROBE:
copy=1` and `copy=2`, both `mismatches=0`). Critically, the
320x240 source video images copied into the 800x600 logical backbuffer
were NOT checked: v0.1.58 logged `skipped scaled Blt 320x240 ->
800x600` for copies 3, 60 and 120. A successful native Blt is not
proof of faithful scaling or colors.

v0.1.59 keeps the SAME default-off `[Debug] ColorProbe=0` switch
and sampling cadence. For XRGB8888→RGB565 Blts with distinct source
and destination rectangles, it now computes 9 corresponding source
and destination pixel coordinates using the 16.16 fixed-point
nearest-neighbor stepping of Wine 11.0's
`wined3d/surface.c::surface_cpu_blt`. Wine logs that requested
LINEAR filtering is unsupported in its CPU copy path; the actual CPU
scaler uses point/nearest sampling. We only claim this mapping as a
**candidate** until the live samples confirm which Wine path was
used. Coordinates, source XRGB values, expected RGB565 values, and
observed RGB565 pixels are logged as:
`COLOR-PROBE: copy=N mode=scaled-cpu-nearest-candidate match=X/9
mismatches=Y`.

Only copies without color key, alpha, or effect flags qualify;
unexpected formats and readback failures still fail closed.
All native Blt calls remain completely unchanged. Lock/Unlock
readback is still opt-in and may cause brief stalls, so perform the
targeted introduction/movie test and then restore `ColorProbe=0`.
A mismatch may also reflect a different Blt backend, a concurrent
writer, or scaling semantics rather than necessarily a pixel-format
bug; no game rendering correction is claimed.

## v0.1.58 — opt-in intro color diagnosis (no color changes)

The earlier corrupted Crimson Skies title artwork contains strong
magenta/green speckling, not simply swapped red and blue. Game logs
show that real native 32-bit XRGB source surfaces (800x600 and
320x240, masks R=0x00FF0000/G=0x0000FF00/B=0x000000FF)
are copied by IDirectDrawSurface4::Blt into the virtual
game's native **16-bit RGB565** offscreen backbuffer. All Wine native
copies returned S_OK, but a successful HRESULT alone says nothing
about the underlying pixel channels.

New default-off `[Debug] ColorProbe=0` (requires `Log=1`)
samples NINE source and destination pixel pairs **after**
selected successful native source-copy Blts, at copies 1–3 and
every 60 up to 1200. It is strictly limited to matching
one-to-one 32-bit XRGB8888 to 16-bit RGB565 operations, canonical
RGB masks, and valid source/destination rectangles. Scaled copies,
other formats, un-lockable resources, or unexpected pitches are
logged as skipped/unavailable rather than guessed. Sampling uses
temporary native read-only DirectDraw4 Lock/Unlock calls on the
current game thread (no asynchronous workers or additional COM refs);
**it may cause a brief GPU stall and is diagnostic only**. The game
pixels are never intentionally written or transformed; the actual
native Blt remains unchanged. If either Lock fails, the original
successful Blt result is retained.

`COLOR-PROBE: copy=N match=X/9 mismatches=Y` and per-swatch
`xrgb.../expected.../got...` values in `CrimsonSkiesFix.log`
allow us to determine whether Wine converts the current source
RGB channels correctly. If the expected and observed RGB565
values match while the screenshot still looks wrong, the problem
must be upstream of conversion or downstream of this backbuffer,
not in these sampled Blt pixel values. Mismatches suggest a
conversion/filter/presentation issue requiring targeted follow-up;
they do NOT automatically prove Wine is at fault.

**Next test:** enable `ColorProbe=1` and `Log=1`, leave
`ExperimentalWindowedAdapter=1`, `FocusRepaint=1` and
`WindowEraseGuard=1` as previously validated, and retain
`CrashBltTrace=0`. View the title/logo animation with the
corrupt magenta/green pixels, capture a screenshot if it remains
corrupt, then send `CrimsonSkiesFix.log`. Disable `ColorProbe`
again after this targeted diagnostic, as readback of 3D surfaces
is more intrusive than ordinary passive logging. No claim is
made that v0.1.58 fixes the color.

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

## v0.1.42 — actual virtual Surface4::Flip dispatch into real windowed DirectDraw Blts

The owner's v0.1.41 `--com-surface-chain` log is a complete **PASS**: three different real-DirectDraw4-backed virtual Surface4 COM pointers, working Primary→Backbuffer→Z32 GetAttachedSurface references, 640x480 logical formats, native D3D3 HAL on the unwrapped RGB565 backbuffer and identity-preserving GetRenderTarget. The windowed native black clear/stretch succeeds at 1920x1080 without changing the desktop resolution.

Stage B4k introduces an explicit, **default-off, one-time `arm_graph_primary_flip` gate** for the virtual-primary COM object's **actual IDirectDrawSurface4::Flip vtable method**. The graph must include Primary→Backbuffer→Depth, an `AdapterContract` at Phase::Exposed backed by successful physical preflight and exactly matched client size/mode. Its `Flip` method rejects wrong target or flags and never forwards virtual Flip to the native exclusive Flip. When armed, a valid Flip calls the existing fail-closed `FlipToBltBridge` and genuine HWND-clipped primary black clear + scaled RGB565 offscreen Blt. A failed Blt propagates the actual HRESULT and latches the bridge's fault: no silent exclusive retry. The virtual primary retains strong child references; the session is destroyed with the wrapper.

New unit tests validate successful COM-vtable Flip dispatch, 2 acknowledged frames / 4 physical native Blts, refusal before arm or with invalid target/flags, and no native `Flip` calls. The existing default-off and passive forwarding tests remain.

Standalone Windows x86 v0.1.42 adds **`--com-virtual-flip`**. It repeats the complete v0.1.41 live COM graph and D3D3 HAL setup, checks the unarmed Flip refuses, preflights real windowed presentation, explicitly arms the graph and runs **120 virtual COM `Flip` calls** with changing RGB565 source colors and D3D3 HAL scene calls. Every acknowledged virtual frame corresponds to two real windowed Blts. It logs groups of 30, exact HRESULT failure and `COM-VFLIP-FINAL: PASS` only when the sequence completes.

The actual game DLL still has **no caller that arms this virtual graph or replaces its original DirectDraw4 interfaces**. This is not yet an in-game borderless release. Owner should run the new separate standalone probe from an isolated folder: `wine ./windowed_ddraw_probe.exe --com-virtual-flip` and share its log.

## v0.1.41 — real three-surface COM attachment graph with fail-closed operations

The owner-confirmed 0.1.40 `--com-device-roundtrip` log reports **PASS** for the one-target native RGB565 HAL CreateDevice, physical GetRenderTarget -> original COM-wrapper identity mapping, scenes and 4:3 borderless presentation, without changing the desktop.

Stage B4j adds an **opt-in graph-enabled variant of our full 45-slot Surface4 COM forwarding object**. It creates three independent game-facing COM pointers representing **Primary -> Backbuffer -> Z32** over genuine Windows/Wine DirectDraw4 primary, 640x480 RGB565 3D target and attached 32-bit Z surface. The wrappers expose game-sized virtual GetCaps, GetSurfaceDesc, GetPixelFormat; actual `IDirectDrawSurface4::GetAttachedSurface` vtable calls return the correct stable child wrapper with a separate client-owned AddRef, while the parent owns an independent strong attachment reference. Role-restricted Primary->Backbuffer and Backbuffer->Z edges are acyclic and release their child COM refs on final parent Release.

Safety restriction: graph wrappers *explicitly reject* virtual `Flip`, `Blt`, `Lock`, and attachment mutations as not implemented, rather than calling Wine's native/exclusive operations with incompatible virtual pointers. Ordinary non-graph forwarding remains unchanged; the original game's DLL and INI are not modified.

Two additional Windows x86 mock-COM tests cover real typed 45-slot GetAttachedSurface/QI/refs/desc/format dispatch and cascade child lifetime, rejecting unsupported calls without invoking physical Flip/Blt. The new standalone `--com-surface-chain` mode repeats these calls with **genuine live DirectDraw4 COM objects**, passes only the **unwrapped native RGB565 backbuffer** to HAL CreateDevice, maps native GetRenderTarget back to the virtual backbuffer COM pointer, then runs D3D3 BeginScene/EndScene and the original physical black-clear + scaled viewport Blt.

This is the first **actual COM attachment topology test** under Wine, not a complete game replacement: virtual Flip and Blt are still fail-closed. Owner test: use the separately built `CrimsonSkiesFix-windowed-probe-Windows-x86-EXPERIMENTAL` executable from the next successful Windows x86 run, execute `wine ./windowed_ddraw_probe.exe --com-surface-chain`, then send adjacent `windowed_ddraw_probe.log`.

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


## Standalone v0.1.43 proof: 800×600 virtual COM Flip (not in-game)

The Windows x86 `windowed_ddraw_probe.exe` (separate Actions artifact, **not** the deployed `ddraw.dll`) supports `--com-virtual-flip-800` to repeat the fully native DirectDraw4 + D3D3 HAL + RGB565/Z32 + 45-slot three-surface virtual COM graph with an **800×600** internal render target. The existing `--com-virtual-flip` remains **640×480** and unchanged. Both use the same true borderless desktop-sized test window, preserve the desktop mode, reject unarmed/invalid Flip operations, and attempt 120 real two-Blt presentations. The test never launches Crimson Skies, modifies its files, or enables borderless in the game.

From a writable directory **outside the game installation**, run `wine ./windowed_ddraw_probe.exe --com-virtual-flip-800`, then send the adjacent `windowed_ddraw_probe.log`. Expect `ASPECT-VIEWPORT: source=800x600`, `COM-VFLIP-RESULT: requested=120 successful=120`, `COM-VFLIP-FINAL: PASS`, and `PROBE COMPLETED: PASS` if the Wine backend accepts this mode. A negative HRESULT is meaningful evidence; do not replace the installed game DLL for this test.


## Standalone v0.1.44: repeat full Surface4/Direct3D3 resource epochs (not in-game)

The opt-in `--com-virtual-flip-transition` creates **one** real desktop-size borderless HWND, one real `IDirectDraw4` instance with `DDSCL_NORMAL`, and one windowed primary+HWND clipper. It then **recreates and releases** the independent RGB565 3D target, matching Z32, three game-facing virtual Surface4 COM identities, actual HAL Direct3D3 device and virtual Flip bridge across successive source resolutions **800×600 → 640×480 → 800×600**. Each complete epoch independently performs 120 guarded physical two-Blt virtual-Flip presentations, checks the viewport, successful COM identity, Z32 and unchanged desktop resolution, and drops per-epoch COM owners before allocating the next target. The 640 and 800 single-mode options are preserved.

This is a bounded, isolated resource-reinitialization probe only. It does **not** call `SetDisplayMode`, emulate all game surface COM methods, intercept `CreateSurface`/D3D3 methods, or exercise the game's real resolution transition. The game DLL remains unchanged and borderless remains disabled by default.

Run in an independent writable test directory under the same Wine/GE-Proton setup:
```bash
wine ./windowed_ddraw_probe.exe --com-virtual-flip-transition
```
Return the adjacent `windowed_ddraw_probe.log`, including `COM-TRANSITION-EPOCH-START`, `COM-TRANSITION-EPOCH-END`, and `COM-TRANSITION-FINAL` lines. Each of the three epochs should report `COM-VFLIP-RESULT: requested=120 successful=120`. This is not a gameplay FPS test.

## Standalone v0.1.45: irreversible COM presenter retirement at a mode switch

The owner-supplied `windowed_ddraw_probe(20261030-172205).log` confirms full success for the standalone v0.1.44 **800×600 → 640×480 → 800×600** test: 120/120 virtual Surface4 Flip→two native windowed DirectDraw Blts in **each** of three resource epochs (360/360), all with real RGB565/Z32 HAL D3D3 resources and one unchanged 1920×1080 desktop. Elapsed 1504/1505/1508 ms measures only the standalone test.

A remaining safety risk is the game keeping an **old COM primary interface reference after changing resolution**. The disabled-in-game `com_surface4_forward` implementation now offers `retire_graph_primary_flip`, which synchronizes with in-flight virtual Flip using the existing per-primary mutex, drops the old native presentation session, and permanently marks that old primary as **retired** (rearming fails). Its valid COM identity remains reference-counted until the last Release, but future Flip returns `E_NOTIMPL` without native/exclusive fallback.

The updated `--com-virtual-flip-transition` standalone test now retires the old primary after each 120-frame epoch, verifies rejection of stale Flip, intentionally holds **one extra client AddRef** for the old COM primary while building the next epoch's new physical target and virtual chain, verifies the still-live stale pointer cannot present, then releases it. It verifies the final epoch's retirement too. Rust unit tests also reject rearming retired primary objects. All changes are test-only or in the inactive COM facade; **the active game proxy, config and existing options are unchanged**.

From the separate Windows x86 probe artifact, run under Wine in an independently writable folder:

```bash
wine ./windowed_ddraw_probe.exe --com-virtual-flip-transition
```

Send the adjacent `windowed_ddraw_probe.log`. Expected: three `COM-TRANSITION-RETIRE`, two `COM-TRANSITION-STALE` and one `COM-TRANSITION-LAST-STALE`, then `COM-TRANSITION-FINAL: PASS` and `PROBE COMPLETED: PASS`. This still does not activate game borderless or prove all real game COM methods are implemented.

  
## Standalone v0.1.46: virtual backbuffer RGB565 / depth Z32 null-source Blts

The owner's `windowed_ddraw_probe(20261030-173553).log` confirms **v0.1.45 PASS** under Wine: 800×600→640×480→800×600, 3×120 virtual Flip presentations (360/360); two held stale COM primary references independently reject Flip with `E_NOTIMPL`, and final primary rejects Flip at retirement. Desktop mode stays 1920×1080. None of these are actual game frames.

The inactive Surface4 graph facade now forwards exactly **backbuffer RGB565 `DDBLT_COLORFILL`** and **depth Z32 `DDBLT_DEPTHFILL`** with a NULL source to each role's own real native Surface4, optionally with `DDBLT_WAIT`. It requires a correctly sized Win32-x86 100-byte DDBLTFX, validates any supplied destination rectangle against the corresponding 640×480/800×600 native surface, and explicitly refuses virtual primary Blt, source-copy Blt, wrong fill/flags and retired backbuffer/Z32 operations. The three graph roles are tombstoned together when the prior primary presenter is retired. Other graph-specific Surface4 methods, including Lock and BltFast, remain unsupported or unvalidated for game use.

The existing **`--com-virtual-flip-transition`** EXE test now performs the two fills by calling the actual **virtual Surface4 vtable**, then validates primary and source-copy Blts fail closed, *for each real surface epoch*. After each retirement, and again while the old primary remains COM-alive into the next epoch, the test verifies stale backbuffer and Z32 fills also return `E_NOTIMPL`. This checks specific single-threaded Wine behavior; it does **not** prove full concurrent race safety or in-game rendering.

After a successful Actions build, from a writable directory outside the game:
```bash
wine ./windowed_ddraw_probe.exe --com-virtual-flip-transition
```
Return the adjacent `windowed_ddraw_probe.log`. Expect 3× successful `COM-GRAPH-BLT.backbuffer.ColorFill` and `COM-GRAPH-BLT.depth.DepthFill`, 3×120 `COM-VFLIP` successes, and `COM-TRANSITION-FINAL: PASS`. The game DLL's active hooks/INI/default pacing are unchanged. No PR merge or real borderless activation.


## Experimental game adapter — v0.1.47, first in-game implementation (not production ready)

Owner's v0.1.46 standalone Wine log confirmed **360/360** real virtual Flip presentations across 800×600→640×480→800×600, real virtual RGB565 ColorFill and Z32 DepthFill on all three epochs, and safe rejection of stale old COM writes. No further per-method standalone probe is planned.

In the actual \`ddraw.dll\`, the new **distinct default-off** option \`[Graphics] ExperimentalWindowedAdapter=1\` activates a first integration attempt on the real game's **IDirectDraw4**. With a valid process-owned HWND and a standard exclusive request, the hook negotiates real \`DDSCL_NORMAL\`, creates its own **native windowed primary/clipper**, and on a supported 640×480/800×600 16-bit SetDisplayMode prepares native RGB565, native Z32 and a real native HAL D3D3 device preflight. It builds verified separate virtual Surface4 Primary→Backbuffer→Depth COM objects, tests real two-Blt presentation and, **only then**, exposes the matched virtual primary or Z32 to game CreateSurface. The user's desktop display mode must remain unchanged. Virtual backbuffer→Z32 AddAttachedSurface accepts only the exact already-attached object; no generic source-copy forwarding. The game-facing virtual backbuffer is **unwrapped to its native Surface4 before D3D3 CreateDevice/SetRenderTarget** and maps back at GetRenderTarget. Mode changes retire the previous graph.

If native preflight fails **before any virtual game surface is exposed**, the original exclusive cooperative/mode calls are restored; if a later step fails after exposure, return a visible failure and log it rather than attempt unsafe live exclusive fallback. All experiment logs automatically go next to \`ddraw.dll\` even if \`[Debug] Log=0\`.

**IMPORTANT:** this is a first **in-game** implementation, not confirmed playable or fully borderless. \`IDirectDrawSurface4\` may receive extra COM calls / QueryInterface versions that the graph does not yet support; the runtime may report error or the game may fail to launch with this option. Existing \`Borderless=1\`, fps settings and old builds are not changed. Disable the experiment using \`ExperimentalWindowedAdapter=0\` to return to the previous exclusive path.

**Owner test (only after Windows x86 Actions SUCCESS):**

1. Back up the previous \`ddraw.dll\` and existing \`CrimsonSkiesFix.ini\`; replace only \`ddraw.dll\` from the new x86 game artifact.
2. In the existing \`CrimsonSkiesFix/CrimsonSkiesFix.ini\` set \`[Graphics] ExperimentalWindowedAdapter=1\` (and preferably \`[Debug] Log=1\`).
3. Run \`CRIMSON.EXE\` under the usual Wine/GE-Proton configuration. Observe the actual game, not \`windowed_ddraw_probe.exe\`.
4. Send \`CrimsonSkiesFix.log\` from beside \`ddraw.dll\`, plus whether it reaches the menu, draws any 3D image, and retains 1920×1080 desktop.
5. On any crash, immediately revert option to \`0\`, keep the log and report the exact last visible scene. Never merge this PR until validated.

No user files or game EXE modified. The branch/PR remain draft.
