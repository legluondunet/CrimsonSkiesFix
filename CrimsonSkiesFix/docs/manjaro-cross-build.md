# Build the Windows x86 MSVC DirectDraw DLL on Manjaro Linux

GitHub Actions is disabled due to the owner's exhausted free quota. This
source branch can produce a **local Windows x86 MSVC DLL** on Manjaro with
`cargo-xwin`, without installing Visual Studio or compilers on Windows.

**Critical:** retain target `i686-pc-windows-msvc`. Do **not** switch to
`i686-pc-windows-gnu`: the proposed 22-export fix uses MSVC-specific
`build.rs`, `ddraw_exports.def`, and x86 assembly ABI forwarders.
The source has not yet been demonstrated to compile successfully with xwin
or passed native Windows gameplay tests. Treat the first run as an experiment.

## Minimal prerequisites

If Rust and Git are already present, don't reinstall them. On Manjaro:

```bash
sudo pacman -S --needed git clang lld llvm python rustup
rustup default stable
rustup target add i686-pc-windows-msvc
rustup component add llvm-tools
cargo install --locked cargo-xwin
```

Check `cargo --version`, `rustup --version`, `clang-cl --version`,
`lld-link --version`, and `llvm-lib --version`. Rustup may need to be
reconciled with an existing Rust/pacman toolchain; avoid replacing an
existing working Rust environment. `cargo-xwin` installs in `~/.cargo/bin`
and downloads the Microsoft CRT/SDK into a user cache. Downloads require
network access and acceptance of the Microsoft SDK license.

## Clone the PR branch

```bash
git clone --branch diagnostics/windows-native-ddraw-startup-20261106 \
  https://github.com/legluondunet/CrimsonSkiesFix.git
cd CrimsonSkiesFix
```

For an existing checkout: `git fetch origin`, then
`git switch diagnostics/windows-native-ddraw-startup-20261106` and
`git pull --ff-only`.

## Compile + static verification + build a local candidate ZIP

```bash
bash CrimsonSkiesFix/tools/build_linux_windows_x86.sh
```

The script:
- Checks Git commit and refuses uncommitted tracked modifications.
- Runs Python verifier self-tests and native Linux Rust tests.
- Uses `XWIN_ARCH=x86`, static CRT and the MSVC x86 target.
- Compiles Windows-target Rust tests (`--no-run`) and the 32-bit
  `ddraw_loader_smoke.exe` and `ddraw.dll`.
- Verifies all **22 named PE32 x86 DDRAW exports and canonical ordinals**.
- Creates a five-file package with the normal INI template, GPL license,
  source commit, DLL SHA-256 and machine-checkable manifest.
- Independently checks the local candidate and creates a ZIP beside the
  cloned repository, named `CrimsonSkiesFix-local-<sha8>.zip`.

**It does not execute Win32 test binaries, run the game, touch the active
Crimson Skies game folder or change user settings.** A successful Linux
cross-build is not a substitute for native Windows 11 hardware gameplay.

On Windows, back up the *known working* full-22-export bridge and backend,
and substitute the **single DLL from the newly built ZIP**. Keep the tested
reduced-graphics INI for the first acceptance test, with `[Debug] Log=1`.
Confirm Accelerator selection, gameplay, a fresh log, and normal exit. Then
retest default graphics/widescreen/FOV/menus, videos, Alt+Tab, Wine/Proton
and clean shutdown before merging or releasing.

## If this fails

Send the first error line plus the exact tool versions. In particular, a
failure to link `ddraw_exports.def` may indicate differences between
MSVC `link.exe` and LLVM `lld-link`; do not silently bypass the
22-export verification or switch to the Windows GNU target.
