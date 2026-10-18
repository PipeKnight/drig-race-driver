#!/bin/sh
# Solo-AI survey of every shipped route: grid-opponent-race --route R ROOT CD01 BP01 1 TICKS ai.
# Usage: tools/route_survey.sh EXE OUT_DIR [TICKS] [ROUTE_GLOB]
# Reads GRID_GAME_ROOT (default: Steam path), GRID_CONTACT_DIRECTIONS, GRID_BODY_EVENT_PARAMETERS.
exe=$1; out=$2; ticks=${3:-16000}; glob=${4:-'*/*/route_*'}
root=${GRID_GAME_ROOT:-"C:/Program Files (x86)/Steam/steamapps/common/Grid"}
mkdir -p "$out"
for d in "$root"/tracks/$glob; do
  route=${d#"$root"/}
  case $route in tracks/frontend/*) continue;; esac
  name=$(echo "${route#tracks/}" | tr / _)
  "$exe" --route "$route" "$root" "$GRID_CONTACT_DIRECTIONS" "$GRID_BODY_EVENT_PARAMETERS" 1 "$ticks" ai > "$out/$name.txt" 2>&1
  echo "$name exit=$? $(grep -E '^grid-opponent-race:|lap|retired' "$out/$name.txt" | tail -1 | cut -c1-140)"
done
