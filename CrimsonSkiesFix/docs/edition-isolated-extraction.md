# Multi-edition isolated CD extraction and update test

Owner's German test (2026-11-10) revealed that the earlier fixed
`CrimsonSkiesFix.extract-test/` directory was still occupied by the
previous French extraction. The old `--extract-cd` correctly refused
to overwrite it, but the subsequent `--extract-update` succeeded on
that **French** directory and misleadingly appeared to validate the
German edition. This was not a successful DE test.

The source of the bug was ambiguous edition provenance: both commands
used the same destination and the update command accepted no edition
selector. The new experimental build fixes this without overwriting
or deleting any existing user data.

## Per-edition staging directories

| Selected original CD profile | Safe extraction target |
|---|---|
| `C_SKIES_DISC_001` | `CrimsonSkiesFix.extract-test.USA/` |
| `C_SKIES_FR_001` | `CrimsonSkiesFix.extract-test.FR/` |
| `C_SKIES_DE_001` | `CrimsonSkiesFix.extract-test.DE/` |

`--extract-cd` determines the edition from **all internal file
paths, lengths and file SHA-256 hashes**, *not* from filename or ISO/BIN
size, and writes only to the new directory for that recognized edition.
No existing target is overwritten. The previous unsuffixed
`CrimsonSkiesFix.extract-test/` is left untouched intentionally.

The update extractor now **requires an explicit edition selector** and
verifies the entire original `GAME/` subset against that edition's
compiled-in CD manifest before extracting any CAB asset. Renaming
a different edition's folder does not bypass this verification.

## German follow-up test under Wine

Place the current experimental launcher, German ISO and verified
1.02 updater beside one another. The old FR directory can remain
untouched.

```bash
WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-cd "Crimson Skies (Germany).iso"
cp CrimsonSkiesFix.log extract-cd-DE.log

WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-update DE "Crimson_Skies_Patch_102.exe"
cp CrimsonSkiesFix.log extract-update-DE.log
```

Expected:

- `--extract-cd`: `Edition=C_SKIES_DE_001`,
  `Destination=.../CrimsonSkiesFix.extract-test.DE`,
  `EXTRACT_RESULT=COMPLETE`
- `--extract-update DE`: `Edition=C_SKIES_DE_001`,
  `AssetStatus=EXTRACTED_VERIFIED_FROM_LZX_CAB`,
  `EXTRACT_RESULT=COMPLETE`

For FR use `--extract-update FR [PATCH]`, and for USA/Europe use
`--extract-update USA [PATCH]`. The PATCH argument is optional if
exactly one verified original updater is adjacent to the launcher.

The update command refuses a missing, partial, mixed, modified or
wrong-edition staging tree, instead of ever claiming success for an
unrelated extracted edition. Stage verification rehashes every original
CD `GAME/` file before opening the updater CAB, so this can entail
a second full read of 500+ MB and may take time.

**These are staging/verification tests only.** The protected original
`CRIMSON.EXE` remains unsupported by the strict runtime gate.
Do not call the result a complete install or merge PR #26 yet.

## Regression tests

- Different editions have different deterministic output directories.
- Unknown edition selectors are refused.
- Empty or incomplete staging is refused before patching.
- Pre-existing staging destinations are refused, never removed.
- All three exact original-CD manifests continue to be required.
