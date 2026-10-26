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

## 2026-10-26: IOCTL return status definitively observed (Wine +relay)

The user provided the complete `safedisc-ioctl-relay.log` captured with our filtered Wine relay diagnostic: **11,732 lines / 1,218,180 bytes**. A local full-file count (not a truncated grep sample) establishes:

| Directly observed item | Count / value |
| --- | --- |
| `KERNEL32.DeviceIoControl(0x6c, 0xef002407, ...)` entries | **1,902** |
| Corresponding `KERNEL32.DeviceIoControl()` returns | **1,902**, **all `0x00000000` (FALSE)** |
| `ntdll.NtDeviceIoControlFile()` return entries with `0xC0000123` | **3,804** (two relay layers, **not** two separate requests) |
| Main SafeDisc request / response buffer lengths | **0x514 / 0x610** |
| Main request pointer in this run | **0x00D825F0**; command field at input +0x0C is therefore **0x00D825FC** |
| Main output pointer in this run | **0x00D82B04** |
| `dplayerx.dll` image base in this run | **0x00A40000** |
| DLL attach completion | **FAILED**: `exception c0000005 in PE entry point`; DLL attach retval=0 |
| `KERNEL32.GetLastError()` after failed `LoadLibraryW` | **0x000003E6 (998 / ERROR_NOACCESS)** |
| Vectored first-chance access violations | **9**, of which first 8 at RVA `0x1BCCD` |
| Ninth access violation | **RVA `0x7221`** (`EIP=0x00A47221`, operand `0xFFFFFFFF`, raw bytes `A6002AC40E193CEC`) |

Representative contiguous relay sequence (first request, lines 133–138):

```text
Call KERNEL32.DeviceIoControl(0000006c,ef002407,00d825f0,00000514,00d82b04,00000610,0022d6d4,00000000)
Ret  ntdll.NtDeviceIoControlFile() retval=c0000123
Ret  ntdll.NtDeviceIoControlFile() retval=c0000123
Ret  KERNEL32.DeviceIoControl() retval=00000000
```

**The NTSTATUS is `STATUS_FILE_DELETED (0xC0000123)`**, per Microsoft MS-ERREF; this is the **status produced by Wine with the temporary `Secdrv -> \\Device\\Null` mapping**. It is *not* proof that the original SafeDisc kernel driver would return this, nor an authoritative semantic explanation for Wine's internal handling of its null-device alias. See https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-erref/596a1078-e883-4972-9bbc-49e60bebca55 .

Other `NtDeviceIoControlFile` entries with `0xC0000022` refer to console file handles (IOCTL `0x00504000`), **not** to the SafeDisc driver.

**Meaning:** The prior lack of driver-device-name resolution is now fixed only syntactically by the alias. **Every single main SafeDisc request demonstrably fails.** Thus `src/secdrv_protocol.rs` remains an offline scaffold and must not yet be attached to a live call without verifying the actual incoming subcommand and the intended protocol response.

**Variation to avoid over-interpreting:** previous runs showed the ninth AV at RVA `0x7212`; this relay-instrumented run showed it at **`0x7221`**, 15 bytes later. That may result from layout, code transformations, anti-debug responses or instrumentation effects. **There is no controlled evidence assigning a cause to this change.**

**Next observation:** use a debugger or strictly read-only, targeted instrumentation at the `KERNEL32.DeviceIoControl` entry to read **only the 32-bit subcommand at input buffer +0x0C** on the **first** `0xEF002407` request. Reading the returned bytes is unnecessary until the subcommand is known. Do not hardcode the input pointer from this trace in a future run (ASLR and stack addresses vary), or commit original user-owned protocol buffers.

## 2026-10-26: next non-invasive step — filtered Wine relay for IOCTL results

