#!/usr/bin/env bash
# Wine-only SafeDisc research: filtered Win32 DeviceIoControl relay trace.
# Copyright (C) 2026 legluondunet
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Changes only a temporary RelayInclude registry value in an isolated prefix.
# Never patches or distributes Crimson Skies executables or DLLs.
#
# WINEPREFIX="$HOME/.wine-crimson-research" \
#   bash scripts/safedisc-relay-probe.sh ./safedisc-memory-probe.exe ./dplayerx.dll

set -euo pipefail

usage() {
    printf 'Usage: WINEPREFIX=isolated-prefix %s PROBE_EXE VERIFIED_DPLAYERX_DLL [LOG_FILE]\n' "$0" >&2
    exit 2
}
(( $# == 2 || $# == 3 )) || usage
[[ -n "${WINEPREFIX:-}" ]] || { echo 'ERROR: Set WINEPREFIX to an isolated, expendable research prefix.' >&2; exit 2; }
[[ "$WINEPREFIX" != "/" && "$WINEPREFIX" != "$HOME/.wine" ]] || { echo 'ERROR: Refusing default/main Wine prefix.' >&2; exit 2; }
[[ -d "$WINEPREFIX" ]] || { echo "ERROR: Wine prefix directory does not exist: $WINEPREFIX" >&2; exit 2; }
command -v wine >/dev/null || { echo 'ERROR: wine is not on PATH.' >&2; exit 2; }

probe=$1
dll=$2
log=${3:-safedisc-ioctl-relay.log}
[[ -f "$probe" && -f "$dll" ]] || { echo 'ERROR: Missing research probe executable or dplayerx.dll.' >&2; exit 2; }
[[ ! -e "$log" && ! -L "$log" ]] || { echo "ERROR: Log already exists; refusing to overwrite: $log" >&2; exit 2; }
regkey='HKCU\Software\Wine\Debug'
relay='kernel32.DeviceIoControl;kernelbase.DeviceIoControl;ntdll.NtDeviceIoControlFile;kernel32.GetLastError;kernelbase.GetLastError'
# Never overwrite a pre-existing RelayInclude value.
if WINEDEBUG=-all wine reg query "$regkey" /v RelayInclude >/dev/null 2>&1; then
    echo "ERROR: RelayInclude already exists in the chosen prefix. Refusing to replace it." >&2
    exit 2
fi

installed=0
cleanup() {
    local code=$?
    trap - EXIT INT TERM
    if (( installed )); then
        if WINEDEBUG=-all wine reg delete "$regkey" /v RelayInclude /f >/dev/null 2>&1; then
            echo 'Temporary Wine RelayInclude filter removed.'
        else
            echo 'WARNING: RelayInclude cleanup failed. Check this isolated prefix before reuse:' >&2
            echo "  WINEPREFIX=\"$WINEPREFIX\" wine reg delete '$regkey' /v RelayInclude /f" >&2
        fi
    fi
    exit "$code"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

WINEDEBUG=-all wine reg add "$regkey" /v RelayInclude /t REG_SZ /d "$relay" /f >/dev/null
installed=1
echo 'Experimental API relay filter installed in isolated Wine prefix.'
echo "Capturing DeviceIoControl and GetLastError relay calls to: $log"
echo 'The probe executes the verified protected DLL and may return 998.'

set +e
WINEDEBUG=-all,+relay wine "$probe" \
    --dll "$dll" --trace-exceptions --load-with-secdrv-alias \
    --output ./dplayerx-relay-output.bin > "$log" 2>&1
code=$?
set -e
echo "Probe exit code: $code (an error is expected)."
echo "Filtered relay lines:"
grep -Ei '(relay:.*(DeviceIoControl|GetLastError)|EXPERIMENT:|Secdrv alias|AV #09:|ERROR: LoadLibraryW)' "$log" | tail -n 45 || true
echo "Full private log: $log"
exit "$code"
