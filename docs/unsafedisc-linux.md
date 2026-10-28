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


### Correction: indexed byte reads do not yet prove an adjacent-byte recurrence

Exact original x86 encodings at `0x950D` and `0x9519` reveal the effective
addresses `[EBP + ECX - 0x1028]` and `[EBP + EAX - 0x1029]`, respectively.
They reference different live index registers. Even though their constant
displacements differ by one, adjacency depends on the still-untraced values
of EAX and ECX. Furthermore `33 D1` at `0x9520` is `XOR EDX,ECX`, and
`03 D0` at `0x955D` is `ADD EDX,EAX`: both operate on **32-bit registers**,
not intrinsically on bytes. Any final byte storage/truncation must be
established from subsequent instructions. The native research CLI now
states these limitations and rechecks the two exact SIB encodings.
This corrects earlier descriptive shortcuts; it does **not** establish a
new decryption algorithm, or prove a destination in protected `.txt`.


### Original 4-KiB chunk byte recurrence: instruction-backed candidate (2026-10-27)

New offline disassembly of the **private hash-matched** original Crimson
`dplayerx.dll` makes the earlier register-index ambiguity resolvable:
`0x9508 MOV ECX,[EBP-0x10]` and `0x9514 MOV EAX,[EBP-0x10]`
load the **same** index before the two differently based memory reads.
Therefore `0x950D` addresses byte `i` and `0x9519` byte `i-1`.
The index is initialized to **1** at `0x94EA`, incremented at
`0x94F3`, and compared against the chunk count at `0x94FC`.
Register upper bits are zeroed before the XOR at `0x9520`, and the low
byte is stored at `0x9525`. `0x9555..0x9562` adds the low byte of
state, storing the truncated result; `0x9581..0x958E` adds the transformed
current byte to the stored state as an 8-bit ADD.

For a **nonempty separately initialized chunk**, a candidate model is:

```text
out[0] = in[0] XOR 0x43
state = out[0] XOR 0x15
for i = 1 .. chunk_length-1:
    out[i] = ((in[i] XOR out[i-1]) + state) mod 256
    state = (state + out[i]) mod 256
```

This is more specific than the previous two independent indexed-read
observations. The new read-only Rust check asserts actual original x86
instruction spans, and synthetic test vectors exercise the isolated model.
It does not yet prove the *input buffer provenance*, control-flow
reachability under every opaque predicate, how state behaves between chunks,
or whether the destination is encrypted `.txt`. Neither original executable
nor generated plaintext is committed.

The `.txt2` helper at `0x18800` is itself a short argument-forwarding
wrapper calling `0x181E0` at RVA `0x18823`. The helper `0x18B00`
begins by calling `0xA810` with a length of `0x28`, followed by a
bytewise comparison against the supplied buffer. This is evidence of
selection and checking, not yet evidence of where the selected memory
actually points.


### Writer destination provenance: descriptor base plus chunk offset

Disassembling the exact original DLL at `0x9649..0x9652` shows
`MOV EAX,[EBP+8]; MOV ECX,[EAX+4]; ADD ECX,[EBP+0xC]; MOV [EBP-8],ECX`.
The first argument to the writer is passed by its caller as `LEA [EBP+8]`
at `0x95B5`, along with chunk offset, chunk length, and scratch-buffer
address. Thus the write address is derived from the descriptor's DWORD
at offset +4, plus the running chunk offset; it is **not hardcoded** in
writer `0x9640`. The original opcode bytes at both sites are now checked
by the native research CLI.

Tracing the caller's descriptor initialization and the result from `.txt2`
helper `0x181E0` is the remaining prerequisite to proving whether any
specific `.txt` address is targeted. This observation alone does not
validate a decrypted page or establish the full unpacking algorithm.


### Selector 1 resolves executable CODE-section criterion (2026-10-27)

The private, SHA-256-verified original DLL has a six-argument call at
`0x9338` into wrapper `0x18800` with selector **1**. This wrapper calls
`0x181E0` at `0x18823`. The latter decrements selector at `0x18255`,
checks the range at `0x18263`, and performs a fixed jump-table dispatch
from original DLL RVA `0x18680` at `0x18272`. Case index zero is the
original absolute VA `0x008F83C2` -> RVA `0x183C2`.

Case `0x183C2` pushes the mask and required flags **`0x20000020`**
(twice) then calls `0x189F0` at `0x183D4`. The latter reads section
header `Characteristics` at **offset `+0x24`** (`0x18A1A`), ANDs with the
provided mask and compares against the required flags (`0x18A24`).
The criterion is `IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE`.

The original `CRIMSON.ICD` has only one section with both bits, `.text`
(`Characteristics=0x60000020`); all other ICD sections fail that mask.
**This is strong static evidence that selector 1 chooses ICD `.text`,**
conditional on the helper receiving the ICD headers and returning the
successful section descriptor. We have not yet completed the full pointer
provenance chain proving the writer's runtime destination, nor produced a
validated decoded page.

The native Rust research CLI now checks the precise original opcode windows,
both E8 targets and the first jump-table DWORD in the local DLL.


### Section-header iterator and descriptor output confirmed from original DLL

Additional original-file static x86 validation shows selector helper
`0x189F0` walks PE section headers in **0x28-byte** increments (`0x18A2C`),
reads `IMAGE_SECTION_HEADER.Characteristics` at `+0x24` (`0x18A1A`),
and returns the matching **header address** (`0x18A3A`). This is not
itself the image memory address where transformed bytes are written.

At `0x183E6..0x18405` selector case 1 copies output fields derived from
the selected section header / local working structure into caller-provided
pointers, then invokes `0x18B90` at `0x18405`. That helper reads
structure offsets `+0x08` and `+0x10`, returning a bounded value.
This is strong evidence of a structured section-selection API rather than
an arbitrary byte search, but the exact provenance of the **writer's**
`descriptor[1]` (its `+4` DWORD) still requires a full call-argument trace.
Native inspection now checks the corresponding exact original instruction
bytes and the helper's E8 target. No decryption, execution or original
binary modification was performed.


### Memory-writer control-flow guard (2026-10-27)

Original `dplayerx.dll` bytes at RVA `0x967D` implement `TEST EAX,EAX`
after the first indirect `VirtualProtect` call. A nonzero return proceeds
toward RVA `0x9690`, where ECX/ESI/EDI are initialized for the
`REP MOVSD` and `REP MOVSB` copy. A zero result takes an unconditional
short jump toward RVA `0x9702`, bypassing these copies. The native
research CLI checks the exact branch and copy-setup opcode windows.
This is a statically verified failure guard, **not a runtime trace**.
The destination is still derived from a caller-provided descriptor;
section membership and any plaintext reconstruction require separate
proof. Private originals remain unmodified and unpublished.


### Selector-to-writer stack descriptor identity and extent rule (2026-10-27)

Instruction-aligned disassembly of the exact private DLL resolves a
previous missing link: inside one `0x92B0` frame, `0x9334` and `0x95B5`
both form `LEA reg,[EBP+8]`, passing the **same in-frame descriptor** to
selector `0x18800` and protected writer `0x9640`, respectively. Thus the
writer does consume the descriptor operated on by the executable CODE
section selector, although its absolute `+4` destination value still needs
independent confirmation against the original ICD load/base context.

The section extent helper `0x18B90` is independently verified against
original bytes. It returns `field(+0x10)` when `field(+8)==0`, otherwise
`min(field(+8),field(+0x10))`; this corresponds to the familiar nonzero
`VirtualSize` bound when the working structure contains PE section fields.
It does **not** prove whether every later copy uses that bound unchanged.

The native read-only `--modifier-research` checks exact original opcodes
at both descriptor-passing call sites, writer call target and the complete
extent selection branches. No Windows code execution, protected-code output,
or user-file modification is involved.


### Exact retail ICD selector geometry and preferred-load address (2026-10-27)

Independent offline PE-header parsing of the user's private original
`CRIMSON.ICD` confirms `ImageBase=0x00400000`, with a unique
`IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE` match:

- `.text`: `VirtualAddress=0x00001000`, `VirtualSize=0x002014BD`,
  `SizeOfRawData=0x00202000`, `Characteristics=0x60000020`.
- The other five sections (`.rdata`, `.data`, `.idata`, `.rsrc`, `.reloc`)
  do not match `0x20000020`.
- Therefore, **if** selector 1 receives the verified ICD section headers,
  it selects `.text`, whose **preferred** mapped start is `0x00401000`.

This `ImageBase + RVA` computation is a *conditional PE geometry result*;
it is **not** evidence that the writer descriptor's `+4` slot holds
`0x00401000` at runtime. ASLR/relocation/load base and the helper's
output contract still need to be traced. In particular, `0x189F0`
returns a section-header pointer and `0x183C2` populates separate
output arguments: those are not interchangeable with the writer's
final writable memory pointer. No recovered plaintext is asserted.


### New upstream prerequisite in the selector: PE-header initialization at 0x18940

The original `.txt2` function `0x181E0`, before dispatching to selector 1,
passes two stack scratch-buffer addresses plus its first pointer argument
to `0x18940` (call at `0x181FF`). It then reads two 16-bit values from
scratch (`0x18204..0x1821F`) and computes an address using a
**40-byte stride** (`LEA ECX,[ECX+ECX*4]`, then `[EAX+ECX*8]` at
`0x18226..0x18229`). This is consistent with computing a PE section-table
address from header offsets and the IMAGE_SECTION_HEADER entry size.

This new finding pinpoints `0x18940` and the caller-provided first
pointer as the next elements to resolve before asserting the exact value
written into the selected section descriptor or passing a plaintext
comparison. There is no claim yet that its pointer is an on-disk ICD
buffer, a loaded module base, or a mapped `.text` destination.


### Exact role of `0x18940`: read DOS and NT headers, then validate PE signature

Independent `objdump -D -Mintel` inspection of the user's private original
`dplayerx.dll` (`.txt2` RVA `0x18940`) establishes the following exact sequence:

- `0x18942` and `0x18947` take the first input pointer and first scratch buffer;
  `0x1894B..0x18951` call `0xA810` with requested **0x40 bytes**, offset zero.
- `0x1895A` reads DWORD `e_lfanew` from the DOS scratch buffer at `+0x3C`.
- `0x18960..0x18968` call `0xA810` again, this time with **0xF8 bytes**,
  starting at `e_lfanew`, into the second scratch buffer.
- `0x1896D..0x1897F` compares its first DWORD to **0x00004550** (`PE\\0\\0`)
  and returns a boolean in AL. This is a *header validation* helper, not
  itself a section selector or destination address computation.

The caller `0x181E0` obtains section count and optional-header size from
this scratch structure, constructs a `40 * section_index` stride and then
passes the resulting section table address to a selector. The original
`0x183C2` path calls `0x189F0`; success copies selected-section metadata
into three caller-supplied output locations. In particular it writes its
first two output DWORDs from scratch offsets `+0x24` and `+0x1C`
(`0x183ED..0x18403`). Determining the exact scratch-buffer layout and
those outputs' runtime meanings is still necessary before asserting an
absolute `.text` write address.

**Important scope correction:** merely finding a section header does not
mean `0x18940` supplies an image base. Its observed job is reading and
validating PE headers through `0xA810`. The actual pointer and offset
passed to `0xA810` must be traced through the caller to determine whether
it addresses disk-backed ICD bytes or a loaded image. No decrypted output
is claimed.


### Runtime initializer for writer descriptor +4 recovered (2026-10-27)

The original unencrypted DLL `.text` contains a concrete initializer at
RVA `0x986A`: `FF 15 98 E0 8F 00` calls the ordinary PE IAT slot RVA
`0x1E098`. Independent PE import-table parsing identifies this as
`KERNEL32.dll!GetModuleHandleA`, not a speculative dynamically resolved
function. `0x9869` pushes the module-name pointer returned by the preceding
helper; `0x9870` executes `A3 E4 59 90 00`, storing the function result
(`EAX`) into virtual-only `.data` global RVA `0x259E4`. At `0x988E` that
global is read again and written into descriptor field `+4` at `0x9893`.
The earlier caller `0x923E..0x9244` copies the same global into the writer
path's descriptor.

**Refined conclusion:** the writer base is derived from a loaded module
handle, not a statically embedded `0x00401000` address. The precise name
passed to `GetModuleHandleA`, its actual return value, the module's load
base and the section-relative offset remain to be validated. This is a
static argument/data-flow proof and not a runtime observation or decoded
`.text` page. `inspect --modifier-research` now checks the actual FF15,
store/load opcodes and binds the FF15 IAT slot to the named PE import.


### Critical correction: memory writer targets the SafeDisc DLL module itself

The missing `GetModuleHandleA` input can now be resolved from the **actual
private original DLL bytes**: RVA `0x985C` pushes preferred VA `0x00900030`
(data RVA `0x20030`) and calls the already-verified metadata string decoder
`0xE25C` at RVA `0x9861`. Offline decoding of the exact file-backed bytes
at `.data+0x30` yields **`dplayerx.dll`** (NUL terminated). Its returned
pointer is passed to the imported `KERNEL32!GetModuleHandleA` at `0x986A`,
and that module handle is stored into global RVA `0x259E4` at `0x9870`.

**Therefore, when these calls succeed, the writer's descriptor +4 holds
the loaded `dplayerx.dll` base, NOT `CRIMSON.ICD` or the preferred
`0x00401000` ICD code address.** This supersedes the earlier working
hypothesis that the particular `0x92B0 -> 0x9640` copy route directly
writes decrypted game `.text`. The selected section and byte offset in
this path must be examined as a **DLL self-modification/initialization**
route first. Another path or later stage may still restore protected game
code; no such stage is claimed here.

