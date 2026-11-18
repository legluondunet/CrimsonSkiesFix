#!/usr/bin/env bash
# PR #51: non-destructive 32-bit GStreamer plugin and SONAME audit (Manjaro).
# All output stays beside this script, in the installer directory.
set -Eeuo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
for cmd in mktemp tar date uname; do
  command -v "$cmd" >/dev/null || { echo "Missing command: $cmd" >&2; exit 2; }
done
OUT=$(mktemp -d "$ROOT/PR51-gstreamer-abi-XXXXXXXX")
echo "Gathering 32-bit GStreamer dependency information (read-only)..."

{
  printf 'AuditUTC=%s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  uname -a
  printf 'GST_PLUGIN_PATH=%s\n' "${GST_PLUGIN_PATH:-unset}"
  printf 'GST_PLUGIN_SYSTEM_PATH=%s\n' "${GST_PLUGIN_SYSTEM_PATH:-unset}"
  printf 'LD_LIBRARY_PATH=%s\n' "${LD_LIBRARY_PATH:-unset}"
  if command -v pacman >/dev/null 2>&1; then
    echo 'Relevant installed packages:'
    for package in lib32-gstreamer lib32-gst-plugins-base \
      lib32-gst-plugins-good lib32-gst-plugins-bad lib32-gst-libav \
      lib32-nettle lib32-libvpx gstreamer gst-plugins-good nettle libvpx; do
      pacman -Q "$package" 2>/dev/null || printf '%s: not installed\n' "$package"
    done
  else
    echo 'pacman is unavailable'
  fi
} > "$OUT/environment-and-packages.txt" 2>&1

{
  echo 'Present /usr/lib32 library SONAMEs:'
  for lib in /usr/lib32/libnettle.so* /usr/lib32/libvpx.so*; do
    [[ -e "$lib" || -L "$lib" ]] || continue
    ls -ld -- "$lib" || true
  done
  echo
  if command -v ldconfig >/dev/null 2>&1; then
    echo 'Relevant dynamic linker cache:'
    ldconfig -p 2>/dev/null | grep -E 'lib(nettle|vpx)\.so' || true
  fi
} > "$OUT/installed-sonames.txt" 2>&1

: > "$OUT/summary.txt"
missing=0
for plugin in libgstadaptivedemux2.so libgstvpx.so; do
  target="/usr/lib32/gstreamer-1.0/$plugin"
  report="$OUT/$plugin.txt"
  if [[ ! -f "$target" ]]; then
    printf '%s: plugin file absent\n' "$plugin" | tee -a "$OUT/summary.txt"
    continue
  fi
  {
    printf 'Plugin=%s\n' "$target"
    command -v file >/dev/null 2>&1 && file "$target"
    if command -v pacman >/dev/null 2>&1; then pacman -Qo "$target" || true; fi
    if command -v readelf >/dev/null 2>&1; then
      echo 'ELF NEEDED:'
      readelf -d "$target" | grep NEEDED || true
    fi
    echo 'Dynamic loader dependencies:'
    if command -v ldd >/dev/null 2>&1; then
      ldd "$target" 2>&1 || true
    else
      echo 'ldd unavailable'
    fi
  } > "$report" 2>&1
  if grep -q '=> not found' "$report"; then
    printf '%s: at least one ELF dependency unresolved\n' "$plugin" | tee -a "$OUT/summary.txt"
    missing=$((missing+1))
  else
    printf '%s: no unresolved ELF dependency detected\n' "$plugin" | tee -a "$OUT/summary.txt"
  fi
done
{
  printf 'PluginsWithUnresolvedDependencies=%s\n' "$missing"
  echo 'No package installation, system configuration change, Wine launch, or prefix write was performed.'
  echo 'Do not create symlinks between different library SONAME versions.'
} >> "$OUT/summary.txt"
ARCHIVE="$ROOT/$(basename "$OUT").tar.gz"
tar -czf "$ARCHIVE" -C "$ROOT" "$(basename "$OUT")"
echo "Archive ready: $ARCHIVE"
echo "Check the report for personal filesystem paths before sharing."
