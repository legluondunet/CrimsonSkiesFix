#!/usr/bin/env python3
"""Verify the frozen dev.14 artwork and assemble migration-only launchers.

Normal consolidated builds embed controls-art.pack in fix.dll. Until the
binary source files are checked in, the exact authenticated overlay remains
supported for explicitly marked test candidates. No PE headers are altered.
"""
import argparse
import hashlib
import struct
import zlib
from pathlib import Path

SHA = "79e6ac7529f3234dd364d752ff5405915d6daf537f5114cc4c1751813a0e6044"
SIZE = 2555973
MAGIC = b"CSFART14"
END = b"CSFEND14"
WIDTH, HEIGHT, COUNT = 1600, 840, 3
RAW = WIDTH * HEIGHT * 4

def decode_pack(pack: bytes) -> list[bytes]:
    if len(pack) < 20 or pack[:8] != MAGIC:
        raise ValueError("invalid art header")
    if struct.unpack_from("<III", pack, 8) != (WIDTH, HEIGHT, COUNT):
        raise ValueError("invalid art dimensions or image count")
    images, pos = [], 20
    for _ in range(COUNT):
        if pos + 4 > len(pack):
            raise ValueError("truncated art length")
        length = struct.unpack_from("<I", pack, pos)[0]
        pos += 4
        if not 0 < length <= RAW or pos + length > len(pack):
            raise ValueError("invalid compressed art length")
        decoder = zlib.decompressobj()
        try:
            raw = decoder.decompress(pack[pos:pos+length], RAW + 1)
        except zlib.error as exc:
            raise ValueError("invalid compressed art") from exc
        if len(raw) != RAW or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
            raise ValueError("wrong expanded image size or trailing zlib data")
        images.append(raw)
        pos += length
    if pos != len(pack):
        raise ValueError("trailing art bytes")
    return images

def verify_pack(pack: bytes) -> None:
    if len(pack) != SIZE or hashlib.sha256(pack).hexdigest() != SHA:
        raise ValueError("art pack hash or size mismatch")
    decode_pack(pack)

def validate_pe(exe: bytes) -> None:
    if len(exe) < 64 or exe[:2] != b"MZ":
        raise ValueError("not a PE executable")
    pe = struct.unpack_from("<I", exe, 60)[0]
    if pe < 64 or pe + 26 > len(exe) or exe[pe:pe+6] != b"PE\x00\x00\x4c\x01":
        raise ValueError("not PE32 x86")
    if struct.unpack_from("<H", exe, pe+24)[0] != 0x10b or struct.unpack_from("<H", exe, pe+22)[0] & 0x2000:
        raise ValueError("not a PE32 x86 executable")

def assemble(exe: bytes, pack: bytes) -> bytes:
    verify_pack(pack)
    validate_pe(exe)
    if exe[-16:-8] == END or pack in exe:
        raise ValueError("art already present")
    return exe + pack + END + struct.pack("<Q", len(pack))

def verify_launcher(exe: bytes, pack: bytes, embedded: bool) -> None:
    verify_pack(pack)
    validate_pe(exe)
    if embedded:
        if pack not in exe or exe[-16:-8] == END:
            raise ValueError("expected embedded art and no migration overlay")
    elif exe[-16:-8] != END or exe[-8:] != struct.pack("<Q", len(pack)) or exe[-16-len(pack):-16] != pack:
        raise ValueError("expected exact authenticated migration overlay")

if __name__ == "__main__":
    import sys
    if len(sys.argv) == 3 and sys.argv[1] == "--verify-pack":
        verify_pack(Path(sys.argv[2]).read_bytes())
        print("CONTROLS-ART PACK PASS")
    elif len(sys.argv) == 4 and sys.argv[1] == "--verify-embedded":
        verify_launcher(Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes(), True)
        print("CONTROLS-ART EMBEDDED PASS")
    elif len(sys.argv) == 4 and sys.argv[1] == "--verify-overlay":
        verify_launcher(Path(sys.argv[2]).read_bytes(), Path(sys.argv[3]).read_bytes(), False)
        print("CONTROLS-ART OVERLAY PASS")
    else:
        p = argparse.ArgumentParser()
        p.add_argument("launcher", type=Path)
        p.add_argument("art", type=Path)
        p.add_argument("output", type=Path)
        a = p.parse_args()
        if a.output.resolve() == a.launcher.resolve():
            p.error("output must be distinct from input")
        pack = a.art.read_bytes()
        data = assemble(a.launcher.read_bytes(), pack)
        verify_launcher(data, pack, False)
        a.output.write_bytes(data)
        print(hashlib.sha256(data).hexdigest(), a.output.name)
