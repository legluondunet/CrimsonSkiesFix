# CrimsonSkiesFix — release preparation and acceptance (12 November 2026)

**State:** experimental `0.1.90` candidate being prepared; **not published, not tagged**. This supersedes the 4 November *status snapshot* without deleting that historical audit. Supported artifact shape is the **one-file Windows x86 standalone launcher**. Source branch is stacked PR #27 on draft PR #26, not yet on `main`.

## Product and validation matrix

| Component | Acceptance evidence | Release claim |
| --- | --- | --- |
| Standalone install from original **FR** CD | Owner's `0.1.90-77d16572` GE-Proton session reports `INSTALL_FROM_CD_AND_LAUNCH`, exact approved executable, verified original data, guarded in-memory Video script swap, normal exit | **FR accepted under GE-Proton** |
| Standalone install from original **DE** CD | Owner's `0.1.90-2e9e6bc1` GE-Proton session reports `INSTALL_FROM_CD_AND_LAUNCH`, exact C2B CAB aliases, Video Graphics/Resolution not modifiable and normal exit | **DE accepted under GE-Proton** |
| Repeated launch without CD | Owner's second `0.1.90-2e9e6bc1` session reports `VERIFIED_EXISTING_AND_LAUNCH`, `GosOverlayState=VERIFIED_MANAGED` and `GAME_EXIT_CODE=0`; authenticated original DE archive size 62,678,770 bytes | **DE repeated launch accepted** |
| Original **USA/Europe** edition | 112-file authenticated CD profile; CAB source names and SHA-256 assertions for its chapter archives (USA C2B hash matches DE); unit tests pass | **Implemented only — NO owner runtime validation; USA media unavailable** |
| Native **Windows 11** | Historical test of an earlier DLL/explicit INI, plus latest x86 GitHub runner's 22-export DLL load test | **Latest standalone build and memory-virtualization runtime UNTESTED** |
| Renderer / graphics | Owner-tested under GE-Proton: borderless 1920×1080, wide 3D FOV/HUD, 4:3 menus, startup dialog bypass, RGB565 image fix, GameOS Graphics+Resolution read-only | **GE-Proton scope only** |
| Restore and repair | Previous tests of reversible managed GOS overlay, strict hash checks, preserved player files and no automatic deletion of abandoned staging; tests of collisions and synthetic invalid assets | **Guards implemented; not a claim of every real-world repair scenario** |
| Logs / packaging | Independent `CrimsonSkiesFix_installer.log` (retains last installation/repair/error across no-op relaunches) and fresh DLL `CrimsonSkiesFix.log` for every game session. CI regression tests cover separation. Earlier owner FR/DE runtime tests used the former unified log. | **Two-log candidate: CI tested; new owner runtime acceptance pending** |

### Exact authentication constraints

- Single accepted original decrypted 1.02 `CRIMSON.EXE`: `2,580,480` bytes, SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.
- FR, DE, USA original-disc profiles use per-file cryptographic hashes, not ISO whole-file size, volume label, or guessed short names. Original `CRIMSON.ROF` is authenticated for each edition before in-memory script substitution.
- Existing original game assets and player INI remain intact except the opt-in/default verified **small GOS overlay** with an authenticated original backup; the large original `CRIMSON.ROF` is not rewritten. Do not remove the proxy before following `GosVideoLock=restore` if restoring the original GOS screen is desired.
- **Eight** original chapter CAB files expand to **177 chapter ZBD entries**, plus **six** direct CD ZBD files; all temporary CABs are removed only after a separately verified chapter manifest is generated. Old documentation calling this 178 CAB members was inaccurate and is corrected.

## Readiness audit

- [x] Verify tracked CI release workflow builds an actual PE32 x86 DLL and Windows x86 launcher, verifies 22 DirectDraw exports and loader calls, tests Win32 library and launcher, and independently re-downloads **one** EXE artifact for PE validation.
- [x] Retain 30-day CI artifact retention.
- [x] Confirm launcher installs same-build GPL-3.0-or-later license, README, INI example and `build-info.txt` beside its embedded DLL, but never ships licensed original commercial game bytes, updater, decrypted EXE, active user INI or player saves.
- [x] Document that `GosVideoLock=off` does **not** reverse an existing managed overlay; `restore` does.
- [x] Remove unused dead-code scanner and unused ISO directory-entry struct field (the original ISO flags still drive `is_directory` and `multiextent` guards).
- [x] Replace obsolete experimental notes describing a five-file DLL artifact / only three exports with the current one-file launcher workflow.
- [x] Restore two independently owned logs: persistent installer/repair report (only meaningful operations rewrite it) and fresh per-session DLL diagnostics; remove the old child environment append handoff. Previous owner gameplay logs used the old unified behavior.
- [x] Correct stale FR/DE/USA test claims and 177-vs-178 chapter count.
- [x] Reduce first-run INI and shipped template to `FPSLimit=60` and `Log=0`; built-in game fixes remain default-on, existing INIs are untouched, and legacy diagnostic overrides plus explicit GOS restoration remain parseable. This change follows previous FR/DE gameplay acceptance and therefore requires new CI and a runtime smoke test.
- [ ] Green CI on the **last release-preparation branch SHA**, including new two-log persistence/regression tests (earlier FR/DE acceptance used the unified logger).
- [ ] Owner-approved merge **PR #26** into `main`, followed by retarget/review/merge of **PR #27**. Draft status alone is not approval.
- [ ] Green CI on the **final main SHA**, and verify the downloaded artifact is the one emitted by that exact build.
- [ ] Capture final `CrimsonSkiesFix.exe` SHA-256 and bytes; the build label and `build-info.txt` must match the final full commit, not an old PR-head or merge-ref.
- [ ] Owner explicitly approves tag, candidate title, GitHub prerelease and attached EXE. Without approval, **do not tag, publish or merge**.

### Suggested publication label

`CrimsonSkiesFix 0.1.90 — experimental standalone prerelease (FR/DE GE-Proton tested)`.

The project is **not** claiming native Windows 11 gameplay support for this exact release, general CD edition coverage beyond the strict profiles, or USA runtime testing. An experimental prerelease may be offered with these restrictions; it must not be marketed as a stable cross-platform build.

### Merge order and build provenance

1. Review/approve the draft standalone PR [#26](https://github.com/legluondunet/CrimsonSkiesFix/pull/26), which contains installer and workflow infrastructure.
2. Merge #26 only when the owner authorizes it. Retarget stacked [#27](https://github.com/legluondunet/CrimsonSkiesFix/pull/27) to `main`, refresh its comparison, and merge #27 only when authorized.
3. Run the x86 standalone workflow on `main`; both `windows-x86` and `verify-single-download` must pass. Obtain the sole artifact from **that same completed run** and record `sha256sum CrimsonSkiesFix.exe`.
4. Only after approval, create a fresh version tag on that exact SHA, publish the experimental prerelease and attach the one-file launcher build. No automatic GitHub release is performed by the current workflow.

**Do not transfer approval from historical PR tests to later binaries:** after any change to runtime or bundled docs, the final SHA and corresponding executable are a new candidate. Rust/PE tests do not replace a real gameplay session.

Details: [prepared release notes](release-notes-draft.md) and [future native Windows acceptance protocol](windows11-acceptance-2026-11-04.md).

legluondunet assisted by ChatGPT (OpenAI)
