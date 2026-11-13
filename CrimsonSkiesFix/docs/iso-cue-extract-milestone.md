# First ISO/CUE-BIN extraction milestone (experimental)

The launcher can now **extract the actual files contained under `GAME/`** from
an approved original disc image, without relying on a mounted drive or the
Windows CD installer.

Command (Windows or Wine):

```sh
WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-cd
# Multiple images next to launcher? Choose one:
WINEDEBUG=-all wine CrimsonSkiesFix.exe --extract-cd "Crimson Skies (Germany).iso"
```

## Conservative extraction contract

- `--extract-cd` selects exactly one adjacent ISO or CUE/BIN or an explicitly
  named image. The same ISO9660/Joliet and CUE data-sector parser is used by
  `--inspect-cd`.
- **Before writing anything**, it scans and SHA-256-verifies every file in
  the CD and requires a **complete, exact** match to one of the three
  reviewed manifests (`C_SKIES_DISC_001`, `C_SKIES_FR_001`,
  `C_SKIES_DE_001`). Unknown, altered or ambiguous editions are refused.
  The image size and whole-image checksum play no role in recognition.
- Copies original files from `GAME/`, removing only that prefix, to the new
  isolated folder `CrimsonSkiesFix.extract-test/`. It preserves the exact
  local-language files, original videos, sound and archives, and writes no
  DirectX/system installers, launcher DLL or registry settings.
- **Every extracted file is hashed as written** and must have exactly the
  size and SHA-256 listed in the verified CD manifest. All file reads are
  bounded to the data track, avoiding audio sectors in shared BINs.
- Refuses a pre-existing test directory (no accidental deletion or
  overwrites). On an interrupted/failed extraction, an incomplete test
  directory may remain for inspection; remove it *manually* after reviewing
  the log. No game files already installed elsewhere are touched.
- Writes `CrimsonSkiesFix.log` beside the launcher with the edition, file
  count, payload size and `EXTRACT_RESULT`. The command never runs any
  protected CD game EXE or official Windows installer.
- This is an **EXTRACTION TEST**, not a playable installation. Do not rename
  `CrimsonSkiesFix.extract-test/` to `CrimsonSkiesFix/`: 1.02 patching,
  SafeDisc handling and final installation publication are not complete yet.

## Before claiming automatic installation

Apply the verified updater's relevant `CRIMPTCH.ROF` in place without
replacing localized resources; integrate a tested LZX decoder for its CAB,
and independently verify the accepted **unwrapped** 1.02 CRIMSON.EXE
before publishing a playable game. The CD's `CRIMSON.EXE` is protected and
intentionally rejected by the runtime DLL. The `unsafedisk` research CLI
is not yet a fully validated, self-contained converter for all three
original-disc variants; do not assume one exists.

## Windows/Wine manual test

Use a new folder with one disc image and the experimental launcher.
Run `--extract-cd`; confirm `EXTRACT_RESULT=COMPLETE`, a
`CrimsonSkiesFix.extract-test/CRIMSON.EXE`, `GOSDATA/ASSETS`, and edition
media. The extracted file count is only the `GAME/` subset of the CD and
is not expected to equal the full manifest's 112/107 file count.

Do **not** merge until tested on genuine ISO/CUE/BIN media on both
Windows 11 and Wine, followed by a full 1.02 game-installation pipeline.
