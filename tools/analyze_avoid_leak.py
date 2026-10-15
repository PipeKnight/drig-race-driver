"""Bin the avoid-exclusion leak of a trace_grid_ai_avoid_recovery.ps1 trace by gate angle.

Usage: python -I tools/analyze_avoid_leak.py local/ai-avoid-recovery/live/trace-race-03.tsv

Considers AI drivers (drv != 0) whose path obstacle has ETA < 2 s and which the
emulated racing-normally exclusion 0x79A530 marks excluded. Prints how often avoid
fired anyway, binned by the deviation of the target's horizontal heading from the
gate direction t (pi - ob_angle). See docs/AI_AVOID_RECOVERY_LIVE.md "The 0.36% leak".
"""
import collections
import csv
import math
import sys


def num(row, key):
    try:
        return float(row[key])
    except (KeyError, TypeError, ValueError):
        return math.nan


rows = list(csv.DictReader(open(sys.argv[1], encoding='utf-8-sig'), delimiter='\t'))
near = [r for r in rows if r['drv'] != '0' and num(r, 'ob_eta') < 2 and r['ob_excluded'] == '1']
fired = [r for r in near if r['av_fired'] not in ('', '0')]
print(f'excluded near frames {len(near)}, fired {len(fired)}')
bins = collections.defaultdict(lambda: [0, 0])
for r in near:
    k = min(int((math.pi - num(r, 'ob_angle')) / 1e-4), 20)
    bins[k][0] += 1
    bins[k][1] += r['av_fired'] not in ('', '0')
for k in sorted(bins):
    label = f'{k * 1e-4:.1e}..{(k + 1) * 1e-4:.1e}' if k < 20 else '>= 2.0e-03'
    n, f = bins[k]
    print(f'{label} rad  frames {n:6}  fired {f:3}  rate {f / n:.2f}')