The complete baseline/alias logs prove that the protected DLL successfully opens the temporary `Secdrv -> \\Device\\Null` name, executes **1,902** main SafeDisc `0xEF002407` driver requests, then fails its initialization with error **998** and the same 9 AVs. The `+file` Wine channel logs **entry arguments** but not the Win32 `DeviceIoControl` return value or the command field at `request + 0x0C`.

**New research script:** `scripts/safedisc-relay-probe.sh`. It enables Wine's **`+relay`** channel and uses Wine's documented per-function **`HKCU\\Software\\Wine\\Debug\\RelayInclude`** registry filter to record only `DeviceIoControl`, `NtDeviceIoControlFile`, and `GetLastError` relay events. Unlike dynamic hooking or DLL patching, relay mode observes the existing API boundary with Wine's own built-in diagnostic machinery.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" \
  bash scripts/safedisc-relay-probe.sh ./safedisc-memory-probe.exe ./dplayerx.dll
```

- Use **only** a disposable, isolated Wine prefix with no other applications running.
- The script **refuses to replace** an existing `RelayInclude` registry value, checks the DLL and probe paths, refuses to overwrite an existing log, and requests registry cleanup via an EXIT/INT/TERM trap (normal process completion). It does **not** modify game binaries or install a driver.
- The underlying helper still runs the hash-verified original `dplayerx.dll` initialization with the short-lived null-device alias; it can fail, hang or crash. A forced kill of the shell can prevent registry cleanup; check the prefix before reusing it.
- Wine's relay capture **may not include all internal builtin-to-builtin calls**, and some versions may export `DeviceIoControl` differently; no matching events should be reported as **inconclusive**, not as successful IOCTLs.
- Relay may reveal **BOOL/NTSTATUS return values** and some caller `GetLastError` calls. It **does not** read the first request's actual `Command` field at input offset `0x0C`, which will still require breakpoint-based observation in the future. Do not assume the offline Rust model's `0x3C`–`0x3E` examples are used by this DLL.
- Wine's relay filters are read at process start, so setting `WINEDEBUG` alone without `RelayInclude` would make a much larger trace. **Do not upload private output logs to the public PR.**

Wine implementation reference: https://github.com/wine-mirror/wine/blob/master/dlls/ntdll/relay.c (parses the `RelayInclude` key and `; `-separated function list at runtime). The test syntax is verified in Windows GitHub Actions with `bash -n`; **the experiment has not yet been run on the user's Wine host**.

## Complete alias Wine log audited (2026-10-26)

The user subsequently supplied the entire original `safedisc-alias.log` (**43,885 lines, 4,900,009 bytes**). The log confirms the following exact counts **for research process 0124** (excluding other Wine processes):

| Observation | Count | Supporting sample |
| --- | ---: | --- |
| `CreateFileW(\\\\.\\Secdrv)` starts | **3,805** | first self-test at line 8055; DLL starts at 8294 |
| Successful Secdrv handle returns | **3,805** | first self-test returned `0x4C`; all 3,804 DLL opens returned `0x6C` |
| `KERNEL32_DeviceIoControl` calls for `0xEF002407` | **1,902** | lines 8308, 8325, etc. |
| Matching `NtDeviceIoControlFile` entries for `0xEF002407` on `0x6C` | **1,902** | first at line 8310, last at line 43747 |
| Failed opens for `\\\\.\\SICE` | **195** | `INVALID_HANDLE_VALUE`, e.g. lines 8515 onward |
| Failed opens for `\\\\.\\NTICE` | **195** | `INVALID_HANDLE_VALUE`, e.g. lines 8523 onward |

The most frequent pattern inside DLL attach is **two successful Secdrv opens followed by one `0xEF002407` request**. The input buffer is always passed at **`0x00C82670`**, output buffer at **`0x00C82B84`**, with bytes **`0x514`** and **`0x610`** respectively. The first request is on lines 8308–8310; the last request is line 43747, **immediately before native DLL unload at line 43748**. This is a **bounded, unsuccessful retry loop of 1,902 attempted protocol exchanges**, rather than proof that a single successful driver reply occurred.

Crucially, some Wine lines report `get_nt_and_unix_names ... -> ret c00000cb` for `Secdrv`, but they are *not* final Win32 open results in this alias test: the adjacent `CreateFileW returning 0000006C` lines unequivocally demonstrate valid handles. Treating the intermediate Wine helper code as a failed device open would be erroneous.

**Remaining observability gap:** Wine's `+file,+loaddll` trace logs each `NtDeviceIoControlFile` **entry**, but not an unambiguous per-call return `NTSTATUS` or the contents of the in/out packet. The command ID field inside a 0x514-byte request is at **offset `0x0C`**, corresponding to live address **`0x00C8267C`** in this particular run. This address is **specific to the observed session**, not a fixed offset for all runs; it should never be hardcoded into the patcher. We cannot claim which protocol subcommands were attempted.

The standard Wine messages do show that `LoadLibraryW` **still returns error 998** after all retries (last error near line 43830), and the same nine first-chance exceptions remain. The `0x00504000` IOCTLs on console handles `0x0C`/`0x10` are **not** part of the Secdrv protocol. Debugger-device checks of `SICE` and `NTICE` are separate from this protocol as well.

**Next technical gate:** obtain the actual request command word and actual `DeviceIoControl` return/last-error value from a **single real call**, using controlled debugger or call-boundary instrumentation. Only after that should any userspace adapter feed responses from `src/secdrv_protocol.rs`; the current pure Rust response model remains unconnected and unvalidated against actual inputs. No proprietary packet contents are committed.

## Null alias clean run: IOCTL reached but DLL initialization still fails

The user repeated the `--load-with-secdrv-alias --trace-exceptions` test with **`WINEDEBUG=-all`** (no trace interleaving) against the original hash-verified `dplayerx.dll`. It clearly confirmed:

- The temporary `Secdrv -> \\Device\\Null` name mapping was created and opened.
- The helper requested cleanup of the alias after the DLL loader returned.
- There were still **9 first-chance access violations**: 8 in `.txt2` at RVA `0x1BCCD` with live bytes `CD0175088BD29090`, followed by a ninth at RVA `0x7212`.
- During the ninth fault, with the DLL loaded at `0x00940000`, the live 8 bytes were `A34EECE3726BF637`; `A3 4E EC E3 72` is x86 `MOV [0x72E3EC4E],EAX`, a **write** to the faulting target `0x72E3EC4E`. A random-looking absolute store at this entrypoint is not evidence the code has reached a valid decoder.
- The two 16-byte `.txt` candidate regions sampled at the ninth fault became `83C4C7B266C1CE56BB08D30FE494913F` (RVA `0x65BE`) and `807D0870D1EA2F1823C6D689DB0EC040` (RVA `0x6621`). These differ from the candidate samples in earlier **no-alias** runs, but the observed DLL load bases also differ and other runtime factors may affect the transformation. There is no controlled proof that the alias uniquely caused the different bytes.
- **`LoadLibraryW` still returned error 998.** Name resolution by itself is therefore insufficient.

The user-provided earlier filtered Wine `+file,+loaddll` trace did show multiple `0xEF002407` requests with input `0x514` and output `0x610`, matching the documented SafeDisc main driver packet layouts. However, **there are no confirmed successful protocol replies** from a real or emulated driver. The next major requirement is to inspect the actual command fields and implement only responses validated against real requests, not infer decryption from a null-device alias.

### GPL-licensed pure Rust protocol research scaffold

A new **feature-gated, side-effect-free** Rust module `src/secdrv_protocol.rs` implements a deterministic subset of the documented `0xEF002407` command format, adapted from the GPL-3.0 SafeDiscShim implementation with attribution:

- Validates the main IOCTL and exact 0x514 input / 0x610 or 0xC18 output buffer lengths.
- Implements synthetic responses for only documented commands `0x3C`, `0x3D`, and `0x3E`, including the verification-data arithmetic and the shim's version and extra-data fields.
- Rejects every unknown or unsupported command; no custom API hooks, kernel device, Windows injection, original EXE/ICD modification or proprietary code included.
- Built only when Rust feature `research` is requested; Windows CI runs `cargo test --features research --lib`.

**This is a tested protocol-model component, not a working user-space driver, DLL loader hook, or ICD decryptor.** The next integration milestone is to observe the real first `0xEF002407` request's command word and return status, then adapt the verified response path under an explicitly isolated test harness.

References: https://github.com/RibShark/SafeDiscShim/blob/main/src/secdrv_ioctl.cpp and https://list.winehq.org/hyperkitty/list/wine-devel%40list.winehq.org/thread/A2SGOS3ZAABYF3LBHRLGNSFL3GQRBDIO/ .

## 2026-10-26: Actual SafeDisc driver IOCTL reached with null-device alias

The user ran the optional `--load-with-secdrv-alias` experiment under Wine Staging 11.17 and provided the **last 90 filtered lines**, *not yet the full raw `safedisc-alias.log`*. The alias was removed on ordinary return.

**The earlier 90-line filtered excerpt displayed five native calls; the full uploaded log later confirmed 1,902 identical IOCTL invocations:**

```text
0124:trace:file:NtDeviceIoControlFile (0x6c,(nil),(nil),(nil),
  0x10020f500,0xef002407,0xc82670,0x00000514,0xc82b84,0x00000610)
