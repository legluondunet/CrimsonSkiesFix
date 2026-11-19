#!/usr/bin/env bash
# PR #51 — autonomous A/B test, Manjaro Wine-staging 11.19.
# Run from the directory containing CrimsonSkiesFix.exe:
# curl -fL URL -o ./pr51-crimflag-wine1119-ab.sh && bash ./pr51-crimflag-wine1119-ab.sh
# All reports, INI backups, temporary Wine prefixes and archive stay in PWD.
# Tests a DEDICATED GAME COPY; the installer may update game/fix.dll.
set -Eeuo pipefail

die() { printf 'PR51 ERROR: %s\n' "$*" >&2; exit 2; }
ROOT="$(pwd -P)"
EXE="$ROOT/CrimsonSkiesFix.exe"
INI="$ROOT/CrimsonSkiesFix.ini"
[[ -f "$EXE" && ! -L "$EXE" ]] ||
  die "Ouvre un terminal dans le dossier contenant CrimsonSkiesFix.exe."
[[ -f "$ROOT/game/CRIMSON.EXE" && ! -L "$ROOT/game/CRIMSON.EXE" ]] ||
  die "L'installation de test doit contenir game/CRIMSON.EXE."
[[ ! -e "$INI" || ( -f "$INI" && ! -L "$INI" ) ]] ||
  die "INI non régulier ou lien symbolique : refus de le modifier."
for cmd in wine wineboot wineserver awk mktemp cp gzip tar sha256sum grep rm mkdir; do
  command -v "$cmd" >/dev/null 2>&1 || die "Commande manquante : $cmd"
done
VERSION="$(wine --version)"
[[ "$VERSION" == *11.19* ]] ||
  die "Ce test comparatif cible Wine-staging 11.19, version trouvée : $VERSION"
# The probe string is embedded in the experimental unified EXE as fix.dll
# source text. Fail rather than silently testing a non-prototype launcher.
LC_ALL=C grep -aFq 'PR51-CRIMFLAG:' "$EXE" ||
  die "Ce CrimsonSkiesFix.exe n'inclut pas le prototype CRIMFLAG. Utilise le ZIP expérimental de la PR #51."

SESSION="$(mktemp -d "$ROOT/PR51-crimflag-XXXXXXXX")"
mkdir -p "$SESSION/original" "$SESSION/tmp" "$SESSION/cache"
export TMPDIR="$SESSION/tmp"
export XDG_CACHE_HOME="$SESSION/cache"
ACTIVE_PREFIX=""
declare -A PRESENT=()
for file in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
  if [[ -e "$ROOT/$file" ]]; then
    [[ -f "$ROOT/$file" && ! -L "$ROOT/$file" ]] ||
      die "Fichier existant non régulier : $file"
    PRESENT["$file"]=1
    cp -p -- "$ROOT/$file" "$SESSION/original/$file"
  else
    PRESENT["$file"]=0
  fi
done

restore_root_files(){
  for file in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
    if [[ "${PRESENT[$file]:-0}" == 1 ]]; then
      cp -p -- "$SESSION/original/$file" "$ROOT/$file" ||
        printf 'ATTENTION : restauration impossible de %s\n' "$file" >&2
    else
      [[ ! -e "$ROOT/$file" || -f "$ROOT/$file" ]] &&
        rm -f -- "$ROOT/$file"
    fi
  done
}
cleanup(){
  local status=$?
  trap - EXIT INT TERM
  set +e
  if [[ -n "$ACTIVE_PREFIX" && -d "$ACTIVE_PREFIX" ]]; then
    WINEPREFIX="$ACTIVE_PREFIX" WINEDEBUG=-all wineserver -k >/dev/null 2>&1
  fi
  restore_root_files
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

{
  printf 'WineVersion=%s\n' "$VERSION"
  printf 'WineBinary=%s\n' "$(command -v wine)"
  printf 'WinebootBinary=%s\n' "$(command -v wineboot)"
  printf 'WineserverBinary=%s\n' "$(command -v wineserver)"
  printf 'LauncherSHA256='
  sha256sum "$EXE"
  printf 'ModeOrder=off,video\n'
  printf 'WINEDLLOVERRIDES=amstream=b;quartz=b\n'
  printf 'NoWinetricks=yes\n'
  printf 'CurrentDirectory=%s\n' "$ROOT"
} > "$SESSION/environment.txt"

configure_mode(){
  local mode="$1"
  # Use the ORIGINAL INI for each run. Remove existing Debug/Log and
  # Debug/CrimFlagVideoTest keys in a temporary copy to avoid duplicates.
  # The original INI is restored byte-for-byte on any normal/error exit.
  if [[ "${PRESENT[CrimsonSkiesFix.ini]}" == 1 ]]; then
    awk '
      function trim(x) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", x); return x }
      /^\[[^]]+\]/ {
        section=tolower(trim($0)); gsub(/\r/, "", section);
        in_debug=(section=="[debug]");
      }
      {
        check=$0; sub(/\r$/, "", check);
        if (in_debug && check ~ /^[[:space:]]*(log|Log|LOG|[cC]rim[fF]lag[vV]ideo[tT]est)[[:space:]]*=/) next;
        print $0;
      }
    ' "$SESSION/original/CrimsonSkiesFix.ini" > "$INI"
  else
    : > "$INI"
  fi
  printf '\n[Debug]\nLog=1\nCrimFlagVideoTest=%s\n' "$mode" >> "$INI"
}

