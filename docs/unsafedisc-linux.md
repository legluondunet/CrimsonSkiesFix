# unSafeDisc Linux — experimental native port for Crimson Skies

This branch begins a **native Rust implementation for Linux and Windows** of the unSafeDisc
1.5.5 research workflow for the *Windows PC 1.0.2.0* release of **Crimson Skies**.

**Current status: inspection only, not a working unpacker.** The new
`unsafedisc-linux` binary reads and verifies locally supplied game files,
extracts SafeDisc-specific key material and optionally validates a reported
32-bit import-address XOR key against a known working unwrapped executable.
It **does not** modify or reconstruct a game executable, run a Windows DLL,
interact with `secdrv`, load Wine, or decrypt the protected ICD.

## Independent offline comparison of the exact original binaries (2026-10-27)

The user provided a private 7z archive containing all four required retail/research binaries.
All four were extracted locally and independently matched their trusted hard-coded
SHA-256 and length values, including the unprotected TESTME.EXE reference.
**No binaries were committed, uploaded to GitHub, or executed.**

### Shared PE image geometry; exact bytewise section differences

CRIMSON.ICD (2,580,578 bytes) and TESTME.EXE (2,580,480 bytes) have
the same PE32 ImageBase 0x00400000, entry-point RVA 0x001F7056,
six section names, RVA layouts, raw offsets and section raw lengths.
They are directly comparable at equal file offsets:

| Section | Bytes different | Bytes total |
| --- | ---: | ---: |
| PE headers | 54 | 4096 |
| .text | 1,813,201 | 2,105,344 |
| .rdata | **0** | 90,112 |
| .data | 167,371 | 172,032 |
| .idata | 3,362 | 16,384 |
| .rsrc | **0** | 24,576 |
| .reloc | 2 | 167,936 |

The protected ICD additionally contains 98 trailing bytes beyond
TESTME.EXE, including the NB10 debug-information signature.
Thus read-only resource data is already preserved, but protected
code, writable data and selected imports differ extensively.

### High-confidence relocation-preservation finding

Parse the real ICD base-relocation directory (not a guessed offset),
map every 4-byte HIGHLOW reference whose start lies in .text,
then compare the original protected and unwrapped files:

| ICD .text byte class | Identical bytes | Total bytes | Rate |
| --- | ---: | ---: | ---: |
| HIGHLOW-covered | **246,598** | **246,972** | **99.849%** |
| Other .text bytes | **45,545** | **1,858,372** | **2.451%** |
| Entire .text | **292,143** | **2,105,344** | **13.876%** |

Of the **61,743 original .text HIGHLOW DWORDs**, exactly
**61,498** are fully identical; the remaining **245** differ.
Every one of the 245 mismatched DWORD values in **both** files
is a four-byte-aligned preferred VA pointing into .idata, in
the approximate range 0x00A1FC7C–0x00A20490.
That is evidence for special import-reference handling as
well as relocation-preserving code transformation.

The 245 site-specific mismatches cannot safely be repaired
by a global old-VA → new-VA dictionary: the protected side
has 111 distinct values versus 89 distinct reference values;
**65 protected-side values have multiple different reference
targets** depending on the site. Any repair needs stronger
instruction/call-site or deprotection evidence.

### Exact first DWORD XOR key independently corroborated

The 8 DWORDs at ICD raw offsets
0x2433D0, 0x243A48, 0x243A50, 0x243AB8,
0x243D78, 0x2443F0, 0x2443F8, 0x244460
each satisfy (protected DWORD XOR reference DWORD)
= **0x7496538F**, directly matching the reported import-XOR
key word. This says nothing yet about the correctness of the
other three TEA words, the on-disk DLL title-specific EAX
modifiers at RVAs 0x65BE / 0x6621, or a valid decryptor.

### Practical next decryption validation gate

Before any unwrapping/export code is enabled, validate one
entire 4096-byte ICD .text page against TESTME.EXE. Compare
non-relocation bytes separately from the 4-byte HIGHLOW words,
and verify the latter's 245 site-dependent import variants.
Do not count incidental equal bytes as evidence of decryption,
do not publish the user-supplied binaries, and do not execute
the original DLL in the native CLI.
### Rust-native comparison oracle: optional --reference-audit

The optional inspect **--reference-audit** flag allows the Rust CLI
to repeat the most important independent analysis locally when
the owner supplies the **exact SHA-256-verified original**
TESTME.EXE reference. It is a verification gate, not an unpacker.

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --reference '.\TESTME.EXE' --reference-audit
```

The comparison uses the real ICD HIGHLOW relocation directory to
mark each complete 4-byte relocation word, reports identical
protected/reference DWORDs separately from unrelated .text
bytes, and gives first-page non-relocated byte matches.
Differing relocation DWORDs are counted as import-address
variants only if both VAs are aligned and lie in the original
.idata section. A source VA mapping to more than one unwrapped
VA is detected as an ambiguous global import mapping.

The implementation reads the inputs and prints counts only;
original binaries are not embedded in test fixtures, distributed
or committed to this repository. A synthetic test exercises
relocation-covered equality, non-relocated differences and
ambiguous IAT mappings without depending on game files.
## Recovered selection/CRC gates from the owner's genuine unSafeDisc 1.5.5 (2026-10-27)

The owner supplied the original historical zip containing
`unSafedisc.exe` (20,480 bytes), `readme.txt` and `history.txt`.
The exact PE32 executable SHA-256 is
`e484c9aadf8b3b5bf0db58b26cb9ecfcaba0fd971fb1bb66216f558f0f1eaa9f`.
It has an ordinary IMAGE_BASE 0x00400000, CODE and DATA sections,
and **no need for the later third-party version.dll import modification**.
The executable is analyzed statically and kept private; neither this
binary nor the user's game binaries are committed to the repository.

The release's **own history.txt explicitly identifies Crimson Skies**:

- v1.5.4 fixed a bug in which decryption mistakenly included Crimson
  `.rdata`; the authors used `CRIMSON.ICD` to reproduce it.
- v1.5.5 fixed the relocation-aware CRC regression introduced during
  the Crimson workaround, and was tested against both Crimson and
  Unreal Tournament.

Independent static x86 inspection of the actual 20,480-byte original
disassembled with `objdump -D -Mintel -j CODE` confirms:

- `0x402257` through `0x4022B0`: collect section headers except
  those whose **first four raw name bytes** are `.rsr`, `.bss`,
  `BSS\0`, `.tls`, `.ida`, `.rda`, `.eda` or `.rel`. In
  particular, `.rdata`, `.idata`, `.rsrc`, `.reloc` are skipped.
- `0x401D1A` through `0x401D34`: cap the processing extent to
  `min(SizeOfRawData, VirtualSize)` if VirtualSize is nonzero.
- `0x401F01`: checksum byte accumulator only runs when selected
  `IMAGE_SECTION_HEADER.Characteristics` contains bit `0x20`
  (`IMAGE_SCN_CNT_CODE`); data sections can still be selected
  for processing without contributing to that CRC stage.
- `0x401F43` through `0x401F62`: original DLL size `0x21E00`
  (138,752 bytes) activates the special extra pre-TEA stage.
  That exactly matches the owner's Crimson `dplayerx.dll`.
- `0x401A55`: standard 32-round TEA-like paired DWORD transform
  for the protected payload, but it is **not sufficient** without
  the DLL-specific modifiers and actual key.

Applying this documented historical section filter directly
to the original, hash-verified Crimson ICD PE metadata gives:

| Original ICD section | Included in processing | Code-bit CRC eligible |
| --- | --- | --- |
| `.text` | **yes** | **yes** |
| `.data` | **yes** | no |
| `.rdata` | no | no |
| `.idata` | no | no |
| `.rsrc` | no | no |
| `.reloc` | no | no |

The new native read-only flag `--historical-selection` reports this
result using the normal SHA-256 verified retail loader, ICD and DLL:

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --historical-selection
```

It is guarded by a synthetic Rust test for the eight original
prefix exclusions and the independent `IMAGE_SCN_CNT_CODE` gate.
The actual historical SafeDisc DLL is **not loaded or executed**;
this reproduces a static name-based filter, **not a successful
whole-section decryption**. The `.data` codebit CRC exclusion
should not be confused with skipping its separate decode step.
## Major breakthrough: exact original import permutation recovered (2026-10-27)

With the user's privately supplied, SHA-256 verified original
unSafedisc.exe **v1.5.5**, actual disassembly of
`0x4017AC..0x401821` and `0x401869..0x4018D9` now explains the
apparently noninvertible mappings of `.text` HIGHLOW DWORDs.

These are the **245 differing relocation-covered DWORDs**
between the private original CRIMSON.ICD and the exact
unwrapped TESTME.EXE reference. Every one is an aligned
Windows import-table VA from one of **two** original IAT
tables:

| Original imported table | Image VA start | Table entries | Differences |
| --- | --- | ---: | ---: |
| `KERNEL32.dll` | `0x00A1FC7C` | 74 | **182** |
| `USER32.dll` | `0x00A2039C` | 62 | **63** |
| **Total** | | | **245** |

Historical code uses the instruction site's location to
rotate the import index differently for every `FF15`
instruction. The following pure arithmetic independently
reproduces all 245 differences:

```text
first_word = 0x7496538F
text_start_rva = 0x1000
site_rva = RVA of original HIGHLOW operand DWORD
table_first_va, count = (0x00A1FC7C, 74) or (0x00A2039C, 62)
old_index = (encrypted_import_VA - table_first_va) / 4
shift = (site_rva - text_start_rva - 2 + first_word) % count
new_index = (old_index + count - shift) % count
restored_import_VA = table_first_va + 4 * new_index
```

**Direct independent validation:** 182/182 KERNEL32 references
and 63/63 USER32 references match the exact word at the same
RVA in TESTME.EXE: **245/245 (100%), with no exceptions**.
The two modules use different table lengths, hence different
moduli, and the rotation depends explicitly on the operand
RVA. This resolves the earlier apparent ambiguity that
65 protected VAs could map to multiple reference VAs.

### Independent modular constraint on the first key DWORD

The 245 original import-reference pairs themselves imply:

```text
first_word % 74 = 73    (all 182 KERNEL32 sites)
first_word % 62 = 49    (all 63 USER32 sites)
first_word % lcm(74,62) = 2219, with lcm = 2294
0x7496538F % 2294 = 2219
```

This is an **independent congruence check** on the reported
first SafeDisc key word from code-reference rotations.
It alone does NOT uniquely recover a 32-bit word;
the eight separately verified `.idata` XOR differences
still provide the full 32-bit first word.
Example original source RVA `0x1294` has protected value
`0x00A1FD78`, restored to reference `0x00A1FD9C`; USER32
site `0x48C5` has `0x00A20458` → `0x00A20430`.

The optional native Rust `--reference-audit` now derives
the expected restored VA for every changed imported
relocation word, reports exact matches and unexplained
mismatches, and refuses an unexpected total for the
exact known verified Crimson image. The historical
import-index transformation is implemented with **no
execution of the game, SafeDisc DLL or Windows loader**.

Important: This is the *import reference correction* stage;
the pre-TEA modifier functions and `.text`/`.data`
plaintext decoding remain unresolved. It does not mean
the complete game can yet be unpacked or executed.
## Independent native checksum reproduction from original v1.5.5 assembly (2026-10-27)