```

- `0xEF002407` is **exactly** the main SafeDisc driver IOCTL `secdrvIoctl::ioctlCodeMain` defined in the independently developed GPL-3.0 project [SafeDiscShim](https://github.com/RibShark/SafeDiscShim/blob/main/src/secdrv_ioctl.h). It is not a random or Wine-console IOCTL.
- Input size `0x514` (**1,300 bytes**) and output size `0x610` (**1,552 bytes**) exactly match the layouts in [SafeDiscShim's `MainIoctlInBuffer` and `MainIoctlOutBuffer`](https://github.com/RibShark/SafeDiscShim/blob/main/src/secdrv_ioctl.cpp). The in-buffer is three version DWORDs, command DWORD, 0x100 verification DWORDs, size DWORD, 0x40 extra DWORDs. The out-buffer is three version DWORDs, 0x100 verification DWORDs, size DWORD, 0x80 extra DWORDs.
- All five calls use handle `0x6c`, consistent with a newly opened null-device alias, but attribution to a specific `CreateFileW` return requires the **unfiltered log**.
- This is a significant observed change relative to the earlier missing-driver baseline: the protected DLL **reaches the correct SafeDisc main IOCTL interface after the DOS-device alias is installed**.
- The log also includes later `0x00504000` IOCTLs on **other handles** (`0x0c`, `0x10`) associated with console output; these must not be mistaken for SafeDisc IOCTLs.
- The same **nine first-chance access violations** are still reported (eight at the relocated first-fault RVA `0x1BCCD`, one at RVA `0x7212`), and alias removal is logged. The ninth memory value now differs from prior runs; protected-code state or execution conditions may vary.
- **Crucial limitation:** the filtered excerpt shows only IOCTL **entry calls**, **not their NTSTATUS return values or populated output structures**. A successful `NtDeviceIoControlFile` call cannot be inferred; a null device provides no SafeDisc command emulation. The full terminal `ERROR:` message was interleaved with Wine file trace, so we cannot conclude from this excerpt whether `LoadLibraryW` still returned 998.

**Next discriminating evidence:** inspect/upload the **unfiltered** `safedisc-alias.log` and run one clean summary with Wine tracing disabled:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --load-with-secdrv-alias \
  --output ./dplayerx-alias-memory.bin
```

