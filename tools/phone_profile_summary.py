"""Summarise phone logs of the profile APK: CPU against GPU per race window, top spans, GPU passes.

Usage: py -I tools/phone_profile_summary.py [--top 12] [--by-perf-test | --table [--baseline LOG] [--presses TXT]] LOG [LOG ...]
Each LOG is a phone log-*.txt (the `[  seconds]  line` format of Download/GRID-Reassembly). Race windows are
the 1-second frame_stats windows with sim_ticks_per_s > 0. For each log it prints: the 1 s FPS min/p10/median,
frame-time shares from ft_hist, the render sizes, the median frame split (main thread, render thread, GPU
timestamps and their segments, draw counts), which of main_ms, render_ms and gpu_ms is largest per window,
and from the 10 s profile tables (feature `profile`, docs/PERFORMANCE.md "Per-system profile") the windows'
FPS, the two busiest threads, the top schedules, spans and GPU passes over the race windows.
With --by-perf-test the windows are split by the Performance test state (`perf_test=` on frame_stats; profile and
GPU windows count only when no `perf_test event=apply` fell inside them), one report and verdict per state.
With --table it prints one row per contiguous Performance test step instead (the Auto cycle and
tools/android_bench.ps1, docs/ANDROID_GPU.md section 7): medians of FPS, frame and GPU times and draws, the p10 FPS,
the change against the first normal step, and a heat-drift line from the first and last normal steps. The first
SETTLE windows of each step are skipped. Each window gets the camera it showed: the view from the game's
`drive_camera event=change view=` lines (the first change names the starting view), or `back` while a Look Back
press of --presses TXT was held (android_bench.ps1 presses.txt: seconds since start, action, `sent hold_ms=`). When
the camera changed during the run it adds the same medians per step and camera, dropping the window of each change.
--baseline LOG (with --table) adds the same race ticks from a normal run of the same scripted race (android_bench
-PerfTest ''): the autopilot drives a lap of about 85 s, so each 20 s step sees another part of the track, and only a
run that reaches the same place at the same tick separates a switch from the scenery. Columns: the baseline's median
FPS and GPU ms over the step's tick range and the step's GPU ms minus it.
Ends with one verdict line per log (or state). Read-only; prints no log lines.
"""
import argparse
import collections
import re
import statistics

LINE = re.compile(r'^\[\s*([0-9.]+)\]\s+(\S+)\s?(.*)$')
TICK = re.compile(r'\btick=(\d+)')
VIEW = re.compile(r'\bview=(\S+) from=(\S+)')
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


def parse(path, presses=None):
    """Race windows, profile windows and GPU windows of one log, each tagged with the Performance test state; race
    windows also with their camera (`camera`, see the module notes)."""
    race, pwin, gwin, views = [], [], [], []
    first_race = None
    state, changed = 'normal', -1e9
    tick = None
    with open(path, encoding='utf-8', errors='replace') as f:
        for raw in f:
            m = LINE.match(raw.rstrip('\n'))
            if not m:
                continue
            t, kind, rest = float(m.group(1)), m.group(2), m.group(3)
            if kind == 'drive_camera' and 'event=change' in rest:
                m = VIEW.search(rest)
                if m:
                    views.append((t, m.group(2), m.group(1)))
            elif kind == 'drive_input':
                m = TICK.search(rest)
                tick = int(m.group(1)) if m else tick
            elif kind == 'perf_test':
                fl = fields(rest)
                if fl.get('event') == 'apply':
                    state, changed = fl.get('switches', '?'), t
            elif kind == 'frame_stats':
                fl = fields(rest)
                if not num(fl.get('sim_ticks_per_s')):
                    continue
                first_race = first_race if first_race is not None else t
                fl['_tick'] = tick
                fl['_t'] = t
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
    label_cameras(race, views, presses)
    return race, pwin, gwin


