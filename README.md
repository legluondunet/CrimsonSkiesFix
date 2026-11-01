# Crimson Skies Patcher (Rust)

A command-line patcher for the **Windows PC release of Microsoft Crimson Skies (2000)**. It changes resolution and widescreen field of view (FOV), can apply a narrowly verified Wine/Proton crash workaround, and preserves the original game files before writing changes.

## Compatibility — unwrapped Crimson Skies v1.02 only

**CrimsonSkiesFix is intended for the unwrapped (SafeDisc-free) Crimson Skies
version 1.02 executable only.** Older game versions and the original
SafeDisc-protected executable are not supported.

If necessary, first update your game using the
[official Crimson Skies v1.02 patch (archived copy)](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102),
checking that the updater applies to your language/edition.
**The official update installs SafeDisc-protected files; applying the
update alone does not produce an unwrapped executable.**
Use an unwrapped copy from a game you are entitled to use, in accordance
with applicable law.

**Verified clean, unmodified v1.02 unwrapped EXE reference:**

- **File:** `CRIMSON.EXE` (or the unwrapped output before renaming)
- **Size:** `2,580,480` bytes
- **SHA-256:** `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`

To check the file **before applying any other fixes**:

```sh
sha256sum CRIMSON.EXE
```

Or on Windows (PowerShell):

```powershell
Get-FileHash .\CRIMSON.EXE -Algorithm SHA256
```

The hash identifies the exact unwrapped reference executable validated
during our SafeDisc research. A previously patched or otherwise modified
EXE can have a different hash; the hash is **not** a checksum for
`ddraw.dll` or for the retail SafeDisc launcher.
