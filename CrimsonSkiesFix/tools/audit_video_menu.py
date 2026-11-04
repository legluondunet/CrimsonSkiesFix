#!/usr/bin/env python3
"""Read-only Crimson Skies Video Preferences resource inventory.

Usage:
  python tools/audit_video_menu.py "/path/to/game" -o video-ui-audit.json
Scans relative filenames and bounded text markers. Never writes game assets,
redistributes content, or emits absolute game paths.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path

EXPECTED="43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75"
WORDS=("preferences","graphics","resolution","video","gosresid",
       "goschecked","goscolordisabled","nselectedvideodevice","scriptloader")
TYPES={".txt",".ini",".cfg",".gos",".xml",".csv",".lua",".scr",".script",
       ".json",".dat",".zbd",".zrd",".pak",".bin",".db"}
MAX_BYTES=8*1024*1024
MAX_FILES=18000
MAX_HITS=24

def digest(path:Path)->str:
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b""):
            h.update(chunk)
    return h.hexdigest()

def inspect_file(path:Path,relative:str,size:int)->dict|None:
    name=relative.casefold()
    if not any(t in name for t in WORDS) and path.suffix.casefold() not in TYPES:
        return None
    result={"relative_path":relative,"size_bytes":size}
    if size==0 or size>MAX_BYTES:
        if any(w in name for w in WORDS):
            result["note"]="name only; empty or over bounded content limit"
            return result
        return None
    try:raw=path.read_bytes()
    except OSError:
        result["note"]="unreadable"
        return result
    lower=raw.lower()
    hits=[]
    for term in WORDS:
        token=term.encode("ascii")
        for encoding,pattern in (
            ("ascii",token),
            ("utf16le",b"".join(bytes((c,0)) for c in token))
        ):
            cursor=0
            while len(hits)<MAX_HITS:
                cursor=lower.find(pattern,cursor)
                if cursor<0:break
                entry={"term":term,"offset":cursor,"encoding":encoding}
                if encoding=="ascii":
                    context=raw[max(0,cursor-28):cursor+64]
                    entry["preview"]="".join(chr(b) if 32<=b<=126 else " " for b in context)[:92]
                hits.append(entry)
                cursor+=len(pattern)
    if not hits and not any(w in name for w in WORDS):return None
    result["matches"]=sorted(hits,key=lambda x:x["offset"])[:MAX_HITS]
    result["sha256"]=hashlib.sha256(raw).hexdigest()
    return result

def audit(root:Path)->dict:
    root=root.resolve()
    if not root.is_dir():raise ValueError("game folder not found")
    executable=next((p for p in (root/"CRIMSON.EXE",root/"crimson.exe")
                     if p.is_file()),None)
    verified=executable is not None and digest(executable)==EXPECTED
    result={"tool":"CrimsonSkiesFix video menu audit v1",
            "exe_verified":verified,"absolute_paths_excluded":True,
            "inspected_files":0,"interesting_files":[],"truncated":False}
    if not verified:return result
    for directory,subdirs,names in os.walk(root):
        subdirs[:]=sorted(n for n in subdirs if n.casefold() not in
                          {".git","screenshots","savedgames","logs","crash-dumps"})
        for name in sorted(names):
            path=Path(directory)/name
            try:
                relative=path.relative_to(root).as_posix()
                if path.is_symlink() or relative.casefold()=="crimson.exe":continue
                size=path.stat().st_size
            except (OSError,ValueError):continue
            result["inspected_files"]+=1
            if result["inspected_files"]>MAX_FILES:
                result["truncated"]=True
                return result
            hit=inspect_file(path,relative,size)
            if hit:result["interesting_files"].append(hit)
    return result

def main()->int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game_folder",type=Path)
    parser.add_argument("-o","--output",type=Path)
    args=parser.parse_args()
    try:result=audit(args.game_folder)
    except (OSError,ValueError) as error:parser.error(str(error))
    data=json.dumps(result,indent=2,ensure_ascii=False)+"\n"
    if args.output:
        args.output.write_text(data,encoding="utf-8")
        print("Wrote",args.output)
    else:print(data,end="")
    return 0 if result["exe_verified"] else 2

if __name__=="__main__":
    raise SystemExit(main())
