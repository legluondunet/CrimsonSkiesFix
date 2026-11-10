# Owner-supplied original Crimson Skies updater 1.02: exact provenance

On 2026-11-10, the owner provided **`Crimson_Skies_Patch_102.exe`**.
Static, read-only inspection confirmed that it is **byte-identical** to the
previously analyzed original 1.02 updater `crmsk102.exe` (see
[`crimson-skies-102-official-update.md`](crimson-skies-102-official-update.md)).
We did not execute this historical Windows setup program.

| Property | Verified value |
|---|---|
| Updater size | **2,315,728 bytes** |
| Updater SHA-256 | `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af` |
| Format | Intel x86 PE32 setup executable |
| Embedded CAB offset | `0x346F0` (214,768 bytes into the updater) |
| Embedded CAB length | **2,091,693 bytes** (also verified by the CAB header) |
| CAB internal structure | **2 folders, 5 files, LZX compression** |
| Embedded CAB SHA-256 | `3b19e427d594b08f4acc67b52ab03844a1deba4caf16621a2149ed774bc7c38f` |
| Setup metadata | `SETUPDATA` resource located at `0x33DC8` in the prior audit |

## Embedded CAB file list (from CAB directory)

| Path | Uncompressed bytes | Relevance |
|---|---:|---|
| `setupenu.dll` | 32,768 | Legacy setup helper |
| `readmeUSA.rtf` | 57,539 | Update documentation |
| `CRIMSON.EXE` | 344,851 | **SafeDisc-protected** 1.02 launcher |
| `CRIMSON.ICD` | 2,580,578 | **SafeDisc-protected** 1.02 executable payload |
| `gosdata/assets/CRIMPTCH.ROF` | 797 | Separate 1.02 gameplay asset |

These five paths and sizes were extracted from the CAB *directory metadata*
without executing the installer. Per-payload SHA-256 values are recorded in
the earlier updater audit. Do **not** assume that extraction from the CAB
makes `CRIMSON.EXE` acceptable to CrimsonSkiesFix: the compatibility
runtime intentionally accepts **only** the exact unwrapped 1.02 executable
(SHA-256 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`).

## Behavior required in the final ISO/CUE-BIN installer

1. Recognize the update file by its **exact 2,315,728-byte size and
   SHA-256**, not by `update1.02`, `crmsk102.exe`, or any other filename.
   Do not permit an arbitrary similarly named EXE to run.
2. The **native LZX-compressed CAB** needs a vetted decompression/extraction
   implementation compatible with Windows and Wine, without running the
   historical executable merely to extract files.
3. Apply relevant official 1.02 **assets** to recognized CD sources with
   hashes checked and a rollback path. The FR CD `C_SKIES_FR_001` already
   includes an identical 797-byte `CRIMPTCH.ROF`; leave it unchanged.
4. The USA/Europe and DE discs lack this asset; add it from the authenticated
   updater as required. Do not replace regional `STRINGS.DLL`, `LANGUAGE.DLL`,
   `LANGUI.DLL`, MPEG videos, sounds or `CRIMSON.ROF`.
5. Resolve the final **strict unwrapped 1.02 executable** requirement
   before any claimed fully automatic install. The updater provides a
   *protected* pair, and no conversion step is implemented here.
6. Do not redistribute the commercial patch, game executables or CAB data.
   Read the patch from the owner's files at installation time.

## Safe read-only preflight now provided

The experimental launcher supports:

```text
CrimsonSkiesFix.exe --inspect-update
CrimsonSkiesFix.exe --inspect-update "Crimson_Skies_Patch_102.exe"
```

The implicit mode searches adjacent files by **verified content**; the
explicit mode audits the provided file even if renamed. A successful check
writes `CrimsonSkiesFix.log` beside the launcher and reports the verified
CAB structure and file names. Failure reports the observed unsupported
size/hash without running the file. **This is not patch installation**.
The log is replaced by each explicit diagnostic command, so copy any
`--inspect-cd` log before inspecting the updater.

**Status:** PR #26 remains a draft. CI can verify the file-gate logic
with test fixtures, but real-file validation was performed locally by
static inspection of the owner's upload; the commercial patch itself
is intentionally not committed to source control or CI artifacts.
