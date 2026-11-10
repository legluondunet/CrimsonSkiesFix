# Restore original long ZBD filenames using the Microsoft retail setup table

**Narrow scope:** only names inside the original eight `ZBD/C*.CAB` chapter
trees, and deletion of those eight redundant CAB archives after a complete
verified extraction. No game-root filenames, DLLs, EXE, patching, or registry
settings are changed in this milestone.

## Definitive source: original German `SETUP.EXE`

The owner provided original disc files on 2026-11-10. SHA-256:

- `SETUP.EXE`: `cf9ce3f28feb5a4e82909a00d1b351dec62343e2897997a450c83f0f61d62839`
- `SETUPENU.DLL`: `f9d9a2e65ce504a234bb9cbd417d874b6cc2f98451ffd53da46da6f89f68acb8`
- `MCP.DLL`: `9a2c0a98843e3feb20ce42b5f8721451de78f51b57717725cd479c254d863f37`

These match the **DE CD file-content profile** already in the repository.
The original `SETUP.EXE` contains a consecutive literal source-path /
installed-target-path list for chapter files. It lists **178 chapter
files**, including **40 explicit differing DOS-short-to-retail-long filename
pairs**, and six direct (non-CAB) ZBD files. The `cab` Rust library only
exposes the name *stored in the CAB*, not the separately chosen
retail destination name. The decompressor itself was not losing the
long filename. The official installer was renaming selected files.

Examples read directly from the original setup (uppercase is from CD,
lowercase is the retail destination):

| Original source filename | Retail destination filename |
|---|---|
| `C1/RTEXTU~1.ZBD` | `C1/rtexture15.zbd` |
| `C1B/RTEXTU~1.ZBD` | `C1B/rtexture11.zbd` |
| `C1C/RTEXTU~1.ZBD` | `C1C/rtexture10.zbd` |
| `C2/RTEXTU~1.ZBD` | `C2/rtexture14.zbd` |
| `C2B/RTEXTU~1.ZBD` | `C2B/rtexture2.zbd` |
| `C2B/RT60AC~1.ZBD` | `C2B/rtexture9.zbd` |
| `C3/RTEXTU~1.ZBD` | `C3/rtexture12.zbd` |
| `C4/RTEXTU~1.ZBD` | `C4/rtexture14.zbd` |
| `C5/RTEXTU~1.ZBD` | `C5/rtexture14.zbd` |

The other name pairs for `C1/C1B/C1C/C2/C3/C4/C5` are
`RTEXTU~2`→`rtexture2`,
`RTEXTU~3`→`rtexture4`,
`RTEXTU~4`→`rtexture6`, and
`RT6C9C~1`→`rtexture8`.
**C2B is exceptional:** its `RTEXTU~1..4` are
`rtexture2/4/6/8`, and `RT60AC~1` becomes `rtexture9`.

All **40 pairs** are explicitly encoded in
`installer/src/chapter_cabs.rs` and tested. No heuristic guesses are
made. The first stage extracts the original byte-identical contents,
and the later finalization stage independently verifies every original
CAB member before **renaming only the known DOS 8.3 aliases**, validating
their unchanged SHA-256, writing the sealed chapter-name/size/hash
manifest, and removing all eight source CABs. `--verify-install`
works against this final canonical-name manifest.

## Three original Microsoft setup tables cross-checked (2026-11-10)

After the German setup was inspected, the owner provided the original FR
`INSTALL.EXE` and USA/Europe `SETUP.EXE` (with their matching
`SETUPENU.DLL`). Their sizes and SHA-256 exactly match the pre-existing
full original-CD manifests in `installer/profiles/`.

| Edition | Installer (bytes) | SHA-256 |
|---|---|---|
| Germany | `SETUP.EXE` (487477) | `cf9ce3f28feb5a4e82909a00d1b351dec62343e2897997a450c83f0f61d62839` |
| France | `INSTALL.EXE` (491573) | `30895f3dc36bce45632beb6cbc42a350f06c5c4b2fabb7330d9fc28b1906bdc9` |
| USA/Europe | `SETUP.EXE` (487477) | `04e26ce391bb9f2a5d81518588aa0f7d553a95c5202a742d58af54b4070a89df` |

Their internal ZBD file-copy directives contain **184 source→target
records each**, comprising **178 chapter files plus 6 standalone ZBDs**.
Each edition has exactly **40 short→long chapter filename differences**.
The **installed destination path sets are identical across all three
editions**. The FR and DE original source paths also agree, ignoring
ASCII case. The USA/Europe disc has **five DIFFERENT source aliases**
for chapter `C2B` alone:

| Canonical installed file | FR / DE CAB member | USA / Europe CAB member |
|---|---|---|
| `C2B/rtexture2.zbd` | `C2B/RTEXTU~1.ZBD` | `C2B/RTEXTU~2.ZBD` |
| `C2B/rtexture4.zbd` | `C2B/RTEXTU~2.ZBD` | `C2B/RTEXTU~3.ZBD` |
| `C2B/rtexture6.zbd` | `C2B/RTEXTU~3.ZBD` | `C2B/RTEXTU~4.ZBD` |
| `C2B/rtexture8.zbd` | `C2B/RTEXTU~4.ZBD` | `C2B/RT6C9C~1.ZBD` |
| `C2B/rtexture9.zbd` | `C2B/RT60AC~1.ZBD` | `C2B/RTEXTU~1.ZBD` |

**Do not apply the German C2B source map to the USA original CD!**
The installer now selects the appropriate exact original setup table
using the edition already authenticated against the source CD's hashed
file manifest (`C_SKIES_DISC_001` / `C_SKIES_FR_001` /
`C_SKIES_DE_001`). Unknown short aliases fail closed.

This comparison also confirms that the older handwritten claim that all
three source alias tables were equal would have been incorrect. The
`C2B/rtexture9.zbd` target is explicitly listed by all three
original setups, although it was absent from the owner's historic
reference installation tree. Do not arbitrarily delete that file.

The three executables also contain Windows registry API imports and
registry-related setup strings, but their execution conditions and
specific write operations have **not** been reconstructed or implemented.
Registry work and game-root file adjustments are outside the owner's
approved scope for this step.

## One edition/reference-tree difference remains

The complete classic installation tree supplied by the owner contains
**177** chapter ZBD files plus six standalone. The original German
setup's own complete literal source→destination table contains **178**
chapter files. The discrepancy is isolated to the `C2B` chapter:
the setup explicitly lists `C2B/rtexture9.zbd` (source
`C2B/RT60AC~1.ZBD`), which does **not** appear in the owner's
installed file tree.

This could be due to selection settings, version, or other conditional
setup logic; the available data cannot establish the cause. Do not
remove it merely to force the reference count to match. Instead, log
the discrepancy and preserve the verified file unless a separate
owner-validated test establishes it should be excluded.

## Report source, not supplied on CD

`cab-contents.txt` is **not a CD asset**. It is an optional text file
produced by invoking `cabextract -l C1.CAB` (or a loop over all
eight CABs) and redirecting output. Now that the verified original
`SETUP.EXE` supplies the rename table, this report is not needed to
reconstruct the long names.
