# Native Windows 11 acceptance protocol — CrimsonSkiesFix (future test)

**Status (12 November 2026): NOT validated for the current experimental standalone candidate.** Earlier native Windows 11 tests used an older DLL and explicit compatibility INI; those results do **not** establish that the current default-on in-memory GOS VIDEO.SCRIPT redirection or full auto-install works on Windows. The GitHub Windows CI validates PE32 loading and all 22 DLL export names/ordinals, **not gameplay**.

This protocol documents a later optional acceptance test; unavailability of a native Windows machine does not prevent preparing or publishing a **clearly labelled experimental, GE-Proton-tested prerelease** if the owner approves.

## Preparation (safe and reversible)

- Back up the original game directory, especially `SavedGames/`, `Planes/`, the current `CrimsonSkiesFix.ini` and `GOSDATA/ASSETS/crimptch.rof` with any existing `*.original.bak`.
- Use the **one-file** artifact `CrimsonSkiesFix.exe` from a successful CI run on the exact final source SHA, not an older five-file DLL download. The launcher embeds the tested x86 `ddraw.dll`, which it extracts into the game directory.
- If installing from disc, put one supported genuine ISO/CUE-BIN (FR/DE verified under GE-Proton; USA remains untested in gameplay), the official 1.02 updater and your own exact approved decrypted 1.02 EXE next to the launcher. For a preinstalled verified game, the original media need not be present.
- Supported unmodified decrypted `CRIMSON.EXE`: **2,580,480 bytes**, SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`. Do not supply third-party widescreen executables.
- Use a test copy of the installation. Do not remove arbitrary `.rebuilding`, `.installing`, `.previous` or GOS `.bak` paths to bypass safety checks.
- A completed run creates `CrimsonSkiesFix/CrimsonSkiesFix.ini` (active INI inside the game folder), `CrimsonSkiesFix/ddraw.dll`, `CrimsonSkiesFix/build-info.txt`, and **two independent logs** beside the standalone launcher: `CrimsonSkiesFix-installer.log` (latest meaningful installer/repair report) and `CrimsonSkiesFix.log` (fresh DLL game log per session). The downloadable archive itself contains only the launcher EXE. To request detailed gameplay logging, set `[Debug] Log=1` in that installed active INI.

## End-to-end test — one launch and one repeat launch

1. **Installation / verification:** run `CrimsonSkiesFix.exe` without parameters. Confirm a first-time install reports `INSTALL_FROM_CD_AND_LAUNCH` or that an existing verified install reports `VERIFIED_EXISTING_AND_LAUNCH`; no unverified game may launch. Confirm the original protected EXE backup is preserved.
2. **Startup:** no obsolete driver/resolution chooser, no unexpected DDRAW error, correct RGB565 opening image and properly framed video and audio.
3. **Menu and GOS Video:** the brand signature appears only in MainMenu. Graphics and Resolution options are visibly unavailable; Resolution text reflects actual monitor size; unrelated Video settings can still be changed. Return to MainMenu and confirm signature restoration.
4. **3D:** play several minutes in a mission. The 3D world must fill the monitor at the correct FOV with a properly proportioned HUD; visual colours, cursor/input and audio must work.
5. **Mode transitions:** go into/out of missions and settings. The game may create virtual 1920×1080/800×600 surfaces, but the Windows desktop resolution must not change. Check for white sidebars, freeze, wrong scaling or lost input.
6. **Alt+Tab:** from menu and from live gameplay, switch away and back twice, check repaint and foreground input. Verify cutscene transitions if encountered.
7. **Exit and repeat:** exit normally; the installer report has `GAME_EXIT_CODE=0` on first install or repair, and the launcher prints the status to the console on subsequent verified-only launches. Relaunch using the same EXE and active INI; confirm it verifies the previous game without rewriting the GOS archive or re-extracting media, and that `CrimsonSkiesFix-installer.log` retains its previous successful install report. Exit cleanly again.
8. **Optional GOS restore on a backed-up test copy:** use `GosVideoLock=restore` in the installed `[Graphics]` section, launch once to restore the original small overlay after verification, then deliberately choose `off` or `on`. Setting `off` alone does not restore the overlay.

## Evidence and decision

- Keep the **full final source commit and `DllSha256`** from `CrimsonSkiesFix/build-info.txt`, and the matching game initialization entry in the **DLL-only** `CrimsonSkiesFix.log`. Read the separate `CrimsonSkiesFix-installer.log` for installation/repair data; an ordinary verified-only launch must not overwrite it.
- Record the observed pass/fail conditions, operating system, screen resolution/DPI, GPU and launcher build label. Do not confuse internal API callback rates with actual display FPS.
- Windows runner tests and previous Windows 11 gameplay sessions on older builds are not evidence for this exact candidate.
- Native Windows runtime acceptance or a claim of stable Windows compatibility requires a real test of the same final binary. Any experimental prerelease published without this test must state the limitation prominently.

This document grants **no permission** to merge PRs, create tags, alter the user's game files or publish a release; those actions require the owner's explicit approval.

legluondunet assisted by ChatGPT (OpenAI)
