#!/usr/bin/env python3
"""Read-only PE32/PE32+ WinMM import inventory for an installed game directory.

Reports static and delay-load imports, including unsafe imports by ordinal.
This does NOT detect a later dynamic GetProcAddress/LoadLibrary sequence.
No game files are modified, and the tool does not run any executable.
"""
from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path


class ImportErrorPE(ValueError):
    pass


def imports_from_image(data: bytes) -> list[dict]:
    def u16(pos: int) -> int:
        if pos < 0 or pos + 2 > len(data):
            raise ImportErrorPE(f"truncated PE u16 at {pos:#x}")
        return struct.unpack_from("<H", data, pos)[0]

    def u32(pos: int) -> int:
        if pos < 0 or pos + 4 > len(data):
            raise ImportErrorPE(f"truncated PE u32 at {pos:#x}")
        return struct.unpack_from("<I", data, pos)[0]

    def u64(pos: int) -> int:
        if pos < 0 or pos + 8 > len(data):
            raise ImportErrorPE(f"truncated PE u64 at {pos:#x}")
        return struct.unpack_from("<Q", data, pos)[0]

    if len(data) < 0x100 or data[:2] != b"MZ":
        raise ImportErrorPE("not a PE image")
    pe = u32(0x3c)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ImportErrorPE("missing PE signature")
    num_sections = u16(pe + 6)
    opt_size = u16(pe + 20)
    opt = pe + 24
    magic = u16(opt)
    if magic == 0x10b:
        dd_base, image_base, word_size, ordinal_flag = 96, u32(opt + 28), 4, 0x80000000
    elif magic == 0x20b:
        dd_base, image_base, word_size, ordinal_flag = 112, u64(opt + 24), 8, 0x8000000000000000
    else:
        raise ImportErrorPE(f"unexpected PE magic {magic:#x}")
    if opt + opt_size > len(data) or opt_size < dd_base or not 0 < num_sections <= 96:
        raise ImportErrorPE("invalid PE headers/section count")
    section_header = opt + opt_size
    sections: list[tuple[int, int, int, int]] = []
    for index in range(num_sections):
        pos = section_header + index * 40
        virt_size, virt_addr = u32(pos + 8), u32(pos + 12)
        raw_size, raw_off = u32(pos + 16), u32(pos + 20)
        sections.append((virt_addr, max(virt_size, raw_size), raw_size, raw_off))
    size_headers = u32(opt + 60)

    def rva_offset(rva: int, size: int = 1) -> int:
        if rva < size_headers and rva + size <= min(size_headers, len(data)):
            return rva
        for start, vsize, raw_size, raw_off in sections:
            delta = rva - start
            if 0 <= delta and delta + size <= min(vsize, raw_size):
                offset = raw_off + delta
                if offset + size <= len(data):
                    return offset
        raise ImportErrorPE(f"unmapped or truncated RVA {rva:#x}")

    def cstr(rva: int) -> str:
        offset = rva_offset(rva)
        end = data.find(b"\0", offset, min(offset + 260, len(data)))
        if end == -1:
            raise ImportErrorPE(f"unterminated import DLL/name at RVA {rva:#x}")
        return data[offset:end].decode("ascii")

    directory_count = u32(opt + (92 if word_size == 4 else 108))
    def directory(index: int) -> tuple[int, int]:
        if directory_count <= index or dd_base + 8 * (index + 1) > opt_size:
            return (0, 0)
        pos = opt + dd_base + 8 * index
        return u32(pos), u32(pos + 4)

    def normal_rva(value: int, attrs: int) -> int:
        if attrs & 1:
            return value
        if value < image_base:
            raise ImportErrorPE("delay import VA before ImageBase")
        return value - image_base

    results = []
    def collect_thunks(dll_name: str, thunk_rva: int, kind: str) -> None:
        if not thunk_rva:
            raise ImportErrorPE(f"empty {kind} thunk for WinMM")
        for index in range(16384):
            ofs = rva_offset(thunk_rva + word_size * index, word_size)
            thunk = u32(ofs) if word_size == 4 else u64(ofs)
            if thunk == 0:
                return
            if thunk & ordinal_flag:
                # Windows PE ordinal imports store the ordinal in the low 16 bits.
                if thunk & ~(ordinal_flag | 0xffff):
                    raise ImportErrorPE("invalid reserved ordinal thunk bits")
                results.append({"kind": kind, "dll": dll_name, "by": "ordinal", "value": thunk & 0xffff})
            else:
                name_rva = thunk
                name_ofs = rva_offset(name_rva, 3)
                _hint = u16(name_ofs)
                results.append({"kind": kind, "dll": dll_name, "by": "name", "value": cstr(name_rva + 2)})
        raise ImportErrorPE(f"unterminated {kind} thunk table")

    imp_rva, imp_size = directory(1)
    if imp_rva and imp_size:
        complete = False
        for index in range(min(imp_size // 20, 8192)):
            ofs = rva_offset(imp_rva + index * 20, 20)
            original, name_rva, first = u32(ofs), u32(ofs + 12), u32(ofs + 16)
            if data[ofs:ofs + 20] == b"\0" * 20:
                complete = True
                break
            if name_rva and cstr(name_rva).lower() == "winmm.dll":
                collect_thunks("WINMM.dll", original or first, "static")
        if not complete:
            raise ImportErrorPE("unterminated normal import descriptors")

    delay_rva, delay_size = directory(13)
    if delay_rva and delay_size:
        complete = False
        for index in range(min(delay_size // 32, 8192)):
            ofs = rva_offset(delay_rva + index * 32, 32)
            if data[ofs:ofs + 32] == b"\0" * 32:
                complete = True
                break
            attrs, dll_ptr, iat, int_ptr = u32(ofs), u32(ofs + 4), u32(ofs + 12), u32(ofs + 16)
            if attrs not in (0, 1):
                raise ImportErrorPE("unknown delay import pointer format")
            if dll_ptr and cstr(normal_rva(dll_ptr, attrs)).lower() == "winmm.dll":
                collect_thunks("WINMM.dll", normal_rva(int_ptr or iat, attrs), "delay")
        if not complete:
            raise ImportErrorPE("unterminated delay import descriptors")
    return results


def audit(directory: Path, max_file_bytes: int = 64 * 1024 * 1024) -> dict:
    if not directory.is_dir() or directory.is_symlink():
        raise ImportErrorPE("game directory missing or symlinked")
    entries = []
    errors = []
    for path in sorted(directory.iterdir(), key=lambda p: p.name.casefold()):
        if path.suffix.lower() not in (".exe", ".dll"):
            continue
        if path.name.casefold() == "winmm.dll":
            # Inspect the actual clients of WinMM, not the replacement provider itself.
            continue
        if path.is_symlink() or not path.is_file():
            errors.append({"file": path.name, "error": "not a regular local file"})
            continue
        if path.stat().st_size > max_file_bytes:
            errors.append({"file": path.name, "error": "PE candidate exceeds audit size bound"})
            continue
        try:
            image = path.read_bytes()
            # An empty/dummy non-PE file is not a loadable Win32 client.
            if not image.startswith(b"MZ"):
                continue
            parsed = imports_from_image(image)
            if parsed:
                entries.append({"file": path.name, "imports": parsed})
        except (OSError, UnicodeError, ImportErrorPE) as exc:
            errors.append({"file": path.name, "error": str(exc)})
    ordinal_clients = [
        {"file": entry["file"], **item}
        for entry in entries for item in entry["imports"] if item["by"] == "ordinal"
    ]
    return {
        "directory": str(directory), "static_delay_imports": entries,
        "ordinal_clients": ordinal_clients, "inspection_errors": errors,
        "scope": "top-level PE files only; dynamically loaded modules and GetProcAddress not detected",
    }


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("game_dir", type=Path)
    cli.add_argument("--json", type=Path, help="write read-only JSON report")
    cli.add_argument("--strict", action="store_true",
                     help="exit nonzero if WinMM ordinal imports or inspection errors are found")
    args = cli.parse_args()
    try:
        report = audit(args.game_dir)
    except (OSError, ImportErrorPE) as exc:
        cli.exit(2, f"WINMM-IMPORT-PREFLIGHT FAIL: {exc}\n")
    if args.json:
        args.json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    count = sum(len(e["imports"]) for e in report["static_delay_imports"])
    ordinal = report["ordinal_clients"]
    errors = report["inspection_errors"]
    print(f"WINMM-IMPORT-PREFLIGHT: imports={count} ordinal_imports={len(ordinal)} "
          f"inspection_errors={len(errors)}")
    for item in ordinal:
        print(f"  UNSAFE: {item['file']}: {item['kind']} WINMM ordinal #{item['value']}")
    for item in errors:
        print(f"  UNKNOWN: {item['file']}: {item['error']}")
    print(f"Scope: {report['scope']}")
    if args.strict and (ordinal or errors):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