The CLI now verifies the original metadata PUSH/CALL, decodes the exact
`dplayerx.dll` argument from the verified local DLL and checks the
`GetModuleHandleA` IAT binding. It does not execute the code, obtain a
runtime handle, or generate game/DLL plaintext.


### Five-slot module registry behind DLL self-initialization (2026-10-27)

Verified against the owner's original SHA-256-matched DLL (hash
`dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff`):
RVA `0x987C` calls `0xA140` with the 3-DWORD global descriptor
and mode `1`. At `0xA14F`, `0xA140` calls `0xB540`.

`0xB540` searches a module-record table from DLL RVA `0x24AE8`
through `0x24EF7`, in increments of `0xD0`. This yields **five slots**.
It tests each slot's first 16-bit word for zero, clears 0x34 DWORDs
(0xD0 bytes) upon allocating an unused slot, marks the slot active,
and copies the original descriptor's three DWORDs into record offsets
`+4`, `+8`, `+12` (`0xB583`, `0xB58B`, `0xB591`). It then calls
`0xB6D0`, which initializes an auxiliary eight-pointer structure.
The slots are adjacent to, but do not overlap, the earlier dynamically
resolved API pointers beginning at RVA `0x24EF8`.

This gives a stronger explanation for `0xA140`: a registry-backed
module initialization routine, not a single standalone memory write.
Its later helpers may initialize additional code/data sections, but
**neither recovered game plaintext nor the precise target bytes of the
`0x92B0` transformation have been verified**. The Rust inspector checks
the original opcode windows and rel32 destinations. No proprietary
source bytes or memory dumps are committed.


### Eight auxiliary records: allocator and rollback confirmed (2026-10-27)

Disassembly from the owner's exact SHA-256 verified `dplayerx.dll`
shows that `0xB6D0` allocates a **temporary eight-DWORD array** on
its stack and invokes `0xB640` once per slot, passing indices `0..7`.
If every result is non-NULL, it copies eight DWORDs to the caller's
record (`0xB6F7..0xB705`) and returns AX=1. If any call returns NULL,
it instead calls `0xB770` on the temporary array (`0xB70C..0xB715`)
and returns AX=0, without committing that pointer array.

`0xB640` requests a `0xCC`-byte, 16-byte-aligned object through
`0xB610` (`0xB642..0xB649`), then calls `0xB7C0` twice to construct
two additional linked objects. It stores their pointers at offsets `0`
and `+4`, and establishes reciprocal links at the second-level objects'
offsets `+4` and `+8` (`0xB692..0xB69C`). Allocation failures take a
cleanup path rather than returning a partial object. These are static
function-level observations, **not** evidence that any of the eight
objects contains decrypted `.txt` code.

`inspect --modifier-research` now validates key original instruction
sequences and direct CALL targets. The raw game binaries remain private;
no DLL execution, unpacking, or protected plaintext emission is added.


### Eight auxiliary pointers index linked interval/range lists (2026-10-27)

Independent disassembly of the exact private `dplayerx.dll` clarifies the
previously unknown `0xB7C0` path. It allocates a **12-byte list header**
through `0xB610` (`0xB7C0..0xB7C8`), clears two link-related fields,
then calls `0xC2A0` at `0xB7F3` to allocate a **12-byte list node**.
`0xC2A0` stores three caller-provided DWORDs into node offsets
`+0`, `+4`, and `+8`. Other consumers reveal these as a starting
position, length/extent, and comparison tag: `0xC350` checks membership
within a half-open interval and the associated tag, while `0xC3D0`
compares the current node's start against a probe value.

`0xBAD0` traverses this linked node structure (`0xBAF1` / `0xBB15`),
calling `0xC3D0` and `0xC350` on successive nodes. `0xBB70`
iterates eight list pointers and invokes a list cleanup procedure on
each. Thus the auxiliary structures are **range bookkeeping** and
should not be described as eight containers of decrypted machine-code
bytes. What the particular eight categories or range tags mean for
SafeDisc's protected regions remains unknown.

The read-only Rust inspector now checks instruction-backed list-node
construction, traversal, range-boundary and call-target evidence.
Original binaries remain local, with no game plaintext emitted.


### Original range matcher `0xC350`: conditional in-place extension (2026-10-27)

Direct disassembly of the owner's SHA-256-verified original DLL at `.text`
RVA `0xC350..0xC3A9` provides a more exact contract than simply
“test range bounds”. This routine receives a pointer to a three-DWORD
range record `(start, length, tag)` plus requested `address`, `size`,
and `tag`. It first requires `start <= address < start + length` (unsigned).
Then it calculates `last = address + size - 1` in 32-bit x86 arithmetic.

If `start <= last < start + length`, it returns AX=1 only if the tags
match, otherwise AX=0. Crucially, **when `last` is outside the recorded
interval**, it still accepts a matching tag and **updates the stored
length to `last - start`** at original RVA `0xC398..0xC39A`, returning
AX=1. The exact function thus performs a **conditional range extension**,
not a read-only interval membership check. Its arithmetic and comparisons
should not be silently generalized to overflow-safe mathematical intervals:
the observed code uses unsigned 32-bit machine operations, with no proven
prior nonzero-size or overflow precondition.

The only previously identified caller at RVA `0xBB08` lies in `0xBAD0`,
which iterates linked range nodes, calls `0xC3D0` first to compare an
address against the node start, then invokes this range matcher. This
supports a mutable range-bookkeeping interpretation; it does not establish
what the eight registry categories mean, whether ranges refer to the DLL
or ICD, or any decrypted plaintext. Original files remain private.


### Distinguishing the C3D0 callers in linked-range traversal (2026-10-27)

Rechecked the owner's original DLL disassembly at original RVAs
`0xBC00..0xBC34`, `0xBC40..0xBC97`, and `0xBCA0..0xBD0D`.
`0xBC00` takes `(head, probe)`, walks nodes using the link at `+4`,
and calls `0xC3D0` at `0xBC12` with the current node's stored pointer
and the probe. On a match it **returns the node pointer**, otherwise
continues or returns NULL. This call site does not supply a separate
range tag and therefore cannot by itself reveal the eight category IDs.

`0xBC40` takes `(head, address, size, tag)` and tries `0xC4B0` first
at `0xBC5E` (four arguments: current node payload plus three inputs).
If that check succeeds it returns the node. Otherwise it invokes
`0xC3D0` at `0xBC6F` on the current node's payload and `address`;
a match on this fallback terminates the search without claiming success.
This is materially different from `0xBC00`'s plain search: matching a
start key need not mean a requested interval/tag can be accepted.

`0xBCA0` additionally invokes `0xBD10` to obtain a candidate before
searching with `0xBC00`, and conditionally delegates to `0xBD50` for
node/list manipulation. Therefore the `0xC3D0` references are mostly
**linked-list search and insertion helpers**, not direct evidence of
which encrypted code region is selected. The values used as eight
categories still require upstream caller tracing.

All addresses and relative calls in this note were verified by static
inspection of the private original DLL. No DLL execution, plaintext
reconstruction or proprietary binary upload was performed.


### Corrected original x86 range comparators: ordering, adjacency and coalescing (2026-10-27)

Independent instruction-aligned analysis of the owner's hash-verified original
`dplayerx.dll` establishes that the earlier description of `0xC3D0`
as a **start-key equality matcher was incorrect**. The exact original bytes
`8B 4C 24 04 8B 44 24 08 3B 01 1B C0 F7 D8 C3`
implement an unsigned `CMP probe,[node]` followed by `SBB/NEG`,
and return 1 **only when `probe < node.start`**. Equality returns zero.
Consequently `0xBC00` is a sorted-list lower-bound style walk; `0xBC40`
uses this ordering predicate to terminate searching when a probe is before
the next entry. The caller at `0xBEA1` uses the same ordering comparison.

The checked `0xC4B0` comparator, called at `0xBC5E`, accepts a record
`(start,length,tag)` and `(requested_start,requested_length,requested_tag)`.
It returns 1 when the tags match **and the ranges are immediately adjacent**
in either direction:
```text
record.tag == requested_tag AND
(
    record.start + record.length == requested_start OR
    requested_start + requested_length == record.start
)
```
Arithmetic here is native 32-bit wrapping; this is not general overlap,
containment, or plain tag equality. The 50-byte function is read-only.
A distinct `0xC4F0` compares **two existing record pointers** with
the same tag-and-adjacency predicate; exact bytes and direct callers
`0xB9E6` / `0xBA35` have been verified.

The caller `0xB9D0` uses `0xC4F0` to find adjacent tagged neighbors,
then invokes `0xC310` to merge the start/length of neighboring records,
and `0xBA90` to unlink and release redundant list nodes. Its loop repeats
while merges occur. This proves **in-place coalescing of tagged, adjacent
ranges**, rather than storing separate decrypted code buffers.

**Correction to the earlier `0xC350` note:** the extension writes
`(requested_address + requested_length) - stored_start` at `0xC398..0xC39A`,
not `last - stored_start`. The code computes `last = requested_end - 1`
only for its inclusion test, but uses the *exclusive* requested end for the
new length. No overflow-safe abstraction or zero-length precondition has
been proved.

The Rust `--modifier-research` inspector now asserts the complete original
15-byte `0xC3D0`, 50-byte `0xC4B0` and 50-byte `0xC4F0` functions,
eight direct E8 call targets, and synthetic test cases for strict ordering,
both adjacency directions, tag mismatch, overlapping/nonadjacent intervals
and wrapping arithmetic. These are **static memory-management semantics**,
not evidence of SafeDisc game-code plaintext. The eight category meanings,
the source of their numeric tags and the link to encrypted ICD pages remain
unverified.


### Numeric range categories map to the eight pointer slots (2026-10-27)

The original dispatcher `0xA2A0` looks up the module record through
`0xB5D0`. At `0xA2BE` the x86 instruction `MOV ESI,[EAX+EDI*4+0x14]`
selects the current category's list pointer. Based on the eight-pointer
initialization already verified, its field offsets are `+0x14`,
`+0x18`, ... `+0x30` for indices 0..7.

The fourth argument at seven independently verified direct call sites is an
immediate category value:

| PUSH at | E8 call at | Index |
| --- | --- | --- |
| `0xAE27` | `0xAE2C` | 0 |
| `0xB0B1` | `0xB0B7` | 2 |
| `0xBFED` | `0xBFF2` | 3 |
| `0xABD3` | `0xABD8` | 4 |
| `0xAB43` | `0xAB48` | 5 |
| `0xAAB3` | `0xAAB8` | 6 |
| `0xAC28` | `0xAC2D` | 7 |

All seven exact original `PUSH imm8` operands and E8 targets
(`0xA2A0`) are checked in the read-only Rust inspector.
This demonstrates the indexing mechanism and that at least seven
numeric categories are used directly. **No direct constant-1 call is
proven by this sample**, but that does not establish that category 1 is
unused: indirect or dynamically computed indices must still be examined.
Nor does the category number alone identify what kind of protected
memory it represents.

An isolated synthetic helper validates `0..7` as the legal slot indices
and rejects `8` and larger; **this must not be mistaken for a runtime
bounds check in the original function**. This is about registry
organization, not game plaintext recovery.


### First CODE+EXECUTE section of the DLL and conditional mode 0x67 (2026-10-27)

A new investigation distinguishes the PE *being inspected* from the
different protected game ICD. The original DLL PE header has **three**
CODE|EXECUTE sections in this order:

| DLL section | RVA | VirtualSize | Characteristics |
| --- | --- | --- | --- |
| `.txt` | `0x1000` | `0x7671` | `0x60000020` |
| `.text` | `0x9000` | `0xE506` | `0x60000020` |
| `.txt2` | `0x18000` | `0x5497` | `0x60000020` |

By contrast, only the game's `CRIMSON.ICD` `.text` section satisfies
the `0x20000020` mask. The original selector-1 case calls `0x189F0`,
which traverses the section headers **in order** and immediately returns
its first matching header. Thus, **when the input descriptor is the
original DLL**, selector 1 first chooses DLL `.txt` at RVA `0x1000`,
*not* game ICD `.text` and *not* DLL `.text`. This is a substantial
clarification to the earlier section-target hypothesis.

