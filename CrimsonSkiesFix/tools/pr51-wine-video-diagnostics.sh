#!/usr/bin/env bash
# PR #51: compare Wine builtins using a DEDICATED installed test game.
# The script, diagnostic reports and temporary Wine prefixes stay beside the launcher.
# No Winetricks and no changes to the user's existing Wine prefix.
set -Eeuo pipefail

usage() {
  echo "Usage: cd /folder/containing/CrimsonSkiesFix.exe && bash ./pr51-wine-video-diagnostics.sh"
  echo "The script must be placed beside CrimsonSkiesFix.exe (or CrimsonFix_installer.exe)."
  echo "A pre-installed game/CRIMSON.EXE must exist in the same folder."
  exit 2
}
[[ "$#" -eq 0 ]] || usage
for cmd in wine wineboot mktemp cp find gzip sha256sum tar; do
  command -v "$cmd" >/dev/null || { echo "Missing command: $cmd" >&2; exit 2; }
done
# Locate the launcher beside this downloaded script, regardless of where bash was called.
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
mapfile -d '' -t LAUNCHERS < <(
  find "$ROOT" -maxdepth 1 -type f \
    \( -iname 'CrimsonSkiesFix.exe' -o -iname 'CrimsonFix_installer.exe' \
       -o -iname 'CrimsonSkiesFix_installer.exe' \) -print0
)
if [[ "${#LAUNCHERS[@]}" -ne 1 || ! -f "$ROOT/game/CRIMSON.EXE" ]]; then
  echo "Expected exactly one CrimsonSkiesFix launcher beside this script and game/CRIMSON.EXE" >&2
  echo "Supported launcher names: CrimsonSkiesFix.exe, CrimsonFix_installer.exe, CrimsonSkiesFix_installer.exe" >&2
  usage
fi
LAUNCHER="${LAUNCHERS[0]}"
LAUNCHER_NAME="${LAUNCHER##*/}"
INSTALL_ROOT="$ROOT"

# Everything this script creates stays INSIDE the launcher's directory.
OUT=$(mktemp -d "$ROOT/PR51-diagnostics-XXXXXXXX")
WORK="$OUT/.temporary-wine-prefixes"
mkdir -p "$WORK" "$OUT/pre-test-backup"
cleanup() { if [[ -d "$WORK" ]]; then rm -rf -- "$WORK"; fi; }
trap cleanup EXIT
for item in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  if [[ -f "$ROOT/$item" ]]; then cp -p -- "$ROOT/$item" "$OUT/pre-test-backup/"; fi
done
echo "PR51 diagnostics directory: $OUT"
echo "Using the existing dedicated test installation, without copying game files."
echo "WARNING: the launcher may update INI, fix.dll, logs, and game state."
echo "INI and root logs are backed up; game files and saved games are not."


{
  printf 'Source installation: %s\n' "$ROOT"
  printf 'Installation mode: dedicated in-place test\n'
  printf 'Launcher: %s\n' "$LAUNCHER_NAME"
  printf 'Build SHA256: '
  sha256sum "$LAUNCHER"
  printf 'Host: '
  uname -a
  printf 'Wine binary: '
  command -v wine
  wine --version
  printf 'WINEARCH (inherited): %s\n' "$(printenv WINEARCH || true)"
  printf 'Wine prefix test type: TWO new separate disposable prefixes\n'
  printf 'All tests are run with the same Wine executable.\n'
  printf 'Only the second has WINEDLLOVERRIDES=amstream=b;quartz=b.\n'
} > "$OUT/environment.txt" 2>&1

# Probe the original media files without copying or modifying them.
for movie in MSOPEN1.MPG CHAP0.MPG; do
  path=$(find "$ROOT/game" -type f -iname "$movie" -print -quit)
  if [[ -z "$path" ]]; then
    printf '%s: not found\n' "$movie" >> "$OUT/streams.txt"
    continue
  fi
  printf '\n%s\n' "$path" >> "$OUT/streams.txt"
  sha256sum "$path" >> "$OUT/streams.txt"
  if command -v ffprobe >/dev/null 2>&1; then
    ffprobe -v error -show_entries stream=index,codec_type,codec_name,profile \
      -of compact "$path" >> "$OUT/streams.txt" 2>&1 || true
  else
    echo "ffprobe not available; codec detection skipped" >> "$OUT/streams.txt"
  fi
done

run_case() {
  local label="$1" pfx="$WORK/prefix-$1" exit_code=0
  mkdir -p "$pfx"
  echo "Initializing isolated Wine prefix for $label..."
  if ! env -u WINEDLLOVERRIDES WINEPREFIX="$pfx" WINEDEBUG=-all \
      wineboot -u > "$OUT/wineboot-$label.log" 2>&1; then
    echo "Wine prefix creation failed; inspect $OUT/wineboot-$label.log" >&2
    return 1
  fi
  echo "Launching $label. Observe opening movie (picture AND audio), then exit the game."
  if [[ "$label" == builtin ]]; then
    (cd "$INSTALL_ROOT" &&
      env WINEPREFIX="$pfx" WINEDLLOVERRIDES='amstream=b;quartz=b' \
      WINEDEBUG=+loaddll,+ole wine "./$LAUNCHER_NAME") \
      > "$OUT/wine-$label.log" 2>&1 || exit_code="$?"
  else
    (cd "$INSTALL_ROOT" &&
      env -u WINEDLLOVERRIDES WINEPREFIX="$pfx" \
      WINEDEBUG=+loaddll,+ole wine "./$LAUNCHER_NAME") \
      > "$OUT/wine-$label.log" 2>&1 || exit_code="$?"
  fi
  printf '\nCase=%s\nExitCode=%s\n' "$label" "$exit_code" >> "$OUT/observations.txt"
  local observation=""
  read -r -p "For $label: opening video (picture/audio/black/frozen/skipped)? " observation || true
  printf 'OpeningVideo=%s\n' "$observation" >> "$OUT/observations.txt"
  read -r -p "For $label: chapter movie and menu return (if tested)? " observation || true
  printf 'ChapterAndReturn=%s\n' "$observation" >> "$OUT/observations.txt"
  for log in CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
    if [[ -f "$INSTALL_ROOT/$log" ]]; then
      cp -- "$INSTALL_ROOT/$log" "$OUT/$label-$log"
    fi
  done
  gzip -f -1 -- "$OUT/wine-$label.log"
}

echo "Two interactive tests will follow; each gets its own NEW Wine prefix."
echo "No native Microsoft DLLs or Winetricks verbs will be installed."
run_case default
run_case builtin
cleanup
ARCHIVE="$ROOT/$(basename "$OUT").tar.gz"
tar -czf "$ARCHIVE" -C "$ROOT" "$(basename "$OUT")"
echo "Results saved under $OUT"
echo "Archive to share: $ARCHIVE"
echo "Share the archive after checking its logs for private local paths."
echo "Logs may contain local paths or usernames; review them before sharing."
