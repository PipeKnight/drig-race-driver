#!/bin/sh
# Night-light A/B and driving smoke (docs/NIGHT_LIGHTS.md): renders each route with GRID_NIGHT_LIGHTS=off and on at hour 0, from the same 12 audit cameras and as a scripted windowed drive (the interactive branch of night_lights::present), then diffs them with image_ab_compare.ps1.
# Usage: [GRID_EXE=path] sh tools/night_lights_ab.sh [OUT [ROUTE...]]   default OUT local/night-ab, routes Haruna r1 and Le Mans r0.
# Verdict: one line per run plus "night_lights_ab fail=N"; a forced-night drive must print "night_lights shown ... driving=true".
EXE="${GRID_EXE:-target/release/grid-app.exe}"
OUT="${1:-local/night-ab}"
[ $# -gt 1 ] && shift || set -- tracks/japan/haruna/route_1 tracks/europe/le_mans/route_0
fail=0
for r in "$@"; do
  n=$(echo "$r" | sed 's#tracks/##; s#/#-#g')
  for kind in static drive; do
    for v in off on; do
      d=$OUT/$kind-$n/$v; mkdir -p "$d"
      if [ $kind = static ]; then
        GRID_NIGHT_LIGHTS=$v GRID_SKY_HOUR=0 GRID_GRAPHICS_AUDIT_FRAMES=all GRID_AUDIO=off timeout 1500 "$EXE" --route "$r" --scene reference \
          --resolution 1280x720 --graphics-audit "$d" --graphics-audit-points 12 --render "$d/final.png" > "$d/run.out" 2>&1
      else
        GRID_NIGHT_LIGHTS=$v GRID_SKY_HOUR=0 GRID_AUDIO=off timeout 900 "$EXE" --route "$r" --drive --drive-ticks 240 --throttle 0.5 \
          --resolution 1280x720 --render "$d/final.png" > "$d/run.out" 2>&1
      fi
      code=$?
      bad=$(grep -ac 'panicked\|Caught rendering error' "$d/run.out")
      [ $code -ne 0 ] || [ "$bad" -ne 0 ] && fail=$((fail + 1))
      if [ $kind = drive ] && [ $v = on ] && grep -aq 'mode=ForcedNight' "$d/run.out" && ! grep -aq 'driving=true' "$d/run.out"; then fail=$((fail + 1)); fi
      echo "$kind $n $v exit=$code errors=$bad $(grep -a 'night_lights \(version\|shown\)' "$d/run.out" | tr '\n' ' ' | cut -c1-200)"
      cp "$d/final.png" "$d/frame-final.png" 2>/dev/null
    done
    powershell -NoProfile -ExecutionPolicy Bypass -File tools/image_ab_compare.ps1 -Off "$(cygpath -w "$OUT/$kind-$n/off")" \
      -On "$(cygpath -w "$OUT/$kind-$n/on")" -Out "$(cygpath -w "$OUT/$kind-$n/cmp")" | tail -1
  done
done
echo "night_lights_ab fail=$fail out=$OUT"