A separate instruction-aligned fragment at RVA `0x97E7` encodes
`PUSH 0x67; PUSH &global_descriptor; CALL 0xA550`. Helper `0xA550`
stores that value in descriptor field `+8`. The common read
dispatcher `0xA810` routes descriptor modes other than `0x65` to
`0xA620`; for mode `0x67`, `0xA620` can read backed bytes using
`descriptor[+4] + offset`, with `REP MOVSD/MOVSB` at `0xA69D/`0xA6A4`.
It can also consult separately registered backing storage, so the
straight memory-copy branch should not be assumed in all states.

Combining these *static* findings makes a plausible DLL self-restoration
route: descriptor mode `0x67`, module base from
`GetModuleHandleA("dplayerx.dll")`, selector 1 choosing the **first**
executable section (`.txt`), and a separate guarded copy into that
module. **Important limits:** the startup code has opaque predicates;
complete runtime branch reachability, any mapped-buffer alternatives,
the byte-state recurrence over actual `.txt` chunks and independent
plaintext validation remain unresolved. A selected section header is
not by itself a decrypted section or proof that every copy succeeds.

The read-only Rust inspector now asserts original mode-setting, read
dispatch, module-base/offset copy opcodes and the three actual DLL
section-header records. Synthetic tests cover first-match section order.
No copyrighted code bytes, DLL plaintext, or modified executable are
committed.


### Recovered DLL `.txt` selector outputs and copy-in/copy-out offset identity (2026-10-27)

Following the original `0x92B0` caller's descriptor through selector 1
gives the **three original DLL section fields**, on the condition that the
descriptor is initialized for `dplayerx.dll` as described above:

| Selector output | Value for first CODE+EXEC DLL section | PE source |
| --- | --- | --- |
| Section disk offset | `0x400` | `.txt.PointerToRawData` at section header `+0x14` |
| Section memory RVA | `0x1000` | `.txt.VirtualAddress` at header `+0x0C` |
| Effective extent | `0x7671` | `min(VirtualSize, SizeOfRawData)` |

The selected-section helper fills a 40-byte scratch header, then
`0x183ED/0x183F5` writes the raw offset into one output,
`0x183F1/0x18403` writes the **virtual RVA** into another, and
`0x18405 -> 0x18B90` writes the bounded extent to the final output.

At the `0x92B0` caller these map to stack locals:
`[EBP-8]` (disk offset), `[EBP-0x24]` (virtual RVA) and
`[EBP-0x14]` (remaining length). On the chunk path both
`0x9458` (copy-in to `0x9710`) and `0x95B1`
(copy-out to `0x9640`) pass `[EBP-0x24]` as the offset.
The two helpers independently compute
`descriptor[+4] + offset` (`0x9717..` and `0x9649..`).
This statically connects **read and write to the same relative DLL
virtual address**, rather than the raw file offset.

For the original `.txt` selection, the first candidate mapped address
is therefore `loaded_dplayerx_base + 0x1000`; the preferred-base
example is `0x008E1000`, **not** the game's `0x00401000`.
Later chunks advance the selected virtual offset while reducing the
remaining length. Copy-out remains conditional on the first
`VirtualProtect` call succeeding.

The inspector now checks exact instruction boundaries, direct call targets,
`.txt` header order and expected geometry. A synthetic test explicitly
distinguishes `PointerToRawData`, `VirtualAddress` and bounded
extent. **This is a convincing static DLL self-modification address
dependency**, but still not a runtime trace, a verified reconstructed
`.txt` page or an unprotected `CRIMSON.ICD`.

A standalone, read-only experiment applying the current *candidate*
byte recurrence to the first 4-KiB backed `.txt` block yielded no
independently verified executable-code signature or matched plaintext.
It is deliberately **not** claimed as decryption evidence; selector
reachability, real chunk initialization, relocation effects and comparison
against trusted DLL plaintext are still required. No output page is
committed or distributed.


### All 214 original `.txt` relocations target readable DLL image regions

A separate raw-PE relocation-directory walk of the exact original
`dplayerx.dll` independently confirms **214** backed
`IMAGE_REL_BASED_HIGHLOW` DWORDs starting within executable `.txt`.
Each original on-disk DWORD already has a preferred-base VA that points
inside the **same DLL image**, distributed as follows:

| Target section | Original `.txt` HIGHLOW operands |
| --- | ---: |
| `.data` | 195 |
| `.rdata` | 18 |
| `.txt` | 1 |
| Other/out-of-image | 0 |
| **Total** | **214** |

The Rust inspector now rejects mismatching original counts or targets
and prints this distribution during `--modifier-research`. These
recoverable pointers demonstrate that `.txt` contains valid relocation
operands mixed with protected bytes. The current candidate recurrence,
if naively applied to the complete first disk-backed page as an
unfiltered stream, has **not** produced verified code; relocation
handling and any transformation-span segmentation must be established
from original control flow. The raw location of a valid pointer alone
does not prove whether the DLL loader temporarily relocates it before
another stage restores or preserves it.

This finding is **not** a working decryptor and the tool does not
modify, output or embed the private DLL image.


### Categories 4, 5 and 6 originate in distinct original PE section selectors

Comparing the original x86 at three loop entrypoints with their respective
`0xA2A0` category-registering calls yields a direct static connection:

| Original iterator | Selector PUSH/CALL to `0x18800` | Category PUSH/CALL to `0xA2A0` | First-stage PE section filter |
| --- | --- | --- | --- |
| `0xAA80` | `0xAA9A` / `0xAA9D`: selector **10** | `0xAAB3` / `0xAAB8`: **6** | `(Characteristics & 0xC0000060) == 0x40000040` |
| `0xAB10` | `0xAB2A` / `0xAB2D`: selector **2** | `0xAB43` / `0xAB48`: **5** | `(Characteristics & 0xC0000040) == 0xC0000040` |
| `0xABA0` | `0xABBA` / `0xABBD`: selector **1** | `0xABD3` / `0xABD8`: **4** | `(Characteristics & 0x20000020) == 0x20000020` |

The original DLL's first matching section *by flags only* is `.txt` for
category 4, `.data` for category 5, and `.rdata` for category 6.
The selector-2 and selector-10 cases additionally call `0x187B0`,
which can reject a candidate before the next 40-byte section header.
Consequently `.data` and `.rdata` are **first-stage candidates, not
independently proven final selections**. For selector 10, DLL `.reloc`
also satisfies the initial masked comparison, but occurs after `.rdata`
and can be subject to the same exclusion logic.

The new offline Rust proof validates all six literal `PUSH imm8`
instructions, all six E8 destinations, and the three actual paired
Characteristics constants in the cases at RVAs `0x183C9`,
`0x18514`, and `0x18645`. Synthetic tests demonstrate first-match
semantics and why a matching relocation section is not automatically
a valid final result. Numeric category meanings are now **partly
linked to PE section classes**, rather than opaque integer labels.

The other categories 0,2,3,7,1 remain to classify. No dynamic
unpacking path or validated decrypted plaintext is claimed.


### The mysterious 0x187B0 selector filter is a PE data-directory exclusion

The exact private DLL x86 explains the additional selection filter
applied to selector 2 (category 5) and selector 10 (category 6):

1. `0x18770` invokes the validated DOS+NT header reader
   `0x18940`, then copies **0x20 DWORDs = 128 bytes** of
   `IMAGE_DATA_DIRECTORY[16]` entries (each entry is two DWORDs).
   The source is the PE32 optional-header data directory at
   **NT header offset 0x78**.
2. `0x187B0` compares the section's `VirtualAddress` against
   the **first DWORD (directory RVA)** of each entry, advancing its
   pointer by eight bytes for 16 comparisons. A match returns AL=1.
3. Selector 2 (`0x18533`) **skips** a matching directory-start
   section and continues searching. Selector 10 (`0x18668`)
   likewise skips it. This is not a generic regex, section-name
   exclusion, or 16-byte hash scan.

The original `dplayerx.dll` directories include
`IMAGE_DIRECTORY_ENTRY_BASERELOC` index 5 at **RVA 0x27000**
(the start of `.reloc`) and `IMAGE_DIRECTORY_ENTRY_IAT`
index 12 at **RVA 0x1E000** (the start of `.rdata`).
These are precisely the two sections that pass selector-10's initial
mask `(Characteristics & 0xC0000060) == 0x40000040`.
**Both are excluded by the second filter**, leaving no category-6
section selected for this *specific verified DLL* under the
standard mode-0x67 PE header reads. Category 5's candidate
`.data` at RVA `0x20000` does **not** match any directory-start
RVA and survives. Category 4 still selects the first CODE+EXECUTE
section, `.txt`.

This gives the following *original DLL* section result:
- Category **4**: `.txt` (first CODE+EXEC)
- Category **5**: `.data` (writable initialized DATA; not
  a data-directory start)
- Category **6**: **none** (both `.rdata` and `.reloc`
  excluded by their exact data-directory start RVA)

A category-6 list can still be used for other modules with different
section/directory layouts; this is **not** evidence that category 6 is
unreachable throughout the software.

The Rust inspector now validates the original directory-copy and
comparison instruction windows, the call targets, and the actual
sixteen PE directory RVAs. Synthetic tests model exact first-match
exclusion. No source or destination code is decrypted or published.


### Discovery: a second, independent decoder reconstructs the sentinel name `.txt2`

The caller that prepares the three-DWORD module descriptor uses a
**different decoder** from the previously recovered `E25C` metadata
strings. The verified x86 `0x9201` pushes original preferred
VA `0x00900088` (`.data` RVA `0x20088`), `0x9206`
prepares a local destination buffer, and `0x920A` calls `0x18BB0`.

The real x86 loop in `0x18BB0` copies byte zero unchanged, then for
each later, non-NUL input byte computes
`output[i] = (input[i] - 1 mod 256) XOR output[i-1]`.
The original **six-byte** source at RVA `0x20088` decodes exactly to
the NUL-terminated ASCII name **`.txt2`**. This does not use the
seeded LCG/XOR algorithm from `unsafedisc_strings.rs`.

At `0x922E` the caller pushes a pointer to that local decoded name,
and `0x92B0` forwards its third argument at `0x9349` to the
`0x9355 -> 0x18B00` call. This second helper reads **0x28 bytes**
through `0xA810` from the current candidate's section header and
compares the beginning of that buffer as a NUL-terminated string.
Thus there is a **byte-verified section-name comparison with `.txt2`**,
rather than merely a Characteristics-mask search.

A successful comparison affects `0x92B0` control flow, but because
the surrounding x86 includes overlapping/opaque branches, the
static comparison itself does **not** prove whether `.txt2` is
transformed, bypassed, or used as a stopping marker. That distinction
is the next control-flow task, crucial before attempting a complete
DLL section decode.

The read-only native inspector checks the original decoder and call
opcodes, confirms the decoded name from the exact local original, and
adds synthetic bounds tests for the separate tag algorithm.
No decrypted code, original DLL contents or modified game binaries
are added to the repository.


### Correction: named `.txt2` condition prevents equating first match with final copy target

The initial CODE|EXEC selector result of `.txt` is only the **first
candidate**, not an established final output destination of `0x92B0`.
Further original x86 validation establishes the following control
dependencies:

- `0x935D TEST EAX,EAX; 0x935F JE 0x938E` branches on the
  `0x18B00` section-name comparison result.
- `0x9361` sets a local 16-bit flag to **1** on the name-match
  path (the string was decoded as `.txt2`).
- `0x938E..0x9394` advances the search cursor by **0x28**, exactly one
  `IMAGE_SECTION_HEADER`.
- `0x93C0 E9 5D FF FF FF` is a genuine near branch back to
  `0x9322`, permitting repeated section-selection passes.
- `0x93C5..` tests that local flag and can jump to
  `0x9403`, the start of the 4-KiB chunk loop, when nonzero.

This establishes a **multi-candidate, named-section-gated** control
structure rather than a first-match-and-immediately-transform
algorithm. The original flag-setting and branching are checked by
the read-only Rust inspector. Branches surrounding these sites include
overlapping/opaque instructions, and their *complete reached path*
has not been verified for all relevant flags/inputs.

**Consequence:** earlier preferred-base arithmetic
`dplayerx_base + 0x1000` remains valid only for the initial `.txt`
candidate. It must not be described as the **final write target**
without tracking whether the cursor advances to `.text` or `.txt2`,
and exactly when its output locals change. The actual `.txt2`
section has raw offset `0x16200`, mapped RVA `0x18000`, and bounded
size `0x5497`. These are independently verified PE header values,
not a claim that this section is transformed.

The code path's role could be a restoration, re-encoding or
initialization operation. We explicitly do **not** claim that the
existing candidate byte recurrence decrypts DLL `.txt` or
`CRIMSON.ICD` based solely on these findings.


### Synthetic round-trip check for the provisional byte recurrence

To prevent implementation regressions before any real plaintext claim,
the research CLI test suite now includes a separately implemented
inverse for the **provisional** chunk transform:
```text
input[0] = output[0] XOR first
state = output[0] XOR second
for i in 1 .. len-1:
    input[i] = ((output[i] - state) mod 256) XOR output[i-1]
    state = (state + output[i]) mod 256
