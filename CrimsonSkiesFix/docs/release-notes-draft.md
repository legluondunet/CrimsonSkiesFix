# CrimsonSkiesFix — release candidate notes (DRAFT — NOT PUBLISHED)

**Candidate version:** `0.1.90` — experimental standalone Windows x86 build.
**Tested runtime:** Manjaro Linux, GE-Proton11-12 / Wine 11.0.
**Native Windows status:** an earlier DLL/opt-in configuration was tested, but **this standalone candidate and its in-memory GOS VIDEO.SCRIPT substitution have not undergone native Windows gameplay acceptance**.
**Release state:** preparation only; no tag, GitHub Release or final public binary is authorized by this document.

## What's new

- **One-file installer and launcher:** the downloadable artifact contains only `CrimsonSkiesFix.exe` (Windows PE32 x86), which embeds the same build's strictly verified `ddraw.dll` DirectDraw proxy. The proxy exposes **all 22 original DirectDraw exports**; CI checks names, ordinals, and an x86 Windows loader smoke test.
- **Verified original-disc installation:** use a user-owned FR or DE ISO/CUE-BIN image, the authentic official 1.02 updater, and the user's locally supplied **exact approved decrypted 1.02 executable**. The installer authenticates internal disc files by per-file SHA-256, expands eight authentic chapter CAB archives, recreates the retail installation filenames, checks their payloads and removes the temporary CABs after verification. The original protected EXE is preserved as `CRIMSON.EXE.protected.bak`. The original game, updater, and decrypted executable are **not distributed**.
- **Safe repeated launches and repairs:** once installed, the launcher verifies the game and launches it without requiring the disc, patch, or decrypted EXE again. A full reconstruction occurs only when verification fails; saves, custom planes, and the active INI are preserved. An existing `CrimsonSkiesFix.previous` backup or abandoned `.rebuilding` / `.installing` directory is never overwritten or removed automatically.
- **GOS Video Preferences lock:** Graphics and Resolution are rendered read-only through the game's native mechanism. The fix authenticates the original `CRIMSON.ROF` and `ROFFILE.DLL` and supplies the exact patched `VIDEO.SCRIPT` descriptor and compressed data **in game memory only**, without rewriting the large original archive. A verified, reversible small `CRIMPTCH.ROF` overlay/backup may be installed once; its managed state is checked on subsequent launches.
- **Modern video presentation:** borderless desktop-sized graphics, corrected 3D field of view and HUD, preserved 4:3 menus/cutscenes, native-resolution menu label, removed legacy graphics-driver chooser, RGB565 opening-image correction, menu build signature, focus recovery and Wine-only authenticated ROFFILE allocation guard. **New opt-in `AspectRatio=4:3`: true 4:3 3D source raster and matching FOV/HUD, centered with black sidebars on widescreen monitors, without a physical resolution switch.** `AspectRatio=auto` retains existing desktop-native gameplay. Mandatory compatibility fixes remain built-in default-on.
- **Two independent logs:** `CrimsonSkiesFix_installer.log` beside the launcher retains the latest installation, repair, explicit inspection or failure; a successful normal relaunch or successful `--verify-install` will not erase an earlier report. The DirectDraw DLL creates a **new `CrimsonSkiesFix.log` at every game launch**, without an environment handoff or shared append. On installation/repair, the launcher records the child's exit status in its installer log; on verified-only launches it prints the status to the terminal. `[Debug] Log=1` enables detailed gameplay traces; normal first-run settings use `Log=0`.

## Installation and requirements

