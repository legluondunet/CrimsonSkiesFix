"""Tests that do not distribute or require any copyrighted game files."""

import hashlib
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

# Running `python -m unittest discover -s tests` from the project root.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import crimson_patch as patch


class FovTests(unittest.TestCase):
    def test_reference_4_by_3(self):
        self.assertAlmostEqual(patch.horizontal_fov(80, 1024, 768), 80)
        self.assertAlmostEqual(patch.horizontal_fov(60, 1024, 768), 60)

    def test_16_by_9(self):
        self.assertAlmostEqual(patch.horizontal_fov(80, 1920, 1080), 96.418, places=2)
        self.assertAlmostEqual(patch.horizontal_fov(80, 2560, 1440), patch.horizontal_fov(80, 1920, 1080))

    def test_wider_ratio_increases_fov(self):
        self.assertLess(patch.horizontal_fov(80, 1024, 768), patch.horizontal_fov(80, 1920, 1200))
        self.assertLess(patch.horizontal_fov(80, 1920, 1200), patch.horizontal_fov(80, 1920, 1080))

    def test_reference_constant_bytes(self):
        self.assertAlmostEqual(math.degrees(struct.unpack('<f', patch.BASE80_BYTES)[0]), 80, places=4)
        self.assertAlmostEqual(math.degrees(struct.unpack('<f', patch.BASE60_BYTES)[0]), 60, places=4)


class ValidationTests(unittest.TestCase):
    def test_too_short(self):
        with self.assertRaisesRegex(ValueError, 'tronqu'):
            patch.check_binary(b'MZ')

    def test_unrelated_executable(self):
        sample = bytearray(patch.FOV80_OFF + 8)
        sample[:2] = b'MZ'
        with self.assertRaisesRegex(ValueError, 'non reconnue'):
            patch.check_binary(sample)


class OptionalGameFileIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        value = os.environ.get('CRIMSON_TEST_EXE')
        if not value:
            raise unittest.SkipTest('Set CRIMSON_TEST_EXE for integration tests with your own compatible EXE')
        cls.source = Path(value)
        cls.raw = cls.source.read_bytes()
        patch.check_binary(cls.raw)

    def test_patch_resolution_and_preserve_other_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            dest = Path(directory) / 'test.exe'
            command = [sys.executable, str(Path(patch.__file__).resolve()), str(self.source), '--width', '2560', '--height', '1440', '-o', str(dest)]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            new = dest.read_bytes()
            self.assertEqual(len(new), len(self.raw))
            self.assertEqual(struct.unpack_from('<II', new, patch.WIDTH_OFF), (2560, 1440))
            self.assertEqual(new[patch.FOV60_OFF:patch.FOV80_OFF + 4], self.raw[patch.FOV60_OFF:patch.FOV80_OFF + 4])
            patch.check_binary(new)
            expected = bytearray(self.raw)
            struct.pack_into('<II', expected, patch.WIDTH_OFF, 2560, 1440)
            self.assertEqual(new, bytes(expected))
            self.assertFalse(dest.with_suffix('.exe.bak').exists())

    def test_custom_fov_and_reset(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'fov.exe'
            script = str(Path(patch.__file__).resolve())
            run = subprocess.run([sys.executable, script, str(self.source), '--fov', '105', '-o', str(target)], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            data = target.read_bytes()
            width, height = struct.unpack_from('<II', data, patch.WIDTH_OFF)
            base80 = math.degrees(struct.unpack_from('<f', data, patch.FOV80_OFF)[0])
            self.assertAlmostEqual(patch.horizontal_fov(base80, width, height), 105, places=4)
            output = Path(directory) / 'reset.exe'
            run = subprocess.run([sys.executable, script, str(target), '--reset-fov', '-o', str(output)], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            self.assertEqual(output.read_bytes(), self.raw)

    def test_refuses_to_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'existing.exe'
            target.write_bytes(b'keep me')
            result = subprocess.run([sys.executable, str(Path(patch.__file__).resolve()), str(self.source), '--width', '1280', '--height', '720', '-o', str(target)], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(target.read_bytes(), b'keep me')


if __name__ == '__main__':
    unittest.main()
