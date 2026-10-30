#!/usr/bin/env python3
"""Analyze Wine +relay SafeDisc main IOCTL results, without executing the game.

Uses only standard-library Python and logs from an isolated test environment.
Never reads a live process, modifies binaries, or publishes protocol buffers.

Copyright (C) 2026 legluondunet
SPDX-License-Identifier: GPL-3.0-or-later
"""
import argparse
from collections import Counter
from pathlib import Path
import re

MAIN_CALL = re.compile(
    r"^([0-9a-f]+):Call KERNEL32\.DeviceIoControl\("
    r"([0-9a-f]+),ef002407,([0-9a-f]+),([0-9a-f]+),"
    r"([0-9a-f]+),([0-9a-f]+),", re.I
)
MAIN_RETURN = re.compile(
    r"^([0-9a-f]+):Ret  KERNEL32\.DeviceIoControl\(\) retval=([0-9a-f]+)", re.I
)
NT_RETURN = re.compile(
    r"^([0-9a-f]+):Ret  ntdll\.NtDeviceIoControlFile\(\) retval=([0-9a-f]+)", re.I
)
DLL_ATTACH = re.compile(
    r'^([0-9a-f]+):Call PE DLL .*module=([0-9a-f]+) L"dplayerx\.dll".*reason=PROCESS_ATTACH', re.I
)
LAST_ERROR = re.compile(
    r"^([0-9a-f]+):Ret  KERNEL32\.GetLastError\(\) retval=([0-9a-f]+)", re.I
)
FAULT = re.compile(
    r"^AV #([0-9]+): EIP=0x([0-9a-f]+), memory operand=0x([0-9a-f]+)", re.I
)
KNOWN_STATUS = {
    0xC0000123: "STATUS_FILE_DELETED",
    0xC0000022: "STATUS_ACCESS_DENIED",
}
def summarize(lines):
    calls = []
    returns = Counter()
    statuses = Counter()
    process_id = None
    image_base = None
    errors = []
    faults = []
    for line in lines:
        m = MAIN_CALL.search(line)
        if m:
            pid, handle, input_addr, input_size, output_addr, output_size = m.groups()
            if process_id is None:
                process_id = pid.lower()
            if pid.lower() == process_id:
                calls.append((int(handle,16), int(input_addr,16),
                              int(input_size,16), int(output_addr,16),
                              int(output_size,16)))
        m = MAIN_RETURN.search(line)
        if m and m.group(1).lower() == process_id:
            returns[int(m.group(2), 16)] += 1
        m = NT_RETURN.search(line)
        if m and m.group(1).lower() == process_id:
            statuses[int(m.group(2), 16)] += 1
        m = DLL_ATTACH.search(line)
        if m and (process_id is None or m.group(1).lower() == process_id):
            image_base = int(m.group(2),16)
        m = LAST_ERROR.search(line)
        if m and (process_id is None or m.group(1).lower() == process_id):
            errors.append(int(m.group(2),16))
        m = FAULT.search(line)
        if m:
            faults.append((int(m.group(1)),int(m.group(2),16),int(m.group(3),16)))
    return {
        "process": process_id,
        "calls": calls, "returns": returns, "statuses": statuses,
        "base": image_base, "last_errors": errors, "faults": faults,
    }

def report(info):
    calls = info["calls"]
    print("Wine SafeDisc IOCTL relay: offline summary (no game code executed)")
    print(f"Observed process/thread prefix: {info['process'] or 'unavailable'}")
    print(f"0xEF002407 Win32 DeviceIoControl entries: {len(calls)}")
    print("Win32 results: " + (", ".join(
        f"0x{k:08X} ({'FALSE' if k == 0 else 'TRUE'}): {v}"
        for k,v in sorted(info["returns"].items())
    ) or "none"))
    print("NTDLL relay results (can be duplicated by WOW64 layers): " +
          (", ".join(f"0x{k:08X} ({KNOWN_STATUS.get(k, 'unknown')}): {v}"
                      for k,v in sorted(info["statuses"].items())) or "none"))
    if calls:
        patterns = Counter(calls)
        for (handle, inp, insize, outp, outsize), count in patterns.most_common(6):
            print(f"  requests={count}: handle=0x{handle:X}, "
                  f"in=0x{inp:08X}/0x{insize:X}, out=0x{outp:08X}/0x{outsize:X}")
            print(f"    subcommand DWORD address IN THIS RUN: 0x{inp + 12:08X}")
        print("The command DWORD itself is NOT present in this relay log.")
    if info["base"] is not None:
        print(f"DLL image base in this run: 0x{info['base']:08X}")
    if info["last_errors"]:
        print("Win32 GetLastError results: " +
              ", ".join(str(code) for code in info["last_errors"]))
    for number,eip,operand in info["faults"][-2:]:
        rva = f"0x{eip - info['base']:X}" if info["base"] is not None else "unknown"
        print(f"AV #{number:02d}: EIP=0x{eip:08X}, RVA={rva}, operand=0x{operand:08X}")
    if len(calls) != sum(info["returns"].values()):
        print("WARNING: IOCTL call/return counts differ; examine trace for truncation.")
    print("No driver emulation or ICD decryption performed.")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path, help="Private Wine +relay log")
    args = parser.parse_args()
    with args.log.open("r", encoding="utf-8", errors="replace") as file:
        report(summarize(file))

if __name__ == "__main__":
    main()
