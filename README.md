# Crimson Skies Patcher (Rust)

A conservative **dependency-free Rust** command-line patcher for locally owned copies of **Microsoft Crimson Skies (2000)**. It handles resolution, aspect-based FOV, optional Wine/Proton `roffile.dll` compatibility and original-file backups.

**Status: experimental.** The byte-level patches were analyzed from supplied game files; changes still need testing in-game. This program does **not** crack/decrypt/unpack SafeDisc. For a recognized SafeDisc EXE, it can *instead use a user-provided, locally owned, verified unpacked widescreen EXE*. It never downloads, embeds, or distributes proprietary game code. Do not assume different regional/retail releases are interchangeable; check in-game behavior.

## Features

- A Windows x64 release executable that also runs under Wine on Linux; Linux-native builds are possible from source but are not distributed by CI. No Python and **no external Rust crates**.
- Recognize only examined x86 EXEs by exact SafeDisc SHA-256 or normalized SHA-256 for a compatible widescreen build. Unknown builds are refused.
- Patch resolution; preserve the existing widescreen patch's automatic FOV scaling (4:3 baseline 60/80 degrees), or specify a horizontal FOV explicitly.
- Detect when the requested EXE patch is already applied; do not write unnecessarily.
- Optionally convert a *recognized SafeDisc-protected* copy into the compatible widescreen form **using another EXE supplied by the user**. This is a local replacement, not SafeDisc reverse engineering. **It requires explicit consent** (`--accept-local-replacement`).
- Under Linux/Wine, attempt the verified [`roffile.dll` NUL guard](https://github.com/thehack904/CrimsonSkies-Wine-Fix) if found in `GOSDATA/ASSETS/BINARIES/`; specify `--dll PATH` when the directory is different. On Windows, this fix runs only if a Wine environment is detected or when `--wine` / `--dll` is specified.
- Verify the original ROF DLL full-file SHA-256; map the target **RVA to file offset** using PE sections; refuse unrecognized DLLs.
- **Create a backup before modifying any original**, use exclusive backup creation, validate existing backups, write through a same-folder temporary file, and offer explicit restore.

## Supported EXE builds

| Variant | SHA-256 | Supported actions |
| --- | --- | --- |
| Widescreen 1920×1080 | `5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67` | Resolution / FOV / restore |
| Widescreen 1920×1200 | `612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d` | Resolution / FOV / restore |
| Known SafeDisc variant A | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` | Identify; local replacement only |
| Known SafeDisc variant B | `7d6029471d4e5f2c7f4b08ce86c0ffd1f9bc8c697cf3952695fd68ff011f25a1` | Identify; local replacement only |

Supported widescreen images are recognized **after normalizing only the four adjustable values** (width, height, 60° reference, 80° reference), so subsequent edits made by this patcher can also be recognized. The older 2,580,480-byte `CSFIX.dll` and `DDRAW.dll` variants with zeroes at those offsets are **not** confirmed to contain the same widescreen code; they are deliberately refused.

This tool does not establish that the supplied unpacked executable is equivalent to a particular retail/SafeDisc build. Retain a complete backup of the installation and test compatibility.

## Usage

Quit the game and Wine processes first. Keep any unpacked game EXEs on **your local disk**, never in this Git repository.

Inspect the game files:

```bash
crimson-patcher status --exe '/path/to/Crimson Skies/CRIMSON.EXE'
```

Change resolution, using **automatic aspect-correct FOV**:

```bash
crimson-patcher apply --exe '/path/to/Crimson Skies/CRIMSON.EXE' --width 2560 --height 1440
```

Change resolution with an optional target horizontal FOV:

```bash
crimson-patcher apply --exe '/path/to/Crimson Skies/CRIMSON.EXE' --width 1920 --height 1080 --fov 105
```

For a **recognized SafeDisc image**, explicitly provide one of your own compatible unpacked widescreen game executables. This **replaces** the protected image after backing it up; it is **not** an automatic SafeDisc unpacker:

```bash
crimson-patcher apply --exe '/path/to/Crimson Skies/CRIMSON.EXE' \
  --unpacked '/path/to/your/compatible/unpacked/CRIMSON.EXE' \
  --accept-local-replacement --width 1920 --height 1080
```

Under native Linux, Wine compatibility mode is automatically selected. If your `roffile.dll` is not at the expected relative path, provide it explicitly:

```bash
crimson-patcher apply --exe '/path/to/Crimson Skies/CRIMSON.EXE' \
  --width 1920 --height 1080 \
  --dll '/path/to/Crimson Skies/GOSDATA/ASSETS/BINARIES/roffile.dll'
```

Use `--wine` to request strict Wine DLL detection/failure if the file is missing, or `--no-wine` to skip the optional DLL modification completely. Use `--dll` to request a particular DLL explicitly even when Wine auto-detection is off.

Restore original EXE and optional DLL from their own verified backups:

```bash
crimson-patcher restore --exe '/path/to/Crimson Skies/CRIMSON.EXE' --wine
```

Original backups have the suffixes `.crimson-original.bak` for the EXE and `.before-nul-guard` for the ROF DLL. **Backups are never overwritten.** Keep the installation backup until you have tested the entire game. When the selected modifications are already present, the program reports that nothing needs changing.

### Automatic FOV

The widescreen-patched game calculates its expected horizontal FOV from the stored reference angles:

```text
horizontal_fov = 2 * atan(tan(reference_angle / 2) * ((width / height) / (4 / 3)))
```

Thus 1920×1080 and 2560×1440 give the same aspect-based FOV (about **96.4°** for the 80° reference mode). If no `--fov` is given, the tool **resets** its reference constants to 80°/60° for consistent automatic scaling, even if a custom FOV had been written earlier.

## ROF fix: scope and attribution

The ROF NUL-guard idea, verified SHA-256, instruction context, and RVA come from **thehack904's MIT-licensed [CrimsonSkies-Wine-Fix](https://github.com/thehack904/CrimsonSkies-Wine-Fix)**. The exact verified original DLL SHA-256 is:

```text
1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a
```

At RVA `0x231E`, the fix replaces `INC ESI` (`46`) with `NOP` (`90`) in `80 3E 20 7D 01 46 89 37`. It does **not** modify Windows, Wine, or Proton itself. Only one DLL build is verified; other ROF builds are intentionally not patched. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Build from source

Install a Rust toolchain **at build time**, then run:

```bash
cargo test
cargo build --release
```

The locally compiled binary is at `target/release/crimson-patcher` on Linux or `target/release/crimson-patcher.exe` on Windows. **No Python is required by users of the compiled release.** GitHub Actions builds and tests **Windows x64 only**, then uploads `crimson-patcher-windows-x64` as a workflow artifact. Run the Windows executable directly on Windows or through Wine on Linux (for example, `wine crimson-patcher.exe status --exe '/path/to/Crimson Skies/CRIMSON.EXE'`). Linux-native builds remain possible manually with Rust installed; no copyrighted game files are bundled.

## Safety and limitations

- Stop the game before applying/restoring a patch; never modify loaded binaries.
- Exact versions only. Unknown EXEs/DLLs fail closed; a matching size or PE version is **not sufficient**.
- SafeDisc conversion cannot be completed from the protected EXE alone with this implementation. You need a compatible unpacked local image, and swapping executable builds may affect compatibility.
- The ROF workaround has only been validated for one DLL build and may not fix every crash. See upstream limitations.
- A patch succeeding at the byte level is not evidence of gameplay compatibility.
- The program preflights requested edits and uses verified backups; an unexpected crash or storage failure still merits manual inspection before retrying.

## License

Original Rust code: **GPL-3.0-or-later**, copyright © 2026 **legluondunet**. ROF-compatible behavior was based on the MIT-licensed work by thehack904 (attribution in `THIRD_PARTY_NOTICES.md`). Crimson Skies and its game files remain the property of their rights holders. This project is not affiliated with Microsoft.