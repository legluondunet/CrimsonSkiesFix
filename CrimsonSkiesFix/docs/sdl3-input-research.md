# SDL3 input research — 1.2, stage 1

Base: exact owner-validated 1.1 candidate 5852c118. No changes on PR #71.
Two owner-supplied 2,580,480-byte EXEs independently hash to
43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75.
No proprietary files are committed. Static x86 inspection:

- DirectInputCreateA IAT RVA 0x61fbb0, thunk VA 0x5f7332, call at 0x5350c0.
- Joystick initialization at VA 0x5363a0 (call 0x53510d), EnumDevices type 4,
  callback at 0x536520, QueryInterface to Device2, GetCapabilities,
  SetDataFormat, SetCooperativeLevel, per-axis property/configuration and
  Acquire. Native readiness at 0x75c1dc, COM pointer at 0x75c1e0.
- Poll at 0x536b50 (call 0x5355f5), Device2 Poll slot 25, GetDeviceState
  size 0x110 (DIJOYSTATE2) to 0x75cb78, then copies 0x110 to current
  0x75c1e4 and previous 0x75c2f4. Readiness accessor 0x536b40, state
  accessor 0x536c30, button edge accessor 0x536c40. This is separate
  from the mouse poll and keyboard. No replacement of these routines yet.
- Release at 0x536b10, call at 0x53540f. Must account for other users of
  native COM pointer, capabilities, GOS bindings, acquire/reacquire and
  cleanup before removing the original joystick backend.

Stage 1 embeds authenticated upstream SDL3 x86 3.4.18 and its zlib notice.
On verified foreground Flip thread, with Log=1, SDL enumerates mapped
gamepads, counts total joysticks, selects first mapped ID, samples axes,
buttons, gyro availability/data and touchpad count, and reacquires after
disconnect. Raw non-gamepad joysticks are inventoried only at this stage.
Logs compare these samples with original GOS joystick state. Neither native
joystick nor keyboard/mouse is suppressed; no synthetic input is emitted.
No mapping is applied to gameplay yet. This deliberate test gate verifies
hardware/SDL availability, not gameplay mapping. Existing Log=0 defaults
remain unchanged. Modern features are planned after analog ownership.
SDL official reference: https://wiki.libsdl.org/SDL3/CategoryGamepad

## Stage 2 — initial SDL-only analog flight adapter

