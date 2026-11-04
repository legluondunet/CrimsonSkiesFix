# Code audit and safe cleanup — 2026-11-04

## Scope

Reviewed the 61 tracked files on `main` immediately after PR #11 (commit
`730f3f101150a6b95a5f93e18df592590b0b5452`), the DLL's module
declarations and the standalone DirectDraw diagnostic program's explicit
`#[path]` imports. Compared the resulting dependency graph with the Windows
x86 CI baseline and active runtime log evidence. This audit is a **focused
static dead-code cleanup**, not a full memory-safety or COM-ABI proof.

## Removed from the repository

| Removed file | Why removal is safe |
| --- | --- |
| `src/com_unknown.rs` | Historical IUnknown-only mock/proof-of-concept, imported only by the DLL root; not called by production hooks, and not included in `windowed_ddraw_probe.rs` |
| `src/rgb565_present.rs` | Standalone RGB565-to-BGRA software composer, imported only by the DLL root; superseded by the active native RGB565/Surface4 adapter; no callers |
| `src/win32_present.rs` | GDI output proposal for composited BGRA frames, imported only by the DLL root; never called by live rendering or the standalone probe |

The removed modules contained their own isolated tests. Those tests disappear
from the DLL's suite, as the deleted implementation is not shipping. This is
intentional; it is not evidence of missing tests for active code.

## Removed from the DLL compilation, retained in the repository

- `mod com_surface4_roundtrip;` was removed from `src/lib.rs`, because the
  game proxy never references it. **The file stays**: the independent
  `src/bin/windowed_ddraw_probe.rs` explicitly imports this module and still
  runs its tests.

## Low-risk source cleanups

- Removed unused `CString` and `slice` imports from `diagnostics.rs`.
- Removed unused imports from `in_game_adapter.rs` without changing its
  COM hooks or native windowed adapter logic.
- Removed unused `WIDTH_RVA` / `HEIGHT_RVA` imports and constants,
  and a redundant unused public forwarding function, from the native
  widescreen patch module. The **actual** early patch and late fallback
  still call `full_patch::apply` as before.
- Updated inaccurate "default-off" module comments and marked the older
  borderless design document as historical. No changes to its research evidence.

## Intentionally retained

- Active, user-tested runtime paths: `in_game_adapter`, `menu_scene`,
  `menu_brand`, `gos_video_lock`, `menu_resolution_label`,
  `native_widescreen`, `startup_selector`, `window_erase_guard`,
  `window_exposure`, and the SHA-256-gated, default-on Wine-only
  `roffile_terminator_fix`.
- `com_surface4_forward`, `com_surface4_abi`, `borderless_native_blt`,
  `borderless_flip`, and `borderless_contract`: even though parts of
  these are historical scaffolds, the working DLL, native menu-branding
  path, and/or the standalone probe still depend on them.
- Diagnostic instrumentation: `surface_usage`, `blt_signature`,
  `scene_correlation`, GOS event/UI probes and read-only hardware
  inventory. These have either runtime call sites or a supported
  troubleshooting purpose, even when logging is normally disabled.
- Python GOS/ROF inspection and reversible overlay tooling: the CI invokes
  the relevant synthetic self-tests. Historical research scripts are retained
  rather than deleted without a user-approved archive policy.

## Regression criteria

The pull request must pass the Windows x86 GitHub Actions workflow, including
Rust unit tests, GOS overlay synthetic tests, release DLL compilation,
`IMAGE_FILE_MACHINE_I386` verification, and shipping-layout validation.
Expect a **lower DLL-only test count** because test-only orphan modules were
deleted; the independent probe suite must continue to pass. No on-device Wine
or Windows gameplay test is implied by green CI.

## Future work, not included in this cleanup

- Measure runtime-only module boundaries before further extraction; do not
  remove a module merely because it is default-off or appears in a historical
  prototype.
- Review the large `lib.rs` DirectDraw method-hook dispatch and COM facade
  integration in a dedicated behavior-preserving PR with targeted tests.
- Review warning debt and `unsafe`/COM pointer lifetimes separately; do not
  combine a substantial architectural rewrite with dead-code pruning.
- Re-test startup, menu/mission transitions, Alt+Tab and video preferences
  under Wine/Proton; perform native Windows runtime tests when available.

legluondunet assisted by ChatGPT (OpenAI)
