# SafeDisc DLL state-pointer relocation audit (experimental)

This research feature belongs to the **separate, read-only**
`unsafedisc-linux` inspector on
`feature/unsafedisc-linux-crimson`. It does **not** unpack Crimson Skies
or modify any files.

## Reproduce

Build from this branch and run on the **original, exact-hash-verified**
Crimson Skies 1.0.2.0 / SafeDisc 1.50.20 files:

```sh
cargo test --bin unsafedisc-linux
cargo run --release --bin unsafedisc-linux -- inspect \
  --loader CRIMSON.EXE --icd CRIMSON.ICD \
  --dplayerx dplayerx.dll --modifier-research
```

The `--modifier-research` diagnostic now enumerates DWORD operands
covered by genuine PE32 `IMAGE_REL_BASED_HIGHLOW` relocation entries,
including file-backed non-executable sections. Only operands containing
the DLL's exact preferred virtual address **0x00905010** (state DWORD
RVA **0x25010**) are reported. A DWORD crossing a 4 KiB relocation-page
boundary is accepted if all four bytes are file-backed. Results are
sorted and deduplicated.

The output groups references into four categories:

- **Known read**: one of nine literal absolute-memory operands with
  independently anchored original opcode bytes.
- **Known write**: one of seven anchored absolute-memory store operands,
  including the two explicit resets.
- **Known address-pass**: one of three anchored `PUSH imm32` operands
  passing the state pointer to a helper.
- **Unclassified**: a relocated DWORD equal to the state address that
  has not been matched to the nineteen specific anchors above.
  Up to 24 of these operands are printed for further static analysis.

The category counts are **computed from the current user's verified
DLL**, not assumed. The *nineteen* known sites are expected instruction
operands established in prior original-byte audits, **not** a claimed
total for relocated state references.

## Interpretation

These are **relocation-backed pointers, not a control-flow graph**.
An unclassified operand could be code or data. Conversely, references
computed arithmetically, aliases passed through pointers, registers,
or indirect memory writes are not recoverable from the relocation table
alone. The state at the actual `0x19C78` byte-mixer call has **not**
been determined; initial PE `.data` zero-fill cannot stand in for
runtime state.

The underlying conditional one-byte transformation is separately
checked against original instructions. Neither the whole decoder,
its runtime call/branch order, nor the game-specific protected EAX
modifier routines at **0x65BE** and **0x6621** have been recovered.

## Next research checkpoint

Inspect the **unclassified** relocation operands against validated
original instruction boundaries, and audit possible indirect writes
inside helpers `0x1A77F` and `0x1A9B6`. Only after showing the
actual control-flow path and state initialization can a sequence of
conditional byte operations be compared to reference-backed output.
Never infer this from file offsets or PE zero-fill alone.

All synthetic tests operate on invented section buffers. The actual
original DLL is supplied locally by the user and is **never** included
in a source commit, test fixture, workflow artifact or patch.

## Verified original-image results (2026-10-28)

User-supplied original files were extracted locally and all four SHA-256
fingerprints matched the pinned Crimson Skies 1.0.2.0 profile. An
independent Python PE relocation walk of the genuine **138,752-byte**
`dplayerx.dll` reported **1,694 HIGHLOW relocation sites** and exactly
**19 relocation-backed DWORDs** equal to the preferred address
`0x00905010`. All 19 sites are in `.txt2` and exactly match the
nine known read, seven known write and three known pointer-pass operands.
There are **zero additional unclassified matching relocation sites**.
These are actual private-file inspection results, NOT synthetic fixture
results. No game bytes or original executable content is committed.

### Conditional GetVersionExA failure transition

A further original-byte static audit of the routine `0x1A77F` verified:

1. `0x1A7B0` calls the genuine `KERNEL32!GetVersionExA` import via
   IAT slot `0x1E058`; `TEST EAX,EAX` at `0x1A7B6` leads via
   `JE` at `0x1A7B8` to `0x1A840` if the API fails.
2. On this particular **EAX == 0** path, the flag-preserving
   `MOV ECX,ECX` and NOP instructions leave ZF=1. The `JE`
   at `0x1A846` enters `0x1A849`, whose unconditional jump
   enters the original instruction boundary `0x1A854`.
3. The instructions at `0x1A854..0x1A867` load the DWORD at the
   argument pointer `[EBP+8]`, mask it with `0x000000FF`, **store
   the DWORD back to that pointer** and return with `AX=1`.

Thus the **conditional failure-path state transition** is
`state = state & 0xFF` when the caller passed `0x00905010`. The
old low byte survives. This is **not** a four-byte reset, and it does
not describe the success path or prove the value at `0x19C78` entry.
The separately tested Rust function
`version_query_failure_state` models just this conditional transition.

The exact original opcode groups, conditional branch targets and
preservation of flags were verified without running the DLL on Linux.
The two Windows-dependent success branches and pointer-mediated
modifications elsewhere still require independent analysis.

### Second GetVersionExA helper: a different failure path

The second original call at `0x1AA2F` belongs to helper `0x1A9B6`.
Original `TEST EAX,EAX` at `0x1AA35` branches to `0x1AB34`
when the API returns zero. Starting from the resulting x86 TEST flags
(ZF=1, PF=1, CF=0), the static branch trace is:

```text
1AA37 -> 1AB34 -> 1AB36 -> 1AB38 -> 1AB48 -> 1AB55
       -> 1AB4A -> 1AB53 -> 1AB58 -> 1AB61 -> 1AB5A
       -> 1AB5C -> 1AB5E -> 1AB5F -> 1AB64 -> 1AB68 -> 1AB77
```

This route consists of flag-preserving branches, NOPs and self-MOVs,
then `MOV AX,1` and the function epilogue. It does **not** read or
write the passed state DWORD after the failed API call, unlike the
`0x1A77F` helper's low-byte-masking fallback. A separate bounded
Python static branch check was performed against the SHA-256-verified
retail DLL; the exact opcode and jump assertions also live in Rust
and are exercised by `--modifier-research`.

**Conditional distinction:** for failure of `GetVersionExA`,
`0x1A77F` transforms the passed DWORD to `value & 0xFF`, while
`0x1A9B6` returns `AX=1` without updating it through the passed
pointer. The state may differ before either call, and neither
successful Windows-specific path has been reconstructed. These
findings do not establish a particular Windows platform failure or
fully determine the `0x19C78` entry state.
