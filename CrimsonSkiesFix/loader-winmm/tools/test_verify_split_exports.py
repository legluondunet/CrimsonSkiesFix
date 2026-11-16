#!/usr/bin/env python3
"""Synthetic, dependency-free tests of split-only WinMM ABI safety gate."""
from __future__ import annotations
import importlib.util
import unittest
from pathlib import Path

CHECK=Path(__file__).with_name("verify_split_exports.py")
spec=importlib.util.spec_from_file_location("verify_split_exports",CHECK)
assert spec and spec.loader
gate=importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)

class SplitWinmmExportsTest(unittest.TestCase):
    def setUp(self):
        self.expected=gate.expected()
        self.assertEqual(len(self.expected),185)
        self.candidate={
            "named":{**self.expected,"CrimsonSkiesFixBootstrapStatus":2233},
            "occupied":[*range(2048,2234)],
        }
    def test_valid_high_aliases_pass(self):
        gate.check(self.candidate,self.expected)
    def test_legacy_numeric_slot_is_rejected(self):
        for n in (1,2,11,14,185,1023):
            with self.subTest(n=n):
                candidate={**self.candidate,"occupied":[n,*self.candidate["occupied"]]}
                with self.assertRaisesRegex(ValueError,"unsafe_low_ordinals"):
                    gate.check(candidate,self.expected)
    def test_missing_name_and_wrong_number_fail_closed(self):
        missing={**self.candidate,"named":dict(self.candidate["named"])}
        missing["named"].pop("timeGetTime")
        with self.assertRaisesRegex(ValueError,"missing"):
            gate.check(missing,self.expected)
        wrong={**self.candidate,"named":dict(self.candidate["named"])}
        wrong["named"]["timeGetTime"]=12
        with self.assertRaisesRegex(ValueError,"wrong"):
            gate.check(wrong,self.expected)
    def test_unknown_export_and_missing_diagnostic_rejected(self):
        extra={**self.candidate,"named":{**self.candidate["named"],"Unexpected":2340}}
        with self.assertRaisesRegex(ValueError,"extra"):
            gate.check(extra,self.expected)
        no_diag={**self.candidate,"named":dict(self.expected)}
        with self.assertRaisesRegex(ValueError,"missing split runtime diagnostic"):
            gate.check(no_diag,self.expected)

if __name__=="__main__":
    unittest.main()
