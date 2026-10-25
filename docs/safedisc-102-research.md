# SafeDisc 1.02 research status — October 2026

## Scope and verified input files

Only the Windows PC **Crimson Skies 1.0.2.0** protected pair:

| Input | Size | SHA-256 |
| --- | ---: | --- |
| `CRIMSON.EXE` — SafeDisc loader | 344,851 | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` |
| `CRIMSON.ICD` — encrypted game PE32 x86 executable | 2,580,578 | `0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b` |

The other supplied protected EXE (`7d602947…f011f25a1`) reports a different game version (**1.0.0.1**) and is **intentionally excluded**.

The unpacked widescreen reference has a size of **2,580,480 bytes**, the same image base (`0x400000`), entry-point RVA (`0x1F7056`), section names, section virtual addresses and raw offsets as the ICD. This structural match **does not demonstrate** that the encrypted ICD can already be converted.

## Precise DRM version: SafeDisc 1.50.20

The game version (**1.0.2.0**) and the **SafeDisc protection version** are different values. The supplied loader has the `BoG_ *90.0&!!  Yy>` marker at file offset `0x29250` and the consecutive 32-bit version values `1, 50, 20` at `0x29264`. The paired ICD has the same marker at `0x0FD4` and the same values at `0x0FF4`. These have been read directly from both local files, not inferred from the game date.

This matters significantly: [ArthaXerXès' SafeDisc technical write-up](https://gbppr.net/cracking/krobar/other/com26.html) documents an **additional ICD encryption layer beginning with SafeDisc 1.50.20**, using an EAX modification routine from the protection's **`dplayerx.dll`**. Therefore a direct application of TEA decryption on the stored ICD bytes cannot be assumed to succeed. The exact *same-installation* `dplayerx.dll` will be needed for a reliable reconstruction of that routine; no copy was included with the supplied EXE/ICD pair.

**Matching DPLAYERX.DLL supplied and verified (2026-10-25):**

- SHA-256: `dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff`.
- Size: **138,752 bytes**; PE32 i386; image base `0x008E0000`.
- Own SafeDisc marker: `BoG_ *90.0&!!  Yy>` at **file offset `0x1CC58`**, reporting **1.50.20**.
- DLL code sections on disk: `.txt` (raw `0x400`, 30,720 bytes, entropy **7.953**), `.text` (raw `0x7C00`, 58,880 bytes, entropy **6.575**), `.txt2` (raw `0x16200`, 22,016 bytes, entropy **6.246**).
- The two modifier routine **candidate RVAs referenced by the old technical write-up** are `0x65BE` (PE file offset **`0x59BE`**) and `0x6621` (offset **`0x5A21`**), inside the high-entropy `.txt` section. The bytes in that section **are still encrypted on disk**; disassembly at those RVAs from the on-disk DLL is not a decoded routine.

The Rust `probe` command now verifies the **exact matching DLL hash and protection version**, reports its section entropies and maps the two candidate modifier RVAs. It is strictly read-only; **no decoding or SafeDisc bypass has been implemented at this stage**.

**What remains:** recover the DLL's decrypted runtime modifier logic, reproduce the two EAX transforms without executing the protected DLL in the distributed patcher, verify the ICD second-layer decode against the local unpacked reference, and then complete TEA key derivation and PE import reconstruction. The original `DECRPE` bytes alone are insufficient.

## Byte-level comparison of supplied local files

The comparison uses the common 2,580,480-byte file span (ignoring the ICD's 98-byte trailing metadata) and the PE section raw ranges:

| Region | Changed bytes vs. supplied unpacked reference |
| --- | ---: |
| `.text` (2,105,344 bytes) | 1,813,393 (86.1%) |
| `.rdata` (90,112 bytes) | 0 |
| `.data` (172,032 bytes) | 167,371 (97.3%) |
| `.idata` (16,384 bytes) | 3,362 (20.5%) |
| `.rsrc` (24,576 bytes) | 0 |
| `.reloc` (167,936 bytes) | 2 |

The extensive changes in executable code and initialized data **cannot** be repaired by merely changing the PE extension, setting the widescreen constants or replacing the entry point. In particular, an algorithm or verified key is needed to decrypt the contents, and the IAT must be restored.

## Verified loader metadata lead

The exact 1.0.2.0 SafeDisc loader contains the ASCII signature `DECRPE\0\0` at **file offset `0x28E68`**. The bytes that follow are not yet identified as a working game decryption key. The historical technical write-up [*An overview of Safedisc versions*](https://gbppr.net/cracking/krobar/other/com26.html) specifically mentions `DECRPE\0\0` in its discussion of loader-stored raw key material and TEA key derivation. This observation narrows future investigation, but extracting an arbitrary contiguous group of bytes after this marker and treating it as the cipher key **has not been validated**.

## Reverse-engineering leads and required evidence

SafeDisc v1 used the separate `EXE` loader and `ICD` encrypted executable format. Research discussing SafeDisc 1.06–1.11 reports **TEA (Tiny Encryption Algorithm)**, key recovery and post-decryption import table reconstruction:

- [SafeDisc 1.06–1.11 reverse-engineering discussion](https://gbatemp.net/threads/safedisc-1-06-1-11-reverse-engineering-technical-paper.611118/)
- [SafeDisc v1 background](https://en.wikipedia.org/wiki/SafeDisc)

**These are leads, not a validated decoder for Crimson Skies.** The DRM has now been identified as **SafeDisc 1.50.20**, requiring a game-specific investigation of its `dplayerx.dll` second decryption layer **before** TEA and IAT reconstruction. No key, full cipher implementation or working output has been validated for this exact pair. No copyrighted decoded game code or large binary diff may be embedded in or published with this project.

Before claiming autonomous unpacking, an implementation should:

1. Verify the exact original EXE and ICD hashes and PE structure.
2. Derive and validate the required decryption data from the user's own files.
3. Decrypt `.text` / `.data` as appropriate, restore imports and remove only the loader-specific metadata.
4. Reconstruct a working PE32 x86 image **without** embedding copyrighted reference bytes.
5. Compare the output against the locally held reference as a research oracle, including section hashes and import behavior, then test the game in Wine/Windows.
6. Only then expose an automated restore-safe `unpack` path.

## Reproducible, read-only probe

The Rust CLI provides a diagnostic mode that never writes to the input files:

```powershell
.\\crimson-patcher.exe probe --exe CRIMSON.EXE --icd CRIMSON.ICD
.\\crimson-patcher.exe probe --exe CRIMSON.EXE --icd CRIMSON.ICD --dplayerx DPLAYERX.DLL
```

An optional `--reference C:\\path\\to\\unpacked.exe` reports section-level changed byte counts against an already verified local widescreen executable (without bundling or redistributing it). The probe prints the `DECRPE` loader offset, the 16 following bytes **as unverified key material**, and each ICD section's raw range and byte entropy.

A simple TEA decryption experiment using those 16 bytes as the complete key did **not** recover the first 8 bytes of the previously supplied unpacked code (`ICD offset 0x1000`). This negative result is consistent with the documented 1.50.20 additional layer, but does **not** by itself determine the correct key derivation.

## Current implementation

The Rust CLI **verifies both protected files** using SHA-256 (auto-discovers `CRIMSON.ICD` beside the protected loader, or accepts `--icd PATH`), and refuses a missing, corrupt or unknown ICD before modifying anything. With this pair, resolution/FOV modifications require the separately supplied recognized unpacked widescreen executable and explicit `--accept-local-replacement` consent. This is **local replacement, not SafeDisc removal from the provided pair**.

The widescreen patch and the optional narrowly verified `roffile.dll` Wine workaround retain existing safety checks, one-time backups and restoration. Unknown binaries fail closed.

## 2026-10-25: protected DLL changes its code during initialization

The user ran `--trace-exceptions` against the exact verified `dplayerx.dll` with the isolated Wine prefix and `WINEDEBUG=-all`. The helper reported **9 first-chance access violations**. The first **eight** were at **EIP `0x0094BCCD`** (DLL RVA `0x1BCCD`) with fault target `0xFFFFFFFF`; all observed the same **live** 8 bytes as on disk:

```text
RVA 0x1BCCD, original = live = CD 01 75 08 8B D2 90 90
```

The ninth exception was at **EIP `0x00937212`** (DLL RVA `0x7212`) and recorded invalid target **`0xF1563002`**. Its **live code differs from the original DLL**:

```text
RVA 0x7212, file raw offset 0x6612
Original: D4 50 31 C2 C1 CC 5B 04
In-memory: 13 89 A6 35 33 F1 D4 57
```

**Crucial controlled comparison:** inspection of the exact DLL's relocation table shows **no relocation entries in or overlapping either 8-byte interval** (nor near `0x7212` within the preceding four bytes). The PE base-relocation delta cannot explain the **eight changed bytes in the `.txt` instruction stream**. This is the first directly observed runtime modification in `.txt`, likely during the protection's initialization. It **does not yet prove successful decryption** of the relevant modifier routines, or that the actual game could complete SafeDisc initialization.

The eight repeated access violations at `CD 01` and its obfuscated surrounding branches are consistent with a protection/exception control-flow mechanism but do **not** prove the exception is deliberately handled in the original supported Windows environment. `INT 1` (`CD 01`) is distinct from `INT1` (`F1`).

For the next discriminator, the x86 research helper now samples **16 live bytes at both candidate decoder RVAs** (`0x65BE`, `0x6621`) at **two separate stages**: the first `.txt2` access violation and the first `.txt` access violation. `VirtualQuery` supplies the image's actual allocation base; `ReadProcessMemory` observes only the requested regions and does not change exception disposition. This may show whether the modifier routines also change between the observed stages, without writing a proprietary binary dump.

The decoder remains **not implemented**, and the DLL still fails `LoadLibraryW` with Windows error 998 after these observations.

## Next test: inspect actual first-chance exception instruction bytes

The first matched Wine SEH entry occurs immediately after the native `dplayerx.dll` load, at `EIP=0x0094BCCD` (DLL load base `0x00930000`; `RVA=0x1BCCD`; `.txt2` raw offset `0x19ECD`). The exception is `0xC0000005`, an attempted **read** (`ExceptionInformation[0]=0`) from **`0xFFFFFFFF`** (`ExceptionInformation[1]`). Wine then dispatches a handler; it is not yet known if this is an intentionally handled protection fault or a loader failure.

The **on-disk** bytes at `0x19ECD` are `CD 01` (`INT 1`, a two-byte software interrupt), which must **not be conflated** with `INT1` (single-byte `F1`). Neither opcode by itself explains a normal invalid-data-read access violation. Static disassembly could be misleading if the on-disk bytes are modified in memory before execution or if an exception is synthesized. This is a hypothesis to test, not proof.

The x86 helper now offers `--trace-exceptions`: it registers a read-only Windows vectored handler **before** `LoadLibraryW`, records up to the first 12 access-violation addresses and the **live 8 bytes at each faulting instruction pointer** using `ReadProcessMemory` when available, then returns `EXCEPTION_CONTINUE_SEARCH` without modifying the fault context or handler chain. After load returns (including error 998), it prints those observations **without publishing game code or requiring an executable dump**. `--trace-exceptions` cannot be combined with `--map-only`.

Run only in the isolated research prefix using the **hash-verified** DLL and new Windows-x86 GitHub Actions helper:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
    --trace-exceptions --output ./dplayerx-txt-trace.bin
```

