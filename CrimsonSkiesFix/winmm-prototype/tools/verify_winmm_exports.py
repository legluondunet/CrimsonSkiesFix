#!/usr/bin/env python3
"""Verify the exact x86 WinMM prototype DLL, reusing the strict PE export reader."""
import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
GAME_IMPORTS = (
    "mixerGetLineInfoA", "mixerOpen", "mciGetErrorStringA",
    "mixerGetLineControlsA", "mixerClose", "auxSetVolume",
    "auxGetVolume", "mciSendCommandA", "auxGetNumDevs",
    "auxGetDevCapsA", "mixerSetControlDetails", "mixerGetControlDetailsA",
    "mixerGetNumDevs", "timeGetTime",
)


def check(dll=None):
    definition = (ROOT/"src/winmm_exports.def").read_text()
    source = (ROOT/"src/lib.rs").read_text()
    expected = [(name, i, i+1) for i,name in enumerate(GAME_IMPORTS)]
    found = []
    for line in definition.splitlines():
        line = line.split(";", 1)[0].strip()
        match = re.fullmatch(
            r"([A-Za-z][A-Za-z0-9_]*)=crimson_winmm_forward_(\d+)\s+@(\d+)",line)
        if match:
            found.append((match[1],int(match[2]),int(match[3])))
        elif line not in ("","EXPORTS","LIBRARY winmm.dll"):
            raise ValueError(f"unexpected .def line: {line}")
    assert found == expected, "wrong WinMM game-import export list/order"
    rust_names = re.findall(r'    b"([A-Za-z][A-Za-z0-9_]*)\\0",',source)
    assert tuple(rust_names) == GAME_IMPORTS, "Rust exports differ from .def"
    rust_stubs = re.findall(r'_crimson_winmm_forward_(\d+):',source)
    assert rust_stubs == [str(i) for i in range(14)], "incomplete x86 trampoline table"
    if dll is not None:
        # Shared strict PE32 validator also refuses PE forwarding loops.
        sys.path.insert(0,str(ROOT.parent/"tools"))
        import verify_ddraw_exports as pe
        pe.COUNT=14
        actual=pe.exports(dll)
        expected_names={name:i+1 for i,name in enumerate(GAME_IMPORTS)}
        assert actual==expected_names, f"PE32 exports differ: {actual}"
    print("WINMM-EXPORTS PASS: 14 exact named game imports"+(
        " in compiled Windows x86 DLL" if dll is not None else " in source"))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--dll",type=Path)
    args=p.parse_args()
    try:
        check(args.dll)
    except (OSError,ValueError,AssertionError) as e:
        print(f"WINMM-EXPORTS FAIL: {e}",file=sys.stderr)
        return 1
    return 0


if __name__=="__main__":
    sys.exit(main())