def label_cameras(race, views, presses):
    """Sets `camera` on each race window from its middle (a frame_stats line closes a 1 s window): the view of the
    last change before it, or `back` inside a held Look Back press. Nothing when the camera never changed."""
    holds = []
    if presses:
        with open(presses, encoding='utf-8') as f:
            for line in f:
                part = line.split()
                if len(part) >= 4 and part[1] == 'look' and part[2] == 'sent' and part[3].startswith('hold_ms='):
                    holds.append((float(part[0]), float(part[0]) + int(part[3][8:]) / 1000))
    if not views and not holds:
        return
    for w in race:
        mid = w['_t'] - 0.5
        if any(a <= mid <= b for a, b in holds):
            w['camera'] = 'back'
            continue
        cam = views[0][1] if views else 'start'
        for t, _, to in views:
            if t > mid:
                break
            cam = to
        w['camera'] = cam


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


TABLE = ('avg_ms', 'gpu_ms', 'g_shadow', 'g_opaque', 'g_transp', 'g_post', 'g_ui', 'g_upscale', 'd_opaque', 'd_alpha',
         'd_shadow')
SETTLE = 2


def baseline_at(base, ws):
    """The baseline windows inside the race-tick range of the step windows `ws`."""
    ticks = [w['_tick'] for w in ws if w.get('_tick') is not None]
    if not base or not ticks:
        return []
    lo, hi = min(ticks), max(ticks)
    return [w for w in base if w.get('_tick') is not None and lo <= w['_tick'] <= hi]


def table(path, baseline=None, presses=None):
    """One row per contiguous perf_test step, in log order. The dynamic resolution may move inside a step: the
    size column is the step's most common render size, marked * when it moved."""
    race, _, _ = parse(path, presses)
    steps = []
    for w in race:
        key = (w.get('perf_test', 'normal'), None)
        if not steps or steps[-1][0] != key:
            steps.append((key, []))
        steps[-1][1].append(w)
    print(f'== {path}')
    if not steps:
        print('VERDICT no-race-data')
        return
    base_ws = parse(baseline)[0] if baseline else None
    extra = f' {"b_fps":>5} {"b_gpu":>6} {"d_gpu":>6} {"b_size":>9}' if base_ws else ''
    print(f'   {"step":<34} {"n":>3} {"size":>10} {"fps":>5} {"p10":>5} {"dfps":>6}{extra} ' + ' '.join(f'{k:>8}' for k in TABLE))
    base, normals = None, []
    for (label, size), ws in steps:
        ws = ws[SETTLE:] if len(ws) > SETTLE + 2 else ws
        fps = sorted(num(w.get('fps')) or 0 for w in ws)
        med = statistics.median(fps)
        p10 = fps[max(0, int(len(fps) * 0.1) - 1)]
        base = med if base is None and label == 'normal' else base
        if label == 'normal':
            normals.append((med, ws))
        cols = []
        for k in TABLE:
            vals = [v for v in (num(w.get(k)) for w in ws) if v is not None]
            cols.append(f'{statistics.median(vals):8.2f}' if vals else f'{"-":>8}')
        delta = f'{med - base:+6.1f}' if base is not None else f'{"":>6}'
        sizes = collections.Counter(w.get('render', '?') for w in ws)
        size = sizes.most_common(1)[0][0] + ('*' if len(sizes) > 1 else '')
        name = label.replace('shadows+foliage+ground+effects+hud+samples', 'all')
        extra = ''
        if base_ws is not None:
            bw = baseline_at(base_ws, ws)
            bf = [v for v in (num(w.get('fps')) for w in bw) if v is not None]
            bg = [v for v in (num(w.get('gpu_ms')) for w in bw) if v is not None]
            sg = [v for v in (num(w.get('gpu_ms')) for w in ws) if v is not None]
            bs = collections.Counter(w.get('render', '?') for w in bw).most_common(1)
            if bf and bg and sg:
                extra = (f' {statistics.median(bf):5.1f} {statistics.median(bg):6.2f} '
                         f'{statistics.median(sg) - statistics.median(bg):+6.2f} {bs[0][0]:>9}')
            else:
                extra = f' {"-":>5} {"-":>6} {"-":>6} {"-":>9}'
        print(f'   {name[:34]:<34} {len(ws):>3} {size:>10} {med:5.1f} {p10:5.1f} {delta}{extra} ' + ' '.join(cols))
    if len(normals) >= 2:
        g0 = statistics.median(num(w.get('gpu_ms')) or 0 for w in normals[0][1])
        g1 = statistics.median(num(w.get('gpu_ms')) or 0 for w in normals[-1][1])
        print(f'   drift (first vs last normal): fps {normals[0][0]:.1f} -> {normals[-1][0]:.1f}, '
              f'gpu {g0:.2f} -> {g1:.2f} ms')
    cams = camera_table(steps)
    print(f'VERDICT steps={len(steps)} normal_fps={base if base is not None else 0:.1f} '
          f'drift_fps={(normals[-1][0] - normals[0][0]) if len(normals) >= 2 else 0:+.1f}{cams}')


