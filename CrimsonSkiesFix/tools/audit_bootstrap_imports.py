#!/usr/bin/env python3
"""Read-only audit of a user-supplied Crimson Skies 1.02 PE32 executable.

Reports the directly imported DLLs, WinMM/DirectDraw imports, and PE delay
imports. Never loads, executes, patches, or copies the game executable.

Usage: python3 tools/audit_bootstrap_imports.py /path/to/CRIMSON.EXE
       python3 tools/audit_bootstrap_imports.py /path/to/CRIMSON.EXE --json

A static import alone does NOT establish hook activation order or prove that
an app-local DLL will be loaded on a particular Windows/Wine installation.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

APPROVED_SHA256 = "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75"
APPROVED_SIZE = 2_580_480
MAX_DESCRIPTORS = 256
MAX_THUNKS = 4096


class InvalidPE(ValueError):
    """The file is not a well-formed PE32 input for this limited audit."""


class PE32:
    def __init__(self, data: bytes):
        self.data = data
        if len(data) < 0x100 or data[:2] != b"MZ":
            raise InvalidPE("missing DOS header")
        pe = self.u32(0x3C)
        if pe < 0x40 or pe + 24 > len(data) or data[pe:pe+4] != b"PE\0\0":
            raise InvalidPE("missing PE signature")
        if self.u16(pe + 4) != 0x14C:
            raise InvalidPE("expected Intel x86 PE32")
        count = self.u16(pe + 6)
        optional_size = self.u16(pe + 20)
        optional = pe + 24
        if optional_size < 224 or optional + optional_size > len(data):
            raise InvalidPE("truncated PE32 optional header")
        if self.u16(optional) != 0x10B:
            raise InvalidPE("expected PE32 (not PE32+)")
        if not 1 <= count <= 96:
            raise InvalidPE("invalid section count")
        self.image_base = self.u32(optional + 28)
        self.headers_size = self.u32(optional + 60)
        self.directories = []
        number_of_directories = self.u32(optional + 92)
        for index in range(min(number_of_directories, 16)):
            at = optional + 96 + index * 8
            if at + 8 > optional + optional_size:
                raise InvalidPE("truncated data directories")
            self.directories.append((self.u32(at), self.u32(at + 4)))
        self.sections = []
        sections_at = optional + optional_size
        for n in range(count):
            pos = sections_at + n * 40
            if pos + 40 > len(data):
                raise InvalidPE("truncated section header")
            virt_size, virt_rva, raw_size, raw_start = struct.unpack_from("<IIII", data, pos + 8)
            self.sections.append((virt_rva, max(virt_size, raw_size), raw_size, raw_start))

    def u16(self, offset: int) -> int:
        self.check(offset, 2)
        return struct.unpack_from("<H", self.data, offset)[0]

    def u32(self, offset: int) -> int:
        self.check(offset, 4)
        return struct.unpack_from("<I", self.data, offset)[0]

    def check(self, offset: int, length: int):
        if offset < 0 or length < 0 or offset + length > len(self.data):
            raise InvalidPE("out-of-bounds PE read")

    def off(self, rva: int, length: int = 1) -> int:
        if rva < self.headers_size and rva + length <= self.headers_size:
            self.check(rva, length)
            return rva
        for base, virtual_size, raw_size, start in self.sections:
            if base <= rva and rva + length <= base + raw_size and rva + length <= base + virtual_size:
                offset = start + rva - base
                self.check(offset, length)
                return offset
        raise InvalidPE(f"RVA 0x{rva:x} is not backed by file bytes")

    def text(self, rva: int) -> str:
        pos = self.off(rva)
        end = self.data.find(b"\0", pos, min(len(self.data), pos + 256))
        if end == -1:
            raise InvalidPE("unterminated PE import name")
        try:
            return self.data[pos:end].decode("ascii")
        except UnicodeDecodeError as exc:
            raise InvalidPE("non-ASCII PE import name") from exc

    def directory(self, idx: int) -> tuple[int, int]:
        return self.directories[idx] if idx < len(self.directories) else (0, 0)

    def thunk_names(self, rva: int) -> list[str]:
        result = []
        for i in range(MAX_THUNKS):
            value = self.u32(self.off(rva + 4 * i, 4))
            if value == 0:
                return result
            if value & 0x80000000:
                result.append(f"#{value & 0xFFFF}")
            else:
                self.off(value, 2)  # IMAGE_IMPORT_BY_NAME.hint
                result.append(self.text(value + 2))
        raise InvalidPE("unterminated PE import thunk table")

    def imports(self, delay: bool = False) -> list[dict]:
        rva, size = self.directory(13 if delay else 1)
        if rva == 0 and size == 0:
            return []
        if rva == 0 or size < (32 if delay else 20):
            raise InvalidPE("invalid import directory")
        width = 32 if delay else 20
        maximum = min(MAX_DESCRIPTORS, size // width)
        out = []
        for i in range(maximum):
            pos = self.off(rva + i * width, width)
            if delay:
                attrs, name, _, iat, int_, _, _, _ = struct.unpack_from("<8I", self.data, pos)
                if all(b == 0 for b in self.data[pos:pos+width]):
                    return out
                if attrs & ~1:
                    raise InvalidPE("unsupported delay-import attributes")
                if not attrs & 1:
                    # Old delay import descriptors store VAs, not RVAs.
                    if name < self.image_base or int_ < self.image_base or iat < self.image_base:
                        raise InvalidPE("invalid VA-based delay-import descriptor")
                    name -= self.image_base
                    int_ -= self.image_base
                    iat -= self.image_base
                thunk = int_ or iat
            else:
                oft, _, _, name, iat = struct.unpack_from("<5I", self.data, pos)
                if not any(self.data[pos:pos+width]):
                    return out
                thunk = oft or iat
            if not name or not thunk:
                raise InvalidPE("missing PE import library name or thunk")
            out.append({"dll": self.text(name), "functions": self.thunk_names(thunk)})
        raise InvalidPE("unterminated PE import descriptor table")


def inspect(data: bytes) -> dict:
    sha = hashlib.sha256(data).hexdigest()
    result = {
        "size": len(data), "sha256": sha,
        "recognized_1_02_executable": len(data) == APPROVED_SIZE and sha == APPROVED_SHA256,
    }
    pe = PE32(data)
    direct = pe.imports()
    delayed = pe.imports(delay=True)
    result["imports"] = direct
    result["delay_imports"] = delayed
    for key, modules in (("static", direct), ("delay", delayed)):
        result[f"{key}_winmm"] = [entry for entry in modules if entry["dll"].lower() == "winmm.dll"]
        result[f"{key}_ddraw"] = [entry for entry in modules if entry["dll"].lower() == "ddraw.dll"]
    result["interpretation"] = (
        "winmm.dll has a static import descriptor (possible startup bootstrap)"
        if result["static_winmm"] else
        "winmm.dll is delay-imported (cannot assume it loads before DirectDraw)"
        if result["delay_winmm"] else
        "winmm.dll has no direct import descriptor (indirect/dynamic loading remains possible)"
    )
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path, help="locally owned, approved 1.02 CRIMSON.EXE")
    parser.add_argument("--json", action="store_true", help="machine-readable result")
    args = parser.parse_args(argv)
    try:
        data = args.exe.read_bytes()
        report = inspect(data)
    except (OSError, InvalidPE) as exc:
        print(f"audit failed: {exc}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print(f"SHA-256: {report['sha256']}")
        print(f"Bytes: {report['size']}")
        print(f"Approved 1.02 EXE: {report['recognized_1_02_executable']}")
        for heading in ("imports", "delay_imports"):
            print(f"{heading}:")
            for item in report[heading]:
                print(f"  {item['dll']}: {', '.join(item['functions'])}")
        print(f"Decision: {report['interpretation']}")
        print("Note: import tables alone cannot prove DLL search order, loader timing, or hook safety.")
    if not report["recognized_1_02_executable"]:
        print("WARNING: not the exact supported Crimson Skies 1.02 executable", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
