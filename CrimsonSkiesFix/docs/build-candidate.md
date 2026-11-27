# Rebuilding the five-edition test candidate

Use the exact commit given in the candidate archive's SOURCE-INFO.txt.
This is a test-only build, not a publication recipe or legal authorization.
The canonical CI recipe is .github/workflows/unified-fix-prototype.yml at
that commit. The installer and runtime Cargo.lock files must remain unchanged.
Build inputs must not include game media, Microsoft updater binaries or
private owner-only embedded data.

## Environment

Use Windows with the x86 MSVC Rust target and linker, Git, PowerShell and
Python 3. Use the Rust version recorded in SOURCE-INFO.txt (full toolchain
and runner metadata are in the dependency-inventory artifact).
In a clean checkout, select the recorded commit, then enter CrimsonSkiesFix/
within the repository. The command below assumes PowerShell.

```powershell
$env:CRIMSONSKIESFIX_COMMIT = (git rev-parse HEAD).Trim()
$env:CRIMSONSKIESFIX_BUILD_COMMIT = $env:CRIMSONSKIESFIX_COMMIT
$env:RUSTFLAGS = '-C target-feature=+crt-static'
rustup target add i686-pc-windows-msvc
cargo metadata --locked --format-version 1 --filter-platform i686-pc-windows-msvc --manifest-path runtime-unified/Cargo.toml | Set-Content -Encoding utf8 runtime-meta.json
cargo metadata --locked --format-version 1 --filter-platform i686-pc-windows-msvc --manifest-path installer/Cargo.toml --features native-usde-updater | Set-Content -Encoding utf8 installer-meta.json
python installer/tools/collect_dependency_notices.py --metadata runtime-meta.json installer-meta.json --output THIRD-PARTY-NOTICES.txt
$env:CRIMSONSKIESFIX_NOTICES_FILE = (Resolve-Path THIRD-PARTY-NOTICES.txt).Path
cargo build --locked --manifest-path runtime-unified/Cargo.toml --release --target i686-pc-windows-msvc
$env:CRIMSONSKIESFIX_BUNDLED_DLL = (Resolve-Path runtime-unified/target/i686-pc-windows-msvc/release/fix.dll).Path
cargo build --locked --manifest-path installer/Cargo.toml --features native-usde-updater --release --target i686-pc-windows-msvc
git diff --exit-code -- installer/Cargo.lock runtime-unified/Cargo.lock
```

Stop on any error; never substitute other dependency revisions. Run the tests,
PE checks and package validation in the exact CI recipe. The output is
installer/target/i686-pc-windows-msvc/release/CrimsonSkiesFix.exe.
The Git dependency is pinned by both manifest and lockfile, additionally
retained at dependency/crimsonskiesfix-1.1 in the unsafedisk repository.
The current CI records toolchain identity, but the full compiler, linker,
Windows SDK and runner environment are not pinned for bit-identical builds.

Before any public release, arrange durable access to the actual corresponding
source and build scripts, including applicable third-party sources and notices.
The test package's references and generated notices do not by themselves
complete that review.