```

Round trips cover empty and single-byte buffers, ordinary byte patterns,
lengths 4095/4096/4097, and multiple synthetic key-byte pairs. The tests
demonstrate that the **mathematical candidate model** is reversible;
they do not show that the original DLL calls that model with those
inputs, that its state resets at those boundaries, or that any restored
bytes match protected code. No actual decoded DLL bytes are emitted.


### Verified DLL PE section header cursors for the `.txt2` name scan

The exact original DLL uses `e_lfanew=0xB0` and PE32 optional-header
size `0xE0`, putting its first 40-byte section header at file/mapped
header offset `0x1A8`. The original section-header *cursor offsets*
for CODE|EXECUTE sections are:

| Section name | Section-header cursor within image | Section's VirtualAddress |
| --- | --- | --- |
| `.txt` | `0x1A8` | `0x1000` |
| `.text` | `0x1D0` | `0x9000` |
| `.txt2` | `0x1F8` | `0x18000` |

These values are **different** from raw section file offsets
(`0x400`, `0x7C00`, `0x16200`) and virtual section data RVAs.
This matters because the `0x18800/0x189F0` selector returns a
**section-header cursor**, which `0x18B00` uses to read the
40-byte header and compare its Name field against decoded `.txt2`.
The cursor advance at `0x938E` is likewise `+0x28`.
The native inspector now derives all three original header positions
from PE headers and verifies their names.

The named match is demonstrably possible at cursor `0x1F8`,
after the two earlier CODE|EXECUTE candidates. This does **not**
by itself establish the ultimate section whose bytes are changed:
the obfuscated loop's full branch paths and the moment output
fields are retained or replaced must still be reconstructed.


### The two `.txt2`-name comparison outcomes converge before the cursor advances

A bounded **flag-semantic** analysis establishes both sides of
`0x935D TEST EAX,EAX`, using the observed boolean (0/1) return of
`0x18B00`. No protected binary is executed:

- **Mismatch (EAX=0):** ZF=1, so `0x935F JE` branches directly
  to `0x938E`, where the section-header cursor advances by 0x28.
- **Match (EAX=1):** ZF=CF=SF=OF=PF=0. The `0x9361`
  `MOV WORD [EBP-4],1` preserves flags. The reached Jcc chain is
  `0x9367(JS not taken) -> 0x9369(JNS taken) -> 0x937A(JG taken)
  -> 0x936A(JO not taken) -> 0x9370(JA taken)
  -> 0x937F(JNP taken) -> 0x938C`.
  At `0x938C`, a SUB modifies BL and EFLAGS; subsequent MOV
  operations reach the cursor increment at `0x938F..0x9394`.

This proves that **matching the `.txt2` name does not immediately
jump into the byte transform at that point**. It sets a persistent
match indicator, and both comparison outcomes continue at the
header-cursor advance. The later opaque `0x9397..0x93C0` control
flow and exit conditions remain unverified, as does which selector
outputs survive until the guarded copy. The inspector checks each
reached instruction's original opcode window and reports the
conditional conclusion explicitly. No final decrypted section is
asserted.


### Major control-flow correction: the observed 0x92B0 iteration appears to avoid all block copies

A bounded original-byte/static flag proof resolves the opaque
`0x9397..0x93C0` detour. Its preserved CF/ZF/OF flags have eight
possible combinations; all eight reach **`0x93C0 -> JMP 0x9322`**,
never the planted `RET` decoy at `0x93BE`.

The exact reached structure is:

- `0x9397 JMP 0x93A0 -> JMP 0x9399`.
  `0x9399 MOV EDX,EDX` preserves flags; the complement
  `0x939B JAE` / `0x939E JB` rejoins at `0x93A3`.
- `0x93A3 JO` / `0x93B0 JO` rejoin at `0x93A5`.
  `0x93A5 JA`, `0x93A9 JE` and flag-preserving XCHGs
  rejoin at `0x93AE JMP 0x93B3`.
- `0x93B3 JBE` goes directly to `0x93C0` when CF or ZF is set.
  Otherwise `0x93B5 PUSH EAX; 0x93B6 CALL 0x93FC` (a single-byte
  RET); `0x93BB POP EAX; 0x93BC JA 0x93C0` reaches the same
  back-edge. No original instruction on the reached route changes the
  tested flags. The earlier direct `0x93BE RET` is a decoy.
- All paths return to the section-selection call at `0x9322`
  after advancing the current section-header cursor by `0x28`.

An even more important *data-flow* observation follows. `0x18800`
passes the three output pointers to `0x181E0`; the latter first
stores **zero to all three outputs**, at `0x18255`, `0x1825C`,
and `0x18266`, before attempting the requested section lookup.
These outputs correspond to `0x92B0` locals
`[EBP-8]` (disk offset), **`[EBP-0x14]` (extent)** and
`[EBP-0x24]` (memory RVA). During an ordinary finite scan of the
DLL's sections, the next failed lookup therefore replaces *all three*
previous success values with zero.

On zero return from the selector, `0x9347 JE` enters `0x93C5`.
Even if the `.txt2` name-match flag was previously set and
`0x93CF JNE` reaches `0x9403`, the extent at
**`[EBP-0x14] == 0`** makes `0x9407 JE 0x962D` skip the
entire chunk-copy body. Consequently **the observed valid finite-scan
path through this particular 0x92B0 invocation does not execute its
0x9710/0x9640 byte-copy helpers at all**.

This is a major limitation of the earlier candidate
`dplayerx.dll .txt self-restoration` narrative. The presence of the
memory writer and byte-transforming code proves only that such a
mechanism *exists* in the binary; it does not establish an actually
reached decoding path. Here, under the static original-header traversal
and documented flag branches, its work appears *bypassed* on exhausted
section selection. Another invocation or a different code path could
still reach those helpers; this audit does not exclude them globally.

The Rust research inspector now validates every opcode of the opaque
detour, the original output-zero stores, call/branch targets, and
provides synthetic eight-combination flag proof plus a model showing
that a failed final selector call clears an earlier successful extent.
This does **not** execute `dplayerx.dll`, reconstruct protected
pages, or weaken the SHA-256 gating. Proprietary originals stay private.


### Critical correction: the .txt2 MATCH exits the selector before zeroing its extent

**This supersedes the preceding no-copy-after-exhaustion hypothesis for
normal successful .txt2 selection.** Running the actual Linux research
inspector on the exact private original exposed a mistaken x86 instruction
boundary: original RVA 0x938C contains the two bytes **EB 37**,
a JMP directly to **0x93C5**. The earlier presumed three-byte
SUB BL,0x37 begins at **0x938B**, an overlapping decoy, and
is not the reached instruction at 0x938C.

The correct local branch flow after 0x935D TEST EAX,EAX is:

- **Mismatch (EAX=0):** 0x935F JE 0x938E advances the section-header
  cursor by 0x28. The flag-preserving opaque detour at 0x9397..0x93C0
  returns to the selector at 0x9322, potentially trying another section.
- **Match (EAX=1):** 0x9361 stores match flag=1; the reached Jcc path
  goes to 0x937F JNP. Because TEST EAX=1 sets PF=0, it jumps to
  **0x938C JMP 0x93C5**. This is *before* the header increment
  and next selector call. The successful section's output fields
  therefore remain intact, including the **0x5497-byte extent**
  of .txt2; 0x93C5 branches to 0x9403, where its nonzero extent
  permits the 4-KiB block loop.
- **Exhaustion without prior matching exit:** the three output fields
  are indeed cleared to zero by 0x181E0, and 0x9407 skips copies.
  But the normal successful name-match path does **not** exhaust.

Therefore the preceding finite-exhaustion simulation was incorrect as
a claim that the normal 0x92B0 invocation does no copying: it omitted
the exact early-exit jump on name match. This is an explicitly
documented correction, not a separate SafeDisc algorithm.

**Corrected conclusion:** if this DLL initialization path is reached,
and the original section-header enumeration and name comparison succeed,
the loop selects **DLL .txt2**, memory RVA **0x18000**, original raw
offset **0x16200**, extent **0x5497**, and enters the
byte-transforming copy loop. Its actual runtime execution and restored
plaintext quality are still unverified. It does **not** establish
decryption of the game ICD.

The Rust exact-byte inspector checks the reached opcode EB 37 at
0x938C, along with both match/mismatch branches. The proprietary
DLL is neither distributed nor modified.


### Likely direction of the .txt2 operation: post-use scrambling, not proved unpacking

After correcting the match path, the Linux inspect-only binary built in
GitHub Actions was **executed locally on the exact SHA-256 validated
original game files** with the --modifier-research flag. It exited
successfully. This also confirmed that all earlier *original-byte*
assertions on this path pass; the synthetic-only CI does not perform
this private-input validation.

A separate byte-level clue about the intended transform:
the **original disk-backed .txt2 section already starts with
55 8B EC**, a normal x86 PUSH EBP / MOV EBP,ESP prologue. The recovered
first-byte initialization uses XOR 0x43, and a hypothetical successful
write of the transformed first byte changes its 0x55 to **0x16**.
This does not resemble restoration of the original readable .txt2
entry prologue. One plausible purpose is **post-initialization
self-scrambling** of helper code that has already served its purpose,
rather than unpacking the protected game's ICD.

This is still a **conditional interpretation**. No original Windows
DLL has been loaded, no VirtualProtect result observed, no transformed
DLL page validated against trusted memory, and the complete loop is
heavily obfuscated. The marker 0x16 is only a predicted *first byte*
from the original verified initial XOR; it is not evidence of working
plain-text reconstruction. An actual read-only Rust implementation
should preserve such distinctions explicitly.


### Statistical clue from the actual first .txt2 chunk: candidate resembles scrambling

Using the original hash-verified file, a **read-only local experiment**
applied the provisional per-chunk byte recurrence to the first
4096 original file-backed bytes of DLL .txt2. The original has
entropy approximately **6.137 bits per byte**, while the candidate
transformed bytes have approximately **7.952 bits per byte**.
The source also begins with a known normal x86 prologue
(0x55 0x8B 0xEC), which the candidate would disturb.

The substantial increase in candidate entropy **supports the
hypothesis that this path obscures previously readable .txt2 helper
code after initialization**, instead of recovering executable plaintext.
It is not proof: the full original x86 loop's opaque control flow,
true buffer state and runtime API success have not all been
independently established. The inspector now reports the
two first-block entropy values without printing or storing the
candidate ciphertext, and synthetic tests validate its entropy
function against constant, binary and uniform inputs.

For native unSafeDisc work the practical implication is to prioritize
the genuinely protected DLL .txt modifier code and game ICD transform,
rather than treating the .txt2 self-modification route as if it had
already produced a decrypted executable page.


### Crimson 1.50.20 relocation-fragment boundaries change the second-layer mask (2026-10-27)

**Important refinement to the earlier per-page adapter:** the exact
historical unSafeDisc 1.5.5 x86 at `0x401D82..0x401EF5`
uses relocation-aware traversal when runtime relocation-directory
metadata selects branch 4. Its calls into `0x401F01` receive
**individual spans of non-relocated bytes**, clipped to 4-KiB page
limits. `0x401F01` calls `0x401F43`, which enables the
`0x401F62` extra transformation for DLL size `0x21E00`
(the owner's Crimson DLL).

**At `0x401F62`, EBX (the per-DWORD index) is reset to zero
for every individual call**, not just when a new 4-KiB page begins.
Therefore the Crimson-specific relocation branch must consider
*both* 4-KiB page boundaries and the HIGHLOW DWORD gaps inside
those pages. A page-only word-index reset is a valid isolated
comparison model but **does not represent the complete relocation
fragment path** of the historical tool.

The exact protected ICD section `.text` has:
- RVA **0x1000**, effective extent **0x2014BD = 2,102,461 bytes**;
- **61,743** full HIGHLOW relocations, covering **246,972** bytes
  (including **36** HIGHLOW DWORDs crossing a 4-KiB page);
- **48,149** non-relocated page-local fragments of **at least 4 bytes**;
- **440,422** complete DWORDs across those fragments;
- **93,801** non-relocated leftover bytes in less-than-4-byte
  fragment tails (including whole runs shorter than one DWORD).

Independent Python inspection of the exact hash-verified original PE
relocation directory provides these counts and verifies that
`440422 * 4 + 93801 + 246972 = 2102461`.
This is **structural accounting of the second layer only**:
the untouched relocation operands and short tails should not be
confused with final unchanged bytes after later TEA, import and
other processing.

The research-only Rust function
`pre_tea_relocation_fragments_with_modifiers` accepts explicit,
strictly sorted, nonoverlapping section-relative HIGHLOW positions.
It skips those four-byte operands, splits by each 4-KiB page,
and resets the existing `pre_tea_dwords_with_modifiers`
index separately at every remaining contiguous byte run.
It uses **synthetic modifier callbacks only**, rejects malformed
relocation geometry, leaves short fragments untouched, and does
not emit any decrypted game file.
`--key-research` now asserts the original geometry against
those independently calculated counts before reporting the
provisional key. Synthetic tests exercise an internal relocation
gap, a crossing-page HIGHLOW DWORD, missing/overlapping positions
and incomplete-DWORD tails.

Caveat: not all historical unSafeDisc branches have been executed,
and the two Crimson-specific modifier routines inside the
high-entropy on-disk DLL `.txt` remain undecoded.
This is a **verified control-flow and span-layout improvement**,
not completion of the game-specific decoder or proof of the
four-word TEA key.

An independent public preservation project, [MW4Remastered
SafeDisc15020SecondLayer](https://github.com/Icehellionx/MechWarrior-4-Remastered/blob/main/src/MW4Remastered.Core/Install/SafeDisc15020SecondLayer.cs),
contains **different title-specific modifier profiles**. Their
existence corroborates the need to identify the *Crimson* profile
and not substitute another game's constants or assume one universal
second-layer transform.


### Most relocation fragments are not DWORD-aligned within their pages

An independent section-relative alignment audit on the original ICD
revealed an even stronger reason the earlier page-wide model cannot
be substituted for the relocation-aware Crimson workflow:

- **35,878 / 48,149** transformable page-local fragments (**74.5%**)
  start at a byte position **not divisible by four** relative to
  their 4-KiB page start;
- **315,414 / 440,422** transformable DWORDs (**71.6%**) are
  inside such unaligned fragments.

The original 1.5.5 `0x401F62` dereferences
`[ESI + 4*i]`, with `ESI` supplied as the **beginning of
each fragment** and `i` reset to zero. This permits DWORDs whose
addresses are not 4-byte aligned in the original page. A
page-wide `chunks_exact_mut(4)` loop, by contrast, can group
different bytes and would also overwrite original HIGHLOW operands.
The fragment adapter intentionally uses the actual byte address of
each non-relocated span; no page-alignment assumption is imposed
on the transformed words.

`--key-research` now independently verifies these two additional
original geometry counts; synthetic tests cover aligned/unaligned
fragments as well as a HIGHLOW crossing a page boundary.
This is further confirmation of **necessary input segmentation**,
not a title-specific EAX modifier implementation.


### Major breakthrough: all four TEA key words independently validated (2026-10-27)

The previously *provisional* 128-bit SafeDisc TEA key, constructed from
the original loader's raw key fields, the original 1.5.5
relocation-aware checksum candidate, the real ICD entry RVA and
the separately reported first word, is now verified against a
**known-plaintext oracle that does not require either game-specific
EAX modifier**.

The exact four 32-bit words, in original little-endian TEA order:

```text
7496538F E9C7914F CF10EA3F 82F5DE2F
```

**Why this check bypasses the still-encrypted DLL modifier routines:**
the historical second-layer loop `0x401F62` only touches complete
4-byte DWORDs of each non-relocated fragment. Its final 1–3
bytes (if present) are *not modified by that extra layer*.
However, the TEA stage can touch those bytes, including its
historical **overlapping last eight-byte block** on fragments with
9–15 bytes remaining.

Starting from the user-supplied SHA-256-verified unwrapped
`TESTME.EXE` `.text` bytes, reverse the exact TEA *decrypt
block call order*: encrypt the last stage's current block first,
then its overlapping tail block, then earlier complete blocks
in reverse. This produces the intermediate bytes that the
second-layer transform **must** have supplied to TEA. Compare
only the 1–3 trailing bytes known to have been unchanged by
that second layer against original protected `CRIMSON.ICD`
at matching offsets, skipping all 61,743 HIGHLOW DWORDs and
respecting 4-KiB boundaries.

Independent Python verification on these exact private
originals produced:

| Key variant | Full fragment tails identical | Individual trailing bytes identical |
| --- | ---: | ---: |
| Four recovered words above | **28,947 / 28,947** | **58,076 / 58,076** |
| Last word's lowest bit toggled | 39 / 28,947 | 177 / 58,076 |

The verified fragments span 9–4096 bytes and include the
overlapping-TEA cases; short fragments under 8 bytes and
divisible-by-four fragments are intentionally excluded from
this oracle, since they do not give an independent unchanged
post-second-layer tail. The 100% agreement across tens of
thousands of independently checkable bytes, combined with the
bit-flip negative control, **establishes the 128-bit key for
this precise Crimson Skies 1.0.2.0 protected payload** and
the TEA overlapping block ordering. This is a substantially
stronger statement than an import XOR check on the first word.

The read-only Rust inspector now implements
`tea_encrypt_block`,
`original_tea_reverse_fragment` and
`validate_tea_key_by_unchanged_fragment_tails`.
When the exact `--reference TESTME.EXE` and
`--key-research --reported-key 0x7496538F` are supplied, it
prints the oracle counts and refuses to label the four words
validated unless all original fragments and bytes match.
Without the reference, it still reports only a candidate.
Synthetic tests verify the textbook TEA vector, the inverse
of existing TEA decryption and overlapping-block order.

**Unresolved and unchanged:** the two *Crimson-specific*
`dplayerx.dll` EAX modifiers at `0x65BE` and `0x6621` are
not decoded or reimplemented. The tool cannot yet derive
the entire original plaintext from protected inputs alone and
does not generate or modify an executable. The reference is
read only, checked by exact SHA-256, and never embedded in
the repository.


### First reference-backed characterization of Crimson's unknown composite EAX modifiers

The independently confirmed TEA key allows inversion of the full
historical block sequence on *each* non-relocated fragment in the
user's hash-verified reference `TESTME.EXE`. For a fragment shorter
than eight bytes there is no TEA block, so its known plaintext
is already the second-layer output. This produces **observable
input/output pairs for the combined Crimson modifier functions**
without executing `dplayerx.dll`:

```text
original_first_DWORD -> SecondModifier(FirstModifier(original_first_DWORD))
```

This specific form holds for **the first DWORD of each fragment**,
because its word-index mask is zero. Subsequent DWORDs have nonzero
index masks *between* the two unknown modifier functions; one cannot
substitute those pairs into a single global composite lookup blindly.

An independent read-only Python analysis of the original matched
protected/reference pair gives:

| Observation | Count |
| --- | ---: |
| Non-relocated fragments of all lengths | **59,622** |
| Fragments with a complete first DWORD | **48,149** |
| First-DWORD composite modifier samples | **48,149** |
| Distinct input DWORDs | **31,799** |
| Distinct output DWORDs | **31,799** |
| Inconsistent outputs for repeated input DWORDs | **0** |
| Distinct inputs colliding on a known output DWORD | **0** |
| Tail bytes untouched by the modifier, all fragment lengths | **93,801 / 93,801 identical** |

The **93,801-byte control** includes all tails, even entire
non-relocated fragments shorter than four bytes. It extends the
58,076-byte TEA-key oracle (which intentionally required length>=8)
to *all* fragment lengths.

This is strong, independent evidence of the actual composite
transformation's consistency and one-to-one behavior across the
**observed** 31,799 input words, plus evidence that the
relocation-aware fragment boundaries and reconstructed TEA
overlap ordering are faithful. It is **not proof of a universal
32-bit permutation** and does not constitute a reusable implementation
of the unknown Crimson title-specific routines: the mapping was
obtained with the exact user-owned plaintext reference, and unseen
inputs cannot yet be decoded.

The Rust inspector now implements
`inspect_original_modifier_reference_pairs`: it reconstructs the
intermediate bytes only **in memory** from the verified reference and
confirmed TEA key, checks all 93,801 fragment-tail bytes against the
original protected ICD, and audits the consistency and reverse
uniqueness of first-DWORD pairs in ephemeral hash maps. It
asserts all expected original statistics and **prints counts only**,
never publishes any proprietary original DWORD values or stored
mapping table. A synthetic test confirms that inconsistent and
non-injective candidate pairs are rejected.


### Complete, index-aware composite EAX observations across 440,422 ICD DWORDs (2026-10-27)

The earlier read-only composite audit used only the first DWORD of
each fragment, where historical `0x401F62` constructs the
index-derived XOR mask `0`. It established 48,149 first-word
observations, 31,799 distinct input/output DWORDs and zero
contradictions.

An independent **second implementation in Python** now walks all
440,422 complete non-relocated DWORDs of the exact original
`CRIMSON.ICD`, with the matching SHA-256-verified
`TESTME.EXE`. For every fragment, it reverses the known TEA stage
on the reference bytes (including the legacy overlapping tail),
reconstructs the second-layer intermediate words, and considers
the original historical mask:

```text
index = word offset FROM THE BEGINNING OF THIS RELOCATION-FREE FRAGMENT
mask = (index << 24) | (index << 16) | (index << 8) | index
source = original_ICD_word XOR mask
observed_result = reference_after_inverse_TEA_word