With the exact, privately supplied original unSafedisc.exe and
the SHA-256 verified CRIMSON.ICD, the relevant x86 instructions
were transcribed **independently into a temporary Python
analysis**, without relying on Rust's checksum implementation.

- `0x401CDE`: zero the checksum accumulator.
- `0x401D1A..0x401D34`: cap `.text` processing to its
  VirtualSize (2,102,461 bytes) rather than full raw
  SizeOfRawData (2,105,344 bytes).
- `0x401D82..0x401EDD`: walk relocations and split
  contiguous data at HIGHLOW relocation windows and
  4-KiB boundaries; every skipped HIGHLOW DWORD
  consumes exactly four bytes, even when it straddles a page.
- `0x401F01..0x401F3D`: mix non-relocation bytes only
  when the CODE section characteristic is present.
  Each nonempty fragment starts weight `0xFD379AB1`,
  with multiplier `0xA7753394` and offset `0x3BC62BB2`.

Actual offline original-file measurements:

| Measured quantity | Result |
| --- | ---: |
| `.text` selected VirtualSize | 2,102,461 bytes |
| `.text` original HIGHLOW DWORDs | 61,743 |
| HIGHLOW byte coverage | 246,972 bytes |
| HIGHLOW words crossing 4-KiB pages | 36 |
| Non-relocated bytes mixed | 1,855,489 |
| Page-bounded nonempty mixer calls | 59,622 |
| **Independent historical checksum** | **`0x1B09633B`** |
| Deliberately incorrect section-contiguous alternative | `0xA181CB30` |

**The historical page-bounded checksum matches the independently
implemented Rust research result byte-for-byte.** The
section-contiguous alternative demonstrably differs,
confirming that it is invalid for this original relocation
traversal. No process or DLL was executed.

Using the separately corroborated first word
`0x7496538F`, the original game's four raw DWORDs
`E51DC3D7 63CA5EAC 4493379C 08B42B8C`,
and entry RVA `0x1F7056`, the original unSafeDisc
`0x401CA2..0x401CDD` key-staging arithmetic gives the
same conditional four-word **TEA candidate** as before:

```text
0x7496538F 0xE9C7914F 0xCF10EA3F 0x82F5DE2F
```

The checksum is strongly independently supported by the
historical original and the actual game's relocation layout.
However, the resulting TEA candidate has **not yet
been authenticated by a successful plaintext-page
decrypt**, because the two title-specific SafeDisc DLL
modifiers at RVA `0x65BE` and `0x6621` are still encrypted
in the original on-disk DLL. Do not claim completed
unpacking or a standalone working reconstructed EXE.
## Cross-section E8 calls into protected encrypted .txt (2026-10-27)

Independent offline inspection of the SHA-256 verified original
Crimson `dplayerx.dll` establishes an important limit on the
earlier negative PE relocation xref reports: **relative E8
CALL instructions contain signed displacements, not absolute
VA pointers**, so they require no PE base relocation.

A bounded scan over the *other* executable sections
`.text` and `.txt2` found five distinct on-disk E8 rel32
encodings whose calculated targets lie in `.txt`:

| Source section | CALL opcode RVA | Encoded destination `.txt` RVA |
| --- | --- | --- |
| `.text` | `0x9197` | `0x3B70` |
| `.text` | `0x989F` | `0x7212` |
| `.text` | `0x98C5` | `0x7C4F` |
| `.txt2` | `0x1ACF6` | `0x8620` |
| `.txt2` | `0x1AD4F` | `0x8480` |

All five actual original `E8 + signed rel32` instructions
were checked against the exact private file bytes and
mapped to backed, bounded `.txt` section target RVAs.
Context around the sites resembles ordinary x86 argument
setup (push / call / stack cleanup) in several cases.

The target section's original first 30,321 bytes have
approximately **7.981 bits/byte Shannon entropy**; the
other original backed executable sections measure roughly
6.589 (`.text`) and 6.296 (`.txt2`) bits/byte.
The `.txt` bytes at all five targets appear undecoded
on disk; no runnable plaintext procedure is established.

The historical modifier functions at RVAs `0x65BE` and
`0x6621` also reside in this same encrypted `.txt`.
None of these five candidate call targets matches the
two modifier RVAs, and CALL encoding alone is not proof
the branch is ever executed. Therefore this result
does not decrypt the DLL or establish a dynamic
self-unpacking sequence.

The already-existing `--attach-research` output now
includes **these five strictly verified static bridges**
and asserts both their actual E8 instruction bytes
and resulting `.txt` target bounds. A standalone
synthetic unit test shows that a real rel32 cross-section
CALL into a backed `.txt` function is invisible to a
HIGHLOW-absolute-pointer-only scan. No original bytes
are embedded in GitHub or its test fixtures.
## Original unSafeDisc DLL loading and encrypted relocation-slot evidence (2026-10-27)

Reanalysis of the user's original, privately supplied 20,480-byte
`unSafedisc.exe` identifies a key historical distinction:

- At absolute `0x402D50..0x402D5F`, the binary pushes a DLL
  name, `hFile=NULL`, and **flags=1** before calling the
  imported `LoadLibraryExA` trampoline `0x402E0C`.
  The constant 1 is Win32 `DONT_RESOLVE_DLL_REFERENCES`.
- The returned HMODULE is saved in historical global
  `[0x70CD5C]`. A few instructions later, the program reads
  `IMAGE_DOS_HEADER.e_lfanew` and the NT optional header
  `AddressOfEntryPoint` (PE offset `+0x28` from NT header)
  and computes an **explicit original DLL entrypoint VA**.
- The branch at `0x402D76..0x402D99` can then manually
  invoke the entrypoint with `(hinstDll, DLL_PROCESS_ATTACH=1,
  lpReserved=NULL)`; it is conditionally bypassed depending
  on the original process `DS`-selector test.

**This is a static Windows execution plan, not an actual
runtime observation.** `DONT_RESOLVE_DLL_REFERENCES` normally
prevents the Windows loader from invoking DllMain automatically;
the old tool implements a guarded manual startup path.
The retrieved `dplayerx.dll` has actual preferred ImageBase
`0x008E0000` and entry RVA `0xFD20`, so the calculated
preferred address is `0x008EFD20` (subject to actual load
relocation). The DLL's normal entrypoint calls its own startup
helper at `0xFC00` before the candidate 0x9770 attach path.
None of this proves that `.txt` decryption executes, but it
more precisely constrains where any initializer must operate.

### The encrypted `.txt` retains real IMAGE_REL_BASED_HIGHLOW pointers

An independent offline parse of the **actual** DLL relocation
directory and `.txt` original backed bytes establishes:

| Original `.txt` relocation property | Count |
| --- | ---: |
| HIGHLOW source DWORDs inside protected `.txt` | **214** |
| DWORD values pointing to `.data` | 195 |
| DWORD values pointing to `.rdata` | 18 |
| DWORD values pointing back inside `.txt` | 1 |
| Out-of-image absolute VAs | **0** |
| HIGHLOW DWORDs crossing a 4-KiB page boundary | **1** |

The one internal pointer originates at RVA `0x21A7` and
references VA `0x008E3270` (`.txt` RVA `0x3270`).
The known protected EAX modifiers at `.txt` RVAs
`0x65BE` and `0x6621` are still unreadable on disk.

Although the raw `.txt` stream has high entropy, the
relocation slots contain actual ordinary, in-image
absolute addresses. **This is strong structural evidence
that some metadata is preserved while surrounding bytes
are encoded.** Any future proposed `.txt` decoder must
preserve and correctly relocate these DWORDs, including
the page-crossing slot; it must not transform all bytes
of the section indiscriminately.

The native read-only `--modifier-research` now checks these
counts directly against the private SHA-256 verified
original DLL, prints section destination categories,
and rejects an unexpected geometry. A synthetic Rust unit
test uses only artificial PE headers/pointers. **No private
game DLL or historical executable is added to GitHub,
and no DLL entrypoint is ever invoked by this tool.**
## DLL attach path: CRT startup isolated from obfuscated 0x9770 handler (2026-10-27)

Further **offline static disassembly of the exact hash-verified
original Crimson `dplayerx.dll`** distinguishes an ordinary
MSVC-like DLL startup/CRT phase from its separate obfuscated
process-attach handler. This does not execute any SafeDisc
code or claim that the DLL `.txt` region has been decoded.

| Original DLL CALL RVA | Confirmed target RVA | Grounded classification |
| --- | --- | --- |
| `0xFC18` | `0x116E0` | Initializes process heap |
| `0xFC53` | `0x10820` | Initializes thread-local storage |
| `0xFC71` | `0x12FF0` | Initializes environment strings |
| `0xFD81` | `0x9770` | Calls **separate obfuscated attach handler** |

Exact x86 import operands inside the first three branches
were cross-checked against the ORIGINAL PE import descriptors:

- `0x116E9`: `FF15` → `KERNEL32!HeapCreate`;
  `0x116F9` also calls the allocator at `0x11990`.
- `0x10826`: `FF15` → `KERNEL32!TlsAlloc`.
- `0x12FF7`: `8B 2D` absolute import load →
  `KERNEL32!GetEnvironmentStrings`.
- `0x13003`: `8B 3D` absolute import load →
  `KERNEL32!GetEnvironmentStringsW`.
- `0x119C1`: `8B 3D` absolute import load →
  `KERNEL32!VirtualAlloc`. In this helper, the
  caller reserves 4 MiB and commits 64 KiB for a
  conventional heap-like allocator, **not proven**
  to allocate or decrypt DLL `.txt`.

This narrows the outstanding work: look for an actual
decoder in or called by the **distinct process-attach
handler at RVA `0x9770`** (or some proven predecessor),
not in the ordinary heap/TLS/environment setup solely
because it invokes a memory allocation API.

The native `--startup-research` command now checks every
relevant exact original relative call, `FF15` opcode,
and import-loaded `8B 2D` / `8B 3D` site, asserting
actual original KERNEL32 function identities. The
new synthetic regression test rejects a mismatched
absolute MOV opcode and validates its PE-base conversion.
No Windows code is executed and no original files are written.
### Original operating-system limitation: why native offline analysis matters

The author's **original historical `history.txt`** says under
**version 1.4** that the program no longer worked on
Windows 2000 because the author could not load
`dplayerx.dll` there. This is a **historical compatibility
statement from v1.4**, not an independently observed
test of version 1.5.5 on Windows 2000 or modern Windows.

The owner's exact v1.5.5 executable still contains
a segment-selector-guarded manual DLL entrypoint
invocation at `0x402D76..0x402D9A`, consistent with
that compatibility concern. The old tool loads
the DLL with `LoadLibraryExA(..., 1)`, which normally
does not perform Windows-loader DllMain initialization.

This makes a fully native, **offline reconstruction of the
cryptographic and relocation transformations** preferable
to running the year-2000 unSafeDisc binary, executing
its protected DLL or relying on a historically fragile
Windows 9x/NT startup path.
## Static flag-resolution breaks first opaque DllMain attach branch (2026-10-27)

