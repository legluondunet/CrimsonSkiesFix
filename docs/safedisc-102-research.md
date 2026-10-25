# SafeDisc 1.02 research status — October 2026

## Scope and verified input files

Only the Windows PC **Crimson Skies 1.0.2.0** protected pair:

| Input | Size | SHA-256 |
| --- | ---: | --- |
| `CRIMSON.EXE` — SafeDisc loader | 344,851 | `675a3c768c3676a525240d365f6d225b2886f51cf81fd24c271ce1c0a8afa736` |
| `CRIMSON.ICD` — encrypted game PE32 x86 executable | 2,580,578 | `0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b` |

The other supplied protected EXE (`7d602947…f011f25a1`) reports a different game version (**1.0.0.1**) and is **intentionally excluded**.

The unpacked widescreen reference has a size of **2,580,480 bytes**, the same image base (`0x400000`), entry-point RVA (`0x1F7056`), section names, section virtual addresses and raw offsets as the ICD. This structural match **does not demonstrate** that the encrypted ICD can already be converted.

## Byte-level comparison of supplied local files

The comparison uses the common 2,580,480-byte file span (ignoring the ICD's 98-byte trailing metadata) and the PE section raw ranges:

| Region | Changed bytes vs. supplied unpacked reference |
| --- | ---: |
| `.text` (2,105,344 bytes) | 1,813,393 (86.1%) |
| `.rdata` (90,112 bytes) | 0 |
| `.data` (172,032 bytes) | 167,371 (97.3%) |
| `.idata` (16,384 bytes) | 3,362 (20.5%) |
| `.rsrc` (24,576 bytes) | 0 |
| `.reloc` (167,936 bytes) | 2 |

The extensive changes in executable code and initialized data **cannot** be repaired by merely changing the PE extension, setting the widescreen constants or replacing the entry point. In particular, an algorithm or verified key is needed to decrypt the contents, and the IAT must be restored.

## Reverse-engineering leads and required evidence

SafeDisc v1 used the separate `EXE` loader and `ICD` encrypted executable format. Research discussing SafeDisc 1.06–1.11 reports **TEA (Tiny Encryption Algorithm)**, key recovery and post-decryption import table reconstruction:

- [SafeDisc 1.06–1.11 reverse-engineering discussion](https://gbatemp.net/threads/safedisc-1-06-1-11-reverse-engineering-technical-paper.611118/)
- [SafeDisc v1 background](https://en.wikipedia.org/wiki/SafeDisc)

**These are leads, not a validated decoder for Crimson Skies.** The key, encryption granularity, cipher parameters, section-specific handling, IAT repair and correct output verification have **not yet been established for this exact pair**. No copyrighted decoded game code or large binary diff may be embedded in or published with this project.

Before claiming autonomous unpacking, an implementation should:

1. Verify the exact original EXE and ICD hashes and PE structure.
2. Derive and validate the required decryption data from the user's own files.
3. Decrypt `.text` / `.data` as appropriate, restore imports and remove only the loader-specific metadata.
4. Reconstruct a working PE32 x86 image **without** embedding copyrighted reference bytes.
5. Compare the output against the locally held reference as a research oracle, including section hashes and import behavior, then test the game in Wine/Windows.
6. Only then expose an automated restore-safe `unpack` path.

## Current implementation

The Rust CLI **verifies both protected files** using SHA-256 (auto-discovers `CRIMSON.ICD` beside the protected loader, or accepts `--icd PATH`), and refuses a missing, corrupt or unknown ICD before modifying anything. With this pair, resolution/FOV modifications require the separately supplied recognized unpacked widescreen executable and explicit `--accept-local-replacement` consent. This is **local replacement, not SafeDisc removal from the provided pair**.

The widescreen patch and the optional narrowly verified `roffile.dll` Wine workaround retain existing safety checks, one-time backups and restoration. Unknown binaries fail closed.
