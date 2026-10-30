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

### Successful GetVersionExA dispatch in 1A77F (original DLL)

A deliberately **bounded, static x86 branch interpreter**, restricted to
the verified original instruction bytes, followed the
`TEST EAX,EAX` at RVA `0x1A7B6` under the condition that
`GetVersionExA` returned nonzero. The apparently looping
`1A7BE -> 1A7D1 -> 1A7C1 -> 1A7C9 -> 1A7D6` overlapping-instruction
detour is finite with the documented x86 TEST flags (ZF=0, CF=0);
`JBE` is not taken and `JA` is taken. The code then compares the
OSVERSIONINFOA platform field `[EBP-0x84]` with 1 and 2.

Five original-DLL inputs were independently walked:

| Successful API, `dwPlatformId` | Conditional destination |
| --- | --- |
| `0` | `0x1A854` low-byte mask |
| `1` (VER_PLATFORM_WIN32_WINDOWS) | `0x1BEEE` helper |
| `2` (VER_PLATFORM_WIN32_NT) | `0x1AE3F` helper |
| `3` | `0x1A854` low-byte mask |
| `0xFFFFFFFF` | `0x1A854` low-byte mask |

The `0x1A7F3` and `0x1A828` direct calls forward the original
`[EBP+8]` state-pointer argument. At each helper, a separate
original-byte check finds an actual imported
`KERNEL32!GetModuleHandleA` call through **IAT RVA 0x1E098**
(`0x1AEC3` in NT helper `0x1AE3F`;
`0x1BF52` in Windows-9x helper `0x1BEEE`).
This proves a common imported dependency but **not** a common
algorithm or runtime success of that API.

The new `version_success_dispatch` Rust helper is a small classifier
of *dispatch destinations only*: it does not run either helper or
predict the output for Windows-9x/NT, and it does not assert that
`GetVersionExA` can be substituted with an arbitrary platform ID on
real Windows. Static full-branch original-byte and import-name checks
are attached to `inspect --modifier-research`. The two synthetic
unit tests cover routing and prevent treating either helper as a
known low-byte transform.

**Next unresolved point:** inspect the pointer writes and return
conditions in `0x1AE3F` and `0x1BEEE` (or establish that they do not
write) before concluding the successful OS-probe effect on the global
state. The separate `0x1A9B6` platform dispatch is documented below.

### Successful GetVersionExA dispatch in the OTHER helper (1A9B6)

A separate bounded x86 branch interpreter now follows the original
`0x1AA35` nonzero-EAX path (distinct from the `0x1A77F` path).
For **each** of three nonzero API-result examples
(`0x1`, `0x80000000`, `0x11223344`), original PE bytes and x86
flag conditions (ZF/CF/PF/SF/OF) yielded the same conditional
platform-class destinations:

| Successful API, `dwPlatformId` | First helper call or fallback entry |
| --- | --- |
| `1` (Windows 9x) | `0x1AA9E` calls `0x1C7F0` |
| `2` (Windows NT) | `0x1AB0F` calls `0x1BAC7` |
| `0`, `3`, `0xFFFFFFFF` | falls into `0x1AB34` |

The program begins with a deliberately obfuscated chain of
overlapping instruction boundaries at `0x1AA3D..0x1AA75`,
then compares the same **OSVERSIONINFOA.dwPlatformId**
at `[EBP-0x84]` to 1 and 2. The modeled 9x/NT call sites both push
the original `[EBP+8]` argument pointer to their helper. Each
actual E8 call destination and the path's anchoring instruction
bytes are asserted in the separate Rust diagnostic.

**Important distinction:** an unsupported platform enters
`0x1AB34` with different EFLAGS from the API-*failure* path. Instead
of assuming identical behavior, an independently checked bounded
five-flag model explored **all 32 possible combinations** of
ZF/CF/PF/SF/OF in the original `0x1AB34..0x1AB77` branch graph.
Every combination reaches `MOV AX,1` and the epilogue without any
argument-pointer load/store or helper call. The branch therefore
**leaves the passed DWORD unchanged**, whether it was entered after
GetVersionExA failure or from a successful query for another platform.
This conclusion is limited to the verified original control-flow
path, not the behavior of `GetVersionExA` itself. The platform-specific
helper bodies `0x1C7F0` and `0x1BAC7` remain unresolved.

Combined original static dispatch map under successful Windows version
queries:

| Helper calling GetVersionExA | Windows 9x, platform 1 | Windows NT, platform 2 | Other platform |
| --- | --- | --- | --- |
| `0x1A77F` | `0x1BEEE` | `0x1AE3F` | low-byte mask |
| `0x1A9B6` | `0x1C7F0` | `0x1BAC7` | `0x1AB34` returns without changing passed DWORD |

These are **conditional static routing claims only**. They do not
demonstrate real runtime reachability, successful execution of any
platform-specific helpers or identification of the mixer-entry state,
and do not recover the protected EAX modifiers at
`0x65BE` and `0x6621`.

### Complete static analysis of the shared 1AB34 fallback

The original code at `0x1AB34` is a flag-only detour through
Jcc/JMP, NOP and self-MOV/XCHG instructions, followed by
`MOV AX,1` at `0x1AB64` and the epilogue at `0x1AB77`.
A bounded conditional traversal of all **32** combinations of
ZF, CF, PF, SF and OF found **no** pointer dereference, memory
write or helper call, with paths between **15 and 26** visited
instruction positions. Rust independently encodes these transitions
in `secondary_fallback_steps`, checks all 26 relevant original
instruction groups in `audit_secondary_fallback_all_flags`, and
tests all flag combinations without running Windows DLL code.
This establishes a *local branch's* state preservation only;
it does not recover the four platform-specific helper bodies.


### Newly identified USER32 keyboard gate and adjacent INT 68h (2026-10-28)

A **separate, short, file-backed original .txt2 routine** begins at
RVA `0x1D42F` (the last 104 bytes of executable virtual .txt2).
Its complete seven-comparison predicate has been checked against the
exact SHA-256-matched `dplayerx.dll`:

1. `PUSH 0; CALL [0x008FE110]` invokes the genuine PE import
   `USER32.dll!GetKeyboardType(0)`, then compares `EAX` against **7**.
2. Only if equal, `PUSH 1; CALL [0x008FE110]` invokes
   `GetKeyboardType(1)` and compares its value against **0x0D01
   through 0x0D07**, inclusive, one exact equality at a time.
3. It returns `AX=1` precisely when both checks match, otherwise
   `AX=0`. It has no other identified outgoing helper call.

A file-backed original `E8 rel32` at **0x1CD46** targets exactly
`0x1D42F`. The immediately following site at `0x1CD4B` masks the
result with `0xFFFF`, tests it and uses `JNE` to skip past a
literal **`INT 0x68`** when nonzero. The neighboring zero-result path
loads **AX=0x4300** immediately before `INT 0x68` at `0x1CD5B`,
then compares the returned EAX to 0x4300. The enclosing
`0x1C7F0` routine also contains a verified zero initialization of its
**stack-local** DWORD `[EBP-0x98]` at RVA `0x1C7FC`. In the
conditional interrupt path, if returned EAX differs from `0x4300`,
`0x1CD6A..0x1CD74` set that local DWORD to **1**; if EAX is still
`0x4300`, the preceding value is preserved. Neither store targets
the global decoder state `0x00905010`. This is again a conditional
*source-code* rule, not an observation of an interrupt result.

The original bytes at `0x1CD31` and `0x1CD39` provide a
narrower local reachability proof: **JL** to `0x1CD46`, followed
(on its untaken path) only by two NOPs and **JGE** to the same target.
Because `JL` and `JGE` are complementary on unchanged SF/OF,
*every* flag assignment entering **0x1CD31** joins the E8 call at
`0x1CD46`. This does **not** show that `0x1CD31` is reached from
the process entrypoint. Rust checks both original branch encodings,
their targets and all four SF/OF combinations.

This is **static, source-byte evidence only**. It would be misleading
to claim that the `1CD46` callsite or interrupt executes during real
Crimson Skies startup. Overlapping instructions and opaque control
flow prevent establishing runtime execution from adjacency alone.
Later source-level analysis in this document *does* prove a path
from the Windows-9x helper entry to the keyboard call. Historical
SafeDisc research identifies the INT 68h / AX=4300h sequence as
an **anti-SoftICE debugger probe** (see historical-source section).
Its real behavior on present-day Windows/Wine and any effect on
the protected EAX modifiers remain **unverified**.
In particular this does *not* prove that Japanese keyboards are
required, or that an INT 68h instruction explains a game crash.

`src/unsafedisc_dll_keyboard_probe.rs` checks original opcodes,
all seven branch displacements, the rel32 call target, both real
`USER32!GetKeyboardType` imports and the neighboring conditional
branch. It includes pure synthetic tests over the keyboard type and
subtype domain, and runs under `inspect --modifier-research`.
An independent local Python check against the verified original DLL
returned PASS without executing any original code.


### Exhaustive conditional predecessor from 0x1CD16 (2026-10-28)

A new independent **static** source-level walk of the
`0x1CD16..0x1CD46` predecessor was performed on the exact
SHA-256-verified, file-backed original `dplayerx.dll` bytes.
It starts at the **instruction boundary** `0x1CD16`, which is
earlier than the previously established `0x1CD31` local join.

The original, byte-anchored complementary x86 branches are:

| Original branch sites | Flag predicates | Converged target |
| --- | --- | --- |
| `1CD16 JBE` / `1CD18 JA` | `CF || ZF` / `!CF && !ZF` | `1CD29` |
| `1CD29 JS` / `1CD2B JNS` | `SF` / `!SF` | `1CD19` |
| `1CD19 JNP` / `1CD23 JA`, `1CD25 JP` | The PF=1 fallthrough contains only `MOV EDX,EDX` and two NOPs | `1CD2E` |
| `1CD2E JNP` / `1CD30 JP` | `!PF` / `PF` | `1CD41` |
| `1CD41 JO` / `1CD43 JNO` | `OF` / `!OF` | `1CD31` |
| `1CD31 JL` / `1CD39 JGE` | `SF != OF` / `SF == OF` | `1CD46` |

