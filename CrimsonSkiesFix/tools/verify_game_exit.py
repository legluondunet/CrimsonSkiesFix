#!/usr/bin/env python3
"""External game-exit observer: no DllMain, injection, COM hooks or game edits.

Use the *same Wine / GE-Proton environment* as the working game:
python3 verify_game_exit.py --cwd "/path/to/Crimson Skies" -- /path/to/wine CRIMSON.EXE

The command must stay alive until CRIMSON.EXE terminates. A detached GUI
launcher does not qualify. Exit status zero supports, but does not prove,
a clean exit; stderr/stdout are inspected for explicit unhandled exceptions.
"""
from __future__ import annotations

import argparse
import datetime as datetime
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time

PREFIX = "CrimsonSkiesFix-exitcheck"
# Do NOT match generic Wine SEH traces: handled exceptions can be benign.
CRASH_PATTERNS = (
    re.compile(r"Unhandled exception(?:\s*:|\s+)", re.I),
    re.compile(r"segmentation fault", re.I),
    re.compile(r"stack smashing detected", re.I),
)
LAUNCHER_EXIT = re.compile(r"Initial process has exited\s*\(return code:\s*(-?\d+)\s*\)")
NTSTATUS_CRASH = {
    0xC0000005,  # Access violation
    0xC000001D,  # Illegal instruction
    0xC0000094,  # Integer divide by zero
    0xC00000FD,  # Stack overflow
    0xC0000409,  # Stack buffer overrun
}


def scan_output(paths: list[Path]) -> tuple[list[str], int | None]:
    unhandled: list[str] = []
    launcher_code: int | None = None
    for path in paths:
        with path.open(encoding="utf-8", errors="replace") as stream:
            for number, line in enumerate(stream, 1):
                if len(unhandled) < 8 and any(p.search(line) for p in CRASH_PATTERNS):
                    unhandled.append(f"{path.name}:{number}: {line.strip()[:300]}")
                if match := LAUNCHER_EXIT.search(line):
                    launcher_code = int(match.group(1))
    return unhandled, launcher_code


def classify(code: int, markers: list[str], launcher_code: int | None) -> str:
    if (code & 0xFFFFFFFF) in NTSTATUS_CRASH:
        return "CRASH_LIKELY_WINDOWS_EXCEPTION"
    if markers:
        return "CRASH_EVIDENCE_IN_PROCESS_OUTPUT"
    if code < 0:
        return "PROCESS_ENDED_BY_POSIX_SIGNAL"
    if launcher_code is not None and launcher_code != 0:
        return "NONZERO_GAME_EXIT_REPORTED_BY_LAUNCHER"
    if code != 0:
        return "NONZERO_PROCESS_EXIT_NEEDS_REVIEW"
    return "ZERO_PROCESS_EXIT_NO_UNHANDLED_EXCEPTION_FOUND"


def observe(command: list[str], cwd: Path, report_dir: Path) -> dict[str, object]:
    if not command:
        raise ValueError("Missing game launch command after '--'")
    if not cwd.is_dir():
        raise ValueError(f"Game directory does not exist: {cwd}")
    report_dir.mkdir(parents=True, exist_ok=True)
    stdout = report_dir / f"{PREFIX}.stdout.log"
    stderr = report_dir / f"{PREFIX}.stderr.log"
    report = report_dir / f"{PREFIX}.report.txt"
    started_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
    start = time.monotonic()
    # Stream potentially large Wine diagnostics directly to files.
    with stdout.open("wb") as out_file, stderr.open("wb") as err_file:
        process = subprocess.Popen(command, cwd=cwd, stdout=out_file, stderr=err_file)
        pid = process.pid
        code = process.wait()
    duration = time.monotonic() - start
    markers, launcher_code = scan_output([stdout, stderr])
    status = classify(code, markers, launcher_code)
    lines = [
        f"Monitor={PREFIX}",
        f"StartedUTC={started_at}",
        f"DurationSeconds={duration:.3f}",
        f"MonitoredPID={pid}",
        f"ProcessExitCode={code}",
        f"ProcessExitHex=0x{code & 0xFFFFFFFF:08X}",
        f"LauncherReportedGameExit={launcher_code if launcher_code is not None else 'not reported'}",
        f"Assessment={status}",
        f"Stdout={stdout.name}",
        f"Stderr={stderr.name}",
        "Scope=Watched command only; detached children cannot be verified",
        "Interpretation=Zero exit without exception evidence supports, but does not prove, a clean exit",
    ]
    lines.extend(f"ExceptionEvidence={m}" for m in markers)
    report.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"EXIT-CHECK: {status}; exit={code}; launcher_exit={launcher_code}; report={report}")
    return {"code": code, "status": status, "report": report}


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="crimson-exit-test-") as temp:
        root = Path(temp)
        good = observe([sys.executable, "-c", "print('done')"], root, root)
        assert good["status"] == "ZERO_PROCESS_EXIT_NO_UNHANDLED_EXCEPTION_FOUND"
        assert "ProcessExitCode=0" in good["report"].read_text(encoding="utf-8")
        nonzero = observe([sys.executable, "-c", "import sys; sys.exit(7)"], root, root)
        assert nonzero["status"] == "NONZERO_PROCESS_EXIT_NEEDS_REVIEW"
        crash = observe([sys.executable, "-c",
                         "import sys; print('Unhandled exception: synthetic', file=sys.stderr); sys.exit(5)"],
                        root, root)
        assert crash["status"] == "CRASH_EVIDENCE_IN_PROCESS_OUTPUT"
        launcher = observe([sys.executable, "-c",
                            "print('Initial process has exited (return code: 5)')"], root, root)
        assert launcher["status"] == "NONZERO_GAME_EXIT_REPORTED_BY_LAUNCHER"
        assert classify(0xC0000005, [], None) == "CRASH_LIKELY_WINDOWS_EXCEPTION"
        assert classify(-signal.SIGSEGV, [], None) == "PROCESS_ENDED_BY_POSIX_SIGNAL"
    print("EXIT-CHECK: self-test PASS (zero, nonzero, exception, launcher, NTSTATUS, signal)")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--cwd", type=Path, help="game directory")
    parser.add_argument("--report-dir", type=Path, help="report directory (default: game directory)")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="-- <actual Wine/Proton command> CRIMSON.EXE")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.cwd is None:
        parser.error("--cwd is required")
    command = args.command[1:] if args.command and args.command[0] == "--" else args.command
    if not command:
        parser.error("specify the game process command after --")
    try:
        cwd = args.cwd.resolve()
        result = observe(command, cwd, (args.report_dir or cwd).resolve())
        return 0 if result["status"] == "ZERO_PROCESS_EXIT_NO_UNHANDLED_EXCEPTION_FOUND" else 1
    except (OSError, ValueError) as error:
        print(f"EXIT-CHECK: ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