The helper can then report the actual `LoadLibraryW` outcome, and comparison with the original no-alias results will distinguish "device name missing" from "driver IOCTL protocol missing". The clean run **still executes untrusted DLL initialization** in the isolated Wine research prefix, and may fail/hang. Any output file is proprietary and must not be published to the repository.

Further work should validate the protocol behavior rather than attempt to invoke the still-obfuscated two `dplayerx.dll` cipher routines directly. **No functioning autonomous ICD unpacker exists yet.**

## 2026-10-26: Successful ephemeral Secdrv name test and next comparison

The user ran `--test-secdrv-alias` in an isolated Wine Staging 11.17 prefix and confirmed `DefineDosDeviceW` successfully creates a temporary mapping from `Secdrv` to `\Device\Null`; a subsequent `CreateFileW(\\.\Secdrv)` opens it successfully. The alias-only test did NOT execute the protected DLL, respond to a SafeDisc IOCTL, modify any original game files, or implement an ICD decoder.

**New opt-in test:** `--load-with-secdrv-alias` creates the same temporary name **before** attempting `LoadLibraryW` and removes it once loading returns. The tool verifies the exact DLL SHA-256, refuses any existing `Secdrv` namespace entry, requires `--trace-exceptions`, and continues reporting first-chance access violations. It does not emulate `secdrv.sys`: once the missing name is resolved, any driver IOCTL will still lack real protocol support.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all,+file,+loaddll \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --load-with-secdrv-alias \
  --output ./dplayerx-alias-memory.bin > safedisc-alias.log 2>&1