An intentionally handled exception might appear without meaning the loader is corrupt; further analysis should compare the first live code bytes with the on-disk `CD 01` sequence. The DLL is still being executed during this test and can crash; no changes to original game files are attempted.

## Wine +seh failure trace: module-local fault addresses

The user recorded the Wine 11.17 `WINEDEBUG=-all,+seh,+loaddll` trace after `LoadLibraryW` returned 998. The native `dplayerx.dll` was loaded at **base `0x00930000`** (the trace shows `EBX=0x00930000`). The following locations are therefore *relative to this session's DLL load base*:

| Fault EIP | RVA | PE section | File raw offset |
| --- | --- | --- | --- |
| `0x0094BCCD` | `0x1BCCD` | `.txt2` | `0x19ECD` |
| `0x00937212` | `0x7212` | `.txt` | `0x6612` |

Each failure was logged as **`0xC0000005` (`EXCEPTION_ACCESS_VIOLATION`)**. The second reported fault included a pointer of `0xF1563022`; this is not evidence of a valid file offset or decryption key.

The trace also passes through native SEH handlers (notably `0x00940748`, RVA `0x10748` inside `.text`). It is plausible that some of these exceptions are part of deliberate copy-protection obfuscation, or that control flow has reached an incompatible/misdecoded path. **No single excerpt proves which**. These are not necessarily the *first* exceptions emitted during `LoadLibraryW`: the user supplied the *last 100 matching log lines*, so next analysis should inspect the **earliest** matching exception plus preceding load events.

