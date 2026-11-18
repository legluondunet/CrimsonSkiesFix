#!/usr/bin/env bash
# PR #51: confirm upstream Wine >=11.18 AMStream flush fix with the installed game.
# All diagnostic output and the disposable prefix stay beside the test installer.
set -Eeuo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
cd "$ROOT"

# Explicit Lutris runner: keep Wine, Wineboot and Wineserver on the SAME build.
# Optional override lets this test script remain usable with another runner.
WINE_BIN="${PR51_WINE_BIN:-/home/romain/.local/share/lutris/runners/wine/wine-11.22-staging-amd64/bin/wine}"
RUNNER_DIR="$(dirname -- "$WINE_BIN")"
WINEBOOT_BIN="$RUNNER_DIR/wineboot"
WINESERVER_BIN="$RUNNER_DIR/wineserver"
for binary in "$WINE_BIN" "$WINEBOOT_BIN" "$WINESERVER_BIN"; do
  if [[ ! -f "$binary" || ! -x "$binary" ]]; then
    echo "Missing or non-executable Lutris runner component: $binary" >&2
    echo "Runner path must contain wine, wineboot, and wineserver together." >&2
    exit 2
  fi
done
for command in mktemp find cp gzip tar sha256sum; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Missing dependency: $command" >&2
    exit 2
  }
done
# Explicit executable paths are used below, and these variables also force
# any child launch to use the same runner rather than a system Wine instance.
export PATH="$RUNNER_DIR:$PATH"
export WINELOADER="$WINE_BIN"
export WINESERVER="$WINESERVER_BIN"
ver=$("$WINE_BIN" --version)
if [[ ! "$ver" =~ ([0-9]+)\.([0-9]+) ]]; then
  echo "Cannot determine Wine version from: $ver" >&2
  exit 2
fi
major=$((10#${BASH_REMATCH[1]}))
minor=$((10#${BASH_REMATCH[2]}))
if (( major < 11 || (major == 11 && minor < 18) )); then
  echo "This test requires Wine 11.18 or newer; found: $ver" >&2
  echo "Wine 11.17 has a known amstream flush deadlock fixed upstream in 11.18." >&2
  echo "Install/select a newer Wine runner first; no game files were changed." >&2
  exit 2
fi

mapfile -d '' -t launchers < <(
  find "$ROOT" -maxdepth 1 -type f \
    \( -iname 'CrimsonSkiesFix.exe' -o -iname 'CrimsonFix_installer.exe' \
       -o -iname 'CrimsonSkiesFix_installer.exe' \) -print0
)
if (( ${#launchers[@]} != 1 )) || [[ ! -f game/CRIMSON.EXE ]]; then
  echo "The script must be beside ONE installer and game/CRIMSON.EXE." >&2
  exit 2
fi
LAUNCHER="${launchers[0]}"
NAME="${LAUNCHER##*/}"
OUT=$(mktemp -d "$ROOT/PR51-wine1118-XXXXXXXX")
PREFIX="$OUT/.temporary-prefix"
mkdir -p "$PREFIX" "$OUT/pre-test-backup"
for item in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  [[ ! -f "$ROOT/$item" ]] || cp -p -- "$ROOT/$item" "$OUT/pre-test-backup/"
done
{
  printf 'Wine=%s\n' "$ver"
  printf 'WineBinary=%s\n' "$WINE_BIN"
  printf 'WinebootBinary=%s\n' "$WINEBOOT_BIN"
  printf 'WineserverBinary=%s\n' "$WINESERVER_BIN"
  printf 'WineRunnerFolder=%s\n' "$RUNNER_DIR"
  printf 'PathPrefix=%s\n' "$RUNNER_DIR"
  printf 'Launcher=%s\n' "$NAME"
  sha256sum "$LAUNCHER"
} > "$OUT/environment.txt"

echo "PR51 test runner: $WINE_BIN ($ver)"
echo "All test files remain in: $OUT"
echo "Initializing a CLEAN local Wine prefix (no Winetricks)..."
env -u WINEDLLOVERRIDES WINEPREFIX="$PREFIX" WINEDEBUG=-all \
  "$WINEBOOT_BIN" -u > "$OUT/wineboot.log" 2>&1

echo "Launching the game with Wine builtins quartz and amstream."
(
  cd "$ROOT"
  env WINEPREFIX="$PREFIX" WINEDLLOVERRIDES='amstream=b;quartz=b' \
    WINEDEBUG='+timestamp,+tid,+quartz,+amstream,+winegstreamer,+sync' \
    "$WINE_BIN" "./$NAME"
) > "$OUT/wine-fixed.log" 2>&1 &
launch_pid=$!

echo "Watch both intro movies, then inspect the main menu."
read -r -p "Are both videos correct and the main menu VISIBLE/interactive? (free text): " note || true
printf 'UserObservation=%s\n' "$note" > "$OUT/observations.txt"
printf 'SnapshotUTC=%s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$OUT/observations.txt"

echo "Stopping only the disposable test prefix (existing Wine prefixes unaffected)..."
env WINEPREFIX="$PREFIX" WINEDEBUG=-all "$WINESERVER_BIN" -k \
  > "$OUT/wineserver-stop.log" 2>&1 || true
wait "$launch_pid" || true
for item in CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  [[ ! -f "$ROOT/$item" ]] || cp -p -- "$ROOT/$item" "$OUT/after-$item"
done
gzip -f -1 -- "$OUT/wine-fixed.log"
rm -rf -- "$PREFIX"
ARCHIVE="$ROOT/$(basename "$OUT").tar.gz"
tar -czf "$ARCHIVE" -C "$ROOT" "$(basename "$OUT")"
echo "Share this archive (after checking for private file paths): $ARCHIVE"
