# Wine and GE-Proton: diagnosing intro videos and cinematics

Scope: [issue #45](https://github.com/legluondunet/CrimsonSkiesFix/issues/45).
**Diagnostic guide, not a verified dependency list.** Quartz/DirectShow is a
possible explanation for missing or blank movies; whether Crimson Skies needs
native Quartz in a particular Wine/Proton build, or an additional decoder, is
**not yet confirmed**. Do not confuse `GOS-VIDEO-LOCK`/`VIDEO.SCRIPT` (game
settings UI) with Windows DirectShow movie playback. No dependency is installed
by CrimsonSkiesFix and Windows native users should not change their setup.

## Reproduce before changing anything

Use a **new test prefix** or a backup of the real prefix; leave your working
installation untouched. Record:

- Wine/GE-Proton version, game locale, graphics runner, Linux distribution,
  prefix architecture, and whether the test is plain Wine or Steam/GE-Proton.
- Which sequence fails (opening movie, other cinematic), whether there is
  sound, whether frames are black/frozen/skipped, and whether the menu/3D game
  still works.
- The launcher used (always `CrimsonSkiesFix.exe`, not the game EXE), its
  `--version` output, and current `[Debug] Log` setting.
- The root `CrimsonSkiesFix_installer.log` and `CrimsonSkiesFix.log` where
  available. By default the game log is produced on the **first successful
  installation launch only** and retained thereafter. To record another test,
  temporarily set `[Debug] Log=1` in `CrimsonSkiesFix.ini`, then restore
  your chosen setting.

For a standard Wine prefix, a possible **read-only diagnostic capture** is:

```bash
WINEPREFIX="$HOME/path/to/test-prefix" wine --version
WINEPREFIX="$HOME/path/to/test-prefix" WINEDEBUG=+loaddll,+quartz,+dshow \
  wine ./CrimsonSkiesFix.exe > wine-movie-before.log 2>&1
```

Replace the example prefix with your actual *test* prefix and run from the
launcher directory. Some Wine builds may not emit every diagnostic channel.
The `wine-movie-before.log` file may be large and may contain local paths;
review it before sharing. A DLL-load trace is **not** proof that an actual
movie graph decoded or presented frames.

## Compare only one variable

For **plain Wine**, if the first capture identifies a plausible DirectShow
problem and you are comfortable modifying that **test prefix only**, try:

```bash
WINEPREFIX="$HOME/path/to/test-prefix" winetricks quartz
```

Winetricks provides a `quartz` verb for `quartz.dll`. This is **not a codec
pack**, may obtain Microsoft redistributables, and is **not guaranteed** to
fix playback. Do **not** install unrelated codecs or multiple verbs at once.
Check winetricks' own messages for errors; if it fails, that is not proof of
a CrimsonSkiesFix bug. Re-run exactly the same cinematic and capture a second
log with the same environment, noting any difference. If a video still fails,
the required decoder/filter and Wine build remain unknown.

**GE-Proton / Steam / Protontricks:** do not point ordinary Wine/Winetricks at
an arbitrary Steam prefix. Confirm the *actual* compatdata prefix and runner
first; Steam/GE-Proton may manage overrides and media components differently.
Use an isolated copy/test prefix and the corresponding Proton-aware tooling
if applicable. Do not assume a plain Wine fix also works under Proton.

## Comparison checklist

| Observation | Before | After isolated Quartz test |
| --- | --- | --- |
| Runner, prefix and architecture | record | same |
| Opening movie: video / sound | record | record |
| In-game cinematic: video / sound | record | record |
| Menu and 3D gameplay | record | must not regress |
| DirectShow/Quartz DLL loading or COM errors | record | record |
| Installer and gameplay logs | save | save |

A useful result contains the **specific** movie that failed and the two
captures. It is equally useful if Quartz changes nothing. Only after
reproduction and diagnosis should the project consider an actionable runtime
warning; it must never claim that merely finding (or failing to find)
`quartz.dll` proves the decoder status.

References:
- [Winetricks source / quartz verb](https://github.com/Winetricks/winetricks/blob/master/files/verbs/dlls.txt)
- [Winetricks usage](https://github.com/Winetricks/winetricks/wiki/Usage)
- [Issue #45](https://github.com/legluondunet/CrimsonSkiesFix/issues/45)