Because intervening instructions on **these proven paths** are only
non-flag-changing self-MOVs and NOPs, every combination of the five
relevant EFLAGS bits (`CF`, `ZF`, `PF`, `SF`, `OF`) is covered.
The independent 32-case check yielded **16 distinct paths of 8–18
visited instruction boundaries**, each arriving at the known
`CALL 0x1D42F` at `0x1CD46`. The Rust function
`trace_gate_predecessor` implements this bounded, fail-closed
**conditional branch model** and regression-tests all 32 combinations;
`audit_gate_predecessor_original` validates the original instruction
bytes and relative branch targets against the SHA-pinned local DLL.

This is a *local reachability proof from 0x1CD16 only*.
It does **not** prove a path from `DllMain` or platform-specific helper
entry `0x1C7F0` to `0x1CD16`. It does not execute x86 or any
Windows API and does not reveal the actual values of the processor
flags or the global state at the game's startup. The wider
`0x1C7F0` body remains partially opaque.


### Conditional direct DWORD mutation inside the Win9x helper (2026-10-28)

A newly audited **separate** block of the original, file-backed
`dplayerx.dll` executable section begins at **RVA `0x1D28F`**,
inside the code span attributed to the successful Windows-9x platform
helper **`0x1C7F0`**, which is the independently verified target of
`0x1AA9E` in `0x1A9B6`.

Two complementary original conditional branches establish a **local**
convergence: `0x1D28F JNE 0x1D2A4`, or on ZF=1 the intervening
flag-preserving `MOV EDX,EDX` and `0x1D297 JE 0x1D2A4`.
The code at `0x1D2A4..0x1D2B3` then does:

```asm
1D2A4  MOV ECX, [EBP+8]
1D2A7  MOV EDX, [ECX]
1D2A9  ADD EDX, 0x34F
1D2AF  MOV EAX, [EBP+8]
1D2B2  MOV [EAX], EDX
```

The original `JNE` at `0x1D2B4` and `JE` at `0x1D2B6`
also converge at `0x1D2C7`; the ordinary MOV instructions between
ADD and these branches do not alter ZF. Thus **if execution enters
0x1D28F** with a valid writable pointer argument, the conditional
local effect on that pointed-to DWORD is exactly
**`value = (value + 0x34F) mod 2^32`** and the store occurs once
before joining the next block.

The recovered rule is deliberately isolated as
`conditional_win9x_34f_step` in
`src/unsafedisc_dll_state_provenance.rs`. Synthetic tests cover
32-bit wraparound and both entry/exit ZF conditions; a dedicated
original-byte audit validates the recorded original instructions and four
original Jcc displacement targets on the exact SHA-256-pinned
private DLL, without executing Windows code.

**Limitations:** no path from the entry of `0x1C7F0` to the
`0x1D28F` block has been verified. The Windows-9x helper may contain
other pointer-mediated mutations, and `GetVersionExA` success does
not by itself prove that this branch executes. The rule does not
identify the input state or the *final* state passed to `0x19C78`,
and neither protected EAX modifier at `0x65BE` / `0x6621` is
recovered. The user-owned original DLL is never committed.


### Stronger Win9x DWORD pointer guard (2026-10-28)

A second exact-original static trace now begins **earlier**, at
`0x1D26A`. Here the code compares the argument **pointer itself**
(`CMP DWORD [EBP+8],0`) against zero:

- **Null argument pointer:** the original `JE 0x1D2FC` skips all
  dereferences and the `0x34F` addition, sets `AX=1`, and jumps
  from `0x1D300` to the verified function epilogue `0x1D428`.
- **Non-null argument pointer:** fallthrough at `0x1D274` enters
  complementary `JNP/JP`, `JO/JNO`, `JL/JA/JGE`, `JL/JGE`,
  `JG/JLE` and finally `JNE/JE` sequences. In the audited
  source-level paths, no intervening instruction modifies the EFLAGS
  used for a branch. All **32** possible combinations of
  CF/ZF/PF/SF/OF therefore reach **`0x1D2A4`**.
  An independent original-image check counted **20 distinct paths**
  of **8–17 instruction boundaries**, starting at `0x1D274`.

The resulting *local* rule is:

```text
IF execution enters original RVA 0x1D26A:
  IF argument_pointer == NULL:
    return with AX=1 (no DWORD dereference)
  ELSE:
    *(u32*)argument_pointer += 0x34F (wrapping 32 bits)
    continue at RVA 0x1D2C7
```

The Rust `win9x_pointer_gate` and
`win9x_nonnull_gate_trace` models, original-byte assertion
`audit_win9x_pointer_guard_original`, and two synthetic regression
tests document precisely this constrained flow. No memory access is
performed by the model.

This considerably strengthens the **local null-vs-nonnull guard**
and state effect. It does **not** prove a path from entry of
`0x1C7F0` to `0x1D26A`, or show whether other branches modify the
pointed-to DWORD before/after it.


### Verified Win9x HELPER ENTRY to keyboard gate (2026-10-28)

**Stronger than the earlier local 1CD16 proof:** the user-provided,
SHA-256-pinned retail `dplayerx.dll` was extracted and inspected
read-only. An independent Python static x86 branch walker begins at
RVA **`0x1C806`**, immediately after the original entry prologue of
the `0x1C7F0` Windows-9x helper:

```asm
1C7F0  PUSH EBP
1C7F1  MOV EBP,ESP
1C7F3  SUB ESP,0x9C
1C7F9  PUSH EBX
1C7FA  PUSH ESI
1C7FB  PUSH EDI
1C7FC  MOV DWORD [EBP-0x98],0
1C806  JL 1C819       ; start of obfuscated flag-only network
```

Although `SUB ESP,0x9C` modifies arithmetic flags, the analysis
requires **no assumption whatsoever** about their values: it
enumerates every possible `CF,ZF,PF,SF,OF` assignment at `0x1C806`.

All **32/32** assignments traverse exclusively safe, recognized
instruction forms before reaching the existing and independently
verified `CALL 0x1D42F` at RVA **`0x1CD46`**:

- `Jcc rel8` and `Jcc rel32` with their *actual original*
  signed displacements and x86 condition-code meanings;
- `JMP rel8/rel32`, ordinary NOPs, and register **self-MOV/
  self-XCHG** (the exact original paths contain no JMP);
- no original memory read, memory write, API call, arithmetic
  flag modification, opaque instruction or cycle was encountered
  in the reachable prefix.

There are exactly **32 distinct static routes**, with **210–395
visited instruction boundaries** each, terminating at the same
`0x1CD46` callsite. The union covers **464 distinct, original
instruction boundaries** across the obfuscated block.

Rust now independently recomputes these results in
`audit_win9x_entry_to_keyboard_gate`, by reading the original
file-backed executable bytes and failing closed on *any* unexpected
opcode, out-of-bounds target, loop, or changed topology. It checks
the actual 1C7F0 prologue and all 32 flag combinations. The pure
`branch_from_eflags` test checks all 16 x86 Jcc conditions in
complementary pairs for all 32 flag assignments. The original-image
audit runs under `inspect --modifier-research`, where the existing
original-file SHA-256 gate must already have passed.

**Conclusion:** *if* a caller enters the **original helper 0x1C7F0**
and its prologue executes normally, the program **must** encounter
the keyboard-gate call at **0x1CD46** before any other observed
side-effecting instruction in this path (the prologue's local
initialization is itself a write). In contrast, **DLL entrypoint
reachability to 0x1C7F0** remains a separate question, as do the
behavior of imported Windows APIs, potential INT 68h semantics,
and a complete path from the gate to the subsequent pointer-state
addition at `0x1D26A`. The protected `0x65BE` and `0x6621`
EAX modifiers are still not recovered, and no `unpack` operation
has been enabled.


### Keyboard-accepted helper path reaches GetVersionExA (2026-10-28)

A further read-only original-image analysis follows the **accepted**
keyboard test (the **conditional** `AX=1` result of `CALL 0x1D42F`),
not the path taking the unresolved `INT 0x68`:

1. `1CD4B AND EAX,FFFFh; 1CD50 TEST EAX,EAX; 1CD52 JNE` enters
   `0x1CD7B`. All 32 five-flag assignments through the subsequent
   *flag-preserving* source-level maze end at **`0x1CE3B`**
   (**32 distinct paths of 30–58 instruction boundaries**).
   The actual flags from `TEST EAX,EAX` with return AX=1 are a
   *single* subset of those 32 checked assignments.
2. Two ordinary, original stack-local stores set
   `[EBP-0x9C]=0x4300` at `0x1CE3B` and
   `[EBP-0x98]=0` at `0x1CE45`. Both preserve EFLAGS.
   From **`0x1CE4F`**, all 32 assignments reach **`0x1CFFF`**
   (**32 distinct paths, 68–125 instruction boundaries**).
3. The original `0x1CFFF` instruction sets local
   `OSVERSIONINFOA.dwOSVersionInfoSize` at
   `[EBP-0x94]=0x94`. From `0x1D009`, all 32 flag
   assignments converge at `0x1D051` (**32 distinct paths,
   12–25 instruction boundaries**).
4. The verified original instructions at `0x1D051`,
   `0x1D057` and `0x1D058` load the address of the
   `OSVERSIONINFOA` stack local, push it and invoke the genuine
   **`KERNEL32.dll!GetVersionExA`**, imported through IAT
   RVA **`0x1E058`** (`FF 15 58 E0 8F 00`).
   The subsequent `TEST EAX,EAX; JE 0x1D19D` distinguishes
   API failure from success. No API return value is guessed.