The owner's reason for porting the original unSafeDisc 1.5.5
workflow natively is its dependency on a legacy Windows 98
environment. A usable Linux/modern Windows replacement should
**not** require executing an old `LoadLibraryExA`/manual DllMain
workaround, a SafeDisc driver or protected retail Windows code.

### First attach CALL now demonstrated as conditionally reachable

Offline symbolic evaluation was applied to the actual, exact
SHA-256 verified Crimson `dplayerx.dll` at `.text` RVA
`0x9787..0x97BE`. The comparison `cmp [ebp-4],1`
sets **ZF=1, PF=1, CF=0, SF=0, OF=0** when invoked
with `DLL_PROCESS_ATTACH=1`. The subsequent reached
instructions are only Jcc, JMP and NOP, so they do not
change these flags before the first CALL.

Following **actual instruction boundaries**, including
intentional overlaps, yields this specific bounded path:

```text
0x9787 (CMP reason,1)
 -> 0x978B (JE taken) -> 0x9792 (JBE taken)
 -> 0x9798 (JE taken) -> 0x97A1 (JBE taken)
 -> 0x9794, 0x9795 (NOP), 0x9796 (JMP)
 -> 0x97A6 (JNP not taken) -> 0x97A8 (JP taken)
 -> 0x97B9 (JO not taken) -> 0x97BB (JNO taken)
 -> 0x97A9 (near JL not taken)
 -> 0x97AF, 0x97B0 (NOP)
 -> 0x97B1 (JA not taken) -> 0x97B3 (JGE taken)
 -> 0x97BE: CALL rel32 -> 0xD62C
```

This is stronger than finding the bytes `E8` at `0x97BE`:
there is a **proved static conditional path** to this
instruction, using actual x86 flags and overlapping
branch targets. An independent short Python instruction
walker reproduced the path on the private original DLL.

**Conditional limitations:** the historical DLL must actually
invoke handler `0x9770` with process-attach reason 1
after its CRT predecessor succeeded. The result does not
prove the historical Windows DLL was run or that the
`0xD62C` function decrypts protected `.txt` code.
More control-flow analysis is required *inside* `0xD62C`.

The native read-only `--attach-research` now computes
and prints the exact reached RVA sequence using a
bounded 32-instruction x86 branch interpreter, which
recognizes only the original CMP and flag-preserving
NOP/Jcc/JMP instructions on that path. A synthetic Rust
unit fixture checks the full 18-instruction sequence and
rejects altered opaque-predicate bytes. The proprietary
binaries remain private and unexecuted.
### Second opaque branch set: `0xD62C` converges to `0xD7BE`

Continuing the first-call path **without executing the DLL**,
the first proven callee `0xD62C` begins with x86 bytes:

```asm
push ebp
mov  ebp,esp
sub  esp,8
push ebx
push esi
push edi
```

For any conventional, valid stack where `sub esp,8` neither
underflows nor yields zero, **CF=ZF=0**. The obfuscated
branches immediately afterward preserve these flags;
PF/SF/OF can vary and are therefore **enumerated rather
than assumed**.

Independent offline evaluation of *all eight* possible
combinations of parity (PF), sign (SF) and overflow (OF)
shows that every reached path ends with exactly:

```text
0xD635 ... flag-preserving opaque branches ...
 -> RVA 0xD663: PUSH EAX
 -> RVA 0xD664: E8 CALL 0xD7BE
```

| PF | SF | OF | First reachable call |
| --- | --- | --- | --- |
| 0/1 | 0/1 | 0/1 | **RVA 0xD664 → 0xD7BE** in every case |

This extends the conditionally verified attach chain:

```text
DllMain RVA 0xFD81
  -> attach handler 0x9770
  -> CMP-flags proven E8 at 0x97BE to 0xD62C
  -> eight-state opcode/flags proof E8 at 0xD664 to 0xD7BE
```

This **does not** prove `0xD7BE` decrypts `.txt`, nor
runtime execution of the DLL. It simply removes two
layers of anti-disassembly conditional jumps.

The Rust read-only `--attach-research` emits the first
bound and now summarizes the eight `0xD62C` paths.
A synthetic regression test builds only the instruction
bytes needed for branch decisions and ensures all eight
cases converge, while failing closed on altered opcodes.
### RET decoy unmasked: the next actual attach callee is `0xC5C0`

Continued analysis of the **original SHA-256-verified**
`dplayerx.dll` beyond the previously established
`0xD664: CALL 0xD7BE` reveals an important correction:

- Original RVA `0xD7BE` contains **only byte `C3` (RET)**.
  The call is therefore an obfuscation/stack roundtrip,
  **not a decrypting function**.
- It returns immediately to RVA `0xD669: POP EAX`.
  The CALL/RET, `PUSH`/`POP`, and the later MOV and Jcc
  instructions **do not alter EFLAGS**.
- With the previously proven normal stack condition
  **CF=ZF=0**, the conditional branch `0xD66A: JA`
  reaches `0xD66E: mov word [ebp-8],1`.
- `0xD674: JBE` is not taken, `0xD676: JA` is taken;
  `0xD687: JS` and `0xD689: JNS` cover either SF state
  and converge to RVA `0xD677`.
- `0xD677: JNP` and `0xD67D: JP` cover either PF state
  and converge to **RVA `0xD68C`**.
- `0xD68C` contains an exact **E8 relative CALL to `0xC5C0`**.

Eight PF/SF/OF combinations give **four distinct instruction
paths, all ending at `0xD68C -> 0xC5C0`**. This was
independently verified by direct Python byte reads of
the exact private original DLL and its signed relative
displacements, and by a separate bounded Rust interpreter.

Therefore, subject to normal stack and successful
previous initialization, the stronger structural chain is:

```text
DllMain 0xFD81 -> attach handler 0x9770
 -> 0x97BE CALL 0xD62C
 -> 0xD664 CALL 0xD7BE (single-byte RET decoy)
 -> 0xD669 resume; preserved flags select reachable Jcc
 -> 0xD68C CALL 0xC5C0 (NEXT NONTRIVIAL CALLEE)
```

**Nothing here establishes that `0xC5C0` decrypts `.txt`.**
That function and its callees require further examination.
The native read-only `--attach-research` asserts every
opcode and expected destination, rejects altered data,
and enumerates all eight paths. A synthetic Rust fixture
tests four different paths converging and rejects a
tampered `RET`. Private executable bytes are not
committed or published.
### Third opaque branch layer: `0xC5C0` reaches `0xE25C`

Following the newly proven call at `0xD68C -> 0xC5C0`,
the **actual** original DLL bytes at `0xC5C0` show
the conventional stack prologue `55 8B EC 83 EC 08 53 56 57`.
With a normal valid stack, `SUB ESP,8` establishes
CF=ZF=0. The process-attach pathway's reached
instructions before its first CALL are only NOP,
`XCHG r,r`, and conditional branches, so the relevant
flags remain stable.

Independently enumerate all combinations of SF and OF:

- `0xC5C9: JG` selects an initial route based on `SF == OF`.
- A route through `0xC5D4: JA` and `0xC5D6: JG` returns
  to the common path at `0xC5CB`.
- The common path at `0xC5CE: JS` (or complementary
  `0xC5D2: JNS`) reaches `0xC5D9` regardless of SF.
- At `0xC5D9`, **PUSH `0x00900508`**, a preferred-base
  virtual address within the original `.data` section.
- At `0xC5DE`, the original signed E8 operand calls
  **RVA `0xE25C`**, not encrypted `.txt` directly.

The four SF/OF combinations agree exactly; their
instruction traces need not match, only their first
reachable call does. All expected opcode bytes,
bounds and targets are checked in the native research CLI.

This adds the conditionally supported static chain:

```text
DllMain -> handler 0x9770 -> D62C
  -> D7BE (RET decoy) -> C5C0
  -> E25C (first call reached within C5C0)
```

**Do not infer decryptor semantics from this call chain.**
Earlier analysis also found direct `.text`/`.txt2` calls
to `0xE25C`. It may be part of initialization or
object construction. The real transformation required
to read the two `.txt` modifier functions at RVAs
`0x65BE` and `0x6621` is still not established.

The Rust `--attach-research` output now shows the
four-case convergence and the verified E8 destination;
the corresponding synthetic test rejects altered
candidate opcodes without embedding private DLL bytes.
## Original E25C disassembled beyond opaque branches: CRT allocation and DF96 (2026-10-27)

Independent offline instruction-boundary tracing of the
SHA-256-verified private original Crimson `dplayerx.dll`
continued from the proven attach-chain callee
`0xC5DE: CALL 0xE25C`.

Unlike the previous D62C/C5C0 case, the prologue
of **E25C does not set EFLAGS**. Therefore its starting
PF/CF/ZF/SF/OF cannot be assumed. Every **32 of 32**
possible Boolean flag states was independently
simulated, accepting only reached, file-backed original
instruction bytes and signed x86 relative branches.

- All 32 flag combinations reach `0xE298: PUSH 0x80`
  (128 bytes), then `0xE29D: CALL 0xF850`.
- The 32 cases produce 32 distinct paths, but all
  converge on that same first **nontrivial** CALL.
- Sixteen of the cases additionally encounter
  `0xE28D: PUSH EAX; 0xE28E: CALL 0xE2FE;`
  where the exact original E2FE byte is `C3` (RET),
  followed by `0xE293: POP EAX`. This is a
  push/call/ret/pop obfuscation detour, not a decryptor.
- The genuine `0xF850` helper reads allocation state
  at preferred VA `0x00905214`, forwards its input
  size to `0xF870`, and the latter uses `0xF8C0`
  as the allocator. This is standard CRT-like
  heap allocation behavior.
- After the allocation call returns, `0xE2A2`
  restores the stack and `0xE2A5` saves the result
  at `[ebp-4]`. Its `ADD ESP,4` can modify EFLAGS,
  so a **separate** 32-state post-call enumeration
  is required. Eight distinct instruction paths
  all converge on `0xE2CC: CMP [ebp-4],0`.
- On the **non-NULL** allocation branch, the code
  pushes the caller's `[ebp+8]` and the newly
  allocated `[ebp-4]` and performs
  **`0xE2DE: CALL 0xDF96`**. The return value is
  masked with `AND EAX,0xFFFF` at `0xE2E6`.

Verified static subchain:

```text
C5DE -> E25C
       -> E28E CALL E2FE (single-byte RET; 16/32 paths)
       -> E298 PUSH 0x80
       -> E29D CALL F850 -> F870 -> F8C0 (CRT allocator)
       -> E2A5 store allocation pointer
       -> E2CC test allocation pointer for NULL
       -> E2DE CALL DF96 (if allocation succeeded)
```

**Interpretation:** this proves that E25C includes
ordinary allocation and processing of a freshly
allocated 128-byte object. The purpose of its
callee `0xDF96` is not yet determined. Nothing in
this finding alone shows any `.txt` decryption,
or recovers the game-specific EAX modifier bytes
at DLL RVAs `0x65BE` and `0x6621`.

The native `--attach-research` now includes the
two 32-state branch checks, confirms the exact
original E8 calls, RET-only decoy, allocator adapter,
NULL pointer branch and the DF96 target. A synthetic
test checks 32 initial, 32 post-allocation cases
(with 16 decoy cases and eight distinct post-alloc
paths), and ensures a replaced fake RET is rejected.
No game binary or Windows program is executed
or published.
### Byte-level transformation found inside `DF96`: a provable XOR primitive at `0xE205`