observed_result = second_modifier(first_modifier(source) XOR mask)
```

**Important:** the inner XOR is between the two still-protected
modifiers, so this is a different function for each mask/index.
Combining all samples in a single unindexed `input -> output` map
would be misleading. Instead, use independent bidirectional maps
for each `(index, masked_input)` and `(index, observed_result)`.

Verified original-file results:

| Measurement | Count |
| --- | ---: |
| All complete second-layer DWORD observations | **440,422** |
| Distinct `(index, masked_input)` combinations | **361,480** |
| Repeated observations of an existing combination | **78,942** |
| Contradictory repeated results | **0** |
| Inverse collisions within the same index | **0** |
| Highest observed DWORD index within a fragment | **1,006** |
| Separate index-0 pairs (subset) | **31,799** distinct |

As a separate **negative control** using the last TEA word with its
least-significant bit toggled, the same offline all-word audit found
10 forward contradictions and 13 reverse collisions, and failed the
existing independent tail oracle. The correct four-word key has zero
such collisions and passes the 58,076-byte tail check. This provides
a broader *structural consistency* check on the previously validated
key and the exact relocation-fragment word-index reset.

The native Rust inspector now computes the index-aware evidence
**entirely in memory**, verifies all expected totals, and prints
only aggregate counts (no original or reference DWORD pairs).
Synthetic tests prove that different indices must be treated as
distinct functions. The corresponding GitHub Actions build passed
**80/80 tests on Linux and Windows**, and the downloaded Linux
inspect-only artifact was also run against all four exact original
files with `--key-research --reference-audit`; every new
reference-backed assertion passed.

**Limits:** these are observed finite samples of a 32-bit transformation,
not a reconstructed executable algorithm, recovered plaintext from
the protected inputs alone, or replacement for either encrypted
`dplayerx.dll` EAX modifier. A 361,480-pair local oracle cannot
be substituted for the title-specific functions when decoding
unseen inputs.


### Algebraic hypotheses for reference-backed index-zero modifier pairs (2026-10-28)

The exact-reference audit already establishes 31,799 distinct observed
index-zero mappings of the *composite* two-stage EAX modifier. A new
pure-Rust, **aggregate-only** probe tests three simple mathematical
families against these pairs: an arbitrary affine transformation over
GF(2) (32 input bits, 32 output bits), a bit rotation followed by
32-bit wrapping addition, and wrapping addition followed by rotation.

The affine check deterministically constructs an input-difference basis,
reports its rank (at most 32), and counts violations of the resulting
candidate model. The two rotate/add checks each test all 32 possible
rotation counts, with an offset inferred from one observed pair and
checked against *every* other distinct pair.

These measurements are computed **only when the exact known plaintext
TESTME.EXE reference is supplied and the existing key/tail consistency
checks pass**. They never print input/output pairs, candidate offsets,
matrices, or game bytes. Synthetic unit tests cover full-rank affine
maps, modular rotations/additions, nonlinear counterexamples, sparse
observations and order-independent results.

**No result from the real retail/reference pair has yet been measured**
for this additional probe. Even a complete fit would establish only
consistency on the observed sample; it would not independently prove an
algorithm on unseen words, reconstruct the two protected EAX functions
at RVA 0x65BE/0x6621 or enable SafeDisc unpacking.


### Joint-index GF(2) necessary-condition audit (2026-10-28)

The earlier `src/unsafedisc_algebra.rs` probe studies the observed
index-zero composite alone. The new, separate
`src/unsafedisc_indexed_affine.rs` experiment tests a stronger
necessary condition across **all** known index-aware pairs, not just
index zero. Original historical extra-layer semantics are:

```text
mask(i) = (i << 24) | (i << 16) | (i << 8) | i
observed y = F2(F1(masked_input) XOR mask(i))
```

**If both** unknown 32-bit EAX modifiers `F1` and `F2` are affine
maps over GF(2), their joint result must satisfy
`y = A(masked_input) XOR B(mask(i)) XOR c`, for GF(2) matrices
`A` and `B`. This remains true regardless of their undisclosed
individual constant offsets.

The read-only native implementation uses a deterministic 64-feature
Gaussian elimination over each unique pair keyed by
`(index,masked_input)`, with features
`masked_input | (u64(mask(index)) << 32)`. It reports the feature
rank (0..64) and the number of inconsistent dependent observations.
Inputs are sorted to make the counts independent of hash iteration
order. Only aggregate counts appear in output; the private reference,
intermediate values and fitted basis rows are never emitted.

Five synthetic tests validate original full-width masks at index
`0x100` and `0x3EE`, a consistent two-stage bit-affine family,
non-affine multiplication with carries, insertion-order independence,
and empty/single-observation behavior. Existing historical key and
unaltered-tail reference checks still gate this research path.

**No real-reference result has yet been obtained from this new test.**
A contradiction would refute *the combined hypothesis that both
modifiers are bit-affine* under the verified historical expression;
it would not identify which modifier is nonlinear. Even zero
contradictions would only establish finite-sample compatibility,
not recover either original routine nor decrypt unseen game data.


### Newly discovered index-mask aliases in the historical OR formula (2026-10-28)

A mathematical property of the original *full-width* x86 mask formula
was overlooked by earlier index-keyed observations:

```text
mask(i) = ((i << 24) | (i << 16) | (i << 8) | i) mod 2^32
mask(1)   = 0x01010101
mask(257) = 0x01010101       # same mask at a different index
mask(3) = mask(259) = mask(515) = mask(771) = 0x03030303
```

**The mask is not injective when the word index exceeds 255.**
For the exact already observed index range 0..1006 there are only
**572 distinct mask values** for 1007 integer indices, and **312 mask
values** are shared by at least two different indices. These counts
follow entirely from the public algebraic expression and do not
require private game bytes.

The apparent 32-bit mask is especially simple for the entire
`0 <= i < 1024` range. Write `i = a + 256*b` where
`0 <= a <= 255`, `0 <= b <= 3`. Because the historical
operation uses bitwise OR (not XOR) for every shifted copy, the
four little-endian mask bytes are
`[a, a|b, a|b, a|b]`. Thus the full 1024-index domain
contains only **576** distinct masks (rather than 1024).
The observed 0..1006 subset has **572**; the four missing
masks require excluded indices 1007..1023. Both totals and
every per-index equality are now checked in synthetic Rust tests.

This yields a stronger observational check, independent of any guessed
bit-affine model: if two samples have identical **(mask, input)** but
different integer indices, the output must be identical because the
original historical computation is deterministic. Likewise two
different inputs under the same mask cannot yield the same output
if both original EAX modifiers are permutations.

The optional exact-reference inspector now canonically regroups the
previously verified `(index,masked_input) -> observed_result` pairs by
`(mask(index),masked_input)`. It reports aggregate mask aliases,
canonical input-pair count, cross-index repeated observations and
forward/inverse conflicts. It uses deterministic input ordering and
never prints any private word, intermediate value or lookup table.
Three synthetic regression tests cover the exact mask alias count,
consistent/inconsistent repeated-input and inverse mappings, and
insertion-order independence.

**Unverified retail impact:** the aggregate results have not yet been
computed against the exact original private ICD+TESTME reference in
this session. Any contradiction is reported as a **warning**, not a
fatal error, pending independent validation of the historical state
semantics. The implementation does not alter game binaries or produce
an unpacker.


### Second-stage-only affine differential oracle (2026-10-28)

The first two affine probes implicitly assumed that **both** unknown
protected `dplayerx.dll` EAX functions were affine. This new, isolated
experiment can reject a much weaker hypothesis: **only the second
function `F2` is bit-affine**, while `F1` may be arbitrary/nonlinear.

Historical source-word expression:

```text
y(x,m) = F2(F1(x) XOR m)
m = (index << 24) | (index << 16) | (index << 8) | index
```

For a fixed masked input `x` observed at two different mask values
`m` and `n`, if `F2(z) = L(z) XOR c` then:

```text
y(x,m) XOR y(x,n) = L(m XOR n)
```

Both the unknown `F1(x)` and constant `c` cancel. This is a
*necessary condition* for F2 alone being GF(2)-affine and is
independent of the nature of F1.

The new `src/unsafedisc_second_stage.rs` experiment:

- Canonicalizes samples by `(masked_input, mask(index))`, detects
  conflicting output aliases and sorts consistently.
- Groups distinct masks per masked input, builds one XOR-difference
  relation against that input's first mask, and shares a single
  32-feature GF(2) elimination basis across **all inputs**.
- Reports only aggregate counts of inputs observed under more than
  one mask, difference relations, mask-difference rank and dependent
  contradictions. No input, output, fitted linear map or private code
  bytes are emitted.
- Has **nine synthetic tests**, including an arbitrarily nonlinear
  F1 with affine F2, a demonstrably nonlinear F2, absent cross-mask
  coverage, identical-mask aliases, conflicting aliases, ordering
  stability, exact mask-rank bounds and separate F2-injectivity controls.
- Separately counts cases where a **fixed input** observed under
  two **different masks** produces the same output. Such a collision
  contradicts injectivity of F2 without any hypothesis about F1.
  Importantly, this injectivity probe is independent of bit-affinity:
  a bit-affine map may still be noninjective. Both remain advisory
  until independently checked against the exact retail reference.

A fundamental limitation is quantified independently: the original
mask values in the observed index range `0..=1006` have **GF(2)
rank 10**, although they are 32-bit values and there are 572
distinct values. As a result the previous joint 64-feature affine
probe cannot exceed **42/64 rank** (32 source bits plus at most 10
mask bits). Any recovered affine candidate would leave at least 22
mask-related feature dimensions unconstrained by these observations.

**Interpretation:** If no input recurs under two different masks,
the F2-only test reports *inconclusive* and claims no information
about F2. A nonzero dependent-contradiction count, after resolving
any canonical alias conflicts and verifying actual original data,
refutes the F2-bit-affine hypothesis for the observed historical
expression. Zero contradictions only establishes sample consistency;
it does not reconstruct F2 or F1 and cannot decrypt new words.

The owner's exact original ICD/reference files are not accessible
in this session; no measured retail results are claimed for this
additional probe. All new code is read-only and restricted to the
already gated `--key-research --reference TESTME.EXE` path.


### Exact-original EAX hypothesis measurements and direct F2 counterexample (2026-10-28)

On 2026-10-28, the user supplied the exact source archive directly
to this conversation. All four original file lengths and SHA-256
digests were independently checked against the existing, pinned
retail 1.0.2.0 input/reference list, as was historical
`unSafedisc.exe`. The extra layer was analyzed **offline and
read-only**, using a standalone temporary C TEA-reversal helper and
separate Python scripts in a private local workspace. None of these
private binaries, recovered DWORDs or lookup tables is checked in.

The independent local analysis parsed all 558 relocation blocks,
confirmed 61,743 HIGHLOW DWORD positions in `.text`, reversed
the historical overlapping TEA operation with the previously
validated four-word key and reconfirmed:

| Test on exact `CRIMSON.ICD` and `testme.exe` | Measured result |
| --- | ---: |
| Relocation-free fragments in `.text` | 59,622 |
| Eligible DWORD observations | 440,422 |
| Unaltered tail bytes matching (all fragment shapes) | 93,801 / 93,801 |
| Unique index-keyed observations | 361,480 |
| Index-zero unique input/output observations | 31,799 |
| Index-zero GF(2) rank / contradicting pairs | **32 / 31,766** |
| `rotl(input,r)+k` candidates passing all first-word pairs | **0 / 32** |
| `rotl(input+k,r)` candidates passing all first-word pairs | **0 / 32** |
| Joint input/mask GF(2) rank / contradicting pairs | **42 / 361,437** |
| Different index values with identical mask, distinct mask values overall | **312 / 572** |
| Index-keyed observations after exact-mask canonicalization | **361,346** |
| Repeated canonical (mask,input) pairs across indexes | **134** |
| Canonical forward/inverse conflicts | **0 / 0** |
| Distinct masked inputs seen under two or more different masks | **13** |
| F2-only XOR-difference relations / rank / contradictions | **13 / 8 / 5** |
| Distinct mask-difference classes / repeated classes | **12 / 1** |
| Identical mask-difference classes with incompatible output deltas | **1** |
| Fixed-input output collisions across different masks | **0** |

All 13 multiply-masked inputs were observed under **exactly two**
different masks; 361,320 other distinct inputs were seen with only
one mask. These limited cross-mask samples are still powerful because
they include one *direct, independent* contradiction: the same
nonzero XOR mask difference leads to two different XOR output
differences from two separate input contexts. This is incompatible
with any **bit-affine F2**, whether F1 is affine or arbitrarily
nonlinear, under the recovered historical
`F2(F1(input) XOR mask)` expression.

The broader Gaussian-elimination F2-only test has 13 difference
relations, 8 independent mask-difference features, and 5
contradicting dependent relations. The zero output-collision result
does **not** prove F2 injective on unseen values; it only means no
injectivity contradiction was observed here. Likewise, the 42-rank
joint model is too small to identify a general 64-dimensional map,
but the large number of contradictions decisively rejects that
candidate family on these observations.

An independent pure-Python TEA forward implementation rechecked
40 fixed-seed sampled original `.text` fragments against the C
inverse-fragment helper with zero mismatches, including fragment
tails and overlapping-block cases. With a one-bit-corrupted fourth
TEA key word, the whole-fragment tail-match statistic fell to
35,902 / 93,801. The 35,725 untouched tail bytes of sub-8-byte
fragments explain why this differs from the previous restricted
`177 / 58,076` key-discrimination count.

The Rust inspector's exact pinned-reference path now asserts all
four **aggregate research signatures** (index-zero algebra,
joint affine, canonical mask aliases, F2-only differentials)
and reports them only after the established SHA-256, relocation,
tail, and wrong-key gates succeed. No real-game data files are
committed; the new signatures consist of counts only.

**Research consequence:** simple rotation/translation and general
GF(2) affine families are eliminated. The next investigation should
focus on nonlinear arithmetic, carries, bitwise Boolean networks,
state dependence or original instruction deobfuscation. No single
actual modifier function has yet been reconstructed and no
`unpack` operation is enabled.


### Conditional F1 XOR forest from repeated reference outputs (2026-10-28)

The newly verified exact-original 361,346 **canonical**
`(input, mask) -> output` observations display extensive reuse
of *outputs* across different inputs and masks. Suppose **F2 is
injective** over these values, while placing **no affinity or
linearity restriction on F2**. For two equal outputs,

```text
F2(F1(x0) XOR m0) == F2(F1(x1) XOR m1)
    => F1(x0) XOR F1(x1) == m0 XOR m1
