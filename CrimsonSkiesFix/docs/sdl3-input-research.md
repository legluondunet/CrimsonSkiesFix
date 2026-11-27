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
