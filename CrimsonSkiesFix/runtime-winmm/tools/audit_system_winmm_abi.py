#!/usr/bin/env python3
"""Compare the x86 WinMM candidate export ABI with the runner's real x86 system WinMM.

Diagnostic only: named WinMM functions can work while their accidental ordinal
numbers silently resolve to unrelated APIs in third-party DLLs. This report
must be reviewed before the 1.1 release and must not be treated as a green
compatibility assertion unless all conflicts have been resolved.
"""
from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path


class PEError(ValueError):
    pass


def export_table(path: Path) -> dict:
    data = path.read_bytes()

    def u16(pos: int) -> int:
        if pos < 0 or pos + 2 > len(data):
            raise PEError(f"{path}: truncated u16 at {pos:#x}")
        return struct.unpack_from("<H", data, pos)[0]

    def u32(pos: int) -> int:
        if pos < 0 or pos + 4 > len(data):
            raise PEError(f"{path}: truncated u32 at {pos:#x}")
        return struct.unpack_from("<I", data, pos)[0]

    if len(data) < 256 or data[:2] != b"MZ":
        raise PEError(f"{path}: not a PE image")
    header = u32(0x3c)
    if data[header:header + 4] != b"PE\0\0":
        raise PEError(f"{path}: missing PE signature")
    if u16(header + 4) != 0x14c:
        raise PEError(f"{path}: expected 32-bit x86 WinMM")
    count = u16(header + 6)
    opt_size = u16(header + 20)
    opt = header + 24
    if u16(opt) != 0x10b or opt_size < 104:
        raise PEError(f"{path}: expected PE32 optional header")
    export_rva, export_size = u32(opt + 96), u32(opt + 100)
    if export_rva == 0 or export_size < 40:
        raise PEError(f"{path}: missing export data directory")

    sections = []
    for i in range(count):
        pos = opt + opt_size + 40 * i
        virt_size, virt_rva = u32(pos + 8), u32(pos + 12)
        raw_size, raw_offset = u32(pos + 16), u32(pos + 20)
        sections.append((virt_rva, max(virt_size, raw_size), raw_offset))

    def offset(rva: int, size: int = 1) -> int:
        for start, span, raw in sections:
            if start <= rva and size <= span and rva - start <= span - size:
                pos = raw + rva - start
                if pos + size <= len(data):
                    return pos
        raise PEError(f"{path}: unmapped export RVA {rva:#x}")

    def export_name(rva: int) -> str:
        pos = offset(rva)
        end = data.find(b"\0", pos, min(len(data), pos + 512))
        if end < 0:
            raise PEError(f"{path}: unterminated export name at {rva:#x}")
        return data[pos:end].decode("ascii")

    root = offset(export_rva, 40)
    base = u32(root + 16)
    func_count = u32(root + 20)
    name_count = u32(root + 24)
    if not base or not func_count or not name_count or func_count > 32768 or name_count > 4096:
        raise PEError(f"{path}: suspicious export dimensions")
    funcs = offset(u32(root + 28), func_count * 4)
    names = offset(u32(root + 32), name_count * 4)
    indices = offset(u32(root + 36), name_count * 2)
    occupied = {base + i for i in range(func_count) if u32(funcs + 4 * i) != 0}
    named = {}
    ordinal_names = {}
    for i in range(name_count):
        name = export_name(u32(names + 4 * i))
        index = u16(indices + 2 * i)
        if index >= func_count or name in named:
            raise PEError(f"{path}: invalid or duplicate export {name!r}")
        ordinal = base + index
        if ordinal not in occupied:
            raise PEError(f"{path}: null export {name!r}")
        named[name] = ordinal
        ordinal_names.setdefault(ordinal, []).append(name)
    return {
        "path": str(path), "machine": "PE32 i386", "base": base,
        "function_slots": func_count, "named_count": name_count,
        "named": named, "occupied": sorted(occupied),
        "ordinal_names": ordinal_names,
    }


def compare(candidate: dict, system: dict) -> dict:
    cn, sn = candidate["named"], system["named"]
    mismatches = [
        {"name": n, "candidate_ordinal": cn[n], "system_ordinal": sn[n]}
        for n in sorted(cn.keys() & sn.keys()) if cn[n] != sn[n]
    ]
    collisions = []
    for ordinal in sorted(set(candidate["occupied"]) & set(system["occupied"])):
        actual = set(system["ordinal_names"].get(ordinal, []))
        offered = set(candidate["ordinal_names"].get(ordinal, []))
        if not actual or offered.isdisjoint(actual):
            collisions.append({
                "ordinal": ordinal,
                "candidate_names": sorted(offered),
                "system_names": sorted(actual),
                "system_ordinal_only": not bool(actual),
            })
    return {
        "candidate": candidate["path"], "system": system["path"],
        "candidate_named_exports": len(cn),
        "system_named_exports": len(sn),
        "matched_names": len(cn.keys() & sn.keys()),
        "missing_from_candidate": sorted(sn.keys() - cn.keys()),
        "extra_in_candidate": sorted(cn.keys() - sn.keys()),
        "named_ordinal_mismatches": mismatches,
        "occupied_ordinal_collisions": collisions,
        "candidate_reserved_1_to_4": {
            str(n): candidate["ordinal_names"].get(n, []) for n in range(1, 5)
        },
        "system_reserved_1_to_4": {
            str(n): system["ordinal_names"].get(n, []) for n in range(1, 5)
        },
    }


def main() -> int:
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--candidate", type=Path, required=True)
    cli.add_argument("--system", type=Path, required=True)
    cli.add_argument("--json", type=Path, required=True)
    cli.add_argument("--strict", action="store_true",
                     help="fail when named ordinals differ or occupied ordinal slots collide")
    args = cli.parse_args()
    try:
        report = compare(export_table(args.candidate), export_table(args.system))
    except (OSError, PEError, ValueError, UnicodeError) as error:
        cli.exit(2, f"WINMM-SYSTEM-ABI-AUDIT FAIL: {error}\n")
    args.json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    mismatch = len(report["named_ordinal_mismatches"])
    collision = len(report["occupied_ordinal_collisions"])
    print(f"WINMM-SYSTEM-ABI-AUDIT: matched_names={report['matched_names']} "
          f"named_ordinal_mismatches={mismatch} occupied_ordinal_collisions={collision}")
    for item in report["named_ordinal_mismatches"][:12]:
        print(f"  name={item['name']} candidate=@{item['candidate_ordinal']} "
              f"system=@{item['system_ordinal']}")
    for item in report["occupied_ordinal_collisions"][:12]:
        print(f"  collision=@{item['ordinal']} candidate={item['candidate_names']} "
              f"system={item['system_names']}")
    print(f"Full report: {args.json} (diagnostic; mismatches do not pass release gate)")
    return 1 if args.strict and (mismatch or collision) else 0


if __name__ == "__main__":
    raise SystemExit(main())
