# CrimsonSkiesFix — runtime module boundaries (2026-11-04)

## Scope and purpose

This document describes the **Win32 x86 DirectDraw `ddraw.dll` proxy**
after the first structure-only refactor (#14). It does not claim a new
compatibility fix, FPS improvement, cross-process safety proof, or Windows
runtime validation. The effective behavior on the owner's Wine / GE-Proton
environment is established by separate gameplay logs.

## Architecture and ownership

| Module | Responsibility | Important constraints |
| --- | --- | --- |
| `src/lib.rs` | DirectDraw exported entrypoints, game-specific method interception, COM method dispatch, configuration/log initialization, and the remaining native DirectDraw/D3D3 hook glue | Win32 x86 ABI, installer paths, interface-slot numbers, hook timing and correct COM ownership must not change during pure refactoring |
| `src/com_vtable.rs` | Global original-method registry keyed by **(vtable address, method slot)**, read-only vtable slot discovery, installation and rollback | Publish original address before vtable write; share the same map across all COM interfaces; do not run this in `DllMain` |
| `src/frame_pacing.rs` | Original opt-in QueryPerformanceCounter pacing | Preserve mutex state, Sleep/spin behavior, the selected FPS limit and caller-controlled gating |
| `src/render_sampling.rs` | Bounded API call-rate diagnostic logs for Blt, Flip, Unlock and D3D3 scenes | Log messages report **COM call counts**, not verified displayed FPS; no extra per-frame output |
| `src/com_labels.rs` | Read-only GUID classification, surface capability labels and legacy descriptor summaries | Never rewrite the caller's interface or surface descriptor; log labels retain exact text |
| `src/blt_crash_trace.rs` | Opt-in bounded rolling native Blt trace file | Default off; preserve 2,048-call rollover and capture ENTER **before** the native COM invocation |
| `src/in_game_adapter.rs` | Game-owned modern DD4 virtual primary/backbuffer/Z32 graph, native borderless windowed behavior | Active and user-tested: must not be conflated with retired software/GDI prototypes |
| `src/com_surface4_forward.rs`, `src/com_surface4_abi.rs`, `src/borderless_native_blt.rs` | Exact COM facade layout, real native Surface4 delegation and RGB565 presentation / menu branding | Preserve native IUnknown references and all 45 Surface4 ABI slots |
| `src/native_widescreen.rs`, `src/startup_selector.rs` | Exact supported EXE early in-memory FOV/HUD resolution patch and startup selector bypass | Verified EXE and instruction fingerprints; no disk modifications |
| `src/gos_video_lock.rs`, `src/menu_*.rs` | Exact GOS preference/menu callbacks, resolution label, video lock and main-menu signature | Preserve non-video settings, localized UI behavior and persistent native override restore policy |
| `src/roffile_terminator_fix.rs` | **Wine-only**, exact-SHA256 `ROFFILE.DLL` EOF allocation padding | Default enabled for the supported binary under Wine; no native Windows hook or on-disk changes |

## Observable invariant set for regressions

1. Supported `CRIMSON.EXE` SHA-256 admission is unchanged.
2. The `ROFFILE.DLL` SHA-256/byte-pattern gate and `file_size+2` native allocation wrapper are unchanged.
3. Native 1920×1080 or detected monitor widescreen/FOV/HUD values, menu 800×600 rendering, video lock, first-frame RGB565 handling and the focus/erase recovery remain operational.
4. DirectDraw COM hooks install the **same** function addresses into the **same** vtable slots and forward via the **same** original-method addresses.
5. `CrashBltTrace` remains off in the shipped configuration; enabling it still records a rotating trace without holding the file mutex across native `Blt`.
6. `ModernFPSPacing` remains off by default, and its selected runtime opt-in semantics are unchanged.
7. Original `CrimsonSkiesFix.ini` user settings, log location, packaged DLL name and tool artifacts are unchanged.

## Build and runtime proof

CI: `.github/workflows/crimsonskiesfix-x86.yml` must pass x86 Rust library tests, standalone probe tests, release compilation, PE32 machine verification and packaging. Module-specific tests cover null COM objects, absent vtables, pointer/slot identity, zero vtable entries, self-hook refusal, known GUIDs, capability roles and descriptor labels.

Green Windows x86 compilation proves binary consistency but **not** a native Windows gameplay test. Since no functional behavior is intentionally changed, require an owner Wine/Proton smoke test with at least the MainMenu, Video Preferences round-trip, native 3D gameplay, one mode transition and ROFFILE log counters before merging.

## Next extraction candidates (not changed by this PR)

- Split original DirectDraw v1 hooks from DirectDraw4/7 hooks, but only after all callbacks and method indices have dedicated ABI tests.
- Split native Direct3D3 CreateDevice/BeginScene/EndScene and render-target forwarding into a separate module, preserving the virtual/native Surface4 bridge semantics.
- Isolate initial configuration and log initialization only after testing exact INI precedence, no filesystem work during `DllMain`, and cross-module log ordering.
- Inspect the lifetime of the global original-vtable registry and the lock/patch publication boundary under concurrent COM calls, preferably with targeted thread tests and dedicated Wine repros. This is a separate behavioral safety investigation, not a mechanical cleanup.

legluondunet assisted by ChatGPT (OpenAI)
