# Crimson Skies Patcher (Rust)

A command-line patcher for the **verified Windows PC Crimson Skies 1.0.2.0 (2000) binary family**. It changes resolution and widescreen field of view (FOV), can apply a narrowly verified Wine/Proton crash workaround, and preserves the original game files before writing changes.

**Experimental:** the supported binary modifications have been checked, but gameplay compatibility across all missions, editions, Windows and Wine/Proton has **not** been established. This tool operates on files you already own and does not ship or download copyrighted game files.

## What does the patch do?

| Feature | Behavior |
| --- | --- |
| **Resolution** | Modifies width and height in a **recognized, already-unpacked widescreen** `CRIMSON.EXE`. |
| **Automatic FOV** | Restores the 60°/80° widescreen reference angles, so the existing widescreen game patch adapts horizontal FOV to the screen aspect ratio. **No `--fov` is needed.** |
| **Optional custom FOV** | `--fov DEGREES` adjusts the reference angles for a target horizontal FOV. |
| **SafeDisc** | **Does not decrypt/unpack SafeDisc.** For the recognized **SafeDisc 1.0.2.0 EXE and matching ICD pair**, it can substitute a **separately provided, verified, already-unpacked widescreen EXE**, with explicit consent and backup. |
| **Wine / Proton** | Optionally applies [CrimsonSkies-Wine-Fix](https://github.com/thehack904/CrimsonSkies-Wine-Fix) to **one exact `roffile.dll` build** to address Preferences / Quit Mission crashes. |
| **Safety** | Identifies file versions, checks backups, refuses unknown files, avoids unnecessary writes, saves originals before modifications and supports restoration. |

The patcher is written in Rust using **only the standard library**: no external crates or Python runtime. GitHub Actions distributes a **Windows x64** `crimson-patcher.exe`, which also runs through **Wine on Linux**. The game EXE being patched is 32-bit x86.

## Compatible game versions

**Supported game:** *Crimson Skies* **1.0.2.0** (**2000, Windows PC**), limited to the exact binary variants listed here. Version 1.0.0.1 is **not supported**. This is **not** a claim of support for all retail editions, languages, fan patches, or digital releases.

**Version 1.0.2.0** was identified from the supplied Windows PE version resources. Identification still uses SHA-256 / verified widescreen structure, **not the version string alone**. Other regional or repackaged 1.02 binaries are not automatically supported.

| Recognized executable | Size | SHA-256 | Available operations |
| --- | ---: | --- | --- |
| Unpacked widescreen, 1920×1080 reference | 2,580,480 bytes | `5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67` | Resolution, FOV, backup, restore |
| Unpacked widescreen, 1920×1200 reference | 2,580,480 bytes | `612c5a9b45a8dbc9d22c78154a995a77d956d5faf90b90b5c31102b4843c8a8d` | Resolution, FOV, backup, restore |
| **SafeDisc 1.02 loader `CRIMSON.EXE`** | 344,851 bytes | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` | Verify paired ICD; optional local replacement **only** |
| **SafeDisc 1.02 payload `CRIMSON.ICD`** | 2,580,578 bytes | `0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b` | Verify only; encrypted executable, **not yet decrypted** |

The widescreen family is recognized **after normalizing only the four modifiable values** (width, height, two FOV reference angles), so an EXE subsequently modified by this tool can still be recognized. Older `CSFIX.dll` / `DDRAW.dll` executable variants are **not confirmed to include the same widescreen implementation** and are intentionally refused.

**When patching the protected 1.02 release, the EXE/ICD pair is required.** Place an unmodified `CRIMSON.ICD` beside the recognized SafeDisc `CRIMSON.EXE`, or pass `--icd PATH`. The SHA-256 of **both** files is checked before any replacement. **The program cannot decrypt the ICD by itself yet.** A recognized unpacked EXE and protected EXE are not necessarily equivalent retail builds. See [SafeDisc 1.02 research notes](docs/safedisc-102-research.md). Keep backups and test the game.

### Read-only SafeDisc research probe

The **experimental `probe` command** verifies the protected **1.0.2.0 EXE + ICD** pair, locates the loader's `DECRPE` marker, and lists ICD PE section sizes and entropy **without modifying either file**. You can also provide a locally owned, verified unpacked widescreen EXE as a reference to report the percentage of different bytes in each section. The reference is used only for **comparison**, never copied or published.

Windows PowerShell:

```powershell
.\crimson-patcher.exe probe `
  --exe "C:\Games\Crimson Skies\CRIMSON.EXE" `
  --icd "C:\Games\Crimson Skies\CRIMSON.ICD"
```

Optional reference comparison:

```powershell
.\crimson-patcher.exe probe `
  --exe "C:\Games\Crimson Skies\CRIMSON.EXE" `
  --icd "C:\Games\Crimson Skies\CRIMSON.ICD" `
  --reference "C:\Backups\Crimson Skies\Unpacked\CRIMSON.EXE"
```

This is an **analysis command, not a decryption command**. The program now recognizes the **SafeDisc 1.50.20** protection version from both original files. That revision has an extra decryption layer using **`dplayerx.dll`** before TEA, so `DECRPE` bytes alone do not yield a usable decoder. The automatic unpacker remains unimplemented and no proprietary game bytes are included in the codebase. If you have the matching `dplayerx.dll`, provide it for **read-only inspection** with `--dplayerx PATH`. See [research notes](docs/safedisc-102-research.md). See [research notes](docs/safedisc-102-research.md).

### Experimental dplayerx.dll memory observation (Windows x86)

The Windows-only CI also builds an **optional 32-bit research utility**, `safedisc-memory-probe.exe`, distributed separately under the **`safedisc-memory-probe-win32`** artifact. This is **not part of the patcher**, and it does not crack SafeDisc. Its sole purpose is to test whether the protected `.txt` section of the exact verified `dplayerx.dll` changes after loading the DLL into an isolated Windows/Wine process.

**Safety warning:** unlike the read-only `probe` command above, the helper invokes `LoadLibraryW`, which **executes DLL initialization code** and may crash or have side effects. Use **only your hash-verified DLL**, copied into an expendable test directory under an **isolated Wine prefix**. It does not modify the original EXE/ICD/DLL files and it refuses to overwrite an existing snapshot. A DLL that refuses to initialize without the game loader is an expected possible outcome.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" wineboot -u
WINEPREFIX="$HOME/.wine-crimson-research" wine ./safedisc-memory-probe.exe \
    --dll ./dplayerx.dll --output ./dplayerx-txt-memory.bin
```

Please share the **textual command output** first; any snapshot may contain copyrighted code and should **not** be committed to the repository. Successful section loading/decryption alone is not a working SafeDisc unpacker. See [research notes](docs/safedisc-102-research.md).

### Troubleshooting Wine error 998: test mapping without executing the DLL

If the optional x86 memory helper reports `LoadLibraryW failed: Windows error 998`, **no memory snapshot was produced**. The new **`--map-only`** mode distinguishes failure to map the PE image from later DLL import processing / initialization, using `LoadLibraryExW` with `DONT_RESOLVE_DLL_REFERENCES`.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" wine ./safedisc-memory-probe.exe \
    --dll ./dplayerx.dll --map-only
```

This mode **does not execute DLL initialization**, does not require `--output`, and writes no file. It cannot recover the SafeDisc decryptor; mapped-vs-disk changes may be relocations and do not establish decryption. Please share only the text result. See [SafeDisc research](docs/safedisc-102-research.md).

### Inspect first-chance DLL faults (experimental)

On Wine Staging 11.17, the standalone 32-bit memory helper can map the verified DLL in `--map-only` mode, but a normal `LoadLibraryW` fails with **error 998**. The first observed exception is an invalid read at `dplayerx.dll` **RVA `0x1BCCD`**. To check whether its actual instructions in memory differ from the encrypted/obfuscated on-disk bytes, download the latest **`safedisc-memory-probe-win32`** artifact and run:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --output ./dplayerx-txt-trace.bin
```

This adds a passive vectored-exception observer before the existing normal `LoadLibraryW` attempt. The latest version also samples 16 **in-memory** bytes at both documented decoder candidate RVAs (`0x65BE` and `0x6621`) during the first `.txt2` exception and the first `.txt` exception. Comparing these samples helps identify when the protection modifies its routines. The previous trace already proved that `RVA 0x7212` changes in memory in a way **not explained by relocations**. It logs the first 12 access-violation addresses and live instruction bytes without changing exception handling, execution registers, or original files. **Loading the DLL still executes its initialization and can fail/crash**, so use only the exact verified DLL in a throwaway Wine prefix. The output filename is required for the existing tool interface; with load error 998 **no memory snapshot is written**. Share the **console text**, not any potential proprietary binary snapshot.

### Optional private SafeDisc two-stage code capture

A verified Wine trace showed that the two candidate SafeDisc modifier RVAs, `0x65BE` and `0x6621`, change all 16 sampled bytes between the first exception in `.txt2` and the first exception in `.txt`, **without any PE relocations at those addresses**. This is an observed runtime transformation, **not proof that the original game has been decrypted**.

For further **private** investigation, the standalone Windows x86 `safedisc-memory-probe.exe` can optionally capture the full 30,321-byte protected `.txt` section at those two moments:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --capture-stages ./safedisc-txt \
  --output ./dplayerx-txt-trace.bin
```

The optional capture outputs are `safedisc-txt-stage1.bin` and `safedisc-txt-stage2.bin`. They are written only after the callback is removed, even if the underlying `LoadLibraryW` reports error 998. If the same filenames already exist, the helper **refuses to overwrite them**. The files contain code from the user's proprietary DLL; **do not commit, redistribute or add them to GitHub releases**. The command still executes the DLL initializer, so use only the verified DLL inside an isolated Wine prefix. More details: [technical notes](docs/safedisc-102-research.md).

### Full Wine Secdrv trace: confirmed retry loop

The complete `safedisc-alias.log` (43,885 lines) shows **3,804 successful opens of the temporary `Secdrv` alias by the DLL** and **1,902 actual `0xEF002407` IOCTL attempts** on handle `0x6C`. An additional successful open of the same alias was performed by the probe itself before DLL loading. Each IOCTL uses a `0x514`-byte request and `0x610`-byte output, with request pointer `0x00C82670` for **that one run only**. The DLL also probes `SICE` and `NTICE` (195 unsuccessful opens each) before terminating with load error **998**.

**Wine `+file` logging does not expose the request's command DWORD at input offset `0x0C`, the IOCTL return status, or response bytes.** The next diagnostic therefore needs one controlled observation at the Win32 `DeviceIoControl` boundary; the pure-Rust SafeDisc protocol model must not be connected to the DLL until the real command and result are observed. See [the research note](docs/safedisc-102-research.md). Nothing has been decrypted or installed as a driver.

### SafeDisc main IOCTL protocol research (no DLL hooks yet)

The alias-assisted `dplayerx.dll` initialization still fails with **Windows error 998** after **9 first-chance access violations**. Its last observed protected-code instruction at RVA `0x7212` is a write to an invalid absolute pointer, so the execution path **has not been established as decrypted or valid**.

The protected DLL does reach `NtDeviceIoControlFile` code **`0xEF002407`** with the exact `0x514`-byte request and `0x610`-byte reply layout described by [SafeDiscShim](https://github.com/RibShark/SafeDiscShim). This project's GPL-3.0-or-later `src/secdrv_protocol.rs` is a **pure, feature-gated Rust research model** of the documented SafeDisc v1 protocol subset. It builds deterministic test responses for commands `0x3C`, `0x3D`, `0x3E`, handles version/checksum fields and validates lengths; it explicitly **rejects unimplemented commands**.

The module can be tested without running any game or DRM code:

```powershell
cargo test --features research --lib
```

GitHub Actions runs these tests on Windows. **This module is not installed as a Windows driver, is not hooked into the game process, does not supply replies to `dplayerx.dll`, and cannot decrypt the original ICD.** The next step is to identify actual input command values and IOCTL return statuses before considering a guarded user-space adapter.

### Compare DLL initialization with a temporary Secdrv alias (research only)

In Wine Staging 11.17, the user's `--test-secdrv-alias` **successfully** opened a temporary DOS-device alias `Secdrv -> \Device\Null`. This resolves the name, but it is **not SafeDisc driver emulation**: the null device cannot handle the vendor IOCTL commands or decrypt an ICD.

The **opt-in** `--load-with-secdrv-alias` mode retains the same temporary mapping for the duration of `LoadLibraryW`, while collecting the existing exception trace. It is intentionally disabled by default and refuses to replace a pre-existing device name.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all,+file,+loaddll \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --load-with-secdrv-alias \
  --output ./dplayerx-alias-memory.bin > safedisc-alias.log 2>&1

grep -Ei 'Secdrv|NtDeviceIoControlFile|Vectored observer|AV #|ERROR:|alias' safedisc-alias.log | tail -n 90
```

**Caution:** unlike the name-only test, this **runs the exact hash-verified protected DLL's initialization code**, which can crash, hang, or terminate the helper. Use only in an expendable Wine prefix, never on a normal Windows installation. On ordinary return the helper requests removal of its own temporary alias; abnormal termination can prevent cleanup. This does **not** implement the SafeDisc IOCTL protocol or automatic EXE/ICD decryption. Do not upload any potentially generated proprietary memory capture to GitHub.

### SafeDisc driver availability: reversible name-resolution test

The complete Wine Staging 11.17 research log confirms **1,902 failed attempts to open `\\\\.\\Secdrv`** while the exact protected `dplayerx.dll` initializes; all returned `STATUS_OBJECT_NAME_NOT_FOUND (0xC0000034)`. This missing device is confirmed, but it is not yet proven to be the sole cause of the subsequent access violation / error 998.

The optional 32-bit research helper has a strictly opt-in `--test-secdrv-alias` mode. It checks whether the isolated Wine environment can temporarily resolve the name `Secdrv` through a DOS-device alias to the null device, **without loading the SafeDisc DLL or emulating driver requests**:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll --test-secdrv-alias
```

Unlike the offline stage comparison, this command **temporarily modifies the DOS-device namespace** inside the test environment and removes only its own mapping during normal cleanup. Run it only in a throwaway Wine prefix; an abrupt process termination may prevent cleanup. The test refuses to replace an existing mapping, does not touch original game files, does not execute `dplayerx.dll`, and does not implement SafeDisc IOCTL responses. A successful alias test **is not** a working SafeDisc driver or an ICD decoder.

### SafeDisc v1 standalone DLL limitations

The matching `dplayerx.dll` has a protected DLL-attach path: entry RVA `0xFD20` calls `_DllMain@12` (RVA `0x9770`), which reaches the exact observed error location in protected `.txt` (RVA `0x7212`). The same DLL statically references the `\\\\.\\Secdrv` device and calls `CreateFileA` to open it. The original SafeDisc loader also references `Secdrv` and `drvmgt.dll`.

This explains why standalone `LoadLibraryW` under an isolated Wine prefix **cannot be assumed to reproduce the original game's decryption conditions**. The open-source [SafeDiscShim](https://github.com/RibShark/SafeDiscShim/issues/17) project independently documents SafeDisc 1 requiring its driver emulation **before** the original `dplayerx.dll` attaches. Therefore, the observed stage-2 `.txt` modifications are **not** a validated, callable cipher implementation. The existing `--analyze-stages` command also verifies this startup path **offline** against the exact hash-verified DLL; no driver, hooks, or game execution are required.

This project still seeks an **autonomous decryption of the original game EXE/ICD**, not a replacement executable, but no autonomous SafeDisc 1.50.20 decoder has been completed yet.

### Compare two captured SafeDisc stages offline (no DLL execution)

After capturing stage 1 and stage 2 from the same original **SafeDisc 1.50.20** DLL, the Windows x86 research helper can compare both snapshots against the user's original `dplayerx.dll` without loading or executing it:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
    --analyze-stages ./safedisc-txt-stage1.bin ./safedisc-txt-stage2.bin
```

This verifies the two inputs' sizes and computes their hashes, entropy, relocation-aware comparisons and candidate window differences. With the supplied exact 1.50.20 captures, **stage 1 was exactly original `.txt` + 214 PE relocations, while 29,164 bytes changed outside relocated words at stage 2**. The second stage begins with an x86-like prologue but is **not proven to contain fully decrypted executable code**, and the game's ICD still cannot be unpacked by this tool.

**The two snapshots contain proprietary code; do not commit or redistribute them.**

### Check your file's SHA-256

Windows PowerShell:

```powershell
Get-FileHash "C:\Games\Crimson Skies\CRIMSON.EXE" -Algorithm SHA256
```

Linux:

```bash
sha256sum "/path/to/Crimson Skies/CRIMSON.EXE"
```

A hash can differ if the file was **already modified by this patcher**; the `status` command also checks its normalized widescreen signature.

## Download (Windows x64 / Wine)

1. Open [GitHub Actions — Build and test Rust patcher (Windows)](https://github.com/legluondunet/crimson-skies-resolution-patcher/actions/workflows/build.yml).
2. Open the latest successful workflow run.
3. Under **Artifacts**, download **`crimson-patcher-windows-x64`** and extract `crimson-patcher.exe`.
4. Copy the patcher to your game directory, or point `--exe` at the installed game executable.

**Only Windows x64 is built by GitHub Actions.** Linux users run this Windows EXE through Wine. No Python installation is needed. Linux-native compilation is still possible manually from source.

**Before patching:** exit Crimson Skies and all Wine/Proton processes using its files. Keep a separate copy of the entire installation during initial testing.

## How to use — Windows

Run the following in **PowerShell** from the directory containing `crimson-patcher.exe`. Change the sample installation path.

**1. Inspect without patching:**

```powershell
.\crimson-patcher.exe status --exe "C:\Games\Crimson Skies\CRIMSON.EXE" --no-wine
```

**2. Patch resolution and calculate FOV automatically** (requires a compatible, already-unpacked widescreen EXE):

```powershell
.\crimson-patcher.exe apply --exe "C:\Games\Crimson Skies\CRIMSON.EXE" --width 1920 --height 1080 --no-wine
```

You can also select another resolution, for example `--width 2560 --height 1440` or `--width 3440 --height 1440`, subject to the tool's resolution and aspect-ratio checks.

**3. Optional custom horizontal FOV (105°):**

```powershell
.\crimson-patcher.exe apply --exe "C:\Games\Crimson Skies\CRIMSON.EXE" --width 1920 --height 1080 --fov 105 --no-wine
```

Omit `--fov` on the next apply command to **restore automatic aspect-based FOV**.

### If your EXE is SafeDisc-protected

This is **not** a SafeDisc unpacker. The 1.02 protected EXE **and companion ICD** are both verified first. If you already own one of the supported **unpacked widescreen** EXEs, store it separately and explicitly authorize **replacement** of your protected EXE after backup:

```powershell
.\crimson-patcher.exe apply `
  --exe "C:\Games\Crimson Skies\CRIMSON.EXE" `
  --icd "C:\Games\Crimson Skies\CRIMSON.ICD" `
  --unpacked "C:\Backups\Crimson Skies\Unpacked\CRIMSON.EXE" `
  --accept-local-replacement `
  --width 1920 --height 1080 --no-wine
```

`--icd` is optional if the matching `CRIMSON.ICD` is beside the protected EXE; it is always hash-checked. `--unpacked` and `--accept-local-replacement` remain required until autonomous decryption is implemented and validated. The supplied EXE is verified against the supported widescreen family; **the patcher cannot establish whether it is identical in gameplay content to the user's retail build**.

## How to use — Linux / Wine

Download the **Windows** build and, for these examples, place `crimson-patcher.exe` in your Crimson Skies directory:

```bash
cd "$HOME/Games/Crimson Skies"
wine ./crimson-patcher.exe status --exe CRIMSON.EXE --no-wine
```

Patch resolution and automatically adjust FOV **without changing the optional DLL**:

```bash
wine ./crimson-patcher.exe apply --exe CRIMSON.EXE --width 1920 --height 1080 --no-wine
```

To **require the Wine ROF workaround**, add `--wine`. The DLL is normally searched for under `GOSDATA/ASSETS/BINARIES/roffile.dll` relative to the EXE:

```bash
wine ./crimson-patcher.exe apply --exe CRIMSON.EXE --width 1920 --height 1080 --wine
```

If the DLL has a different location, provide `--dll PATH`. With `--wine`, missing or unsupported DLLs **cause the operation to fail before changes are written**. `--no-wine` disables *automatic* DLL handling; supplying `--dll` explicitly still requests that DLL.

Without `--wine` or `--no-wine`, the program tries to detect Wine from environment variables and attempts the optional fix when a compatible DLL is found. If using a recognized SafeDisc EXE, add the local `--unpacked PATH --accept-local-replacement` arguments shown above.

## How automatic FOV works

The widescreen game patch uses baseline reference angles of **60° and 80°**. Without `--fov`, our tool restores those constants and the game adjusts the effective horizontal FOV according to the image ratio:

```text
horizontal_fov = 2 * atan(tan(reference_fov / 2) * ((width / height) / (4 / 3)))
```

For the **80° reference**:

| Resolution | Format | Calculated horizontal FOV |
| --- | --- | ---: |
| 1024×768 | 4:3 | 80° |
| 1920×1200 | 16:10 | ~90.4° |
| 1920×1080 | 16:9 | ~96.4° |
| 2560×1440 | 16:9 | ~96.4° |

It is the **aspect ratio**, not pixel count alone, that affects FOV. The values are theoretical and should be confirmed in-game. Custom `--fov` accepts 10°–160°.

## Wine/Proton `roffile.dll` compatibility fix

Adapted from [thehack904 / CrimsonSkies-Wine-Fix](https://github.com/thehack904/CrimsonSkies-Wine-Fix) (MIT license). The upstream workaround targets a parser bug involved in crashes when opening **Preferences** or selecting **Quit Mission**.

**Only this exact original `roffile.dll` is recognized**:

```text
1bc7b4b1adba1bf6a510e62f93473824c662acbf2a2176087b7b4b85338ba58a
```

At **RVA `0x231E`**, the patch changes one instruction byte: `INC ESI` (`46`) → `NOP` (`90`), after PE section mapping and byte-context verification. Different DLL versions are **not** patched. The fix does not change Wine, Proton or Windows itself. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for license attribution and upstream scope.

## Backups, already-patched detection and restore

- **Before writing:** checks EXE/DLL compatibility and verifies any existing backup. Unknown or invalid versions are refused; an unchanged result is reported instead of written again.
- **EXE backup:** `CRIMSON.EXE.crimson-original.bak` (the source filename plus `.crimson-original.bak`).
- **DLL backup:** `roffile.dll.before-nul-guard`.
- **Existing backups are never overwritten.** Changed files are written via same-directory temporary files, with a post-write check.
- **Restore uses the saved backup**. If you originally patched an already-unpacked EXE, restoration does not create a SafeDisc EXE that was never backed up. Backup files remain after restoration.
- Keep an independent game-directory backup in case of file-system errors or crashes.

Restore just the EXE on Windows:

```powershell
.\crimson-patcher.exe restore --exe "C:\Games\Crimson Skies\CRIMSON.EXE" --no-wine
```

Restore the EXE and supported ROF DLL under Wine:

```bash
wine ./crimson-patcher.exe restore --exe CRIMSON.EXE --wine
```

## Build from source

Rust toolchain required for compilation only; **no external crates**:

```bash
cargo test
cargo build --release
```

Output: `target/release/crimson-patcher.exe` on Windows or `target/release/crimson-patcher` when built natively on Linux. **GitHub Actions compiles/tests and uploads Windows x64 only.**

## Safety, limitations and license

- **SafeDisc 1.02 EXE+ICD only:** both files are verified by SHA-256. **No autonomous SafeDisc removal yet:** a local compatible, unpacked EXE is required for replacement; no proprietary EXE/DLL is distributed.
- **Not universal:** unknown EXE/ICD/DLL binaries are refused. The examined original pair reports **1.0.2.0**, but other editions, languages and digital releases have **not** been validated.
- Widescreen visuals, interface proportions and long-session game behavior still need testing.
- A successful binary edit does not establish compatibility of the modified game.
- Do not patch while the game is running.

Original Rust code: **GPL-3.0-or-later**, copyright © 2026 **legluondunet**. The ROF workaround is adapted from MIT-licensed work by **thehack904**; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Crimson Skies and the game files belong to their respective rights holders. This independent community tool is not affiliated with Microsoft.