CAMERA_COLS = ('fps', 'avg_ms', 'gpu_ms', 'g_opaque', 'd_opaque')


def camera_table(steps):
    """Per step and camera (label_cameras): medians of fps, frame and GPU times. The first window of each contiguous camera run holds the change and is dropped, as are the step's
    SETTLE windows. Returns the verdict suffix."""
    cameras = list(dict.fromkeys(w['camera'] for _, ws in steps for w in ws if w.get('camera')))
    if len(cameras) < 2:
        return ''
    med = lambda ws, k: statistics.median(v for v in (num(w.get(k)) for w in ws) if v is not None)         if any(num(w.get(k)) is not None for w in ws) else None
    fmt = lambda v: f'{v:6.1f}' if v is not None else f'{"-":>6}'
    print('   by camera (fps / frame ms / gpu ms):')
    print(f'   {"step":<34} ' + ' '.join(f'{c[:20]:>20}' for c in cameras))
    total = collections.defaultdict(list)
    for (label, _), ws in steps:
        ws = ws[SETTLE:] if len(ws) > SETTLE + 2 else ws
        runs, prev = collections.defaultdict(list), None
        for w in ws:
            cam = w.get('camera')
            if cam != prev:
                prev = cam
                continue
            runs[cam].append(w)
        cells = []
        for c in cameras:
            total[c] += runs[c]
            cells.append(' '.join(fmt(med(runs[c], k)) for k in ('fps', 'avg_ms', 'gpu_ms')))
        name = label.replace('shadows+foliage+ground+effects+hud+samples', 'all')
        print(f'   {name[:34]:<34} ' + ' '.join(f'{x:>20}' for x in cells))
    print(f'   {"all steps":<34} {"n":>4} ' + ' '.join(f'{k:>8}' for k in CAMERA_COLS))
    for c in cameras:
        vals = [med(total[c], k) for k in CAMERA_COLS]
        print(f'   {c[:34]:<34} {len(total[c]):>4} ' + ' '.join(f'{v:8.2f}' if v is not None else f'{"-":>8}' for v in vals))
    return ''.join(f' {c}_gpu={med(total[c], "gpu_ms") or 0:.1f}' for c in cameras)


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
    ap.add_argument('--table', action='store_true', help='one row per contiguous Performance test step')
    ap.add_argument('--baseline', help='with --table: a normal run of the same scripted race, matched by race tick')
    ap.add_argument('--presses', help='with --table: android_bench.ps1 presses.txt (Look Back hold times)')
    ap.add_argument('logs', nargs='+')
    args = ap.parse_args()
    for path in args.logs:
        if args.table:
            table(path, args.baseline, args.presses)
        else:
            summarise(path, args.top, args.by_perf_test)


if __name__ == '__main__':
    main()
