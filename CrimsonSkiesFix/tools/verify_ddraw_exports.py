#!/usr/bin/env python3
"""Independently verify complete Windows x86 DDRAW DLL exports.

Crimson Skies hardware acceleration works on Windows 11 with all 22 names
and canonical ordinals, but not with the earlier three-export proxy.
Does not execute the inspected DLL or require third-party packages.
"""
from __future__ import annotations

import argparse
import re
import struct
import sys
from pathlib import Path

COUNT = 22


class ExportError(ValueError):
    pass


def expected(definition: Path) -> dict[str, int]:
    result: dict[str, int] = {}
    for line in definition.read_text(encoding="utf-8-sig").splitlines():
        line = line.split(";", 1)[0].strip()
        if not line or line.upper() == "EXPORTS" or line.upper().startswith("LIBRARY "):
            continue
        match = re.fullmatch(
            r"([A-Za-z][A-Za-z0-9_]*)\s*(?:=\s*([A-Za-z_][A-Za-z0-9_]*))?\s+@(\d+)",
            line,
        )
        if match is None:
            raise ExportError(f"invalid .def export: {line!r}")
        name, _internal, ordinal = match.groups()
        if name in result:
            raise ExportError(f"duplicate .def export name: {name}")
        result[name] = int(ordinal)
    if len(result) != COUNT or set(result.values()) != set(range(1, COUNT + 1)):
        raise ExportError("definition must contain exactly 22 exports with ordinals 1..22")
    return result


def exports(dll: Path) -> dict[str, int]:
    data = dll.read_bytes()
    if len(data) < 512 or data[:2] != b"MZ":
        raise ExportError("not a PE image")

    def u16(pos: int) -> int:
        if pos < 0 or pos + 2 > len(data):
            raise ExportError("truncated PE U16")
        return struct.unpack_from("<H", data, pos)[0]

    def u32(pos: int) -> int:
        if pos < 0 or pos + 4 > len(data):
            raise ExportError("truncated PE U32")
        return struct.unpack_from("<I", data, pos)[0]

    pe = u32(0x3C)
    if pe < 0x40 or pe + 24 > len(data) or data[pe:pe + 4] != b"PE\0\0":
        raise ExportError("bad PE header")
    if u16(pe + 4) != 0x14C or not (u16(pe + 22) & 0x2000):
        raise ExportError("expected a Windows x86 DLL")
    sections = u16(pe + 6)
    optional_size = u16(pe + 20)
    optional = pe + 24
    if u16(optional) != 0x10B or optional_size < 104:
        raise ExportError("invalid PE32 optional header")
    if u32(optional + 92) < 1:
        raise ExportError("missing export directory data entry")
    export_rva, export_size = u32(optional + 96), u32(optional + 100)
    if not export_rva or export_size < 40:
        raise ExportError("missing PE export directory")
    section_header = optional + optional_size
    section_info = []
    for index in range(sections):
        s = section_header + index * 40
        if s + 40 > len(data):
            raise ExportError("truncated section header")
        section_info.append((u32(s + 12), max(u32(s + 8), u32(s + 16)), u32(s + 20)))

    def offset(rva: int, length: int = 1) -> int:
        for va, size, raw in section_info:
            if va <= rva and rva + length <= va + size:
                filepos = raw + (rva - va)
                if filepos + length <= len(data):
                    return filepos
        raise ExportError(f"unmapped or truncated RVA {rva:#x}")

    def read_name(rva: int) -> str:
        pos = offset(rva)
        end = data.find(b"\0", pos, min(len(data), pos + 256))
        if end == -1:
            raise ExportError("unterminated export name")
        try:
            return data[pos:end].decode("ascii")
        except UnicodeDecodeError as exc:
            raise ExportError("non-ASCII export name") from exc

    table = offset(export_rva, 40)
    # IMAGE_EXPORT_DIRECTORY: characteristics, timestamp, version, dll-name,
    # ordinal base, function/name counts, 3 array RVAs.
    base = u32(table + 16)
    functions = u32(table + 20)
    names = u32(table + 24)
    addr_funcs = u32(table + 28)
    addr_names = u32(table + 32)
    addr_ordinals = u32(table + 36)
    if base != 1 or functions != COUNT or names != COUNT:
        # An explicit .def may be combined with Rust's no_mangle exports.
        # List unexpected exports before refusing the candidate, to make
        # linker/ABI issues actionable without relaxing this strict gate.
        entries: list[str] = []
        if 0 < names <= 64 and 0 < functions <= 64:
            name_table = offset(addr_names, 4 * names)
            ordinal_table = offset(addr_ordinals, 2 * names)
            for index in range(names):
                entry_name = read_name(u32(name_table + 4 * index))
                ordinal = base + u16(ordinal_table + 2 * index)
                entries.append(f"{entry_name}@{ordinal}")
        raise ExportError(
            f"invalid export shape: base={base}, funcs={functions}, names={names}; "
            f"actual={sorted(entries)}"
        )
    f = offset(addr_funcs, 4 * functions)
    n = offset(addr_names, 4 * names)
    o = offset(addr_ordinals, 2 * names)
    observed: dict[str, int] = {}
    ordinal_indexes: set[int] = set()
    for index in range(names):
        ordinal_index = u16(o + 2 * index)
        if ordinal_index >= functions or ordinal_index in ordinal_indexes:
            raise ExportError("invalid or duplicated ordinal index")
        ordinal_indexes.add(ordinal_index)
        target_rva = u32(f + 4 * ordinal_index)
        if not target_rva or export_rva <= target_rva < export_rva + export_size:
            # PE forwarding to another ddraw.dll could recurse into ourselves.
            raise ExportError("null export RVA or unsafe PE forwarding entry")
        name = read_name(u32(n + 4 * index))
        if name in observed:
            raise ExportError(f"duplicated PE export: {name}")
        observed[name] = base + ordinal_index
    return observed


def verify(dll: Path, definition: Path) -> None:
    wanted, observed = expected(definition), exports(dll)
    if wanted != observed:
        missing = sorted(set(wanted) - set(observed))
        extra = sorted(set(observed) - set(wanted))
        wrong = sorted(
            (name, wanted[name], observed[name])
            for name in set(wanted) & set(observed)
            if wanted[name] != observed[name]
        )
        raise ExportError(
            f"DDRAW exports differ: missing={missing}, extra={extra}, wrong_ordinals={wrong}"
        )
    print("DDRAW-EXPORTS: PASS: 22 exact native names/ordinals, PE32 x86, no forwarders")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dll", required=True, type=Path)
    parser.add_argument("--definitions", required=True, type=Path)
    args = parser.parse_args()
    try:
        verify(args.dll, args.definitions)
    except (ExportError, OSError) as exc:
        print(f"DDRAW-EXPORTS: FAIL: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
