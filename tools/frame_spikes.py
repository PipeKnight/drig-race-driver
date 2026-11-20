"""Classify the slow 1-second frame_stats windows of fps-log runs: render- or main-gated, and what the worst frame spent.

Usage: py -I tools/frame_spikes.py [--limit 12] [--skip 3] GLOB [GLOB ...]
Each GLOB names run folders that hold a stdout.log with frame_stats lines (from grid-app with the fps log, for
example local/frame-split-smoke/fps-don-2560-enh-b-*). Windows count from the countdown's GO line; the first
--skip windows after it are left out. A window is slow when its max_ms (the longest frame interval) exceeds
--limit. It is render-gated when the worst render-thread frame (rworst) is within 1.5 ms of max_ms, otherwise
main-gated. Prints one verdict block per GLOB: slow windows by gate and phase, the render stage that dominates
the worst render frame, the mean main-thread split of the slow main frames (slow10_*) in main-gated windows,
and per lap how many windows had a single frame over 3 ms (docs/PERFORMANCE.md "Spikes after the countdown").
"""
import argparse
import collections
import glob
import os

STAGES = 'meshes views acquire queue sort prepare bind encode submit present cleanup'.split()
LAPS = 'dt dp dpo dpn gd gdg tp cb out mt'.split()


def windows(path, skip):
    go = None
    with open(path, encoding='utf-8', errors='replace', newline='') as f:
        for line in f:
            if 'remaining=GO' in line:
                go = 0
            if go is None or not line.startswith('frame_stats'):
                continue
            go += 1
            fields = dict(kv.split('=', 1) for kv in line.split()[1:] if '=' in kv)
            if go > skip and 'rworst' in fields and 'lap_dt' in fields:
                yield go, fields


def classify(pattern, limit, skip):
    runs = sorted(glob.glob(os.path.join(pattern, 'stdout.log')))
    total = 0
    gate, phase, stage = collections.Counter(), collections.Counter(), collections.Counter()
    main_split = collections.defaultdict(float)
    main_n = 0
    lap_over = collections.Counter()
    for path in runs:
        for go, f in windows(path, skip):
            total += 1
            for lap in LAPS:
                lap_over[lap] += float(f['lap_' + lap].split('/')[1]) > 3.0
            worst = [float(v) for v in f['rworst'].split('/')]
            longest = float(f['max_ms'])
            if longest <= limit:
                continue
            kind = 'render' if longest - worst[0] <= 1.5 else 'main'
            gate[kind] += 1
            phase[(kind, 'early' if go <= 17 else 'late')] += 1
            stage[max(zip(worst[1:], STAGES))[1]] += 1
            if kind == 'main' and int(f['slow10_frames']) > 0:
                main_n += 1
                for key in ('main', 'fixed', 'outside', 'ticks'):
                    main_split[key] += float(f['slow10_' + key])
                for lap, v in zip(LAPS, f['slow10_laps'].split('/')):
                    main_split[lap] += float(v)
    print(f"{pattern}: {len(runs)} runs, {total} windows after GO+{skip}, slow (max_ms > {limit}) {sum(gate.values())}: "
          f"render-gated {gate['render']}, main-gated {gate['main']}")
    print('  by phase (early = up to GO+17 windows): ' + ', '.join(f'{k}-{p} {n}' for (k, p), n in sorted(phase.items())))
    print('  dominant stage of the worst render frame: ' + ', '.join(f'{k} {n}' for k, n in stage.most_common()))
    if main_n:
        m = {k: v / main_n for k, v in main_split.items()}
        print(f"  main-gated, slow main frames (mean of {main_n} windows): main {m['main']:.2f} fixed {m['fixed']:.2f} "
              f"outside {m['outside']:.2f} ticks {m['ticks']:.2f} | laps " + ' '.join(f'{k} {m[k]:.1f}' for k in LAPS if m[k] >= 0.3))
    print('  windows with a lap frame over 3 ms: ' + ' '.join(f'{k} {lap_over[k]}' for k in LAPS if lap_over[k]))


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--limit', type=float, default=12.0)
    p.add_argument('--skip', type=int, default=3)
    p.add_argument('globs', nargs='+')
    a = p.parse_args()
    for pattern in a.globs:
        classify(pattern, a.limit, a.skip)


if __name__ == '__main__':
    main()
