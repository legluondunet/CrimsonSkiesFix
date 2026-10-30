#!/usr/bin/env python3
"""Synthetic PE32 export-table tests; no original game binaries."""
import importlib.util
from pathlib import Path
import struct
import unittest

script=Path(__file__).with_name("winedbg-export-breakpoint.py")
spec=importlib.util.spec_from_file_location("winedbg_export",script)
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def fixture(forward=False,machine=0x14C):
    b=bytearray(0x600)
    b[0:2]=b"MZ"
    struct.pack_into("<I",b,0x3C,0x80)
    b[0x80:0x84]=b"PE\0\0"
    struct.pack_into("<HH",b,0x84,machine,1)
    struct.pack_into("<H",b,0x94,0xE0)
    opt=0x98
    struct.pack_into("<H",b,opt,0x10B)
    struct.pack_into("<I",b,opt+60,0x200)
    struct.pack_into("<II",b,opt+96,0x2000,0x100)
    section=0x178
    struct.pack_into("<IIII",b,section+8,0x400,0x2000,0x400,0x200)
    exp=0x200
    struct.pack_into("<IIIII",b,exp+20,1,1,0x2040,0x2044,0x2048)
    struct.pack_into("<I",b,0x240,0x2080 if forward else 0x3010)
    struct.pack_into("<I",b,0x244,0x2060)
    struct.pack_into("<H",b,0x248,0)
    b[0x260:0x260+16]=b"DeviceIoControl\0"
    if forward:
        b[0x280:0x280+25]=b"KERNELBASE.DeviceIoControl\0"
    return bytes(b)

class Exports(unittest.TestCase):
    def test_normal_export(self):
        self.assertEqual(module.export_rva(fixture(),"DeviceIoControl"),(0x3010,False,None))
    def test_forwarded_export(self):
        self.assertEqual(module.export_rva(fixture(True),"DeviceIoControl"),
                         (0x2080,True,"KERNELBASE.DeviceIoControl"))
    def test_reject_non_x86(self):
        with self.assertRaisesRegex(ValueError,"Not an x86 PE32"):
            module.export_rva(fixture(machine=0x8664),"DeviceIoControl")
    def test_reject_missing_symbol(self):
        with self.assertRaisesRegex(ValueError,"not found"):
            module.export_rva(fixture(),"AnotherFunction")

if __name__=="__main__":
    unittest.main()