grep -Ei 'Secdrv|NtDeviceIoControlFile|Vectored observer|AV #|ERROR:|alias' safedisc-alias.log | tail -n 90
```

**Interpret carefully:** a successful `CreateFileW` from the DLL followed by a failed `NtDeviceIoControlFile` would confirm reaching an unimplemented kernel-driver protocol. A persistent 998 failure without IOCTLs means the alias is insufficient to fix initialization. Even successful `LoadLibraryW` would not establish a working ICD decryptor.

**Use only in an expendable Wine prefix.** This test executes the DLL and may hang, crash or terminate the process; an abrupt exit can prevent alias cleanup. No original `secdrv.sys` kernel driver should be installed; no private binary or memory capture should be committed to GitHub.

## Verified Wine Secdrv I/O failures: 1,902 sequential attempts (2026-10-26)

The user supplied the original **26,436-line** `WINEDEBUG=-all,+file,+loaddll` log (about 2.97 MB) of the standalone DLL research probe under Wine Staging 11.17. We reviewed the **full file**, not just grep snippets.

**Strong new direct evidence:**
- The x86 research process (`0124`) loaded the original `dplayerx.dll` at **`0x00940000`** (Wine log line 8053; ASLR differs from prior runs, as expected).
- Between lines **8271 and 26295** the same process made **exactly 1,902 `CreateFileW("\\\\.\\Secdrv")` attempts**, all requesting `GENERIC_READ | GENERIC_WRITE`, `FILE_SHARE_READ | FILE_SHARE_WRITE`, `OPEN_EXISTING`.
- Every attempt was resolved by Wine to NT object name **`\\??\\Secdrv`**, and every one failed. `NtCreateFile` logged name resolution `c00000cb`, followed by `CreateFileW` reporting **`STATUS_OBJECT_NAME_NOT_FOUND (0xC0000034)`** and returning **`INVALID_HANDLE_VALUE (0xFFFFFFFF)`**. First cycle lines 8271–8278; last cycle lines 26295–26302.
- Immediately after the final unsuccessful attempt, at **line 26303**, Wine unloaded the native `dplayerx.dll`. The observer then printed its **nine access violations**. The log confirms the DLL **was not able to establish a usable Secdrv device handle before failing**.
- `NtDeviceIoControlFile` entries seen **after** the DLL unload are on other handles (for console output) and must **not** be misidentified as SafeDisc driver IOCTLs. The log has **no successful `Secdrv` opens and no evidenced IOCTL sent to an opened Secdrv handle**.

**Conclusion with careful causality:** missing `Secdrv` in the isolated Wine prefix is now **confirmed** and a compelling explanation for the loader error 998, but we cannot prove from this one run that driver availability is its *only* missing precondition. The existing second-stage `.txt` modifications do not prove a usable decrypted SafeDisc decoder.

**New minimal next experiment:** the optional Windows x86 helper has `--test-secdrv-alias`, which verifies the exact DLL hash **without executing it**, refuses to overwrite any existing `Secdrv` mapping, temporarily creates a DOS-device symbolic link from `Secdrv` to `\\Device\\Null` via `DefineDosDeviceW`, tests a matching `CreateFileW` call, then uses a Rust destructor to request removal of the alias. Run **only in the dedicated expendable Wine research prefix**, not in a normal Windows session:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll --test-secdrv-alias
```

