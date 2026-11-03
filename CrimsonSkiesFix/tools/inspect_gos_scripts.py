#!/usr/bin/env python3
r"""Read-only inventory of Crimson Skies GOS scripts and installation layout.

The exact v1.02 executable constructs assets\scripts\<name>.script,
including Preferences and MainMenu. These bytecode assets are NOT
embedded in the executable. The requested assets may also be held in
packed game data: a file path in the EXE does NOT prove a loose .script
file exists. This tool inventories .script files and potential archive
containers by filename/size only; it never modifies or extracts game
assets, nor decodes unknown script or archive formats.
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
CONTAINER_EXTENSIONS = frozenset({
    ".zbd", ".pak", ".cab", ".dat", ".arc", ".zip", ".7z", ".rar",
    ".wad", ".res", ".bin", ".idx", ".iso", ".cpk",
})
MAX_ARCHIVE_EXAMPLES = 80
MAX_TOP_LEVEL = 60
MAX_EXTENSION_KINDS = 24


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


def scan_game_layout(root: Path) -> dict[str, object]:
    """Inspect names, extensions and sizes only. No archive content is opened."""
    import os
    from collections import Counter

    if root.is_file():
        return {
            "root_kind": "file",
            "root_name": root.name,
            "top_level_entries": [],
            "examined_file_count": 1,
            "examined_directory_count": 0,
            "extension_histogram": [{"extension": root.suffix.lower() or "(none)", "count": 1}],
            "container_candidate_count": 0,
            "container_candidates": [],
            "container_candidates_truncated": False,
            "game_root_executable_detected": False,
        }

    top_level = []
    for child in root.iterdir():
        if child.is_symlink():
            continue
        if child.is_file() or child.is_dir():
            top_level.append({
                "name": child.name,
                "kind": "directory" if child.is_dir() else "file",
            })
    top_level.sort(key=lambda item: item["name"].casefold())
    direct_exe = any(
        entry["kind"] == "file" and
        entry["name"].lower() in ("crimson.exe", "crimson-unwrapped.exe", "testme.exe")
        for entry in top_level
    )

    extensions: Counter[str] = Counter()
    archives: list[tuple[int, str, int]] = []
    total_files = total_directories = 0
    start_depth = len(root.parts)
    for directory, dirs, filenames in os.walk(root, followlinks=False):
        base = Path(directory)
        depth = len(base.parts) - start_depth
        dirs[:] = sorted(d for d in dirs if not (base / d).is_symlink()) if depth < 8 else []
        total_directories += 1
        for filename in filenames:
            file = base / filename
            if file.is_symlink():
                continue
            try:
                size = file.stat().st_size
            except OSError:
                continue
            total_files += 1
            ext = file.suffix.lower() or "(none)"
            extensions[ext] += 1
            if ext in CONTAINER_EXTENSIONS:
                rel = file.relative_to(root).as_posix()
                priority = 0 if ext == ".zbd" else 1
                archives.append((priority, rel, size))
    archives.sort(key=lambda item: (item[0], item[1].casefold()))
    return {
        "root_kind": "directory",
        "root_name": root.name,
        "top_level_entries": top_level[:MAX_TOP_LEVEL],
        "top_level_entries_truncated": len(top_level) > MAX_TOP_LEVEL,
        "examined_file_count": total_files,
        "examined_directory_count": total_directories,
        "scan_depth_limit": 8,
        "extension_histogram": [
            {"extension": ext, "count": count}
            for ext, count in extensions.most_common(MAX_EXTENSION_KINDS)
        ],
        "container_candidate_count": len(archives),
        "container_candidates": [
            {"file": name, "bytes": size}
            for _, name, size in archives[:MAX_ARCHIVE_EXAMPLES]
        ],
        "container_candidates_truncated": len(archives) > MAX_ARCHIVE_EXAMPLES,
        "game_root_executable_detected": direct_exe,
        "warning": ("Archive names identify potential containers only. Neither the "
                    "presence of .zbd nor any filename proves the GOS Preferences "
                    "script is present or extractable."),
    }


def inventory(root: Path, show_all: bool = False) -> dict[str, object]:
    paths = sorted(locate_scripts(root), key=rank)
    interesting = [p for p in paths if show_all or rank(p)[0] < 3]
    return {
        "format": "CrimsonSkiesFix-GOS-script-inspection-v2",
        "inspection_mode": ("read-only; no .script bytes modified; "
                            "potential archives NOT opened or extracted"),
        "layout": scan_game_layout(root),
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
        (root / "interp.zbd").write_bytes(b"TEST-SYNTHETIC-DATA")
        (root / "CRIMSON-UNWRAPPED.EXE").write_bytes(b"MZ-not-a-real-exe")
        report = inventory(root)
        assert report["script_count"] == 2
        assert report["format"].endswith("-v2")
        assert report["layout"]["container_candidate_count"] == 1
        assert report["layout"]["container_candidates"][0]["file"] == "interp.zbd"
        assert report["layout"]["game_root_executable_detected"]
        empty = root / "EmptyGame"
        empty.mkdir()
        blank = inventory(empty)
        assert blank["script_count"] == 0
        assert blank["layout"]["examined_directory_count"] == 1
        assert blank["layout"]["container_candidate_count"] == 0
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