All six stage-boundary instructions are anchored by exact original
bytes. `trace_original_flag_segment` and
`audit_win9x_gate_success_to_version_api` are fail-closed
Rust source-level readers in the same inspector module. They reject
unexpected opcodes, loops, out-of-range jumps and mismatched path
statistics; this audit runs **only after** retail-file SHA-256 and
PE geometry verification. No Windows function is executed.

**Consequence:** for an entry into helper `0x1C7F0` and a
successful `GetKeyboardType` predicate, the code **must reach its
own GetVersionExA call** without passing through the adjacent
`INT 0x68`. This *conditional* chain is distinct from the earlier
`GetVersionExA` call in `0x1A9B6` which selected the platform
helper in the first place. Effects of the nested version query
and the resulting path to `0x1D26A` still require research.
In particular, **no unconditional `+0x34F` state effect is
asserted** for this helper.


### Conditional GetVersionExA -> pointer increment now linked (2026-10-28)

The nested version-query branch inside helper `0x1C7F0` was
followed farther. This is the first *bounded, source-backed local
chain* from the helper's entry and an accepted keyboard check
to the already proven `+0x34F` pointer-store block, under precisely
identified **additional API conditions**:

1. The helper-entry-to-keyboard gate and the gate-accepted-to-
   `GetVersionExA` paths described above are taken.
2. The nested **`KERNEL32!GetVersionExA` at `0x1D058`**
   **succeeds** (`EAX!=0`). Its failure `JE` at `0x1D060`
   enters `0x1D19D` directly, bypassing the forthcoming local
   `0x0F` assignment.
3. On success, an additional flag-preserving maze from
   `0x1D066` reaches `0x1D0C6` where the **actual original
   instruction compares `OSVERSIONINFOA.dwPlatformId` at
   `[EBP-0x84]` with `1`**. A matching platform `1` uses
   `JE 0x1D19D`; any **other** reported platform continues.
4. The non-`1` path from `0x1D0D3` converges on an actual,
   unambiguous `MOV DWORD [EBP-0x98],0x0F` at `0x1D133`.
   This is the **stack-local** flag, NOT the decoder state.
   The next control-flow network reaches `0x1D19D`, which
   compares the same local flag with zero. With the known
   value **15**, its `JBE 0x1D305` is **not taken**;
   the fallthrough starts at `0x1D1AA`.
5. The final fully explored flag-only network joins the existing
   **pointer-null check at `0x1D26A`**. If the actual
   **argument pointer is nonnull**, the already verified
   `0x1D2A4..0x1D2B3` instructions then perform
   `*pointer = (*pointer + 0x34F) mod 2^32`.
   If the pointer is NULL the original code returns without
   dereferencing it.

The 32-flag independent original-image traversal found the following
unconditional *local* convergence from each named start:

| Conditional flag-only segment | Distinct paths | Instruction boundaries |
| --- | ---: | ---: |
| `1D066 -> 1D0C6` | 16 | 15–33 |
| `1D0D3 -> 1D133` | 32 | 16–28 |
| `1D13D -> 1D19D` | 16 | 15–27 |
| `1D1AA -> 1D26A` | 32 | 31–60 |

`audit_win9x_nested_version_to_pointer_guard` verifies these
segments with original executable bytes, the pointer and local
flag CMP/MOV instructions, and the four outer Jcc displacements;
`accepted_keyboard_nested_version_to_guard` is a **pure
synthetic branch model** with regression tests.

**Precisely scoped result:** the local `+0x34F` operation is
reached when all of these hold:
`helper 0x1C7F0 entered` AND
`keyboard predicate accepted` AND
`nested GetVersionExA returned success` AND
`nested dwPlatformId != 1` AND
`passed DWORD pointer != NULL`.

There are **other, unresolved paths**, including the
keyboard-rejected `INT 0x68` route; the conditional proof
must not be misread as an exclusive or universal rule.
In particular, the **outer** `GetVersionExA` in `0x1A9B6`
chooses helper `0x1C7F0` on a reported platform `1`.
The inner and outer API results would ordinarily be expected
to agree on a stable operating system; this analysis does not
assert that the inner report is different, or that this
conditional addition actually occurs during normal gameplay.

The DLL entrypoint path to `0x1A9B6`, exact 32-bit state at
the byte mixer `0x19C78`, and protected EAX modifier
routines `0x65BE`/`0x6621` are **not** recovered.


### Keyboard-rejected INT 68h path: conditional state-gate truth table (2026-10-28)

The original **keyboard-rejected** path has now been composed with
the previously audited nested version query and pointer-null guard.
This adds a second conditional route to the isolated `+0x34F`
DWORD mutation; the interrupt's behavior is **not** assumed or
emulated.

- At `0x1C7FC`, the helper initializes the **stack-local**
  `[EBP-0x98]` to zero.
- If `GetKeyboardType` predicate fails, `0x1CD52 JNE`
  is not taken, so the file-backed original `PUSHA`, `XOR EAX,EAX`,
  `MOV AX,0x4300`, `INT 0x68` at `0x1CD5B` are reached.
- **Assuming only that INT 68h returns normally** and the original
  stack/frame remains usable, `0x1CD5D` saves the resulting full
  EAX to the local `[EBP-0x9C]`. The original
  `CMP EAX,0x00004300` at `0x1CD63` / `JE` at `0x1CD68`
  preserve `[EBP-0x98]=0` if **full EAX equals 0x4300**,
  or set `[EBP-0x98]=1` at `0x1CD6A..0x1CD74` otherwise.
  The comparison is **32-bit**, not merely AX (an EAX such as
  `0xFFFF4300` is treated as different).
- `POPA; JMP 0x1CE4F` at `0x1CD75..0x1CD7A` bypasses the
  keyboard-accepted-only writes at `0x1CE3B` and `0x1CE45`,
  **so the one/zero interrupt flag is not overwritten there**.
  The already audited `1CE4F -> 1CFFF -> 1D051` flag-only networks
  reach the nested `KERNEL32!GetVersionExA` at `0x1D058`.
- If this nested API succeeds with `dwPlatformId != 1`,
  the original `1D133` stack-local store **overwrites**
  `[EBP-0x98]` with **15**. Otherwise (failure or platform 1),
  the interrupt-derived 0 or 1 remains unchanged.
- Original `CMP [EBP-0x98],0; JBE 1D305` at `0x1D19D`
  bypasses the pointer test for zero, and routes nonzero values
  through the already verified flag-only path to `0x1D26A`.
  Only a **nonnull, usable pointer** can then produce
  `*ptr = (*ptr + 0x34F) mod 2^32`.

The complete conditional local matrix (assuming normal INT return
on the rejected route and a valid original stack frame) is:

| Keyboard check | Returned EAX from INT 68h | Nested GetVersionExA | Local flag at 1D19D | Pointer guard |
| --- | --- | --- | ---: | --- |
| Accepted | Not invoked | Failed or platform 1 | 0 | Bypassed |
| Accepted | Not invoked | Success, platform != 1 | 15 | Reached |
| Rejected | Exactly 0x4300 | Failed or platform 1 | 0 | Bypassed |
| Rejected | Different from 0x4300 | Failed or platform 1 | 1 | Reached |
| Rejected | Any returned EAX | Success, platform != 1 | 15 | Reached |

This is implemented as an explicit **conditional-source model**
`conditional_nested_pointer_route` in
`src/unsafedisc_dll_keyboard_probe.rs`, with distinct
`KeyboardResult` and `NestedVersionResult` enums. It deliberately
does **not** define a `RejectedInterruptNeverReturned` path because
that has no reachable post-interrupt program counter: no assertion
about success, failure, trapping, exception handling, drivers or
Wine behavior is made. Four synthetic regression tests cover the
return-value/OS-version combinations; original instruction checks
for `PUSHA/POPA`, local writes, 32-bit comparison, and all local
branch targets are SHA-pinned.

**Important:** the separate outer `GetVersionExA` in helper
`0x1A9B6` selects `0x1C7F0` only for a Windows-9x platform report.
The new conditional route shows that a matching **inner** platform
1 is *not itself sufficient* to rule out the isolated `+0x34F`
addition: it may also be reached via the keyboard-rejected
INT-return comparison. **Whether that interruption ever returns
normally on a target system is not established.** Actual entry
state at `0x19C78` and the two protected EAX modifiers remain
unknown. This does not enable unpacking or run any protected bytes.


### Both conditional Win9x helper returns now separated (2026-10-28)

Two **additional** original-image 32-flag static walks through the
late portion of helper `0x1C7F0` distinguish the *low 16-bit return
value* from whether the passed state DWORD was actually updated.

**If `[EBP-0x98] == 0` at `0x1D19D`:**
the original `JBE 0x1D305` bypasses the pointer check and takes an
obfuscated EFLAGS-only path to **`0x1D425`**, which contains
`66 33 C0` (**`XOR AX,AX`**) and then the verified epilogue
`0x1D428`. For **all 32 flag combinations**, this route
converges with **32 distinct paths of 44–77 instruction boundaries**.
It sets **AX=0**, and does not access the argument DWORD in this
audited suffix.

**If `[EBP-0x98] > 0` at `0x1D19D`:**
the original flag-only network reaches the pointer-null guard
`0x1D26A`. A NULL pointer branches to **`0x1D2FC`**, which
contains `66 B8 01 00` (**`MOV AX,1`**) and jumps directly
to the same `0x1D428` epilogue: **AX=1, but no DWORD write**.

A usable nonnull pointer instead reaches the original single
`+0x34F` DWORD store at `0x1D2B2` and the complementary
`JNE/JE` pair at `0x1D2B4/0x1D2B6`. From `0x1D2C7`
all 32 flag combinations reach `0x1D2FC` with **16 distinct
routes of 10–21 instruction boundaries**. Some valid original
branches temporarily jump backward as far as **`0x1D2B7`**
before converging; the bounded Rust interpreter permits only this
explicitly audited back-edge range while still refusing loops,
memory operations and unexpected instructions. Result:
**AX=1 and the passed DWORD was incremented exactly once**.