The test **does not load the DLL, run its code, install a driver, respond to any SafeDisc IOCTL, decrypt the ICD, modify game files, or publish proprietary code**. It *does* temporarily create an operating-system DOS device name; abrupt process termination may prevent cleanup, so keep the test prefix isolated. Windows/Wine may reject alias creation or opening; any success means *only* that the name can be resolved to a null device, **not** that SafeDisc is emulated. This test has not yet been run under the user's Wine environment.

## Next step: identify whether Secdrv device I/O precedes the DLL fault

The relocation-aware offline stage analyzer now passes under **Wine Staging 11.17** using the exact protected DLL and both user-owned snapshots:

- Stage 1 unexpected bytes after relocations: **0**.
- Stage 1 → 2 differences: **29,164 / 30,321** (96.18%), all **outside** relocated words.
- Both documented EAX modifier candidate RVAs change **16/16** bytes.
- Static protected DLL startup calls and embedded `Secdrv` device path check out.

This validates *a code transformation*, but not successful decryption. The sampled candidate 256-byte windows still have high, cipher-like entropy (about **7.2 bits/byte**) and implausible disassembly. Even the first plausible x86 prologue in stage 2 quickly enters implausible/privileged instructions.

The high-value next question is **whether `dplayerx.dll` actually attempts to open `\\\\.\\Secdrv` before the observed failure**, as distinct from merely containing a code path that opens that device. A passive Wine **`+file,+loaddll`** trace is the safest next test:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all,+file,+loaddll \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
    --trace-exceptions --output ./dplayerx-filecheck.bin \
  > safedisc-file.log 2>&1