run_mode(){
  local mode="$1" prefix="$SESSION/prefix-$1" launch_pid exit_code=0 note=""
  restore_root_files
  configure_mode "$mode"
  mkdir -p "$prefix"
  ACTIVE_PREFIX="$prefix"
  echo
  echo "=== Test Wine 11.19 : CrimFlagVideoTest=$mode ==="
  echo "Création d'un préfixe Wine indépendant dans le répertoire actuel..."
  if ! env -u WINEDLLOVERRIDES WINEPREFIX="$prefix" WINEDEBUG=-all \
      wineboot -u > "$SESSION/wineboot-$mode.log" 2>&1; then
    die "wineboot a échoué ; consulter $SESSION/wineboot-$mode.log"
  fi
  (
    cd "$ROOT"
    env WINEPREFIX="$prefix" WINEDLLOVERRIDES='amstream=b;quartz=b' \
      WINEDEBUG=-all wine ./CrimsonSkiesFix.exe
  ) > "$SESSION/wine-$mode.log" 2>&1 &
  launch_pid=$!
  echo "Observe les deux introductions, puis le menu principal."
  echo "Reviens au terminal après l'observation ; le jeu sera arrêté par le script."
  read -r -p "Résultat $mode (vidéos, menu, boutons, musique) : " note || true
  printf 'Mode=%s\nObservation=%s\n' "$mode" "$note" >> "$SESSION/observations.txt"
  # Kill ONLY this disposable prefix, never any user's other Wine game.
  env WINEPREFIX="$prefix" WINEDEBUG=-all wineserver -k \
    > "$SESSION/wineserver-$mode.log" 2>&1 || true
  wait "$launch_pid" || exit_code=$?
  printf 'Mode=%s\nLauncherExitCodeAfterStop=%s\n' "$mode" "$exit_code" \
    >> "$SESSION/observations.txt"
  for file in CrimsonSkiesFix.ini CrimsonSkiesFix.log CrimsonSkiesFix_installer.log; do
    [[ ! -f "$ROOT/$file" ]] ||
      cp -p -- "$ROOT/$file" "$SESSION/$mode-$file"
  done
  if [[ -f "$SESSION/$mode-CrimsonSkiesFix.log" ]]; then
    grep -E 'PR51-CRIMFLAG:|MENU-SCENE:|PR51-MENU-FLIP|PR51-MENU-UNLOCK' \
      "$SESSION/$mode-CrimsonSkiesFix.log" > "$SESSION/$mode-key-events.txt" || true
  fi
  gzip -1 -- "$SESSION/wine-$mode.log"
  rm -rf -- "$prefix"
  ACTIVE_PREFIX=""
  echo "Logs capturés pour le mode $mode."
}
run_mode off
run_mode video

restore_root_files
ARCHIVE="$ROOT/$(basename "$SESSION").tar.gz"
tar -czf "$ARCHIVE" -C "$ROOT" "$(basename "$SESSION")"
echo
echo "Fin des deux tests. L'INI et les anciens logs ont été restaurés."
echo "Archive à transmettre : $ARCHIVE"
echo "Vérifie les éventuels chemins personnels dans les journaux avant l'envoi."