A combined pure conditional Rust model
`conditional_win9x_outcome` distinguishes these three cases:

| Local flag | Argument pointer | Original return AX | DWORD write |
| --- | --- | ---: | --- |
| Zero | Null or nonnull | **0** | No |
| Nonzero | Null | **1** | No |
| Nonzero | Valid writable DWORD | **1** | **Add 0x34F exactly once** |

Because the source instructions write only **AX**, no claim is
made about the original *high 16 bits of EAX*. The model also
requires explicitly supplied observed Windows API outcomes and,
if the keyboard gate rejected, an **assumed normally returning
INT 68h**. These are not measured; neither actual pointer validity,
full DLL startup reachability, global state at mixer entry, nor
decryption is claimed.

Original-byte checks now cover the `MOV AX,1`, `XOR AX,AX`,
shared function epilogue, both flag-only suffixes, and the earlier
interrupt / platform conditions. A standalone offline verifier,
`verify_rejected_keyboard_int68.py` (distributed separately,
**not** part of the proprietary DLL), compares 15 original opcode
groups and both 32-flag suffix topologies against a user-supplied
SHA-256-pinned retail `dplayerx.dll`. No protected code is run.

### Historical identification: anti-SoftICE INT 68h (2026-10-28)

The purpose of this old Windows-9x sequence is no longer a mere
unexplained interrupt. Two independently published historical
reverse-engineering descriptions agree that SafeDisc used the
`AH=0x43; INT 0x68` pattern for **SoftICE debugger detection**:

- The publisher's [No Starch Press excerpt of *Crackproof Your Software*,
  chapter 7, p. 97](https://nostarch.com/download/crackproof_excerpts.pdf)
  explicitly identifies INT 68h as a Windows-9x SoftICE probe and
  describes a return in AX of `0xF386` when the debugger is present.
- [Black Check's 1999 SafeDisc analysis of *Midtown Madness*](https://www.gbppr.net/fravia/blackche.htm)
  reproduces a remarkably close instruction sequence:
  zero EAX, load AX=`0x4300`, invoke INT 68h, save EAX at
  `[EBP-0x9C]`, and compare **EAX** with `0x4300`. That author
  reports an expected normal/no-debugger EAX of `0x4300`, and a
  differing EAX with SoftICE or particular debugging countermeasures.

The independently hash-verified *Crimson Skies* original `dplayerx.dll`
contains **the same specific opcode pattern** around RVA `0x1CD55`
through `0x1CD68`, including the **full 32-bit EAX comparison**.
This strongly supports identifying the source intent as an
**anti-debugging/SoftICE check inherited from historical SafeDisc**.
The check's result is combined with a local pointer-state gate as
proven above. This contextual interpretation **does not mean** the
interrupt is safely callable or returns the documented value under
modern Windows, Wine, NTVDM or non-9x execution; no such
runtime experiment has been performed. It also does not by itself
show that an inconsistent decoder key or a Crimson Skies startup
crash actually results.


### Conditional anti-SoftICE perturbation traced to actual global state (2026-10-28)

The original Win9x helper's `+0x34F` write is now connected to the **actual**
global decoder state by an original-byte-backed, conditional pointer chain:

1. RVA **`0x1987D`** executes `PUSH 0x00905010`, the preferred virtual
   address of the previously verified global decoder-state DWORD.
   Its `CALL` at **`0x19882`** targets helper **`0x1A9B6`**.
2. If the outer `GetVersionExA` succeeds and reports platform **1**,
   the independently audited branch of `0x1A9B6` forwards its `[EBP+8]`
   argument unchanged with `MOV ECX,[EBP+8]; PUSH ECX` at **`0x1AA9A`**,
   followed by `CALL 0x1C7F0` at **`0x1AA9E`**.
3. If that Win9x helper's nested conditional paths reach the pointer
   guard **`0x1D26A`**, a nonnull, usable pointer causes the original
   `MOV ECX,[EBP+8]; MOV EDX,[ECX]; ADD EDX,0x34F;
   MOV EAX,[EBP+8]; MOV [EAX],EDX` block at **`0x1D2A4`** to update
   **the same global DWORD** by `+0x34F`, modulo 2^32.

Most significantly, with **both outer and inner OS-version reports
equal to platform 1**, the source path can still change the global state
**if** the keyboard predicate rejects and **if** the original `INT 68h`
returns normally with **full EAX different from `0x4300`**. If it
instead returns `0x4300`, this branch bypasses the write and returns
with AX=0. The former route sets AX=1 and adds `0x34F` to the state.

This matches a documented SafeDisc **anti-SoftICE debugger** pattern
(see historical references above), but must **not** be presented
as an observed result on current Windows or Wine. Reaching the
original `0x1987D` callsite from DLL startup is not established.
The global mixer-entry state at `0x19C78` remains undetermined.

`audit_original_global_state_call_chain` validates the original pointer
PUSH, two actual rel32 CALL targets, argument forwarding and final
original ADD/store. `conditional_original_state_route` is a pure
model requiring explicit hypothetical outer/inner OS observations;
it never runs the DLL. The independent offline Python verifier also
checks these exact opcode groups and their original targets.


### Earlier branch into mixer preparation: opaque A310 low-16 result

The original worker loop's non-exit path at **`0x19692`** prepares
arguments for a **direct `CALL 0xA310` at `0x196AE`**. The helper's
implementation is **not** recovered as part of this research.
If it returns normally, the worker performs
`ADD ESP,0x14; AND EAX,0xFFFF; TEST EAX,EAX; JE 0x19819`
at **`0x196B3..0x196C2`**. Thus precisely the *low 16 bits* of
the helper's returned EAX decide whether the worker skips
its current mixer path (zero) or continues (nonzero). A high-only
result such as `0x00010000` still counts as **zero** here.

On the nonzero result, three independently audited control-flow
networks lead through original stack-local computations and
ultimately into the later mixer setup:

| Original flag-only segment | Distinct paths for 32 EFLAGS | Length |
| --- | ---: | ---: |
| `0x196C3 -> 0x196DB` | 8 | 4–7 |
| `0x196E4 -> 0x196F8` | 3 | 7–11 |
| `0x19704 -> 0x19728` | 4 | 10–18 |

All 32 assignments converge at each target, regardless of
flags left by intervening CMP/SUB/ADD instructions. The data
preparation thereafter reaches the **`CALL 0x19965` at `0x1974A`**;
the next section describes its conditional continuation to the
**`CALL 0x19C78` at `0x19798`**.

The original SHA-pinned bytes and exact relative call/branch
targets are checked by `audit_original_worker_mixer_predecessor`.
The pure `worker_setup_skips_mixer` model is regression-tested
against low-16-bit boundary values, and an independently edited
offline Python verifier confirms the three 32-flag graphs.

**Limitations:** there is no proof that `0xA310` returns nonzero
on a real game invocation, or even that the DLL initializer
executes this worker iteration. The zero-result branch
also enters `0x19819`, whose subsequent finalizer behavior
is conditional. The call to the original mixer is never
executed by this research tooling.

### Verified mixer setup: second no-op RET trampoline

Immediately before the `0x19798 -> 0x19C78` mixer call, another
original helper is called at **`0x1974A -> 0x19965`**.
If it returns normally, the caller adjusts its stack by 20 bytes
at `0x1974F` (`ADD ESP,0x14`, modifying flags). All **32**
possible resulting combinations of `CF/ZF/PF/SF/OF` enter a short
flag-only network from **`0x19752`** that converges at **`0x1976F`**.
The independently checked original-image topology has **six
distinct paths of 7–10 instruction boundaries**.

Eight of the 32 flag assignments (precisely `CF=0,ZF=0`) take
`0x19764 PUSH EDX; 0x19765 CALL 0x197F8`, where the original
instruction at **`0x197F8`** is a single-byte **`RET`**.
The matching `0x1976A POP EDX; 0x1976B JA 0x1976F`
is then guaranteed to take `JA`, because this detour is selected
only when `CF=0,ZF=0`; the apparent early `RET` at `0x1976D`
cannot be reached through this detour.

On the other 24 flag configurations, the original branch maze
bypasses the no-op call and reaches `0x1976F` directly.
The original data preparation at `0x1976F..0x19797` then constructs
the arguments for the byte mixer call at `0x19798`.

`audit_original_mixer_setup_detour` and its exhaustive flag walk
were integrated into the existing SHA-gated `unsafedisc-linux`
research inspector. `verify_worker_finalizer.py` reproduces the
actual original-byte checks and the 32-case traversal without
running any DLL code.

**Limitations:** the conditional source-level chain begins on
return from original helper `0x19965`, whose own behavior and
reachability are not established by this check. The no-op
call is verified as a bare-RET trampoline, not a legitimate
separate function that is known to execute in the running game.

### New bridge: original byte mixer directly precedes worker counter update

The SHA-256-pinned original file contains an unambiguous **`E8 rel32`
CALL at RVA `0x19798` with target `0x19C78`**, the independently
audited byte-mixer routine. If it **returns normally**, the original
`ADD ESP,0x18` at `0x1979D` adjusts the call arguments, then the
obfuscated flag-only network beginning at `0x197A0` leads to the
already verified `0x197D4` cursor update.

All **32** possible assignments to `CF/ZF/PF/SF/OF` immediately after
the `ADD ESP,0x18` are independently covered. The exact original
executable-byte walk yields **24 distinct paths of 12–23 instruction
boundaries**, all converging at **`0x197D4`**; the union comprises
24 original source instruction boundaries. Every reached original
instruction is a Jcc, JMP, NOP or register self-MOV/XCHG;
none modifies the flags, accesses external state or calls an API.
The use of all 32 assignments avoids guessing the flags produced
by the preceding `ADD ESP,0x18`.

`audit_original_mixer_to_counter_bridge` checks the genuine original
rel32 mixer-call target, the stack adjustment, five additional opcode
windows and all 32 real-code static branch traces. The independent
Python standard-library `verify_worker_finalizer.py` reproduces
the original-image check without invoking the DLL.

**Limitations:** this is conditional on entering the mixer CALL at
`0x19798` and its return. No full executable traversal from the
DLL entrypoint or shared worker entry `0x18F81` to `0x19798`
is yet established; no mixer output byte or decoder-state value
is inferred. The mixer may modify the global state in other ways
as separately documented, and the number of loop iterations
remains unknown. This path alone does not establish the total
state delta at the next mixer invocation.

### Stronger worker-exit precondition: unsigned sum and no-op RET trampoline

A newly audited **earlier** source block at `0x197D4..0x197DD` computes:

```asm
197D4  MOV EAX,[EBP-0x0C]
197D7  ADD EAX,[EBP-0x08]
197DA  MOV [EBP-0x0C],EAX
```

Across **all 32** CF/ZF/PF/SF/OF combinations after this addition,
the original obfuscated control flow between `0x197DD` and `0x197FA`
converges: **8 distinct paths, 5–13 instruction boundaries**.
Sixteen flag configurations go directly to `0x197FA`; the other sixteen
pass through `0x197EF PUSH EDX; 0x197F0 CALL 0x1965A`, where
`0x1965A` is exactly an original **RET** byte, then
`0x197F5 POP EDX; 0x197F6 JGE 0x197FA`. This detour
is selected only when **SF==OF**, so its JGE is always taken.
The apparent early `RET` at `0x197F8` is unreachable along these
original-byte-backed paths.

The continuation copies `[EBP-0x04]` to `[EBP-0x14]` at
`0x197FA..0x197FF`; the separately audited backward flag-only maze
then reaches `0x19686` and its unsigned `JAE` to `0x19819`.
The exact local counter/limit rule is therefore:

```text
updated_cursor = (old_cursor + increment) mod 2^32
finalizer = (updated_cursor >= limit)  // UNSIGNED comparison
```

The original-byte auditor `audit_original_worker_counter_exit`, bounded
`trace_original_pre_exit` and pure `unsigned_worker_exit` Rust model
verify this. An independent SHA-pinned Python reader of the original
DLL checked **17 opcode windows**, the RET trampoline, and all
32 flag combinations. No protected DLL code was executed.

This is NOT proof that the real DLL initializer calls into the
`0x197D4` block, nor that the unsigned counter condition is met.
The pure `conditional_finalizer_at_worker_exit` model additionally
requires this **current-iteration** unsigned exit test before
composing the Win9x helper and the caller's direct state add.
Its `None` result means that this particular loop-exit edge was
not taken (or the modeled outer Win9x branch was not selected):
it does **not** mean the worker cannot reach it on a later iteration.
On a failed JAE the worker continues along unmodeled code. The
combined state delta below remains **conditional** on the original
worker reaching this loop-exit branch.

### Worker finalizer and conditional combined state delta (2026-10-28)

An independent, **exact SHA-256 pinned**, read-only inspection of the
original `dplayerx.dll` now extends the original state-pointer chain
backward through an unsigned counter comparison and forward through
the caller's **direct state update** after helper return.

1. The original flag-preserving maze starting at RVA **`0x19800`**
   always returns to **`0x19686`**, for all 32 possible assignments
   to `CF/ZF/PF/SF/OF`: **5 distinct paths, 8–12 instruction
   boundaries**. `0x19686 MOV EDX,[EBP-0x0C]` followed by
   `0x19689 CMP EDX,[EBP-0x14]` and `0x1968C JAE 0x19819`
   is a **runtime conditional, unsigned local counter comparison**.
   It is NOT asserted always to succeed; on the opposite branch
   the worker continues processing unmodeled code.
2. **Given that JAE is taken**, all 32 EFLAGS assignments from
   `0x19819` converge at the original `PUSH 0x00905010`
   at **`0x1987D`**: **16 distinct paths, 31–41 instruction
   boundaries**. Original `CALL 0x1A9B6` at **`0x19882`**
   then receives the actual global decoder-state address.
3. On the successful outer Win9x dispatch, `0x1A9B6` forwards
   the same pointer to `0x1C7F0`. The original wrapper also
   preserves the helper's AX result: `MOV WORD [EBP-0x98],AX`
   at **`0x1AAA6`**, then `MOV AX,[EBP-0x98]` at
   **`0x1AABA`** and `JMP 0x1AB77` to its epilogue.
   Its obfuscated detour via `CALL 0x1AB75` targets a bare
   `RET`; with the previous `JNE` not taken, the following
   `JE` must jump to the AX reload. This avoids assuming
   that the helper's AX result survives an unknown wrapper.
4. The original caller immediately executes the contiguous,
   byte-verified instruction sequence **`0x19887..0x198A5`**:

```asm
ADD ESP,4
AND EAX,0xFFFF
NEG EAX
SBB EAX,EAX
AND EAX,0x4F3
MOV EDX,[0x00905010]
ADD EDX,EAX
MOV [0x00905010],EDX
```

   This produces exactly **`+0` when AX==0** and
   **`+0x4F3` when AX!=0**, with 32-bit wrapping.
   The upper 16 bits of the actual helper EAX do not matter,
   because the caller first masks them out.

**Consequence in the narrow, conditional, already audited Win9x path:**
if both outer and nested OS reports are platform `1`, the keyboard
predicate rejects and the historical `INT 68h` **returns normally
with full EAX different from `0x4300`**, the helper modifies the
actual state by `+0x34F`, then returns AX=1. The original caller
subsequently adds `+0x4F3`. Hence **the net change across
this modeled helper/caller pair is `+0x842` modulo `2^32`**.
Conversely, under the same hypothetical conditions with an
INT return equal to `0x4300`, the helper returns AX=0
without its write, and the caller adds zero.

| Hypothetical helper conditions (outer Win9x) | Helper writes | AX | Caller delta | Net delta |
| --- | ---: | ---: | ---: | ---: |
| Keyboard rejected, normal INT return EAX == 0x4300, nested platform 1 | No | 0 | 0 | 0 |
| Keyboard rejected, normal INT return EAX != 0x4300, nested platform 1 | +0x34F | 1 | +0x4F3 | **+0x842** |
| Keyboard accepted, nested platform 1 | No | 0 | 0 | 0 |
| Keyboard accepted, nested platform != 1 | +0x34F | 1 | +0x4F3 | **+0x842** |

The complete provenance and both original flag-only regions
are checked by `src/unsafedisc_dll_worker_finalizer.rs` under the
SHA-gated `inspect --modifier-research` option. **Nine new synthetic
Rust unit tests** check upper AX masking, state wrapping, matching
versus differing interrupt outputs and exclusion of non-Win9x
outer platforms. A self-contained external **Python standard-library
offline verifier** (`verify_worker_finalizer.py`) independently
confirmed all 17 original opcode windows, four rel32 call targets,
branch destinations, all three 32-flag convergence counts and
the arithmetic on the supplied retail DLL; a one-byte tamper
test correctly failed the pinned SHA-256 gate.

**Still not proven:** a DllMain-to-`0x19800` route; whether the
unsigned counter JAE is taken during startup; actual runtime
`INT 68h` behavior; the full set of global-state mutations
before the `0x19C78` byte mixer; and the protected EAX
modifiers at `0x65BE` / `0x6621`. This is not an unpacker,
an assertion of corrupt state under Wine, or a reason to
patch out the anti-debug code before obtaining evidence.


### Newly audited first call of shared worker and 11-way dispatcher (2026-10-28)

The original DLL has two previously identified, literal direct calls
to the shared worker RVA **`0x18F81`**, at `0x1A1B2` and `0x1A39C`.
Rather than assuming either caller runs from DllMain, this inspection
begins *at the worker entry itself* and establishes its first call.

The exact original worker prologue at `0x18F81..0x18F90` saves
EBP, allocates 28 bytes of locals, saves EBX/ESI/EDI, and initializes
`[EBP-0x10]` to zero. The obfuscated network from `0x18F91`
converges on argument preparation at **`0x18FA9`** for all 32
possible CF/ZF/PF/SF/OF combinations: **4 distinct paths of
5–8 original instruction boundaries**. This network contains
only original Jcc instructions, self-MOV and NOPs, so it
does not change the branch flags or perform another call.

The original worker next prepares six arguments and invokes
**`CALL 0x18800` at `0x18FBF`**. The verified `0x18800`
wrapper reorganizes arguments and calls **`0x181E0` at
`0x18823`**, then removes seven arguments and returns. The
latter dispatcher allocates its stack frame and first calls
the independent original routine **`0x18940` at `0x181FF`**;
its return and effects remain unmodeled.

After that call, the original `0x181E0` code copies and masks
two local 16-bit values, constructs an address, and initializes
three DWORD output pointers to zero. Its own original switch
uses `MOV ECX,[ESP+0x178]` at **`0x18240`**, then
`DEC ECX` at `0x18255`, `CMP ECX,10` at `0x18263`;
`JA 0x184EB` at `0x1826C` and otherwise
`JMP DWORD [ECX*4+0x008F8680]` at `0x18272`.

The address `0x008F8680` is preferred DLL base `0x008E0000`
plus table RVA **`0x18680`**. Its **11 consecutive DWORD
entries all have actual PE `HIGHLOW` relocations**, and their
original preferred-base VA values resolve to these exact RVAs:

| Input selector (1-based) | Case RVA |
| ---: | --- |
| 1 | `0x183C2` |
| 2 | `0x1850D` |
| 3 | `0x1857F` |
| 4 | `0x184EB` |
| 5 | `0x18304` |
| 6 | `0x18357` |
| 7 | `0x185D5` |
| 8 | `0x18279` |
| 9 | `0x18486` |
| 10 | `0x1863E` |
| 11 | `0x1841D` |


#### Exact selector of the FIRST worker invocation: case 1, not arbitrary

The *first* call made by the original `0x18F81` worker has now
been traced all the way through the argument-shuffling wrapper.
At **`0x18FB9`** the worker pushes the **literal argument `1`**
as the second argument to `0x18800`; the other five values are
a pointer, a zero local DWORD and three output pointers.

The byte-anchored `0x18800` wrapper forwards that argument
**unchanged** as the second of seven arguments to `0x181E0`.
The dispatcher allocates `0x160` bytes, saves four registers
(16 bytes), calls the independent `0x18940` helper with three
temporary stack arguments, and restores the temporary stack
with `ADD ESP,0x0C` at **`0x1821F`**. Therefore the
`MOV ECX,[ESP+0x178]` at `0x18240` reads the original
**second** argument: `0x160 + 0x10 + 8 = 0x178`.

The original `DEC ECX` then changes that `1` to index `0`.
Consequently, **if `0x18940` returns normally and the
original stack layout remains valid, this initial dispatch
selects the table's case 1 at RVA `0x183C2`**, whose
original first direct call (after preparing its arguments)
is **`0x183D4 -> 0x189F0`**. Both the original call and
its target are SHA-pinned in
`audit_initial_worker_fixed_selector`.

This resolves the **first invocation selector**, not all later
selector values or the opaque behavior of `0x18940` and
`0x189F0`. The actual return value to the worker remains
unknown; the conditional bypass versus `0x18B00` path
documented below is therefore unaffected.

**Out-of-range input** (zero or 12 and above) reaches the
original `JA` fallback RVA **`0x184EB`**. The legitimate
input `4` also selects `0x184EB`, which is not an error.
This is an **11-case switch**. The first worker invocation's
selector is now statically proven to be `1`, assuming normal
helper returns; subsequent invocations and selector values
remain unknown.

The Rust `src/unsafedisc_dll_worker_entry.rs` validates the original
entry prologue, conditional branch network, five direct CALL
targets, the switch instructions, every relocation-backed
DWORD and the exact 11 targets; it includes three synthetic
selector tests. A separate stdlib-only Python verifier
`verify_worker_entry_dispatch.py` was run against the SHA-256-pinned
retail DLL and passed every check. Both implementations are
read-only and never execute the original DLL.

**Still unresolved:** entrypoint/DllMain reachability of the
two identified worker callsites; return values of the original
subroutine `0x18940`; later dispatch selector values or actual case results;
exact state changes while the dispatcher runs;
the later opaque `0xA310`/`0x19965` helpers, and the final
global state at the byte mixer `0x19C78`. This analysis
does not enable SafeDisc unpacking or decrypt protected bytes.


#### First worker wrapper return: full DWORD zero test

When the first `0x18800` wrapper returns normally to `0x18FC4`,
the worker adjusts its stack, saves returned EAX at `[EBP-0x10]`,
and performs the **full 32-bit** `CMP DWORD [EBP-0x10],0`
at `0x18FCA`. The original `JE` at **`0x18FCE`** takes
the **zero result** directly to RVA **`0x19022`**,
where the local DWORD is reset to zero. This differs from
the **LOW-16-bit** test after the unrelated later `0xA310` call.

**For any nonzero 32-bit return value**, the original
`JL/JA/JMP/JO/JNO` maze at `0x18FD0..0x18FF0`
converges on the next argument setup. For values with the high
sign bit clear, the path has **12 instruction boundaries**;
with it set, the path has **15**. The original CMP sets CF=OF=0,
ZF=0 and SF to the high sign bit. The intervening original
instructions on the audited paths do not alter these flags.
The next direct call at **`0x18FFC`** targets **`0x18B00`**.

`audit_first_dispatch_return_branch` verifies the exact original
comparison, opcode groups, both result routes and the real
`0x18B00` call target. Its pure model
`first_dispatch_return_route` distinguishes the full-DWORD
zero bypass from nonzero continuation. An independently
executed standard-library offline verifier
`verify_worker_entry_dispatch.py` checks the same original
byte sites and rejects any retail DLL hash mismatch.

**Limitations:** the return value of `0x18800` / `0x181E0`
is unknown. This static flow does not prove which side of
`0x18FCE` is taken at game startup, how many worker
iterations occur, or how this first call reaches later
`0x196AE` / `0x19798` sites.


#### First-case matcher: exact 0x28-byte record search

After the now-proven first selector chooses case **1** at RVA
`0x183C2`, that case pushes its arguments and executes the
original **`CALL 0x189F0` at `0x183D4`**. The exact-original
read-only disassembly of `0x189F0` reveals an **unsigned
record-search loop**:

- The original input offset is compared unsigned against an end
  offset. If `offset >= end`, the helper returns EAX=0.
- Each iteration calls the original routine **`0xA810` at
  `0x18A15`**, with the current offset and length **`0x28`**
  (40 bytes), plus base and output arguments. The operation
  performed internally by `0xA810` is not yet reconstructed.
- After that call returns, the code reads a DWORD at offset
  **`0x24`** in the output record, computes
  **`record_field & mask`** and compares it with `expected`.
  On equality, it returns the **current record offset**.
- Otherwise, it advances the offset by **`0x28`** using 32-bit
  arithmetic, checks unsigned `offset < end`, and loops or
  returns zero. Whether the source ever reaches offset zero
  on a successful match is not established.

The exact opcode windows at `0x189F0`, `0x18A0C`,
`0x18A1A`, `0x18A28`, `0x18A33`, `0x18A3A` and `0x18A41`,
the direct rel32 calls, and three relevant Jcc displacements
are verified in `audit_original_first_case_record_matcher`.
Pure Rust helpers test the record predicate, unsigned limit
and wrapping record stride. The standalone Python
`verify_worker_entry_dispatch.py` reproduces the checks
against the SHA-256-verified original DLL, without executing
any original DLL code.

**Limitations:** input records, the opaque `0xA810` implementation
and the actual returned offset to the worker remain unknown.
Consequently, neither the first worker zero/nonzero return
branch nor any eventual global decoder-state value can yet be
predicted from these static instructions alone.


#### Original A810 read gateway: three conditional outcomes

The previous section's first-case record-search helper
`0x189F0` calls original **`0xA810`** to obtain one
40-byte record. This function resides in the **readable `.text`**
section, so its immediate dispatch conditions can be inspected
without decoding protected `.txt` instructions.

At `0xA810` it reads an argument pointer and compares the
DWORD at its **offset `+8`** against hexadecimal **`0x64`**.
An exact match returns immediately with **AX=0**, before calling
a backend. For other types it compares a separate **WORD** at
preferred address **`0x00904AE4`** with `1` (this is **not**
the mixer state at `0x00905010`). When that WORD is not `1`,
the original **`CALL 0xA870` at `0xA82A`** runs an
initialization probe; if its returned **AX is zero**, `0xA810`
returns early without dispatching a read backend.

When initialization is accepted, original
`CMP DWORD [ESI+8],0x65` at `0xA841` selects:

| Original type field | Original branch |
| --- | --- |
| Exactly `0x64` | Immediate `AX=0` (no backend call) |
| Exactly `0x65`, initialization accepted | `CALL 0xA560` |
| Any other type excluding `0x64`, initialization accepted | `CALL 0xA620` |
| Non-`0x64` type, initialization failed | Immediate return with `AX=0` |

The SHA-pinned Rust auditor `audit_original_a810_backend_dispatch`
verifies nine opcode groups in `.text` and all three direct
call targets, without invoking them. Three pure synthetic
unit tests exercise type and initialization outcomes; the
independent Python verifier also checks the original bytes
and all four conditional branch displacements.

**Limits:** the source of the argument type, meaning of the
initialization WORD, effects of `0xA870`, `0xA560`, and `0xA620`,
and the record contents are not established here. The
return value of `0xA810` is not necessarily the return value
of the first worker invocation. No game memory is accessed.


### Original process-memory and file-read backends: A560, A620 and A870 (2026-10-28)

New **read-only, SHA-256-anchored** inspection of the original
unencrypted `.text` code settles which backends the previously
recovered `A810` data-source dispatcher actually uses:

| Source type (`[source+8]`) | Backend | Original behavior (conditional on valid inputs) |
| --- | --- | --- |
| `0x64` | No backend | `A810` returns `AX=0` immediately |
| `0x65` | `0xA560` | Seek file handle, then read exact requested byte count |
| `0x66` | `0xA620` | Read another process's memory through a dynamically loaded callback |
| `0x67` | `0xA620` | Copy directly from a mapped/local memory pointer |
| Other | `0xA620` | Returns `AX=0` (unsupported original case) |

**File backend A560.** The original code first checks type 0x65.
At `0xA578`, its genuine PE IAT call is
`KERNEL32.dll!SetFilePointer` (`IAT RVA 0x1E044`), with handle
from `[source]`, offset argument and `FILE_BEGIN` origin 0.
If EAX equals `0xFFFFFFFF`, the original code returns `AX=0`
without consulting `GetLastError`, so it can treat a potentially
ambiguous return as failure. Otherwise it calls the real
`ReadFile` import at `0xA597` (`IAT 0x1E040`) and returns
`AX=1` **only if** the imported function succeeds and the DWORD
written to its `lpNumberOfBytesRead` equals the requested count.
Partial reads or failures return `AX=0`.

**Remote-process backend A620, type 0x66.** The original
`0xA6C3` instruction calls an **indirect function pointer**
at preferred VA `0x00904AE0`; it passes a process handle
from `[source]`, an address computed as `[source+4]+offset`,
an output pointer, a requested byte count and a bytes-read
output pointer. It returns `AX=1` **only if** that callback
returns nonzero and the actual count matches the request.
On some failures it also calls the genuine imported
`GetLastError` at IAT `0x1E04C`. The function pointer's
identity is independently proven by the original resolver below.

**Local-memory backend A620, type 0x67.** It calls `0xB5D0`
to retrieve an optional mapping descriptor, then tests the
pointer at descriptor offset `+0x3C`, a signed-nonnegative
offset and a size at `+0x40`. Its mapped branch copies data
with `REP MOVSD`/`REP MOVSB` from the descriptor's data
pointer plus offset. A separate fallback copies from
`[source+4]+offset`. Both direct-copy branches set `AX=1`
after executing their respective copies.

**Map descriptor lookup B5D0 is now independently explained.**
The source pointer passed to `B5D0` is compared against a small
table at preferred VA **`0x00904AE8`**. Each descriptor is
**`0xD0` bytes** long; the scan ends before VA `0x00904EF8`,
so there are exactly **five** candidate descriptors, not an
unbounded search. For each candidate, the original code first
checks a **WORD at offset `+0` equals 1**; active candidates
are compared with **12 bytes** from the supplied query against
the descriptor's key at offset `+4` using `REPE CMPSB`.
The first matching descriptor is returned; otherwise EAX=0.

These five descriptors reside in the **virtual zero-filled**
part of `.data`, so the original on-disk DLL contains no
runtime descriptor keys or active flags. Their values cannot
be recovered by pretending the file contains initialized data.
`audit_original_memory_map_lookup` verifies the exact original
opcodes, five-entry/stride geometry and branch targets;
its two synthetic tests check active exact-match semantics.

**Safety limit:** the mapped branch clamps its copy count to
`min(requested, descriptor_total_size)`, not
`descriptor_total_size - offset`. The fallback path does not
perform an independently established bound check. This is
static historical disassembly, **not** a safety guarantee
or a recommendation to run these routines on untrusted data.

#### Matching original write backends: AA00 / A5C0 / A720

The exact original `.text` code also contains a **write gateway**
at RVA **`0xAA00`**, structurally parallel to read gateway `0xA810`.
It checks type `0x64` (early AX=0) and the same initialization WORD
at preferred VA `0x00904AE4`, invoking `A870` when needed.
Upon accepted initialization, **type `0x65`** dispatches to
the original file writer **`0xA5C0`**, and other types
dispatch to **`0xA720`**, which recognizes only `0x66` and `0x67`.

| Source type | Original write operation | Condition for AX=1 |
| --- | --- | --- |
| `0x65` | `A5C0`: `SetFilePointer` followed by imported `WriteFile` | Seek return != `0xFFFFFFFF`, nonzero WriteFile AX and exact written byte count |
| `0x66` | `A720`: dynamically resolved `WriteProcessMemory` through VA `0x00904ADC` | Nonzero returned EAX and exact written byte count |
| `0x67` | `A720`: source-memory copy with two calls through `VirtualProtect` pointer VA `0x00904AD8` | Both protection changes report success on the audited success path |
| Other | Unsupported backend | AX=0 |

In the `0x67` branch, the first call to `VirtualProtect` supplies
**new protection `0x04` (`PAGE_READWRITE`)**. After the
original `REP MOVSD` / `REP MOVSB` copy, the routine invokes
`VirtualProtect` again with the saved prior protection. This
second invocation is checked, and failure produces AX=0.

**Caution:** the source also contains an earlier, conditional
mapped-memory copy before the first `VirtualProtect` call.
Therefore a later AX=0 result is **not a proof that nothing
was written**. The original code may also copy into memory
without establishing comprehensive address bounds. These
observations are conditional source-code semantics only.

The new `audit_original_write_backends` verifies original
readable `.text` instruction sequences, the AA00/A5C0/A720
direct call targets, the genuine imported `WriteFile` and
`SetFilePointer` IAT identities and both dynamically populated
process-memory pointer slots. Synthetic Rust tests cover
the write backend type selection, full-write comparisons
and both protection outcomes. The separate offline Python
verifier checks these original-byte sites without loading
the DLL or invoking any Windows API.

#### Four XOR-encoded names establish the dynamic API imports

Four null-terminated encoded byte strings are embedded in
the original `.data` at RVAs `0x204B8`, `0x204C8`,
`0x204E0`, and `0x204F8`. The **original decoder `0xA970`**
uses a chained recurrence for the ith output character:

```text
plain[0]   = encoded[0]
plain[i>0] = plain[i-1] XOR ((encoded[i] - 1) & 0xFF)
```

Running this rule against the **exact original bytes** yields:

| Encoded-string RVA | Decoded name |
| --- | --- |
| `0x204B8` | `Kernel32.dll` |
| `0x204C8` | `ReadProcessMemory` |
| `0x204E0` | `WriteProcessMemory` |
| `0x204F8` | `VirtualProtect` |

The original initialization routine `0xA870` passes these
names through its string decoder and dynamic resolution path
(via the imported `LoadLibraryA` and `GetProcAddress`).
The three dynamically resolved functions are stored at:

| Preferred VA | Dynamically resolved function |
| --- | --- |
| `0x00904AE0` | `ReadProcessMemory` |
| `0x00904ADC` | `WriteProcessMemory` |
| `0x00904AD8` | `VirtualProtect` |

These are **dynamically populated global pointer slots in
zero-filled `.data`**, not static entries in the DLL's
import address table. Original `A620` type-66 code calls the
first global; other routines use the remaining functions.
The use of process-memory APIs provides an original-code
mechanism for inspecting or changing process memory, but does
**not prove** which target process/address was used during
Crimson Skies startup or whether those calls succeeded.

New module `src/unsafedisc_dll_io_backends.rs` checks the
four decoded names, original decoder and API resolver
opcodes, actual import identities, A560 file-read opcodes
and A620 process-memory/mapped-copy branch instructions.
It includes pure synthetic tests for the type switches,
file/process full-read and full-write success conditions,
XOR decoding, virtual-map matching and protection outcomes. A separate standalone
`verify_io_backends.py` uses only the Python standard library
to check original file-backed bytes; it **passes** on the
retail SHA-256-pinned DLL and **rejects** a tampered 1-byte copy.

**Outstanding:** the input types and byte records observed
at runtime; effects of `A870` initialization failures;
the semantics of map helper `B5D0`; the protected code
read from the external process; original loader entrypoint
reachability and the final global mixer-state sequence.
No DLL code, SafeDisc driver, Windows API or executable
decryption was run or enabled by this research.


### First worker case is a PE32 executable-section locator (2026-10-28)

The previously reconstructed first worker invocation always supplies
selector **1** to the 11-case `0x181E0` dispatcher, conditional on
its preparatory helper returning normally. Its case-1 RVA `0x183C2`
is now identified as a **PE executable-code section finder**.

#### The original PE header and section-table checks

The genuine code at **RVA `0x18940`** makes two original
`A810` reads: a **0x40-byte DOS header** at offset zero and
a **0xF8-byte NT header** at the DWORD `e_lfanew`
at DOS-header offset `+0x3C`. It then compares the NT
header's first DWORD against `0x00004550` (the `PE\0\0`
signature), returning a one-byte boolean in AL.

**Important limitation:** the historical code does not check
the success return of either `A810` read in this short routine.
Nor does it explicitly verify `MZ` there. For security, the
new *independent* Rust metadata parser **does** require MZ,
PE32, valid section-header extents and bounded section counts.
That parser is intentionally stricter than the original x86
implementation. No live game source, disk handle or process
memory is read by the new code.

The genuine dispatcher at **`0x18204..0x1822C`** extracts
`SizeOfOptionalHeader` and `NumberOfSections` as unsigned
16-bit quantities from the NT header and calculates a
section-table start and end, using **0x18 bytes for the
PE signature + COFF file header**, and **0x28 (40) bytes
per section header**.

Case **`0x183C2`** invokes the original record scanner
`0x189F0` with mask **`0x20000020`** and expected value
**`0x20000020`**. In PE section flags, these bits are
`IMAGE_SCN_CNT_CODE` (`0x20`) and
`IMAGE_SCN_MEM_EXECUTE` (`0x20000000`). Hence the first
matching section is one which is simultaneously marked
as **code and executable**. Other characteristics bits
(including IMAGE_SCN_MEM_READ) are deliberately ignored.

From the selected 40-byte section header, original case 1
writes **PointerToRawData** (header +0x14) and
**VirtualAddress** (header +0x0C) to its output pointers.
Its call to `0x18B90` selects a length using
`min(VirtualSize, SizeOfRawData)` if VirtualSize is nonzero,
otherwise SizeOfRawData. The original selected length is
metadata, not a reconstructed or decrypted code section.

#### Reproduced on all four SHA-256-verified original files

The independent stdlib-only local checker
`verify_pe_section_case1.py` read all four exact-original
uploaded inputs and applied the section predicate. Results:

| Original file | First executable code section | Header file offset | Raw data file offset | RVA | Original selected length |
| --- | --- | ---: | ---: | ---: | ---: |
| `CRIMSON.EXE` | `.txt` | `0x1A8` | `0x400` | `0x1000` | `0xEA00` |
| `CRIMSON.ICD` | `.text` | `0x1F0` | `0x1000` | `0x1000` | `0x2014BD` |
| `testme.exe` | `.text` | `0x1F0` | `0x1000` | `0x1000` | `0x2014BD` |
| `dplayerx.dll` | `.txt` | `0x1A8` | `0x400` | `0x1000` | `0x7671` |

The checker also verifies **13 exact opcode groups** of
original `dplayerx.dll` and **six original E8 rel32 call
targets** (`0x18951/0x18968 -> A810`, `0x181FF -> 18940`,
`0x183D4 -> 189F0`, `0x18405 -> 18B90`,
`0x18A15 -> A810`). A standalone synthetic parser test
covers combined/individual flag masking, the first-match
rule, length calculation, absent executable sections,
truncation and invalid PE metadata. The new Rust module
is `src/unsafedisc_dll_pe_sections.rs`; its original-image
static audit is SHA-gated under `inspect --modifier-research`.

**Interpretation is deliberately narrow:** these four selected
sections are reproducible **file-metadata results**, not proof
which source type or file the running SafeDisc worker really
opened, what bytes it successfully read, or which data it
fed to the later byte mixer. In particular, although the
reference `testme.exe` and protected `CRIMSON.ICD` have the
same first-section layout, that alone does not decrypt
their differing code bytes or recover either EAX modifier.
**No SafeDisc unpacking was enabled and no retail file was
modified or published.**


### Original previously opaque A310 worker helper: source gate and cache (2026-10-28)

The earlier worker source-helper call **`0x196AE -> 0xA310`**
was previously treated as an unknown function whose returned
**low 16 bits** determine whether the worker proceeds toward
the byte mixer. The actual `0xA310` routine resides in the
original **readable `.text`** section and has now been partially
reconstructed using exact-original source instructions.

On entry it first checks two required DWORD pointers:

1. The original cursor-position pointer at `[ESP+0x18]`
   (after the four preserved-register pushes) must be nonnull.
2. The limit pointer at `[ESP+0x1C]` must also be nonnull.
3. It reads the two pointed-to DWORDs and compares them
   **unsigned**. If `cursor >= limit`, it jumps to original
   `0xA3FF`, which returns **AX=0**.
4. Otherwise it calls original **`0xB5D0`** to find a
   matching source descriptor by the previously audited active
   12-byte key. If B5D0 returns zero, this A310 call returns
   **AX=0** as well.

**Only when all four preconditions are satisfied** does the
routine proceed into its cache and processing logic. This
accounts for several real source-level early-zero outcomes
without guessing the success of any memory or Win32 calls.

#### Original 4 KiB page cache and delegated processing

After successful map lookup, the original instructions compare
its cached page-position DWORD at descriptor `+0x34` with the
cursor, and compare the limit with cached descriptor `+0x38`.
A nonzero cursor aligned to a 4096-byte boundary is first
decremented by one **for this comparison only**.
The actual page-index test is
`((probe_cursor XOR cached_cursor) & 0xFFFFF000) == 0`.

Depending on the cache values and an independent WORD at
preferred VA **`0x00904AD4`**, the original code may call
`0xC0D0`, `0xC0C0` or `0xBFB0` before updating the cached
cursor and limit. Their effects are not treated as known.

The common final call **`0xA3F2 -> 0xBED0`** receives arguments
derived from the caller and the map record; **A310 returns
BED0's return value**, with no additional invented success
or failure interpretation. The existing worker checks only
its low 16 bits. That returned value remains unresolved
until BED0's own logic is reconstructed.

`src/unsafedisc_dll_a310.rs` verifies the exact original
instruction windows, every named original rel32 call
(196AE->A310, A33E->B5D0, A394->C0D0, A3A2->C0C0,
A3C8->BFB0, A3F2->BED0), and the null-pointer/unsigned
comparison branch offsets. Four synthetic Rust tests model
the three early-zero exits and the 4 KiB boundary probe.
The standalone local Python checker `verify_a310_worker_gate.py`
independently validates the same original file-backed bytes
with a pinned SHA-256 gate. None of this executes the code.

**Limits:** pointer validity after passing the null check,
active map contents, page-cache contents, runtime page-boundary
behavior and BED0's outcome are not established. This is a
factual partial reconstruction of a former opaque helper,
not a complete or safe process-memory reader.


### BED0 eight-stage worker engine: selected index 4, seven followups (2026-10-28)

**New original-file-backed finding.** The earlier analysis of `0xA310`
stopped at its call to `0xBED0`. The actual `0xBED0` implementation is
in the readable original `.text` of `dplayerx.dll`, and its bounded
control flow can now be described without executing any game code.

The known worker call at **RVA `0x196AE`** supplies five arguments to
`0xA310`, including **`PUSH 0x04` at `0x196A0`**. On the previously
identified valid-pointer, in-range, mapped-record path, `0xA310`
forwards this fourth argument unchanged to `0xBED0` in the
straight-line sequence **`0xA3E3..0xA3F6`**. In the `0xBED0` frame,
**`MOV EBX,[ESP+0x38]`** reads that original fourth argument.
Thus the **specific already known worker path uses index 4**.
This does not assert that other callers of BED0 use index 4.

#### Actual original stage sequence

- `0xBED0` makes a **local copy of eight DWORD slots** using
  `MOV ECX,8; REP MOVSD` at `0xBEEC..0xBEFD`. Its later
  original accesses to the local slots are 4-byte-spaced.
- It selects the DWORD at stack index `EBX` and invokes
  **`CALL 0xBDD0` at `0xBF0F`**. If that helper returns
  **low AX zero**, the source jumps directly to output writeback
  at **`0xBF4E`**; if **selector equals 3**, it also skips
  further stages regardless of nonzero first AX.
- Otherwise it loops over **indices `0..7`**, skipping the
  selected index, and invokes **`CALL 0xBCA0` at `0xBF38`**
  for each other index. It stops immediately on the first
  BCA0 result with **AX zero** or after exhausting eight indices.
- Therefore, on the known worker's selector **4**, the ordered
  BCA0 sequence is **`0, 1, 2, 3, 5, 6, 7`** if all
  preceding BCA0 calls return nonzero. This is a maximum
  of **seven** follow-up calls, not eight.
- The common BED0 epilogue at **`0xBF4E..0xBF66`**
  **unconditionally writes two working DWORDs through output
  pointers**, even if a stage returned AX=0. The returned
  low AX is that of the last invoked stage. In particular,
  a zero result is *not* proof of no memory modification.

The original **`0xBDD0`** failure branch is more specific than
a generic error: after setting its local EBP to zero, the original
`0xBE52..0xBE5E` executes `MOV EDX,[EBX]; DEC EDX;
MOV [EDI],EDX` and returns with **AX=0**. In its BED0
call, EDI is the selected slot's pointer, and EBX addresses
another working DWORD; thus **a failed BDD0 can write
`selected_pointee = (third_arg_value - 1) mod 2^32`**.
This store requires a usable pointer, which is a runtime
assumption, and is deliberately **not** elided from the
source-level model.


#### BDD0 also clips the matched interval with C430 / C450

After the candidate test succeeds, original `0xBDD0`
calls two independently readable `.text` helpers:

- **`0xBE24 -> 0xC430`** returns unsigned
  `max(candidate_start, requested_lower)`.
- **`0xBE34 -> 0xC450`** returns unsigned
  `min((candidate_start + candidate_length - 1) mod 2^32, requested_upper)`.

On the successful `BDD0` branch the resulting two endpoints
are written back to its two pointer operands; the matched
list node is stored in an additional output field and `AX=1`.
The helper `0xC450` uses an **inclusive upper endpoint**
and `LEA` with **wrapping 32-bit arithmetic**, not checked
64-bit addition. For example, candidate `[10,19]` restricted
to `[12,15]` gives endpoints 12 and 15. A candidate with
start `0xFFFFFFFF`, length 2 produces inclusive endpoint zero
under the original wrapping rule; it must not be treated
as a safe, generalized interval operation.

The pure Rust helpers `original_c430_lower` and
`original_c450_upper` reproduce these exact operations.
The original-byte audit pins both opcode windows, both
relative calls, and the unsigned conditional branches;
two synthetic Rust tests cover ordinary bounds and wraparound.

#### C550: exact unsigned range predicate inside BDD0

The original `0xBDD0` and its `0xBE60` list-search helper
both invoke the independent **`0xC550`** predicate. Its
source code is directly readable in `.text`. Given an
original pair `(start,length)` and query `(low,high)`,
the real unsigned x86 branches implement:

```text
if start < low:
    return low < ((start + length) & 0xFFFFFFFF)
if start <= high:
    return true
if start > low:
    return false
return low < ((start + length) & 0xFFFFFFFF)
```

Thus it tests a form of **unsigned interval intersection**
but is *not* a safe general range validator. It uses
**wrapping 32-bit arithmetic**; notably a zero-length
range can satisfy the second branch, and a range wrapping
past `0xFFFFFFFF` may be reported as non-overlapping.

The Rust function `original_c550_range_predicate` is a
pure exact-control model. Its original-byte audit checks
the `0xC550` opcode sequence, four conditional
displacements and both original callsites
`0xBE11 -> C550`, `0xBE7D -> C550`.
Two synthetic tests cover boundaries, truncation and
wrapping. The independent Python script
`verify_bed0_pipeline.py` now verifies **15 opcode windows,
15 relative call targets and 14 branch destinations**
on the exact retail DLL and rejects a changed one-byte copy.

The original **`0xBDD0`** success branch eventually sets its
return AX to one, updates two local pointer operands and records
the selected node. Its failure path at **`0xBE52`** sets
AX to zero but **also writes** a decremented pointer/cursor
value before returning. The secondary **`0xBCA0`** has a
separate, conditionally successful writeback branch at
`0xBCFA..0xBD09`; it invokes the still partially unresolved
`0xBD10`, `0xBC00` and `0xBD50` helpers. Actual stage
success and output DWORD values remain undetermined.

#### Verified code and limitations

The new read-only Rust module
`src/unsafedisc_dll_bed0.rs` checks **12 exact original opcode
windows, 12 original relative call targets and eight Jcc
displacements** against the SHA-256-pinned DLL, then models
the conditional 8-stage dispatch using explicitly supplied
hypothetical helper AX values. Six synthetic unit tests
cover stage ordering, selected-slot skipping, early failure,
the special selector 3, nonzero AX values and a deliberate
rejection of out-of-range selectors. The original code does
not safely validate that selector before using it as a stack
index, so the synthetic model intentionally refuses values
outside `0..7` instead of claiming them safe.

An **independent** standard-library Python audit
`verify_bed0_pipeline.py` independently checked the same
original-byte sites and conditional stage order against the
exact retail DLL. Its SHA-256 check also correctly rejected
a deliberately modified one-byte copy. **No original bytecode,
process memory or Windows API was executed.**

Remaining unknowns include the actual values of the eight
working DWORDs, the results and side effects of BDD0 and BCA0
and their deeper `C*` helpers, runtime reachability of this
worker path, and the decoder state at the byte mixer.
These findings **do not** recover the SafeDisc TEA EAX
modifiers or enable `unpack`.
