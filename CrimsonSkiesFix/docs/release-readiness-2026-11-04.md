# CrimsonSkiesFix — release readiness audit (2026-11-04)

**Target:** experimental `0.1.90` Windows x86 proxy, after owner-validated PR #18 and the packaging/provenance PR #19 merged into `main` (`ebed26ceaf340b6506f0c220c1998ce318b36ed4`). This remains a release-candidate preparation audit, not a declaration of a stable 1.0 release.

## Findings and dispositions

| Area | Evidence | Decision |
| --- | --- | --- |
| Game version gate | The DLL accepts only the fixed 2,580,480-byte Crimson Skies 1.02 SafeDisc-free EXE with SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`. | Keep exact admission rule; do not support arbitrary unpacked or patched binaries. |
| DLL architecture | CI builds `i686-pc-windows-msvc`, verifies PE `IMAGE_FILE_MACHINE_I386`, and exercises `LoadLibraryW`/`GetProcAddress`/`FreeLibrary` on a Windows runner. | Maintain as mandatory automated gate; it is NOT native Windows 11 game-play testing. |
| Source and license in downloadable artifact | Previous archive shipped `ddraw.dll`, `README.md`, `CrimsonSkiesFix.ini.example`, but omitted the repository's GNU GPL-3 full text. | Package `CrimsonSkiesFix/LICENSE` copied verbatim from the repository and verify file digests. |
| Per-build provenance | Different PR builds published an identical artifact name; a previous game log was accidentally collected from an older DLL. | Package `CrimsonSkiesFix/build-info.txt` recording full source commit, `LogIdentity`, DLL SHA-256, byte count and source link. Verify its fields against the built artifact. These are integrity/provenance data, NOT code signing. |
| First-run preferences | `src/config.rs::DEFAULT_INI`, `Config::default`, and the shipped example enable the native adapter/widescreen, GOS Video lock, RGB565 first-image fix, `FocusRepaint`, `WindowEraseGuard`, and Wine-only verified ROFFILE patch. | Preserve first-run defaults and CI assertions. Never overwrite an existing active INI; package only the example. |
| Persistent game-asset modification | `GosVideoLock=on` modifies only the verified small `crimptch.rof` overlay with a separate original backup; `off` does not auto-restore. | Keep explicit restore and opt-out guidance prominent in the README. Do NOT package game files or perform automatic uninstall edits. |
| Root repository description | Historical README still said modern widescreen was largely unvalidated/opt-in, despite repeated owner-successful Wine logs. | Align with the detailed README and distinguish validated Wine runtime from pending Windows 11 gameplay. |
| Windows compatibility | Repeated user tests under Manjaro/Wine/GE-Proton confirm native widescreen, menus, GOS preferences and surface transitions. The runner tests only Windows DLL load/exports. | **Blocking criterion for a claimed stable Windows release:** real game session on a native Windows computer, ideally Windows 11, with startup, GOS settings, in-game rendering, transitions, video playback, Alt+Tab and clean exit. |

## Updated user artifact contract

```text
Crimson Skies/
├── CRIMSON.EXE                               # user's accepted original binary
├── ddraw.dll                                 # sole file copied to game root
└── CrimsonSkiesFix/
    ├── CrimsonSkiesFix.ini.example           # template; not loaded automatically
    ├── README.md
    ├── LICENSE
    ├── build-info.txt                        # records full source commit and DLL hash
    └── CrimsonSkiesFix.ini                   # generated locally; NEVER distributed
