# Official Crimson Skies 1.02 update: static installer analysis

This document records a static analysis of Microsoft's original Windows
**Crimson Skies 1.02 updater**, `crmsk102.exe`, carried out on
**1 November 2026** from an owner-supplied copy. It explains which
game files the patch provides, the registry keys its installation
configuration mentions, and whether the patch is still useful when
running a separately unwrapped 1.02 game executable.

**Scope:** The updater was **not executed**. Its executable resources,
embedded CAB archive, and compiled installer configuration were
inspected as data. File contents and references in the installer are
verified observations; the complete runtime behavior of the historical
setup engine and compatibility with every localized installation remain
unverified.

## 1. Update package identity

| Property | Value |
|---|---|
| Filename | `crmsk102.exe` |
| Size | 2,315,728 bytes |
| SHA-256 | `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af` |
| MD5 | `37af1ee0ce98a17dbeab68e4cd217f9f` |
| Embedded archive | Win32 resource `CABFILE / FILESUSA.CAB` (language `0x0409`) |
| CAB offset / size | `0x346F0` / 2,091,693 bytes |
| Compiled installer configuration | `SETUPBINARY / SETUPDATA / 1033`, offset `0x33DC8`, size 2,337 bytes |

The matching update is historically archived as
[Crimson Skies Update v1.02 on ModDB](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102).
The `SETUPDATA` resource is a **compiled binary configuration**, not a
plaintext batch file or easily editable `.iss` installation script.

## 2. CAB payload: exact files

The embedded CAB contains the following **complete files**, not only
binary differences to apply to a previous installation:

