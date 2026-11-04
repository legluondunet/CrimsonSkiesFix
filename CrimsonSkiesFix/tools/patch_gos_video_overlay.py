#!/usr/bin/env python3
"""Optional and reversible experiment: use GameOS's own disabled dropdown state.

The owner-provided retail GOS scripts prove 10000 deactivates @ctl@PM's
hit region and greys its text, while 10018 enables it again.
This experiment creates a modified copy of the game's *patch overlay*
crimptch.rof. It never changes the large original crimson.rof, original
game executable, or the CrimsonSkiesFix DLL.

The tool uses the validated read-only inspect_gos_rof.py in the same folder.
Default --preview creates a separate ROF file without changing the game.
Only explicit --install changes the patch overlay, after an exclusive backup.
Use --restore to roll back the optional experiment.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import struct
import tempfile
import zlib
from pathlib import Path

try:
    from inspect_gos_rof import RofArchive, RofError, find_archive
except ModuleNotFoundError as missing:
    if missing.name != "inspect_gos_rof":
        raise
    # Earlier standalone owner download used the explicit _controls suffix.
    from inspect_gos_rof_controls import RofArchive, RofError, find_archive

BASE_ARCHIVE_SHA256 = (
    "acc9946874110e9741183384010bc02fd48923ae3d608fcf02a96498c3731174"
)
VIDEO_ORIGINAL_SHA256 = (
    "f0b0ae62dc582a55dfc6eaf4a8b2937ef2084a03596b8fa52542a5de4512afa9"
)
VIDEO_PATCHED_SHA256 = (
    "a5f716d2e1ad007ac217a172e9d17b459444f7ffb23855a5a10d9a1841b9b22d"
)
ALLOWED_EXE_SHA256 = (
    "43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75"
)
BASE_VIDEO = "ASSETS/SCRIPTS/VIDEO.SCRIPT"
PATCH_AIRFRAME = "ASSETS/SCRIPTS/AIRFRAME.SCRIPT"
BACKUP_SUFFIX = ".CrimsonSkiesFix.original.bak"
MAX_PATCH_BYTES = 4 * 1024 * 1024
MAX_MEMBER_BYTES = 2 * 1024 * 1024
EXPECTED_ORIGINAL_VIDEO_BYTES = 4337


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(file: Path) -> str:
    digest = hashlib.sha256()
    with file.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def case_child(directory: Path, name: str) -> Path:
    if not directory.is_dir() or directory.is_symlink():
        raise RofError(f"invalid directory: {directory}")
    found = [p for p in directory.iterdir()
             if p.name.casefold() == name.casefold() and not p.is_symlink()]
    if len(found) != 1:
        raise RofError(f"expected exactly one {name} in {directory}")
    return found[0]


def game_paths(root: Path) -> tuple[Path, Path]:
    if not root.is_dir() or root.is_symlink():
        raise RofError("provide the actual game installation root")
    exe = case_child(root, "CRIMSON-UNWRAPPED.EXE")
    if exe.stat().st_size != 2580480 or sha256_file(exe) != ALLOWED_EXE_SHA256:
        raise RofError("unapproved CRIMSON-UNWRAPPED.EXE SHA-256; aborting")
    archive_path = find_archive(root)
    if sha256_file(archive_path) != BASE_ARCHIVE_SHA256:
        raise RofError("crimson.rof does not match the owner-verified original; aborting")
    patch_path = case_child(case_child(root, "GOSDATA"), "ASSETS")
    patch_path = case_child(patch_path, "crimptch.rof")
    if not patch_path.is_file() or patch_path.stat().st_size > MAX_PATCH_BYTES:
        raise RofError("unexpected original overlay size")
    return archive_path, patch_path


def insert_disabled_messages(source: bytes) -> bytes:
    """Insert native GameOS messages AFTER each relevant widget re-init."""
    if len(source) != EXPECTED_ORIGINAL_VIDEO_BYTES or sha256(source) != VIDEO_ORIGINAL_SHA256:
        raise RofError("unexpected original VIDEO.SCRIPT hash/length; nothing changed")
    pairs = (
        (b"initialize(HVA)\r\n", b"initialize(HVA)\r\nmail (10000, HVA)\r\n", 1),
        (b"initialize(IVA)\r\n", b"initialize(IVA)\r\nmail (10000, IVA)\r\n", 3),
    )
    result = source
    for old, new, expected in pairs:
        if result.count(old) != expected:
            raise RofError("VIDEO.SCRIPT init sites differ; refusing to patch")
        result = result.replace(old, new)
    if result.count(b"mail (10000, HVA)") != 1 or result.count(b"mail (10000, IVA)") != 3:
        raise RofError("post-edit GOS message count mismatch")
    if sha256(result) != VIDEO_PATCHED_SHA256:
        raise RofError("post-edit VIDEO.SCRIPT hash mismatch")
    return result


def write_rof(members: dict[str, bytes]) -> bytes:
    """Rebuild only the tiny patch overlay, from validated original members."""
    if not members or len(members) > 4:
        raise RofError("unexpected overlay member count")
    # The file format is documented at Laeresh/CSVM docs/formats/rof.md.
    tree: dict[str, object] = {}
    for path, data in members.items():
        if len(data) > MAX_MEMBER_BYTES:
            raise RofError("overlay member exceeds bound")
        names = path.upper().split("/")
        if len(names) != 3 or names[:2] != ["ASSETS", "SCRIPTS"]:
            raise RofError("unexpected overlay member path")
        if not all(name and name.isascii() and name.replace("_", "").replace(".", "").isalnum()
                   for name in names):
            raise RofError("unsafe overlay member path")
        top = tree.setdefault("ASSETS", {})
        scripts = top.setdefault("SCRIPTS", {})
        if names[2] in scripts:
            raise RofError("duplicate overlay member")
        scripts[names[2]] = data

    output = bytearray()
    def directory(items: dict[str, object]) -> int:
        location = len(output)
        names = b"".join(name.encode("ascii") + b"\0" for name in items)
        output.extend(struct.pack("<II", len(items), len(names)))
        output.extend(bytes(len(items) * 24))
        output.extend(names)
        offset_in_pool = 0
        for idx, (name, value) in enumerate(items.items()):
            if isinstance(value, dict):
                offset = directory(value)
                decoded = stored = 0
                kind = 1
            else:
                offset = len(output)
                # Original ROF payloads typically use zlib's fast 0x78 0x01 stream.
                encoded = zlib.compress(value, level=1)
                decoded, stored = len(value), len(encoded)
                kind = 2
                output.extend(encoded)
            struct.pack_into("<IIIIII", output, location + 8 + idx * 24,
                             offset, decoded, stored, kind,
                             len(name) + 1, offset_in_pool)
            offset_in_pool += len(name) + 1
        return location

    if directory(tree) != 0:
        raise RofError("unexpected root position")
    if len(output) > MAX_PATCH_BYTES:
        raise RofError("rebuilt overlay exceeds bound")
    validated = RofArchive(bytes(output))
    if set(validated.members) != set(members) or any(
        validated.read(path) != data for path, data in members.items()
    ):
        raise RofError("rebuilt overlay did not round-trip exact member contents")
    return bytes(output)


def prepare_patch(base_path: Path, patch_path: Path) -> tuple[bytes, bytes]:
    base = RofArchive(base_path.read_bytes())
    video = base.read(BASE_VIDEO)
    if video is None:
        raise RofError("original Video.script is missing")
    changed = insert_disabled_messages(video)
    existing = RofArchive(patch_path.read_bytes())
    if set(existing.members) != {PATCH_AIRFRAME}:
        raise RofError("overlay differs from expected single AIRFRAME.SCRIPT; aborting")
    airframe = existing.read(PATCH_AIRFRAME)
    if not airframe:
        raise RofError("AIRFRAME overlay missing or empty")
    new_overlay = write_rof({
        PATCH_AIRFRAME: airframe,
        BASE_VIDEO: changed,
    })
    return new_overlay, changed


def atomic_write_new(path: Path, data: bytes):
    """Exclusive output, never overwrite an existing file."""
    with path.open("xb") as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())


def install(original: Path, replacement: bytes):
    backup = original.with_name(original.name + BACKUP_SUFFIX)
    if backup.exists():
        raise RofError("backup exists: restore or inspect previous installation first")
    with original.open("rb") as read:
        source = read.read()
    # Record an exact user-owned backup before the first possible edit.
    atomic_write_new(backup, source)
    tempname = None
    try:
        with tempfile.NamedTemporaryFile(
            prefix="._csfix_rof_", suffix=".tmp", dir=original.parent, delete=False
        ) as tmp:
            tempname = Path(tmp.name)
            tmp.write(replacement)
            tmp.flush()
            os.fsync(tmp.fileno())
        if original.read_bytes() != source:
            raise RofError("overlay changed externally while preparing patch")
        os.replace(tempname, original)
        if original.read_bytes() != replacement:
            raise RofError("installed overlay failed verification")
    finally:
        if tempname is not None and tempname.exists():
            tempname.unlink()
    print(f"EXPERIMENT INSTALLED: {original}")
    print(f"Original preserved at: {backup}")


def restore(original: Path):
    backup = original.with_name(original.name + BACKUP_SUFFIX)
    if not backup.is_file() or backup.is_symlink():
        raise RofError(f"original overlay backup missing: {backup}")
    current_bytes = original.read_bytes()
    backed_bytes = backup.read_bytes()
    old = RofArchive(backed_bytes)
    if set(old.members) != {PATCH_AIRFRAME}:
        raise RofError("backup is not the original one-member overlay")
    if current_bytes == backed_bytes:
        # Backup was created but the patch was never swapped in.
        backup.unlink()
        print("Original patch overlay was already intact; stale backup cleaned up")
        return
    patch = RofArchive(current_bytes)
    if set(patch.members) != {PATCH_AIRFRAME, BASE_VIDEO}:
        raise RofError("current overlay is not the expected two-member experiment")
    current_video = patch.read(BASE_VIDEO)
    if current_video is None or sha256(current_video) != VIDEO_PATCHED_SHA256:
        raise RofError("current VIDEO.SCRIPT is not this exact experiment; refusing restore")
    if old.read(PATCH_AIRFRAME) != patch.read(PATCH_AIRFRAME):
        raise RofError("AIRFRAME content changed; refusing unsafe restoration")
    os.replace(backup, original)
    print(f"Original patch overlay restored: {original}")


def self_test():
    from inspect_gos_rof import make_fixture

    synthetic = b"initialize(HVA)\r\ninitialize(IVA)\r\ninitialize(IVA)\r\ninitialize(IVA)\r\n"
    assert synthetic.count(b"initialize(IVA)") == 3
    image = write_rof({
        PATCH_AIRFRAME: b"original-airframe",
        BASE_VIDEO: synthetic,
    })
    decoded = RofArchive(image)
    assert decoded.read(PATCH_AIRFRAME) == b"original-airframe"
    assert decoded.read(BASE_VIDEO) == synthetic
    assert make_fixture().startswith(b"\x01\x00\x00\x00")
    # Mutating a GOS script is prohibited unless the exact original hash matches.
    try:
        insert_disabled_messages(synthetic)
    except RofError:
        pass
    else:
        raise AssertionError("unexpected script was accepted")
    with tempfile.TemporaryDirectory() as work:
        root = Path(work)
        overlay = root / "crimptch.rof"
        overlay.write_bytes(write_rof({PATCH_AIRFRAME: b"original-airframe"}))
        pristine = overlay.read_bytes()
        # Test the backup-only interrupted-install recovery separately.
        from_preview_backup = root / ("crimptch.rof" + BACKUP_SUFFIX)
        atomic_write_new(from_preview_backup, pristine)
        restore(overlay)
        assert overlay.read_bytes() == pristine
        assert not from_preview_backup.exists()
        installed = write_rof({
            PATCH_AIRFRAME: b"original-airframe",
            BASE_VIDEO: b"mail (10000, HVA)",
        })
        install(overlay, installed)
        assert overlay.read_bytes() == installed
        assert (root / ("crimptch.rof" + BACKUP_SUFFIX)).read_bytes() == pristine
        try:
            restore(overlay)
        except RofError:
            pass
        else:
            raise AssertionError("restore must reject a synthetic non-owner script")
        assert overlay.read_bytes() == installed
        # Only the self-test returns its synthetic preview to pristine bytes.
        os.replace(root / ("crimptch.rof" + BACKUP_SUFFIX), overlay)
        assert overlay.read_bytes() == pristine
    print("GOS Video patch overlay synthetic self-test passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path, nargs="?", help="Original Crimson Skies game directory")
    options = parser.add_mutually_exclusive_group()
    options.add_argument("--preview", type=Path,
                         help="Generate NEW optional overlay .rof outside the game; do not install")
    options.add_argument("--install", action="store_true",
                         help="Explicitly install after creating an exclusive original .bak")
    options.add_argument("--restore", action="store_true",
                         help="Restore the originally backed-up patch overlay")
    options.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.path is None or not (args.preview or args.install or args.restore):
        parser.error("provide game root and exactly one of --preview, --install, --restore")
    try:
        root = args.path.expanduser()
        base, patch = game_paths(root)
        if args.restore:
            restore(patch)
        else:
            overlay, video = prepare_patch(base, patch)
            print(f"Verified original VIDEO.SCRIPT: {VIDEO_ORIGINAL_SHA256}")
            print(f"Patched VIDEO.SCRIPT SHA-256: {sha256(video)}")
            print("Native GOS message 10000 added to HVA and IVA immediately after initialization")
            print("Existing AIRFRAME.SCRIPT patch member preserved byte-for-byte after decompression")
            if args.preview:
                output = args.preview
                if output.suffix.lower() != ".rof":
                    raise RofError("--preview must end in .rof")
                if output.absolute() == patch.absolute():
                    raise RofError("preview output cannot be the installed patch archive")
                atomic_write_new(output, overlay)
                print(f"PREVIEW ONLY (game unchanged): {output}")
            else:
                install(patch, overlay)
    except (OSError, ValueError) as exc:
        parser.exit(2, f"GOS patch error (no automatic retry): {exc}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
