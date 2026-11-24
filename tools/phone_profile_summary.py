"""Summarise phone logs of the profile APK: CPU against GPU per race window, top spans, GPU passes.

Usage: py -I tools/phone_profile_summary.py [--top 12] [--by-perf-test] LOG [LOG ...]
Each LOG is a phone log-*.txt (the `[  seconds]  line` format of Download/GRID-Reassembly). Race windows are
the 1-second frame_stats windows with sim_ticks_per_s > 0. For each log it prints: the 1 s FPS min/p10/median,
frame-time shares from ft_hist, the render sizes, the median frame split (main thread, render thread, GPU
timestamps and their segments, draw counts), which of main_ms, render_ms and gpu_ms is largest per window,
and from the 10 s profile tables (feature `profile`, docs/PERFORMANCE.md "Per-system profile") the windows'
FPS, the two busiest threads, the top schedules, spans and GPU passes over the race windows.
With --by-perf-test the windows are split by the Performance test state (`perf_test=` on frame_stats; profile and
GPU windows count only when no `perf_test event=apply` fell inside them), one report and verdict per state.
Ends with one verdict line per log (or state). Read-only; prints no log lines.
"""
import argparse
import collections
import re
import statistics

LINE = re.compile(r'^\[\s*([0-9.]+)\]\s+(\S+)\s?(.*)$')
SPLIT = ('main_ms fixed_ms outside_ms extract_ms render_ms r_meshes r_prepare r_bind r_encode r_submit acquire '
         'r_present r_cleanup gpu_ms g_pre g_shadow g_opaque g_transp g_post g_ui g_upscale g_gap d_opaque d_alpha '
         'd_shadow d_transp shadow_views vis casters views3d').split()
NAME = re.compile(r'name=(.*)$')


def fields(rest):
    return dict(kv.split('=', 1) for kv in rest.split() if '=' in kv)


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def short(name):
    name = name.replace('system: name=', '').replace('"', '')
    return name if len(name) <= 90 else name[:87] + '...'


def parse(path):
    """Race windows, profile windows and GPU windows of one log, each tagged with the Performance test state."""
    race, pwin, gwin = [], [], []
    first_race = None
    state, changed = 'normal', -1e9
    with open(path, encoding='utf-8', errors='replace') as f:
        for raw in f:
            m = LINE.match(raw.rstrip('\n'))
            if not m:
                continue
            t, kind, rest = float(m.group(1)), m.group(2), m.group(3)
            if kind == 'perf_test':
                fl = fields(rest)
                if fl.get('event') == 'apply':
                    state, changed = fl.get('switches', '?'), t
            elif kind == 'frame_stats':
                fl = fields(rest)
                if not num(fl.get('sim_ticks_per_s')):
                    continue
                first_race = first_race if first_race is not None else t
                race.append(fl)
            elif kind == 'profile_window':
                pwin.append([t, num(fields(rest).get('fps')), [], [], [], state, changed])
            elif kind in ('profile_thread', 'profile_span', 'profile_schedule') and pwin:
                fl = fields(rest.split(' name=')[0])
                nm = NAME.search(rest)
                nm = nm.group(1) if nm else '?'
                slot = {'profile_thread': 2, 'profile_span': 3, 'profile_schedule': 4}[kind]
                pwin[-1][slot].append((nm, fl))
            elif kind == 'gpu_window':
                gwin.append((t, {}, state, changed))
            elif kind == 'gpu_pass' and gwin:
                fl = fields(rest.split(' name=')[0])
                nm = NAME.search(rest)
                gwin[-1][1][nm.group(1) if nm else '?'] = (num(fl.get('gpu_ms')), num(fl.get('fragments')))
    # A 10 s table counts only after 10 s of racing and when its state held for the whole window.
    start = first_race if first_race is not None else 1e18
    pwin = [w for w in pwin if w[0] > start + 10 and w[0] - w[6] > 10]
    gwin = [g for g in gwin if g[0] > start + 10 and g[0] - g[3] > 10]
    return race, pwin, gwin


