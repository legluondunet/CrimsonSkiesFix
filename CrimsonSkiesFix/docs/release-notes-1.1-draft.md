# CrimsonSkiesFix 1.1 — draft release notes

**Not published.** The current stable release is v1.0. Candidate baseline: 1.1.0-rc.1-3287914a, validated on 26 November 2026. Do not publish these notes as a completed release until the release gates are satisfied.

## What's new since 1.0

- One standalone launcher for five authenticated original CD editions (English USA/Europe 1.0, German 1.0, French 1.0, French 1.02 and English Europe 1.02). A separate decrypted game executable is no longer a user prerequisite for the supported media paths.
- CDs already in version 1.02 require only the original image beside the launcher. For earlier CDs, supply the authentic official Microsoft 1.02 update separately. The launcher verifies it and prepares installation offline; no download or historical updater execution by the launcher.
- Clear blocking-error notifications in both the terminal and native Win32 dialogs. Missing CD, duplicate CD images, missing/invalid update and multiple verified updater copies have actionable messages. The browser opens download instructions only after the user chooses Yes. Noninteractive automation suppresses dialogs.
- Consolidated architecture: `game/fix.dll` contains the compatibility code; native system WinMM remains untouched. Always launch through `CrimsonSkiesFix.exe`.
- Retains modern borderless fullscreen, widescreen FOV/HUD adaptation, original 4:3 menus and movies, optional native 4:3 gameplay, FPS limiting and simplified video settings. Original game cursor retained.
- New root configuration, documentation and state layout. Separate noncumulative installer and runtime logs. First-launch logging automatically returns to Log=0 after normal exit, when the installer-managed INI has not been customized.

## Installation

Use a new writable directory; automatic migration from the 1.0 layout is not supported. Put the launcher, exactly one supported CD image (ISO or CUE with its referenced tracks), and the official update if required, in this directory. Run the launcher. Keep original media and update available for future repair. Do not run game/CRIMSON.EXE directly. No game files or Microsoft updater bundled in the launcher.

## Validation and limitations

Real installation/launch evidence exists for all five profiles across different candidate revisions and operating systems. Latest 3287914a confirmed under Wine 11.19 with German 1.0 + official update: compact dialogs, browser opening, fresh install, verified EXE and data, main menu, exit 0 and final Log=0. CI 45904563284: 341 runtime + 89 installer tests, Windows x86 and Wine32 success. This is not exhaustive Windows/Wine/Proton coverage, nor evidence of all gameplay or multiplayer behaviors. No claim of CD-only conversion for the earlier discs. See the main README for update sources, supported inputs and current limitations.
