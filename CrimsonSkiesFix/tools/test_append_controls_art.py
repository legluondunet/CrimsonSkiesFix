#!/usr/bin/env python3
import struct, unittest, zlib
from unittest.mock import patch
import append_controls_art as art

class ControlsArtTests(unittest.TestCase):
    def pack(self):
        raw = bytes(art.RAW)
        stream = zlib.compress(raw)
        return art.MAGIC + struct.pack("<III",art.WIDTH,art.HEIGHT,art.COUNT) + (struct.pack("<I",len(stream))+stream)*3
    def exe(self):
        b=bytearray(128);b[:2]=b"MZ"
        struct.pack_into("<I",b,60,64)
        b[64:70]=b"PE\x00\x00\x4c\x01"
        struct.pack_into("<H",b,88,0x10b)
        return bytes(b)
    def test_decodes_all_images(self):
        self.assertEqual([len(x) for x in art.decode_pack(self.pack())],[art.RAW]*3)
    def test_invalid_container(self):
        data=self.pack()
        for bad in [data[:-1],data+b"x",b"X"+data[1:],data[:8]+bytes(4)+data[12:]]:
            with self.assertRaises(ValueError):art.decode_pack(bad)
    def test_wrong_digest(self):
        with self.assertRaises(ValueError):art.verify_pack(self.pack())
    def test_pe_validation(self):
        art.validate_pe(self.exe())
        for data in [b"MZ",self.exe()[:89],self.exe()[:88]+b"\0\0"+self.exe()[90:]]:
            with self.assertRaises(ValueError):art.validate_pe(data)
    def test_overlay_embedded_and_duplicate(self):
        p=self.pack();exe=self.exe()
        with patch.object(art,"verify_pack",side_effect=lambda b:art.decode_pack(b)):
            out=art.assemble(exe,p)
            self.assertTrue(out.startswith(exe))
            art.verify_launcher(out,p,False)
            art.verify_launcher(exe+p,p,True)
            for original in [out,exe+p]:
                with self.assertRaises(ValueError):art.assemble(original,p)
            with self.assertRaises(ValueError):art.verify_launcher(out,p,True)
            with self.assertRaises(ValueError):art.verify_launcher(exe+p,p,False)
if __name__=="__main__":
    unittest.main()
