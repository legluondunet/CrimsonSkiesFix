#!/usr/bin/env python3
"""Bundle the official pinned 32-bit SDL3 Windows runtime, fail closed.

Run from the repository root or CI:
    python3 scripts/package_sdl3_x86.py SDL3-build

SDL3 is dynamically loaded by CrimsonSkiesFix, never a mandatory import.
Only the upstream x86 Windows binary is bundled, never x64 or an unpacked
filesystem tree. The remote ZIP SHA-256 is taken from SDL's official GitHub
release asset digest and pinned rather than trusting whatever is "latest".
"""
from __future__ import annotations

import hashlib
import io
import pathlib
import shutil
import struct
import sys
import urllib.request
import zipfile

VERSION = "3.4.18"
ASSET = f"SDL3-{VERSION}-win32-x86.zip"
URL = f"https://github.com/libsdl-org/SDL/releases/download/release-{VERSION}/{ASSET}"
SHA256 = "99f2ee6e126d828c5e5b3d20201930f3d16a8c6e147d1de90a59f4ae6df138c9"
ASSET_SIZE = 994224
MAX_DLL_SIZE = 16 * 1024 * 1024
LICENSE = pathlib.Path(__file__).resolve().parents[1] / "third_party/SDL3-LICENSE.txt"


def check_pe32_x86(data: bytes) -> None:
    """Reject x64, other machines, truncated/corrupted PE and PE32+."""
    if len(data) < 512 or len(data) > MAX_DLL_SIZE or data[:2] != b"MZ":
        raise ValueError("SDL3.dll is missing or not an ordinary PE image")
    pe_at = struct.unpack_from("<I", data, 0x3C)[0]
    if pe_at < 0x40 or pe_at + 26 > len(data):
        raise ValueError("invalid PE header pointer")
    if data[pe_at:pe_at + 4] != b"PE\x00\x00":
        raise ValueError("not a PE executable")
    machine = struct.unpack_from("<H", data, pe_at + 4)[0]
    optional_header = struct.unpack_from("<H", data, pe_at + 24)[0]
    if machine != 0x014C or optional_header != 0x010B:
        raise ValueError("SDL3.dll must be PE32 i386, not x64 or PE32+")


def extract_verified_runtime(archive: bytes, expected: str = SHA256) -> bytes:
    if hashlib.sha256(archive).hexdigest().lower() != expected.lower():
        raise ValueError("SDL3 upstream archive SHA-256 mismatch; refusing packaging")
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        names = bundle.namelist()
        if any(
            name.startswith("/") or "\\" in name or
            any(part in ("", ".", "..") for part in name.rstrip("/").split("/"))
            for name in names
        ):
            raise ValueError("unexpected unsafe path in SDL3 release ZIP")
        dll_entries = [n for n in names if n.rsplit("/", 1)[-1] == "SDL3.dll"]
        if len(dll_entries) != 1:
            raise ValueError("SDL3 release ZIP needs exactly one SDL3.dll")
        info = bundle.getinfo(dll_entries[0])
        if info.file_size > MAX_DLL_SIZE or info.file_size < 512:
            raise ValueError("SDL3.dll decompressed size is implausible")
        if info.is_dir() or ((info.external_attr >> 16) & 0xF000) == 0xA000:
            raise ValueError("SDL3.dll is not a regular file")
        data = bundle.read(info)
    check_pe32_x86(data)
    return data


def package(directory: pathlib.Path, archive: bytes) -> None:
    if len(archive) != ASSET_SIZE:
        raise ValueError("unexpected SDL3 release ZIP size")
    dll = extract_verified_runtime(archive)
    if not LICENSE.is_file():
        raise FileNotFoundError("official SDL3 license text missing")
    text = LICENSE.read_bytes()
    if b"Sam Lantinga" not in text or b"redistribute it" not in text:
        raise ValueError("SDL3 license file is not the expected upstream notice")
    directory.mkdir(parents=True, exist_ok=True)
    if not directory.is_dir():
        raise ValueError("not an output directory")
    output = directory / "SDL3.dll"
    temp = directory / "SDL3.dll.tmp"
    if temp.is_symlink() or output.is_symlink():
        raise ValueError("refusing symlink for SDL3.dll")
    temp.write_bytes(dll)
    temp.replace(output)
    shutil.copyfile(LICENSE, directory / "SDL3-LICENSE.txt")
    print(
        f"SDL3: bundled official {VERSION} Windows x86 DLL "
        f"({len(dll)} bytes, PE32) + unmodified zlib license"
    )
    print(f"SDL3: verified upstream asset SHA-256 {SHA256}")


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 4) or (len(argv) == 4 and argv[2] != "--archive"):
        print("Usage: package_sdl3_x86.py OUTPUT_DIRECTORY [--archive ZIP]", file=sys.stderr)
        return 2
    out = pathlib.Path(argv[1])
    if len(argv) == 4:
        archive = pathlib.Path(argv[3]).read_bytes()
    else:
        req = urllib.request.Request(
            URL, headers={"User-Agent": "CrimsonSkiesFix-build/1.0"}
        )
        with urllib.request.urlopen(req, timeout=90) as response:
            archive = response.read(ASSET_SIZE + 1)
    package(out, archive)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except (OSError, ValueError, zipfile.BadZipFile) as exc:
        print(f"ERROR: refusing SDL3 package: {exc}", file=sys.stderr)
        sys.exit(1)