On-disk disassembly at `RVA 0x1BCCD` enters a byte sequence beginning `CD 01` (often disassembled as `INT 1`), close to opaque conditional branches; at `RVA 0x7212` the `.txt` region still has nearly maximal entropy. On-disk instructions **cannot be assumed to equal runtime instructions** if SafeDisc modifies code before a fault.

To inspect the initial failure chain, retaining context and avoiding misleading final-repeat log lines:

```bash
grep -En -m 20 'dispatch_exception code=c0000005|loaddll:.*[dD][pP][lL][aA][yY][eE][rR][xX]|ERROR:' safedisc-load.log
grep -B 12 -A 20 -m 1 'dispatch_exception code=c0000005' safedisc-load.log
```

**Relocation cross-check:** for the DLL image base `0x00930000` vs preferred `0x008E0000` (delta `0x00050000`), independently applying the exact **214** `IMAGE_REL_BASED_HIGHLOW` relocations inside `.txt` to its verified on-disk bytes gives **214 changed bytes**, exactly as reported in `--map-only`. These observations strongly support plain PE relocation and **no mapped-only decryption**. The extended helper computes the precise residual when run again.

## Follow-up: map-only succeeds; differences match relocation count

The user ran `safedisc-memory-probe.exe --dll dplayerx.dll --map-only` under Wine Staging 11.17. Image mapping succeeded, and the helper reported **214 changed bytes over the 30,321-byte `.txt` virtual section**, with entropy dropping only **7.9808 → 7.9797**. No code was executed and no snapshot was created.