grep -Ein -C 2 'secdrv|createfile|ntcreatefile|error 998' safedisc-file.log | tail -n 100
```

An occurrence of `Secdrv` from the **file open trace**, together with its return code, supports diagnosing missing driver emulation. The **absence** of a logged `Secdrv` access does not strictly rule it out (Wine trace coverage can vary, and other names/paths may be used). We must not infer from static strings alone that a missing driver caused exception 998.

**Avoid installing original `secdrv.sys` kernel drivers or copying proprietary DLL code into the repo.** The GPL-licensed SafeDiscShim is a useful technical reference for a userspace emulation layer on Windows, but its Windows process-injection integration is **not** a drop-in Wine fix, and it does not recover or implement the game-specific ICD decryption automatically. For compatibility background see https://github.com/RibShark/SafeDiscShim/issues/17 and https://github.com/RibShark/SafeDiscShim/blob/main/src/hooks.cpp .

## 2026-10-26: Why standalone DPLAYERX.DLL loading is likely insufficient

**New direct evidence from the exact original user-owned PE, not inferred from the Wine error alone:**

- The DLL's PE entry point is **RVA `0xFD20`**. At **RVA `0xFD81`** it contains a direct `CALL rel32` to **RVA `0x9770`** (the `_DllMain@12` export).
- During DLL attach, code at **RVA `0x989F`** contains a direct `CALL rel32` to **RVA `0x7212`**, within the ciphered `.txt` section. This is **the same RVA as the ninth observed access violation**. This creates a concrete static connection from the startup path to our in-memory protected-code fault.
- The DLL contains literal **`Secdrv`** at raw offset **`0x1D524`**, and **`\\\\.\\%s`** at raw offset **`0x1D52C`**. At **RVA `0xECC9`**, instructions push the preferred-base virtual addresses of these strings; the routine calls `wsprintfA`. Another instruction at **RVA `0xED09`** calls the `CreateFileA` import through the IAT. This code attempts to construct and open the SafeDisc driver device path `\\\\.\\Secdrv`; the exact runtime return value was **not** observed in our current DLL memory logs.
- Independently, the SafeDiscShim maintainers found that **SafeDisc v1's `dplayerx.dll` can crash during DLL initialization without an emulated `secdrv` driver already ready**; for that compatibility shim, initialization had to happen before the DLL attach stage. See https://github.com/RibShark/SafeDiscShim/issues/17 (May 2024).

**Interpretation:** The isolated `LoadLibraryW` test did not reproduce the original protected EXE/ICD/driver/CD environment. The ninth access violation and the partly transformed `.txt` section may be downstream of absent DRM prerequisites or intentionally opaque exception-driven code. A plausible x86 prologue in stage 2 does not suffice to assert that the two EAX modifier routines are decrypted and callable. Repeating isolated `LoadLibraryW` calls will not recover their correct logic unless the missing context is reconstructed.

The offline research helper now verifies this direct-call chain, driver strings, and `CreateFileA` thunk against the exact hash-verified DLL without executing any DRM code. It **does not** install a kernel driver, patch the SafeDisc DLL, or redistribute original game code.

**Technical direction:** rather than treating the incomplete stage-2 memory image as an unpacked DLL, investigate the required initialization conditions and derive the exact modifier transforms against independently checked ICD data. The two binary stage snapshots remain private and are not committed or released. This research change does not implement or claim a working automatic SafeDisc bypass.

## Verified full .txt stage captures: relocation-aware comparison (2026-10-26)

The user supplied **both 30,321-byte private snapshots** and their SHA-256 matched the helper's console output:

| Image | SHA-256 |
| --- | --- |
| First `.txt2` AV (stage 1) | `9891f87ff250a46c32c44c91525d213e38e9fb36290ce750758572e6fc7c1fbb` |
| First `.txt` AV (stage 2) | `e09b39802625972d5664ffae7d4e3b3e26280c7a070129d2c69b08e89c283c15` |

**Independent local validation against the original, hash-verified `dplayerx.dll`:**

- The DLL PE relocation directory contains **214** `IMAGE_REL_BASED_HIGHLOW` entries in `.txt`; the runtime base is `0x00930000`, preferred base `0x008E0000`, delta **`0x00050000`**.
- Applying all 214 PE relocations to the original DLL's `.txt` raw section reproduces **stage 1 exactly**: **0 residual changed bytes**.
- Exactly **29,164 of 30,321 bytes (96.18%)** differ between stage 1 and stage 2; **1,157 bytes** are unchanged.
- All **856 bytes** belonging to the 214 relocated four-byte windows are still unchanged between stage 1 and stage 2. All **29,164 changes are outside these windows** (29,164 of 29,465 non-relocated bytes differ). This is very strong evidence the protection is **relocation-aware when transforming its own code**.
- `.txt` entropy falls only from **7.9797** to **7.9205 bits/byte**.
- At `RVA 0x1000`, stage 2 begins with a plausible ordinary x86 prologue: `55 8B EC 51 53 56 57` (`push ebp; mov ebp,esp; push ecx; push ebx; push esi; push edi`).
- Nevertheless disassembly quickly becomes implausible, including instructions that would be privileged or use unrealistic absolute targets, and the sampled candidate modifier addresses `0x65BE` and `0x6621` are **not currently a demonstrated usable, coherent x86 routine**. **Stage 2 is not established as a fully unpacked DLL**, much less as a decrypted game ICD.

These exact statistics can now be reproduced **locally** using the optional Windows x86 research helper, with the original DLL available, but **without executing the DLL or writing any file**:

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --analyze-stages ./safedisc-txt-stage1.bin ./safedisc-txt-stage2.bin
```

