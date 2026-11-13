# CrimsonSkiesFix 0.1.90 — experimental prerelease acceptance checklist

**13 November 2026.** Pre-release work only. No GitHub tag or published release. The approved runtime baseline is the source on `main`, which already includes merged PRs #26 and #27. The final CLI is in PR #28; release-oriented source cleanup is stacked on that branch and must be merged only after approval.

## User-facing commands

| Invocation | Purpose |
| --- | --- |
| `CrimsonSkiesFix.exe` | Verify or install/repair the approved game, then launch |
| `CrimsonSkiesFix.exe --install-only` | Verify or install/repair fully, without launching the game |
| `CrimsonSkiesFix.exe --verify` | Read-only verification of the installed game and bundled DLL |
| `CrimsonSkiesFix.exe --launch-no-verify` | Direct OS start of `CrimsonSkiesFix/CRIMSON.EXE`; no hashes, presence checks, patching or repairs |
| `CrimsonSkiesFix.exe --inspect image.iso` | Identify/diagnose original or unsupported ISO media in a dedicated log |
| `CrimsonSkiesFix.exe --inspect image.cue [image.bin|image.img]` | Identify/diagnose CUE and referenced track, including an explicit data-track path |
| `CrimsonSkiesFix.exe --version` | Report build and source identity |
| `CrimsonSkiesFix.exe --help` / `-h` | Usage |
| `--verbose` | Optional extra diagnostics alongside a command |

The removed `--verify-install`, `--inspect-cd`, `--extract-cd`, `--install-cd`, `--import`, `--audit-shortnames`, `--inspect-update` and `--extract-update` commands are not public interfaces. No positional ISO/CUE arguments are accepted without `--inspect`.

## Source cleanups on top of the unified CLI

- Remove obsolete `DiscInstallation` summary that no caller consumes.
- Eliminate manual-install-only source/edition selectors. The automatic path passes the already SHA-authenticated official 1.02 updater into the internal installer; the patch is checked again when applying it.
- Eliminate the manual extraction staging entry point and unused result destination; keep all real retail-root files mandatory during extraction.
- Retain `--inspect` (and its ISO/CUE regression tests) as a **production diagnostic**. The old updater-only inspect report remains a test-only fixture.
- Do not touch DDraw, Wine/Windows rendering, GOS overlay/hashes, player saves, backup behavior or the exact retail filename mapping.

## Release gates

- [x] Historical first-run FR and DE original-CD installs, routine relaunch and GE-Proton gameplay tested on older candidate builds.
- [x] Windows x86 CI green on the last head of PR #28 (`a6481355`), including PE32 x86, 22 DDraw exports, Rust/Python tests, one-file artifact and basic CLI safety.
- [x] Windows x86 and downloaded-artifact verification passed at source-cleanup head `59d81dd76cfa92a46b35118bbe9af532c68101b0` (two successful workflow runs).
- [ ] Validate the single downloaded EXE really corresponds to the final merged `main` SHA (manifest and embedded DLL), not a PR's synthetic merge SHA.
- [x] Real **USA/Europe** CUE/BIN `C_SKIES_DISC_001` on candidate `0.1.90-59d81dd7` under Wine/GE-Proton: `--install-only` extracted 45 GAME files and all 8 chapter CABs; installer log says `Action=INSTALL_FROM_CD_ONLY`, `VerifiedGameData=YES`, `ApprovedExecutable=SHA256_VERIFIED`, `OriginalProtectedBackup=PRESERVED`, `GosOverlayState=OFFICIAL_102`, `GameLaunch=SKIPPED`, and `AUTO_INSTALL_RESULT=READY`.
- [ ] Re-test `--install-only` from **FR/DE** real original media on final candidate; `--install-only` USA test validates installation without launch, not gameplay.
- [ ] Relaunch without ISO/CUE/updater present, then `--verify` and confirm the installer log is not overwritten on a successful routine launch.
- [ ] Test `--launch-no-verify` separately; confirm no verification or repair, and that it does not change the installer report.
- [x] `--inspect` with authentic USA/Europe `Crimson Skies (USA, Europe).cue`: `C_SKIES_DISC_001`, exact internal file manifest, 112/112 hashed, 0 suspect, `INSPECT_RESULT=COMPLETE` in `CrimsonSkiesFix_inspect.log`.
- [ ] Test `--inspect` with unsupported media and verify the dedicated failure report doesn't modify the installed game or installer history (synthetic CI failure path already covered).
- [ ] In-game Wine/Proton acceptance: 1920×1080 borderless fullscreen/FOV/HUD, 4:3 menus/videos and optional 4:3 mode, main-menu signature, no legacy chooser, Video Graphics/Resolution controls read-only, opening image, Alt-Tab, saves and clean exit.
- [ ] Check default `CrimsonSkiesFix.ini` has exactly user-facing AspectRatio, FPSLimit and Log settings, and previous custom INIs are preserved.
- [ ] Verify the gameplay `CrimsonSkiesFix.log` is new each session, separately from the persistent installer journal.
- [ ] Confirm artifact retention is 30 days, downloadable payload is exactly one `CrimsonSkiesFix.exe`, and no commercial game files or decrypted executable are included.
- [ ] Record final SHA-256, size and commit for that exact EXE after `main` CI succeeds.
- [ ] Confirm the proposed tag `v0.1.90` is unused before any tag creation.
- [ ] Obtain explicit owner approval for merges and for publishing an **experimental prerelease**. Do not create a public GitHub Release or tag as part of this cleanup.

## Tested vs. untested boundaries

- **FR/DE original CD + GE-Proton:** historical owner acceptance only, not yet an endorsement of the final CLI/dual-log binary.
- **USA/Europe:** real original CUE/BIN authenticated and `--install-only` installation successful under Wine on `0.1.90-59d81dd7` (112/112 source file hashes, 45 GAME files, 8 CAB archives, approved EXE); **actual game launch, USA gameplay and USA GOS video behavior remain to be tested.**
- **Native Windows 11:** DLL loader/exports CI checks, not the same as native game-session acceptance of the current candidate.
- Keep the public release description appropriately scoped; do not claim a stable broadly tested Windows release.

## Suggested release title (only after approval)

`CrimsonSkiesFix 0.1.90 — experimental standalone prerelease (FR/DE GE-Proton tested)`

The first downloadable artifact should be produced from the exact tagged source SHA after successful final tests, not borrowed from an earlier PR build.
