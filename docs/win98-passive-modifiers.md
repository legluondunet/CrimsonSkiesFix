# Windows 98 passive DLL modifier capture — research milestone

**Scope:** original Crimson Skies 1.0.2.0 / SafeDisc 1.50.20, studied
using an owner-provided exact unSafeDisc 1.5.5 executable and an original
Windows 98 VM. The native tool remains **read-only**, and cannot yet unpack
the protected ICD.

## Distinguishing debug interference from passive observation

- Normal Win98 launch of unSafeDisc: successful user-reported decryption.
- `DEBUG_PROCESS` with **no breakpoints and no target writes**:
  `dplayerx.dll` is loaded at `0x00F90000`, then a first-chance access
  violation is reported at DLL RVA `0x7240` with a failing read of
  `0x16820C55`. This suggests debug-sensitive behavior, but is not proof
  of an intentional anti-debug check.
- Normal process launch plus external `ReadProcessMemory`: process exit 0,
  six PE headers, selected header pointers for `.text` and `.data`,
  and one full `0x29000`-byte mapped DLL image captured without missing pages.
  Exit code 0 does not independently prove the resulting game EXE is valid.

Capture identity (private owner file, **not committed**):

```text
name: CS98-DLL-00.BIN
size: 167936 bytes (0x29000)
sha256: 5931af1ba40b0f28b0bb3e9fabc663127bbd9a8c1de8ea1e9c87b56154c84519
PE preferred DLL base: 0x008E0000
observed Win98 DLL base: 0x00F90000
relocation delta: 0x006B0000
```

The mapped capture is RVA-indexed: bytes at `0x65BE` represent code at
**DLL RVA** `0x65BE`, not the file offset in the original on-disk PE.

## Recovered narrow function contract

Strict inspection of two consecutive 99-byte EAX-only routines from the
captured image, with GNU objdump as a second independent decoder:

| Function | RVA inclusive | EAX operations | Direct CALL / memory ops |
|---|---|---:|---|
| F1 | `0x65BE..0x6620` | 24 | none |
| F2 | `0x6621..0x6683` | 33 | none |

Only NOP, INC/DEC/NEG EAX, ADD/SUB/XOR EAX immediate and ROL/ROR EAX
immediate appear. These instructions do not touch ECX, EDX, EBX, ESI or EDI.
The original unSafeDisc call-site `0x401F62..0x401FA7` conditionally yields:

```text
mask(i) = (i<<24) | (i<<16) | (i<<8) | i         // full-width x86 u32
result = F2(F1(word XOR mask(i)) XOR mask(i))
```

The Rust interpreter at `src/unsafedisc_modifier_snapshot.rs` only decodes
a strict bounded instruction whitelist; it never executes DLL code, and
contains no proprietary captured code bytes. Index rotations and arithmetic
use u32 wrapping semantics, with rotate counts reduced modulo 32.

## Running the new native read-only verification

```bash
cargo test --bin unsafedisc-linux
cargo build --release --bin unsafedisc-linux
./target/release/unsafedisc-linux inspect-snapshot \
  --snapshot /path/to/CS98-DLL-00.BIN
```

The CLI refuses unknown snapshots by **exact SHA-256 and size**, and checks
PE geometry, instruction counts, four known transformation outputs, and
forward/inverse synthetic roundtrips. Keep captured DLL memory files private.
It does not generate a decrypted game file.

The offline exploratory `analyze_modifiers.py` provided separately to the
owner also checks that HIGHLOW relocation entries do not overlap these two
instruction windows. If comparing against an unmodified original DLL on
disk, the comparison must account for raw PE section offsets and relocations.

## Explicit unknowns

- Real-time verification of the exact first and second EAX return values in
  the unmodified Win98 process (no breakpoint trace could be collected).
- Whether both functions are stable over time and across machines.
- Their full call frequency, input values and surrounding stages.
- PE loader behavior and game runtime compatibility of the reconstructed
  candidate, including the legacy name-case/header/filler differences.

## Next deterministic oracle: the original retail/reference pair

The same native CLI already audits the **exact** SHA-256-verified original
`CRIMSON.ICD` and unmodified decrypted `TESTME.EXE`. A four-word TEA
key has been independently corroborated against 28,947 unchanged
fragment tails (58,076 bytes). Under the `inspect` command, the
original reference audit identifies **440,422** extra-layer DWORDs.

With the verified Win98 capture present, the following command now
**checks each of those 440,422 words** against the captured F1/F2 result:

```bash
./target/release/unsafedisc-linux inspect \
  --loader CRIMSON.EXE --icd CRIMSON.ICD \
  --dplayerx dplayerx.dll --reference TESTME.EXE \
  --key-research --modifier-snapshot CS98-DLL-00.BIN
```

This prints only aggregate match counts, never raw retail/reference
pairs. A mismatch refuses the verification instead of pretending the
capture solved full SafeDisc decryption.

A second, independently synthesized **forward** oracle now executes the
bounded **F1/F2 -> TEA** transformation on transient copies of each
non-HIGHLOW .text fragment and compares only **aggregate** byte totals.
It covers the historically overlapping 8-byte TEA blocks, all
page-boundary-crossing relocation gaps, and the fragment-local
index-mask resets.

Expected input geometry for the exact title/version:

| Property | Count |
| --- | ---: |
| Non-HIGHLOW byte positions | 1,855,489 |
| HIGHLOW bytes excluded | 246,972 |
| Fragment-local complete F1/F2 DWORDs | 440,422 |
| Fragments with at least one DWORD | 48,149 |

## Verified owner's exact-game run (2026-10-28)

