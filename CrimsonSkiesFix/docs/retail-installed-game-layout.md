# Classic Microsoft-installed Crimson Skies file tree (owner reference)

The owner supplied a complete `tree "Crimson Skies/"` from an
original installation on 2026-11-10: **68 directories and 225 files**.
Use it to compare the installer's resulting *paths*. This list does
not contain file hashes and does not establish identical bytes.

The original FR `INSTALL.EXE`, DE `SETUP.EXE`, and USA/Europe
`SETUP.EXE` are now the authoritative sources for the installation
source→destination file-copy records. See
`docs/retail-installer-policy.md` for **implemented filesystem
rules** and the deliberately deferred operating-system side effects.
See `docs/cab-original-setup-long-names.md` for the exact three-way
CAB long-name comparison.

## Chapter layout

The user's traditional tree contains 177 files in eight chapter
directories: `C1=23`, `C1B=17`, `C1C=17`, `C2=25`,
`C2B=16`, `C3=27`, `C4=27`, `C5=25`. Six additional
`ZBD/*.zbd` are present directly at root, totaling 183.
The authentic setups each describe 178 chapter members, including
`C2B/rtexture9.zbd`, not visible in this tree; preserve the source
file pending a reasoned explanation of the difference.

The canonical layout contains `GOSDATA/ASSETS`, `ZBD/`
chapters/missions, `Planes/` and `SavedGames/`. The eight
`C*.CAB` source archives are **not** present in the final game
directory; the installer removes them only after verified
decompression and manifest creation.

## Important original-game root names

The original setup maps `GAME/CRIMSO_1.IFR` to
`crimsonff.ifr` and `GAME/CSZONE_1.EXE` to
`cszoneregister.exe` for all three editions; the FR edition
renames `GAME/CLUF.RTF` to `EULA.RTF`. All three
also install seven authenticated CD-root companion binaries
(`CLCD16.DLL`, `CLCD32.DLL`, `CLOKSPL.EXE`,
`DPLAYERX.DLL`, `DRVMGT.DLL`, `MCP.DLL`,
`SETUPENU.DLL`) plus the localized CD-root Readme.

The classic tree additionally contains `EBUSetup.sem` and
`UNINSTAL.EXE`, generated/managed by the historical installer.
They are not yet replicated: their function and creation logic are
not established by copying source CD files. SafeDisc and registry
system-wide changes also remain separately scoped.

## Reference name diagnostic

The read-only `--audit-shortnames` command supports a
previous experimental game installation and a genuine Microsoft
installed ZBD directory. It walks all nested chapter directories,
SHA-256 hashes the files and reports ambiguous matches rather than
guessing aliases. Ensure both installations use corresponding
game editions and versions.

```bash
REF_ZBD="$(winepath -w "$PWD/Crimson Skies/ZBD")"
WINEDEBUG=-all wine CrimsonSkiesFix.exe --audit-shortnames USA "$REF_ZBD"
cp CrimsonSkiesFix.log shortnames-reference-USA.log
```

The PR remains draft pending genuine CD/Windows/Wine verification.
