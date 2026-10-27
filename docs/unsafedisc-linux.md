# unSafeDisc Linux — experimental native port for Crimson Skies

This branch begins a **native Rust implementation for Linux and Windows** of the unSafeDisc
1.5.5 research workflow for the *Windows PC 1.0.2.0* release of **Crimson Skies**.

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
