#!/usr/bin/env python3
"""Synthetic PE fixtures: no copyrighted game image, Python-only tests."""
from __future__ import annotations

import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("audit_game_winmm_imports", HERE / "audit_game_winmm_imports.py")
assert spec and spec.loader
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def fixture(*, dll_name: bytes = b"WINMM.dll", by_ordinal: bool = True, delay: bool = False) -> bytes:
    data = bytearray(4096)
    def w16(pos: int, n: int) -> None: struct.pack_into("<H", data, pos, n)
    def w32(pos: int, n: int) -> None: struct.pack_into("<I", data, pos, n)
    data[:2] = b"MZ"
    w32(0x3c, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    w16(0x84, 0x14c)
    w16(0x86, 1)
    w16(0x94, 0xe0)
    opt = 0x98
    w16(opt, 0x10b)
    w32(opt + 28, 0x400000)
    w32(opt + 60, 0x200)
    w32(opt + 92, 16)
    sec = opt + 0xe0
    data[sec:sec + 5] = b".rdata"
    w32(sec + 8, 0x800)
    w32(sec + 12, 0x1000)
    w32(sec + 16, 0x800)
    w32(sec + 20, 0x200)
    if delay:
        w32(opt + 112 + 13 * 8 - 16, 0x1200)  # PE32 data directory starts at +96.
        w32(opt + 112 + 13 * 8 - 12, 64)
        w32(0x400, 1)  # grAttrs = RVA
        w32(0x404, 0x1250)  # DLL name
        w32(0x410, 0x1260)  # delay INT
        data[0x450:0x450 + len(dll_name) + 1] = dll_name + b"\0"
        w32(0x460, 0x80000002 if by_ordinal else 0x1280)
        if not by_ordinal:
            data[0x480:0x480 + 10] = b"\0\0timeGetTime"
    else:
        w32(opt + 96 + 1 * 8, 0x1100)
        w32(opt + 100 + 1 * 8, 40)
        w32(0x300, 0x11a0)  # OriginalFirstThunk
        w32(0x30c, 0x1180)  # import descriptor Name
        w32(0x310, 0x11a0)  # import descriptor FirstThunk
        data[0x380:0x380 + len(dll_name) + 1] = dll_name + b"\0"
        w32(0x3a0, 0x80000012 if by_ordinal else 0x11c0)
        if not by_ordinal:
            data[0x3c0:0x3c0 + 13] = b"\0\0timeGetTime\0"
    return bytes(data)


class WinmmOrdinalAuditTest(unittest.TestCase):
    def test_static_ordinal_import(self):
        self.assertEqual(audit.imports_from_image(fixture()), [
            {"kind": "static", "dll": "WINMM.dll", "by": "ordinal", "value": 18}
        ])

    def test_static_named_import(self):
        self.assertEqual(audit.imports_from_image(fixture(by_ordinal=False)), [
            {"kind": "static", "dll": "WINMM.dll", "by": "name", "value": "timeGetTime"}
        ])

    def test_unrelated_library_ignored(self):
        self.assertEqual(audit.imports_from_image(fixture(dll_name=b"DDRAW.dll")), [])

    def test_delay_ordinal_import(self):
        self.assertEqual(audit.imports_from_image(fixture(delay=True)), [
            {"kind": "delay", "dll": "WINMM.dll", "by": "ordinal", "value": 2}
        ])

    def test_broken_descriptor_fails_closed(self):
        data = bytearray(fixture())
        struct.pack_into("<I", data, 0x98 + 96 + 1 * 8 + 4, 20)
        with self.assertRaises(audit.ImportErrorPE):
            audit.imports_from_image(bytes(data))

    def test_directory_scan_does_not_modify_contents(self):
        with tempfile.TemporaryDirectory() as root:
            game = Path(root)
            exe = game / "CRIMSON.EXE"
            exe.write_bytes(fixture())
            (game / "winmm.dll").write_bytes(b"ignored, provider not client")
            (game / "OTHER.DLL").write_bytes(fixture(by_ordinal=False))
            before = exe.read_bytes()
            report = audit.audit(game)
            self.assertEqual(len(report["ordinal_clients"]), 1)
            self.assertEqual(report["ordinal_clients"][0]["file"], "CRIMSON.EXE")
            self.assertFalse(report["inspection_errors"])
            self.assertEqual(exe.read_bytes(), before)

    def test_malformed_exe_is_reported(self):
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / "bad.exe").write_bytes(b"MZ" + b"\0" * 300)
            report = audit.audit(Path(root))
            self.assertEqual(len(report["inspection_errors"]), 1)


if __name__ == "__main__":
    unittest.main()
