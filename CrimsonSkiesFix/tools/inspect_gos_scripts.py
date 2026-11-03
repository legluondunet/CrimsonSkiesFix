#!/usr/bin/env python3
"""Read-only inventory of Crimson Skies external GameOS .script files.

The exact v1.02 executable constructs assets\scripts\<name>.script,
including Preferences and MainMenu. These bytecode assets are NOT
embedded in the executable. This tool never modifies game files and
never attempts to decode unverified script opcodes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

KEYWORDS = (
    "preference", "graphic", "resolution", "video", "gosscript", "gosbutton",
    "goscolor", "disabled", "enable", "control", "uidata", "goscallback",
    "gosresid", "button", "device", "options", "dropdown", "select",
)
ASCII = re.compile(rb"[\x20-\x7e]{5,}")
UTF16LE = re.compile(rb"(?:[\x20-\x7e]\x00){5,}")
MAX_READ_BYTES = 32 * 1024 * 1024
MAX_STRINGS = 16


def locate_scripts(root: Path) -> list[Path]:
    """Find .script files case-insensitively, without following symlinks."""
    if not root.exists():
        raise FileNotFoundError(f"Path does not exist: {root}")
    if root.is_file():
        return [root] if root.suffix.lower() == ".script" else []
    result: list[Path] = []
    # Keep a depth cap and don't resolve external symlinks.
    import os
    start_depth = len(root.parts)
    for directory, dirs, files in os.walk(root, followlinks=False):
        base = Path(directory)
        depth = len(base.parts) - start_depth
        dirs[:] = sorted(d for d in dirs if not (base / d).is_symlink()) if depth < 8 else []
        for name in files:
            file = base / name
            if file.suffix.lower() == ".script" and not file.is_symlink():
                result.append(file)
    return sorted(result, key=lambda p: str(p).casefold())


def sampled_strings(data: bytes) -> tuple[int, list[dict[str, object]]]:
    """Only report bounded keyword matches; do not dump script assets."""
    matches: list[tuple[int, str, str]] = []
    for m in ASCII.finditer(data):
        s = m.group().decode("ascii")
        if any(k in s.lower() for k in KEYWORDS):
            matches.append((m.start(), "ascii", s[:100]))
    for m in UTF16LE.finditer(data):
        s = m.group().decode("utf-16le")
        if any(k in s.lower() for k in KEYWORDS):
            matches.append((m.start(), "utf16le", s[:100]))
    matches.sort(key=lambda t: (t[0], t[1], t[2]))
    return len(matches), [
        {"offset": f"0x{offset:x}", "encoding": kind, "text": value}
        for offset, kind, value in matches[:MAX_STRINGS]
    ]


def describe(path: Path, root: Path) -> dict[str, object]:
    result: dict[str, object] = {"file": str(path.relative_to(root)) if path != root else path.name}
    size = path.stat().st_size
    result["bytes"] = size
    if size > MAX_READ_BYTES:
        result["status"] = "too-large-to-inspect"
        return result
    data = path.read_bytes()
    result["sha256"] = hashlib.sha256(data).hexdigest()
    result["first_16_bytes_hex"] = data[:16].hex()
    n, hits = sampled_strings(data)
    result["keyword_match_count"] = n
    result["keyword_examples"] = hits
    result["status"] = "ok"
    return result


def rank(path: Path) -> tuple[int, str]:
    filename = path.name.casefold()
    if filename == "preferences.script":
        return 0, str(path).casefold()
    if filename in {"mainmenu.script", "scriptloader.script"}:
        return 1, str(path).casefold()
    if any(token in filename for token in ("option", "video", "menu", "ui")):
        return 2, str(path).casefold()
    return 3, str(path).casefold()


def inventory(root: Path, show_all: bool = False) -> dict[str, object]:
    paths = sorted(locate_scripts(root), key=rank)
    interesting = [p for p in paths if show_all or rank(p)[0] < 3]
    return {
        "format": "CrimsonSkiesFix-GOS-script-inspection-v1",
        "inspection_mode": "read-only; format not assumed; no .script bytes modified",
        "script_count": len(paths),
        "candidate_count": len(interesting),
        "candidates": [describe(p, root) for p in interesting[:80]],
        "truncated": len(interesting) > 80,
        "note": ("No readable keyword matches does not mean the GOS bytecode lacks a "
                 "control: compressed or tokenized content may contain no plain strings."),
    }


def self_test() -> None:
    import tempfile
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        folder = root / "ASSETS" / "SCRIPTS"
        folder.mkdir(parents=True)
        prefs = folder / "Preferences.SCRIPT"
        prefs.write_bytes(b"\x00\x01Graphics\x00Resolution\x00\x00" + "Disabled".encode("utf-16le"))
        (folder / "MainMenu.script").write_bytes(b"MAINMENU")
        (folder / "Other.txt").write_bytes(b"ignored")
        report = inventory(root)
        assert report["script_count"] == 2
        assert report["candidates"][0]["file"].endswith("Preferences.SCRIPT")
        assert report["candidates"][0]["keyword_match_count"] == 3
        assert len(locate_scripts(prefs)) == 1
        assert len(locate_scripts(folder / "Other.txt")) == 0
        assert sampled_strings(b"\xff\xfe")[0] == 0
    print("inspect_gos_scripts self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", nargs="?", type=Path, help="Game root, Assets/Scripts folder, or one .script file")
    parser.add_argument("--all", action="store_true", help="Inspect all .script files, up to 80")
    parser.add_argument("--output", type=Path, help="Write JSON report to a new file")
    parser.add_argument("--self-test", action="store_true", help="Exercise offline test fixtures")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.path is None:
        parser.error("path required except with --self-test")
    try:
        report = inventory(args.path, show_all=args.all)
    except (OSError, ValueError) as exc:
        parser.exit(2, f"Error: {exc}\n")
    rendered = json.dumps(report, indent=2, ensure_ascii=False)
    if args.output:
        if args.output.suffix.lower() != ".json" or args.output.exists():
            parser.error("--output must be a NEW .json file; existing files are never overwritten")
        with args.output.open("x", encoding="utf-8") as output:
            output.write(rendered + "\n")
        print(f"Report written to {args.output} (read-only source inspection)")
    else:
        print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
