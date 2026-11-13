# CrimsonSkiesFix 1.0 — release checklist

**13 November 2026.** **[CrimsonSkiesFix 1.0 published](https://github.com/legluondunet/CrimsonSkiesFix/releases/tag/v1.0)** (Cargo: `1.0.0`; Git tag: `v1.0`). Owner declined further manual tests; known limits remain disclosed. PRs #26, #27, #28 and #31 are merged. The exact release source is `5a9274d0af8018a346af2fa8e50e2e28dffc6d9d`; both automated CI jobs passed before publication.

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

### Evidence already available

- [x] FR and DE original-CD installation, regular relaunch and GE-Proton gameplay on earlier candidate builds.
- [x] Windows x86 Rust/PE/DirectDraw exports and independent downloaded-artifact CI passed on the old candidate `59d81dd7`. These checks are not claims about the new version-stamped binary.
- [x] Real USA/Europe original CUE/BIN `C_SKIES_DISC_001` inspected on `0.1.90-59d81dd7`: 112 original-disc file hashes matched, none suspect.
- [x] Real USA/Europe `--install-only` on `0.1.90-59d81dd7`: 45 original GAME files extracted, eight CAB archives expanded, installed game SHA checks passed, approved EXE authenticated, original protected EXE backed up, `GameLaunch=SKIPPED`, `AUTO_INSTALL_RESULT=READY`.
- [x] Version 1.0 selected. Rust crates use SemVer `1.0.0`; the public release name is **CrimsonSkiesFix 1.0**, the proposed tag is **`v1.0`**, and CI's one-file artifact name is **`CrimsonSkiesFix-Windows-x86-1.0`**.
- [x] Verified `v1.0` tag was not present on GitHub on 13 November 2026.

### Remaining publication operations — no additional manual test request

- [x] PR #28 and stacked PR #31 merged into `main` with the owner's approval.
- [x] Both automatic Windows x86 and independent downloaded-artifact jobs passed on release SHA `5a9274d0af8018a346af2fa8e50e2e28dffc6d9d` (run [43485004103](https://github.com/legluondunet/CrimsonSkiesFix/actions/runs/43485004103)).
- [x] Published only `CrimsonSkiesFix.exe` from the already-verified release CI artifact, PE32 x86, **1,455,104 bytes**, SHA-256 `897e0273ff506fe4545a94eb5d1b633847bc39657592af10e4bde414d442ac1f`, source `5a9274d0af8018a346af2fa8e50e2e28dffc6d9d`. The CI artifact retains 30 days; the GitHub Release asset is a durable release download.
- [x] Owner approved; tag `v1.0` and public GitHub Release published, with compatibility limits disclosed. Windows issue #22 closed at owner's request with an explanatory comment.

### Explicitly untested cases — accepted as release limitations

No additional user-side test is requested for version 1.0. The following outcomes are **not verified for the final build**, despite code-level tests or earlier gameplay evidence:

- New `--install-only` pathway on real FR/DE sources with the exact final version-stamped binary.
- `--launch-no-verify` on a real installed game and repeated-launch/dual-log behavior of the exact final binary. The automated CLI unit/smoke checks do not replace those real runs.
- USA/Europe game execution, mission play, GOS Video lock, Alt-Tab and save behavior; only **USA source inspection and no-launch installation** were demonstrated.
- Complete fresh native Windows 11 gameplay acceptance with the current DirectDraw/GOS runtime.

## Public compatibility statement

- **FR/DE + GE-Proton:** owner-tested game behavior on earlier candidate builds.
- **USA/Europe + Wine/GE-Proton:** real original-disc inspection and no-launch installation verified on candidate `0.1.90-59d81dd7`; gameplay not yet validated.
- **Native Windows:** x86 binaries built, loader and export checks performed by CI; current complete gameplay behavior not validated.
- Do not present the lack of further manual testing as proof of cross-platform compatibility.

## Planned release identity

**CrimsonSkiesFix 1.0** — Windows x86 standalone launcher and DirectDraw compatibility patch.

- Package versions: `1.0.0` (Rust SemVer)
- Git tag: `v1.0`
- CI artifact: `CrimsonSkiesFix-Windows-x86-1.0`
- Release payload: a single `CrimsonSkiesFix.exe`

The published executable comes from the verified final 1.0 release source commit `5a9274d0`, not from a prior PR build or earlier version `0.1.90`. [Download the official 1.0 EXE](https://github.com/legluondunet/CrimsonSkiesFix/releases/download/v1.0/CrimsonSkiesFix.exe).
