# Automatic verified install, repair and launch

This experimental standalone installer was integrated into `main` by merged PRs #26 and #27. A follow-up CLI simplification is proposed in PR #28. The owner validated FR and DE installations and repeated launches under GE-Proton11-12 in November 2026. USA/Europe is identified by an exact CD manifest but lacks current owner runtime testing; native Windows validation of this candidate remains pending. Running the Windows-x86 `CrimsonSkiesFix.exe` without arguments verifies, installs, repairs when needed, and launches the approved game.

## Local sources (no original files are bundled)

For a fresh installation or a full repair, place these files beside the launcher (and use the DirectDraw patch embedded in its executable):

- Exactly one original and complete USA/Europe, FR or DE CD image: an ISO, or CUE with its referenced BIN/IMG file.
- Exactly one official Microsoft Crimson Skies 1.02 updater. The original updater's size and SHA-256 are pinned; the name itself is not trusted.
- One locally owned decrypted original game executable. Any EXE filename is accepted ONLY if its size is exactly 2,580,480 bytes and its SHA-256 is 43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75. No third-party variant, download, decompression or SafeDisc bypass is performed by this installer.

For an already verified, fully playable installation, these three external files are unnecessary to launch. For an old protected-pending installation, the approved executable by itself suffices to complete activation.

## Default behavior

1. If the game directory has an original-CD installation receipt, verify the edition-specific original-file SHA-256 inventory, all installed retail support files, **177 chapter CAB members** via the sealed chapter manifest (plus 6 original direct ZBD files), the official 1.02 ROF, the included DirectDraw proxy and the approved game EXE.
2. If the game data is verified but the protected CD EXE is still pending, locate the local approved decrypted EXE by SHA-256 and install its copy as CRIMSON.EXE. Preserve the untouched protected original under CRIMSON.EXE.protected.bak. The complete file manifest continues to authenticate this backup against the original CD.
3. If only the bundled DirectDraw DLL differs, use the existing verified backup-and-replace mechanism and revalidate the whole game without rereading the CD.
4. If the game data is corrupted, build a complete new installation alongside the old one, from an authenticated original CD, the authenticated official updater, and an authenticated decrypted executable. Preserve the original directory and copy SavedGames, Planes and CrimsonSkiesFix.ini into the verified new installation. Refuse unsafe links and collisions.
5. Publish only after validating the rebuilt game, replacing the game directory through a reversible rename. Preserve the previous complete installation in CrimsonSkiesFix.previous. Never overwrite that backup, and never remove a partially completed staging directory automatically.
6. If there is no game installation, create and fully verify the same complete new game before publishing. **Before reading/extracting a CD**, identify and authenticate the approved decrypted EXE, embedded DirectDraw patch and official updater; refuse extraction if any is missing or invalid.
7. Normally launch only the approved executable, with the game directory as the current working directory. With `--install-only`, verify and publish but **do not launch**. With explicit `--launch-no-verify`, only attempt to execute the installed `CRIMSON.EXE` without source, file-presence or integrity checks, repair or extraction (OS launch errors can still occur).

The launcher keeps **two independent log files** beside itself: `CrimsonSkiesFix_installer.log` contains the last meaningful install/repair or error, and `CrimsonSkiesFix.log` is always a fresh DLL-generated game session. A verified normal relaunch does **not** overwrite the install log; the explicit `--verify` command leaves the installer log unchanged. Installation and repair failures replace it with their diagnostic report. A first-time install or repair report receives `GAME_EXIT_CODE` after the child exits; a verified-only relaunch prints the child's exit code in the console and preserves the previous installer report. The DLL no longer uses any environment handoff or appends to the installer log. The prior hyphenated `CrimsonSkiesFix-installer.log` is safely renamed on startup only if its content identifies a generated report and the underscore target is absent; otherwise unknown/occupied paths are preserved. A legacy `CrimsonSkiesFix.installer.log` is removed **only after** the next real installer report is written and only if its small regular-file contents authenticate it as old launcher output; unknown files are preserved.

Legacy imported games without the new original-CD receipt retain their existing EXE/DLL verification for backward compatibility. They do not receive a full original-CD hash audit unless rebuilt from exact original media.

## Supported command-line interface

- No arguments: automatically verify, install/repair if needed, and launch.
- `--install-only`: same authenticated install/repair path without a game launch.
- `--launch-no-verify`: direct OS launch of `CrimsonSkiesFix/CRIMSON.EXE`, without any presence/hash/patch/source check or repair. Deliberate unchecked mode; do not confuse it with the normal launcher.
- `--verify`: authenticate the existing installed executable, game assets and bundled DirectDraw proxy without running the game.
- `--version` and `--help`: build identification and CLI usage.

The obsolete options `--import`, `--audit-shortnames`, `--inspect-update`, `--extract-update`, `--verify-install`, `--inspect-cd`, `--extract-cd` and `--install-cd` (including its `--into` variant) are all rejected. The exact-disc inspection, authenticated extraction, official-update resource extraction, CAB name correction and verification remain implemented internally and operate automatically during normal installation, repair and `--install-only`.

No original game or decrypted executable is shipped with CrimsonSkiesFix. No registry, DirectPlay, system DLL, shortcut, or uninstaller operations are introduced here. Those historical setup actions remain for separate verification.

## Release-candidate acceptance matrix (12 November 2026)

- **FR — accepted by owner under GE-Proton:** real-disc installation, automatic launch, and verified in-memory GOS VIDEO.SCRIPT lock (the owner's acceptance sessions predate the two-log change).
- **DE — accepted by owner under GE-Proton:** real-disc installation including authenticated CAB 8.3→long filename mapping, repeat verified launch without re-extraction, managed GOS overlay, read-only Graphics/Resolution controls, and clean process exit.
- **USA/Europe — implementation only:** exact 112-file original-CD profile and matching synthetic CAB assertions; **no owner original CD remains for actual runtime testing**. Do not advertise gameplay validation.
- **Native Windows — outstanding for this candidate:** older DLL with an explicit compatibility INI was tested on Windows 11, but not the latest standalone package or native VIDEO.SCRIPT memory virtualization. Not a blocker for a clearly labelled experimental GE-Proton-tested prerelease if the owner elects to publish it.
- **Safety checks enforced by CI:** missing sources do not overwrite the original installation, player data is preserved on rebuild, staged directories are not deleted automatically, and exactly one PE32 x86 launcher artifact is emitted.

Do not claim all-edition or native-Windows gameplay acceptance from the Rust unit tests or the installer manifest alone.
