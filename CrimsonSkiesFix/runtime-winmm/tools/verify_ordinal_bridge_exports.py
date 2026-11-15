#!/usr/bin/env python3
"""Verify the generated experimental WinMM bridge PE32 ABI, not runtime safety."""
from __future__ import annotations
import argparse
import re
import struct
from pathlib import Path

MAX_ORDINAL = 1023
NAMED_START = 2048
NAMED_COUNT = 185

def verify(dll: Path, definitions: Path) -> None:
    data=dll.read_bytes()
    def u16(p: int) -> int:
        if p<0 or p+2>len(data):raise ValueError(f"truncated U16 {p:#x}")
        return struct.unpack_from("<H",data,p)[0]
    def u32(p: int) -> int:
        if p<0 or p+4>len(data):raise ValueError(f"truncated U32 {p:#x}")
        return struct.unpack_from("<I",data,p)[0]
    if data[:2]!=b"MZ":raise ValueError("missing DOS signature")
    pe=u32(0x3c)
    if data[pe:pe+4]!=b"PE\0\0" or u16(pe+4)!=0x14c:
        raise ValueError("expected PE32 i386 DLL")
    opt=pe+24
    count=u16(pe+6)
    opt_size=u16(pe+20)
    if u16(opt)!=0x10b or count>128 or opt_size<104:
        raise ValueError("malformed PE32 headers")
    sections=[]
    for i in range(count):
        p=opt+opt_size+i*40
        sections.append((u32(p+12), max(u32(p+8),u32(p+16)),u32(p+16),u32(p+20)))
    def off(rva: int, size: int=1) -> int:
        for addr,span,raw_size,raw in sections:
            delta=rva-addr
            if 0<=delta and delta+size<=min(span,raw_size):
                pos=raw+delta
                if pos+size<=len(data):return pos
        raise ValueError(f"unmapped export RVA {rva:#x}")
    def cstr(rva: int) -> str:
        p=off(rva)
        e=data.find(b"\0",p,min(p+256,len(data)))
        if e<0:raise ValueError("unterminated export")
        return data[p:e].decode("ascii")
    export_rva,export_size=u32(opt+96),u32(opt+100)
    if not export_rva or export_size<40:raise ValueError("no export directory")
    exp=off(export_rva,40)
    base,functions,names = u32(exp+16),u32(exp+20),u32(exp+24)
    if base!=1 or functions < NAMED_START+NAMED_COUNT-1 or not NAMED_COUNT<=names<=NAMED_COUNT+2:
        raise ValueError(f"wrong export shape: base={base}, funcs={functions}, names={names}")
    funcs=off(u32(exp+28),4*functions)
    name_rvas=off(u32(exp+32),4*names)
    name_ord=off(u32(exp+36),2*names)
    wanted={}
    for line in definitions.read_text(encoding="utf-8").splitlines():
        line=line.split(";",1)[0].strip()
        m=re.fullmatch(r"([A-Za-z][A-Za-z0-9_]*)=crimson_winmm_(?:forward|extra)_\d+\s+@(\d+)",line)
        if m:
            if m[1] in wanted:raise ValueError("duplicate expected export")
            wanted[m[1]]=NAMED_START+len(wanted)
    if len(wanted)!=NAMED_COUNT:
        raise ValueError("missing original name inventory")
    actual={}
    for i in range(names):
        name=cstr(u32(name_rvas+4*i))
        ordinal=base+u16(name_ord+2*i)
        if ordinal>functions or name in actual:
            raise ValueError("invalid duplicate export/name table")
        actual[name]=ordinal
    if any(actual.get(name)!=ordinal for name,ordinal in wanted.items()):
        raise ValueError("named exports not isolated in 2048+ slots")
    if any(n<1024 for n in actual.values()):
        raise ValueError("ordinal-only slot has a name that can collide")
    for n in range(1,MAX_ORDINAL+1):
        target=u32(funcs+4*(n-base))
        if not target or export_rva<=target<export_rva+export_size:
            raise ValueError(f"ordinal #{n} has no real x86 thunk")
    for n in range(MAX_ORDINAL+1,NAMED_START):
        if u32(funcs+4*(n-base)):
            raise ValueError(f"reserved gap ordinal #{n} is unexpectedly occupied")
    print(f"WINMM-ORDINAL-BRIDGE-EXPORTS PASS: {MAX_ORDINAL} numeric-only thunks, "
          f"{NAMED_COUNT} names at @{NAMED_START}..@{NAMED_START+NAMED_COUNT-1}; "
          "x86 PE; native provider resolution still needs runtime test")

def main() -> int:
    cli=argparse.ArgumentParser(description=__doc__)
    cli.add_argument("dll",type=Path)
    cli.add_argument("--definitions",required=True,type=Path)
    args=cli.parse_args()
    try:verify(args.dll,args.definitions)
    except (OSError,ValueError,UnicodeError) as exc:
        cli.exit(1,f"WINMM-ORDINAL-BRIDGE-EXPORTS FAIL: {exc}\n")
    return 0
if __name__=="__main__":raise SystemExit(main())