Continuing the call reached on successful 128-byte allocation,
the SHA-256 verified original `dplayerx.dll` has an
`0xE11E: E8 CALL 0xE205` inside the bounded
DF96 processing pathway.

The E205 entrypoint itself contains overlapping
opaque-predicate instructions, but **all 32 combinations
of initial ZF/CF/PF/SF/OF** reach the same meaningful
arithmetic sequence at `0xE228`. Only four distinct
control-flow paths occur. The original exact code is:

```asm
mov eax, [ebp+8]
and eax, 0xff
mov ecx, [ebp+0xc]
and ecx, 0xff
xor eax, ecx
mov byte ptr [ebp-4], al
```

The XOR result is necessarily in `0..255`, so
**OF=SF=0** after `xor`; the subsequent `JL` at
`0xE23E` cannot be taken. The other reached
branch is `0xE244: JMP 0xE252`, where
`mov al,[ebp-4]` returns the byte value.

The resulting independent mathematical primitive is:

```text
fn e205(a, b): u8 = ((a & 0xFF) XOR (b & 0xFF))
```

A separate offline Python symbolic branch checker
independently confirmed 32/32 flag states converge
before the XOR. The read-only native Rust
`--attach-research` now verifies the actual first
entry bytes, DF96 E11E relative call, all 32
paths, low-byte XOR opcode sequence, and return.
A synthetic Rust test checks four distinct branch
traces, 256 XOR vectors and failure on tampered
opcode bytes.

**Important boundary:** This verifies a genuine
bytewise XOR helper used by the 128-byte DF96
initialization/processing routine, rather than a
mere obfuscation stub. It does **not** yet show
that DF96 operates on encrypted `.txt` section
bytes or establish the separate title-specific
SafeDisc modifier functions at `0x65BE` and `0x6621`.
## Build natively

Requires a standard Rust toolchain on Linux; no third-party crates:

```bash
cargo test --bin unsafedisc-linux
cargo build --release --bin unsafedisc-linux
```

Experimental native **Linux x86_64** and **Windows x86_64** builds are
published as downloadable artifacts by
[GitHub Actions](../.github/workflows/unsafedisc-linux.yml) on this branch.
They are inspect-only research builds, not game patches or unpackers.

On Windows, open the latest successful **Experimental native unSafeDisc CLI
(Linux and Windows)** workflow run, scroll to **Artifacts**,
and download `unsafedisc-windows-x86_64-inspect-only`. Extract the ZIP and
run the executable from PowerShell in the folder containing your original
verified game files:

```powershell
.\unsafedisc-linux.exe inspect `
  --loader '.\CRIMSON(1).EXE' `
  --icd '.\CRIMSON.ICD' `
  --dplayerx '.\dplayerx.dll' `
  --layout
```

For a separate experimental research report, add `--checksum-research` to
the **same** `inspect` command, with or without `--layout`. Example using
the Windows executable already downloaded from GitHub Actions:

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --checksum-research
```

This reports **two different candidate values**, using exactly the same
61,743 relocation exclusions in `.text`, a zero starting accumulator and
the isolated byte-mixer recovered from unSafeDisc 1.5.5:

1. **Page-local model:** the byte-mixer's weight restarts at each relocation-
   bounded span and at every 4-KiB page.
2. **Section-wide model:** the weight restarts only at the HIGHLOW exclusion
   boundaries (without an additional 4-KiB page split).

Neither candidate includes the original unSafeDisc full PE section traversal,
the SafeDisc `dplayerx.dll` modifier or a verified starting checksum state.
The comparison exists **to test competing traversal hypotheses**, not to
claim a final checksum or recover a TEA key. No files are written or decrypted.
Synthetic fixtures demonstrate that the two models may diverge on otherwise
identical input, including a relocation that crosses a page.

PowerShell uses a backtick to continue a command on the next line.
This action only reads files; the original EXE, ICD and DLL are never modified.

## Inspect your own original files

Keep the protected SafeDisc `CRIMSON.EXE`, `CRIMSON.ICD` and
`dplayerx.dll` separately from any patched or unwrapped copies.

```bash
./target/release/unsafedisc-linux inspect \
  --loader './CRIMSON(1).EXE' \
  --icd './CRIMSON.ICD' \
  --dplayerx './dplayerx.dll'
```

To report the **geometry only** of the `.text` relocation coverage (no game
checksum is computed), add the optional `--layout` flag:

```bash
./target/release/unsafedisc-linux inspect \
  --loader './CRIMSON(1).EXE' \
  --icd './CRIMSON.ICD' \
  --dplayerx './dplayerx.dll' \
  --layout
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

### Inspect section characteristics before choosing the checksum traversal

The original SafeDisc tool's exact PE **section selection** has not been
recovered yet. A checksum of only \`.text\` may omit sections that the real
tool processed. Use the optional native, strictly read-only \`--sections\`
flag to print all PE32 section headers and relocation-start coverage:

\`\`\`powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --sections
\`\`\`

The report uses the official PE section-header fields: \`VirtualSize\`
(header offset +8), \`VirtualAddress\` (+12), \`SizeOfRawData\` (+16),
\`PointerToRawData\` (+20) and \`Characteristics\` (+36). It shows
type flags **C/I/B** (code/initialized/uninitialized data), memory flags
**X/R/W** (executable/readable/writable), each section's RVA,
virtual size, raw file span, and the count of \`HIGHLOW\` DWORDs whose
**start RVA** lies inside that raw section span.

The result is **metadata only**, not a recovered SafeDisc filter:
relocations can span raw section boundaries and uninitialized memory may
have no file data. File section flags do not, by themselves, establish
whether unSafeDisc selected or excluded a section. No new checksum or
TEA key is asserted by \`--sections\`.

### Historical checksum selection and extent recovered (2026-10-27)

A static, **non-executing** analysis of the publicly archived
[unSafeDisc 1.5.5 variant](https://github.com/nckstwrt/SafeDiscLoader/tree/main/UnSafeDisc%20and%20SafeDisc2Cleaner)
(Git blob \`825c467b6b403e1c85a5875ede5696dedfe17652\`)
revealed concrete limits that the earlier two full-raw research
models did not apply. This historical binary was modified by its
archiver to load a compatibility \`version.dll\`; the core code
positions below agree with the prior inspection of unSafeDisc 1.5.5.

- \`0x401CDE..\`: reset the per-run checksum accumulator at
  \`[0x60BCB0]\` to zero.
- \`0x401D11..0x401D19\`: read PE section
  \`IMAGE_SECTION_HEADER.Characteristics\` from offset \`+0x24\` and
  save it to \`[0x60BCD0]\`.
- \`0x401D1A..0x401D34\`: begin with
  \`SizeOfRawData\` (\`+0x10\`), then cap by \`VirtualSize\` (\`+0x08\`)
  when that size is nonzero and smaller.
- \`0x401F01..\`: test \`[0x60BCD0] & 0x20\`. Only sections
  marked \`IMAGE_SCN_CNT_CODE\` enter the checksum's byte mixer
  at \`0x401F0D\`. The path at \`0x401F43\` conditionally calls the
  still-unrecovered DLL modifiers (DLL size \`0x21E00\`).
- \`0x401DCA..0x401EDE\`: process relocation boundaries and
  4-KiB page increments. A faithful treatment of the complete
  page/relocation boundary control flow remains to be confirmed.

For the exact verified Crimson Skies 1.0.2.0 ICD, \`.text\` is
the **only** section with bit \`0x20\`. Its
\`SizeOfRawData = 2,105,344\`, but its
\`VirtualSize = 2,102,461\`; this means the earlier models included
**2,883 raw-padding bytes that the historical size gate would exclude**.
The \`.data\` virtual size (4,215,252) is larger than its backed raw
size (172,032), so this historical routine would *not* treat
virtual-only \`.data\` bytes as file input.

The native Rust \`inspect --checksum-research\` command now reports
**two historical-extent-bounded branch candidates**, using
only 2,102,461 backed bytes from \`.text\`, while retaining the two
earlier full-raw values as explicit comparators. The relocation-aware bounded helper
validates relocation windows over the whole backed section and then
clips them to the effective bytes, including a partial final 4-KiB
page and a HIGHLOW DWORD that may straddle that cutoff.

**Important:** the zero initial accumulator, CODE flag gate and
VirtualSize limit are now supported by instruction-level evidence,
but the *precise original relocation/page walk* and both DLL
modifiers remain unresolved. The bounded output is therefore still
an **experimental candidate, not the recovered SafeDisc checksum**.
No original game data or external binary is committed.

### Static branch-selector path located in unSafeDisc 1.5.5

Follow-up static disassembly of archived unSafeDisc 1.5.5 establishes
an initialization path *before* checksum computation:

- `0x402505`: copy the protected PE32 relocation data-directory
  RVA from NT headers offset `+0xA0` into runtime `[0x403B2B]`.
- `0x401FCF`: unconditionally invoke `0x402257`, which normalizes
  section names and searches for the protected relocation-directory RVA.
- `0x40230B..0x402365`: scan PE sections, checking file backing,
  raw RVA containment and a guard DWORD at the chosen header's `+0x34`.
- `0x402366..0x402373`: on success, set runtime
  `[0x403CAC] = 4`. The checksum dispatch at `0x401D48`
  therefore selects the relocation-aware path when this setup succeeds.

The opt-in `--checksum-research` command now evaluates these **static
directory/section guard conditions** against the exact hash-verified
Crimson Skies ICD. On a matching section it prints its RVA,
directory size and relevant guard DWORD, then reports the
**expected selector assignment to 4**. If any guard fails,
the report is **INCONCLUSIVE**, not a guessed value.

This is stronger evidence for the relocation-aware candidate
`0x1B09633B`, but not a dynamic observation of the original
Windows tool's runtime state, nor an independent validation of the
checksum's result against a historically reported checksum.
Both DLL modifier routines and full TEA key derivation remain unresolved.
The Rust inspection never executes the historical tool or its DLL.

### Investigate the two game-specific `dplayerx.dll` modifiers without loading the DLL

In SafeDisc 1.50.20 the extra decoding layer calls two
**title-specific** EAX modifier functions, at relative virtual
addresses `0x65BE` and `0x6621` (first + `0x63`).
The historical SafeDisc 1.5.5 decoder and the
[June 2001 technical analysis](https://gbppr.net/cracking/krobar/other/com26.html)
agree on these **routine RVAs**, but their bytes are
**not necessarily the same in other games' dplayerx.dll builds**.

Use the native **read-only** flag on the exact SHA-256 verified
Crimson Skies `dplayerx.dll`:

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --modifier-research
```

The report reads the **original DLL PE section headers** (RVA,
file offset, raw size, VirtualSize and Characteristics), and prints
the DLL's PE32 **entry RVA** with its first **96 file-backed bytes**.
It also maps each of the two historical modifier RVAs to its
actual backed file offset, requiring an executable section, and
prints **128 bytes per RVA**, grouped into 16-byte rows.

The original user's verified Crimson Skies DLL showed these
on-disk bytes at the two modifier offsets:

