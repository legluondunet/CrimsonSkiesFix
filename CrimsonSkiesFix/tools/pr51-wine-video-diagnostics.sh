#!/usr/bin/env bash
# PR #51: isolated Wine builtins comparison for an already installed game.
# No Winetricks, DLL registration, prefix changes outside temporary directories.
set -Eeuo pipefail

usage() {
  echo "Usage: bash pr51-wine-video-diagnostics.sh /path/to/installed/crimsonskiesfix"
  echo "Use an installation containing CrimsonSkiesFix.exe and game/CRIMSON.EXE."
  exit 2
}
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
mkdir -p "$WORK/install"
echo "PR51 diagnostics: $OUT"
echo "Preparing a PRIVATE TEST COPY of the installed game; source remains unchanged."
cp -a -- "$ROOT/CrimsonSkiesFix.exe" "$WORK/install/"
cp -a -- "$ROOT/game" "$WORK/install/"
for item in CrimsonSkiesFix.ini state docs; do
  if [[ -e "$ROOT/$item" ]]; then cp -a -- "$ROOT/$item" "$WORK/install/"; fi
done

{
  printf 'Source installation: %s\n' "$ROOT"
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
    (cd "$WORK/install" &&
      env WINEPREFIX="$pfx" WINEDLLOVERRIDES='amstream=b;quartz=b' \
      WINEDEBUG=+loaddll,+ole wine ./CrimsonSkiesFix.exe) \
      > "$OUT/wine-$label.log" 2>&1 || exit_code="$?"
  else
    (cd "$WORK/install" &&
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
    if [[ -f "$WORK/install/$log" ]]; then
      cp -- "$WORK/install/$log" "$OUT/$label-$log"
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