Independent inspection of the *exact original DLL's* `.reloc` directory at RVA `0x27000` (raw offset `0x20A00`, size `0xE64`) identifies **exactly 214 `IMAGE_REL_BASED_HIGHLOW` (type 3) relocation entries within `.txt`**. This one-to-one count is strong evidence that the observed differences are ordinary PE base relocation adjustments rather than unpacked code.

The updated x86 helper now reconstructs the `.txt` section's expected mapped contents using the actual `LoadLibraryExW` load base minus the DLL's preferred base `0x008E0000`. It reports **how many mapped bytes remain different after applying the exact PE relocation table**, distinguishing relocations from any additional change. The user's updated runtime result is still needed to verify that this residual is zero.

The original `LoadLibraryW` error 998 is therefore **after basic PE mapping**. The precise failure point (import resolution versus initialization / obfuscated code) is not yet identified. To capture the Wine exception trace without modifying the originals, use an isolated Wine prefix:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all,+seh,+loaddll \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --output ./dplayerx-txt-memory.bin > safedisc-load.log 2>&1
grep -Ei 'exception|seh:|dplayerx|ERROR:|err:' safedisc-load.log | tail -n 100
```

Inspect and privately retain `safedisc-load.log`; there may be long, noisy Wine traces. **Do not publish a full proprietary memory snapshot**, if one is eventually generated. A Wine exception trace does not itself implement a SafeDisc bypass.

## 2026-10-25: Wine LoadLibraryW failure 998

The first experimental x86 helper was run on **Wine Staging 11.17** using the exact verified DLL in a fresh Wine prefix. It passed the file hash check, but **`LoadLibraryW` returned error 998 (`ERROR_NOACCESS`)**. No snapshot was written. The loader error is compatible with an invalid memory access or protection logic during DLL loading, but the reported error **does not identify an exact faulting instruction**, so we cannot yet say it is definitely `DllMain`.

Static PE inspection establishes:

- The DLL entry point is **RVA `0xFD20`** in the readable `.text` section.
- Its startup path calls functions at RVA `0x9770` and `0xFC00`. Disassembly near RVA `0x9770` shows obfuscated control-flow and accesses during startup.
- Declared imports are from **`KERNEL32.dll` and `USER32.dll`** only; a missing third-party imported DLL has not been found.
- The DLL has **no TLS callback directory** in its PE data directory. This narrows, but does not prove, the likely initialization path.

The diagnostic helper now offers **`--map-only`**, which uses `LoadLibraryExW(..., DONT_RESOLVE_DLL_REFERENCES)` to map the PE image **without resolving its imports or running `DllMain`**. It **does not execute the DLL or save a snapshot**:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" wine ./safedisc-memory-probe.exe \
  --dll ./dplayerx.dll --map-only
```