- `0x65BE`: `E5 FF CA 93 02 81...`
- `0x6621`: `DE E6 24 61 17 0C...`

Disassembling the first bytes directly as 32-bit x86 starts with
`in eax, 0xff`, followed by `retf 0x293`: these are highly
implausible entry instructions for a normal user-mode EAX transformer.
The excerpt also contains a privileged `hlt` and I/O instructions.
This suggests the modifier code in the file is **not ready to
execute**. Possible explanations include DLL-internal encryption
or a wrong interpretation of on-disk versus loaded RVAs; neither
hypothesis has been established as fact for this exact title.

Contemporary SafeDisc discussions describe DLL initialization
that decodes protected code. We must now analyze the **actual
DLL startup stub** rather than blindly implement these bytes as
two real modifier functions.

This is bounded *static data*, **not execution**, dynamic loading,
decryption, a complete disassembly or a replacement DLL.
No file is created or modified.

#### What we learned from the original DLL entrypoint

The user-supplied 96-byte entrypoint excerpt at DLL RVA `0xFD20`
is intelligible x86 and includes a conventional `ret 0xC`.
We independently disassembled it as an x86 DLL startup wrapper:

- At `0xFD23`, it reads the 3-argument DllMain `reason` from the
  stack and checks for `1` (process attach) and `2` (thread attach)
  before calling its startup handlers.
- At `0xFD31` and `0xFD56`, it references data/pointer values at
  absolute virtual addresses `0x90519C` and `0x906C70`.
  The user's **actual** DLL ImageBase is `0x008E0000`: these
  are RVAs **`0x2519C` and `0x26C70`**, both inside the
  **virtual-only, loader-zero-filled** tail of the `.data` section.
  No bytes for these variables exist on disk; they may be assigned
  during initialization. An earlier guessed base `0x900000`
  incorrectly placed them in `.txt` and is superseded here.
- At `0xFD6C`, a relative call with bytes
  `E8 8F FE FF FF` points to **DLL RVA `0xFC00`**, another
  on-disk executable `.text` location.
- Startup continues at `0xFD7E`; only the first two bytes
  of this path were present in the initial 96-byte excerpt.

The user's SHA-256-verified original DLL explicitly reports **no PE
TLS directory**. Therefore there are no PE-declared TLS callbacks to
follow for this image. We focus on DllMain/CRT initialization and
its other calls. No particular startup routine has yet been shown
to decrypt `.txt` or `.txt2`.

The enhanced `--modifier-research` output additionally reports:

- DLL preferred image base (read from PE32 OptionalHeader);
- PE TLS directory metadata and up to eight bounded callback pointers,
  with short raw x86 excerpts when available; Windows calls TLS callbacks
  **before** the DLL entrypoint;
- **160 bytes at RVA `0xFC00`**, the actual relative-call target;
- **96 bytes at RVA `0xFD7E`**, the rest of the startup branch;
- the location of VAs `0x90519C` and `0x906C70`, including whether
  each is file-backed or is only present in the mapped image.

The output never loads the DLL and refuses malformed RVA/VA pointer
ranges or unterminated callback lists. Raw data is never assumed to
equal the post-load memory image.

The **DLL startup excerpt** is the next input for independent
x86 disassembly: identify any initialization/decryption stage and
map runtime code before attempting to interpret either modifier.
Do not assume the raw modifier windows are already decoded x86.
Until that is confirmed, even the four reconstructed
conditional TEA words must **not** be used to assert success.

#### Trace the original DLL startup calls without loading it

The previous user's native run confirmed original DLL ImageBase
**`0x008E0000`**, no PE TLS directory, and original `.data`:

- Section RVA `0x20000`, `SizeOfRawData = 15,872`,
  `VirtualSize = 27,776`.
- Backed RVA span `[0x20000, 0x23E00)`; virtual-zero-fill tail
  `[0x23E00, 0x26C80)`.
- Address `0x90519C` maps to RVA `0x2519C`, and
  `0x906C70` maps to RVA `0x26C70`: both in virtual-zero-fill.

The readable DLL `0xFC00` helper resembles conventional DLL
runtime initialization and calls many other routines, but the
present evidence does **not** identify it as the `.txt` decoder.

The opt-in read-only **`--startup-research`** flag checks the
actual original bytes at ten known `E8 rel32` CALL instruction
locations in the DllMain and `0xFC00` paths, computes signed
relative destination RVAs, validates that each has at least
64 original file-backed executable bytes, and prints a 64-byte
excerpt for each. It also distinguishes backed data from
loader-zero-filled `.data` metadata and reports TLS presence.

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --startup-research
```

These are **validated direct-call targets only**. This does not
build a complete control-flow graph or prove that any target
changes protected memory; dynamic calls through IAT or function
pointers are not followed. No decryption, loading, execution or
modification is performed.
#### Identify the actual Windows API imports behind startup helper calls

The user supplied the native `--startup-research` result from the
original, exact SHA-256-verified Crimson Skies 1.50.20 DLL.
Confirmed ten direct E8 call destinations:

| From RVA | To RVA |
| --- | --- |
| `0xFD81` | `0x9770` |
| `0xFC18` | `0x116E0` |
| `0xFC53` | `0x10820` |
| `0xFC5C` | `0x11720` |
| `0xFC71` | `0x12FF0` |
| `0xFC89` | `0x129E0` |
| `0xFC8E` | `0x126A0` |
| `0xFC93` | `0x12D40` |
| `0xFC98` | `0x12C50` |
| `0xFC9D` | `0xF920` |

The raw x86 code at `0x116E0` creates/initializes a resource
using an imported function at VA `0x8FE020`, and stores its
return value at VA `0x906C68`. The complementary routine
`0x11720` traverses its objects, makes a three-argument
indirect call via VA `0x8FE0A4`, and invokes an imported
function at VA `0x8FE0A0` during cleanup. These patterns look
like a heap/resource-management subsystem, not by themselves
a title-specific SafeDisc decoder.

To establish the actual API names, native
`--startup-research` now parses original PE32 import descriptors
and import-by-name / ordinal thunks with strict bounds. It
matches ten IAT virtual addresses used by the readable
entry/startup code, printing `library!symbol` when found;
it never calls the API or loads the DLL. Signed relative
targets are still independently validated.

**Important:** the original DLL's `.txt` modifier bytes
at RVA `0x65BE` and `0x6621` remain unreadable as normal
x86 on disk. The call map and named startup imports
cannot, alone, prove that any particular routine decodes
that section. A stronger test requires finding the actual
byte-transform/memory-write loop and validating it against
the original memory image or independent plaintext.
#### Confirmed startup API bindings and process-attach research

The user tested the exact SHA-256 verified Crimson Skies `dplayerx.dll`
using native `--startup-research`. The PE import parser resolved
**69 import bindings** and mapped the following startup IAT VAs:

| IAT VA | Imported function |
| --- | --- |
| `0x8FE020` | `KERNEL32.dll!HeapCreate` |
| `0x8FE0A0` | `KERNEL32.dll!HeapDestroy` |
| `0x8FE0A4` | `KERNEL32.dll!VirtualFree` |
| `0x8FE090` | `KERNEL32.dll!TlsAlloc` |
| `0x8FE08C` | `KERNEL32.dll!TlsSetValue` |
| `0x8FE054` | `KERNEL32.dll!GetModuleFileNameA` |
| `0x8FE078` | `KERNEL32.dll!GetVersion` |
| `0x8FE074` | `KERNEL32.dll!GetCommandLineA` |
| `0x8FE0E0` | `KERNEL32.dll!GetEnvironmentStrings` |
| `0x8FE0E4` | `KERNEL32.dll!GetEnvironmentStringsW` |

These confirm the roles of several previously unknown CRT/runtime
startup calls (heap management, TLS allocation, command-line and
environment setup). They do **not** identify the proprietary
SafeDisc code-section decoder.

The more unusual helper at **DLL RVA `0x9770`** receives the
DllMain `reason` value and contains irregular control flow.
Direct signed relative x86 branch bytes already supplied by the
user show **two paths converge on DLL RVA `0x98F5`**:

- `0x9781`: `0F 84 6E 01 00 00`, JZ to RVA `0x98F5`
  when `reason == 0`.
- `0x978D`: `E9 63 01 00 00`, JMP to RVA `0x98F5`
  when `reason` is neither `0` nor `1`.
- Process attach (`reason == 1`) enters the irregular instruction
  region near `0x9792`, which may contain opaque control flow.

A further read-only extension to **`--startup-research`**
now verifies both actual instruction encodings, checks that the
computed branch targets converge and map to backed executable code,
then prints **256 bytes at `0x9770`** and **192 bytes at
`0x98F5`**. It also lists any imported APIs whose names indicate
code-memory protection, dynamic symbol resolution or code loading
(e.g. `VirtualProtect`, `VirtualAlloc`, `GetProcAddress`).

**No such API's absence proves no decoder exists:** a loader may
resolve symbols dynamically, call through unrecognised pointers,
or modify its own mapped pages through other mechanisms.
The data displayed remain static on-disk excerpts only;
no protected DLL code is executed or patched.
#### Overlapping process-attach CALL candidates (read-only)

The latest user run extracted 256 original, hash-verified on-disk bytes
starting at DllMain attach helper `0x9770`. This region contains
overlapping instruction decodes. In particular, bytes at RVA `0x9850`:

```asm
0x9850: 2B FF                   sub edi, edi
0x9852: 15 04 E0 8F 00          adc eax, 0x008FE004

; alternate instruction boundary
0x9851: FF 15 04 E0 8F 00       call dword ptr [0x008FE004]
```

Thus a naive linear x86 disassembler can miss a hidden IAT call.
Whether original control flow **ever reaches `0x9851`** is still
unproven; the bytes alone are insufficient to assert execution.

The user-provided region also contains E8 rel32 encodings at
these original RVA sites:

| CALL RVA | Computed target RVA | Original section |
| --- | --- | --- |
| `0x97BE` | `0xD62C` | `.text` |
| `0x97EE` | `0xA550` | `.text` |
| `0x9821` | `0x1A20D` | `.txt2` |
| `0x9861` | `0xE25C` | `.text` |

The `0x1A20D` target's location in executable `.txt2` is
especially worth inspecting, but it is not proof that `.txt2`
executes or that `.txt` is decrypted. Some E8 encodings can
themselves be part of overlapping dead bytes.

Native `inspect --attach-research` now validates **four E8
and two FF15 opcode encodings** in the exact original DLL,
calculates direct targets via signed rel32, requires target
bytes to be in a backed executable PE section, shows 128
bytes per destination, and matches indirect IAT VAs to the
same independently parsed original PE import table.

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --attach-research
```

Both `FF15` candidates are at `0x9851` and `0x986A`.
The tool deliberately does **not** assert that either is a
reachable instruction boundary, nor that any target implements
the SafeDisc title-specific DLL EAX modifiers. Protected
code is never executed, patched, decrypted or copied into
the Git repository.
#### Verified cross-links between `.txt2` and `.text`

From the user's `--attach-research` run with the original,
SHA-256-verified Crimson Skies SafeDisc 1.50.20 DLL, a
128-byte static excerpt at `.txt2` RVA `0x1A20D` contains
two direct E8 CALLs and two indirect FF15 CALL-shaped
byte sequences. Crucially, the direct destinations are
shared with previously discovered `.text` routines:

