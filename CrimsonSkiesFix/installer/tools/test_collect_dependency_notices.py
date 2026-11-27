import json
import tempfile
import unittest
from pathlib import Path
from collect_dependency_notices import collect


class NoticesTest(unittest.TestCase):
    def test_exact_package_dedup_and_missing_license(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkg = root / "pkg"
            pkg.mkdir()
            (pkg / "Cargo.toml").write_text("", encoding="utf-8")
            (pkg / "LICENSE-MIT").write_text("Copyright Example", encoding="utf-8")
            (pkg / "NOTICE").write_text("Notice Example", encoding="utf-8")
            (pkg / "game.bin").write_bytes(b"not a notice")
            entry = {"name": "example", "version": "1.0", "source": "registry+example",
                     "manifest_path": str(pkg / "Cargo.toml"), "license": "MIT"}
            a = root / "a.json"
            a.write_text(json.dumps({"packages": [entry, entry]}), encoding="utf-8")
            result = collect([a, a])
            self.assertEqual(result.count("example 1.0"), 1)
            self.assertIn("Copyright Example", result)
            self.assertIn("Notice Example", result)
            self.assertNotIn("game.bin", result)
            (pkg / "LICENSE-MIT").unlink()
            with self.assertRaises(ValueError):
                collect([a])

    def test_outside_declared_license_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pkg = root / "pkg"
            pkg.mkdir()
            (pkg / "Cargo.toml").write_text("", encoding="utf-8")
            (root / "LICENSE").write_text("outside", encoding="utf-8")
            entry = {"name": "example", "version": "1.0", "source": "registry+example",
                     "manifest_path": str(pkg / "Cargo.toml"), "license_file": "../LICENSE"}
            a = root / "a.json"
            a.write_text(json.dumps({"packages": [entry]}), encoding="utf-8")
            with self.assertRaises(ValueError):
                collect([a])


if __name__ == "__main__":
    unittest.main()
