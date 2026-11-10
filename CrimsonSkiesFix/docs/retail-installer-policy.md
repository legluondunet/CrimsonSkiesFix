# Original Microsoft installation rules: authoritative file-copy policy

The project recognizes three original disc editions by complete file path,
byte length and SHA-256, and takes the on-disc **FR INSTALL.EXE** and
**DE/USA SETUP.EXE** source-to-destination file-copy records as the
canonical instructions. These original binaries are **not** committed to
the project or executed by CrimsonSkiesFix.

## Source authenticity

- FR `INSTALL.EXE`, SHA-256 `30895f3dc36bce45632beb6cbc42a350f06c5c4b2fabb7330d9fc28b1906bdc9`
- DE `SETUP.EXE`, SHA-256 `cf9ce3f28feb5a4e82909a00d1b351dec62343e2897997a450c83f0f61d62839`
- USA/Europe `SETUP.EXE`, SHA-256 `04e26ce391bb9f2a5d81518588aa0f7d553a95c5202a742d58af54b4070a89df`

These match the original-CD manifest profiles in `installer/profiles/`.
The owner also supplied an existing classic Microsoft installation tree,
which provides an independent path-layout reference (68 directories,
225 files, but without per-file hashes).

## Original file installation rules implemented

1. Extract and SHA-256-check **all original `GAME/` files** from the
   edition-authenticated disc before modifying any names. The CD
   `GAME/ZBD/C*.CAB` archives are temporary source data, not installed
   game artifacts.
2. Apply original setup source→target names for `GAME/` root:
   `CRIMSO_1.IFR` → `crimsonff.ifr`,
   `CSZONE_1.EXE` → `cszoneregister.exe`, and on FR
   `CLUF.RTF` → `EULA.RTF`. USA/DE `GAME/EULA.RTF` is
   already correctly named. Other GAME assets retain original content
   and folder structure.
3. Apply the original root-CD copy rules: place `CLCD16.DLL`,
   `CLCD32.DLL`, `CLOKSPL.EXE`, `DPLAYERX.DLL`,
   `DRVMGT.DLL`, `MCP.DLL`, and `SETUPENU.DLL` in the game
   root, with destination names `clcd16.dll`, `clcd32.dll`,
   `clokspl.exe`, `dplayerx.dll`, `drvmgt.dll`, `mcp.dll`,
   `SETUPENU.DLL`. Copy the edition's CD-root
   `LISEZMOI.RTF` (FR) / `README.RTF` (USA/DE)
   as installed `Readme.rtf`. Each file is SHA-256 verified
   against its original CD profile, including after installation.
4. Expand the **eight authenticated chapter CAB archives**. Use the
   edition-specific source→destination table recovered from the three
   original setup executables for the 40 shortened ZBD filenames.
   For example, USA `C2B/RTEXTU~1.ZBD` → `C2B/rtexture9.zbd`,
   but FR/DE `C2B/RT60AC~1.ZBD` → `C2B/rtexture9.zbd`.
   Verify decompressed member content, write the final filename/size/
   SHA-256 manifest, then remove exactly the eight source CABs.
5. Keep writable `SavedGames/` and `Planes/` in the game directory.
   Do not overwrite any existing installation, even one belonging to
   another edition. No installed original game content is bundled.
6. Apply the official 1.02 asset `CRIMPTCH.ROF` using its separately
   authenticated Microsoft updater, without changing the CD-protected
   `CRIMSON.EXE` (the separate last-stage executable policy).
7. Publish the new game directory **only after** all extracted files,
   expected original→installed paths, the eight CD-root companion
   files, all chapter SHA-256 hashes, proxy DLL and official 1.02
   asset have been verified. `--verify-install` uses the same
   original-CD checks and sealed CAB-free chapter manifest.

## Additional actions present in the Microsoft setup — pending policy

The setup also describes or invokes actions beyond the file-copy
table. These must **not** be approximated from string matches:

- Registry configuration: `%REGROOT\\Zone`,
  `%REGROOT\\Version`, `%REGROOT\\UIAssetPath`, legacy
  DirectPlay application registration, an uninstall entry, and
  `RunOnce`. Different entries use HKLM versus HKCU.
- Legacy system actions: DirectX/DirectPlay prerequisites, font
  installation in Windows system folders, shortcuts and setup
  uninstall bookkeeping.
- Installer-generated files `EBUSetup.sem` and `UNINSTAL.EXE`
  appear in the classic installation but have no byte-identical
  source record in the disc's `GAME/` manifest. Do not invent
  contents or spoof the original uninstaller.

These are **identified but not implemented** until their parameters,
execution conditions and safe Windows/Wine equivalents are verified.
The intent is functional fidelity on modern systems, not blindly
replacing modern system DLLs or writing privileged registry entries.

The file installer therefore reproduces the authenticated **game
directory** rules while deferring global system and registry side
effects to separately reviewed compatibility work. It never runs the
original setup in the user's environment.

## Known media discrepancy

The original three setup tables each describe 178 CAB chapter members
plus six standalone ZBD resources. One historic Microsoft-installed
reference tree contains only 177 chapter files, missing
`C2B/rtexture9.zbd`. All three original setup copy tables explicitly
list that file. Do not silently drop a verified source member to force
counts to match; capture more evidence of the historical edition/setup
options if needed.
