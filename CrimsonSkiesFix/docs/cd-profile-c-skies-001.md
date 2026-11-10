# CD profile C_SKIES_DISC_001 — first owner-supplied disc scan (2026-11-10)

This is the first **content-based** reference for CrimsonSkiesFix's ISO/CUE/BIN installer.
The CUE filename was `Crimson Skies (USA, Europe).cue`; that label is a source
hint, **not** an independently established language or regional identity.
The CD contains `00000409.016` and `00000409.256`, suggesting US English
installer resources. The label `C_SKIES` is diagnostic, not part of edition
acceptance.

## Read-only inspector results

- Input: CUE, data track 1, MODE1/2352, ISO9660 filesystem.
- Extracted internal-file manifest: **112 files**, **112 valid SHA-256**, **0 suspects**.
- Sum of file payload byte sizes: **582,406,464 bytes**.
- The authoritative per-file baseline is
  [c_skies_disc_001.tsv](../installer/profiles/c_skies_disc_001.tsv).
  It contains path, size and SHA-256 of every file *inside* the CD.
- Neither the overall ISO/BIN size nor the whole-image hash is recorded as
  an edition signature. A differently mastered image with the same complete
  internal files matches this profile, irrespective of source filename,
  ISO descriptor label, CUE index or BIN padding.

## Relevant original CD files

| Internal path | Size in bytes | SHA-256 |
|---|---:|---|
| `GAME/CRIMSON.EXE` | 344,851 | `7d6029471d4e5f2c7f4b08ce86c0ffd1f9bc8c697cf3952695fd68ff011f25a1` |
| `GAME/CRIMSON.ICD` | 2,560,095 | `c586773b0475aa6560b4464e5aa0d1da4cc249099b8db16be7240f4221d7e59d` |
| `GAME/GOSDATA/ASSETS/CRIMSON.ROF` | 60,236,221 | `acc9946874110e9741183384010bc02fd48923ae3d608fcf02a96498c3731174` |
| `GAME/GOSDATA/ASSETS/BINARIES/ROFFILE.DLL` | 135,168 | `1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a` |

The CD also holds ten `GAME/GOSDATA/ASSETS/GRAPHICS/MPG/` videos,
eight `GAME/ZBD/*.CAB` files, and several substantial `*.ZBD`
resources. Other top-level material consists of legacy installers,
DirectX components, fonts, documentation, SafeDisc-related files, and
online-service tools; do not automatically install unrelated system
dependencies as part of the game patch.

## Installation implications / gaps

1. The two CD executables are **not** the exact accepted unwrapped
   1.02 PE executable (2,580,480 bytes; SHA-256
   `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`).
   The game launcher must continue to refuse unknown/protected builds.
2. The original CD **does not include**
   `GAME/GOSDATA/ASSETS/CRIMPTCH.ROF`. The already-investigated official
   `crmsk102.exe` updater contains the 797-byte original asset, and
   protected 1.02 executables. Its compatibility and integration with this
   scanned CD must still be tested; extraction alone is not a complete
   playable 1.02 installation.
3. Required file layout and installation semantics of `GAME/ZBD/` and
   any game-setup side effects are **not established by a file manifest**.
   A scan is not equivalent to an installed-game directory comparison.
4. Do not merge the public installer PR until the image's install behavior
   and updater interaction are understood and tested on Windows 11 and
   Wine/Proton; do not assume a root-level direct copy is sufficient.
5. A second disc image may still have the same *edition* with different
   mastering bytes. The profile matcher operates only on the 112
   contained file paths, sizes and SHA-256 values.

## Next inputs

A second scan from another country/language or retail pressing will allow
us to compare files and define variants without falsely rejecting identical
content. For the 1.02 update, confirm the exact updater file from the user
and compare the installed files before/after applying it where feasible.
