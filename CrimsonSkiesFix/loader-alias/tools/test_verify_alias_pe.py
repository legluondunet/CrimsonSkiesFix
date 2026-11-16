#!/usr/bin/env python3
"""Synthetic PE fixtures: the CSFIX dependency gate must fail closed."""
import struct
import unittest
from verify_alias_pe import PeError, verify


def fixture(import_name=b'KERNEL32.dll', export_names=(b'DirectDrawCreate', b'DirectDrawEnumerateA')):
    b = bytearray(2048)
    b[0:2] = b'MZ'
    struct.pack_into('<I', b, 0x3c, 0x80)
    b[0x80:0x84] = b'PE\x00\x00'
    struct.pack_into('<H', b, 0x84, 0x14c)
    struct.pack_into('<H', b, 0x86, 1)
    struct.pack_into('<H', b, 0x94, 224)
    opt = 0x98
    struct.pack_into('<H', b, opt, 0x10b)
    struct.pack_into('<I', b, opt+60, 512)
    struct.pack_into('<I', b, opt+92, 16)
    struct.pack_into('<II', b, opt+96, 0x1100, 256)
    struct.pack_into('<II', b, opt+104, 0x1000, 40)
    section = opt+224
    struct.pack_into('<III', b, section+12, 0x1000, 0x600, 0x200)
    struct.pack_into('<I', b, 0x200+12, 0x1080)
    b[0x280:0x280+len(import_name)+1] = import_name+b'\x00'
    struct.pack_into('<I', b, 0x300+24, len(export_names))
    struct.pack_into('<I', b, 0x300+32, 0x1140)
    for i, name in enumerate(export_names):
        rva = 0x1180 + i*0x30
        struct.pack_into('<I', b, 0x340+i*4, rva)
        off = 0x380+i*0x30
        b[off:off+len(name)+1] = name+b'\0'
    return bytes(b)


class TestAliasPe(unittest.TestCase):
    def test_valid_alias(self):
        self.assertEqual(len(verify(fixture())[1]), 2)

    def test_winmm_dependency_forbidden(self):
        for name in (b'winmm.dll', b'WINMM.DLL', b'ddraw.dll', b'CrimsonSkiesFix.dll'):
            with self.subTest(name=name), self.assertRaises(PeError):
                verify(fixture(import_name=name))

    def test_missing_alias_export_fails(self):
        with self.assertRaises(PeError):
            verify(fixture(export_names=(b'DirectDrawCreate', b'unrelated')))

    def test_bad_delay_import_fails(self):
        b=bytearray(fixture())
        struct.pack_into('<II', b, 0x98+96+13*8, 0x1200, 32)
        with self.assertRaises(PeError):
            verify(bytes(b))

    def test_corrupt_section_fails(self):
        b=bytearray(fixture())
        struct.pack_into('<I', b, 0x98+224+20, 0x99999999)
        with self.assertRaises(PeError):
            verify(bytes(b))


if __name__ == '__main__':
    unittest.main()
