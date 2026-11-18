#!/usr/bin/env bash
# PR51: isolate the black-menu hang and capture live Windows thread stacks.
# All files and disposable prefixes stay beside the test installer executable.
set -Eeuo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
cd "$ROOT"
for cmd in wine wineboot wineserver winedbg timeout tar gzip find mktemp sha256sum cp; do
  command -v "$cmd" >/dev/null || { printf 'Missing command: %s\n' "$cmd" >&2; exit 2; }
done
mapfile -d '' -t launchers < <(
  find "$ROOT" -maxdepth 1 -type f \
    \( -iname 'CrimsonSkiesFix.exe' -o -iname 'CrimsonFix_installer.exe' \
       -o -iname 'CrimsonSkiesFix_installer.exe' \) -print0
)
if (( ${#launchers[@]} != 1 )) || [[ ! -f game/CRIMSON.EXE ]]; then
  echo "Place this script beside ONE launcher and game/CRIMSON.EXE in your test installation." >&2
  exit 2
fi
LAUNCHER="${launchers[0]}"
LAUNCHER_NAME="${LAUNCHER##*/}"
OUT=$(mktemp -d "$ROOT/PR51-hang-XXXXXXXX")
PREFIX="$OUT/.temporary-prefix"
mkdir -p "$PREFIX" "$OUT/backups"
for item in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  [[ ! -f "$ROOT/$item" ]] || cp -p -- "$ROOT/$item" "$OUT/backups/"
done
printf 'Wine=%s\n' "$(wine --version)" > "$OUT/environment.txt"
printf 'Executable=%s\n' "$LAUNCHER_NAME" >> "$OUT/environment.txt"
sha256sum "$LAUNCHER" >> "$OUT/environment.txt"
printf 'Purpose=collect live WineDbg snapshot at black MainMenu\n' >> "$OUT/environment.txt"
echo "Diagnostics: $OUT"
echo "Starting one CLEAN Wine prefix without Winetricks or native Microsoft DLLs."
env -u WINEDLLOVERRIDES WINEPREFIX="$PREFIX" WINEDEBUG=-all \
  wineboot -u > "$OUT/wineboot.log" 2>&1

(
  cd "$ROOT"
  env WINEPREFIX="$PREFIX" WINEDLLOVERRIDES='amstream=b;quartz=b' \
    WINEDEBUG=+timestamp,+tid,+quartz,+amstream,+winegstreamer,+dsound \
    wine "./$LAUNCHER_NAME"
) > "$OUT/wine-run.log" 2>&1 &
LAUNCH_PID=$!

echo "Watch the two introductory movies and then the game window."
read -r -p "When the menu is BLACK but the MUSIC plays, press Enter to capture the threads: " answer || true
printf 'SnapshotUserNote=%s\n' "$answer" > "$OUT/observations.txt"
printf 'SnapshotRequestedUTC=%s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$OUT/observations.txt"

env WINEPREFIX="$PREFIX" WINEDEBUG=-all \
  timeout --signal=TERM --kill-after=5 25 \
  winedbg --command "info proc" > "$OUT/winedbg-processes.txt" 2>&1 || true

game_pids=()
while IFS= read -r row; do
  if [[ "${row,,}" == *crimson.exe* ]] &&
     [[ "$row" =~ ^[[:space:]=>]*([[:xdigit:]]{8})[[:space:]] ]]; then
    game_pids+=("${BASH_REMATCH[1]}")
  fi
done < "$OUT/winedbg-processes.txt"

if (( ${#game_pids[@]} == 1 )); then
  PID="${game_pids[0]}"
  printf 'TargetWinPid=0x%s\n' "$PID" >> "$OUT/observations.txt"
  printf 'attach 0x%s\ninfo thread\nbt all\ndetach\nquit\n' "$PID" \
    > "$OUT/winedbg-commands.txt"
  echo "Capturing live WineDbg thread backtraces; no game code modifications..."
  if env WINEPREFIX="$PREFIX" WINEDEBUG=-all \
    timeout --signal=TERM --kill-after=5 55 \
    winedbg --file "$OUT/winedbg-commands.txt" \
    > "$OUT/winedbg-stack.txt" 2>&1; then
    printf 'Debugger=completed\n' >> "$OUT/observations.txt"
  else
    printf 'Debugger=error-or-timeout\n' >> "$OUT/observations.txt"
  fi
else
  printf 'Debugger=ambiguous-pid count=%s\n' "${#game_pids[@]}" >> "$OUT/observations.txt"
  echo "Could not identify a unique CRIMSON.EXE Windows PID; process list saved." >&2
fi

echo "Stopping ONLY the disposable test prefix, not your ordinary Wine prefix."
env WINEPREFIX="$PREFIX" WINEDEBUG=-all wineserver -k \
  > "$OUT/wineserver-stop.log" 2>&1 || true
wait "$LAUNCH_PID" || true
for item in CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  [[ ! -f "$ROOT/$item" ]] || cp -p -- "$ROOT/$item" "$OUT/after-$item"
done
gzip -f -1 "$OUT/wine-run.log"
rm -rf -- "$PREFIX"
ARCHIVE="$ROOT/$(basename "$OUT").tar.gz"
tar -czf "$ARCHIVE" -C "$ROOT" "$(basename "$OUT")"
echo "Archive ready: $ARCHIVE"
echo "Inspect for personal paths before sharing."
