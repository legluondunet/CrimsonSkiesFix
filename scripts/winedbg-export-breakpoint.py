#!/usr/bin/env python3
"""Resolve an x86 Wine PE export to its module-relative WineDbg breakpoint.

Copyright (C) 2026 legluondunet
SPDX-License-Identifier: GPL-3.0-or-later

Offline and read-only. No DLL code execution, Wine changes, or game file edits.
"""
import argparse
import os
from pathlib import Path
import struct
import sys


def uint16(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 2 > len(data):
        raise ValueError(f"PE read outside file at 0x{offset:X}")
    return struct.unpack_from("<H", data, offset)[0]


def uint32(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 4 > len(data):
        raise ValueError(f"PE read outside file at 0x{offset:X}")
    return struct.unpack_from("<I", data, offset)[0]


def export_rva(data: bytes, symbol: str) -> tuple[int, bool, str | None]:
    """Return (RVA, is_forwarder, forwarder), rejecting non-PE32 images."""
    if data[:2] != b"MZ" or len(data) < 0x100:
        raise ValueError("Not a DOS/PE image")
    nt = uint32(data, 0x3C)
    if nt > len(data) - 0x100 or data[nt:nt + 4] != b"PE\0\0":
        raise ValueError("Invalid PE signature")
    machine = uint16(data, nt + 4)
    if machine != 0x14C:
        raise ValueError(f"Not an x86 PE32 DLL (Machine=0x{machine:X})")
    section_count = uint16(data, nt + 6)
    opt_size = uint16(data, nt + 20)
    if section_count < 1 or section_count > 96 or opt_size < 104:
        raise ValueError("Invalid PE section count or optional header size")
    opt = nt + 24
    if uint16(data, opt) != 0x10B:
        raise ValueError("Not a PE32 image")
    headers_size = uint32(data, opt + 60)
    export_dir_rva = uint32(data, opt + 96)
    export_dir_size = uint32(data, opt + 100)
    if not export_dir_rva or export_dir_size < 40:
        raise ValueError("No export directory")
    section_table = opt + opt_size
    if section_table + 40 * section_count > len(data):
        raise ValueError("PE section table is truncated")
    sections = []
    for index in range(section_count):
        offset = section_table + index * 40
        virtual_size = uint32(data, offset + 8)
        virtual_address = uint32(data, offset + 12)
        raw_size = uint32(data, offset + 16)
        raw_ptr = uint32(data, offset + 20)
        sections.append((virtual_address, virtual_size, raw_size, raw_ptr))

    def at(rva: int, length: int = 1) -> int:
        if rva < headers_size and rva + length <= min(headers_size, len(data)):
            return rva
        for va, vs, raw_size, raw_ptr in sections:
            if va <= rva and rva + length <= va + min(max(vs, raw_size), raw_size):
                off = raw_ptr + rva - va
                if off + length <= len(data):
                    return off
        raise ValueError(f"RVA 0x{rva:X} not backed by image data")

    directory = at(export_dir_rva, 40)
    functions_count = uint32(data, directory + 20)
    names_count = uint32(data, directory + 24)
    funcs_rva = uint32(data, directory + 28)
    names_rva = uint32(data, directory + 32)
    ordinals_rva = uint32(data, directory + 36)
    if not functions_count or names_count > 100_000 or functions_count > 100_000:
        raise ValueError("Invalid PE export counts")
    funcs_at = at(funcs_rva, 4 * functions_count)
    names_at = at(names_rva, 4 * names_count)
    ordinals_at = at(ordinals_rva, 2 * names_count)
    expected = symbol.encode("ascii")
    for i in range(names_count):
        name_rva = uint32(data, names_at + 4 * i)
        start = at(name_rva)
        end = data.find(b"\0", start, min(start + 512, len(data)))
        if end < 0:
            raise ValueError("Missing NUL in exported function name")
        if data[start:end] != expected:
            continue
        ordinal_index = uint16(data, ordinals_at + 2 * i)
        if ordinal_index >= functions_count:
            raise ValueError("Export ordinal exceeds function table")
        func_rva = uint32(data, funcs_at + 4 * ordinal_index)
        if export_dir_rva <= func_rva < export_dir_rva + export_dir_size:
            forwarded_at = at(func_rva)
            forwarded_end = data.find(b"\0", forwarded_at, min(forwarded_at + 512, len(data)))
            if forwarded_end < 0:
                raise ValueError("Forwarder string is truncated")
            return func_rva, True, data[forwarded_at:forwarded_end].decode("ascii", "replace")
        if func_rva == 0:
            raise ValueError("Export points to a null RVA")
        return func_rva, False, None
    raise ValueError(f"Export name {symbol!r} not found")


def wine32_candidates(prefix: Path) -> list[Path]:
    roots = [
        prefix / "drive_c/windows/syswow64/kernel32.dll",  # WOW64: 32-bit DLL
        prefix / "drive_c/windows/system32/kernel32.dll",  # 32-bit prefix
    ]
    roots.extend(Path(path) for path in (
        "/usr/lib/wine/i386-windows/kernel32.dll",
        "/usr/lib32/wine/i386-windows/kernel32.dll",
        "/usr/lib64/wine/i386-windows/kernel32.dll",
        "/usr/local/lib/wine/i386-windows/kernel32.dll",
    ))
    return roots


def resolve(dll: Path, symbol: str, base: int) -> tuple[int, int]:
    content = dll.read_bytes()
    rva, forwarded, target = export_rva(content, symbol)
    if forwarded:
        raise ValueError(f"{symbol} forwarded to {target}; resolve its target DLL instead")
    addr = base + rva
    if addr > 0xFFFFFFFF:
        raise ValueError("Export address exceeds 32-bit process address space")
    return rva, addr


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", type=lambda x: int(x, 0), required=True,
                        help="Loaded DLL base from WineDbg info share, e.g. 0x7BAB0000")
    parser.add_argument("--dll", type=Path, help="Explicit path to matching x86 PE DLL")
    parser.add_argument("--symbol", default="DeviceIoControl", help="Exact export name")
    args = parser.parse_args()
    if not 0 <= args.base <= 0xFFFFFFFF:
        parser.error("--base must be a 32-bit address")
    if args.dll:
        candidates = [args.dll]
    else:
        env = os.environ.get("WINEPREFIX")
        if not env:
            parser.error("WINEPREFIX is required when --dll is omitted")
        candidates = wine32_candidates(Path(env))
    problems: list[str] = []
    for dll in candidates:
        if not dll.is_file():
            continue
        try:
            rva, address = resolve(dll, args.symbol, args.base)
        except (OSError, ValueError) as exc:
            problems.append(f"{dll}: {exc}")
            continue
        print(f"PE32: {dll}")
        print(f"Export: {args.symbol}, RVA=0x{rva:X}")
        print(f"Loaded DLL base=0x{args.base:08X} => absolute address=0x{address:08X}")
        print("Paste into the existing WineDbg session before 'cont':")
        print(f"  disas 0x{address:08X}")
        print(f"  break *0x{address:08X}")
        print("  info break")
        print("If disassembly looks wrong, DO NOT continue: the mapped image may differ.")
        return
    for message in problems:
        print("SKIPPED: " + message, file=sys.stderr)
    raise SystemExit("Cannot resolve matching x86 PE export; pass --dll explicitly")


if __name__ == "__main__":
    main()
