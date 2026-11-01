# Crimson Skies Patcher (Rust)

A command-line patcher for the **Windows PC release of Microsoft Crimson Skies (2000)**. It changes resolution and widescreen field of view (FOV), can apply a narrowly verified Wine/Proton crash workaround, and preserves the original game files before writing changes.

## Compatibility — SafeDisc-free Crimson Skies v1.02

**CrimsonSkiesFix requires Crimson Skies version 1.02 with SafeDisc
removed.** This includes both the clean, unwrapped v1.02 executable and
supported v1.02 executables **already modified for widescreen resolution
and/or field of view (FOV)**. Earlier game versions and the original
SafeDisc-protected launcher are not supported.

If necessary, first update your game using the
[official Crimson Skies v1.02 patch (archived copy)](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102),
checking that the updater applies to your language/edition. **The
official update still installs SafeDisc-protected executables; updating
alone does not produce an unwrapped EXE.** Use files from a game you
are entitled to use, in accordance with applicable law.

### Verified v1.02 executable references

All three files below are **2,580,480 bytes**. The SHA-256 identifies
the *exact file*, not its game version alone:

| SafeDisc-free EXE variant | SHA-256 |
|---|---|
| Clean, unmodified v1.02 executable | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |
| Verified widescreen EXE, 1920×1080 | `5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67` |
| Verified widescreen EXE, 1920×1200 | `612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d` |

**Other supported resolution/FOV combinations will have different raw
SHA-256 values.** The legacy Rust resolution/FOV patcher checks the
known widescreen EXE *family* by normalizing its resolution to
1920×1080 and restoring the expected FOV fields before calculating
SHA-256. Its **normalized widescreen signature** is:

```text
5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67
```

This normalized signature is **not** the hash of every patched file:
the 1920×1200 variant above, for example, matches **after**
normalization. Arbitrary executable modifications are **not** thereby
verified or supported. The separate legacy Rust CLI accepts the
recognized *widescreen-patched* family for its resolution/FOV editing
path; it does **not** classify the clean unmodified EXE above as
widescreen-patched input. The DirectDraw DLL project and legacy Rust
EXE patcher have different responsibilities.

To inspect your executable's **raw** SHA-256:

```sh
sha256sum CRIMSON.EXE
```

Windows PowerShell:

```powershell
Get-FileHash .\CRIMSON.EXE -Algorithm SHA256
```

The checksum is for the game EXE, **not** for `ddraw.dll` or the
original SafeDisc launcher. A changed hash on a pre-patched EXE is
normal; compare its known patch lineage or normalized signature
rather than expecting it to match the clean v1.02 reference.