```

This provides **25,168** XOR constraints on **37,025** distinct
source inputs in **11,857** connected components; the largest
component contains **140** source values. The graph is a forest:
no independent cycles occur, so there are **zero** cycle
contradictions. This is neither proof of F2 injectivity nor
recovery of F1; every connected component has an undetermined
XOR offset and almost all inputs remain unconstrained.

A synthetic-tested read-only Rust module,
`src/unsafedisc_first_stage.rs`, deterministically canonicalizes
the observations, groups equal outputs, tracks an XOR-weighted
union-find graph and measures an **order-independent** GF(2)
necessary condition on the candidate *F1*:

| Exact-reference conditional F1 measurement | Count |
| --- | ---: |
| Equal-output groups | 11,857 |
| XOR edge constraints | 25,168 |
| Source vertices participating in edges | 37,025 |
| Largest component | 140 |
| Consistent/conflicting cycles | 0 / 0 |
| Rank of source-input XOR differences | **32** |
| Rank of augmented (input difference, mask difference) equations | **42** |
| **Rank obstruction to affine F1, if F2 injective** | **10** |
| Repeated input-difference classes | 299 |
| Input-difference classes with contradictory mask differences | **52** |
| Conditional `rotl(x,r)+k` LSB-compatible rotations | **0 / 32** |
| Conditional `rotl(x+k,r)` LSB-compatible rotations | **0 / 32** |

Unlike the *number* of Gaussian-elimination residuals (which can
change with input ordering), **augmented rank minus input rank**
is order independent. The temporary independent Python test
confirmed augmented rank 42 versus input rank 32. This is a
strong conditional disproof of an affine F1, not of injectivity:
the graph has no cycles and currently cannot test F2 injectivity
beyond the earlier local collision probes.

This experiment has eight synthetic tests covering
affine-compatible mapping, direct repeated-delta contradictions,
inconsistent graph cycles, canonical aliases, deterministic
ordering, the empty/no-evidence case and simple modular-rotation
LSB necessary conditions. A wraparound addition has no carry
into its lowest bit. Consequently, for the two families
`F1(x)=rotl(x,r)+k` and `F1(x)=rotl(x+k,r)`, each possible rotation
must satisfy an elementary, offset-independent relation between
the low input-difference bit and one output-difference bit.
All 32 rotation counts in both families fail on the independently
measured retail/reference equal-output constraints **if F2
is injective**. This does not reject more general nonlinear
modifiers. The executable inspector
pins the aggregate genuine-title graph signature only after the
same original-game, SHA-256, TEA and reference gates have passed.
No plaintext, input/output table or executable bytes are stored.

**Next research:** deterministic F1 and F2 nonlinear candidate
families and authentic protected-DLL code decryption, rather than
mistaking evidence for a general 32-bit decoder.


### The DLL bootstrap's five PE directory selectors narrow the .txt initialization candidates (2026-10-28)

An independent instruction-by-instruction static analysis of the
SHA-256 pinned original `dplayerx.dll` traced the module attach
bootstrap beyond the previously recorded `0x987C -> A140` call.
Before the direct `.txt` call at RVA `0x989F -> 0x7212`,
`A140` calls these five setup helpers in this exact order:

| Call in A140 | Helper | Registered category | .txt2 selector wrapper | Selector |
| --- | --- | ---: | --- | ---: |
| `A181` | `B3D0` | 1 | `18890` | 11 |
| `A192` | `B1C0` | 1 | `18860` | 6 |
| `A1A3` | `AFB0` | 2 | `18830` | 5 |
| `A1B4` | `AE10` | 0 | `188C0` | 7 |
| `A1CB` | `AC10` | 7 | `188F0` | 9 |

The .txt2 `0x181E0` dispatcher bounds-checks the selector and
uses the original 11-entry jump table at RVA `0x18680`.
Each relevant branch calls **`0x18990`**, a section-header iterator.
`0x18990` treats a candidate RVA as a location inside a PE section:
`section.VirtualAddress <= RVA < section.VirtualAddress + (VirtualSize != 0 ? VirtualSize : SizeOfRawData)`.
The candidate is loaded from an **explicit PE data-directory field**
in the already-read NT header scratch area, rather than directly
from the category integer.

The original NT header begins at temporary `ESP+0x38`, and its data
directory starts at **NT+0x78**, hence scratch `ESP+0xB0`
denotes `IMAGE_DATA_DIRECTORY[0].VirtualAddress`. Mapping the
original case-specific `MOV` instructions gives the following
**exact retail DLL candidate selections**:

| Selector | Jump-table target | Header word read | Actual field value | Candidate DLL section |
| ---: | --- | --- | ---: | --- |
| 11 | `1841D` | directory 13 **Size** | `0` | None |
| 6 | `18357` | directory 2 **Size** | `0` | None |
| 5 | `18304` | directory 2 **RVA** | `0` | None |
| 7 | `185D5` | directory 0 **RVA** (export directory) | `0x1F350` | **`.rdata`** |
| 9 | `18486` | directory 10 **Size** | `0` | None |

The export-directory RVA `0x1F350` lies inside the original
DLL's `.rdata` virtual interval
`[0x1E000,0x1F3B0)`. None of the other four selector
inputs can lie inside an ordinary positive-RVA PE section in
the **same pinned DLL** because all four words are zero.

This is a new constraint on *which section-registration helpers
can find a candidate section* during the **static DLL initialization
path**. It does **not** establish that every helper is reachable,
that a non-null section lookup leads to a successful registry update,
or that no other decoder exists. In particular, the complete
behavior of `A140` and its descendants is still relevant to
restoring the protected `.txt` EAX routines.

The native `src/unsafedisc_dll_bootstrap.rs` research module
checks actual rel32 call destinations at A140 and each helper,
category/selector immediate opcodes, the .txt2 11-way jump-table
entries, exact case-specific stack reads, `0x18990` range-check
instruction opcodes and PE data-directory words. The
`--modifier-research` CLI presents only aggregate section selection
metadata after SHA-256 verification. **Four new synthetic Rust
tests** cover directory offset mapping, correct virtual
section bounds, raw-size fallback when VirtualSize=0, and
the original-like candidate distribution. No game or DLL
binary data is committed or executed.


### Clarification: A140's input-directory size guards eliminate two helpers, not four (2026-10-28)

Following the previous five-selector analysis, the exact
SHA-256-confirmed original x86 provides a more precise distinction
between **a helper's source inputs** and **the separate PE section
that helper later searches**.

At `0xA162`, `A140` computes a destination pointer
`record + 0x44`, then calls DLL `.txt2` routine `0x18770`
at `0xA167`. The latter calls the validated NT-header reader
`0x18940` and performs `MOV ECX,0x20; REP MOVSD`, copying
exactly **32 DWORDs = 128 bytes = 16 IMAGE_DATA_DIRECTORY
RVA/Size pairs** into the module record. Both the source PE
directory and record destination are instruction-backed.

This lets us trace which **source directory** each helper
receives, independently of its later internal section-selector
result:

| Helper | Registered category | Input from record+0x44 | Original source RVA | Original source size | Helper's first size guard | Later selector result |
| --- | ---: | --- | ---: | ---: | --- | --- |
| `B3D0` | 1 | directory 12 (IAT) | `0x1E000` | `0x11C` | Nonzero: continues | Selector 11 → none |
| `B1C0` | 1 | directory 1 (Import) | `0x1ED14` | `0x3C` | Nonzero: continues | Selector 6 → none |
| `AFB0` | 2 | directory 2 (Resource) | 0 | 0 | **Immediate zero-size exit** | Selector 5 would yield none |
| `AE10` | 0 | directory 0 (Export) | `0x1F350` | `0x60` | Nonzero: continues | Selector 7 → `.rdata` |
| `AC10` | 7 | directory 9 (TLS) | 0 | 0 | **Immediate zero-size exit** | Selector 9 would yield none |

**Crucial correction:** Four selectors returning zero **does not
mean four initializer helpers do nothing**. `B3D0` and `B1C0`
both receive **nonzero source lengths**, call `A2A0` to update
a category-tracking structure **before** performing their later
section lookup, and may have effects even if that lookup returns
no section. Only `AFB0` and `AC10` are statically gated
off before their `A2A0` calls due to zero source lengths.
Consequently this particular five-helper path leaves **three
candidate active helpers**, conditional on reaching `A140`
with the expected initialized record.

The extra Rust `src/unsafedisc_dll_bootstrap.rs` assertions now
check the actual `A140` loads from record offsets
`0xA4/0xA8`, `0x4C/0x50`, `0x54/0x58`,
`0x44/0x48`, and `0x8C/0x90`;
the `0x18770` copy instructions; each helper's `TEST`
on the input Size; and exact directory values read from
the verified original PE headers. Two synthetic tests verify
record-slot geometry and zero-size early exits, independent
of the local original executable.

This is still **not a runtime execution trace**, and neither
the two original encrypted EAX functions at `0x65BE` /
`0x6621` nor the full protected `.txt` byte restoration
is yet reconstructed. The source originals remain private.


### Complete ordered A140 bootstrap: final category-4 .txt registration candidate (2026-10-28)

Beyond the five now-classified data-directory-specific helpers,
the exact original DLL `A140` invokes three additional
**section class iterators** after them, in the following order:

| A140 call RVA | Iterator | Original selector | Category registered | Original DLL outcome |
| --- | --- | ---: | ---: | --- |
| `0xA1D4` | `0xAB80` | 1 (CODE+EXECUTE) | 4 | First matching section: **`.txt`** |
| `0xA1DD` | `0xAAF0` | 2 (writable DATA) | 5 | **`.data`** |
| `0xA1E6` | `0xAA60` | 10 (read-only initialized DATA) | 6 | **None**, after two PE-directory-start exclusions |

The exact selector and category PUSH immediates, helper E8 targets,
and downstream `A2A0` category-update E8 calls were checked against
the original SHA-256-pinned DLL's executable instructions.
The section choices are independently reconstructed from the
verified original `IMAGE_SECTION_HEADER.Characteristics` flags:

- Selector **1** matches the three code/executable sections
  `.txt`, `.text`, and `.txt2`; the iterator takes **the first**,
  `.txt` at RVA `0x1000`. This is the **encrypted**
  section containing the EAX functions, **not a plaintext dump**.
- Selector **2** matches one writable initialized DATA section
  `.data`, at RVA `0x20000`; it is not a PE directory start,
  so the directory-exclusion rule does not remove it.
- Selector **10** initially matches `.rdata` and `.reloc`,
  but both have RVA equal to a nonzero PE data-directory
  starting RVA (`0x1E000` is IAT, `0x27000` is
  base relocations). The iterator excludes both and finds
  **no** surviving section in this original build.

**Combined with the five directory-specific helpers**, this
gives an eight-step attach initialization order, plus a later
direct call into protected `.txt` at original RVA `0x989F
-> 0x7212`. The ordered discovery focuses our next analysis
on the category-4 registry/segment mechanisms (`A2A0` and
helpers `B850`, `B880`, `B8E0`), and on what, if
anything, makes the protected code executable prior to this
direct call. **Section registration is not decryption**;
the original module loader's reachability and actual
runtime state remain unproven.

The read-only Rust module now verifies the A140 final
three E8 calls, each selector/category immediate, all
six downstream original targets and the exact section
matching/exclusion totals, then prints the outcomes
from `--modifier-research`. Two synthetic unit tests
cover the real original-like section table and a
counterexample where a later section survives the
directory-start filter. Neither original protected
code nor generated decrypted data is committed.


### Original A2A0 category handlers have an ordered, short-circuiting dispatch (2026-10-28)

A new local static disassembly of the exact original DLL identifies
the control-flow contract behind the previously mapped category
registrations. At original RVA `0xA2A0`, the dispatcher first
calls `0xB5D0` (module record lookup). A missing record returns
immediately. Otherwise it prepares the stored category pointer
and module data and calls three category-related helpers in a
strictly ordered **short-circuiting** chain:

| Original call RVA | Callee | Entry condition |
| --- | --- | --- |
| `A2D3` | `B850` | Registered module exists |
| `A2E4` | `B880` | `B850` returns AX=0 |
| `A2F5` | `B8E0` | Both `B850` and `B880` return AX=0 |

The original branch opcodes `A2B1 TEST EAX,EAX / JE`,
`A2DB TEST AX,AX / JNE` and `A2EC TEST AX,AX / JNE`
are all exact-file checked. The same dispatcher also
calls `BF70` at RVA `A2C3` before entering these
handlers; no semantic claim about BF70 is necessary yet.

This supplies a concrete, testable interpretation of the
A140 registration work: finding a nonzero-sized source directory
only makes registration **eligible**. The actual category
operation can stop at B850 without touching B880 or B8E0,
depending on the previous return value. It would therefore
be wrong to infer that all three handlers run for every
registered category.

The new `src/unsafedisc_dll_category_dispatch.rs`
module verifies the exact CALL targets and gating branch
bytes under the already enforced original DLL SHA-256,
and runs **four synthetic tests** for missing records,
nonzero-first result, nonzero-second result and
both-zero fallthrough. The CLI prints only the guarded
static summary. This remains offline static analysis, not
execution of the protected DLL, a reconstructed EAX
routine, or a plaintext page.


### Category-4 .txt registration reaches a range-bookkeeping dispatcher, not yet a decoder (2026-10-28)

With the eight original `A140` initialization calls now
mapped to their PE section sources, we traced the **next**
layer of category-4 registration on the exact user-supplied
`dplayerx.dll` without executing it:

```text
A140 -> AB80 (selector 1 -> first CODE+EXECUTE = encrypted .txt)
             -> A2A0 (register category 4)
                  -> B850 (first range/list strategy)
                  -> B880 (second range/list strategy)
                  -> B8E0 (fallback range/list strategy)
