#!/usr/bin/env python3
"""Audit x86 WinMM-only runtime PE exports using independent repository parser."""
from pathlib import Path
import sys
import struct

SCRIPT_PATH=Path(__file__).resolve()
sys.path.insert(0,str(SCRIPT_PATH.parents[2]/"tools"))
import verify_ddraw_exports as pe

REQUIRED=(
    "mixerGetLineInfoA","mixerOpen","mciGetErrorStringA",
    "mixerGetLineControlsA","mixerClose","auxSetVolume","auxGetVolume",
    "mciSendCommandA","auxGetNumDevs","auxGetDevCapsA",
    "mixerSetControlDetails","mixerGetControlDetailsA",
    "mixerGetNumDevs","timeGetTime",
)

def check(dll:Path):
    data=dll.read_bytes()
    if len(data)<512 or data[:2]!=b"MZ":
        raise AssertionError("not a PE image")
    pos=struct.unpack_from("<I",data,0x3c)[0]
    if pos+26>len(data) or data[pos:pos+4]!=b"PE\0\0":
        raise AssertionError("malformed PE header")
    if struct.unpack_from("<H",data,pos+4)[0]!=0x14c:
        raise AssertionError("not a 32-bit x86 PE")
    # The shared strict validator reads the actual number of exported names
    # from the export directory. Its own parser checks complete ordinal tables,
    # forwarders, duplicate names and contiguous ordinals.
    opt=pos+24
    sections=struct.unpack_from("<H",data,pos+6)[0]
    opt_size=struct.unpack_from("<H",data,pos+20)[0]
    if opt_size<104 or opt+opt_size+40*sections>len(data):
        raise AssertionError("truncated PE optional/section headers")
    export_rva=struct.unpack_from("<I",data,opt+96)[0]
    def off(rva):
        section_headers=opt+opt_size
        for i in range(sections):
            q=section_headers+40*i
            size,base,raw_size,raw=struct.unpack_from("<IIII",data,q+8)
            if base<=rva<base+raw_size:
                at=raw+rva-base
                if at+40<=len(data):return at
        raise AssertionError("export RVA unmapped")
    exp=off(export_rva)
    names=struct.unpack_from("<I",data,exp+24)[0]
    if not 14<=names<=16:
        raise AssertionError(f"unexpected export name count: {names}")
    pe.COUNT=names
    exports=pe.exports(dll)
    missing=sorted(set(REQUIRED)-set(exports))
    extra=sorted(set(exports)-set(REQUIRED))
    if missing or set(extra)-{"DllMain"}:
        raise AssertionError(f"missing={missing}; extra={extra}")
    print(f"WINMM-RUNTIME-EXPORTS PASS: {len(REQUIRED)} required names, extras={extra}")
    return exports

if __name__=="__main__":
    try:
        check(Path(sys.argv[1]))
    except (OSError,IndexError,AssertionError,pe.ExportError) as exc:
        print(f"WINMM-RUNTIME-EXPORTS FAIL: {exc}",file=sys.stderr)
        sys.exit(1)
