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
- The complete use of relocation normalization and section checksum in
  the full ICD transformation, despite a previously validated four-word
  TEA key and its unchanged-fragment tail oracle.

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
capture solved full SafeDisc decryption. The owner must keep all four
canonical originals and the mapped memory capture private.

**Not yet established:** whether the runtime-captured F1/F2 functions
match the retail/reference oracle for the entire original code section.
The GitHub Actions tests run on synthetic fixtures only and cannot make
that title-specific determination without the owner's inputs.

Do **not** merge this research PR or enable an `unpack` command without
independent full-file validation against the exact original game.
