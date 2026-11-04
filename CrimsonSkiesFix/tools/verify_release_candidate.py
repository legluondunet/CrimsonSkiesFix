#!/usr/bin/env python3
"""Independent, read-only verifier for a downloaded CrimsonSkiesFix x86 artifact.

Checks the unpacked directory, not the repository's build intermediates.
No game assets, Wine, Windows, external packages or network access required.
This verifies *integrity and provenance*, NOT authenticity/code signing.
"""
from __future__ import annotations

import argparse
import hashlib
import re
import struct
import sys
import tempfile
from pathlib import Path

EXPECTED_FILES = frozenset({
    "ddraw.dll",
    "CrimsonSkiesFix/CrimsonSkiesFix.ini.example",
    "CrimsonSkiesFix/README.md",
    "CrimsonSkiesFix/LICENSE",
    "CrimsonSkiesFix/build-info.txt",
})
FIELDS = frozenset({
    "Project", "Version", "Commit", "LogIdentity", "Architecture",
    "DllSha256", "DllSizeBytes", "Source",
})
SOURCE_BASE = "https://github.com/legluondunet/CrimsonSkiesFix/tree/"


class VerificationError(ValueError):
    """Invalid or inconsistent distributable artifact."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def cargo_version(path: Path) -> str:
    # The [package] section is sufficient here; no dependency on tomllib
    # or external tools is needed for Windows/Ubuntu Python runners.
    text = path.read_text(encoding="utf-8")
    match = re.search(r"(?ms)^\[package\]\s*\n(.*?)(?=^\[|\Z)", text)
    if match is None:
        raise VerificationError("Cargo.toml lacks [package]")
    versions = re.findall(r'(?m)^version\s*=\s*"([0-9A-Za-z.+-]+)"\s*$', match.group(1))
    if len(versions) != 1:
        raise VerificationError("Cargo.toml has no unique package version")
    return versions[0]


def load_manifest(path: Path) -> dict[str, str]:
    data: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        if not line or line.isspace():
            continue
        if "=" not in line:
            raise VerificationError("Malformed build-info.txt record")
        key, value = line.split("=", 1)
        if key not in FIELDS or key in data or not value:
            raise VerificationError(f"Unknown, duplicated or empty manifest field: {key}")
        data[key] = value
    if data.keys() != FIELDS:
        raise VerificationError(
            f"Manifest field mismatch: missing={sorted(FIELDS - data.keys())}, "
            f"extra={sorted(data.keys() - FIELDS)}"
        )
    return data


def verify_pe32_dll(path: Path) -> None:
    size = path.stat().st_size
    if size < 0x100:
        raise VerificationError("ddraw.dll is too small to be a PE image")
    with path.open("rb") as stream:
        if stream.read(2) != b"MZ":
            raise VerificationError("ddraw.dll missing MZ header")
        stream.seek(0x3C)
        pe_offset = struct.unpack("<I", stream.read(4))[0]
        if not 0x40 <= pe_offset <= size - 26:
            raise VerificationError("ddraw.dll PE offset outside file")
        stream.seek(pe_offset)
        header = stream.read(26)
    if len(header) != 26 or header[:4] != b"PE\x00\x00":
        raise VerificationError("ddraw.dll missing PE signature")
    machine = struct.unpack_from("<H", header, 4)[0]
    characteristics = struct.unpack_from("<H", header, 22)[0]
    optional_magic = struct.unpack_from("<H", header, 24)[0]
    if machine != 0x014C or optional_magic != 0x010B or not characteristics & 0x2000:
        raise VerificationError(
            f"ddraw.dll not a Windows i386 PE32 DLL: machine={machine:#x}, "
            f"magic={optional_magic:#x}, characteristics={characteristics:#x}"
        )


def verify(directory: Path, expected_commit: str, cargo: Path, license_path: Path) -> dict[str, str]:
    if not re.fullmatch(r"[0-9a-fA-F]{40}", expected_commit):
        raise VerificationError("Expected source commit must be a full 40-digit hex SHA")
    commit = expected_commit.lower()
    if not directory.is_dir() or directory.is_symlink():
        raise VerificationError("Artifact directory missing or symbolic")
    entries = list(directory.rglob("*"))
    if any(item.is_symlink() for item in entries):
        raise VerificationError("Artifact contains a symbolic link")
    if any(not (item.is_file() or item.is_dir()) for item in entries):
        raise VerificationError("Artifact contains a special file")
    actual = {item.relative_to(directory).as_posix() for item in entries if item.is_file()}
    if actual != EXPECTED_FILES:
        raise VerificationError(
            f"Unexpected artifact files: missing={sorted(EXPECTED_FILES - actual)}, "
            f"extra={sorted(actual - EXPECTED_FILES)}"
        )

    binary = directory / "ddraw.dll"
    verify_pe32_dll(binary)
    if not (directory / "CrimsonSkiesFix/README.md").stat().st_size:
        raise VerificationError("Empty README")
    if not (directory / "CrimsonSkiesFix/CrimsonSkiesFix.ini.example").stat().st_size:
        raise VerificationError("Empty configuration template")
    if sha256(directory / "CrimsonSkiesFix/LICENSE") != sha256(license_path):
        raise VerificationError("Packaged GPL license differs from repository")

    version = cargo_version(cargo)
    manifest = load_manifest(directory / "CrimsonSkiesFix/build-info.txt")
    expected = {
        "Project": "CrimsonSkiesFix",
        "Version": version,
        "Commit": commit,
        "LogIdentity": f"CrimsonSkiesFix {version}-{commit[:8]} x86",
        "Architecture": "Windows-x86",
        "DllSha256": sha256(binary),
        "DllSizeBytes": str(binary.stat().st_size),
        "Source": f"{SOURCE_BASE}{commit}",
    }
    for key, value in expected.items():
        if manifest[key] != value:
            raise VerificationError(f"Incorrect {key}: expected {value!r}, got {manifest[key]!r}")
    return expected


def self_test() -> None:
    with tempfile.TemporaryDirectory() as temp:
        base = Path(temp)
        root = base / "artifact"
        folder = root / "CrimsonSkiesFix"
        folder.mkdir(parents=True)
        cargo = base / "Cargo.toml"
        cargo.write_text('[package]\nname = "crimson-skies-fix"\nversion = "0.1.90"\n\n[dependencies]\n', encoding="utf-8")
        license_path = base / "LICENSE"
        license_path.write_bytes(b"Mock license for synthetic test only\nSecond line with LF\n")
        (folder / "LICENSE").write_bytes(license_path.read_bytes())
        (folder / "README.md").write_text("Installation instructions", encoding="utf-8")
        (folder / "CrimsonSkiesFix.ini.example").write_text("[Graphics]\nBorderless=1\n", encoding="utf-8")
        dll = root / "ddraw.dll"
        fake_pe = bytearray(0x120)
        fake_pe[:2] = b"MZ"
        struct.pack_into("<I", fake_pe, 0x3C, 0x80)
        fake_pe[0x80:0x84] = b"PE\x00\x00"
        struct.pack_into("<H", fake_pe, 0x84, 0x014C)
        struct.pack_into("<H", fake_pe, 0x96, 0x2000)
        struct.pack_into("<H", fake_pe, 0x98, 0x010B)
        dll.write_bytes(fake_pe)
        commit = "a" * 40
        manifest_path = folder / "build-info.txt"
        fields = {
            "Project": "CrimsonSkiesFix",
            "Version": "0.1.90",
            "Commit": commit,
            "LogIdentity": "CrimsonSkiesFix 0.1.90-aaaaaaaa x86",
            "Architecture": "Windows-x86",
            "DllSha256": sha256(dll),
            "DllSizeBytes": str(dll.stat().st_size),
            "Source": f"{SOURCE_BASE}{commit}",
        }
        def write_manifest() -> None:
            manifest_path.write_text("".join(f"{key}={value}\n" for key, value in fields.items()), encoding="utf-8")

        def reject(label: str) -> None:
            try:
                verify(root, commit, cargo, license_path)
            except VerificationError:
                return
            raise AssertionError(f"Verifier accepted corrupted artifact: {label}")

        write_manifest()
        verify(root, commit, cargo, license_path)
        fields["Commit"] = "b" * 40
        write_manifest()
        reject("wrong source commit")
        fields["Commit"] = commit
        fields["DllSha256"] = "0" * 64
        write_manifest()
        reject("wrong binary digest")
        fields["DllSha256"] = sha256(dll)
        write_manifest()
        (root / "extra.bin").write_bytes(b"unexpected")
        reject("extra file")
        (root / "extra.bin").unlink()
        (folder / "CrimsonSkiesFix.ini").write_text("user data", encoding="utf-8")
        reject("active user INI shipped")
        (folder / "CrimsonSkiesFix.ini").unlink()
        (folder / "LICENSE").write_text("tampered license", encoding="utf-8")
        reject("license mismatch")
        (folder / "LICENSE").write_bytes(license_path.read_bytes())
        (folder / "LICENSE").write_bytes(license_path.read_bytes().replace(b"\n", b"\r\n"))
        reject("Windows CRLF license differs from repository LF")
        (folder / "LICENSE").write_bytes(license_path.read_bytes())
        dll.write_bytes(b"not-a-PE" + bytes(300))
        reject("invalid PE32 DLL")
        dll.write_bytes(fake_pe)
        fields["LogIdentity"] = "CrimsonSkiesFix 0.1.90-deadbeef x86"
        write_manifest()
        reject("wrong log build label")
        fields["LogIdentity"] = "CrimsonSkiesFix 0.1.90-aaaaaaaa x86"
        write_manifest()
        verify(root, commit, cargo, license_path)
    print("RELEASE-CANDIDATE-VERIFY: self-test PASS (valid and 8 rejected corruptions)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="run synthetic positive/negative tests")
    parser.add_argument("--directory", type=Path, help="unpacked archive directory")
    parser.add_argument("--expected-commit", help="full commit SHA from GitHub run")
    parser.add_argument("--cargo-toml", type=Path, help="same source revision's Cargo.toml")
    parser.add_argument("--license", type=Path, help="same source revision's LICENSE")
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
        else:
            if not all((args.directory, args.expected_commit, args.cargo_toml, args.license)):
                parser.error("--directory, --expected-commit, --cargo-toml and --license are required")
            result = verify(args.directory, args.expected_commit, args.cargo_toml, args.license)
            print(
                "RELEASE-CANDIDATE-VERIFY: PASS "
                + result["LogIdentity"]
                + " commit=" + result["Commit"]
                + " dll_sha256=" + result["DllSha256"]
            )
    except (VerificationError, OSError) as exc:
        print(f"RELEASE-CANDIDATE-VERIFY: FAIL: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
