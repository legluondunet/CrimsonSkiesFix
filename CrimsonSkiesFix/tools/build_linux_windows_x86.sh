#!/usr/bin/env bash
# Independent local cross-build to Windows i686 MSVC without GitHub Actions.
# Does not modify game files, the game's INI, or existing candidates.
set -euo pipefail
fail() { printf 'LOCAL-LINUX: ERROR: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"; }
for cmd in git rustup cargo python3 clang-cl lld-link llvm-lib sha256sum; do need "$cmd"; done

target=i686-pc-windows-msvc
tools_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
crate_root="$(cd -- "$tools_dir/.." && pwd -P)"
repo_root="$(cd -- "$crate_root/.." && pwd -P)"
cd "$repo_root"
test -f CrimsonSkiesFix/Cargo.toml || fail 'unexpected repository layout'
commit="$(git rev-parse --verify HEAD)"
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || fail 'invalid source commit'
[[ -z "$(git status --porcelain --untracked-files=no)" ]] ||
    fail 'tracked source is modified; commit or stash changes before building'
short="$(printf %.8s "$commit")"
out="$(dirname "$repo_root")/CrimsonSkiesFix-local-$short"
archive="$out.zip"
[[ ! -e "$out" && ! -e "$archive" ]] ||
    fail "candidate already exists: $out (or ZIP)"
cd "$crate_root"

# Download only x86 Microsoft CRT + SDK rather than default x64/ARM variants.
export XWIN_ARCH=x86
export CRIMSONSKIESFIX_COMMIT="$commit"

echo "LOCAL-LINUX: source=$commit target=$target"
python3 tools/verify_release_candidate.py --self-test
python3 tools/verify_game_exit.py --self-test
python3 tools/inspect_gos_scripts.py --self-test
python3 tools/inspect_gos_rof.py --self-test
python3 tools/patch_gos_video_overlay.py --self-test

rustup target add "$target"
# Run Linux unit tests, and compile Windows-targeted tests without launching
# Win32 executables on Linux. Real Windows testing is a separate step.
cargo test --locked
# +crt-static belongs to the Windows MSVC CRT; never apply it to host Linux tests.
export RUSTFLAGS='-C target-feature=+crt-static'
cargo xwin test --locked --target "$target" --no-run
cargo xwin build --locked --release --target "$target" --bin ddraw_loader_smoke
cargo xwin build --locked --release --target "$target" --lib

dll="$crate_root/target/$target/release/ddraw.dll"
smoke="$crate_root/target/$target/release/ddraw_loader_smoke.exe"
[[ -s "$dll" && -s "$smoke" ]] || fail 'missing built x86 DLL or loader smoke executable'
python3 tools/verify_ddraw_exports.py --dll "$dll" --definitions src/ddraw_exports.def

mkdir -p "$out/CrimsonSkiesFix"
cp -- "$dll" "$out/ddraw.dll"
cp -- "CrimsonSkiesFix/CrimsonSkiesFix.ini.example" "$out/CrimsonSkiesFix/CrimsonSkiesFix.ini.example"
cp -- README.md "$out/CrimsonSkiesFix/README.md"
cp -- "$repo_root/LICENSE" "$out/CrimsonSkiesFix/LICENSE"

python3 - "$out" "$commit" "$crate_root/Cargo.toml" <<'PY'
import hashlib
from pathlib import Path
import re,sys
out, commit, cargo_file = Path(sys.argv[1]),sys.argv[2],Path(sys.argv[3])
cargo = cargo_file.read_text(encoding="utf-8")
section = re.search(r"(?ms)^\[package\]\s*\n(.*?)(?=^\[|\Z)", cargo)
if not section: raise SystemExit("LOCAL-LINUX: no Cargo package section")
version_match = re.search(r'(?m)^version\s*=\s*"([^"]+)"\s*$', section.group(1))
if not version_match: raise SystemExit("LOCAL-LINUX: no Cargo package version")
version = version_match.group(1)
dll = out / "ddraw.dll"
digest = hashlib.sha256(dll.read_bytes()).hexdigest()
fields = [
    "Project=CrimsonSkiesFix",
    f"Version={version}",
    f"Commit={commit}",
    f"LogIdentity=CrimsonSkiesFix {version}-{commit[:8]} x86",
    "Architecture=Windows-x86",
    f"DllSha256={digest}",
    f"DllSizeBytes={dll.stat().st_size}",
    f"Source=https://github.com/legluondunet/CrimsonSkiesFix/tree/{commit}",
]
(out / "CrimsonSkiesFix/build-info.txt").write_text("\n".join(fields)+"\n",encoding="utf-8")
PY

python3 tools/verify_release_candidate.py --directory "$out" \
  --expected-commit "$commit" --cargo-toml Cargo.toml --license "$repo_root/LICENSE"

python3 - "$out" "$archive" <<'PY'
from pathlib import Path
import sys, zipfile
src,dst=Path(sys.argv[1]),Path(sys.argv[2])
with zipfile.ZipFile(dst,"x",compression=zipfile.ZIP_DEFLATED) as z:
    for f in sorted(src.rglob("*")):
        if f.is_file(): z.write(f,arcname=f.relative_to(src))
print(f"LOCAL-LINUX: archive={dst}")
PY
echo "LOCAL-LINUX: PASS — Linux tests, Windows-target test compilation, 22 PE exports, candidate manifest"
echo "LOCAL-LINUX: commit=$commit"
echo "LOCAL-LINUX: dll_sha256=$(sha256sum "$out/ddraw.dll" | cut -d' ' -f1)"
echo "LOCAL-LINUX: NOT a native-Windows-gameplay acceptance or official release."