1. Download the **single `CrimsonSkiesFix.exe`** from the designated release candidate. It is an x86 Windows executable; launch under a compatible Wine/Proton environment or supported Windows installation. Keep it in a writable directory.
2. For a new installation, place beside the launcher **one supported original ISO or CUE with BIN/IMG**, **the authentic official 1.02 updater**, and **your own approved decrypted 1.02 executable**. Run the launcher without arguments. It creates a separately verified `CrimsonSkiesFix/` game folder.
3. For an existing, verified installation, run the same launcher again; the original source media are not required.
4. The active options file is `CrimsonSkiesFix/CrimsonSkiesFix.ini` **inside the game folder**. The new generated INI and supplied `.ini.example` contain `[Graphics] AspectRatio=auto`, `FPSLimit=60` and `[Debug] Log=0`; 4:3 is optional and all mandatory compatibility fixes are compiled into defaults. An existing active INI is preserved, never overwritten.
5. If you want the original GOS video screen back, set `[Graphics] GosVideoLock=restore` while the fix is still installed, launch once and confirm the verified restoration before uninstalling. `GosVideoLock=off` alone does **not** undo an existing managed overlay.
6. Logs beside the launcher: `CrimsonSkiesFix_installer.log` (persistent installation/repair/error or inspection report) and `CrimsonSkiesFix.log` (fresh game DLL diagnostics). Installed `build-info.txt` records the full source commit, DLL SHA-256 and log identity. No source media, game files, third-party widescreen executable or SafeDisc removal utility is included.

**Strict game executable requirement:** original approved unprotected version 1.02, size `2,580,480` bytes, SHA-256:
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.
The launcher does not decrypt or replace an unknown game executable.

## Compatibility and evidence (12 November 2026)

| Edition / environment | Actual evidence | Status |
| --- | --- | --- |
| **FR**, Manjaro / GE-Proton11-12 | Owner tested initial verified CD install, game launch, Graphics/Resolution lock and successful game exit; these gameplay tests used the earlier unified logger, before this two-log release-candidate change | **Owner-validated** |
| **DE**, Manjaro / GE-Proton11-12 | Owner tested initial CD install (including exact CAB aliases), native Graphics/Resolution lock, then `VERIFIED_EXISTING_AND_LAUNCH` with `GosOverlayState=VERIFIED_MANAGED` and exit code 0 | **Owner-validated** |
| **USA/Europe** | Exact original-disc 112-file SHA-256 profile, alias-table and synthetic tests are implemented. The original disc is no longer available to the owner | **Implemented, not runtime-tested** |
| **Native Windows (latest candidate)** | CI verifies that the x86 DLL loads and exports are correct; older manual Windows 11 tests were on a different runtime path | **Not game-tested with this candidate** |

The game data and supported executable must still be provided separately by the user. Unknown ISO/CD variants, unknown decryptions and third-party widescreen-modified executables are rejected. GPU, compositor, DPI, game video/audio codec, input and performance differences are not validated exhaustively. The GameOS graphics lock is deliberately limited to Graphics and Resolution; **other Video preference editability was not exhaustively retested**. The `Log=0` configuration can still include compatibility-adapter diagnostics; this does not imply that gameplay FPS measurements are enabled.

## Maintainer release checklist (not a license to publish)

- [x] FR and DE owner acceptance under GE-Proton; both `Log=0` and `Log=1` sessions observed on the prior unified-log version.
- [ ] Two-log change passes CI and, ideally, a short owner runtime smoke test; CI-only is not real-game verification.
- [x] USA edition remains explicitly marked **untested**; unavailable source media do not block an honestly labelled prerelease.
- [x] One-file artifact only, GPL-3.0-or-later license and source/provenance records installed with the proxy.
- [ ] Verify minimal three-option INI (AspectRatio, FPSLimit and Log), default-on compatibility features, and native 4:3 gameplay in the final candidate. Earlier owner game testing used the older generated INI and native aspect.
- [ ] Approve and merge stacked PR **#26 first, then #27** (or reconcile their changes into one accepted branch), retaining the owner's control of merges.
- [ ] Build exact final **main** commit and confirm the Windows x86 / Linux downloaded-artifact CI jobs both succeed.
- [ ] Record the **final main source SHA, release artifact size and SHA-256** after its build. Old PR binaries cannot serve as the final release.
- [ ] Verify the chosen `v0.1.90` tag name is unused and approve an explicitly labelled **experimental prerelease**, not a stable Windows release.
- [ ] Only after explicit owner approval: publish tag, release notes and the single final `CrimsonSkiesFix.exe` artifact. Do not upload original game, official updater or decrypted binary.

**No native Windows gameplay claim and no stable release claim are permitted from this evidence alone.**

Copyright for the original game and its data belongs to their respective owners; the project distributes original open-source patch code only, GPL-3.0-or-later.

legluondunet assisted by ChatGPT (OpenAI)