```

The active INI and the installed game's own `GOSDATA/` directory are not part of the downloadable artifact. The CI checks that no log, game asset or active `CrimsonSkiesFix.ini` is bundled.

`build-info.txt` fields: `Project`, `Version`, `Commit`, `LogIdentity`, `Architecture`, `DllSha256`, `DllSizeBytes`, `Source`. The short SHA printed during initialization corresponds to the first eight hexadecimal digits of the full source commit. The DLL digest distinguishes two binaries built from different commits with the same version.


## Independent candidate verification (after PR #19)

This repository also provides `tools/verify_release_candidate.py`, a
read-only verifier for the *unpacked, downloaded* `CrimsonSkiesFix-ddraw`
artifact. The Windows x86 CI executes the synthetic positive/negative
self-tests and checks the staging directory. A second **Ubuntu**
`verify-download` job retrieves the archive from the very same workflow
and checks the final downloaded payload independently of the PowerShell
packaging script, using the exact PR-head (or main) source SHA.

Checks include: exactly five files, no shipped user INI/game assets,
a valid x86 PE32 DLL, complete `build-info.txt`, matching DLL SHA-256 and
byte size, matching full Git commit and logged build label, and a byte-for-byte
GPL license match. The verifier does not claim digital signature/authenticity
or native Windows 11 gameplay compatibility.

Run the same check locally after unpacking an artifact, from the repository
root (replace the commit with the SHA from that GitHub Actions run):

```sh
python3 CrimsonSkiesFix/tools/verify_release_candidate.py \
  --directory /path/to/unpacked-artifact \
  --expected-commit THE_FULL_40_DIGIT_SOURCE_SHA \
  --cargo-toml CrimsonSkiesFix/Cargo.toml \
  --license LICENSE
```

The `Cargo.toml` and `LICENSE` must come from the **same commit** as the
artifact; do not verify an earlier downloaded archive against today's
unrelated `main`. A successful offline check is a package-integrity
statement only.

**Prepared, but not executed:** a single-session Windows 11 acceptance
protocol in `docs/windows11-acceptance-2026-11-04.md`, and non-published
draft notes in `docs/release-notes-draft.md`. These documents do not
authorize a GitHub tag, release, or merge.

## Release acceptance checklist

- [ ] Windows x86 build and independent Ubuntu downloaded-artifact verification both pass on the exact **final accepted source SHA**, with Rust tests, native DLL loader smoke, PE32 checks, source manifest and complete archive layout.
- [ ] Independently verify the exact final downloaded `ddraw.dll` archive's five files, license, binary hash, source SHA and logged identity (using the `verify-download` job and optionally the offline verifier). The standalone `windowed_ddraw_probe.exe` remains compiled, checked as PE32 and covered by its automated tests, but is no longer uploaded as a separate artifact or included in the game package.
- [x] Prior Wine/GE-Proton gameplay validation exists for PR #18 build `7ee1db87` (9 graphics generations, 8 retired chains, ROFFILE 4/4, signature preserved), but **this is not a test of a future DLL after a code change**.
- [ ] **Voluntary game exit under Wine/GE-Proton**: separately verify two normal quit actions from the main menu and after gameplay with the process exit observer (`tools/verify_game_exit.py`, `docs/exit-validation-2026-11-04.md`). The latest in-game log does not record a process exit code; do not infer success or failure from the last DirectDraw line.
- [ ] Native Windows 11 gameplay session with the exact new release candidate: startup chooser bypass, first image, movies in 4:3, MainMenu, Video preferences/lock/actual resolution, gameplay and FOV/HUD at native monitor size, several 1920×1080↔800×600 transitions, Alt+Tab focus recovery, and clean exit.
- [ ] Confirm the owner's explicit decision to designate a release stable; publish an appropriately versioned tag and release notes only after approval.

## Game shutdown diagnostic policy

The proxy intentionally does **not** log from `DllMain(DLL_PROCESS_DETACH)` because this code runs under the Windows loader lock. We can only verify process termination externally. A separate historical Wine crash log (ROFFILE +0x22E0 / `c0000005`, umu exit 5) predates the current ROFFILE-protected test and **is not proof of a current shutdown crash**. The external observer captures a process status and unhandled-exception markers without injecting any hooks. Simulated process self-tests are automated on Linux and Windows; the real game exit still needs user confirmation.

## Release communication

- Prefer the wording **experimental, Wine/GE-Proton tested; native Windows gameplay unverified** until a real Windows test has been completed.
- Do not describe counts of DirectDraw/Direct3D API calls per second as measured display FPS.
- Do not claim that a Windows runner loading `ddraw.dll` proves that the whole game, its audio/video codecs or graphics HAL work on Windows 11.
- The proxy is GPL-3.0-or-later, the original game remains separately licensed; packaging the license/source link improves provenance but does not waive any GPL distribution duties.
- Do not merge this PR, create tags or publish a stable release without the owner's explicit authorization.

legluondunet assisted by ChatGPT (OpenAI)
