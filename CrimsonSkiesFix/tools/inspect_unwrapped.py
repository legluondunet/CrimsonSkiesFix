#!/usr/bin/env python3
"""Read-only verifier for the user-supplied unwrapped Crimson Skies PE32.

Never patches the executable; prints raw source bytes at verified instruction
boundaries. Intended for offline reverse-engineering, not as a runtime gate.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

KNOWN_HASH = "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75"
SITES = {
    "mode_1024x768_first": (0x41229, bytes.fromhex("68 00 03 00 00 68 00 04 00 00")),
    "mode_1024x768_second": (0x41241, bytes.fromhex("68 00 03 00 00 68 00 04 00 00")),
    "dispatch_width_1024": (0x1A8FA7, bytes.fromhex("b9 00 04 00 00")),
    "dispatch_height_768": (0x1A8FC9, bytes.fromhex("b9 00 03 00 00")),
    "expected_fov_cave_empty": (0x2024BD, bytes(12)),
}

def inspect(data: bytes) -> dict:
    out = {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data),
           "recognized": False, "sites": {}}
    if len(data) < 0x100 or data[:2] != b"MZ":
        out["error"] = "invalid DOS header"
        return out
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if pe > len(data) - 0xF8 or data[pe:pe+4] != b"PE\0\0":
        out["error"] = "invalid PE signature"
        return out
    machine = struct.unpack_from("<H", data, pe+4)[0]
    opt = pe+24
    magic = struct.unpack_from("<H", data, opt)[0]
    image_base = struct.unpack_from("<I", data, opt+28)[0]
    image_size = struct.unpack_from("<I", data, opt+56)[0]
    out.update(machine=machine, image_base=image_base, image_size=image_size)
    if machine != 0x14c or magic != 0x10b:
        out["error"] = "not x86 PE32"
        return out
    count = struct.unpack_from("<H", data, pe+6)[0]
    headers = struct.unpack_from("<H", data, pe+20)[0]
    table = opt+headers
    sections = []
    for i in range(count):
        at=table+i*40
        if at+40 > len(data):
            out["error"] = "truncated section headers"
            return out
        name=data[at:at+8].rstrip(b"\0").decode("ascii","replace")
        vsize,rva,raw_size,raw_off=struct.unpack_from("<IIII",data,at+8)
        sections.append((name,vsize,rva,raw_size,raw_off))
    def read_rva(rva: int, length: int):
        for name,vsize,start,raw_size,raw_off in sections:
            if start <= rva and rva+length <= start+raw_size and rva+length <= start+max(vsize,raw_size):
                offset=raw_off+rva-start
                return data[offset:offset+length] if offset+length <= len(data) else None
        return None
    for name,(rva,expected) in SITES.items():
        actual=read_rva(rva,len(expected))
        out["sites"][name]={"rva":f"0x{rva:x}",
                            "actual":actual.hex(" ") if actual is not None else None,
                            "matches":actual==expected}
    out["recognized"]=out["sha256"]==KNOWN_HASH and all(v["matches"] for v in out["sites"].values())
    return out

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("exe",type=Path)
    args=p.parse_args()
    print(json.dumps(inspect(args.exe.read_bytes()),indent=2))
if __name__=="__main__":
    main()
