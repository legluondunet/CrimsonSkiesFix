# CrimsonSkiesFix 1.1 — WinMM test candidate (NOT A RELEASE)

This archive contains `CrimsonSkiesFix-WinMM-1.1-TEST.exe`, the x86
standalone installer/launcher for the *unreleased* 1.1 WinMM runtime.
The published **1.0** release still uses `ddraw.dll`; this candidate
embeds and installs `winmm.dll` instead. It ships no copyrighted game
files, disc images, or unprotected executable.

**Only test on a copy of your installation.** No v1.1 release has been
published, and this candidate should not replace the published v1.0
installer while migration from older or third-party `ddraw.dll`
wrappers is still being assessed.

## Graphics behavior in this candidate

- Original 1.02 decrypted game EXE is SHA-256 authenticated before the
  in-memory widescreen/FOV transformation; the on-disk EXE is unchanged.
- `AspectRatio=auto` uses the verified native monitor resolution for 3D,
  with the *upstream camera height/width fix automatically enabled*.
  The game's correct horizontal FOV is preserved while its vertical
  projection is calculated using the current 3D raster ratio.
  **`Log=1` is not required to enable the fix.**
- `AspectRatio=4:3` draws genuine native-resolution 4:3 3D centered
  on the desktop (for example 1600×1200 on a 1920×1200 display).
  It preserves the original 4:3 camera ratio.
- Menu and movie surfaces keep their original 800×600 and video
  dimensions and their centered 4:3 composition. They are not made
  widescreen by scaling the original source pixels.
- Graphics Preferences displays the **effective, authenticated 3D
  resolution**, for example 1600×1200 (4:3), 1920×1200 (16:10),
  or 1920×1080 (16:9). The GOS mode table, mode selection and native
  menu/movie surfaces are unchanged.
- Abandoned development keys `ExperimentalHudFov43Probe`,
  `ExperimentalHudEntityY43Probe`, `ExperimentalHudCanvas43Probe`,
  `ExperimentalProjectedY43Probe`, `ExperimentalY43ReticleGuard`
  and `ExperimentalUpstreamAspectProbe` are **ignored** if they remain
  in an existing INI. No old user INI is overwritten or rewritten.

The functional camera/FOV correction, aircraft proportions, HUD gauges,
reticle, objective indicators and absence of black-polygon regressions
have been tested on selected Windows 11 and GE-Proton configurations.
That is not a guarantee for every GPU, monitor, scene or Wine build.

## Safe installer / runtime upgrade policy

- A local `ddraw.dll` blocks installation/repair by this test build;
  it will **not** overwrite or remove v1.0 or third-party wrappers.
  Use a separately prepared installation without an app-local
  `ddraw.dll` until a supported migration path exists.
- The approved 1.02 unwrapped `CRIMSON.EXE` and game assets remain
  hash-checked. The required original disc data and official 1.02
  update are not included.
- If the current regular `winmm.dll` differs from the embedded one,
  the installer replaces it **regardless of the previous DLL's hash**.
  The exact previous active DLL becomes `winmm.dll.bak`. Any old
  `.bak` is replaced, leaving **only one backup**.
- Backup replacement is transactional: an old `.bak` may temporarily
  become `.bak.previous` during replacement, then is removed after
  success. Interrupted transactions and unsafe file types stop rather
  than being silently overwritten.
- If the current `winmm.dll` already matches the embedded one, the
  installer does nothing to it or its backup. If no active DLL exists,
  a previously existing backup is preserved.
- Normal launch and `--install-only` use the safe upgrade path.
  `--verify` checks the WinMM installation.
  `--launch-no-verify` starts the game **without verifying or
  repairing files**.
- The INI is `CrimsonSkiesFix/CrimsonSkiesFix.ini`; installer and
  gameplay logs are separate:
  `CrimsonSkiesFix_installer.log` and `CrimsonSkiesFix.log`.

## Basic configuration

```ini
[Graphics]
AspectRatio=auto
FPSLimit=60

[Debug]
Log=0
```

Set `AspectRatio=4:3` to test native 4:3 gameplay. Use
`Log=1` only to collect diagnostics, not to activate widescreen
graphics or camera corrections. When testing the default configuration,
**keep `Log=0`** and look for correct aircraft/HUD aspect, no torn
terrain or objective markers, and normal transitions back to the menu.

## Standalone binary / CI identity

The experimental Actions workflow
[`winmm-installer-candidate.yml`](../../.github/workflows/winmm-installer-candidate.yml)
builds and embeds the WinMM DLL, runs x86 tests and publishes an artifact
named `CrimsonSkiesFix-WinMM-1.1-TEST-ONLY-NOT-RELEASE`.

The installer in this ZIP is
`CrimsonSkiesFix-WinMM-1.1-TEST.exe`, *not* the v1.0
`CrimsonSkiesFix.exe`. The binary's `--version` output must contain
`1.1.0-rc.1` and `runtime=winmm.dll EXPERIMENTAL`; if it says
`1.0.0` or `runtime=ddraw.dll`, do **not** test that binary as 1.1.
The ZIP includes a SHA-256 manifest for the experimental EXE.

Optional Wine/Proton launch commands on a copied game folder:

```bash
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --version
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --install-only
WINEDLLOVERRIDES='winmm=n,b' wine CrimsonSkiesFix-WinMM-1.1-TEST.exe --verify
```

The temporary Wine override does not write to the prefix registry.

## Remaining release gates

Before merging/publishing 1.1, compare the **default `Log=0`**
graphics in both auto and 4:3 modes; check another mission, menu,
pause, cutscene, audio and Alt-Tab under Windows and Wine/Proton.
Also review v1.0-to-WinMM migration policy and installer behavior on
installations containing local `ddraw.dll`. Green x86 CI alone does
not validate visual rendering on actual graphics drivers.
