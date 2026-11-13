# CrimsonSkiesFix — pre-release code cleanup (13 November 2026)

**Scope:** conservative source cleanup stacked on PR #29; no release, no merge, and no change to the original game executable or DirectDraw rendering hooks. This document describes source-level changes, not completed user gameplay tests.

## Confirmed obsolete code removed

- `installer/src/main.rs`: removed the now-unread `DiscInstallation` success summary, its 11 fields and the result construction originally used by the deleted manual `--install-cd` reporter. The automatic installer needs only success/failure, so it now receives `Result<(), String>`.
- `installer/src/main.rs`: removed optional CD/patch/expected-edition arguments used exclusively by the deleted manual CD command. The internal installer still authenticates the disc, official update, chapter CAB payloads and original retail files.
- `installer/src/disc_inspect.rs`: removed the unused `extract_game` diagnostic staging entry point, its optional staging destination and the unused `Extraction.destination` field. The automatic path always explicitly passes its private stage and includes the eight original CD-root support files.
- `installer/src/auto_install.rs`: removed a duplicate `update102::find` preflight. The called `install_from_cd` performs the same preflight before creating any extraction staging directory.
- `installer/src/disc_inspect.rs`: retained the synthetic ISO/CUE inspection function behind `#[cfg(test)]`, preserving the existing file-hash and CUE parser regression tests without compiling the obsolete public inspector into the production installer.
- Removed obsolete command references from ISO/CUE error messages, module comments and related test expectations. Ambiguous source selection now advises placing exactly one ISO/CUE next to the launcher.

## Preserved deliberately

- **Retail integrity gates:** independent SHA-256 validation of original disc files, official updater and decrypted 1.02 EXE; chapter CAB extraction and sealed manifests; verification of installed assets.
- **Recoverability:** non-destructive staging and repair, previous-game backup, protected executable backup, saved games, player INI and symlink safety checks.
- **Legacy compatibility:** recognized old installer-log spellings and report headers are still accepted for safe migration; removing former CLI options does not justify deleting previously generated logs.
- **Windows/Wine compatibility:** native RGB565/XRGB8888 video fixes, borderless Surface4 forwarding, 4:3 menus, 3D FOV, Wine ROFFILE guard and diagnostic instrumentation remain untouched. Existing opt-in diagnostics are not automatically dead code.
- **Retained diagnostics:** `--verify`, `--import`, official updater diagnostics and chapter filename audit are unchanged; this PR does not redefine the public CLI.

## Tests and release gates

1. Windows x86 GitHub Actions: compile both Rust crates, run all launcher/library Rust tests, Python synthetic tests, DirectDraw export/LoadLibrary smoke, PE32 and single-EXE artifact assertions.
2. CLI: check `--help`, `--version`, `--verify` failure on absent game and rejection of the four retired flags (the baseline tests are in PR #29).
3. Original **FR and DE** discs: fresh install, repeated launch without source media, verified repair, 1.02 update, long CAB filenames, correct log separation and game exit.
4. In game: borderless desktop resolution/DPI, 3D widescreen FOV, 4:3 UI and video, startup driver dialog removed, Graphics/Resolution settings locked, Alt-Tab, gameplay and saves.
5. **USA/Europe** real media and latest **native Windows 11** gameplay are not validated by this source cleanup; avoid claiming support is runtime-tested until the appropriate tests occur.

## Remaining decisions (not changed here)

- The older `--extract-update` diagnostic expects edition-specific test staging folders ordinarily created by the now-removed manual CD extraction command. It may be a future CLI simplification candidate, but is not silently removed in this cleanup.
- Previously discussed standard CLI names such as `--install-only`, `--launch-no-verify`, `--verbose` and `--inspect` should be reconciled with the actual `--help` before a public release. Their presence is **not** asserted by this PR.
- A broad COM pointer/lifetime or shader/renderer refactor is too risky for release cleanup without targeted regression evidence.

**State at drafting:** PR #29's two Windows x86 workflow runs for commit `c9192fb` completed successfully. This stacked cleanup requires **its own** CI and Wine/Windows runtime acceptance before publication.
