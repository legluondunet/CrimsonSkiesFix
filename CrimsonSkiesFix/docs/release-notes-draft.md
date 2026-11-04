# CrimsonSkiesFix — release candidate notes (draft, not published)

**Build currently under preparation:** `0.1.90` (Windows x86 DirectDraw proxy).
**Distribution policy:** experimental candidate, tested under Wine/GE-Proton; **Windows 11 gameplay NOT yet validated**.
**Release status:** not tagged, not released. The final candidate commit and SHA-256 must come from the approved `main` build, not from a PR artifact that may later be squashed.

## Highlights

- Native monitor-sized Crimson Skies 1.02 rendering, automatic FOV and HUD correction (including 1920×1080), preserving original 4:3 framing for videos and menus.
- Removal of the obsolete DirectDraw startup resolution/driver selection prompt for the verified executable.
- GameOS Video Preferences Graphics and Resolution controls locked using the game's native UI mechanism, preserving its mode/device tables; current native display resolution shown in the menu.
- The CrimsonSkiesFix build label appears only on the original main menu, including after returning from Preferences/gameplay.
- Native RGB565 first-image correction; focus repaint and `WM_ERASEBKGND` protection to reduce white-screen artifacts.
- Exact-hash-gated Wine-only ROFFILE.DLL allocation padding to avoid a known out-of-bounds INI parser read.
- Windows x86 DLL proxy with an independently verified PE32 DLL, three DirectDraw exports, and a downloaded-artifact integrity check.
- A clean package with `ddraw.dll`, configuration example, usage guide, complete GPL license and a `build-info.txt` containing the source SHA and compiled DLL SHA-256.

## Compatibility

Only a single validated **Crimson Skies 1.02** executable is accepted, size **2,580,480 bytes**, SHA-256:

`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`

Other protected, modified, or unknown executable versions are deliberately rejected. The DLL does not distribute a copy of the game or modify the verified EXE on disk.

**Verified in user gameplay tests:** Manjaro Linux with Wine 11.0 / Steam GE-Proton11-12 at 1920×1080, including in-game rendering, GOS menus, repeated resolution changes, return to menu/signature, and ROFFILE fix activation.

**Still pending:** real Windows 11 gameplay acceptance test (DLL loading in a GitHub Windows runner is not a game test); additional display/driver configurations; long-duration behavior and codec compatibility on Windows.

## Install and upgrade

1. Obtain the approved `CrimsonSkiesFix-ddraw-Windows-x86-EXPERIMENTAL` artifact from its GitHub Actions build.
2. Check `CrimsonSkiesFix/build-info.txt` against the included `ddraw.dll` using the offline verifier or `Get-FileHash`. This is an integrity check, **not** a signed binary.
3. Back up any existing `ddraw.dll` and copy the archive layout next to the verified game executable. Preserve your existing `CrimsonSkiesFix/CrimsonSkiesFix.ini`.
4. If you want a completely unmodified GOS Video archive, set `[Graphics] GosVideoLock=off` in a manually created active INI **before first launch**. Otherwise the default-on native lock backs up and modifies only the small original `GOSDATA/ASSETS/crimptch.rof` overlay.
5. To uninstall an already-installed native GOS Video lock, use `GosVideoLock=restore` while the DLL remains installed and **confirm restoration** before removing the proxy. Simply setting `off` does not restore an installed overlay.

Consult the detailed `CrimsonSkiesFix/README.md` in the artifact for full instructions, log paths and limits. This project is GPL-3.0-or-later and requires preserving applicable license/source information when redistributing.

## Release gates before publication

- [ ] PR approval and merge by the owner; final source/tag name chosen.
- [ ] GitHub Windows x86 build and downloaded-artifact verification green on **final** `main` SHA.
- [ ] Final binary hash, source commit and version logged in the candidate's distributed `build-info.txt`.
- [ ] Real gameplay session under Windows 11 with the exact candidate build, full scenario in `docs/windows11-acceptance-2026-11-04.md`.
- [ ] Explicit owner authorization for creating a GitHub tag and publishing an appropriately labelled release.

**Do not describe this draft as a published release or as proven stable under Windows.**

legluondunet assisted by ChatGPT (OpenAI)
