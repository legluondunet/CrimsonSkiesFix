# CrimsonSkiesFix — release readiness audit (2026-11-04)

**Target:** current experimental `0.1.90` Windows x86 proxy, after the owner-validated PR #18. This is a packaging and release-readiness review, not a declaration of a stable 1.0 release.

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

## Release acceptance checklist

- [ ] Two successful Windows x86 CI checks at the exact PR head, with Rust DLL and diagnostic tests, PE x86 check, native DLL loader smoke and complete artifact-layout check.
- [ ] Inspect both downloadable archives; verify `LICENSE` is byte-for-byte identical to the repository, and `build-info.txt` matches the packaged `ddraw.dll` SHA-256 and the exact source commit.
- [x] Prior Wine/GE-Proton gameplay validation exists for PR #18 build `7ee1db87` (9 graphics generations, 8 retired chains, ROFFILE 4/4, signature preserved), but **this is not a test of a future DLL after a code change**.
- [ ] Native Windows 11 gameplay session with the exact new release candidate: startup chooser bypass, first image, movies in 4:3, MainMenu, Video preferences/lock/actual resolution, gameplay and FOV/HUD at native monitor size, several 1920×1080↔800×600 transitions, Alt+Tab focus recovery, and clean exit.
- [ ] Confirm the owner's explicit decision to designate a release stable; publish an appropriately versioned tag and release notes only after approval.

## Release communication

- Prefer the wording **experimental, Wine/GE-Proton tested; native Windows gameplay unverified** until a real Windows test has been completed.
- Do not describe counts of DirectDraw/Direct3D API calls per second as measured display FPS.
- Do not claim that a Windows runner loading `ddraw.dll` proves that the whole game, its audio/video codecs or graphics HAL work on Windows 11.
- The proxy is GPL-3.0-or-later, the original game remains separately licensed; packaging the license/source link improves provenance but does not waive any GPL distribution duties.
- Do not merge this PR, create tags or publish a stable release without the owner's explicit authorization.

legluondunet assisted by ChatGPT (OpenAI)
