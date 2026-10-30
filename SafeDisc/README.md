# SafeDisc — original game protection research

This directory groups **all SafeDisc-related research and tools** in
CrimsonSkiesFix, independently of the `CrimsonSkiesFix/` DirectDraw DLL.

## Rust research

- `src/bin/unsafedisc-linux.rs`: native Linux/Windows SafeDisc
  inspection and reconstruction research CLI
- `src/unsafedisc_*.rs`: verified historical unSafeDisc 1.5.5
  disassembly/reconstruction stages and PE section/relocation work
- `src/main.rs`: conservative Crimson Skies EXE/Wine patcher
- `docs/`: detailed SafeDisc 1.x notes and validation evidence
- `archive/legacy-safedisc-102/`: earlier, otherwise unmerged SafeDisc
  secdrv protocol and Wine research source retained for provenance;
  deliberately **not** compiled into the active Rust crate

Build from the repository root:

```sh
cargo test --manifest-path SafeDisc/Cargo.toml
cargo build --release --manifest-path SafeDisc/Cargo.toml
```

## Python v0.9 prototype (restored from the original archive)

The separate **Crimson Skies unwrapping prototype v0.9** is preserved
unaltered in `python/unwrapper/`:

- `crimson_unwrap_candidate.py` — SHA-pinned candidate PE32 unwrapper
- `analyze_modifiers.py` — bounded analysis of F1/F2 modifications
- `historical_output_profile.py` — exact-title historical output style
- `import_site_rvas.txt`, `permutation_metadata.txt`
- `README-FR.md` — original French documentation and limitations

This is **not** a generic SafeDisc remover. It targets an exact known
Crimson Skies 1.0.2.0 version, requires the owner's original protected
ICD and a separate SHA-validated Win98 memory snapshot
(`CS98-DLL-00.BIN`), and does not ship game binaries or snapshots.
Output candidates were compared with an historical reference, but
**successful gameplay was not established by that comparison**.
Read the original `python/unwrapper/README-FR.md` before use.

## Python v0.8 historical snapshot

The earlier independent v0.8 Python prototype is also retained in
`python/archive/v0.8/` with its exact original source and French
README, so comparisons with the later v0.9 implementation remain
possible. The shared `analyze_modifiers.py` and static metadata files
are preserved byte-for-byte in both versions.

## Additional Python/Wine research

`scripts/` contains the restored `analyze-safedisc-relay.py`
and `winedbg-export-breakpoint.py` helpers from the historical
`feature/safedisc-102-pair-validation` branch, along with their
unit tests and shell probe. They are separate from the v0.9 unwrapper.

Run synthetic, offline Python checks:

```sh
cd SafeDisc
python -m compileall -q python/unwrapper scripts
python -m unittest discover -s scripts -p 'test_*.py'
```

## CI isolation

The SafeDisc workflows are `build.yml` (Windows patcher) and
`unsafedisc-linux.yml` (Linux/Windows research). They trigger only
for relevant SafeDisc source/docs/workflow changes, or via manual
dispatch. The DirectDraw DLL workflow
`crimsonskiesfix-x86.yml` **does not inspect or compile SafeDisc/**.

Licensed GPL-3.0-or-later; see the top-level LICENSE.
