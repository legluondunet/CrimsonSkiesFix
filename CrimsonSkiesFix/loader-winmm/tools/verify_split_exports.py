#!/usr/bin/env python3
"""Split-only PE32 WinMM export gate: no ambiguous low numeric WinMM ordinals.

The original game imports all 14 WinMM entries BY NAME. A numbered import
from a third-party dependency, however, must NEVER silently call an unrelated
multimedia function. This verifies the split loader's private high-index named
aliases and leaves the 1..1023 numeric range empty. The full Wine/Windows
ordinal ABI remains unsupported: this is a fail-closed test, NOT a forwarder.
"""
from __future__ import annotations
import argparse
import importlib.util
import re
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
AUDITOR=ROOT/"runtime-winmm/tools/audit_system_winmm_abi.py"
SPEC=importlib.util.spec_from_file_location("winmm_abi",AUDITOR)
assert SPEC and SPEC.loader
ABI=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ABI)

DEF=Path(__file__).resolve().parents[1]/"winmm_loader_exports.def"
LOW_MAX=1023
HIGH_FIRST=2048
EXPORT_COUNT=185


def expected(definition:Path=DEF)->dict[str,int]:
    out:dict[str,int]={}
    for line in definition.read_text(encoding="utf-8-sig").splitlines():
        stripped=line.split(";",1)[0].strip()
        if not stripped or stripped in ("LIBRARY winmm.dll","EXPORTS"):
            continue
        match=re.fullmatch(
            r"([A-Za-z][A-Za-z0-9_]*)=crimson_winmm_(?:forward|extra)_([0-9]+)\s+@(\d+)",
            stripped)
        if match is None:raise ValueError(f"invalid split export definition: {stripped!r}")
        name,idx,num=match[1],int(match[2]),int(match[3])
        if name in out:raise ValueError(f"duplicate split export: {name}")
        if num < HIGH_FIRST:raise ValueError(f"unsafe low WinMM ordinal: {num}")
        if name in ("DllMain","CrimsonSkiesFixBootstrapStatus"):
            raise ValueError("reserved diagnostic export")
        out[name]=num
    if len(out)!=EXPORT_COUNT or set(out.values())!=set(range(HIGH_FIRST,HIGH_FIRST+EXPORT_COUNT)):
        raise ValueError("expected 185 contiguous high-index named exports @2048..2232")
    return out


def check(candidate:dict, definition:dict)->None:
    observed=candidate["named"]
    # These two exports are expected from Rust's no_mangle declarations.
    allowed={"DllMain","CrimsonSkiesFixBootstrapStatus"}
    missing=set(definition)-set(observed)
    extra=set(observed)-set(definition)-allowed
    wrong={n:(definition[n],observed[n])
           for n in set(definition)&set(observed) if definition[n]!=observed[n]}
    unsafe=sorted(x for x in candidate["occupied"] if 1<=x<=LOW_MAX)
    if missing or extra or wrong or unsafe:
        raise ValueError(f"missing={sorted(missing)} extra={sorted(extra)} "
                         f"wrong={wrong} unsafe_low_ordinals={unsafe[:30]}")
    if observed.get("CrimsonSkiesFixBootstrapStatus") is None:
        raise ValueError("missing split runtime diagnostic export")


def main()->int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll",type=Path)
    args=parser.parse_args()
    try:
        wanted=expected()
        candidate=ABI.export_table(args.dll)
        check(candidate,wanted)
    except (OSError,ValueError,ABI.PEError) as e:
        parser.exit(2,f"WINMM-SPLIT-ABI: FAIL: {e}\n")
    print("WINMM-SPLIT-ABI: PASS 185 named exports @2048..2232; "
          "OS ordinal range @1..@1023 vacant; low numeric imports fail closed")
    return 0

if __name__=="__main__":
    raise SystemExit(main())