```

These three `A2A0` strategies contain further bounded,
verified original E8 calls to helper families:

- `B850 -> BAD0`;
- `B880 -> BC40`, `C2F0`, `B9D0`;
- `B8E0 -> BC00`, `B7C0`, `B940`.

The concrete `C2A0` helper requests a **12-byte allocation**
using `B610`, then stores three DWORD fields at offsets
`+0`, `+4`, `+8` of the new object. The `C2F0`
helper compares an input against a range's start DWORD,
stores the unsigned minimum, and adds an extent to the
next DWORD using ordinary 32-bit wrapping arithmetic.
These are specifically **metadata/list/range** operations
in the observed helpers, not a validated byte decryption
transform. The static category-4 selection points into
encrypted `.txt`; it does not on its own produce an
executable copy of that section.

The native `--modifier-research` gate now verifies the
**11 call targets**, the original byte instructions for
the 12-byte node allocation/stores and two-word range
update, as well as two synthetic range/metadata tests.
The originally verified unSafeDisc reference is unchanged,
and no original game bytes or decoded blocks are written.

**Limits:** these exact opcodes support classification of
the immediate helpers but do not exhaust all nested
calls, opaque DLL startup decisions or changes made
elsewhere in memory. A later or indirect step must
still account for the unreadable on-disk modifier RVAs
`0x65BE` and `0x6621`. We do not claim that
the whole `A2A0` tree has been proven decryption-free.


### Native original-file verification of the completed bootstrap audit (2026-10-28)

The exact, SHA-256-matched owner-supplied retail
`CRIMSON.EXE`, `CRIMSON.ICD`, `dplayerx.dll`,
and known plaintext `testme.exe` were inspected with
the **actual native Linux `unsafedisc-linux` executable**
downloaded from GitHub Actions research CI. The
read-only binary at the updated research branch head
passed both of the following genuine-file invocations,
returning **exit status zero**:

```bash
./unsafedisc-linux inspect --loader CRIMSON.EXE \
  --icd CRIMSON.ICD --dplayerx dplayerx.dll --modifier-research

./unsafedisc-linux inspect --loader CRIMSON.EXE \
  --icd CRIMSON.ICD --dplayerx dplayerx.dll \
  --reference testme.exe --key-research