If map-only works, but ordinary `LoadLibraryW` fails with 998, the failure occurs during import handling or initialization rather than basic PE image mapping. If map-only fails too, investigate PE image mapping first. **Any differences between memory mapping and raw disk data in map-only mode can be relocations and must not be mistaken for decrypted code**.

## Experimental 32-bit in-memory DLL observer

The Windows-only CI additionally builds a standalone **`safedisc-memory-probe.exe`** for **Windows x86**, because `dplayerx.dll` is 32-bit. It is opt-in and completely separate from the main 64-bit resolution patcher.

After verifying the exact DLL SHA-256 **before** execution, this helper uses Windows `LoadLibraryW` to load the DLL in its own process, samples the mapped `.txt` code section at RVA `0x1000` after initialization, and prints:

- On-disk vs loaded section entropy (bits per byte).
- Number of changed bytes between file-backed `.txt` and loaded `.txt`.
- If bytes differ, the SHA-256 of an optional private, create-only snapshot file.

This does **not** dump or publish game code automatically, modify EXE/ICD/DLL originals, or decode the ICD. **Important safety caveat:** calling `LoadLibraryW` executes untrusted game DRM DLL initialization code and can crash, fail, exit, or have side effects unrelated to the input files. Only the exact user-owned DLL is accepted; run from an *isolated Wine prefix and expendable directory*. An unchanged `.txt` or failed load indicates DLL initialization was insufficient without the SafeDisc loader, not that the files are invalid. The snapshot contains proprietary code; **do not commit it to GitHub**.

Example under a separate Wine prefix (after copying the user-owned `dplayerx.dll` and downloaded helper to an expendable directory):

```bash
WINEPREFIX="$HOME/.wine-crimson-research" wineboot -u
WINEPREFIX="$HOME/.wine-crimson-research" wine ./safedisc-memory-probe.exe \
    --dll ./dplayerx.dll --output ./dplayerx-txt-memory.bin
```

Treat this as an experiment: it has not yet been validated on Wine with the original DLL. A successful section snapshot is **not** a validated SafeDisc decryption routine.

