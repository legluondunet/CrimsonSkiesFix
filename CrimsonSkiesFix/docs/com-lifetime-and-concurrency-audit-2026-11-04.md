# CrimsonSkiesFix — audit COM / shared state (2026-11-04)

**Baseline:** `main` commit `6c2865eb80a4318bc32caf13b749930229f27e98` (PRs #14–#16 merged).
**Scope:** Windows x86 `ddraw.dll`, native DirectDraw COM vtables, virtual Surface4 reference counts, generation changes and shared mutexes.
**Evidence type:** source review + focused Windows x86 regression tests. A compile/test pass cannot establish the absence of deadlocks or use-after-free under all Wine and Windows scheduling conditions.

## Results and dispositions

| Priority | Location | Verified behavior / risk | Disposition |
| --- | --- | --- | --- |
| P1 — corrected here | `src/com_vtable.rs::replace_method` | Existing `(vtable,slot)` map key previously returned `true` unconditionally. It could report a hook installed after a foreign write, stale slot restoration, or a conflicting hook request, without verifying the actual slot. | **Fail closed:** successful repeat installation requires the currently installed slot to equal the requested hook and the saved original to differ. No repatching or COM ownership changes. Regression tests use synthetic vtable slots and never change real Win32 memory. |
| P1 — investigate separately | `src/in_game_adapter.rs::cooperative`, `display_mode`, `prepare_epoch`, `create_surface`, `remap_render_target` | `RUNTIME: Mutex<Option<Runtime>>` can remain locked across native COM method calls, Win32 window operations, virtual graph construction/retirement and `Drop` of `Com` owners. If these calls synchronously reenter a hook needing `RUNTIME`, the same thread may deadlock; not demonstrated by existing logs. | **No production change in this PR.** A future two-phase generation transaction (reserve under lock, call native code outside lock, revalidate generation before commit) needs dedicated Wine reproductions and rollback tests. Merely releasing the lock without pinning COM resources would introduce a use-after-free risk. |
| P2 — investigate separately | `src/com_vtable.rs::ORIGINALS` | Original methods are keyed by vtable address and slot for the DLL lifetime, with no remove-on-COM-Release. Shared immutable vtables normally outlive individual COM objects, so deleting a key per object is unsafe. Dynamically recycled tables could still collide with an existing key even after the new slot check. | Keep global registration and present fail-closed identity check. Do not add unconditional Release-based cleanup. Consider tracking hook type / installed slot, or stronger vtable provenance, in a separately tested change. |
| P2 — observed contract | `src/com_surface4_forward.rs::facade_add_ref`, `facade_release` | A synthetic Surface4 facade owns one strong real COM reference. Its `AtomicU32` owner count prevents resurrection from zero; final Release cascades its sole strong attachment edges and staging ref, releases its backing native COM pointer, then drops the facade. Callers MUST hold valid references during method calls. | Behavior retained. Existing 45-slot ABI, three-object acyclic attachment, final-Release and retired-generation tests already verify these contracts. New tests in `in_game_adapter.rs` explicitly cover the distinct adapter-owned vs game-transferred COM references. |
| P2 — observed contract | `src/com_surface4_forward.rs::forward_flip`, `retire_graph_primary_flip` | Both operations hold the *same per-primary* `virtual_flip` mutex; retirement waits for an in-flight native Blt and permanently tombstones primary, backbuffer and depth. | Retain lock scope until native presentation completes; splitting it risks releasing the presenter while in use. Investigate reentrancy only with an instrumented native callback trace and generation tests. |
| P2 — observed contract | `src/d3d3_hooks.rs::d3d3_device_get_render_target` | On successful native `GetRenderTarget`, a game-facing virtual backbuffer receives an independent AddRef before the returned native COM reference is released and swapped. | Retain ownership exchange and original HRESULT. A future isolated mock for the bidirectional native/virtual target identity could guard this path without invoking Wine's HAL. |
| Low — no action | `src/lib.rs` | Config and diagnostics use immutable `OnceLock`, atomics and mutex-protected accumulators. `DllMain` only publishes the module handle; native DLL loads/config/COM patches happen later. | No change. This is a deliberate once-per-process configuration contract, not a demonstrated race. |

## Invariants for any future behavioral change

1. Every successful COM `QueryInterface` and returned interface must own one reference; a borrowed pointer is only valid while the caller pins a live owning interface.
2. Native method addresses must be published before a vtable slot is patched, and a patch must only be considered already installed if the slot **still contains the same callback**.
3. Shared system COM vtables must not be unpatched or removed when one particular game object is released; there can be multiple objects using a single table.
4. Do not unpin `Runtime.epoch`, `Runtime.dd4_owner` or virtual Surface4 presenters during a callback that might access native resources.
5. On mode switch, the outgoing virtual primary must become permanently retired before the old `Epoch` owner references are dropped; in-flight Flips must serialize with retirement.
6. Retain COM surface identity and exact x86 `extern "system"` signatures; a successful `GetRenderTarget` remap must leave exactly one valid client-owned virtual reference.
7. No file system initialization, native DLL load, COM hooks or synchronous user callback in `DllMain`.

## Tests contributed by this PR

**COM vtable registry:** repeat-install only succeeds when the current slot equals the original requested hook (including a second object sharing one vtable); conflicting hook requests, externally overwritten slots and restored originals return `false`. The tests deliberately preseed the registry with a mock original pointer: no call to `VirtualProtect` or game-owned COM memory.

**Native COM owner:** synthetic 3-slot `IUnknown` vtable verifies (1) the adapter's Rust `Com` owner releases its returned native COM ref once on `Drop`, and (2) a separate `AddRef` transferred to the game survives the adapter's `Drop` until the caller explicitly releases it. Invalid null outputs are rejected.

**Existing coverage retained:** `com_surface4_forward` tests the 45-slot COM ABI, primary->backbuffer->depth graph reference cascade, lock/unlock staging, invalid graph wiring and retired-generation fill/Flip behavior. `in_game_adapter` tests strict cooperative replay, native descriptors and exact legacy RGB565 gating.

## Verification / limitations

- CI target: Windows x86 Rust DLL unit tests, standalone DDRAW diagnostic-probe tests, release DLL build, PE32 verification and artifact publication.
- The two new groups exercise **local contracts**, not arbitrary cross-thread COM callbacks, actual device loss or concurrent SetDisplayMode/Flip in Wine.
- A full owner gameplay smoke test on the PR build should still exercise **1920×1080 → 800×600 → 1920×1080**, multiple MainMenu re-entries, preferences locking and ROFFILE allocation reports.
- **No native Windows gameplay validation** until a suitable Windows machine is available.
- No unconfirmed concurrency defect is presented as a proven crash. The production change is limited to conservative handling of an already-registered COM vtable slot.

legluondunet assisted by ChatGPT (OpenAI)