```

The `--modifier-research` run passed all actual protected
DLL static opcode/call/section-layout checks, including:

- The five PE directory source-copy and zero-size guard
  results: directory sources `12, 1, 2, 0, 9`, only
  `12, 1, 0` with nonzero extents.
- The three final ordered selector/category pairs:
  `(1,4)->.txt`, `(2,5)->.data`, `(10,6)->none`.
- The `A2A0` range registry call tree and the exact
  `C2A0` 12-byte allocator and `C2F0` range
  arithmetic opcodes.

The `--key-research` invocation separately passed all
existing **four original/reference EAX aggregate signatures**,
including the conditional F1 XOR forest, the F2-only
mask-differential counterexample, and the independent TEA
fragment-tail oracle. No protected DLL code was executed;
the native inspector only **read** the private originals
from a local scratch directory. No original file or
recovered DWORD sample was uploaded as a GitHub artifact.

This closes the previous gap between GitHub's
**synthetic-only CI** and original-file Rust assertions.
It does **not** imply a working SafeDisc decoder or
known plaintext at the two protected EAX RVAs.


### B850/B880/B8E0 form a range-record and linked-list subsystem (2026-10-28)

Deeper **local read-only x86 disassembly** of the exact original
`dplayerx.dll` now distinguishes the A2A0 category-dispatcher's
three handlers from the memory *byte-transforming* routines.
The three functions form a **range-record / linked-list
management path**. The following original E8 call edges were
independently confirmed by recalculating signed rel32
destinations from the original pinned DLL:

| Call at RVA | Callee RVA | Observed role |
| --- | --- | --- |
| `B865` | `BAD0` | Search existing range/list record |
| `B8A1` | `BC40` | Find a compatible neighboring record |
| `B8B4` | `C2F0` | Adjust start and accumulated length |
| `B8BE` | `B9D0` | Update record/list position |
| `B8F6` | `BC00` | Search for a candidate record |
| `B90F` | `B7C0` | Allocate and initialize a range record |
| `B919` | `B940` | Link the record into a list |
| `BAF5` | `C3D0` | Compare a candidate against a record start |
| `BB08` | `C350` | Check range containment and category |
| `BB29` | `B9D0` | Update an existing list entry |
| `BC5E` | `C4B0` | Check same-category adjacency |
| `BC6F` | `C3D0` | Order range records |
| `BC12` | `C3D0` | Order range records |
| `B7C8` | `B610` | Allocate **0x0C-byte** record |
| `B7F3` | `C2A0` | Initialize new record's range data |
| `B964` | `B980` | Post-insertion linked-list check |
| `B9E6` | `C4F0` | Test candidate interval adjacency |
| `B9FC` | `C530` | Compare interval endpoints |
| `BA12` | `C310` | Expand a recorded interval's span |
| `BA1F` | `BA90` | Update linked-list relationships |

Additional original **opcode** evidence:

- `B850` stores a successful `BAD0` result in record
  field `+8` (`B871: MOV [ESI+8],EAX`).
- `B880` stores a selected/merged record in `[EDI+8]`
  (`B8C6`); `B8E0` updates `[EBX+8]` (`B924`).
- `C2F0` lowers the start DWORD if needed and **adds**
  a length to DWORD `+4`. This is a mathematical
  interval operation, not a bytewise code transform.
- `C4B0` first checks category DWORD `+8`, then
  tests whether interval endpoints **touch** in either
  order. Distinct category identifiers are not merged.
- `B940` updates a four-pointer predecessor/successor
  chain using explicit writes to node fields `+4`
  and `+8`.
- `B7C0` requests a 12-byte allocation and initializes
  range-record links via `C2A0`.

The newly isolated `src/unsafedisc_dll_ranges.rs` checks
**20 exact original E8 targets** and **15 original instruction
patterns**; every one was also separately rechecked locally
against the user's SHA-256-pinned DLL. It provides **five
synthetic Rust tests**: unique bounded call sites, contiguous
range joining in both directions, rejection of overlap/category
mismatch, handling empty/overflowed synthetic ranges, and
coverage of the three categories' helper calls.

The synthetic arithmetic helper **rejects overflow** for safety,
even though original 32-bit x86 ADD instructions are wrapping;
it is *not* claimed as an instruction-accurate full emulator.
This still leaves potential side effects inside allocator/list
helpers and unknown runtime branches. Nonetheless, the
identified core of this subsystem is **range classification
and linked-list bookkeeping**, not a recovered executable
`.txt` decoder.

**Research priority moves downstream:** determine where the
registered memory regions are later consumed by the actual
byte transformer / code-restoration path. In particular,
the fact that `B850`, `B880`, and `B8E0` are
invoked from A2A0 does **not** establish that any of
them decrypts modifier RVAs `0x65BE` or `0x6621`.


### A140 ignores individual helper outcomes after the module-record gate (2026-10-28)

One more control-flow distinction from the exact original
`dplayerx.dll` changes the search priorities. The original A140
initializer tests the result of `B540` at
`A159: TEST ESI,ESI` and `A15B: JE A1F4`.

- If B540 **fails to return a record**, A140 skips all eight
  follow-on calls and exits with AX=0.
- If B540 returns a record, A140 proceeds through its **five
  source-directory helpers** (`B3D0`, `B1C0`, `AFB0`,
  `AE10`, `AC10`) and **three section-class iterators**
  (`AB80`, `AAF0`, `AA60`) **in order**.
- Between these eight original calls, A140's instruction
  sequence performs argument preparation and stack cleanup
  without testing the callees' return values. At
  `A1EE: MOV EDI,1` and `A1F4: MOV AX,DI`, it
  returns success (AX=1) if it reached the end.
- This does **not** mean that every helper performs actual
  changes. `AFB0` and `AC10` have independently
  established zero-size guards; the A2A0 category handler
  also branches internally based on its own returns.

The distinct conclusions are: **A140 itself does not
stop because a directory-specific helper has no matching
section**, whereas **A2A0 can short-circuit within its
range-list handlers**. Confusing these two layers would
incorrectly discard potential initialization work.

The native `unsafedisc_dll_bootstrap.rs` now verifies
the original `A159/A15B` branch and `A1EE/A1F4/A1FA`
return instructions, and adds two synthetic tests for
absent/present module-record control flow. This is a
static conditional proof, not an observed successful
Windows startup or .txt plaintext recovery.


### Two more original .txt2 call sites can invoke A140 with other descriptors (2026-10-28)

Searching the *other readable executable sections* of the original
SHA-256-verified `dplayerx.dll` for instruction-aligned direct
`CALL rel32` references to the verified RVA `A140`
revealed **two additional call sites inside `.txt2`**:

| Source section and call RVA | Prior original instruction | Destination | Stack cleanup |
| --- | --- | --- | --- |
| `.txt2` `0x1A137` | `1A133: LEA ECX,[EBP-0xC]; PUSH ECX` | `0xA140` | `ADD ESP,8` |
| `.txt2` `0x1A299` | `1A295: LEA EDX,[EBP-0xC]; PUSH EDX` | `0xA140` | `ADD ESP,8` |

The previously known caller `.text:0x987C` uses
the DLL-global descriptor prepared with
`GetModuleHandleA("dplayerx.dll")`. By contrast,
these two call sites pass addresses of **stack-local
descriptors**. The exact content of those descriptors
at runtime is **not known**, nor are the opaque paths
to these calls proven reachable. The static observation
therefore establishes *potential repeated initialization
with different descriptor inputs*, not confirmed
processing of another module or an additional decoder.

This matters for scope: **all pinned category and
PE-directory counts obtained from the original
`dplayerx.dll` header apply to that particular DLL
descriptor**, not automatically to these other possible
A140 invocations. A different module's PE directory,
if supplied by a reachable caller, could lead to a
different set of eligible registration helpers.

The research CLI now verifies original rel32 targets,
both exact `LEA/PUSH` argument setups and matching
`ADD ESP,8` cleanups, along with a synthetic test of
the three recognized direct call-site RVAs. No .txt
code is executed and no runtime descriptor values
are invented.

A next static investigation can trace the descriptor
fields populated before these two calls and determine
whether either route refers to the **game's ICD** or
only to other DLL-internal sections. That remains an
open question rather than an asserted finding.


### A second .txt2 initializer uses a caller-supplied decoded module name (2026-10-28)

The previously identified `.txt2:0x1A299 -> A140`
call has a concrete, independently checked **upstream
descriptor preparation** sequence in the same readable
`.txt2` routine:

| Original RVA | Exact instruction / verified effect |
| --- | --- |
| `1A243` | `MOV EAX,[EBP+0xC]`: load caller-supplied pointer |
| `1A246` | `PUSH EAX` |
| `1A247` | `CALL E25C`: existing decoded-label helper |
| `1A24F` | `PUSH EAX`: pass decoded-name pointer |
| `1A250` | `CALL DWORD PTR [0x008FE098]`: **same verified GetModuleHandleA IAT slot** used at `0x986A` |
| `1A256` | `MOV [EBP-8],EAX`: save returned module handle |
| `1A259` | `PUSH 0x67`: descriptor read mode |
| `1A25B` | `LEA ECX,[EBP-0xC]; PUSH ECX` |
| `1A25F` | `CALL A550`: mode/descriptor setup |
| `1A295` | `LEA EDX,[EBP-0xC]; PUSH EDX` |
| `1A299` | `CALL A140`: module registry initialization |

All three E8 destinations (`E25C`, `A550` and
`A140`) and the IAT operand have been independently
verified on the real SHA-256-pinned DLL. This caller
uses the same already-confirmed Windows API identity
as the ordinary `dplayerx.dll` setup, but its encoded
module-name pointer **originates in the caller's arguments**.

**Limit:** the actual string supplied by that caller
has **not** been established here. Nor have the complete
opaque branch predicates been proved to reach every
instruction. The resolved handle could be for another
DLL or another loaded module; no particular game process
or ICD address can yet be asserted.

The native `unsafedisc_dll_bootstrap.rs` inspector now
validates this chain and the identity of the referenced
`GetModuleHandleA` IAT slot and includes a synthetic
stage-order regression. This strengthens the reason to
trace callers of the `.txt2` function before generalizing
the exact original DLL's zero-sized directory findings
to all possible `A140` invocations.


### Original .text caller resolves the .txt2 name argument to dplayerx.dll (2026-10-28)

Further exact-byte examination identifies an **instruction-aligned
caller of the .txt2 routine at 0x1A20D**:

```text
.text RVA 0x981A: PUSH 0x00900030
.text RVA 0x981F: PUSH 0
.text RVA 0x9821: CALL 0x1A20D
.text RVA 0x9826: ADD ESP,8
```

The pushed pointer is the original PE preferred base
`0x008E0000` plus data RVA `0x20030`.
The independent, already verified E25C metadata decoder
produces the literal module name **`dplayerx.dll`**
from the hash-pinned .data bytes at this RVA. In the
callee `0x1A20D`, the `[EBP+0xC]` argument is
passed to E25C and then to GetModuleHandleA; the
local mode-0x67 descriptor reaches A140 at `0x1A299`.

Therefore this **specific** cross-section chain
`.text 0x9821 -> .txt2 0x1A20D -> .text A140`
uses **dplayerx.dll** as its requested module
name, not an unspecified game module. This
**narrows an earlier uncertainty**, but does not
prove the chain runs on every attach path or that
the other .txt2 caller at `0x1A137` uses the same
string. Nor does it establish that the loaded module
handle has a particular runtime value.

The native inspector now verifies the precise
`PUSH/PUSH/CALL/ADD ESP` instruction sequence,
reads the pushed preferred VA, maps it to the
original .data RVA, decodes the real metadata
to `dplayerx.dll`, and checks the already-known
`1A247 -> E25C`, GetModuleHandleA IAT,
`1A25F -> A550`, `1A299 -> A140`
call sequence. A synthetic regression verifies
base-relative RVA arithmetic and instruction offsets.

The **remaining** A140 invocation at 0x1A137
requires independent tracing before any further
module-specific generalization. All research is
offline and read-only.


### First .txt2 A140 caller uses the process's main executable module (2026-10-28)

The remaining direct A140 call, at `.txt2:0x1A137`,
has now been traced to a distinct descriptor initialization.
Unlike the other caller, which receives the encoded
literal `dplayerx.dll`, this first `.txt2` routine
constructs a **process-main-module descriptor**.

The independent local original-PE import parser maps
the following **real KERNEL32.dll import bindings**:

| DLL RVA | Original instruction | Import / effect |
| --- | --- | --- |
| `1A0CE` | `CALL DWORD PTR [0x008FE004]` | `GetCurrentProcess` |
| `1A0D4` | `MOV [EBP-0xC],EAX` | Save process pseudo-handle in descriptor field 0 |
| `1A0F7` | `PUSH 0` | NULL module-name argument |
| `1A0F9` | `CALL DWORD PTR [0x008FE098]` | `GetModuleHandleA(NULL)`, main executable module |
| `1A0FF` | `MOV [EBP-8],EAX` | Save executable image base in descriptor field +4 |
| `1A116` | `PUSH 0x67` | Mode 0x67 |
| `1A118` | `LEA EAX,[EBP-0xC]; PUSH EAX` | Pass descriptor to A550 |
| `1A11C` | `CALL A550` | Initialize descriptor's mode |
| `1A133` | `LEA ECX,[EBP-0xC]; PUSH ECX` | Pass descriptor to A140 |
| `1A137` | `CALL A140` | Register the process-main-module descriptor |

**Important distinction:** `GetCurrentProcess`
returns a process **pseudo-handle**, not a module
base address. `GetModuleHandleA(NULL)` provides
the loaded executable module handle/base if the API
call succeeds. The original descriptor fields are
therefore two separate kinds of Win32 handles and
must not be conflated.

This resolves the prior uncertainty about **both
visible `.txt2` A140 call sites**:

- `0x1A137` initializes a descriptor that targets
  the process main executable module via
  `GetModuleHandleA(NULL)`.
- `0x1A299` receives the `dplayerx.dll`
  name from its caller `.text:0x9821` and
  resolves that DLL's module handle.

Both paths still contain opaque control-flow sections,
so static caller/argument evidence is **not** proof
that both actually execute in every Windows attach
scenario. In particular, the main executable may be a
SafeDisc launcher image or may have undergone other
initialization by the time the call runs; no unprotected
game-code RVA can be inferred from the API identity
alone.

The native inspector verifies both original IAT
operands, resolves the *import symbols by name* from
the PE import directory, checks argument setup and
exact `A550/A140` E8 targets, and includes a
synthetic test that distinguishes process and module
handle roles. No executable is run or changed.

**Next major question:** given the main executable's
actual PE section directory at that point in the
startup sequence, which of the A140 registration
paths and categories would act on the protected
sections? This is the strongest current lead toward
understanding the transition from module registration
to the protected code at `.txt:0x65BE/0x6621`.


### The on-disk CRIMSON.EXE main-module registration profile differs from dplayerx.dll (2026-10-28)

With the new exact-original `.txt2` caller proof,
`GetModuleHandleA(NULL)` refers to the process main
executable module, not the DLL. A separate read-only
parser therefore evaluated the **original on-disk
SafeDisc loader `CRIMSON.EXE`**, verified by the
pre-existing exact SHA-256 and PE32 size gates.

The five original A140 directory-specific helper inputs,
in the established order **IAT (12), Imports (1),
Resources (2), Exports (0), TLS (9)**, differ:

| Input source directory | `dplayerx.dll` Size | `CRIMSON.EXE` loader Size | Initial nonzero guard |
| --- | ---: | ---: | --- |
| IAT (12) | `0x11C` | **`0x1DC`** | Both pass |
| Import (1) | `0x3C` | **`0x64`** | Both pass |
| Resource (2) | `0` | **`0x7AA`** | **Loader only** |
| Export (0) | **`0x60`** | `0` | **DLL only** |
| TLS (9) | `0` | `0` | Neither passes |

The final three section-class selectors, evaluated
against each original image's **own on-disk** section
headers and data-directory starts, give:

| Registered category / selector | DLL profile | Main SafeDisc loader profile |
| --- | --- | --- |
| 4 / selector 1 (code+execute) | `.txt` (3 matching) | `.txt` (3 matching) |
| 5 / selector 2 (writable data, excluding directory starts) | `.data` (1 matching) | `.data` (1 matching) |
| 6 / selector 10 (read-only data, excluding directory starts) | None (2/2 excluded) | **None (3/3 excluded)** |

For the loader, the three read-only candidates
`.rdata`, `.rsrc` and `.reloc` are all
eliminated by their exact directory-start RVAs
respectively IAT `0x29000`, resources
`0x34000` and base relocations `0x58000`.

**Strict limit:** these are PE header values from
`CRIMSON.EXE` as stored on disk, not a runtime
dump of `GetModuleHandleA(NULL)`. SafeDisc could
have altered the mapped main executable or its
headers before the registry call. They are also
**not** the corresponding fields of
`CRIMSON.ICD`, whose unwrapped layout is
different. A class selector finding a section
is not proof that subsequent A2A0 registration
or any code transform succeeded.

A new native `audit_original_loader_header`
routine in `src/unsafedisc_dll_bootstrap.rs`
is called only after the native CLI independently
hash-verifies the protected loader input. It checks
the original five source directory sizes, constructs
the section list, applies the same static selector
and directory-start exclusions as for the DLL,
and asserts the expected **loader-specific**
results. A synthetic unit test covers category
selection and the resource-versus-export difference.
The CLI labels these results as **on-disk-only**.
The game/DLL files are neither executed nor modified.
