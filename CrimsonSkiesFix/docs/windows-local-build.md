# Local Windows x86 build when GitHub Actions minutes are exhausted

The owner confirmed on 2026-11-07 that the account's included
GitHub Actions compute allowance is exhausted for **private repositories**.
The **public** SpiderMan2k1Fix repository still runs its standard hosted
runners for free. CrimsonSkiesFix is currently **private**; its jobs that fail
before their first step are **not evidence of a Rust compiler error**.
Keep the PR review
process and branch protection intact; produce test builds **on the user's
native Windows 11 PC** instead, with source commit and SHA-256 recorded.

This is a local substitute for the build/test/artifact part of CI; it does
**not** automatically prove that any game works or that a release is stable.

## Prerequisites

- Native Windows 11, Windows PowerShell 5.1+ or PowerShell 7.
- Git for Windows (authenticated to the repository).
- Rust stable installed through rustup, with `cargo` and `rustup` on PATH.
- **Microsoft Visual Studio C++ Build Tools**, MSVC x86 target tools and
  Windows SDK. If `link.exe` cannot be found, use the **x64_x86 Cross Tools
  Command Prompt for VS** (launch `powershell` from there), or the matching
  Developer PowerShell. A Rust target installation alone does not install MSVC
  linker tools and Windows SDK.
- Python 3, available as `python` or the Windows `py -3` launcher.

### First clone / choose the PR

From a terminal signed into GitHub:

```powershell
git clone --branch diagnostics/windows-native-ddraw-startup-20261106 https://github.com/legluondunet/CrimsonSkiesFix.git
cd CrimsonSkiesFix
```

If the repository is already cloned:

```powershell
git fetch origin
git switch diagnostics/windows-native-ddraw-startup-20261106
git pull --ff-only
```

Check that this is the **PR #23 branch** and not `main`.
The build script refuses modified tracked source files so the generated manifest
is tied to a reproducible committed source SHA.

### Build and verify locally

Use a Visual Studio C++ developer terminal with the MSVC linker:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\CrimsonSkiesFix\tools\build_windows_local.ps1
```

This script runs the same core steps as the Windows/Ubuntu GitHub workflow:

1. Add the Rust `i686-pc-windows-msvc` target and compile unit tests.
2. Build the actual release `ddraw.dll` and independent x86 loader smoke binary
   with static CRT setting matching the workflow.
3. Inspect **all 22 DirectDraw names and canonical ordinals** in the resulting
   DLL. Reject 3-export images and unsafe PE forwarders.
4. Load and unload the DLL on native Windows, checking `GetProcAddress` by
   both name and ordinal. This does **not** start the game.
5. Stage exactly the normal five-file release-candidate layout including GPL
   license and a `build-info.txt` with source SHA and DLL SHA-256.
6. Run the independent offline release verifier and create a ZIP named like
   `CrimsonSkiesFix-local-<commit8>.zip`.

The folder and ZIP are created **beside the cloned repository**, not in the
game directory. The script never overwrites an existing candidate or any
active game INI, GOS resource, or `ddraw.dll`.

**If any step fails, STOP.** Send the first error from the PowerShell output
and the source commit; do not copy an unverified DLL into the game directory.

## Controlled native Windows gameplay acceptance

Only after `LOCAL-CI: PASS`, keep backups of the known-good 22-export bridge
and `CrimsonSkiesFix.backend.dll`, then test the **new single-Rust** DLL from
the local ZIP. Do not distribute the two-DLL diagnostic bridge as a release.

First use the previously successful reduced-graphics diagnostic INI with
`[Debug] Log=1`; verify `Accelerator` selection, actual mission play,
and a fresh CrimsonSkiesFix log. Then restore intended defaults in stages:
native widescreen/FOV/HUD, windowed adapter, borderless, GOS UI, first image,
video playback, Alt+Tab and clean exit. Retest under Wine/Proton after any
Windows-specific changes.

Report the exact `build-info.txt` source SHA and DLL SHA-256 with each log.
No merge, tag or stable release before the final actual build is validated.

The independent successful **22-export no-hook control** and the successful
**22-export bridge + original Rust backend mission** are important diagnostic
baselines, but do not substitute for running the proposed PR's final Rust DLL.
