# Automatic verified install, repair and launch

This is an EXPERIMENTAL draft milestone in PR #26. Running the standalone Windows-x86 CrimsonSkiesFix.exe with no arguments now verifies, installs, repairs when required, and launches the game.

## Local sources (no original files are bundled)

For a fresh installation or a full repair, place these files beside the launcher:

- Exactly one original and complete USA/Europe, FR or DE CD image: an ISO, or CUE with its referenced BIN/IMG file.
- Exactly one official Microsoft Crimson Skies 1.02 updater. The original updater's size and SHA-256 are pinned; the name itself is not trusted.
- One locally owned decrypted original game executable. Any EXE filename is accepted ONLY if its size is exactly 2,580,480 bytes and its SHA-256 is 43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75. No third-party variant, download, decompression or SafeDisc bypass is performed by this installer.

For an already verified, fully playable installation, these three external files are unnecessary to launch. For an old protected-pending installation, the approved executable by itself suffices to complete activation.

## Default behavior

1. If the game directory has an original-CD installation receipt, verify the edition-specific original-file SHA-256 inventory, all installed retail support files, 178 CAB members via the sealed chapter manifest, the official 1.02 ROF, the included DirectDraw proxy and the approved game EXE.
2. If the game data is verified but the protected CD EXE is still pending, locate the local approved decrypted EXE by SHA-256 and install its copy as CRIMSON.EXE. Preserve the untouched protected original under CRIMSON.EXE.protected.bak. The complete file manifest continues to authenticate this backup against the original CD.
3. If only the bundled DirectDraw DLL differs, use the existing verified backup-and-replace mechanism and revalidate the whole game without rereading the CD.
4. If the game data is corrupted, build a complete new installation alongside the old one, from an authenticated original CD, the authenticated official updater, and an authenticated decrypted executable. Preserve the original directory and copy SavedGames, Planes and CrimsonSkiesFix.ini into the verified new installation. Refuse unsafe links and collisions.
5. Publish only after validating the rebuilt game, replacing the game directory through a reversible rename. Preserve the previous complete installation in CrimsonSkiesFix.previous. Never overwrite that backup, and never remove a partially completed staging directory automatically.
6. If there is no game installation, create and fully verify the same complete new game before publishing and launching.
7. Launch only the approved executable, with the game directory as the current working directory.

The separate CrimsonSkiesFix.installer.log beside the launcher records the auto-install/repair action and any error. The game's DirectDraw proxy keeps its own game runtime log.

Legacy imported games without the new original-CD receipt retain their existing EXE/DLL verification for backward compatibility. They do not receive a full original-CD hash audit unless rebuilt from exact original media.

## Diagnostic commands retained

--inspect-cd and --extract-cd operate on original disc images.
--inspect-update and --extract-update inspect the official patch.
--install-cd remains a manual data-installation diagnostic and may leave the original protected executable pending. Running the launcher without options then completes activation when an approved local EXE is present.
--verify-install and --verify perform explicit validation.
--import imports an existing user-owned installation with the pinned EXE.

No original game or decrypted executable is shipped with CrimsonSkiesFix. No registry, DirectPlay, system DLL, shortcut, or uninstaller operations are introduced here. Those historical setup actions remain for separate verification.

## Owner acceptance tests before merging

- Fresh USA/Europe, FR and DE CDs: fully verified file layout and CAB removal, original EXE backed up, approved 1.02 EXE activated and actual game launch.
- Subsequent launch without disc/update/decrypted source files succeeds, without extraction.
- Partial or corrupted game assets cause a safe repair, preserving saved games, custom planes, INI and original directory.
- Missing official CD, updater or approved EXE produces a clear error without altering an existing incomplete game (apart from a deliberate verified proxy-only update).
- Existing repair staging or backup never gets overwritten.
