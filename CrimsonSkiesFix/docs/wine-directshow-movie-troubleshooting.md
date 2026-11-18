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

This experiment uses an **already installed dedicated test game directory**.
Place the [pr51-wine-video-diagnostics.sh](../tools/pr51-wine-video-diagnostics.sh)
script **beside** `CrimsonSkiesFix.exe` (or `CrimsonFix_installer.exe`),
with `game/CRIMSON.EXE` in the same installation root.
No additional game copy is made.

In a Linux terminal, **change to the launcher's directory** and download
the PR branch script **into that same directory**:

```bash
cd "/chemin/vers/crimsonskiesfix"
curl -fL "https://raw.githubusercontent.com/legluondunet/CrimsonSkiesFix/docs/wine-directshow-video-diagnostics-20261117/CrimsonSkiesFix/tools/pr51-wine-video-diagnostics.sh" \
  -o pr51-wine-video-diagnostics.sh
bash ./pr51-wine-video-diagnostics.sh
```

**For the current Wine 11.17 MainMenu failure**, the first A/B test has
already shown both cases used Wine builtins and behaved the same. Use the
new **single run** with more precise DirectShow/AMStream tracing instead:

```bash
bash ./pr51-wine-video-diagnostics.sh --menu-only
```

Before this test, set `[Debug] Log=1` in the test installation's
`CrimsonSkiesFix.ini`; the diagnostic script does not rewrite INI options
automatically. Watch the two intro movies and whether the MainMenu becomes
visible, then exit the game. The script stores both the extended Wine trace
and bounded `PR51-MENU-FLIP` / `PR51-MENU-UNLOCK` runtime messages if the
new compiled `fix.dll` has been deployed by the latest launcher.
It still uses a fresh prefix inside the same installer directory.

The script derives the installation directory from its own file location,
so there is **no path argument and no `--in-place` flag**. It creates
`PR51-diagnostics-XXXXXXXX/` **inside that same directory**, including
temporary isolated Wine prefixes, saved logs, codec metadata, and copies of
the existing root INI and root logs. At completion it deletes only its own
temporary prefixes, and creates a matching `PR51-diagnostics-XXXXXXXX.tar.gz`
archive **alongside the launcher** for easy sharing. It does not download
Winetricks, native DLLs, or a second installation of the game.

The two test launches run against the existing test installation.
**Do not run this script from a production game installation.**
The launcher can change INI, logs, fix.dll and game state. Root INI/logs
are backed up in the local diagnostics folder, but **game data and saves
are not backed up**. Exit the game after each run and enter your
observations in the terminal. The script uses two disposable prefixes
of your system Wine (not GE-Proton). Comparing picture and sound for
MSOPEN1.MPG and a chapter transition is essential; Wine loading logs alone
are not proof of playback.

The script optionally uses `ffprobe` on installed MPG files to identify
the exact video and audio codecs. Review diagnostic logs for private paths
before uploading the `.tar.gz` file. For GE-Proton, follow a separate
runner-specific test using its own toolchain.

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

## Upstream fix for the Win32 AMStream video flush deadlock (Wine 11.18)

**Strongly supported hypothesis from the user's November 18 black-menu run:**
the verified 11.17 Wine trace shows the video thread blocked at
`ddraw_mem_allocator_GetBuffer()` in `amstream`, while the game
thread requests `multimedia_stream_Seek(..., 0)` and waits for
`winegstreamer/quartz_parser.c: pin.flushing_cs` held by the video
thread. It is not a missing MPEG-1 decoder: both intro videos play and
the menu music continues.

