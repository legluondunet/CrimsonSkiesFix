# Experimental WinMM-only CrimsonSkiesFix runtime

This crate compiles the **original gameplay and DirectDraw hooks** in
`../src/lib.rs` as `winmm.dll`. It is different from the limited forwarding
prototype in `../winmm-prototype`.

**CrimsonSkiesFix's own `ddraw.dll` is not required by this build.** The
original game still requires a working DirectDraw implementation, whether
provided by Windows/Wine or an optional external graphics wrapper.

## Implementation

The SHA-256-approved original 1.02 `CRIMSON.EXE` imports both WinMM and
DirectDraw. During WinMM's minimal process-attach entry, the DLL checks the
exact expected in-memory **PE and import descriptor fingerprints**, ensures the
two DirectDraw IAT slots point to the loaded DirectDraw module, and changes
those two pointer entries to target the original CrimsonSkiesFix hook functions.
No disk EXE mutation, log, config, file I/O, library loading or Rust allocation
occurs in that loader-lock path. All original game verification and graphics
hook setup is **deferred until the game itself calls DirectDraw**.

The original `CrimsonSkiesFix` runtime and renderer interception code is
shared; the bootstrapping filename changes, not the game's gameplay fixes.
External DirectDraw is resolved from the **already-loaded** `ddraw.dll`
module, not forced from `System32`.

## Current proof and limitations

- The exact owner's EXE was parsed and the fixed import RVA preimage verified.
- Windows x86 compile and a **synthetic** PE fingerprint test are CI candidates.
- WinMM's 14 game-imported named functions are forwarded by an experimental x86
  ABI-preserving tail-jump proxy. Its ordinals **are not canonical WinMM ordinals**.
- Unknown dependent modules that import other WinMM APIs or ordinal entries may
  not work yet; do not deploy automatically.
- For Windows, an app-local WinMM DLL must actually be loaded; for Wine,
  native/builtin DLL overrides must be tested.
- This code has **not yet been validated in an actual game mission**.
  Test the output in a disposable working game installation.
- `DllMain` performs only constrained PE checks and IAT writes; nevertheless
  these operations run under the Windows loader lock. Any kernel or Wine
  behavior difference must be resolved before production release.
- No fallback attempts to overwrite an unknown or user-supplied graphics DLL.
- Does not alter the production installer, `main`, or release `v1.0`.

## Build

```sh
cd CrimsonSkiesFix/runtime-winmm
cargo build --release --target i686-pc-windows-msvc
```

The output is `target/i686-pc-windows-msvc/release/winmm.dll`.
Use the isolated CI artifact when testing; do **not** copy the production
CrimsonSkiesFix `ddraw.dll` into the same test directory.