| Instruction candidate | Static destination |
| --- | --- |
| `.txt2` `0x1A23A` FF15 | `KERNEL32!GetCurrentProcess` IAT |
| `.txt2` `0x1A247` E8 | `.text` `0xE25C` |
| `.txt2` `0x1A250` FF15 | `KERNEL32!GetModuleHandleA` IAT |
| `.txt2` `0x1A25F` E8 | `.text` `0xA550` |

Static disassembly of the original 12-byte `.text`
helper at RVA `0xA550`:

```asm
mov ecx, [esp+4]
mov eax, [esp+8]
mov [ecx+8], eax
ret
```

This helper only stores the second argument into the
first argument's structure at `+8`. In the `.txt2` excerpt
it receives immediate `0x67`, so it appears to store a
type/tag field or state value; **it is not a proven decoder**.

A related `.text` routine at RVA `0xA560` references
IAT VA `0x8FE044` and `0x8FE040`, in a four-argument
file-position-call / five-argument file-read-call pattern.
These names were *not* previously resolved in the output.
The enhanced `--attach-research` therefore checks the
exact `FF15` instruction bytes at `0xA578` and
`0xA597` and resolves both names against the original
PE import descriptors. It also checks the two shared
`.txt2` calls and asserts the original 12 bytes at `0xA550`.

**Important:** overlapping jump/instruction boundaries in
both `.text` and `.txt2` still make all of these instruction
decodes **static candidates**, not proof of runtime reachability.
Nothing has established a byte-transforming loop for `.txt`
or actual decrypted EAX modifier code at `0x65BE`/`0x6621`.
#### Relocation-backed references to protected DLL `.txt`

The user's latest `--attach-research` confirms that the
`.text` `0xA560` routine calls `KERNEL32!SetFilePointer`
at `0xA578` and `KERNEL32!ReadFile` at `0xA597`.
Those exact import names strongly establish a file-position/read
helper rather than a demonstrated `.txt` decryptor. The short
`0xA550` helper only stores a DWORD in `[ecx+8]`.

The native inspector now offers a more specific way to look
for the actual protected code initialization path:

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --xref-research
```

This read-only diagnostic parses the original, hash-verified
`dplayerx.dll` relocation directory (PE base-relocation type
`HIGHLOW=3`) and examines only relocated **four-byte words
in other file-backed executable sections**. It converts each
absolute VA into a target RVA with the DLL's actual ImageBase
`0x008E0000` and reports references into protected `.txt`.
It highlights exact matches or nearby targets relative to the
known original modifier RVAs `0x65BE` and `0x6621`.

Unlike scanning for arbitrary 4-byte patterns, requiring a
real PE HIGHLOW relocation substantially reduces accidental
matches in obfuscated x86, while preserving RVA evidence from
`.text` and `.txt2`. It prints only up to 24 examples per
source section and counts any additional matches.

**Limitations:** A relocation-backed pointer can be data rather
than a reachable instruction. No match does **not** imply that
the DLL never references `.txt`: runtime-computed address
arithmetic, relative CALL/JMP and non-relocated pointers are
not covered. This diagnostic does not resolve protected
code, run SafeDisc, or change original input files.
#### No executable-section relocation pointers into protected `.txt` (user-confirmed)

The user ran `--xref-research` against their exact, SHA-256 verified
Crimson Skies SafeDisc 1.50.20 DLL and obtained:

```text
33 PE base-relocation blocks; 1694 HIGHLOW entries; 16 ABSOLUTE padding entries.
1198 HIGHLOW DWORDs in other file-backed executable sections.
0 relocation-backed references into .txt from those executable sections.
0 exact references to protected modifier RVAs 0x65BE and 0x6621.
```

**Important distinction:** no matching *relocation-backed pointers*
from code is not evidence that `.txt` cannot be accessed or decrypted.
Any RVA computed arithmetically, stored as a non-relocated offset,
reached by relative control flow, or referenced via a pointer table
in `.data` / `.rdata` would have been missed.

The new `--xref-research` runs **both** the original executable-only
analysis and a complementary analysis of **all other backed
sections**, including `.data` and `.rdata` (but excluding
protected `.txt` itself, whose on-disk bytes appear encoded).
The CLI retains the old code-only totals, then adds total backed
HIGHLOW source DWORDs, relocation-backed pointers into `.txt`
from all sources, per-section first 24 entries, and exact modifier
matches, always deriving addresses from the real PE ImageBase.

Original DLL binary is not executed, loaded, or modified;
the expanded counts have not yet been observed against the
user's local DLL in this research step.
#### No `.txt` references in *any* other file-backed HIGHLOW word

The user ran the expanded `--xref-research` against the exact
original Crimson Skies SafeDisc 1.50.20 `dplayerx.dll`:

```text
DLL relocation blocks: 33
HIGHLOW relocations: 1694; ABSOLUTE padding: 16
File-backed HIGHLOW source DWORDs from other executable sections: 1198
File-backed HIGHLOW source DWORDs from all other sections: 1480
Pointers to .txt from executable sections: 0
Pointers to .txt from ALL other backed sections: 0
Exact pointers to modifiers 0x65BE and 0x6621: 0
```

Thus **282 additional file-backed HIGHLOW words** in non-code
sections provided no pointers to the original protected code.
This rules out one specific on-disk address-referencing mechanism
for the original DLL, but not non-relocated literals or runtime
computed targets.

New **`--literal-research`** searches the exact verified original
DLL for non-relocated four-byte **little-endian candidates** in
`.text`, `.txt2`, `.rdata` and `.data` (excluding protected
`.txt` and `.reloc`). It checks every byte offset, including
unaligned patterns that may occur in overlapping x86 code,
and excludes every four-byte window overlapping any original
HIGHLOW relocated DWORD. Reported categories per section:

- An exact little-endian DWORD encoding an RVA **within**
  protected `.txt` (`0x1000` through `0x8670`).
- An absolute virtual-address DWORD in the corresponding
  preferred-image-base range (`0x008E1000` through `0x008E8670`).
- Exact markers for the start/end, VirtualSize/SizeOfRawData
  of `.txt`, and the two EAX modifier RVA/VA values.

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --literal-research
```

Results are counts and at most 16 raw RVA/value pairs per
category and source section, not a fully disassembled or verified
control-flow graph. Both negative results and positive matches
remain **inconclusive** as to whether code can reach these
addresses at runtime. A file-backed DWORD matching a RVA may
be an ordinary integer, an obfuscated overlapping instruction,
a pointer-table entry, or a real address operand.

Inputs are hash-verified, read-only; no DLL is loaded, decrypted
or written.
#### Literal scan results: 779 overlapping DWORD candidates but no VA-range matches

The user's most recent SHA-256 verified Crimson Skies DLL
`--literal-research` run reported:

| Source section | Unrelocated 4-byte window values in `.txt` RVA range |
| --- | ---: |
| `.text` | 372 |
| `.txt2` | 52 |
| `.rdata` | 160 |
| `.data` | 195 |
| **Total** | **779** |

There were **zero** non-relocated four-byte values in the
preferred-base `.txt` *VA* range and **28 exact special
markers** (19 `.text`, 4 `.txt2`, 5 `.data`).

Almost all of these can be **incidental little-endian
patterns**: the scanner includes byte offsets *inside* x86
instructions, ASCII metadata and ordinary small integer
constants. Do not interpret the 779 as cross-references.

Two prominent markers warrant contextual inspection:

- `0x1000` = 4096, a ubiquitous memory granularity and
  also the `HeapCreate` initial size in the previously
  identified startup call at RVA `0x116E3`.
- `0x7800` = 30,720, exactly the original `.txt` section
  **SizeOfRawData**, observed at `.data` source RVAs
  `0x22FFF` and `0x230CF`. This match alone is not a
  demonstrated pointer or code-unpacking loop.

The **original output truncated the 19 exact markers
in `.text` after 16 lines**, so the absence of exact
modifier RVA words **cannot yet be concluded from the
reported lines**. To resolve that ambiguity, the revised
`--literal-research` now prints *all* exact markers
(up to 64 per section), separately counts exact `0x65BE`
and `0x6621` *RVA* literals and their ImageBase-adjusted
virtual addresses, and counts exact `0x7800` markers.

It also prints tightly bounded, verified read-only byte
context around ten known examples: `.text` at `0x9410`,
`0x9427`, `0x116E3` (HeapCreate control), `.txt2` at
`0x1921A`, `0x19230`, `0x19287`, `0x192A1`, and `.data`
at `0x22FFF`, `0x230CF`, `0x23148`.
Each known original DWORD is rechecked before reading
the 16 bytes preceding and 28 after its start, and
the output explicitly avoids claiming that the DWORD
corresponds to an instruction boundary.

Even if all these constants are mundane sizes and no
other reference is found, a **runtime-computed address**
or on-load code transformation cannot be excluded.
### Conditional four-DWORD TEA key reconstruction (experimental)

