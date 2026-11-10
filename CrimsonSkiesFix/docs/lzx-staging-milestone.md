# Pure Rust official 1.02 CAB LZX extraction: test-only staging milestone

The owner's exact verified official patch contains a single embedded CAB
at offset `0x346F0`, size 2,091,693 bytes. The CAB uses Microsoft LZX,
with five audited members (including protected `CRIMSON.EXE/ICD` and
`gosdata/assets/CRIMPTCH.ROF`).

This experimental milestone introduces the **MIT-licensed pure Rust
`cab` crate pinned at 0.6.0**, which decodes LZX on Windows and Wine
without running Microsoft's old setup executable or invoking a system
CAB extraction utility.

## Safe two-command test

First, place a recognized original CD `.iso` or `.cue` + referenced BIN
and the owner's original `Crimson_Skies_Patch_102.exe` beside the launcher.

```sh
WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-cd
WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-update
```

The first command extracts CD `GAME/` files into the isolated folder
`CrimsonSkiesFix.extract-test/`. The second command:

1. Finds the *exact* 2,315,728-byte update EXE by verified SHA-256
   `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af`,
   regardless of its filename. It rejects ambiguous multiple copies.
2. Verifies Intel PE32, the embedded CAB header, and the known 5-member
   inventory, including uncompressed sizes.
3. Uses a fully bounded decoder to decompress the 797-byte
   `gosdata/assets/CRIMPTCH.ROF` entry, and checks that its
   SHA-256 is exactly
   `7ae28cfb65645a3e22aadc64a68badc23e2543bc56356d599fb59375d9ebbefe`.
4. For the French CD, checks that the existing identically hashed
   `CRIMPTCH.ROF` is correct, then **leaves it untouched**.
5. For USA/Europe or DE, adds only that verified asset under the test
   staging directory. It never copies the protected executables, and
   never replaces language-specific files or runs the historical updater.
6. Writes `CrimsonSkiesFix.log` next to the launcher, reporting
   `EXTRACT_RESULT=COMPLETE` and
   `AssetStatus=EXTRACTED_VERIFIED_FROM_LZX_CAB` or
   `AssetStatus=ALREADY_PRESENT_UNCHANGED`.

For explicit updater selection:
`CrimsonSkiesFix.exe --extract-update "Crimson_Skies_Patch_102.exe"`.

## Important limitations

- **No final install is published or game launched by these commands**.
  `CrimsonSkiesFix.extract-test/` contains an unmodified, SafeDisc-protected
  CD EXE, which the runtime DLL intentionally refuses. The approved unwrapped
  1.02 executable must be supplied/reconstructed and independently verified
  before the installer is ready.
- The update's other four CAB members are **not installed** by this test
  command; it is specifically an original *asset* extraction test.
- The user must provide the original updater and their own CD media. Neither
  is bundled in the open-source launcher.
- Failed extractions can leave an incomplete `CrimsonSkiesFix.extract-test/`
  for inspection; the launcher does not silently remove or overwrite it.
- Windows and Wine real-input testing of the new LZX stage is still needed
  before promoting it into the automatic installation pipeline.

The PR remains a **draft** until the full installation and final game
execution have been validated across supported editions.
