#!/usr/bin/env python3
"""Safely inspect Crimson Skies' GameOS UI resource archive.

Independently implemented from publicly documented ROF format:
https://github.com/Laeresh/CSVM/blob/main/docs/formats/rof.md

Reads GOSDATA/ASSETS/crimson.rof in place and optionally produces one
new zip containing ONLY original Preferences/MainMenu UI scripts, the
original layout CSV and a small JSON inventory. Does not modify any
installed game resources, original EXE, archive or DLL.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import tempfile
import zipfile
import zlib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

MAX_ARCHIVE = 256 * 1024 * 1024
MAX_FILES = 20000
MAX_ENTRIES_PER_NODE = 10000
MAX_POOL = 2 * 1024 * 1024
MAX_DEPTH = 32
MAX_EXTRACT = 4 * 1024 * 1024
MAX_REPORT_PATHS = 32
TARGETS = {
    "ASSETS/SCRIPTS/PREFERENCES.SCRIPT": "Preferences.script",
    "ASSETS/SCRIPTS/VIDEO.SCRIPT": "Video.script",
    "ASSETS/LAYOUT.CSV": "LAYOUT.CSV",
    "ASSETS/SCRIPTS/MAINMENU.SCRIPT": "MainMenu.script",
    "ASSETS/SCRIPTS/SCRIPTLOADER.SCRIPT": "ScriptLoader.script",
}


@dataclass(frozen=True)
class Member:
    path: str
    offset: int
    size: int
    stored_size: int
    kind: int


class RofError(ValueError):
    pass


class RofArchive:
    def __init__(self, data: bytes):
        if len(data) < 8 or len(data) > MAX_ARCHIVE:
            raise RofError("unexpected ROF size")
        self.data = data
        self.members: dict[str, Member] = {}
        self.directories: list[str] = []
        self.walk(0, "", frozenset(), 0)

    def u32(self, off: int) -> int:
        if off < 0 or off + 4 > len(self.data):
            raise RofError(f"field outside archive at {off:#x}")
        return struct.unpack_from("<I", self.data, off)[0]

    def walk(self, node: int, prefix: str, ancestors: frozenset[int], depth: int):
        if depth > MAX_DEPTH or node in ancestors:
            raise RofError("archive directory cycle or nesting exceeds limit")
        ancestors = ancestors | {node}
        count, pool_len = self.u32(node), self.u32(node + 4)
        if count > MAX_ENTRIES_PER_NODE or pool_len > MAX_POOL:
            raise RofError("implausible ROF node counts")
        entry_start = node + 8
        pool_start = entry_start + count * 24
        pool_end = pool_start + pool_len
        if pool_end > len(self.data):
            raise RofError("truncated ROF directory")
        for i in range(count):
            e = entry_start + i * 24
            offset, expanded, stored, kind, namelen, nameoff = struct.unpack_from(
                "<IIIIII", self.data, e
            )
            if not 2 <= namelen <= 256 or nameoff + namelen > pool_len:
                raise RofError("invalid ROF name range")
            raw = self.data[pool_start + nameoff:pool_start + nameoff + namelen]
            if raw[-1] != 0:
                raise RofError("nonterminated ROF name")
            try:
                name = raw[:-1].decode("ascii")
            except UnicodeDecodeError as exc:
                raise RofError("non-ASCII ROF member name") from exc
            if name in (".", "..") or any(c in name for c in "/\\:") or not name:
                raise RofError("unsafe ROF member name")
            path = prefix + name
            if len(self.members) + len(self.directories) >= MAX_FILES:
                raise RofError("too many ROF entries")
            if kind == 1:
                if expanded or stored:
                    raise RofError("directory has payload sizes")
                self.directories.append(path)
                self.walk(offset, path + "/", ancestors, depth + 1)
            elif kind in (0, 2):
                if offset > len(self.data) or stored > len(self.data) - offset:
                    raise RofError("payload outside ROF archive")
                if kind == 0 and stored != expanded:
                    raise RofError("uncompressed size mismatch")
                key = path.upper()
                if key in self.members:
                    raise RofError("duplicate ROF member")
                self.members[key] = Member(path, offset, expanded, stored, kind)
            else:
                raise RofError("unknown ROF entry kind")

    def read(self, path: str) -> bytes | None:
        m = self.members.get(path.upper())
        if m is None:
            return None
        if m.size > MAX_EXTRACT or m.stored_size > MAX_EXTRACT:
            raise RofError("requested member exceeds safe extraction limit")
        payload = self.data[m.offset:m.offset + m.stored_size]
        if m.kind == 0:
            return payload
        try:
            decompressor = zlib.decompressobj()
            result = decompressor.decompress(payload, m.size + 1)
            if len(result) != m.size or not decompressor.eof or decompressor.unconsumed_tail:
                raise RofError("ROF member failed bounded size or zlib EOF verification")
            if decompressor.unused_data:
                raise RofError("ROF member has trailing compressed junk")
        except zlib.error as exc:
            raise RofError("cannot decompress selected ROF member") from exc
        return result


def find_archive(root: Path) -> Path:
    if root.is_file():
        if root.name.lower() != "crimson.rof":
            raise RofError("provide game root or crimson.rof itself")
        return root
    current = root
    for expected in ("GOSDATA", "ASSETS", "crimson.rof"):
        if not current.is_dir():
            raise RofError("game root has no GOSDATA/ASSETS/crimson.rof")
        matches = [
            p for p in current.iterdir()
            if p.name.casefold() == expected.casefold() and not p.is_symlink()
        ]
        if len(matches) != 1:
            raise RofError("cannot uniquely locate GOSDATA/ASSETS/crimson.rof")
        current = matches[0]
    if not current.is_file():
        raise RofError("crimson.rof is not a file")
    return current


def report(archive: RofArchive, file: Path, selected: dict[str, bytes]) -> dict:
    extcounts = Counter(Path(m.path).suffix.upper() for m in archive.members.values())
    return {
        "format": "CrimsonSkiesFix-GOS-ROF-inspection-v1",
        "mode": "read-only source; export only explicitly selected UI files",
        "archive_name": file.name,
        "archive_bytes": len(archive.data),
        "archive_sha256": hashlib.sha256(archive.data).hexdigest(),
        "member_count": len(archive.members),
        "directory_count": len(archive.directories),
        "extensions": dict(sorted(extcounts.items())),
        "script_names_sample": sorted(
            (m.path for m in archive.members.values() if m.path.upper().endswith(".SCRIPT"))
        )[:MAX_REPORT_PATHS],
        "ui_members": [
            {
                "archive_path": key,
                "name": TARGETS[key],
                "present": key in archive.members,
                "bytes": len(selected[key]) if key in selected else None,
                "sha256": hashlib.sha256(selected[key]).hexdigest() if key in selected else None,
            }
            for key in TARGETS
        ],
        "note": ("Original UI source files only; no game executables, "
                 "textures or archive modifications"),
    }


def bundle(source: Path, output: Path) -> dict:
    file = find_archive(source)
    size = file.stat().st_size
    if not 8 <= size <= MAX_ARCHIVE:
        raise RofError("unexpected ROF archive size")
    archive = RofArchive(file.read_bytes())
    targets = {name: archive.read(name) for name in TARGETS}
    selected = {name: value for name, value in targets.items() if value is not None}
    if "ASSETS/SCRIPTS/PREFERENCES.SCRIPT" not in selected or "ASSETS/LAYOUT.CSV" not in selected:
        raise RofError("original archive lacks Preferences.script or LAYOUT.CSV")
    result = report(archive, file, selected)
    if output.exists():
        raise FileExistsError(f"refusing to overwrite {output}")
    with output.open("xb") as dst:
        with zipfile.ZipFile(dst, "w", compression=zipfile.ZIP_DEFLATED) as z:
            z.writestr("gos-rof-report.json", json.dumps(result, indent=2, ensure_ascii=False) + "\n")
            for name in TARGETS:
                if name in selected:
                    z.writestr(TARGETS[name], selected[name])
    return result


def make_fixture() -> bytes:
    """Exercise original ROF node layout with fully synthetic test data."""
    out = bytearray()
    def node(mapping):
        pos = len(out)
        names = b"".join(name.encode("ascii") + b"\0" for name in mapping)
        out.extend(struct.pack("<II", len(mapping), len(names)))
        out.extend(b"\x00" * (24 * len(mapping)))
        out.extend(names)
        nameoff = 0
        for i, (name, value) in enumerate(mapping.items()):
            if isinstance(value, dict):
                off = node(value)
                expanded = stored = 0
                kind = 1
            else:
                expanded = len(value)
                compressed = zlib.compress(value)
                off = len(out)
                stored = len(compressed)
                kind = 2
                out.extend(compressed)
            struct.pack_into(
                "<IIIIII", out, pos + 8 + i * 24,
                off, expanded, stored, kind, len(name) + 1, nameoff
            )
            nameoff += len(name) + 1
        return pos
    assert node({
        "ASSETS": {
            "SCRIPTS": {
                "PREFERENCES.SCRIPT": b"gui_create prefs;",
                "VIDEO.SCRIPT": b"gui_create video;",
                "MAINMENU.SCRIPT": b"main",
            },
            "LAYOUT.CSV": b"[@Preferences@]\r\nPF_D_GRAPHICS=D,...\r\n",
        }
    }) == 0
    return bytes(out)


def self_test() -> None:
    binary = make_fixture()
    arc = RofArchive(binary)
    assert len(arc.members) == 4
    assert arc.read("ASSETS/SCRIPTS/PREFERENCES.SCRIPT") == b"gui_create prefs;"
    assert arc.read("assets/layout.csv").startswith(b"[@Preferences@]")
    assert arc.read("missing") is None
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        (root / "GOSDATA" / "ASSETS").mkdir(parents=True)
        (root / "GOSDATA" / "ASSETS" / "CRIMSON.ROF").write_bytes(binary)
        out = root / "review.zip"
        result = bundle(root, out)
        assert result["member_count"] == 4
        with zipfile.ZipFile(out) as z:
            assert z.read("Preferences.script") == b"gui_create prefs;"
            assert z.read("Video.script") == b"gui_create video;"
            assert z.read("LAYOUT.CSV").startswith(b"[@Preferences@]")
            assert "gos-rof-report.json" in z.namelist()
        try:
            bundle(root, out)
            raise AssertionError("must not overwrite")
        except FileExistsError:
            pass
        bad = bytearray(binary)
        struct.pack_into("<I", bad, 8, 0)
        try:
            RofArchive(bytes(bad))
            raise AssertionError("cycle must not parse")
        except RofError:
            pass
    print("inspect_gos_rof self-test passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path, nargs="?",
                        help="Crimson Skies game root or path to crimson.rof")
    parser.add_argument("--bundle", type=Path,
                        help="Create a NEW zip containing only selected UI assets")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.path is None or args.bundle is None:
        parser.error("provide game root and --bundle NEW_FILE.zip")
    if args.bundle.suffix.lower() != ".zip":
        parser.error("--bundle must name a ZIP file")
    try:
        result = bundle(args.path, args.bundle)
    except (OSError, ValueError, zipfile.BadZipFile) as exc:
        parser.exit(2, f"ROF inspection error: {exc}\n")
    print(f"UI bundle created: {args.bundle}")
    print(f"Archive: {result['archive_name']} ({result['archive_bytes']} bytes)")
    print("Extracted UI files: " + ", ".join(
        x["name"] for x in result["ui_members"] if x["present"]
    ))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
