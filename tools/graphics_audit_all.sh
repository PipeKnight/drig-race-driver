#!/bin/sh
# Runs `grid-app --graphics-audit` (docs/GRAPHICS_AUDIT.md) over every route
# folder with a track.jpk, or the routes given after OUT: one folder per route
# (materials.tsv, frames.tsv, flag-NNN.png, sheet.png, run.out) and one
# summary line per route in OUT/summary.txt (default OUT local/graphics-audit).
# Usage: [GRID_EXE=path] sh tools/graphics_audit_all.sh [OUT [POINTS [ROUTE...]]]
#   e.g. sh tools/graphics_audit_all.sh local/ga 24 tracks/usa/washington/route_0
G="C:/Program Files (x86)/Steam/steamapps/common/Grid"
OUT="${1:-local/graphics-audit}"
POINTS="${2:-24}"
EXE="${GRID_EXE:-target/release/grid-app.exe}"
[ $# -gt 2 ] && shift 2 || set --
mkdir -p "$OUT"
: > "$OUT/summary.txt"
if [ $# -eq 0 ]; then
  for d in "$G"/tracks/*/*/route_*; do
    case "$d" in */tracks/frontend/*) continue ;; esac  # menu garages, not race routes
    [ -f "$d/track.jpk" ] && set -- "$@" "${d#"$G/"}"
  done
fi
for r in "$@"; do
  n=$(echo "$r" | sed 's#tracks/##; s#/#-#g')
  mkdir -p "$OUT/$n"
  rm -f "$OUT/$n/final.png"
  GRID_AUDIO=off timeout 1500 "$EXE" --route "$r" --scene reference \
    --resolution 1280x720 --graphics-audit "$OUT/$n" --graphics-audit-points "$POINTS" \
    --render "$OUT/$n/final.png" > "$OUT/$n/run.out" 2>&1
  code=$?
  echo "$r exit=$code $(grep -a 'graphics_audit_materials' "$OUT/$n/run.out" | tail -1 | sed 's/ file=.*//') $(grep -a 'graphics_audit verdict\|^grid-app:\|ERROR' "$OUT/$n/run.out" | tail -1 | sed 's/ directory=.*//')" >> "$OUT/summary.txt"
done
echo "graphics_audit_all routes=$(wc -l < "$OUT/summary.txt") flagged=$(grep -c 'verdict=FLAG' "$OUT/summary.txt") errors=$(grep -vc 'verdict=PASS\|verdict=FLAG' "$OUT/summary.txt") summary=$OUT/summary.txt"
