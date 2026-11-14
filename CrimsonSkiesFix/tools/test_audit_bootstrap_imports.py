#!/usr/bin/env python3
"""Synthetic-only PE32 regression tests; never require licensed game bytes."""

import struct
import unittest
import audit_bootstrap_imports as audit


def fixture(static_names=(), delay_names=()):
    data = bytearray(0x1200)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, 0x14C, 1)
    struct.pack_into("<H", data, 0x94, 224)
    opt = 0x98
    struct.pack_into("<H", data, opt, 0x10B)
    struct.pack_into("<I", data, opt + 28, 0x400000)
    struct.pack_into("<I", data, opt + 60, 0x200)
    struct.pack_into("<I", data, opt + 92, 16)
    sh = opt + 224
    data[sh:sh+8] = b".idata\0\0"
    struct.pack_into("<IIII", data, sh+8, 0x1000, 0x1000, 0x1000, 0x200)

    def put(rva, payload):
        off = rva - 0x1000 + 0x200
        data[off:off+len(payload)] = payload

    if static_names:
        struct.pack_into("<II", data, opt + 104, 0x1000, (len(static_names)+1)*20)
        for i, name in enumerate(static_names):
            desc = (0x1000 + i*20)
            thunk = 0x1100 + i*0x20
            lib = 0x1300 + i*0x20
            fn = 0x1400 + i*0x20
            put(desc, struct.pack("<5I", thunk, 0, 0, lib, thunk + 0x100))
            put(thunk, struct.pack("<II", fn, 0))
            put(lib, name.encode() + b"\0")
            put(fn, b"\0\0" + (b"timeGetTime" if name.lower()=="winmm.dll" else b"DirectDrawCreate") + b"\0")

    if delay_names:
        struct.pack_into("<II", data, opt + 200, 0x1600, (len(delay_names)+1)*32)
        for i, name in enumerate(delay_names):
            desc = 0x1600+i*32
            lib = 0x1700+i*0x20
            thunk = 0x1800+i*0x20
            fn = 0x1900+i*0x20
            put(desc, struct.pack("<8I", 1, lib, 0, thunk+0x80, thunk, 0, 0, 0))
            put(lib, name.encode() + b"\0")
            put(thunk, struct.pack("<II", fn, 0))
            put(fn, b"\0\0timeGetTime\0")
    return bytes(data)


class TestAudit(unittest.TestCase):
    def test_static_winmm_and_ddraw(self):
        report = audit.inspect(fixture(static_names=("WINMM.dll", "DDRAW.dll")))
        self.assertEqual(report["static_winmm"][0]["functions"], ["timeGetTime"])
        self.assertEqual(report["static_ddraw"][0]["functions"], ["DirectDrawCreate"])
        self.assertFalse(report["recognized_1_02_executable"])

    def test_delay_only_is_not_startup_evidence(self):
        report = audit.inspect(fixture(delay_names=("winmm.dll",)))
        self.assertEqual(report["static_winmm"], [])
        self.assertEqual(report["delay_winmm"][0]["functions"], ["timeGetTime"])
        self.assertIn("delay-imported", report["interpretation"])

    def test_no_winmm(self):
        report = audit.inspect(fixture(static_names=("ddraw.dll",)))
        self.assertFalse(report["static_winmm"])
        self.assertFalse(report["delay_winmm"])
        self.assertIn("no direct import", report["interpretation"])

    def test_truncated_and_corrupted_headers_fail_closed(self):
        with self.assertRaises(audit.InvalidPE):
            audit.inspect(b"MZ")
        data = bytearray(fixture(static_names=("winmm.dll",)))
        struct.pack_into("<I", data, 0x300, 0x7FFF_FFFF)
        with self.assertRaises(audit.InvalidPE):
            audit.inspect(bytes(data))

    def test_import_by_ordinal(self):
        data = bytearray(fixture(static_names=("winmm.dll",)))
        struct.pack_into("<I", data, 0x300, 0x8000_0022)
        report = audit.inspect(bytes(data))
        self.assertEqual(report["static_winmm"][0]["functions"], ["#34"])


if __name__ == "__main__":
    unittest.main()
