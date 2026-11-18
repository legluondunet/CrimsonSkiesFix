# Wine and GE-Proton: diagnosing intro videos and cinematics

Scope: [issue #45](https://github.com/legluondunet/CrimsonSkiesFix/issues/45).
**Diagnostic guide, not a verified dependency list.** Quartz/DirectShow is a
possible explanation for missing or blank movies; whether Crimson Skies needs
native Quartz in a particular Wine/Proton build, or an additional decoder, is
**not yet confirmed**. Do not confuse `GOS-VIDEO-LOCK`/`VIDEO.SCRIPT` (game
settings UI) with Windows DirectShow movie playback. No dependency is installed
by CrimsonSkiesFix and Windows native users should not change their setup.

## Verified current Lutris behavior and the game's movie assets

The owner's [Crimson Skies Lutris installer: CD + d7vk + fixes](https://lutris.net/games/install/40026/view)
contains exactly:

```yaml
- task:
    app: amstream quartz
    description: Installing codecs...
    name: winetricks
    silent: true
```

This is *not* a request for standalone video codecs. The official Winetricks
`src/winetricks` recipes extract **Microsoft's** `amstream.dll` and
`quartz.dll` from Windows 7 SP1 downloads, register them with
`regsvr32`, and set the DLL priority to **native,builtin**. These files are
not part of CrimsonSkiesFix and **must not** be bundled as if freely
redistributable. Replacing Winetricks with a silent downloader of the same
files is not the intended solution.

Our verified CD manifest `installer/profiles/c_skies_disc_001.tsv` contains:
- `GOSDATA/ASSETS/GRAPHICS/MPG/MSOPEN1.MPG`
- `GOSDATA/ASSETS/GRAPHICS/MPG/CHAP0.MPG` through `CHAP5.MPG`
- `GOSDATA/ASSETS/GRAPHICS/MPG/FINAL.MPG`, `ZIPPER.MPG`, `CRIMFLAG.MPG`

The independent [static-recompilation project](https://github.com/sp00nznet/crimsonskies)
describes the game's .MPG assets as **MPEG-1 cutscenes**, and lists imports
from `AVIFIL32.dll`, `WINMM.dll` and `ole32.dll`. This is promising
evidence, **not** a proof of individual video/audio stream codecs nor of the
exact game playback call path. Inspect one authenticated locally owned MPG
with `ffprobe` to verify both stream codecs before choosing a
self-contained decoder. `AVIFile` imports do not by themselves prove
that cutscene playback uses AVIFile rather than DirectShow/AMStream.

## Target: no Winetricks and no prefix mutation

1. **First try Wine/Proton's own** `amstream` and `quartz` (DLL override
   `builtin`) and its GStreamer/FFmpeg-capable media backend, in an
   *isolated clean prefix*, with no native Microsoft libraries. Do not treat
   a configured old Lutris prefix as evidence of independence: it may already
   have native DLLs and registered COM filters.
2. If those builtins fail, **identify the game call path**, including any
   `CoCreateInstance` and `IAMMultiMediaStream` usage, via bounded diagnostic
   hooks or offline analysis of the approved EXE, with no persistent game
   changes. Record a precise HRESULT / DLL / playback transition.
3. If Wine's COM/DirectShow route cannot support the verified game, evaluate a
   **self-contained, portable MPEG-1/MP2 decoder** bundled in the project's
   existing runtime DLL (e.g. MIT-licensed
   [PL_MPEG](https://github.com/phoboslab/pl_mpeg)). This would still require
   a safe interception of the **actual game video player entrypoint**, and
   game-compatible presentation, A/V sync, skip/stop, alt-tab and lifecycle.
   The decoder alone **does not provide** any DirectShow COM replacement.
   PL_MPEG does not cover MPEG-2 streams; confirm the asset codecs first.
4. Keep Windows native playback untouched. No automatic invocation of
   Winetricks or Protontricks, installation of Microsoft binaries, global
   registry changes, or prefix writes. Retain a safe fallback to the current
   game playback route until the integrated option is verified.

This is an engineering roadmap, **not a claim that an integrated player is
implemented**. No new INI switch should be exposed until the playback
entrypoint is understood and a working prototype exists.


## One-command interactive Wine A/B diagnostic (PR #51)

The companion [pr51-wine-video-diagnostics.sh](../tools/pr51-wine-video-diagnostics.sh)
script automates a controlled comparison on **ordinary system Wine**. It
copies the installed game to a temporary directory, initializes **two new
independent Wine prefixes** (default and Wine builtins only), captures the
Wine/COM loading trace, and optionally uses `ffprobe` to identify video and
audio codecs in `MSOPEN1.MPG` and `CHAP0.MPG`. Both runs are interactive:
inspect the image and sound and exit the game to continue to the next test.

From a terminal on the Linux computer, after downloading the script from the
PR branch and checking its contents, run:

```bash
bash pr51-wine-video-diagnostics.sh /chemin/vers/crimsonskiesfix
```

Pass the directory containing `CrimsonSkiesFix.exe` and `game/CRIMSON.EXE`.
No game ISO, official updater, native Microsoft DLLs or Winetricks verbs are
needed for this **already-installed-game** comparison. The original game,
saves, root INI and existing Wine prefix are not modified. The temporary game
copy and the two temporary prefixes are deleted when the script finishes;
the diagnostic report folder remains **beside the original installation**.
The free disk space needed for the temporary copy can be substantial.

Send `environment.txt`, `streams.txt`, `observations.txt` and the
compressed `wine-default.log.gz` / `wine-builtin.log.gz` files; game and
installer logs are copied separately when present. Check for private local
paths before uploading. The test does not prove Proton behavior: Proton
must be tested with the correct runner afterward.

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

For a standard Wine installation, compare **two clean-prefix runs** before
considering any Microsoft DLLs. The second run requests **only Wine builtins**
for the two multimedia DLL names (no persistent override or registration):

```bash
export WINEPREFIX="$HOME/path/to/test-prefix"
wine --version
WINEDEBUG=+loaddll wine ./CrimsonSkiesFix.exe \
  > wine-video-default.log 2>&1
WINEDLLOVERRIDES='amstream=b;quartz=b' WINEDEBUG=+loaddll \
  wine ./CrimsonSkiesFix.exe > wine-video-builtin.log 2>&1
```

Use your own **clean test prefix** and game folder, not the literal example
path. Check both **picture and audio** in MSOPEN1.MPG / a chapter transition.
Only one thing should change between runs: the override. The logs can be
large and expose file paths; review them before sharing. Wine DLL-loading
logs are not evidence of successful playback. If both runs work, there is
no reason to ship/install native Microsoft DLLs for that runner. If both
fail, inspect missing 32-bit media plugins, video filter availability and
the actual game video call path before selecting a workaround. The host
GStreamer plugin set and runner package may differ significantly across
Wine, Lutris Flatpak, and GE-Proton.

For the **codec identity only**, on Linux with ffprobe available, inspect
one file from *your own installed game*:

```bash
ffprobe -v error -show_entries stream=index,codec_type,codec_name \
  -of compact "game/GOSDATA/ASSETS/GRAPHICS/MPG/MSOPEN1.MPG"
```

Neither ffprobe nor GStreamer plugins are required by the **CrimsonSkiesFix
installer** today; this is a development-time investigation.

**GE-Proton / Lutris:** do not run a system Wine executable on an unrelated
Proton prefix. Keep the same runner and use Lutris' per-game environment
setting to supply `WINEDLLOVERRIDES=amstream=b;quartz=b` for a temporary
test. This is a launch-only override, not a permanent Wine registry change.

## Comparison checklist

| Observation | Clean default | Clean + builtin override |
| --- | --- | --- |
| Runner, prefix and architecture | record | same |
| Opening video (MSOPEN1.MPG): picture / sound | record | record |
| Chapter movie: picture / sound | record | record |
| Menu and 3D gameplay | record | must not regress |
| Wine DLL loading, COM errors and media plugins | record | record |
| Game and installer logs | save | save |

A useful result identifies the **specific** missing/failing movie and both
captures. If builtins alone cannot play them, only then begin prototyping a
portable decoder and correct game-hook integration. Never assume
`quartz.dll` or `amstream.dll` being present proves a working
decoder or COM graph.

References:
- [Current Lutris install script (amstream + quartz)](https://lutris.net/games/install/40026/view)
- [Winetricks implementation (Microsoft amstream/quartz + native,builtin)](https://github.com/Winetricks/winetricks/blob/master/src/winetricks)
- [Original disc file identity manifest](../installer/profiles/c_skies_disc_001.tsv)
- [Crimson Skies recompiler format and import inventory](https://github.com/sp00nznet/crimsonskies)
- [PL_MPEG MPEG-1/MP2 library, MIT](https://github.com/phoboslab/pl_mpeg)
- [Issue #45](https://github.com/legluondunet/CrimsonSkiesFix/issues/45)
