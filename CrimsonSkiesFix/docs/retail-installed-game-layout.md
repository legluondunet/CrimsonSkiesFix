# Original Crimson Skies install layout: reference, not yet implemented

The owner supplied the complete `tree "Crimson Skies/"` from a classic
Microsoft installation on 2026-11-10. It lists **68 directories and
225 files**. This is useful as a reference but does not prove content
equality without comparing SHA-256 of the files.

## Current narrowly scoped installation work

This milestone deliberately covers **only**:

1. Filenames inside `ZBD/C1/`, `C1B/`, `C1C/`, `C2/`, `C2B/`,
   `C3/`, `C4/`, `C5/` and their `Mxx`/`MPx`/`IA1` subdirectories.
   The original CAB file entry `name()` is not necessarily the
   eventual retail-installed long filename; do not fabricate an expansion
   of an 8.3 alias. The name audit compares extracted files with a
   genuine installed `ZBD/` tree recursively and by per-file SHA-256.
2. Removing the eight temporary `C*.CAB` files *after* expanding
   and independently verifying the ZBD resources. Per-file hashes are
   sealed into `CrimsonSkiesFix.chapters.tsv` before publishing the
   transaction so `--verify-install` can work without retained CABs.

The owner's reference tree contains **177** files within eight
chapter directories, plus six direct `ZBD/` files, for **183** ZBDs.
Earlier CAB-based installation logs reported **178 chapter members**,
one more than the reference count. The audit reports counts per chapter;
the extra entry must be identified before deleting or renaming anything.

To examine an already installed USA game from an earlier experimental
build, while providing the classic installed game as the reference:

```bash
REF_ZBD="$(winepath -w "$PWD/Crimson Skies/ZBD")"
WINEDEBUG=-all wine CrimsonSkiesFix.exe --audit-shortnames USA "$REF_ZBD"
cp CrimsonSkiesFix.log shortnames-reference-USA.log
```

The original classic-installed `Crimson Skies/ZBD` directory is
read-only to this command; it requires access to the existing original
installation. Hash matching is only meaningful for the **same game
edition and asset version**.

## Outside this milestone — documented only, not being changed now

The classic root contains `clcd16.dll`, `clcd32.dll`, `clokspl.exe`,
`dplayerx.dll`, `drvmgt.dll`, `mcp.dll`, `SETUPENU.DLL`,
`Readme.rtf`, `EBUSetup.sem`, and `UNINSTAL.EXE`.
Its `crimsonff.ifr`, `cszoneregister.exe`, and French `EULA.RTF`
names differ from some original CD filenames. These are **observed
differences**, not part of the current approved corrections.
Other installer-generated files, registry state, and SafeDisc handling
are deferred.

No original game files or protected executables are bundled with the
project. The PR remains draft pending real-media validation.