The helper checks the exact DLL hash, infers the relocation delta from the stage 1 image, checks all 214 relocation targets, refuses to interpret a nonmatching stage 1, and reports stage-to-stage changes both inside and outside relocated windows. The two proprietary stage snapshots are **not stored in GitHub**.

**The remaining decryption challenge:** SafeDisc 1.50.20's documented second ICD encryption layer uses two build-specific EAX modifier routines; the historical article provides a block-level algorithm, but **not the decoded routines from this specific game DLL**. Before implementing an autonomous ICD decoder, we must reconstruct and independently test those transformations, remove the second ICD layer, then derive/validate the TEA layer and repair the imports. No attempt to extract a usable EXE is yet validated.

Historical algorithm reference: https://gbppr.net/cracking/krobar/other/com26.html

## Candidate modifier stages and private .txt captures (2026-10-26)

A follow-up run using the exact matching user-owned `dplayerx.dll` under Wine Staging 11.17 captured 9 first-chance access violations and sampled both candidate SafeDisc modifier RVAs at the **first `.txt2`** exception and the **first `.txt`** exception. The two 16-byte samples are:

| DLL RVA | First `.txt2` exception | First `.txt` exception |
| --- | --- | --- |
| `0x65BE` | `E5FFCA9302813EFE83265C43B5A25AA9` | `0FDF7CD205FCB8451707DBC079F60436` |
| `0x6621` | `DEE62461170C88C75BC9F4635EBABD24` | `847C0FF35FF8F729038455576734F82E` |

**Direct comparison against the supplied original DLL** confirmed the **first-stage 16 bytes are identical to the original file for each candidate**, and the **second-stage 16 bytes differ at all 16 positions for both**. Inspection of the verified PE relocation directory confirmed **no relocation entries overlap either of these 16-byte windows**, ruling out relocation as the cause of these specific changes. The transformation occurs **between the first `.txt2` and first `.txt` access-violation observations**. A transform has been observed, but there is no proof these small windows are valid executable code, that both are the correct modifier routines, or that the whole section is decrypted.

### Next research step: private stage-local memory snapshots

The Windows x86 helper now optionally supports `--capture-stages PREFIX` *with* `--trace-exceptions`. During the first `.txt2` and first `.txt` exceptions it copies the entire `.txt` virtual section (**30,321 bytes**) into preallocated buffers. It writes the two private files **only after the vectored exception handler has been removed and `LoadLibraryW` returns**, including after the expected error 998. The source DLL, game EXE, and ICD are never modified. Normal `--trace-exceptions` calls still write **no** stage snapshot unless expressly opted in.

```bash
WINEPREFIX="$HOME/.wine-crimson-research" WINEDEBUG=-all \
  wine ./safedisc-memory-probe.exe --dll ./dplayerx.dll \
  --trace-exceptions --capture-stages ./safedisc-txt \
  --output ./dplayerx-txt-trace.bin
```

Potential local files:
- `safedisc-txt-stage1.bin` — `.txt` at first `.txt2` access violation.
- `safedisc-txt-stage2.bin` — `.txt` at first `.txt` access violation.

The helper reports SHA-256, entropy, byte counts vs the raw on-disk section, and changed-byte count between stages; files are create-only and cannot overwrite previous captures. **These files contain executable bytes from proprietary user-owned DRM code and are strictly for private research. Do not commit, attach to the public PR, or include them in a release.** A successful snapshot will support further analysis of code transformation and the candidate routines, but **does not itself yield an ICD decryptor**.

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

