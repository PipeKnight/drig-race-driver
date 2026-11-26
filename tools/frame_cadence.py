"""Split a grid-app fps log's frame time into GPU, main thread, render CPU and drawable wait, with the present cadence.

Usage: py -I tools/frame_cadence.py LOG...
LOG is a grid-app stdout log with the fps log and GPU timing on (frame_stats with gpu_ms, frame_gpu_views).
Per phase (countdown, race per camera view, end) it prints the median fps, frame, main, render CPU
(render_ms minus acquire), acquire, GPU and GPU phases; for the race it bins the interval histogram on the
240 Hz grid (k x 4.17 ms) and gives the share on the 120 Hz grid, the per-view GPU phases, and for windows
with a frame over 33 ms the worst frame's acquire share (docs/PERFORMANCE.md "Refresh pacing (F964)").
"""
import re, sys, statistics as st
from collections import Counter, defaultdict

NUM = re.compile(r'(\w+)=(-?[0-9.]+)(?=\s|$)')
med = lambda xs: st.median(xs) if xs else float('nan')

for path in sys.argv[1:]:
    rows, views = [], defaultdict(list)
    phase, view = 'load', None
    for l in open(path, encoding='utf-8', errors='replace'):
        if l.startswith('screen_transition') and 'reason=race_start' in l:
            phase = 'countdown'
        elif 'type=countdown remaining=GO' in l:
            phase = 'race'
        elif l.startswith('drive_camera event=start') or l.startswith('drive_camera event=commit'):
            view = re.search(r'view=(\S+)', l).group(1)
        elif l.startswith('drive_script_complete'):
            phase = 'end'
        ph = phase if phase != 'race' else 'race:' + str(view)
        if l.startswith('frame_stats '):
            d = {k: float(v) for k, v in NUM.findall(l)}
            d['phase'] = ph
            m = re.search(r'ft_hist=(\S+)', l)
            d['hist'] = [(float(a), int(b)) for a, b in (x.split(':') for x in m.group(1).split(','))] if m else []
            m = re.search(r'rworst=(\S+)', l)
            d['rworst'] = [float(x) for x in m.group(1).split('/')] if m else []
            m = re.search(r' render=\d+x(\d+)', l)
            d['rows'] = int(m.group(1)) if m else 0
            rows.append(d)
        elif l.startswith('frame_gpu_views') and phase == 'race':
            d = {k: float(v) for k, v in NUM.findall(l)}
            kind = re.search(r'kind=(\S+)', l).group(1)
            views[(ph, kind)].append(d)
    name = path.replace('\\', '/').split('/')[-1][:30]
    print(f'== {name}  windows {len(rows)}')
    print(' phase              n  fps   frame  main  rCPU  acq   gpu   shadow opq  transp post  ui   gap  rows')
    order = list(dict.fromkeys(r['phase'] for r in rows))
    for p in order:
        R = [r for r in rows if r['phase'] == p and 'gpu_ms' in r]
        if not R or p == 'load':
            continue
        g = lambda k: med([r[k] for r in R if k in r])
        rcpu = med([r['render_ms'] - r['acquire'] for r in R])
        print(f" {p:17s}{len(R):3d} {g('fps'):5.1f} {g('avg_ms'):5.2f} {g('main_ms'):5.2f} {rcpu:5.2f} {g('acquire'):5.2f} {g('gpu_ms'):5.2f} {g('g_shadow'):5.2f} {g('g_opaque'):5.2f} {g('g_transp'):5.2f} {g('g_post'):5.2f} {g('g_ui'):5.2f} {g('g_gap'):5.2f} {int(g('rows'))}")
    race = [r for r in rows if r['phase'].startswith('race')]
    h = Counter()
    for r in race:
        for b, c in r['hist']:
            h[b] += c
    tot = sum(h.values())
    # Bucket share on the 240 Hz grid: nearest multiple of 4.167 ms.
    grid = Counter()
    for b, c in h.items():
        k = round((b + 0.25) / (1000 / 240))
        grid[k] += c
    print(' race intervals by 240 Hz multiple (k x 4.17 ms):', ', '.join(f'{k}:{100*c/tot:.1f}%' for k, c in sorted(grid.items()) if c / tot >= 0.005))
    on120 = sum(c for k, c in grid.items() if k % 2 == 0)
    print(f' share on the 120 Hz grid (even k): {100*on120/tot:.1f}%   frames<9ms {100*sum(c for b,c in h.items() if b<9)/tot:.1f}%   >=20ms {100*sum(c for b,c in h.items() if b>=20)/tot:.1f}%')
    print(f" race median: gpu {med([r['gpu_ms'] for r in race]):.2f} ms (p90 window {sorted(r['gpu_ms'] for r in race)[int(.9*len(race))]:.2f}), main {med([r['main_ms'] for r in race]):.2f}, render CPU {med([r['render_ms']-r['acquire'] for r in race]):.2f}, acquire {med([r['acquire'] for r in race]):.2f}, frame {med([r['avg_ms'] for r in race]):.2f}")
    print(' per-view GPU in race (median over windows, ms):')
    for (p, kind), V in sorted(views.items()):
        keys = [k for k in V[0] if k.startswith('g_')]
        tot_v = med([sum(v[k] for k in keys) for v in V])
        print(f'  {p:17s} {kind:4s} n={len(V):4d} sum={tot_v:.2f} ' + ' '.join(f'{k[2:]}={med([v[k] for v in V]):.2f}' for k in keys))
    dips = [r for r in race if r.get('max_ms', 0) > 33]
    if dips:
        acq = [r['rworst'][3] for r in dips if len(r['rworst']) > 3]
        tot_w = [r['rworst'][0] for r in dips if r['rworst']]
        print(f" dips (max>33 ms): {len(dips)} of {len(race)} race windows; worst frame {med(tot_w):.1f} ms of which acquire {med(acq):.1f} ms; window gpu {med([r['gpu_ms'] for r in dips]):.2f} vs {med([r['gpu_ms'] for r in race]):.2f}; img_kb_s>0 in {sum(1 for r in dips if r.get('img_kb_s',0)>0)}")
