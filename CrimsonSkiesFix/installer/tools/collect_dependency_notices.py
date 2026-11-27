"""Collect actual license/notice texts from Cargo's exact resolved packages.

Only package-level license/copyright/notice files are collected, not sources.
This is a distribution-review aid, not a determination of legal compliance.
"""
import argparse
import json
import re
from pathlib import Path

NOTICE_NAME = re.compile(r"^(licen[cs]e|copying|copyright|notice|authors)(?:$|[-_. ])", re.I)


def collect(metadata_files):
    packages = {}
    for meta in metadata_files:
        for pkg in json.loads(meta.read_text(encoding="utf-8-sig"))["packages"]:
            if pkg["source"] is None:
                continue  # First-party GPL text is shipped separately.
            key = (pkg["name"], pkg["version"], pkg["source"])
            previous = packages.setdefault(key, pkg)
            if previous["manifest_path"] != pkg["manifest_path"]:
                raise ValueError(f"Conflicting package paths: {key}")
    if not packages:
        raise ValueError("Empty dependency inventory")
    sections = [
        "CrimsonSkiesFix - third-party dependency notices",
        "Generated from locked Cargo metadata for the candidate target.",
        "License expressions are declarations; all collected alternatives are retained.",
        "Review nested notices and corresponding source before release.",
        "",
    ]
    for (name, version, source), pkg in sorted(packages.items()):
        root = Path(pkg["manifest_path"]).resolve(strict=True).parent
        files = {p for p in root.iterdir() if p.is_file() and NOTICE_NAME.match(p.name)}
        if pkg.get("license_file"):
            files.add(root / pkg["license_file"])
        if not files or not any(re.match(r"^(licen[cs]e|copying)", p.name, re.I) or
                                p == root / (pkg.get("license_file") or "") for p in files):
            raise ValueError(f"No license text found for {name} {version}")
        sections += ["=" * 72, f"{name} {version}", f"Source: {source}",
                     f"Declared license: {pkg.get('license') or 'See license file'}",
                     f"Repository: {pkg.get('repository') or 'See source'}", ""]
        for path in sorted(files, key=lambda p: str(p.relative_to(root)).lower()):
            resolved = path.resolve(strict=True)
            relative = resolved.relative_to(root)  # Reject paths outside package.
            if not resolved.is_file():
                raise ValueError(f"Not a file: {path}")
            sections += [f"--- {relative.as_posix()} ---",
                         resolved.read_text(encoding="utf-8-sig").rstrip(), ""]
    return "\n".join(sections) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", nargs="+", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.write_text(collect(args.metadata), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
