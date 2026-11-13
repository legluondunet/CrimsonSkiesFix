# Original Crimson Skies CD file profiles: USA/Europe, FR, DE

Validated **from the complete file-inventory logs supplied by the owner on
2026-11-10**, produced with `CrimsonSkiesFix.exe --inspect-cd`. These scans
describe the **ISO9660 files within the images**, not whole-image fingerprints.

| Profile | User-supplied source | Internal files | Hashed payload bytes | Suspects |
|---|---|---:|---:|---:|
| `C_SKIES_DISC_001` | `Crimson Skies (USA, Europe).cue`, MODE1/2352 | 112 | 582,406,464 | 0 |
| `C_SKIES_FR_001` | `MS-CrimsonSky-FrenchVersion.iso`, ISO/2048 | 112 | 624,946,021 | 0 |
| `C_SKIES_DE_001` | `Crimson Skies (Germany).iso`, ISO/2048 | 107 | 590,082,610 | 0 |

The user-provided names and locale resources support the edition labels,
but do not by themselves establish the publisher's official SKU, release
region, or exact original version. All three report volume label `C_SKIES`,
so this label **cannot distinguish the editions**.

**Canonical manifests:** `installer/profiles/c_skies_disc_001.tsv`,
`c_skies_fr_001.tsv`, and `c_skies_de_001.tsv`. Each profile records *all*
the files visible in the ISO9660 filesystem using relative path, payload byte
size and payload SHA-256, without any whole-image size or checksum.

## File-level comparisons

| Edition pair | Shared paths | Identical shared files | Different shared files | First-only paths | Second-only paths |
|---|---:|---:|---:|---:|---:|
| USA/Europe ↔ FR | 97 | 64 | 33 | 15 | 15 |
| USA/Europe ↔ DE | 103 | 81 | 22 | 9 | 4 |
| FR ↔ DE | 98 | 64 | 34 | 14 | 9 |

64 paths contain identical bytes across **all three** images. In particular,
`GAME/GOSDATA/ASSETS/BINARIES/ROFFILE.DLL` is identical (135,168 bytes,
SHA-256 `1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a`).
`IJL10.DLL`, core C runtime DLLs, certain `GRAPHICS` resources and the
`MSOPEN1.MPG`, `CRIMFLAG.MPG`, `ZIPPER.MPG` videos are also identical.

Both `LANGUAGE.DLL` and `LANGUI.DLL`, plus `STRINGS.DLL` and
`CRIMSON.ROF`, differ in payload between the three editions. Most chapter
videos (`CHAP0.MPG` through `CHAP5.MPG` and `FINAL.MPG`) differ too.
**Never substitute one edition's language files, game archive, videos, or
audio archives for another edition's files.**

The eight `GAME/ZBD/C*.CAB` resources match exactly between USA/Europe and
DE. The corresponding FR archives have identical **byte lengths** but
different SHA-256 hashes, proving why a size-only edition check would be
incorrect. The `SOUNDSH.ZBD` / `SOUNDSL.ZBD` files also have significantly
different payload sizes across editions.

## Important executable and update findings

| CD file | USA/Europe | FR | DE |
|---|---|---|---|
| `GAME/CRIMSON.EXE` size | 344,851 | 344,925 | 344,993 |
| `GAME/CRIMSON.EXE` SHA-256 | `7d602947…011f25a1` | `cf9ec313…4712ae` | `a4ba27ae…e0750af` |
| `GAME/CRIMSON.ICD` size | 2,560,095 | 2,580,578 | 2,560,095 |
| `GAME/CRIMSON.ICD` SHA-256 | `c586773b…59d` | `f3fe7c1e…262eb` | `5fb33f3b…b7172` |
| `GAME/GOSDATA/ASSETS/CRIMPTCH.ROF` | absent | **present, 797 bytes** | absent |

The FR protected `CRIMSON.EXE` and `CRIMSON.ICD` match the previously
examined **French 1.02 CD** pair documented in
`docs/crimson-skies-102-official-update.md`: `cf9ec313…4712ae` and
`f3fe7c1e…262eb`. FR's `CRIMPTCH.ROF` has exactly the official update
payload hash
`7ae28cfb65645a3e22aadc64a68badc23e2543bc56356d599fb59375d9ebbefe`.
Therefore it **must be retained**, not needlessly replaced with a copy from
the official 1.02 updater.

USA/Europe and DE lack this separate asset on disc; the official 1.02
updater includes it. This does **not** establish that all three editions
require exactly the same update actions or can execute the historical updater
unmodified. The DE on-disc SafeDisc pair is different from the protected
pair in the previously audited official 1.02 updater; no assertion is made
that the on-disc DE executable is already 1.02.

The runtime DLL still only allows the known exact **unwrapped 1.02 executable**
(2,580,480 bytes, SHA-256
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`).
A successful **CD scan** is not a complete game install; a protected on-disc
binary is not accepted by the runtime gate.

## Installer implementation policy

1. Detect exactly one supplied ISO or CUE/BIN, read its internal filesystem
   and identify the edition from the full exact file-content manifest.
2. Copy/install the appropriate internal `GAME/` files with per-file SHA-256
   verification, preserving edition-specific language, video, sound and ROF.
3. Do not install bundled legacy DirectX/system files gratuitously.
4. Handle an optional, verified official `update1.02` per validated edition;
   **never overwrite FR's already-identical CRIMPTCH.ROF**.
5. Never silently accept or distribute a protected or unknown EXE as though
   it passed the established strict unwrapped 1.02 executable gate.
6. Keep ISO9660 volume label, image file size, ISO/BIN checksum, CUE track
   layout and filenames out of edition fingerprints. A different dump with
   exactly the same internal files must match.
7. Do not merge the installer PR until these installs are exercised with
   genuine game images and tested on Windows and Wine/Proton.

## Remaining validation

- Confirm actual copy versus installer/CAB extraction semantics and any
  necessary registry/launcher prerequisites from installed-game comparisons.
- Determine whether/how the official 1.02 updater must be handled for both
  USA/Europe and DE; verify final protected/unwrapped EXE handling.
- Exercise reruns, incomplete images, malformed CUE references, safe rollback
  and ISO9660/Joliet path variants.