The owner supplied a new private archive containing exactly the four
original files, all of whose SHA-256 fingerprints were checked against
the pinned expectations. The same captured Win98 DLL image was present.
The exact-head, GitHub Actions-built **Linux CLI** was downloaded and
run locally against these FIVE verified inputs with
`inspect --key-research --modifier-snapshot`, not a synthetic fixture.

The native process exited 0 and independently confirmed:

- **440,422 / 440,422** matching F1/F2 transformed .text DWORDs.
- **1,855,489 / 1,855,489** identical non-HIGHLOW .text bytes after
  forward F1/F2 and the correctly ordered overlapping TEA blocks.
- **168,072 / 168,072** identical non-HIGHLOW .data bytes with the
  same transformation, excluding 990 real HIGHLOW DWORDs.
- **58,076 / 58,076** independent unchanged fragment tail bytes
  from TEA-key validation.

Total full-forward bytes verified: **2,023,561 / 2,023,561**,
no mismatches. The CLI implementation is still read-only; it does not
write game output. No retail input, snapshot or per-word plaintext is
committed to the repository.

A **separate private, experimental Python reconstruction prototype**
was validated against these same SHA-pinned inputs, **without using
the reference as its reconstructed output source**. The prototype:
- Reuses recovered F1/F2, TEA and relocation-aware fragment boundaries
  for .text and .data.
- Restores exactly **245 historically rotated** CALL [IAT] operands,
  using a title-specific verified site list; 228 other CALL [IAT]
  operands are already correct and MUST NOT be changed.
- Restores KERNEL32/USER32 thunk permutations, 136 import-by-name
  strings through a bounded rolling XOR decoder initialized to 0xDF,
  and 21 import descriptor timestamp fields.
- Trims 98 non-section trailing ICD bytes and writes to a **new**
  output path only after exact SHA-256 verification.

The output is a well-formed 2,580,480-byte PE32 with 6 sections,
21 imported modules and 597 valid import entries. SHA-256 of the
reconstruction candidate:
`a39570f6a3d33496b03757fca15d0153e37a91f215e93b6c117eae98d088eabc`.

**2,580,419 / 2,580,480** output bytes equal the unmodified historical
`testme.exe` reference. All **61 residual differing bytes** are
bounded to 54 header/legacy watermark bytes, 2 KERNEL32 name-case
bytes, 3 .text raw-tail filler bytes and 2 .reloc directory-external
filler bytes. Those bytes have **not** been blindly copied from the
reference. Actual game launch and mission compatibility are untested.

The experimental Python prototype lives outside the main Rust CLI
and is not offered as a generally compatible SafeDisc unpacker.
The GitHub CI workflows continue to test only synthetic fixtures.


Do **not** merge this research PR or enable an `unpack` command without
independent full-file validation against the exact original game.


## Final 61-byte exact-title historical output profile (2026-10-28)

**Important distinction:** validated cryptographic decoding is NOT the same
as reverse-engineering every legacy output-formatting side effect. The
owner's private plaintext reference is used exclusively as an oracle, never
copied into the generated candidate. The standalone optional **Python v0.9**
prototype (still outside the Rust CLI) constructs an exact-title output
profile from the already independently reconstructed v0.8 PE and the
SHA-256-pinned original 344,851-byte `CRIMSON.EXE` loader.

The 61 remaining differences were resolved into these exact locations:

| File offsets | Count | Explanation and derivation | Evidence level |
| --- | ---: | --- | --- |
| `0xF6..0xF7` | 2 | Original zero pre-NT gap; reference's 4-byte `e_lfanew-4` value is `0xAD2B0000`, which equals the import-descriptor timestamp restored by the decoder. v0.9 derives its bytes from the reconstructed descriptor, not from plaintext reference. | Exact matching relationship; why the historical program writes here remains unverified |
| `0x100..0x103` | 4 | PE COFF `TimeDateStamp` overwritten with ASCII `1911` instead of original `0x3A15DBDE`. | Legacy formatting **observed** in one historical output; source instructions not yet identified |
| `0x300..0x32F` | 48 | unSafeDisc attribution signature inserted into the PE **header slack**, beyond the section table and before the first section's raw data. | Historical unSafeDisc marker, independently documented in contemporary sources; not code decryption |
| `0x2452E3, 0x2452E6` | 2 | Module name case `KERNEL32.dll` → `KeRNeL32.dll`; no API thunk or address changes. | Exact-title legacy spelling **observed**, origin not yet traced |
| `0x202FFD..0x202FFF` | 3 | Source loader size `344851 = 0x054313`, written as its three big-endian bytes `05 43 13`; located in the .text **raw padding beyond VirtualSize**. | Exact numeric relationship; historical writer mechanism not proven |
| `0x275FFD..0x275FFE` | 2 | Output size `0x276000` represented as `27 60 00` over the final three raw bytes (one was already zero), beyond .reloc VirtualSize. | Exact numeric relationship; historical writer mechanism not proven |

The optional output profile changes NO valid instructions or decryption
results. Its two size relationships use SHA-verified input and generated
output sizes; its pre-NT value comes from the **generated** import table;
the historical timestamp, attribution text, and mixed-case module name
are fixed observed **formatting constants**, not evidence of a recovered
generic authoring algorithm. Do not claim those constants were reverse-
engineered from the original unSafeDisc executable code.

**Exact private local experiment:** no plaintext-reference argument was
supplied to the v0.9 generator. Output length `2,580,480`, SHA-256
`43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`,
byte-for-byte equal to the separately held unmodified `testme.exe`.
A separate default-mode regression reproduced the v0.8 output hash
`a39570f6a3d33496b03757fca15d0153e37a91f215e93b6c117eae98d088eabc`.
No private game inputs or decrypted files were committed. Windows/Wine game
execution and generalization beyond this exact title remain untested.
The Rust CLI remains read-only and PR #5 stays draft.
