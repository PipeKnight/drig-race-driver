#!/bin/sh
# Runs `grid-app --float-audit` (backlog row 102, docs/FLOATING_AUDIT.md) over
# every route folder with a track.jpk; one TSV and log per route plus a
# summary line each, in OUT (default local/float-audit).
# Usage: sh tools/float_audit_all.sh [OUT]
G="C:/Program Files (x86)/Steam/steamapps/common/Grid"
OUT="${1:-local/float-audit}"
mkdir -p "$OUT"
: > "$OUT/summary.txt"
for d in "$G"/tracks/*/*/route_*; do
  [ -f "$d/track.jpk" ] || continue
  r=${d#"$G/"}
  n=$(echo "$r" | sed 's#tracks/##; s#/#-#g')
  timeout 600 target/release/grid-app.exe --route "$r" --scene donington \
    --float-audit "$OUT/$n.tsv" > "$OUT/$n.out" 2>&1
  echo "$r exit=$? $(grep -a 'float_audit\|^grid-app:' "$OUT/$n.out" | tail -1)" >> "$OUT/summary.txt"
done