def report(label, race, rp, rg, top):
    print(f'== {label}')
    if not race:
        print('   no race frame_stats windows')
        print('VERDICT no-race-data')
        return
    sizes = collections.Counter(w.get('render', '?') for w in race)
    hist = collections.Counter()
    for w in race:
        for part in w.get('ft_hist', '').split(','):
            if ':' in part:
                edge, count = part.split(':')
                hist[float(edge)] += int(count)
    fps = sorted(num(w['fps']) for w in race)
    p10 = fps[max(0, int(len(fps) * 0.1) - 1)]
    print(f'   race windows {len(race)}: 1s FPS min {fps[0]:.1f} p10 {p10:.1f} median {statistics.median(fps):.1f} '
          f'max {fps[-1]:.1f}; render sizes {dict(sizes.most_common(4))}')
    total = sum(hist.values()) or 1
    print('   frame times (ft_hist bucket from its lower edge in ms, share of frames): ' +
          ' '.join(f'{e:g}+:{100 * c / total:.0f}%' for e, c in sorted(hist.items())))
    med = {k: statistics.median(v for v in (num(w.get(k)) for w in race) if v is not None)
           for k in SPLIT if any(num(w.get(k)) is not None for w in race)}
    print('   median split: ' + ' '.join(f'{k}={v:.2f}' for k, v in med.items()))
    gate = collections.Counter()
    for w in race:
        trio = {k: num(w.get(k)) or 0 for k in ('main_ms', 'render_ms', 'gpu_ms')}
        gate[max(trio, key=trio.get)] += 1
    print(f'   largest per window: {dict(gate)}')
    if rp:
        print('   profile windows (10 s) FPS: ' + ' '.join(f'{w[1]:.1f}' for w in rp))
        thr = collections.defaultdict(list)
        for w in rp:
            for rank, (nm, fl) in enumerate(w[2][:2]):
                thr[rank].append((num(fl.get('in_spans_ms_per_frame')), num(fl.get('work_ms_per_frame'))))
        for rank, vals in thr.items():
            print(f'   thread rank {rank + 1}: in_spans median {statistics.median(v[0] for v in vals):.1f} '
                  f'work median {statistics.median(v[1] for v in vals):.1f} ms/frame')
        for slot, key, kind in ((4, 'incl_ms_per_frame', 'schedules'), (3, 'excl_ms_per_frame', 'spans')):
            acc = collections.defaultdict(list)
            for w in rp:
                for nm, fl in w[slot]:
                    acc[nm].append(num(fl.get(key)))
            ranked = sorted(acc.items(), key=lambda kv: -sum(kv[1]) / len(rp))[:top]
            print(f'   top {kind} (mean over {len(rp)} windows, ms/frame, {key.split("_")[0]}):')
            for nm, vals in ranked:
                print(f'     {sum(vals) / len(rp):7.2f}  {short(nm)}')
    if rg:
        acc = collections.defaultdict(list)
        for g in rg:
            for nm, (ms, frag) in g[1].items():
                acc[nm].append((ms or 0, frag or 0))
        ranked = sorted(acc.items(), key=lambda kv: -sum(v[0] for v in kv[1]))[:top]
        print(f'   GPU passes (RenderDiagnostics, mean over {len(rg)} windows: gpu ms/frame, fragments/frame):')
        for nm, vals in ranked:
            print(f'     {sum(v[0] for v in vals) / len(rg):7.2f}  {sum(v[1] for v in vals) / len(rg):12.0f}  {nm}')
    bound = max(('main_ms', 'render_ms', 'gpu_ms'), key=lambda k: med.get(k, 0))
    print(f'VERDICT median_fps={statistics.median(fps):.1f} min_fps={fps[0]:.1f} main={med.get("main_ms", 0):.1f} '
          f'render={med.get("render_ms", 0):.1f} gpu={med.get("gpu_ms", 0):.1f} g_opaque={med.get("g_opaque", 0):.1f} '
          f'g_shadow={med.get("g_shadow", 0):.1f} largest={bound} profile_windows={len(rp)}')


def summarise(path, top, by_state):
    race, rp, rg = parse(path)
    if not by_state:
        report(path, race, rp, rg, top)
        return
    states = list(dict.fromkeys([w.get('perf_test', 'normal') for w in race] + [w[5] for w in rp]))
    for st in states or ['normal']:
        report(f'{path} perf_test={st}', [w for w in race if w.get('perf_test', 'normal') == st],
               [w for w in rp if w[5] == st], [g for g in rg if g[2] == st], top)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--top', type=int, default=12)
    ap.add_argument('--by-perf-test', action='store_true', help='one report per Performance test state')
    ap.add_argument('logs', nargs='+')
    args = ap.parse_args()
    for path in args.logs:
        summarise(path, args.top, args.by_perf_test)


if __name__ == '__main__':
    main()
