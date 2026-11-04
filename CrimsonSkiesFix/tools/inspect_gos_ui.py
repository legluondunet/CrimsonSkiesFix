#!/usr/bin/env python3
"""Read-only GOS UI opcode audit of the exact Crimson Skies 1.02 EXE.

Usage: python3 tools/inspect_gos_ui.py "/path/to/CRIMSON.EXE"
Requires only Python 3; does not execute, modify or redistribute game code.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import struct

EXPECTED_SHA = "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75"
EXPECTED_SIZE = 2580480
REGISTRATIONS = {
    "gosCallback": (0x4075D6, 0x407670),
    "uiData": (0x4075F4, 0x4093A0),
    "uiControl": (0x407612, 0x404960),
    "gosPageInit": (0x407630, 0x403680),
    "gosLocalize": (0x40764E, 0x404630),
}

class PE:
    def __init__(self, data: bytes):
        self.data = data
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[:2] != b"MZ" or data[pe:pe+4] != b"PE\0\0":
            raise ValueError("not a PE executable")
        count = struct.unpack_from("<H", data, pe+6)[0]
        optlen = struct.unpack_from("<H", data, pe+20)[0]
        opt = pe+24
        if struct.unpack_from("<H", data, opt)[0] != 0x10B:
            raise ValueError("expected a PE32 executable")
        self.base = struct.unpack_from("<I", data, opt+28)[0]
        head = opt+optlen
        self.sections = []
        for i in range(count):
            pos = head+40*i
            _, virtual, size, raw = struct.unpack_from("<IIII", data, pos+8)
            self.sections.append((virtual, size, raw))

    def read(self, address: int, size: int) -> bytes:
        relative = address-self.base
        for start, length, raw in self.sections:
            if start <= relative and relative+size <= start+length:
                return self.data[raw+relative-start:raw+relative-start+size]
        raise ValueError(f"unmapped executable address {address:#x}")

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.read(address, 4))[0]

def inspect(path: Path) -> dict:
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != EXPECTED_SIZE or digest != EXPECTED_SHA:
        raise ValueError("unsupported executable: size/SHA-256 mismatch")
    exe = PE(data)
    if exe.base != 0x400000:
        raise ValueError("unexpected preferred image base")
    registrations = {}
    for name, (site, target) in REGISTRATIONS.items():
        if exe.read(site, 5) != b"\x68"+struct.pack("<I", target):
            raise ValueError(f"original registration mismatch: {name}")
        registrations[name] = {
            "registration": f"{site:#010x}",
            "handler": f"{target:#010x}",
        }

    def data_target(op: int) -> int:
        if 0x7D0 <= op <= 0x7F6:
            return exe.u32(0x40F518+4*(op-0x7D0))
        if 0x834 <= op <= 0x96D:
            index = exe.read(0x40F788+op-0x834, 1)[0]
            return exe.u32(0x40F5B4+4*index)
        raise ValueError("opcode not in either table")

    targets = {f"{op:#x}": f"{data_target(op):#010x}" for op in (0x83E,0x850,0x855)}
    expected = {
        "0x83e": "0x0040cde2",
        "0x850": "0x0040cc71",
        "0x855": "0x0040cce1",
    }
    if targets != expected:
        raise ValueError("GOS opcode targets differ from verified 1.02")
    expected_bytes = {
        0x40CCE8: bytes.fromhex("a1 70 83 64 00"),
        0x40CCF2: bytes.fromhex("8b 15 6c 83 64 00"),
        0x40CD64: bytes.fromhex("a3 70 83 64 00"),
        0x40CD6E: bytes.fromhex("89 15 6c 83 64 00"),
    }
    for address, original in expected_bytes.items():
        if exe.read(address,len(original)) != original:
            raise ValueError(f"video transfer signature mismatch {address:#x}")
    distinct = len(set(data_target(op) for op in range(0x834,0x96E)))
    return {
        "sha256": digest,
        "preferred_base": f"{exe.base:#010x}",
        "registrations": registrations,
        "uiData_video_opcodes": targets,
        "uiData_second_table_opcodes": 314,
        "uiData_second_table_distinct_handlers": distinct,
        "operation": "read-only; input EXE unchanged",
    }

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(inspect(args.exe), indent=2))
    except (OSError, ValueError) as exc:
        parser.error(str(exc))

if __name__ == "__main__":
    main()
