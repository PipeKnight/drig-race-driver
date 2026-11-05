"""Classify one-frame sun shadow caster dropouts (`GRID_SHADOW_TRACE=1`, shadow_cull.rs) by cause and match them with blink_detect events (row 115).

Usage: py -I tools/shadow_dropouts.py GRID_APP_LOG [BLINK_VERDICT] [--after-frame 200]

Reads the `shadow_dropout frame=N cascade=I cause=C entity=E name=... distance=D centre=...` lines
of a window run (frames after --after-frame, past the pipeline warm-up) and prints, per cause, the
dropout count, the frames, the cascades, the distance range and one example caster. The causes are
the first test in Bevy's `check_dir_light_mesh_visibility` that left the caster out (`not_caster`,
`hidden`, `layers`, `range`, `frustum`), `port_cull` when only shadow_cull's slice test did, with
`+moved` / `+bounds` when its transform or box changed in that frame.

With BLINK_VERDICT (the output of tools/blink_detect.py on the same run's frames) it also counts,
per cause, the dropout frames that have a detector event on the same frame, split into scenery
events (above the detector's --upper-rows) and the rest. Ends with a one-line verdict.
"""
import argparse
import re
from collections import defaultdict


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('log')
    ap.add_argument('verdict', nargs='?')
    ap.add_argument('--after-frame', type=int, default=200)
    ap.add_argument('--upper-rows', type=int, default=11)
    args = ap.parse_args()
    line = re.compile(r'^shadow_dropout frame=(\d+) cascade=(\d+) cause=(\S+) entity=(\S+) name="([^"]*)" '
                      r'distance=([\d.]+)')
    by_cause = defaultdict(list)
    for text in open(args.log, errors='replace'):
        m = line.match(text)
        if m and int(m[1]) > args.after_frame:
            by_cause[m[3]].append((int(m[1]), int(m[2]), m[4], m[5], float(m[6])))
    events = defaultdict(lambda: [0, 0])  # frame -> [scenery, lower]
    if args.verdict:
        ev = re.compile(r'frame (\d+) t=[\d.]+s len=(\d) tiles=\d+ box=\((\d+), (\d+), (\d+), (\d+)\)')
        for text in open(args.verdict, errors='replace'):
            m = ev.search(text)
            if m:
                for f in range(int(m[1]), int(m[1]) + int(m[2])):
                    events[f][0 if int(m[6]) < args.upper_rows else 1] += 1
    total = sum(len(v) for v in by_cause.values())
    print(f'{total} dropouts after frame {args.after_frame} in {args.log}')
    on_scenery = 0
    for why, rows in sorted(by_cause.items(), key=lambda kv: -len(kv[1])):
        frames = sorted({r[0] for r in rows})
        cascades = sorted({r[1] for r in rows})
        dist = [r[4] for r in rows]
        example = rows[0]
        line_ = (f'  {why}: {len(rows)} in {len(frames)} frames, cascades {cascades}, '
                 f'distance {min(dist):.0f}..{max(dist):.0f} m, e.g. frame {example[0]} {example[3] or example[2]}')
        if args.verdict:
            scenery = sum(1 for f in frames if events[f][0])
            lower = sum(1 for f in frames if events[f][1] and not events[f][0])
            on_scenery += scenery
            line_ += f'; frames with a detector event: {scenery} scenery, {lower} lower'
        print(line_)
    if args.verdict:
        print(f'{"FAIL" if on_scenery else "PASS"}: {on_scenery} dropout frames meet a scenery blink event')


if __name__ == '__main__':
    main()