The exact upstream fix is already present in
[Wine 11.18 `dlls/amstream/ddrawstream.c`](https://github.com/wine-mirror/wine/blob/wine-11.18/dlls/amstream/ddrawstream.c).
In 11.17, `ddraw_mem_allocator_GetBuffer()` waits indefinitely when
the stream is committed but no sample is free. Wine 11.18 changes the
wait predicate to stop when `stream->flushing` becomes true **AND**
adds `WakeAllConditionVariable(&stream->allocator_cv)` inside
`ddraw_sink_BeginFlush()`. This pair of changes is essential:
merely adding the `flushing` predicate without waking the allocator
would not break an existing wait.

See [official Wine commit 7c48881f](https://gitlab.winehq.org/wine/wine/-/commit/7c48881f4fe2edba88f70b2e1b0892d),
merged September 11, 2026. Wine 11.18 was released September 18, 2026.

**Next validation:** run the same game installation using Wine 11.18 or
newer, **unchanged** fix/installer binary and clean temporary prefix,
without Winetricks or Microsoft native DLLs. The test script
[pr51-wine-1118-validation.sh](../tools/pr51-wine-1118-validation.sh)
is placed and run beside the installer; it enforces Wine >= 11.18,
uses a fresh prefix stored **inside the installer directory**, backs up
root INI and logs, tests video + menu visibility, then deletes its
temporary prefix and writes `PR51-wine1118-XXXXXXXX.tar.gz` in the
same directory.

```bash
curl -fL "https://raw.githubusercontent.com/legluondunet/CrimsonSkiesFix/docs/wine-directshow-video-diagnostics-20261117/CrimsonSkiesFix/tools/pr51-wine-1118-validation.sh" -o pr51-wine-1118-validation.sh
bash ./pr51-wine-1118-validation.sh
```

For the current Manjaro/Lutris test, the script now defaults to this
**explicit Wine-staging 11.22 runner** (not the system `wine` binary):

```text
/home/romain/.local/share/lutris/runners/wine/wine-11.22-staging-amd64/bin/wine
```

It resolves `wineboot` and `wineserver` **from that same runner's
`bin/` directory**, checks all three executables, and passes consistent
`PATH`, `WINELOADER`, and `WINESERVER` settings. The version test
runs the runner's own `wine --version`. The user can override the path
without editing the script by setting `PR51_WINE_BIN` to another full
Wine executable path. No system Wine installation/update is required.

Run **only in the dedicated test installation**; the installer/game
may update game files or save data and the script does not back up
those data. This experiment changes **only the Wine version** relative
to the prior Wine 11.17 reproduction. If it passes, first consider
documenting the fixed Wine version rather than shipping an invasive
application-side seek/COM hook. Do not declare Crimson Skies fixed
until a real video-menu test confirms it.

### Wine-staging 11.22 regression test (November 18, 2026)

The first dedicated Lutris Wine 11.22 trace
(`PR51-wine1118-Nxn9eGml.tar.gz`) reported `UserObservation=crash`.
The runtime reached the 800x600 movie/menu graph and performed its
first virtual-primary Flip, but it **never logged MainMenu selection**.
Wine then reported a null-read page fault at **0x005A928F** on thread
0x01c4, shortly after Quartz filter and AMStream connection operations.
This is **not** the same point reached in the previous 11.17 black-menu
deadlock, and is not evidence that the 11.18 upstream flush fix is
effective (or ineffective) for this game.

The test accidentally enabled **`+sync` Wine tracing**, producing
**2,196,350 lines** in seconds; this is high-overhead instrumentation
and can perturb timing. The validation script now defaults to
**`WINEDEBUG=-all`** for an uninstrumented baseline, records the
real Wine-launcher exit status, and only stops the dedicated prefix
if still running. Advanced tracing is opt-in via `PR51_WINEDEBUG`.
Re-run the unchanged game binary with the same Lutris Wine 11.22
runner and a new isolated prefix. If the fault remains at 0x005A928F,
investigate its game-side call chain independently of the 11.17 flush
deadlock; a game executable address is not a demonstrated Wine fault.

**WineDbg caveat:** the previous `pr51-winedbg-hang.sh` run found the
game PID, but `winedbg-stack.txt` contained a WoW64
`c0000005` exception instead of usable `CRIMSON.EXE` thread stacks.
Do not repeat that same debugger test on Wine 11.17. A similar
WineDbg WoW64 issue was reported as Wine bug #59827 in 2026.

## Capturing the blocked Wine threads (PR #51 next experiment)

The November 18 Wine 11.17 trace establishes two separate seeks:
one to 1.102 seconds which continues, and a later call
`multimedia_stream_Seek(..., 0)` followed by `GST_Seeking_SetPositions`.
The later path blocks with thread 0x01ac waiting on
`winegstreamer/quartz_parser.c:pin.flushing_cs` held by thread 0x0244.
At this point the user sees a **black MainMenu with music still playing**
after two correctly displayed opening videos. This is consistent with an
in-flight synchronous flush, but **does not alone prove a complete deadlock**.
Avoid seeking/no-flush patches until the thread stacks confirm the route.

[pr51-winedbg-hang.sh](../tools/pr51-winedbg-hang.sh) runs one clean
Wine prefix and captures the game’s live thread backtraces after the
user observes the black menu. It does not require recompilation and
uses Wine's own `winedbg` tool rather than Microsoft binaries.

Run it from a terminal opened **in the dedicated test installation's
directory**; download the script into that directory:

```bash
curl -fL "https://raw.githubusercontent.com/legluondunet/CrimsonSkiesFix/docs/wine-directshow-video-diagnostics-20261117/CrimsonSkiesFix/tools/pr51-winedbg-hang.sh" -o pr51-winedbg-hang.sh
bash ./pr51-winedbg-hang.sh
```

Once the two movies have finished, return to the terminal and press
**Enter while the black menu and background music are still present**.
WineDbg locates the game by its Windows process ID (not Linux PID),
attaches to that process, invokes `info thread` and `bt all`,
and detaches. It writes the process inventory and call stacks to
`PR51-hang-XXXXXXXX/` in the **same installer directory**, then
stops only that freshly created test prefix and produces an archive
`PR51-hang-XXXXXXXX.tar.gz` beside the installer.

The script backs up the root INI and logs before launch; as with the
existing in-place experiment, it **does not back up installed game data
or saves** and should not be used on a production installation. The
debugger may temporarily pause the game, and can fail if the host has
debugger restrictions or WineDbg lacks symbols; failure and inventory
details are preserved without making a false success claim. Inspect
the archive for private paths before sharing.

Wine debugger command syntax is documented in the
[WineDbg manual](https://man.archlinux.org/man/winedbg.1.en);
`--file`, `attach`, `info thread`, `bt all` and `detach` are
supported in Wine 11.x.

## Menu transition-specific runtime diagnostics (PR #51)

With `[Debug] Log=1`, the experimental unified `fix.dll` now logs a
bounded series of `PR51-MENU-UNLOCK` and `PR51-MENU-FLIP` events after
the original MainMenu script is selected. These are diagnostic-only:
no changes to DirectDraw, COM, rendering order, or return values.

- `PR51-MENU-UNLOCK`: a native surface Unlock completed, **not** a
  guarantee that a frame reached the screen.
- `PR51-MENU-FLIP: enter`: a game virtual primary Flip was called
  after MainMenu selection. An absent matching `return` suggests
  an in-flight presenter call or lock needs investigation.
- `PR51-MENU-FLIP: return`: the physical Blt completed, although actual
  pixels and scanout remain unverified.
- No post-menu `PR51-MENU-FLIP` indicates the problem is upstream of
  the game virtual primary Flip, possibly in multimedia graph teardown
  or menu rendering. It does **not** prove a specific deadlock.

The November 18 baseline showed post-menu surface Unlock sampling but
no later *periodic* primary-Flip log. Previous logs emitted only the
first and every 120th Flip, so a few Flips could have been missed. This
new test distinguishes those cases without adding a video decoder.

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
