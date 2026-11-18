#!/usr/bin/env bash
# PR #51: isolated Wine builtins comparison for an already installed game.
# No Winetricks, DLL registration, prefix changes outside temporary directories.
set -Eeuo pipefail

usage() {
  echo "Usage: bash pr51-wine-video-diagnostics.sh [--in-place] /path/to/installed/crimsonskiesfix"
  echo "Use an installation containing CrimsonSkiesFix.exe and game/CRIMSON.EXE."
  exit 2
}
MODE=copy
if [[ "$#" -ge 1 && "$1" == "--in-place" ]]; then
  MODE=in-place
  shift
fi
[[ "$#" -eq 1 ]] || usage
for cmd in wine wineboot realpath mktemp cp find gzip sha256sum; do
  command -v "$cmd" >/dev/null || { echo "Missing command: $cmd" >&2; exit 2; }
done
ROOT=$(realpath -e -- "$1")
[[ -f "$ROOT/CrimsonSkiesFix.exe" && -f "$ROOT/game/CRIMSON.EXE" ]] || {
  echo "Expected CrimsonSkiesFix.exe and game/CRIMSON.EXE under $ROOT" >&2
  exit 2
}

# Reports are a sibling of the original installation, never underneath it.
OUT=$(mktemp -d "$ROOT/../CrimsonSkiesFix-PR51-XXXXXX")
WORK=$(mktemp -d)
cleanup() { if [[ -n "$WORK" && -d "$WORK" ]]; then rm -rf -- "$WORK"; fi; }
trap cleanup EXIT
echo "PR51 diagnostics: $OUT"
if [[ "$MODE" == in-place ]]; then
  INSTALL_ROOT="$ROOT"
  mkdir -p "$OUT/pre-test-backup"
  for item in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
    if [[ -f "$ROOT/$item" ]]; then cp -p -- "$ROOT/$item" "$OUT/pre-test-backup/"; fi
  done
  echo "IN-PLACE MODE: using the existing dedicated installation (no game copy)."
  echo "WARNING: the launcher may update INI, runtime, logs, game data or saved games."
  echo "Existing INI and logs were backed up, but game data and saves were not."
  echo "Use a dedicated test installation only; do not change saves during the test."
else
  INSTALL_ROOT="$WORK/install"
  mkdir -p "$INSTALL_ROOT"
  echo "Preparing a PRIVATE TEST COPY of the installed game; source remains unchanged."
  cp -a -- "$ROOT/CrimsonSkiesFix.exe" "$INSTALL_ROOT/"
  cp -a -- "$ROOT/game" "$INSTALL_ROOT/"
  for item in CrimsonSkiesFix.ini state docs; do
    if [[ -e "$ROOT/$item" ]]; then cp -a -- "$ROOT/$item" "$INSTALL_ROOT/"; fi
  done
fi

{
  printf 'Source installation: %s\n' "$ROOT"
  printf 'Installation mode: %s\n' "$MODE"
  printf 'Build SHA256: '
  sha256sum "$ROOT/CrimsonSkiesFix.exe"
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
      WINEDEBUG=+loaddll,+ole wine ./CrimsonSkiesFix.exe) \
      > "$OUT/wine-$label.log" 2>&1 || exit_code="$?"
  else
    (cd "$INSTALL_ROOT" &&
      env -u WINEDLLOVERRIDES WINEPREFIX="$pfx" \
      WINEDEBUG=+loaddll,+ole wine ./CrimsonSkiesFix.exe) \
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
echo "Results saved under $OUT"
echo "Share environment.txt, streams.txt, observations.txt and the two wine-*.log.gz files."
echo "Logs may contain local paths or usernames; review them before sharing."