| CAB file | Size (bytes) | SHA-256 | Purpose |
|---|---:|---|---|
| `CRIMSON.EXE` | 344,851 | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` | SafeDisc-protected executable launcher |
| `CRIMSON.ICD` | 2,580,578 | `0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b` | SafeDisc-protected game payload |
| `gosdata/assets/CRIMPTCH.ROF` | 797 | `7ae28cfb65645a3e22aadc64a68badc23e2543bc56356d599fb59375d9ebbefe` | Additional game asset/script overlay |
| `readmeUSA.rtf` | 57,539 | `dd8f7bc14e96b68f974f95d252663a4a17aac8a5a318779616fdfa908409be75` | Update notes |
| `setupenu.dll` | 32,768 | `35baf7d1a968d0ce4c4f535693b881199b479cfd94e3f4396e2c14b39ba82547` | Installer support DLL, not a game asset |

The compiled setup file records explicitly identify the two protected
executables and `gosdata/assets/CRIMPTCH.ROF` as game payloads. No
other game file replacements were found in the supplied CAB or those
records; this does **not** prove that the generic installation engine
has no additional side effects.

### The additional `CRIMPTCH.ROF` file matters

The 797-byte ROF is **not empty padding**. It contains a compressed
`ASSETS/SCRIPTS/AIRFRAME.SCRIPT` entry which expands to **1,641 bytes**.
The readable script includes `gui_create`, `gui_init` and
`gui_mailbox`, referencing aircraft selection/descriptions,
`@planeconstruction@`, and related callbacks.

Its precise gameplay fix has not been identified because the
pre-update asset/script has not been independently compared. The
official updater's protected EXE pair cannot substitute for this
separate asset.

## 3. Registry instructions in the installer

The compiled `SETUPDATA` resource explicitly references these two
registry destinations:

| Registry path | Value / parameter | Interpretation |
|---|---|---|
| `HKEY_LOCAL_MACHINE\Software\Microsoft\Microsoft Games\Crimson Skies\1.0` | `Version` associated with `1.02` | Installed-game version registration |
| `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\DirectPlay\Applications\Crimson Skies` | `Guid` associated with `%DPLAYGUID` | DirectPlay registration/handling |

**Caution:** The static configuration shows the paths and associated
values, but does **not** establish whether the DirectPlay `Guid` is
written, read, repaired, or conditionally created at runtime. Do not
invent or manually set a new GUID based on the placeholder
`%DPLAYGUID`.

The installer also contains generic setup-engine references to
`RunOnce` and DirectX-related registry paths. These are **not
confirmation** that this specific update always writes those keys.
Only execution tracing in an isolated historical installation could
provide an exhaustive list of runtime registry and filesystem
changes.

For inspection on 64-bit Windows, the 32-bit registry view may be
queried with:

```powershell
reg query "HKLM\SOFTWARE\Microsoft\Microsoft Games\Crimson Skies\1.0" /v Version /reg:32
reg query "HKLM\SOFTWARE\Microsoft\DirectPlay\Applications\Crimson Skies" /v Guid /reg:32
```

The queries are read-only. Not finding these keys does not by itself
prove that single-player gameplay will fail.

## 4. Localized editions and SafeDisc: what is known

The complete protected `CRIMSON.EXE` and `CRIMSON.ICD` inside the
official updater are **byte-for-byte identical** to the files provided
from a **German-labelled Crimson Skies 1.02** archive and to the
previously investigated original-version protected pair.

An owner-supplied **French 1.02 CD** instead has a different SafeDisc
wrapper:

| Edition / source | Launcher SHA-256 | ICD SHA-256 |
|---|---|---|
| Official 1.02 updater and German-labelled 1.02 set | `675a3c76…a8afa736` | `0e3b4724…146e9833b` |
| French already-1.02 CD | `cf9ec313…4712ae` | `f3fe7c1e…e7262eb` |

Despite those differences, both protected pairs were reconstructed
using the native Rust research tool in
[unsafedisk](https://github.com/legluondunet/unsafedisk) into the
**same 2,580,480-byte unwrapped PE executable**, SHA-256:

```text
43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75
```

This establishes **two observed SafeDisc packaging variants of the
same game executable**, not different underlying unwrapped game code.

The updater contains an English/USA CAB (`FILESUSA.CAB`), a
`setupenu.dll` helper and `readmeUSA.rtf`. Although the binary pair
is shared with the German-labelled files, **it has not been proven
that this particular setup program successfully installs over all
French/German/US original-language installations**. The distinct
French CD encryption may reflect a different production/protection
pass, but its historical cause is unknown. Language-specific voices,
text, and resources can still differ.

## 5. Is the official updater still necessary?

**If the player already has the exact 1.02 unwrapped game EXE**, then
replacing the game with the updater's SafeDisc-protected
`CRIMSON.EXE`/`CRIMSON.ICD` supplies **no additional executable
game code** relative to that verified output.

However, the updater is **not necessarily redundant**:

1. Confirm that `gosdata/assets/CRIMPTCH.ROF` exists with the
   matching official SHA-256 above. If absent, the executable alone
   does not supply this separate script/asset correction.
2. If relying on legacy version detection, check that the
   `Version` registry value is recorded as `1.02`.
3. For legacy multiplayer/DirectPlay functionality, inspect the
   `Guid` registration before changing anything.
4. Retain game localization files independently; do not overwrite
   localized assets merely because the protected EXE pair is shared.

```sh
sha256sum gosdata/assets/CRIMPTCH.ROF
```

The expected hash is
`7ae28cfb65645a3e22aadc64a68badc23e2543bc56356d599fb59375d9ebbefe`.
This command is read-only and works on Linux; on Windows use
`Get-FileHash .\gosdata\assets\CRIMPTCH.ROF -Algorithm SHA256`
in PowerShell.

**Recommendation:** Keep the updater as a documented **source of the
asset patch and legacy installation metadata**, even when
using an already-unwrapped 1.02 executable. Do not automatically
run this old setup program or overwrite working game files solely
to obtain the same underlying executable.

## 6. Provenance and limitations

This is a preservation note, not an installer reimplementation.
Static analysis establishes bundled payloads and configuration
references, **not** a complete trace of installation side effects.
No commercial EXE/ICD, DLL, CAB, ROF, or proprietary game script data
is redistributed here.

Primary technical research:
[unsafedisk — official 1.02 updater audit](https://github.com/legluondunet/unsafedisk/blob/feature/generic-pe-readonly-inspection/docs/crimson-102-official-updater-audit.md).
The findings are also discussed in that project's
[draft PR #2](https://github.com/legluondunet/unsafedisk/pull/2).

---
Research and documentation: **legluondunet assisted by ChatGPT (OpenAI)**.
