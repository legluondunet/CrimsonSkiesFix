#!/usr/bin/env python3
"""Append the exact authenticated dev.13 raster asset pack to an unsigned CI launcher.

No executable code, PE headers or installer DLL bytes are changed. This
experimental packing step is separate from the bare CI build because
the generated art sources are currently delivered as a companion archive.
"""
import argparse, hashlib, struct
from pathlib import Path
SHA = "5cd2e6fa5e897529748a316b814a9f656a3ede0b646385e93858434da5019be8"
SIZE = 2648529
END = b"CSFEND13"
def assemble(exe: bytes, pack: bytes) -> bytes:
    if len(pack) != SIZE or hashlib.sha256(pack).hexdigest() != SHA:
        raise ValueError("art pack hash or size mismatch")
    if len(exe) < 64 or exe[:2] != b"MZ":
        raise ValueError("not a PE executable")
    pe = struct.unpack_from("<I", exe, 60)[0]
    if exe[pe:pe+6] != b"PE\x00\x00\x4c\x01":
        raise ValueError("not PE32 x86")
    if exe[-16:-8] == END:
        raise ValueError("art overlay already present")
    return exe + pack + END + struct.pack("<Q", len(pack))
if __name__ == "__main__":
    p=argparse.ArgumentParser()
    p.add_argument("launcher",type=Path);p.add_argument("art",type=Path);p.add_argument("output",type=Path)
    a=p.parse_args()
    if a.output.resolve()==a.launcher.resolve():
        p.error("output must be distinct from input")
    data=assemble(a.launcher.read_bytes(),a.art.read_bytes())
    a.output.write_bytes(data)
    print(hashlib.sha256(data).hexdigest(),a.output.name)
