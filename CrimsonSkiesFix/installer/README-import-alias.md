# Experimental CrimsonSkiesFix 1.1 — DirectDraw import alias, native WinMM

**TEST ONLY — do not merge or publish as a release.**

This candidate removes the app-local WinMM proxy entirely. The verified
approved decrypted 1.02 `CRIMSON.EXE` remains completely unchanged on disk.
The single-file installer deploys:

```text
CrimsonSkiesFix-IMPORT-ALIAS-TEST.exe
CrimsonSkiesFix_installer.log
CrimsonSkiesFix.log
crimsonskies/
  CRIMSON.EXE
  CSFIX.dll                         <- only the two DirectDraw import exports
  GOSDATA/...
  crimsonskiesfix/
    CrimsonSkiesFix.dll             <- complete game-fix runtime
    CrimsonSkiesFix.ini
    build-info.txt
```

The installer verifies the exact approved game binary SHA-256 before running.
It creates the **child process suspended**, parses and verifies the
original PE32 `DDRAW.dll` import descriptor, locates the real child image
base through the PEB, validates the same 10 bytes in child memory, changes
**only that child-memory name** to `CSFIX.dll`, restores page protection,
and resumes the OS import loader. It does **not** change the game executable
on disk and cannot be invoked on arbitrary Windows x64 PE images.

`CSFIX.dll` has NO WinMM imports or exports, including ordinals.
Its `DllMain` is empty. The two DirectDraw calls reach the established
`CrimsonSkiesFix.dll` runtime only after the game first invokes them.
The Windows/Wine **original system WinMM** supplies all multimedia APIs,
by both names and native ordinals; the previous illegal lazy
`LoadLibraryW(system winmm.dll)` from an unrelated DLL's `DllMain`
is no longer on any game path. The `ddraw.dll` filename is free for
the system backend or a separately installed compatible rendering wrapper.

**Migration:** an old app-local `winmm.dll` is renamed to
`winmm.dll.before-alias.bak` ONLY if its SHA-256 matches one of three
known published experimental split builds. An unknown user DLL or an
occupied migration backup is never overwritten. Existing game content,
player data, and original `ddraw.dll` remain untouched.

**Current limits:**
- This is an experimental launch method, not a release. The launcher must
  start the game; starting `CRIMSON.EXE` directly will use its on-disk
  original imports and will not load `CSFIX.dll`.
- Tests of the synthetic PE32 suspended process succeeded on **native
  Windows x86 and Wine32**, showing real OS WinMM unchanged and both
  DirectDraw imports mapped to a separate DLL. Native production `CSFIX.dll`
  must pass its own Windows/Wine tests as well.
- The DDRAW provider and nested game-fix core still initialize on first
  DirectDraw use, outside the normal game startup import walk. Dependency
  callbacks invoking DirectDraw *from a third-party DllMain* are not
  guaranteed safe until specifically examined.
- Use the currently approved 1.02 EXE. No other editions, encrypted
  executables, or arbitrary third-party executable patches are supported.
- Proprietary game launch/render/mission validation under native Windows
  and GE-Proton is required before promoting this experiment.
- No game code, copyrighted data, updater or codec is distributed.

This strategy is deliberately independent of the old experimental
`winmm.dll` loader so the known WinMM ordinal-ABI conflict and WinMM
forwarding during loader lock are removed from the design.
