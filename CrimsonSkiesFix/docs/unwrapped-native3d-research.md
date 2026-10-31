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

## Verified second-pass x86 disassembly (unwrapped EXE)

The PE `.text` section has VMA 0x401000, file offset 0x1000, and raw size 0x2014bd. Therefore for the code RVA range quoted here, **RVA equals file offset** in this particular executable, but this must not be assumed for other PE sections.

The first mode sequence at absolute VA `0x441229` is:

```asm
441229: 68 00 03 00 00     push 768
44122e: 68 00 04 00 00     push 1024
441233: e8 e8 fb ff ff     call 0x440e20
441241: 68 00 03 00 00     push 768
441246: 68 00 04 00 00     push 1024
44124b: e8 60 fe ff ff     call 0x4410b0
```

Importantly, the current widescreen module's expected fingerprint RVAs 0x4122f and 0x41249 are *not aligned instruction starts* in the unwrapped file: they lie inside the immediate operands of `push 1024` at RVA 0x4122e and RVA 0x41246 respectively. Those are correct fingerprints of a *different, already patched* binary and cannot be transplanted as-is.

A separate mode-selection dispatcher begins at VA `0x5a8ea0` (RVA `0x1a8ea0`), using an indexed jump table at `0x5a908c`. For the 1024×768 selection:

```asm
5a8fa7: b9 00 04 00 00       mov ecx,1024
5a8fac: 8b c1                mov eax,ecx
5a8fae: 89 0d bc 03 a1 00    mov [0xa103bc],ecx
5a8fb4: 89 0d e0 03 a1 00    mov [0xa103e0],ecx
5a8fc4: a3 04 04 a1 00       mov [0xa10404],eax
5a8fc9: b9 00 03 00 00       mov ecx,768
5a8fce: eb 6d                jmp 0x5a903d
```

Other supported modes share this dispatcher and write globals such as `0xa103bc`, `0xa103c0`, `0xa103e0`, `0xa10404`, and `0xa10408`. Their precise roles must be mapped through call-site analysis before changing them.

The above is static evidence for mode selection **only**, not proof of a correct Hor+ camera implementation. No complete FOV-injection patch has been validated against the unwrapped binary. Avoid widening the executable signature matcher or writing blindly into the zero-filled region at RVA `0x2024bd`.

## Third pass: execution branches and camera-hook requirements

Direct `objdump -D -Mintel` on the supplied PE file independently confirms the x86 branch at VA `0x5a8fa7`:

```asm
5a8fa7  mov ecx,0x400         ; 1024
5a8fac  mov eax,ecx
5a8fae  mov [0xa103bc],ecx
5a8fb4  mov [0xa103e0],ecx
5a8fba  mov dword [0xa10308],0
5a8fc4  mov [0xa10404],eax
5a8fc9  mov ecx,0x300         ; 768
5a8fce  jmp 0x5a903d
```

This code is a *mode dispatcher*, not necessarily the camera projection. Later entries at `0x5a8fd0` and `0x5a8ff5` also handle additional fixed widths. Replacing the immediate 1024/768 here without checking the selector and downstream shared state could affect the wrong rendering paths.

For a credible 3D FOV patch we must first determine whether the game feeds the camera to a Direct3D viewport (`IDirect3DViewport3::SetViewport2`) or computes and submits transformed vertices/matrices itself. The hooked `IDirect3DDevice3` in our proxy does not offer a straightforward `SetTransform` hook equivalent to later D3D devices. We should trace the **actual viewport/camera call chain** rather than add a guessed matrix setter or patch one observed x87 constant.

Next implementation criterion: exact x86 site and expected bytes, proof it is used only for the 3D camera, comparison of old/new horizontal field-of-view at 4:3 vs 16:9, and a rollback that does not rely on an incompatible patched binary's RVAs. No active runtime memory patch is introduced by this research.
