#!/usr/bin/env python3
"""Synthetic, copyright-free offline checks for the SafeDisc Wine relay parser."""
import importlib.util
from pathlib import Path
import unittest

parser_path = Path(__file__).with_name("analyze-safedisc-relay.py")
spec = importlib.util.spec_from_file_location("relay_parser", parser_path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class RelayTests(unittest.TestCase):
    def test_one_request_two_native_return_layers(self):
        log = [
            '0154:Call PE DLL (proc=00A4FD20,module=00A40000 L"dplayerx.dll",reason=PROCESS_ATTACH,res=00000000)',
            '0154:Call KERNEL32.DeviceIoControl(0000006c,ef002407,00d825f0,00000514,00d82b04,00000610,0022d6d4,00000000) ret=00a4d4dd',
            '0154:Ret  ntdll.NtDeviceIoControlFile() retval=c0000123 ret=6ffffc3ace39',
            '0154:Ret  ntdll.NtDeviceIoControlFile() retval=c0000123 ret=7b517379',
            '0154:Ret  KERNEL32.DeviceIoControl() retval=00000000 ret=00a4d4dd',
            '0154:Ret  KERNEL32.GetLastError() retval=000003e6 ret=00406811',
            'AV #09: EIP=0x00A47221, memory operand=0xFFFFFFFF, live 8 bytes=A6002AC40E193CEC',
        ]
        result = module.summarize(log)
        self.assertEqual(result["process"], "0154")
        self.assertEqual(len(result["calls"]), 1)
        self.assertEqual(result["calls"][0], (0x6c,0xd825f0,0x514,0xd82b04,0x610))
        self.assertEqual(result["returns"][0], 1)
        self.assertEqual(result["statuses"][0xC0000123], 2)
        self.assertEqual(result["base"], 0xA40000)
        self.assertEqual(result["last_errors"], [998])
        self.assertEqual(result["faults"], [(9,0xA47221,0xFFFFFFFF)])

    def test_unrelated_console_ioctl_is_not_counted(self):
        log = [
            '0154:Call ntdll.NtDeviceIoControlFile(0000000c,00000000,00000000,00000000,0022f970,00504000,00000000,00000000,0022f9b4,00000004)',
            '0154:Ret  ntdll.NtDeviceIoControlFile() retval=c0000022 ret=7b577cb7',
        ]
        result = module.summarize(log)
        self.assertEqual(result["calls"], [])
        self.assertEqual(result["returns"], {})
        self.assertEqual(result["statuses"], {})

if __name__ == "__main__":
    unittest.main()
