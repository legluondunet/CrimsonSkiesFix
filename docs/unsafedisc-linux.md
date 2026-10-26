# unSafeDisc Linux — experimental native port for Crimson Skies

This branch begins a **native Linux Rust implementation** of the unSafeDisc
1.5.5 workflow for the *Windows PC 1.0.2.0* release of **Crimson Skies**.

**Current status: inspection only, not a working unpacker.** The new
`unsafedisc-linux` binary reads and verifies locally supplied game files,
extracts SafeDisc-specific key material and optionally validates a reported
32-bit import-address XOR key against a known working unwrapped executable.
It **does not** modify or reconstruct a game executable, run a Windows DLL,
interact with `secdrv`, load Wine, or decrypt the protected ICD.

## Build natively

Requires a standard Rust toolchain on Linux; no third-party crates:

```bash
cargo test --bin unsafedisc-linux
cargo build --release --bin unsafedisc-linux
```

An experimental x86_64 Linux build is also produced under
[GitHub Actions](../.github/workflows/unsafedisc-linux.yml) on this branch.

## Inspect your own original files

Keep the protected SafeDisc `CRIMSON.EXE`, `CRIMSON.ICD` and
`dplayerx.dll` separately from any patched or unwrapped copies.

```bash
./target/release/unsafedisc-linux inspect \
  --loader './CRIMSON(1).EXE' \
  --icd './CRIMSON.ICD' \
  --dplayerx './dplayerx.dll'
```

Optionally compare against your **locally obtained** and verified working
`TESTME.EXE`, providing the key displayed by unSafeDisc 1.5.5:

```bash
./target/release/unsafedisc-linux inspect \
  --loader './CRIMSON(1).EXE' \
  --icd './CRIMSON.ICD' \
  --dplayerx './dplayerx.dll' \
  --reference './TESTME.EXE' \
  --reported-key 0x7496538F
```

### If `--reference TESTME.EXE` reports a SHA-256 mismatch

The separately supplied Python widescreen resolution patcher modifies the
**game executable** in place. This changes its SHA-256, although the original
`TESTME.EXE` from unSafeDisc was valid. The native inspector **intentionally
requires unmodified plaintext** for the import-address XOR comparison and
will not silently accept an already patched executable.

If you already tested the widescreen patch, the Python patcher created
`testme.exe.crimson-original.bak` next to `testme.exe` before modifying it.
Check both without changing them:

```bash
sha256sum testme.exe testme.exe.crimson-original.bak
```

The original backup must have this full SHA-256:

```text
43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75
```

If the backup matches, add this to the `inspect` command:

```bash
--reference testme.exe.crimson-original.bak
```

**Do not restore or overwrite the tested, patched `testme.exe` just to
perform read-only research.** If no verified backup is present, use a separate
unmodified `TESTME.EXE` generated from your original files.

All three original input files are checked against the **exact SHA-256 and
size** of the known compatible originals. The optional working reference is
also checked by SHA-256 and size; arbitrary game builds are refused.

Expected results for the specific user-provided pair:

| Measurement | Expected |
| --- | --- |
| Protected loader SHA-256 | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` |
| Protected ICD SHA-256 | `0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b` |
| SafeDisc DLL SHA-256 | `dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff` |
| Unwrapped `TESTME.EXE` SHA-256 | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |
| Raw-key DWORD offset in protected loader | `0x28FB8` |
| Expected four raw-key DWORDs | `E51DC3D7 63CA5EAC 4493379C 08B42B8C` |
| Entry-point RVA, ICD and TESTME | `0x1F7056` |
| Exact matching 32-bit import-address XOR DWORDs using `7496538F` | 8 |

The raw-key offset selection follows the disassembled original unSafeDisc
1.5.5 tool: it scans for `CRPE\0\0` and, for a 138,752-byte
`dplayerx.dll`, adds `0x148` before advancing past the 6-byte marker.
The older hypothesis of reading the four DWORDs directly after
`DECRPE\0\0` is **wrong for this game**.

## Technical milestones

| Milestone | Status |
| --- | --- |
| Native Linux Rust command-line executable | Implemented |
| Known-file SHA-256 and PE32 structural verification | Implemented |
| Version-specific unSafeDisc 1.5.5 raw-key offset | Implemented |
| Optional known-plaintext import-address XOR check | Implemented |
| Standard 32-round TEA primitive | Tested with synthetic data only |
| Isolated unSafeDisc 1.5.5 checksum byte-mixing inner loop | Implemented, synthetic vectors only |
| Full checksum traversal (PE section flags, relocations, DLL modifiers) | Not implemented |
| Game-specific EAX modifiers in `dplayerx.dll` | Not implemented |
| Recovery of the complete, *verified* 128-bit TEA key | Not implemented |
| ICD decryption, call repair and import reconstruction | Not implemented |
| Output verification against working `TESTME.EXE` | Future milestone |

The historical tool also contains special handling of game sections and
relocations, confirmed by the supplied unSafeDisc 1.5.5 changelog. Its
`history.txt` says version 1.5.2 was once released with source, but
**that source code is not present in the user's 1.5.5 ZIP**. We are not
repackaging the Windows executable.

## Checksum groundwork (not yet a decryption key)

Static disassembly of the user's original unSafeDisc 1.5.5 `unSafedisc.exe`
at **`0x401F0D..0x401F36`** identifies the inner byte mixer used for
checksum accumulation:

```text
weight = 0xFD379AB1
result = previous_checksum
for each byte b at position i in an n-byte chunk:
    result = (result + b * weight) modulo 2^32
    weight = (weight * 0xA7753394 + (n - i - 1) + 0x3BC62BB2) modulo 2^32
```

The weight resets **for each chunk**. The accumulated result is carried
forward across chunks. This exact inner arithmetic is now ported into Rust as
`checksum_chunk`, with independent fixed synthetic test vectors (including
wraparound and a multi-chunk boundary).

**This is not yet the original key-derivation checksum:** calls into this
mixer are conditional on PE section flags and interspersed with relocation
normalization. The original disassembly at `0x401F43` also calls the
SafeDisc 1.50.20 DLL-specific modifier for a DLL of size `0x21E00`.
Until those operations are faithfully reproduced in the correct order,
the full checksum and the four-word TEA key remain unknown.

## Safety and development policy

Only run research on game files you own or are authorized to analyze.
The diagnostic uses *read-only* file I/O, rejects symlinks and unknown
hashes, and has a bounded maximum file size. It neither loads nor
executes `dplayerx.dll`.

**Do not commit or attach** any retail EXE/ICD/DLL, decrypted code,
memory snapshots, or user-private logs to this repository. The separate
SafeDisc experimentation PR #4 and the production Crimson Patcher
remain unchanged by this new branch. Keep this proposed port
**unmerged until explicitly validated**.

License for the newly written Rust implementation:
**GPL-3.0-or-later**, copyright **legluondunet**.