The owner test of a926c1ba confirms SDL3 stick, trigger, button and gyro transport (PR #72 comment 7211556195). Three exact CALL sites (0x53510d init, 0x53540f release, 0x5355f5 poll) are replaced transactionally, plus the 13-byte input-selection condition at 0x487516. Fixed centered -1000..1000 calibration, original analog state X/Y/Z/Rz, four neutral POV fields, no native force feedback capability, original button transition callback if requested and valid. Z is absolute, so right stick Y integrates throttle at 500 units/s, bounded 0..1000. No native COM pointer is fabricated. Mouse and keyboard untouched. An already-initialized native joystick can be released at the first original polling call only after SDL initialization; if existing force feedback is present or guard fails, native path remains. With no pad, readiness false but original polling registration kept enabled after SDL init so hotplug is possible. Focus loss and disconnection neutralize axes/buttons and require centered sticks/released buttons before rearming. Provisional physical button slots 1..10 are not the final imposed action map; no claims about menu navigation or gyro-driven flight. All mutations are memory only. See log SDL3-FLIGHT.

## Stage 3 — fixed action slots, flight-only throttle and first menu bridge

Owner dev.2 follow-up 7211981477: keep analog signs/calibration and defer sensors/haptics/LED/touchpad. Static disassembly confirms CALL 0x489824 to 0x487460 (cdecl player argument from 0x71c298). Throttle integrates only on this actual player-control call, initialized to 100% on first/new observed player object or after observed null player; no reset on focus or pause. Pointer reuse without a null observation is not proven to denote a fresh mission. The action registration sequence 0x4936c0 onward binds 0x13 Space, 0x14 X, 0x24 E, 0x25 Shift+E, 0x19 F3, 0x12 N, 0x31 F8, 0x2d Q, 0x20 F5, 0x45 Esc; cross-checked with original manual (https://manualmachine.com/gamespc/crimsonskies/1119420-user-manual/). Override only joystick-slot lookup 0x537588 and press-callback reverse lookup 0x537458, retaining native keyboard/mouse actions and on-disk preferences. Exact menu mouse poll integration call 0x53825f (before original button-edge computation) receives SDL stick/D-pad deltas in logical GOS units, A merges into left button. Requires foreground, armed pad, recent polling, verified 800x600 render and mouse dimensions, MainMenu seen, no recent flight consumer. No OS-level input injection. B adds Escape state at exact GOS script key query 0x5d0616 only; whether menus use it remains a real-game test question. No sensor is enabled. Exact preimage checks cover all nine sites before mutation; tests cover original CALL encodings, fixed action inverse, cursor fractional integration and edge states.

## Stage 4 — active-device ownership and page identity

Dev.3 owner accepted the four explicit checks (PR #72 comment 7212330140). Preserve validated axes, fixed action table, menu cursor and throttle. All mapped pads are monitored on the original poll thread (bounded 32); intentional activity after neutral selects a single pad, and switching requires the current pad neutral for 350 ms plus a new contender edge. Simultaneous gestures use stable ascending SDL ID order. No Start requirement. Only selected device publishes native state; switching clears previous/current input edges without resetting throttle or aircraft. Disconnect closes the handle and publishes neutral, and focus loss resets candidate neutral gates. Raw non-gamepad joysticks are not yet supported. Page observer records only bounded resource tokens from original 0x403680 script handler (exact pointer use 0x403765..0x403768 and root copy 0x403703); ReadProcessMemory rejects invalid pointers. The original handler and rendering remain unchanged. No reliable original Joystick page resource is available in the working files, so do not paint an overlay over unidentified UI. Owner test should open the Joystick page once; no replacement panel is claimed in this build.

## Stage 5 — FMV skip

Owner reports dev.4 cannot skip cinematics with the controller. The existing GOS script Escape bridge requires MainMenu seen, 800x600, armed menu, and no recent flight. It does not cover the separate native FMV reader. Verified both local unwrapped EXEs SHA-256 43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75. Original FMV start 0x44aec0 references string FMV at 0x6249e0; update 0x44af20 tests Space at 0x44af3f (E8 5C B0 0E 00), Escape at 0x44af4e, Return at 0x44af5d and keypad Return at 0x44af6f, branching on state 1 to 0x44af7c. Original FMV loop 0x44ae90 polls GOS at 0x44ae9a before its update. Redirect only the Space query at this exact FMV call site to accept a fresh selected-pad A/B/Start press, consume at most once per poll, preserve native key state otherwise and require foreground, connected, armed and recent poll. This does not auto-skip or globally inject keys. Other cinematic implementations (e.g. the separate 0x427acc key-test sequence) remain unverified, so don't hook them blindly. No menu resource changes in this candidate.

## Stage 6 — in-engine animation skip

Owner dev.5 log 20261127-081445 confirms FMV skip on DS4 and Xbox 360 (lines 330,338); zeppelin 3D sequence does not skip. Start mask 0x40 at 666 and 669, first player aircraft marker 673. Static disassembly of both authenticated 1.02 images (SHA-256 43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75) confirms a distinct in-flight animation path: 0x4a02ac tests 0x71c50c for nonzero, then pushes zero and calls 0x536000 at 0x4a02b5 (E8 46 5D 09 00). This nonblocking function reads the keyboard-event scalar 0x75cb60. Nonzero result leads to original clearing of 0x71c50c, call 0x4ed480 with original animation pointer and zero, then 0x535460. The same nonzero animation global gates original player input at 0x4897dd..0x4897f3. Hook only that exact CALL, preserve the native result, require nonzero animation pointer, foreground, owner thread, connected/armed pad, recent poll and fresh A/B/Start edge. Shared consumed poll guard prevents one press triggering both cinematic paths. GOS polling occurs later at 0x4a02e3, so the hook intentionally uses the previous completed poll. No direct animation state changes, global key injection, or edits to pause/flight mapping. This is a verified candidate path, not runtime proof that the zeppelin uses it. The separate briefing sequence 0x427acc refers to brief_c%d%d and is not modified without evidence.

## Stage 7 — safe in-game controls-panel preview

Owner accepted dev.6 FMV and zeppelin skip (PR #72 comment 7213622693). Log 20261127-082734 proves successful `cp_background`, `cp_t_joysticktitle`, mouse and keyboard-related resources (lines 1782–1811), followed by `kb_background` (1838). This is a mixed Controls Preferences page, not a proven joystick-only screen. Dev.7 therefore adds a hold-to-view static diagram instead of masking interactive legacy widgets before visual confirmation. Identity requires successful exact background + joystick-title names; another background, main-menu reload, apply/cancel or non-800x600 frame clears it. Y/Triangle alone with centered sticks on selected armed gamepad and existing menu context, or F1 held in foreground, shows a sepia GDI drawing on the existing private RGB565 scratch, with a 150ms freshness limit for pad state. Release returns to original UI. No input swallowing, no preferences writes, no external artwork or font files; native rendering/controls unchanged. Hint placement and English labels provisional. Resource traces now limited to unique successful Preferences names (512); page identity works Log=0. Need real screenshots and repeated navigation validation before choosing permanent replacement geometry and localization.

## Stage 8 — contextual modal controls diagram

Owner accepted dev.7 screenshot direction and requested actual pad family, one-press opening and next event dismissal, remove F1, move X/Square below A/Cross, native-placement invite and localization. SDL_GetGamepadType provides type (Xbox360/One vs PS3/4/5), name fallback only for standard/unknown; generic positional labels otherwise. Preserve existing flight mapping. Pad modal state suppresses all forwarded pad axes/buttons while displayed, consumes dismissal until neutral, ignores opening release and clears on focus loss/page exit/disconnect. Original 0x535611 CALL to 0x535a80 (E8 6A 04 00 00) is the GOS keyboard poll; scope to modal/held consumed keys only; original aging routine 0x535a00 and direct native GetDeviceData slot 10 with 16-byte records as verified in 0x535a80..0x535cdb. Drain before callbacks, clear only the corresponding native key state indices <0x7de and original any-key scalar, return to native poll otherwise. Existing 0x53825f mouse hook consumes physical mouse event before native edge computation. Native script Escape suppressed while modal. No global input injection. If native drain fails, close and forward original keyboard call. Render fail closes modal; 2s presentation freshness watchdog. Localize title from native buffer 0x646acc after id1118 (original helper 0x4050c0), fallback to exact edition receipt at launcher state path. No asset or native settings modifications. Need real Wine and Windows tests for dismissal and visual layout. Existing FMV/3D skip and flight code preserved apart from scoped pad filtering.

## Stage 9 — Crimson art direction and takeover fix

Owner approved the red/brass industrial concept after dev.8, not the incorrect illustrative labels. Implement original layered curved GDI geometry, red gradient panel, metal/brass plates, colored face symbols, aligned leaders, and the real existing mapping (left stick pitch/roll, right yaw/throttle, shoulders previous/next enemy, south cycle guns, west cycle views, Options/Start pause). Preserve original title above logical y=240; render 800x360 panel into a cached 32-bit DIB per family/language, copy onto existing 800x600 scratch. No generated mockup background, logo, or proprietary game assets bundled. Style approximation, not pixel-identical to approved concept. Dev.8 log showed takeover A/B leaking (lines 562,578). Preserve modal block across device selection/removal until neutral; log close reason and first family/language presentation. Unit tests cover A/B/Y/Start takeover and nonmodal selection. No changes to flight mapping or existing native keyboard/mouse hooks. Need real screenshots and repeated modal dismissal checks.
