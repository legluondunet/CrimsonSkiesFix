# Native 3D widescreen: unwrapped 1.0.2.0 research

## Observed candidate

User-provided `CRIMSON-UNWRAPPED.EXE`:

- SHA-256: `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`
- File length: 2,580,480 bytes
- PE32 image base: `0x00400000`; SizeOfImage: `0x00652000`
- Mapped `.text`: RVA `0x1000`, virtual size `0x2014bd`.

Read-only file inspection confirms:

| RVA | Observed bytes | Meaning |
| --- | --- | --- |
| 0x41229 | `68 00 03 00 00 68 00 04 00 00` | pushes hardcoded 768 and 1024 rather than dereferencing width/height globals |
| 0x1a8eba | `90 5a 00 b8 80 02 00 00` | does not match the already-patched EXE's width global read |
| 0x1a8fc9 | `b9 00 03 00 00` | does not match patched height global read |
| 0x2024bd | zeros | no injected x87 FOV implementation at expected RVA |
| 0x20254b | zeros | no initialized resolution pair at expected RVA |

The existing runtime mutator is written for a *different*, already-widescreen-patched EXE (SHA-256 starts `5578a15c`). It must **never** be enabled for the unwrapped candidate by simply loosening its signature gate.

## Proposed staged implementation

1. Establish exact, independently tested x86 control-flow signatures for all relevant mode paths of the unwrapped candidate.
2. Identify all required camera/FOV update sites and validate the actual geometric result, including menus and video separation.
3. Design a transactional, fail-closed in-memory code-patch operation with protection restoration and instruction-cache flush, and a rollback on any failed precondition.
4. Test with a fixture based on the unwrapped EXE, then build on Windows i686 CI, and only afterward test the opt-in runtime hook under Wine and Windows.

**Current branch only adds explanatory mismatch logs. It does not patch the unwrapped EXE or claim native widescreen works.**
