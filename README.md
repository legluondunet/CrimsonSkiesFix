# Crimson Skies Resolution Patcher

A small, source-only Python tool for changing the resolution and widescreen field of view (FOV) in **Microsoft Crimson Skies (2000)** on Windows or Linux. No Wine installation is needed to *patch* the executable.

> [!IMPORTANT]
> This tool only supports **two specific, previously widescreen-patched, unpacked 32-bit executables** that were examined during development. It does not patch arbitrary releases, does not remove disc protection, and does not include or distribute any game files. The effect in-game, particularly custom FOV settings, still needs user testing.

## Features

- Change resolution (width and height) in a recognized `CRIMSON.EXE`.
- Preserve the widescreen patch's **automatic aspect-ratio FOV scaling** by default; `--fov` is optional.
- Optionally set a target horizontal FOV for the patch's 80° reference mode, or restore the original 80°/60° FOV constants.
- Show the currently stored resolution and calculated FOV values (`--info`).
- Refuse unrecognized executables using a SHA-256 signature that normalizes the four editable values.
- Write a **new** executable; never overwrite the input or an existing output file automatically.
- Require only Python 3.10+ from the standard library.

## Compatibility

The two verified source binaries differ only in the height setting:

| Input binary | Resolution | SHA-256 |
| --- | --- | --- |
| Widescreen-patched EXE A | 1920 × 1080 | `5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67` |
| Widescreen-patched EXE B | 1920 × 1200 | `612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d` |

The patcher also accepts its own modified outputs. **The original retail/SafeDisc-protected EXE and differently patched EXEs are not supported.** No binaries are hosted here: you must supply a legally obtained, compatible copy yourself.

## Usage

Download `crimson_patch.py` and run it with Python 3.10 or newer. Commands shown use Linux syntax; Windows users can replace `python3` with `py -3`.

Show current parameters (does not write anything):

```bash
python3 crimson_patch.py CRIMSON.EXE --info
```

Set the resolution to 2560 × 1440 and **keep automatic FOV adjustment**:

```bash
python3 crimson_patch.py CRIMSON.EXE --width 2560 --height 1440
```

Set 1920 × 1080 with an **optional** horizontal FOV target of 105° for the 80° reference mode:

```bash
python3 crimson_patch.py CRIMSON.EXE --width 1920 --height 1080 --fov 105
```

Restore the patch's default FOV references (80° and 60°):

```bash
python3 crimson_patch.py CRIMSON.EXE --reset-fov
```

Choose the output filename yourself:

```bash
python3 crimson_patch.py CRIMSON.EXE -W 1920 -H 1080 --output CRIMSON_1080P.EXE
```

Unless `--output` is specified, the tool creates a file named like `CRIMSON_2560x1440.exe` (or `CRIMSON_1920x1080_FOV105.exe`). Your original input remains untouched. Make a backup before replacing an executable in your game directory. Launch the patched game as you normally would; on Linux this may be through Wine or a game launcher.

## How FOV works

**You normally do not need `--fov`.** The compatible widescreen patch is designed to adjust horizontal FOV according to the aspect ratio. When no custom FOV is specified, the script changes *only* the requested resolution fields and leaves both FOV reference constants alone.

The horizontal FOV predicted by the patch's aspect-ratio formula is:

```text
horizontal_FOV = 2 * atan(tan(reference_FOV / 2) * ((width / height) / (4 / 3)))
```

For the 80° reference mode, approximate values are:

| Aspect ratio | Example | Calculated horizontal FOV |
| --- | --- | --- |
| 4:3 | 1024 × 768 | 80.0° |
| 16:10 | 1920 × 1200 | 90.4° |
| 16:9 | 1920 × 1080 or 2560 × 1440 | 96.4° |

These are **calculated/theoretical values**, not a promise that every camera mode or 2D element in the game is corrected. Custom `--fov` adjusts the two reference constants together; it is not required for standard widescreen resolutions.

## Safety and limitations

- The script checks the entire EXE against a normalized known-good SHA-256 before writing anything. Do not bypass the guard to patch unrelated versions.
- It edits fixed file offsets, so it cannot automatically discover a new EXE layout.
- The minimum resolution is 320 × 240; the maximum is 16384 × 16384, with a supported aspect ratio of 0.5–4.0. Display driver/game support may impose stricter limits.
- `--fov` accepts a target horizontal value between 10° and 160°.
- Custom FOV and gameplay rendering have not been exhaustively tested in-game on Windows or Wine.
- This is an independent fan utility, **not affiliated with or endorsed by Microsoft**.

## Development and tests

The project uses only the Python standard library. Run its unit tests with:

```bash
python3 -m unittest discover -s tests -v
```

An optional integration test against a compatible **locally owned** executable can be enabled with:

```bash
CRIMSON_TEST_EXE=/path/to/CRIMSON.EXE python3 -m unittest discover -s tests -v
```

No copyrighted game binaries are used in CI or committed to this repository.

## License and credits

Patch utility source code: **GNU GPL-3.0-or-later**. See [`LICENSE`](LICENSE).

Maintained by **legluondunet**, assisted by ChatGPT (OpenAI). Crimson Skies and related trademarks belong to their respective owners. The game itself is **not** licensed under this project's license.