A contemporary SafeDisc reverse-engineering essay by ArthaXerXès
([June 2001](https://gbppr.net/cracking/krobar/other/com26.html))
specifies the original 1.45+ full-key formula. It matches the
historical unSafeDisc 1.5.5 x86 implementation at `0x401CA2..0x401CDD`:

```text
adjust = (original_icd_entry_rva + 0x2D8) mod 2^32
mix = ((first_word - adjust) XOR checksum_candidate) XOR raw[0]
word[0] = first_word
word[1] = ((mix XOR raw[1]) + adjust) mod 2^32
word[2] = ((mix XOR raw[2]) + adjust) mod 2^32
word[3] = ((mix XOR raw[3]) + adjust) mod 2^32
```

An explicit opt-in **`--key-research`** flag now prints the resulting
four **conditional candidate** DWORDs on the exact-hash-verified
protected files. It requires a user-provided first word
(`--reported-key 0x7496538F`) and succeeds only when the historical
relocation-selector PE guards are established and both independent
relocation-aware checksum calculations agree. The optional
`--reference TESTME.EXE` must be an exact verified **unmodified**
reference and allows the reported first word to be corroborated by
import DWORD XOR matches. Without the reference, the program
prints an explicit **NOT independently checked** warning.

```powershell
.\unsafedisc-linux.exe inspect --loader '.\CRIMSON.EXE' --icd '.\CRIMSON.ICD' --dplayerx '.\dplayerx.dll' --reported-key 0x7496538F --key-research
```

With the user's observed relocation-aware checksum candidate
`0x1B09633B`, the reported first word `0x7496538F`, raw key
`E51DC3D7 63CA5EAC 4493379C 08B42B8C` and
original ICD entry RVA `0x001F7056`, this predicts these
**NOT independently decrypted or validated** TEA words:

| Word | Hypothesis |
| --- | --- |
| 0 | `0x7496538F` (explicit reported input) |
| 1 | `0xE9C7914F` |
| 2 | `0xCF10EA3F` |
| 3 | `0x82F5DE2F` |

The entry RVA adjustment in this candidate is `0x001F732E`.
These are *conditional arithmetic results*, not a proven
working TEA key. Neither the title-specific DLL modifier routines
at RVAs `0x65BE`/`0x6621` nor a full plaintext image comparison
has been performed. `unpack` remains unavailable, and no file
is modified or generated. Never claim decryption based on this report.
### Both historical checksum branches implemented as candidate calculations

Historical unSafeDisc 1.5.5 x86 code at `0x401D43..0x401D4E` is:

```asm
mov esi, 0x403CB0
cmp dword ptr [esi-4], 4   ; [0x403CAC]
je 0x401D82               ; relocation-aware branch
; otherwise: ordinary consecutive 4-KiB pages
```

The simple branch is visible at `0x401D4E..0x401D77`: divide
the historically selected section extent by `0x1000`, call the
checksum byte mixer once for each 4-KiB page and once for the
nonempty remainder. **It does not omit relocated DWORDs.**
The other branch at `0x401D82` uses relocation metadata.

The `--checksum-research` CLI now prints **two extent-bounded,
historically branch-specific candidate values** for the user's exact
original `.text` section: (a) simple pages including all raw backed
code bytes up to `VirtualSize`, and (b) relocation-aware chunks omitting
HIGHLOW DWORD bytes. Both start with accumulator zero.
The independently implemented relocation walker still verifies (b).

**The value of the historical runtime selector `[0x403CAC]`
has not been dynamically observed in the original Crimson Skies run.**
However, the newly recovered initialization path provides a conditional
static assignment to `4` when the PE relocation-directory guards match.
The archived
tool's on-disk DATA initial value cannot resolve this alone:
the variable is used in a runtime-populated table. Neither
candidate is asserted as the actual SafeDisc checksum or TEA key.

### Independent verification of relocation-aware page traversal

User-run `--checksum-research` with the exact original protected
Crimson Skies 1.0.2.0 files produced an **extent-bounded candidate**
of `0x1B09633B` when processing the 2,102,461 selected bytes in
the CODE section `.text`. This value remains provisional.

An additional Rust routine now independently walks sorted relocation
intervals **page by page**, recording each nonempty byte-mixer call.
It explicitly carries the excluded bytes of a HIGHLOW DWORD that
begins at page offset `0xFFD`, `0xFFE` or `0xFFF` into the next
4-KiB page, including when the final page is shorter than 4 KiB.

The inspector checks that its candidate checksum agrees with the
previous page-local implementation and that retained plus excluded
bytes exactly equal the historical effective section length.
It reports effective retained/skipped byte counts, nonempty
fragment calls and the number of page-crossing relocations.
The diagnostic **fails** if the independent walks disagree.
Tests cover no relocations, missing relocation blocks, multiple
page crossings and partial final pages.

That check is a **cross-validation of two native code paths**,
not proof that the original unSafeDisc program selected its
relocation-aware branch. Its historical disassembly at
`0x401D48` explicitly selects between the straightforward
4-KiB path and the relocation-aware path based on a runtime DWORD
at `0x403CAC`; the corresponding runtime value for Crimson Skies
has not yet been established. The original DLL transformations
also remain unrecovered.
### Checksum-dependent key word staging (historical disassembly)

Static x86 instructions at `0x401CA2..0x401CDD` of unSafeDisc 1.5.5
derive **three** 32-bit words after running its checksum procedure:

```text
base = (dword[0x403C50] + 0x2D8) modulo 2^32
common = ((dword[0x403C30] - base) XOR dword[0x403C40]) modulo 2^32
for i from 2 down to 0:
    dword[0x403C34 + 4*i] =
        ((dword[0x403C44 + 4*i] XOR common XOR checksum) + base) modulo 2^32
```

The Rust function `derive_three_checksum_dependent_words` ports exactly
this arithmetic into a **pure, synthetic-only adapter**, with known
vectors and integer-overflow tests. It is **not called on the game files**.
The relationship of the runtime globals at `0x403C30/0x403C40/
0x403C44/0x403C50` to the four DWORDs selected at loader file offset
`0x28FB8` has not been confirmed. The final full 128-bit TEA key cannot
yet be asserted, even with the candidate checksum `0x1B09633B`.
### Observed Windows baseline (2026-10-27)

The user ran the exact-hash-verified Crimson Skies 1.0.2.0 inputs on the
native Windows x86_64 inspector. The two current **hypothesis values**
were:

| Model | Observed experimental value |
| --- | --- |
| 4-KiB page-local weight resets | `0x340E8945` |
| Section-wide relocation-delimited chunks | `0xBA86F13A` |

The input `.text` geometry was: 2,105,344 raw bytes, 514 full
4-KiB pages, 61,743 HIGHLOW relocations, 246,972 excluded bytes,
1,858,372 included bytes, 59,622 hypothetical nonempty page-local
spans and 36 relocated DWORDs crossing page boundaries.

These values are **reproducible observations of two different candidate
programs with the full raw `.text` section**, not proof that either model
matches the original unSafeDisc checksum. They are not a TEA key.
Subsequent historical disassembly identified a `0x20` CODE-section
filter (only `.text` qualifies for this ICD) and a nonzero
`VirtualSize` limit, so **neither older full-raw value is the
historically extent-bounded result**. The DLL modifiers and
the full page/relocation walk are still not verified.

To inspect where the two models diverge, the latest
`--checksum-research` mode also attributes weighted contributions
to their originating 4-KiB pages. It prints the total number of pages
with differing contributions and **at most eight** of the earliest
differing page RVAs with each page's model contribution and cumulative
difference. This trace is intentionally compact: no game-code bytes,
memory dumps or decrypted content are printed. The sum of each model's
page contributions must equal the value obtained by the original
isolated model or inspection fails. Synthetic tests cover both totals
and excluded-byte invariance.

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
| PE32 relocation directory and 4-KiB page parsing | Implemented; strict exact-file checks |
| Per-page relocation-skipping checksum helper | Implemented, synthetic only; rejects cross-page DWORDs |
| Joined-page relocation-skipping checksum primitive | Implemented, synthetic only; handles straddling DWORDs, not the full SafeDisc traversal |
| Optional `inspect --layout` geometry report | Implemented; reads verified original `.text` relocation metadata only, **no full checksum** |
| Optional `inspect --checksum-research` report | Three research candidates: two historical **full-raw** comparators plus the **historically extent-bounded** `.text` page-local model; none asserted as a verified SafeDisc checksum |
| Optional `inspect --sections` section header report | Implemented; raw PE32 flags and relocation starts, **no inference of original section selection** |
| Page-oriented pre-TEA modifier adapter | Implemented with synthetic modifiers; resets word index every 4-KiB page |
| Exact per-DWORD pre-TEA stage call sequence | Implemented with **synthetic callback functions**, not real DLL algorithms |
| Full checksum traversal (PE flags, cross-page relocations, DLL modifiers) | Not implemented |
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

## PE relocations and SafeDisc's extra pre-TEA stage

The exact original Crimson Skies `CRIMSON.ICD` contains an intact PE32
**base relocation directory**. The native inspector now verifies its
directory and reports these measurements from the exact hash-verified
original (the unmodified `TESTME.EXE` must have equivalent relocation
entries):

| Quantity | Verified |
| --- | ---: |
| Base relocation blocks | **558** |
| `IMAGE_REL_BASED_HIGHLOW` (3) entries | **71,114** |
| `IMAGE_REL_BASED_ABSOLUTE` (0) padding entries | **280** |
| Relocation blocks in executable `.text` | **514** |
| `.text` HIGHLOW entries | **61,743** |
| HIGHLOW entries **crossing** a 4-KiB boundary in `.text` | **36** |

A HIGHLOW DWORD at a page offset of `0xFFD`, `0xFFE`, or `0xFFF`
overlaps the following page. **These 36 entries are valid**: the relocation
parser accepts them. The *isolated single-page* checksum helper still
refuses a relocation spanning two pages. The additional
`checksum_pages_without_highlow` helper takes **contiguous 4-KiB pages**
and skips every byte of straddling DWORDs. It preserves the accumulated
checksum between pages, but resets the mixer's weight for each relocation-
delimited span and at each page boundary. Tests cover offsets `0xFFD`,
`0xFFE`, `0xFFF`, a second-page relocation, a truncated last page,
and a directory with overlapping relocations across adjacent pages.

The opt-in `inspect --layout` path reports, from the **verified original**
ICD, the `.text` RVA and raw offset, page count, excluded HIGHLOW bytes,
retained bytes, page-boundary crossing count and the number of nonempty
page-local spans. The validation uses the **same exclusion windows** as the
experimental checksum helper, without invoking the checksum mixer. A new
synthetic invariant test shows that changing any skipped relocation byte
cannot change that helper's result. Counts of page-local spans are a
research convention, not proof of unSafeDisc's actual chunk ordering.

**This is not a validated original SafeDisc checksum**. The helper uses
a page-oriented calling convention that must still be checked against the
unSafeDisc 1.5.5 control flow. It has not been run on the user's retail
executable, is not invoked by the production `inspect` command, and
does not implement PE section selection or the `dplayerx.dll` modifiers.
Reporting a complete TEA key now would be misleading.

Static disassembly of `unSafedisc.exe` at `0x401F62..0x401FA7` also
confirms the exact per-DWORD pre-TEA sequence:

```text
for index i in 0..floor(section_fragment_len / 4):
    mask = ((i << 24) | (i << 16) | (i << 8) | i) modulo 2^32
    word = load_le_u32(fragment[4*i..4*i+4])
    word = first_dplayerx_modifier(word XOR mask)    # RVA 0x65BE
    word = second_dplayerx_modifier(word XOR mask)   # RVA 0x6621
    store_le_u32(fragment[4*i..4*i+4], word)
```

**Important subtlety:** the index is the **full 32-bit word number**, not
`i & 0xFF`. The mask at index `0x100` is `0x01010100`, not
`0x00000000`. This matters for blocks containing more than 256 DWORDs.

The exact *outer adapter* has been ported as
`pre_tea_dwords_with_modifiers`, with synthetic function callbacks for
testing its XOR placement and call order. The new
`pre_tea_section_pages_with_modifiers` wrapper resets this word index on
**each 4-KiB page**, processing complete DWORDs and leaving trailing bytes
untouched. Synthetic tests cover a full first and second page and a
short third page, including the full-width mask at index `0x100`.
This page-local sequencing also agrees with the independent
[SafeDisc 1.50.20 image-cipher implementation used by MW4Remastered](https://github.com/Icehellionx/MechWarrior-4-Remastered/blob/main/src/MW4Remastered.Core/Install/SafeDisc15020ImageCipher.cs),
but its title-specific modifiers are **not suitable for Crimson Skies**. **The real EAX modifier routines
from the original protected `dplayerx.dll` have not yet been recovered,
so this is not a functioning decryption step.** The Rust program continues
to expose **`inspect` only**, and it never alters game files.

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


## Original DLL encoded-text recovery

An independent native offline examination of the user's exact, hash-verified DLL shows that its data section contains short strings encoded by a seeded byte transform. This transform is separate from the still-unresolved protected code section.

At RVA `0x20508`, the source bytes `F6 02 63 D5 00 63 8A 51 0D` recover `Kernel32` followed by a NUL terminator. Applying the same transformation across the original file-backed `.data` yields 27 readable candidate strings, including `User32` at RVA `0x20528`.

The recoverable state starts at `0x00DE88BD`; the low byte is XORed with each input byte; the state then updates as `(state * 0xDD437681 - 0x5F9ABCD6) mod 2^32`. A decoded NUL terminates the string, with a bound of 128 bytes. Static x86 instructions at `0xE205` and `0xDF96` support this model.

This is a verified data decoding result, **not** a claim that DLL `.txt` functions at `0x65BE` and `0x6621` have been restored. The original DLL is never executed or published.


## Static references to original encoded DLL labels

The exact verified original DLL also contains **23 static `PUSH imm32` instruction encodings** in its backed `.text` section whose DWORD operands are preferred-base addresses of the decoded data labels. The scanner checks opcode byte `0x68` followed by a full 32-bit VA; this is not an assumption of runtime reachability.

Examples of original code-site RVA → original decoded data-label RVA:

| Code RVA | Data RVA | Recovered label |
| --- | --- | --- |
| `0xC5D9` | `0x20508` | `Kernel32` |
| `0xC684` | `0x20528` | `User32` |
| `0xD28E` | `0x20570` | `LoadLibraryA` |
| `0xD37A` | `0x20598` | `LoadLibraryExA` |
| `0xD7E4` | `0x20660` | `VirtualProtect` |
| `0xDC6C` | `0x20548` | `Wnaspi32.dll` |

The already-proven attach path reaches the decoding helper `0xE25C` with the pointer to the first of those labels. The extra static references reveal how broadly the same metadata scheme is used, but do not establish that the protected executable `.txt` section has been decoded.

The native read-only `--strings` option now reports both all 27 candidate data labels and all 23 code references. Synthetic Rust tests cover exact literal PUSH operands, mismatching opcodes and the byte decoder. All input binaries are SHA-256 validated before inspection.

### PE relocation corroboration for decoded metadata references (2026-10-27)

The `--strings` report now cross-references every one of the **23 original
`0x68 imm32` byte-pattern matches** against the **original DLL's PE32
`IMAGE_REL_BASED_HIGHLOW` relocation table**. It classifies a match as
`HIGHLOW-backed` only when a relocation begins at **the four-byte immediate
operand** (instruction RVA + 1), not merely somewhere in the instruction.

This helps distinguish genuine absolute-address operands from incidental
overlapping byte sequences. The native CLI reports **both categories and
their counts**, rather than hard-coding an unverified number of
relocation-backed references. The expected *total* stays 23 for the
exact-hash original DLL. The independent synthetic regression explicitly
tests an operand relocation, an unrelated opcode-position relocation, and
RVA overflow rejection.

**Evidence limit:** PE relocation corroboration establishes a probable
absolute-address operand; it does not itself prove x86 instruction
alignment, static control-flow reachability or execution, and it does
not recover the encrypted `.txt` code. All analysis remains read-only.

### Relocation-first metadata pointer survey

The same read-only `--strings` run now also **inverts** the search:
it walks genuine, validated PE32 HIGHLOW relocation DWORDs in the
original file-backed, nonencrypted DLL `.text` and `.txt2` sections.
For each DWORD whose *current, unrelocated preferred-base value*
equals one of the 27 decoded labels, it prints the operand RVA,
label RVA and preceding byte. This can discover absolute pointers
stored under encodings other than `PUSH imm32`.

Unlike the 23 raw `0x68 imm32` matches, this survey begins from
actual relocation table entries and does **not** impose a hard-coded
expected count on previously unmeasured matches. The synthetic
regression verifies a relocation DWORD that straddles a 4-KiB page
boundary, an unrelocated lookalike, an out-of-section slot and
overflow rejection. The results remain a **static pointer survey**:
the preceding byte is not treated as an instruction decoder, and
control-flow reachability cannot be inferred.

The actual `.txt` ciphertext is explicitly **excluded** from
this metadata pointer survey.

---

### Original contiguous API pointer table recovered from 13 x86 resolver sequences

An independent, offline, byte-exact scan of the owner's original SHA-256
verified Crimson dplayerx.dll identifies thirteen highly structured
resolver-style x86 sequences in the plaintext .text section.
Each follows an absolute encoded-name pointer, an input module handle
and the same E8-relative call target RVA 0xC708. The returned EAX is
stored in an original DLL .data DWORD, then that same DWORD is
immediately checked against zero.

| Encoded API name | E8 callsite | Destination RVA |
| --- | --- | --- |
| ReadProcessMemory | 0xD6C5 | 0x24EF8 |
| WriteProcessMemory | 0xD76F | 0x24EFC |
| VirtualProtect | 0xD7ED | 0x24F00 |
| CreateProcessA | 0xD84F | 0x24F04 |
| CreateProcessW | 0xD8C9 | 0x24F08 |
| GetStartupInfoA | 0xD95A | 0x24F0C |
| GetStartupInfoW | 0xD9BB | 0x24F10 |
| GetSystemTime | 0xDA30 | 0x24F14 |
| GetSystemTimeAsFileTime | 0xDA8D | 0x24F18 |
| TerminateProcess | 0xDAF2 | 0x24F1C |
| Sleep | 0xDB84 | 0x24F20 |
| SendASPI32Command | 0xDDCC | 0x24F24 |
| GetASPI32SupportInfo | 0xDD5D | 0x24F28 |

All 13 store addresses are distinct consecutive 4-byte slots
covering original RVAs 0x24EF8..0x24F28 inclusive. The original
.data raw section covers RVA 0x20000..0x23E00 while its
VirtualSize covers up to 0x26C80, so **every target is
in memory-only zero-filled .data**, not an initialized
on-disk function-pointer table.

The evidence is unusually strong because the original
IMAGE_REL_BASED_HIGHLOW relocation table independently
identifies ALL THREE absolute DWORD operands in every
sequence: (1) the PUSH string pointer, (2) the
MOV [absolute],EAX storage, and (3) CMP [same absolute],0.
The resolver E8 rel32 targets also all independently
equal 0xC708. This yields 39 confirmed absolute
pointer relocations across the 13 sequences.

The new native read-only inspect --strings diagnostic
derives and checks every sequence, its expected decoded
name, its specific 4-byte slot, relocation evidence
and the memory-only .data bounds. A synthetic Rust test
requires three matching HIGHLOW operands and rejects
a tampered storage-check operand.

This establishes a static resolver-style API pointer
table. It does NOT establish that every sequence is
dynamically reached, prove exactly how RVA 0xC708
resolves Windows exports, or show that any API call
actually decrypts protected .txt machine code. The
original executable and DLL remain unexecuted, private
and unchanged.

### The on-the-fly memory writer: two VirtualProtect calls around a decoded buffer

Following the resolved VirtualProtect pointer slot
from original DLL RVA 0x24F00 back to its relocation-backed
FF15 indirect callers uncovers two actual x86 encodings
in the original plaintext .text section:

- RVA 0x9677: FF15 indirect call via preferred VA 0x00904F00.
- RVA 0x96DC: another FF15 via exactly the same runtime pointer.

Between these two calls, the original code contains
0x969B SHR ECX,2, 0x969E REP MOVSD, 0x96A2 AND ECX,3
and 0x96A5 REP MOVSB. The first call is prepared with
PUSH 4 (PAGE_READWRITE) and a pointer to an old-protection
local variable at RVA 0x9669. The second call receives the
saved prior protection. This supports a copy-into-protected-
memory helper which makes the destination writable,
copies the requested number of bytes, then attempts to
restore the previous protection.

The only literal E8 callsite to the copy-out helper RVA
0x9640 in the original file-backed .text section occurs
at RVA 0x95B9. Its caller's broader sequence also includes:

- RVA 0x940D: compare remaining buffer bytes against
  0x1000, so each selected chunk is at most 4096 bytes.
- RVA 0x9460: E8 call to RVA 0x9710, a separate
  copy-in helper for the original untransformed bytes.
- RVAs 0x950D and 0x9519: reads of buffer byte
  at the current index and immediately preceding index.
- RVA 0x9520: an explicit XOR of those bytes,
  followed by a write at RVA 0x9525.
- RVA 0x955D: an ADD in the transformation sequence.
- RVA 0x95B9: E8 call to RVA 0x9640 for the
  protected copy-out operation.

These are **verified instructions from the exact original
hash-checked DLL** and they explain how a buffer can be
transformed and copied into memory that is normally
nonwritable. Both VirtualProtect calls use a pointer
resolved earlier into virtual-only .data: they do **not**
reference KERNEL32 through an ordinary imported IAT entry.

This is the leading hypothesis for restoring protected
SafeDisc code. It still needs a proven runtime call path,
a demonstrated destination in the encrypted .txt section
and exact recovery of the stateful byte transformation.
The new read-only --modifier-research checks both indirect
FF15 operands, the PE virtual-only slot geometry, the
copy-in / copy-out relative-call targets, and the specific
original x86 operation bytes. A bounded synthetic
regression checks that altered copy instructions fail
closed. No protected code is executed or emitted.

### The 4KiB byte transformation hides two verified constants: 0x43 and 0x15

The original routine at RVA 0x92B0 starts with stack-frame
size 0x102C, enough room for a 4096-byte scratch buffer
plus bookkeeping. Its intentionally redundant immediate
loads disguise two small initial byte constants:

```text
0x92F6: MOV EAX,0xF3E97EC9
0x92FB: MOV EBX,0xF3E97E8A
0x9302: XOR EAX,EBX       = 0x43
0x9305: MOV [EBP-0x18],EAX

0x9309: MOV EBX,0xC502BDE6
0x9313: MOV ECX,0xC502BDD1
0x931A: SUB EBX,ECX       = 0x15
0x931F: MOV [EBP-0x20],EAX
```

The code reuses these locals in its later buffer
manipulation. It also has static direct E8 calls:
RVA 0x924F -> 0x92B0 (transform entry),
0x9338 -> 0x18800 and 0x9355 -> 0x18B00
(file-backed .txt2 helpers involved in input selection).

The native read-only --modifier-research recomputes
the two constants from the actual original immediate
DWORDs and asserts exact relevant x86 instruction bytes
and three call destinations. A standalone synthetic
fixture confirms the values and rejects altered SUB
opcode bytes. This gets closer to reconstructing
the per-chunk byte transform, but does not establish
its input, complete state update, or decrypted .txt
plaintext. No original binary is modified or executed.


### Bounded local inspection of unresolved chunk-mixer instructions (2026-10-27)

The read-only `inspect --modifier-research` output now includes six bounded
raw-code windows from the **locally supplied, SHA-256-verified original DLL**:
`0x92B0` (96 bytes), `0x9300` (96 bytes), `0x9400` (96 bytes),
`0x9500` (128 bytes), `0x18800` (96 bytes) and `0x18B00` (96 bytes).
The first four expose context around the currently incomplete chunk-state
reconstruction; the final two expose the `.txt2` selection helpers.
All reads use validated file-backed executable RVAs. No DLL bytes are embedded
in source or documentation, and the tool never executes the DLL.

These are **byte dumps, not instruction-boundary proofs**, and they do not
establish the source or destination of protected chunks. In particular,
interpreting `0x43` and `0x15` as complete per-byte keys would be premature.
A verified per-chunk algorithm still requires a documented data selector,
initial and carried state, loop termination and destination-section evidence.
