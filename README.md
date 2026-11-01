# Crimson Skies Patcher (Rust)

A command-line patcher for the **Windows PC release of Microsoft Crimson Skies (2000)**. It changes resolution and widescreen field of view (FOV), can apply a narrowly verified Wine/Proton crash workaround, and preserves the original game files before writing changes.

## Compatibility — exact unwrapped Crimson Skies v1.02 only

**CrimsonSkiesFix accepts only the clean, unmodified Crimson Skies
version 1.02 EXE produced by our `unsafedisk` tool.** SafeDisc-protected
launchers, older game versions, and third-party prepatched
resolution/FOV/widescreen executables are **not supported**.

| Verified EXE | Value |
|---|---|
| Filename | `CRIMSON.EXE` |
| Size | `2,580,480` bytes |
| SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

The DirectDraw DLL checks the executable's exact on-disk SHA-256
when DirectDraw initializes. **If it does not match, the DLL refuses to
initialize graphics**, shows the expected and detected hashes in a
Windows error dialog, and logs the rejected hash to
`CrimsonSkiesFix.log` next to `ddraw.dll`, including when
`[Debug] Log=0` (assuming the folder is writable).

The DLL is designed to adapt the verified EXE **in memory** to the
user's monitor resolution, 3D field of view and HUD without changing
the executable on disk. Full native-resolution 3D gameplay remains
experimental and requires live validation.

If your game is earlier than v1.02, install the
[official Crimson Skies 1.02 update (archived copy)](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102)
when applicable to your language/edition. **This official update
does not remove SafeDisc**; it does not by itself supply the
required unwrapped EXE.

Check your EXE with:

```sh
sha256sum CRIMSON.EXE
```

Or under Windows PowerShell:

```powershell
Get-FileHash .\CRIMSON.EXE -Algorithm SHA256
```
