# Windows 11 acceptance protocol — CrimsonSkiesFix (2026-11-04)

**Status:** manual acceptance test **pending** a Windows 11 PC. This checklist is NOT evidence that Windows gameplay was already tested. The CI already checks only PE32 DLL loading/exports and the archive's integrity.

## Preparation (do not overwrite the user's files)

- Use an unmodified, independently backed-up Crimson Skies 1.02 installation. Supported `CRIMSON.EXE`: **2,580,480 bytes**, SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.
- Work in a test copy of the game directory or back up at least `ddraw.dll`, `CrimsonSkiesFix/CrimsonSkiesFix.ini`, and `GOSDATA/ASSETS/crimptch.rof` plus any existing `*.original.bak`. Never delete an unknown GOS backup to force a patch.
- Download the **final approved SHA's** Windows x86 DLL artifact, not an older PR build. It contains exactly five files: `ddraw.dll` and `CrimsonSkiesFix/{README.md,CrimsonSkiesFix.ini.example,LICENSE,build-info.txt}`.
- Inspect the included manifest; compare `DllSha256` to the local DLL. In PowerShell, from the game root:

```powershell
(Get-FileHash .\CRIMSON.EXE -Algorithm SHA256).Hash
(Get-FileHash .\ddraw.dll -Algorithm SHA256).Hash
Get-Content .\CrimsonSkiesFix\build-info.txt
```

- Enable temporary diagnostics by editing the active `CrimsonSkiesFix/CrimsonSkiesFix.ini` (`[Debug] Log=1`). If the file doesn't exist yet, first create one with `[Debug]\nLog=1` and leave all other settings at their built-in defaults; the example file is not automatically read. The diagnostic file `CrimsonSkiesFix.log` is written beside `ddraw.dll`.
- For a clean first-run GOS overlay test, use original unmodified assets only. The default `GosVideoLock=on` modifies `crimptch.rof` **after** an exclusive backup; never test installation over an unknown/previously patched archive.

## One end-to-end game session

1. **Launch:** Start `CRIMSON.EXE` without Wine; verify the outdated driver/resolution chooser does not appear, there is no unexpected DirectDraw error, and the original first image has normal colours rather than magenta.
2. **Movies/menu:** Watch or skip an opening movie and ensure correct video/audio playback with 4:3 proportions. On MainMenu, verify project signature is visible once in the intended location and not elsewhere.
3. **Video Preferences:** Enter Preferences, confirm **Graphics** and **Resolution** are greyed out and unselectable, the Resolution text displays the actual monitor resolution (e.g. 1920×1080), and other settings can be accessed normally. The signature must be absent on this screen, then restored when returning to MainMenu.
4. **Gameplay 3D:** Play several minutes in an actual mission. Check that the view fills the chosen monitor, the camera/FOV and HUD are not distorted, the image colours and game speed appear normal, and no random crashes or freezes occur.
5. **Mode changes:** Return to menu, enter a different scenario/mission and return. In the log, confirm several 1920×1080/800×600 generation changes with retirement of old Surface4 chains; visually confirm a correct screen after each change. These are internal rendering modes; the Windows desktop should retain its original resolution.
6. **Focus and Alt+Tab:** While in the menu and then during live gameplay, switch to another window and back **at least twice**. Check that the display repaints, input resumes, and there is no persistent white/black screen or crash. Give foreground focus time to settle before judging frame rate.
7. **Exit/restart:** Quit normally, relaunch once using the same active INI, verify no repeated destructive modification to the GOS archive and no loss of preferences, then exit normally again.
8. **GOS restoration (optional but recommended on a test copy):** If a native overlay was installed in this test and a valid original backup exists, set `GosVideoLock=restore`, relaunch to trigger restoration, and inspect that the original `crimptch.rof` was restored. Reset the setting to `off` or `on` intentionally afterward.

## Evidence to retain

- The exact source commit and `DllSha256` from `CrimsonSkiesFix/build-info.txt`, plus the first `CrimsonSkiesFix.log` line (version/short SHA).
- A simple statement of **pass/fail for each stage**; for a failure, the precise user action and whether an application window or error message appeared. The log does not necessarily emit a clean-shutdown marker.
- The complete log and optionally an image of any faulty display, **after reviewing/redacting local or identifying details**. The log may include hardware model, OS information and local path fragments.
- If audio or codecs fail on native Windows, document the codecs/environment separately. Do not classify native Windows gameplay as validated solely because the DLL loaded successfully in CI.

## Release decision

A single passing Windows 11 session on a supported EXE is a useful release gate, but it does not prove every Windows driver or monitor will work. If any blocking regression is observed, document it and create a focused fix PR with its own checks before freezing the release build.

The owner decides whether the result justifies a tagged **experimental prerelease** or a stable release. **No GitHub release, tag, merge or GOS archive manipulation is authorized by this document.**

legluondunet assisted by ChatGPT (OpenAI)
