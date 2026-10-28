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
