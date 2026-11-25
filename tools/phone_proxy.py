"""Phone proxy analysis: predict the phone frame split from a phone-like desktop run (docs/PERFORMANCE.md "Phone proxy").

Usage:
  py -I tools/phone_proxy.py report RUN_DIR [--compare RUN_DIR] [--calibration tools/phone_proxy_calibration.json]
  py -I tools/phone_proxy.py fit --phone STATE=LOG[,LOG...] ... --proxy RUN_DIR [--out FILE]
  py -I tools/phone_proxy.py states LOG [LOG ...]
  py -I tools/phone_proxy.py selftest

A RUN_DIR holds one sub-folder per perf_test state (normal, shadows, empty_scene) with the stdout.log of a desktop
run made by tools/phone_proxy.ps1. Logs may be desktop stdout or phone log-*.txt files (`[ seconds ] line`).
Race windows are frame_stats lines with sim_ticks_per_s > 0; the first --skip (3) of each log are dropped.

Model (fitted per component over the perf_test states by `fit`, stored in the calibration JSON):
  - main thread = non-sim part (main_ms - fixed_ms) + per-tick sim cost x ticks per frame. The per-tick cost is
    fixed_ms x fps / sim_ticks_per_s; the phone's ticks per frame follow its predicted frame (60 Hz sim, at most 4
    ticks per frame), so the main part is solved as a fixed point;
  - render thread = render_ms - r_present - acquire (busy, without the waits for the swapchain);
  - GPU = gpu_ms (timestamp queries);
  - frame = max(main, render, gpu) + a fitted gap (Fifo quantisation, waits).
  Each component maps phone = a x proxy + b ("affine") or phone = k x proxy ("ratio"); `fit` keeps the form with
  the smaller leave-one-out error (affine only with a positive slope) and prints the residuals.
`report` prints, per state, the predicted phone split, the platform-independent counters (draws, pipeline and
material switches, uploads, primitives, laps), with --compare the predicted delta and the counters that moved, and
ends with one VERDICT line. `states` prints per-state medians of logs (phone or desktop). Read-only on the logs.
"""
import argparse
import json
import math
import os
import re
import statistics
import sys

LINE = re.compile(r'^(?:\[\s*[0-9.]+\]\s+)?(\S+)\s?(.*)$')
ANSI = re.compile(r'\x1b\[[0-9;]*m')
STATES = ('normal', 'shadows', 'empty_scene')
# perf_test label of a state in frame_stats (`perf_test=off:shadows`).
LABELS = {'normal': 'normal', 'shadows': 'off:shadows', 'empty_scene': 'off:empty_scene'}
TIMES = ('fps main_ms fixed_ms outside_ms extract_ms render_ms r_meshes r_views acquire r_queue r_sort r_prepare '
         'r_bind r_encode r_submit r_present r_cleanup enc_tail gpu_ms g_pre g_shadow g_opaque g_transp g_post g_ui '
         'g_upscale g_gap sim_ticks_per_s').split()
COUNTERS = ('d_opaque d_alpha d_shadow d_transp d_ui shadow_views vis casters views3d t_draws t_items t_pipe_sw '
            't_mat_sw t_pipes t_mats enc_cbs bl_slabs bl_res mesh_up_s mesh_kb_s mesh_stream_kb_s img_up_s '
            'img_kb_s').split()
LAPS = ['lap_dt', 'lap_dp', 'lap_gd', 'lap_tp', 'lap_cb', 'lap_mt']
COMPONENTS = ('main_nonsim', 'tick_ms', 'render_busy', 'gpu_ms')
MAX_TICKS = 4.0
SIM_HZ = 60.0


def num(v):
    try:
        return float(v.split('/')[0]) if isinstance(v, str) else float(v)
    except (TypeError, ValueError, AttributeError):
        return None


def windows(path, skip):
    """Race frame_stats windows of a log, grouped by perf_test label; the first `skip` of each label dropped."""
    out = {}
    with open(path, encoding='utf-8', errors='replace') as f:
        for raw in f:
            m = LINE.match(ANSI.sub('', raw.rstrip('\n')))
            if not m or m.group(1) != 'frame_stats':
                continue
            fl = dict(kv.split('=', 1) for kv in m.group(2).split() if '=' in kv)
            if not num(fl.get('sim_ticks_per_s')):
                continue
            out.setdefault(fl.get('perf_test', 'normal'), []).append(fl)
    return {k: v[skip:] for k, v in out.items() if len(v) > skip}


def derived(w):
    """Model inputs of one window (None when a field is missing)."""
    g = {k: num(w.get(k)) for k in TIMES + COUNTERS + LAPS}
    fps, sim = g['fps'], g['sim_ticks_per_s']
    if None in (fps, g['main_ms'], g['fixed_ms'], g['render_ms']) or not fps or not sim:
        return None
    g['ticks_per_frame'] = sim / fps
    g['main_nonsim'] = g['main_ms'] - g['fixed_ms']
    g['tick_ms'] = g['fixed_ms'] / g['ticks_per_frame']
    g['render_busy'] = g['render_ms'] - (g['r_present'] or 0) - (g['acquire'] or 0)
    g['frame_ms'] = 1000.0 / fps
    if g.get('lap_dt') is not None:
        g['dt_per_tick'] = g['lap_dt'] / g['ticks_per_frame']
    return g


def medians(rows):
    keys = sorted({k for r in rows for k, v in r.items() if v is not None})
    return {k: statistics.median(v for v in (r.get(k) for r in rows) if v is not None) for k in keys}


def state_medians(paths, skip, want=None):
    """{state: (windows, medians)} over the logs; `want` maps every window to that state (one-state run)."""
    acc = {}
    for p in paths:
        for label, ws in windows(p, skip).items():
            state = want or next((s for s, l in LABELS.items() if l == label), label)
            acc.setdefault(state, []).extend(r for r in (derived(w) for w in ws) if r)
    return {s: (len(rows), medians(rows)) for s, rows in acc.items() if rows}


def state_logs(path, state):
    folder = os.path.join(path, state)
    return sorted(os.path.join(folder, f) for f in (os.listdir(folder) if os.path.isdir(folder) else [])
                  if f.startswith('stdout') and f.endswith('.log'))


def repeat_frames(cal, path, state, skip):
    """Predicted phone frame of each repeat (log) of one state."""
    out = []
    for log in state_logs(path, state):
        got = state_medians([log], skip, want=state)
        if state in got:
            out.append(predict(cal, got[state][1])['frame_ms'])
    return out


def run_dir(path, skip):
    """Medians of a proxy run folder: <dir>/<state>/stdout*.log (repeats pooled)."""
    out = {}
    for s in STATES:
        got = state_medians(state_logs(path, s), skip, want=s)
        if s in got:
            out[s] = got[s]
    return out


def lstsq(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    a = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx if sxx > 1e-12 else 0.0
    return a, my - a * mx


def ratio(xs, ys):
    sxx = sum(x * x for x in xs)
    return (sum(x * y for x, y in zip(xs, ys)) / sxx if sxx > 1e-12 else 0.0), 0.0


def loo(fn, xs, ys):
    """Leave-one-out RMS error of a fit form."""
    if len(xs) < 3:
        return float('inf')
    errs = []
    for i in range(len(xs)):
        a, b = fn(xs[:i] + xs[i + 1:], ys[:i] + ys[i + 1:])
        errs.append(a * xs[i] + b - ys[i])
    return math.sqrt(sum(e * e for e in errs) / len(errs))


def solve_main(cal, p, frame_guess):
    """Main thread of the phone at a given phone frame: non-sim part + ticks(frame) x per-tick cost."""
    ticks = min(MAX_TICKS, frame_guess * SIM_HZ / 1000.0)
    return apply(cal, 'main_nonsim', p['main_nonsim']) + ticks * apply(cal, 'tick_ms', p['tick_ms'])


def apply(cal, comp, x):
    c = cal['components'][comp]
    return c['a'] * x + c['b']


def predict(cal, p):
    """Predicted phone split of one proxy state's medians."""
    render = apply(cal, 'render_busy', p['render_busy'])
    gpu = apply(cal, 'gpu_ms', p.get('gpu_ms') or 0.0)
    frame = max(render, gpu, 1000.0 / SIM_HZ)
    for _ in range(50):
        main = solve_main(cal, p, frame)
        new = max(main, render, gpu) + cal['gap_ms']
        if abs(new - frame) < 1e-4:
            break
        frame = new
    main = solve_main(cal, p, frame)
    bound = max((('main', main), ('render', render), ('gpu', gpu)), key=lambda kv: kv[1])[0]
    return {'frame_ms': frame, 'fps': 1000.0 / frame, 'main': main, 'render': render, 'gpu': gpu, 'bound': bound,
            'ticks': min(MAX_TICKS, frame * SIM_HZ / 1000.0)}


def fit(phone, proxy):
    """Calibration from {state: medians} of the phone and of the proxy."""
    states = [s for s in STATES if s in phone and s in proxy]
    cal = {'version': 1, 'states': states, 'components': {}}
    lines = []
    for comp in COMPONENTS:
        xs = [proxy[s][comp] for s in states]
        ys = [phone[s][comp] for s in states]
        forms = {'affine': lstsq, 'ratio': ratio}
        errs = {name: loo(fn, xs, ys) for name, fn in forms.items()}
        # A slope of zero or below would predict the phone faster when the proxy gets slower: such a fit only
        # follows phone noise (DVFS) across the states, so the affine form must keep a positive slope.
        if lstsq(xs, ys)[0] <= 0:
            errs['affine'] = float('inf')
        best = min(errs, key=errs.get) if len(states) >= 3 else 'ratio'
        a, b = forms[best](xs, ys)
        res = [a * x + b - y for x, y in zip(xs, ys)]
        cal['components'][comp] = {'form': best, 'a': round(a, 4), 'b': round(b, 3),
                                   'loo_rms': {k: (round(v, 3) if math.isfinite(v) else None) for k, v in errs.items()},
                                   'residuals': {s: round(r, 2) for s, r in zip(states, res)}}
        lines.append(f'   {comp:12s} {best:6s} a={a:6.3f} b={b:6.2f}  proxy ' +
                     ' '.join(f'{s}={x:.2f}' for s, x in zip(states, xs)) + '  phone ' +
                     ' '.join(f'{y:.2f}' for y in ys) + '  residual ' + ' '.join(f'{r:+.2f}' for r in res) +
                     '  loo ' + ' '.join(f'{k}={v:.2f}' for k, v in errs.items()))
    # Gap between the slowest component and the phone's frame.
    # The gap moves the ticks per frame and so the main part: iterate to the mean residual of zero.
    cal['gap_ms'] = 0.0
    for _ in range(100):
        step = sum(phone[s]['frame_ms'] - predict(cal, proxy[s])['frame_ms'] for s in states) / len(states)
        cal['gap_ms'] += step
        if abs(step) < 1e-6:
            break
    cal['gap_ms'] = round(cal['gap_ms'], 3)
    out = []
    for s in states:
        pr = predict(cal, proxy[s])
        out.append((s, pr, phone[s]))
        cal.setdefault('check', {})[s] = {'phone_frame_ms': round(phone[s]['frame_ms'], 2),
                                          'predicted_frame_ms': round(pr['frame_ms'], 2), 'bound': pr['bound']}
    return cal, lines, out


def fmt_pred(pr):
    return (f'frame={pr["frame_ms"]:.1f}ms ({pr["fps"]:.1f} fps) main={pr["main"]:.1f} render={pr["render"]:.1f} '
            f'gpu={pr["gpu"]:.1f} ticks/frame={pr["ticks"]:.2f} bound={pr["bound"]}')


def counters_line(m):
    keys = ['main_nonsim', 'tick_ms', 'render_busy', 'gpu_ms', 'r_encode', 'enc_tail', 'r_prepare', 'g_opaque',
            'g_shadow'] + COUNTERS + list(LAPS) + ['dt_per_tick']
    return ' '.join(f'{k}={m[k]:.2f}' if abs(m[k]) < 100 else f'{k}={m[k]:.0f}' for k in keys if k in m)


def moved(a, b, limit=8):
    """Counters and times that moved between two medians, by relative change."""
    out = []
    keys = ['main_nonsim', 'tick_ms', 'render_busy', 'gpu_ms', 'r_encode', 'enc_tail', 'r_prepare', 'r_meshes',
            'r_queue', 'extract_ms', 'g_opaque', 'g_shadow', 'g_transp', 'dt_per_tick'] + COUNTERS + list(LAPS)
    for k in keys:
        x, y = a.get(k), b.get(k)
        if x is None or y is None:
            continue
        floor = 0.3 if k in TIMES or k in LAPS or k in COMPONENTS or k == 'dt_per_tick' else 2.0
        if abs(y - x) < floor:
            continue
        rel = (y - x) / max(abs(x), 1e-6)
        if abs(rel) >= 0.05:
            out.append((abs(rel), f'{k} {x:.2f}->{y:.2f} ({100 * rel:+.0f}%)'))
    return [t for _, t in sorted(out, reverse=True)[:limit]]


def report(args):
    cal = json.load(open(args.calibration, encoding='utf-8'))
    base = run_dir(args.run, args.skip)
    if not base:
        print(f'VERDICT FAIL no race windows under {args.run}')
        return 1
    other = run_dir(args.compare, args.skip) if args.compare else {}
    print(f'phone_proxy report run={args.run}' + (f' compare={args.compare}' if args.compare else '') +
          f' calibration={os.path.basename(args.calibration)} (states fitted: {",".join(cal["states"])})')
    verdict = []
    for s in STATES:
        if s not in base:
            continue
        n, m = base[s]
        pr = predict(cal, m)
        print(f'== {s}: {n} windows, desktop fps {m["fps"]:.0f}')
        print(f'   phone predicted: {fmt_pred(pr)}')
        print(f'   counters: {counters_line(m)}')
        part = f'{s}={pr["frame_ms"]:.1f}ms/{pr["bound"]}'
        if s in other:
            n2, m2 = other[s]
            p2 = predict(cal, m2)
            d = p2['frame_ms'] - pr['frame_ms']
            print(f'   compare ({n2} windows): {fmt_pred(p2)}')
            print(f'   delta frame {d:+.1f} ms ({100 * d / pr["frame_ms"]:+.0f}%) main {p2["main"] - pr["main"]:+.1f} '
                  f'render {p2["render"] - pr["render"]:+.1f} gpu {p2["gpu"] - pr["gpu"]:+.1f}')
            mv = moved(m, m2)
            print('   moved: ' + ('; '.join(mv) if mv else 'nothing over 5 %'))
            fb = repeat_frames(cal, args.run, s, args.skip)
            fc = repeat_frames(cal, args.compare, s, args.skip)
            limit = noise_limit(cal, fb, fc)
            print(f'   repeats: base ' + ' '.join(f'{f:.1f}' for f in fb) + ' change ' +
                  ' '.join(f'{f:.1f}' for f in fc) + f'; delta outside noise when |delta| > {limit:.1f} ms')
            part += f'->{p2["frame_ms"]:.1f}ms({d:+.1f}' + ('' if abs(d) > limit else ',noise') + ')'
        verdict.append(part)
    if args.compare:
        print(f'   noise: one run against one run of the same revision differs by up to {cal.get("noise_ms", float("nan")):.1f} ms '
              'of predicted frame (docs/PERFORMANCE.md "Phone proxy"); with repeats the limit is two standard errors of '
              'the delta. A delta marked noise is not a result.')
    print('VERDICT ' + ' '.join(verdict))
    return 0


def noise_limit(cal, fb, fc):
    """|delta| that counts as a result: two standard errors of the delta from the repeats' spread, never below the
    calibrated single-run noise divided by the repeats' square root."""
    floor = cal.get('noise_ms', 0.0) / math.sqrt(max(1, min(len(fb), len(fc))))
    if len(fb) >= 2 and len(fc) >= 2:
        se = math.sqrt(statistics.variance(fb) / len(fb) + statistics.variance(fc) / len(fc))
        return max(2.0 * se, floor)
    return floor


def parse_phone(specs, skip):
    out = {}
    for spec in specs:
        state, paths = spec.split('=', 1)
        got = state_medians(paths.split(','), skip, want=state)
        if state in got:
            out[state] = got[state][1]
    return out


def cmd_fit(args):
    phone = parse_phone(args.phone, args.skip)
    proxy = {s: m for s, (_, m) in run_dir(args.proxy, args.skip).items()}
    cal, lines, out = fit(phone, proxy)
    cal['proxy'] = args.note or args.proxy
    if args.noise is not None:
        cal['noise_ms'] = args.noise
    print(f'phone_proxy fit states={",".join(cal["states"])} gap_ms={cal["gap_ms"]:.2f}')
    for line in lines:
        print(line)
    for s, pr, ph in out:
        print(f'   {s:12s} phone frame {ph["frame_ms"]:.1f} (main {ph["main_ms"]:.1f} render_busy {ph["render_busy"]:.1f} '
              f'gpu {ph.get("gpu_ms", 0):.1f})  predicted {fmt_pred(pr)}')
    if args.out:
        with open(args.out, 'w', encoding='utf-8', newline='\n') as f:
            json.dump(cal, f, indent=1, sort_keys=True)
            f.write('\n')
    spread = max(abs(pr['frame_ms'] - ph['frame_ms']) for _, pr, ph in out)
    print(f'VERDICT fit states={len(out)} max_frame_error_ms={spread:.2f} out={args.out or "-"}')
    return 0


def cmd_states(args):
    for p in args.logs:
        for s, (n, m) in state_medians([p], args.skip).items():
            print(f'{os.path.basename(p)} {s} windows={n} frame={m["frame_ms"]:.1f} ' + counters_line(m))
    return 0


def selftest():
    """Synthetic check of the fit and the fixed point: a proxy that is exactly a scaled phone fits with no error."""
    phone, proxy = {}, {}
    for i, s in enumerate(STATES):
        p = {'main_nonsim': 3.0 + i, 'tick_ms': 1.0 + 0.2 * i, 'render_busy': 4.0 - i, 'gpu_ms': 5.0 - 1.5 * i}
        proxy[s] = p
        ph = {k: 4.0 * v + 1.0 for k, v in p.items()}
        phone[s] = ph
    cal = {'components': {c: {'a': 4.0, 'b': 1.0} for c in COMPONENTS}, 'gap_ms': 2.0}
    for s in STATES:
        pr = predict(cal, proxy[s])
        ph = phone[s]
        expect_main = ph['main_nonsim'] + min(MAX_TICKS, pr['frame_ms'] * SIM_HZ / 1000) * ph['tick_ms']
        assert abs(pr['main'] - expect_main) < 1e-3, (s, pr)
        assert abs(pr['frame_ms'] - (max(pr['main'], pr['render'], pr['gpu']) + 2.0)) < 1e-3, (s, pr)
        phone[s]['frame_ms'] = pr['frame_ms']
    cal2, _, out = fit(phone, proxy)
    for comp in COMPONENTS:
        c = cal2['components'][comp]
        assert abs(c['a'] - 4.0) < 1e-6 and abs(c['b'] - 1.0) < 1e-6, (comp, c)
    assert abs(cal2['gap_ms'] - 2.0) < 1e-6, cal2['gap_ms']
    # Log parsing: a phone-format and a desktop-format line.
    import tempfile
    line = ('frame_stats fps=25.0 sim_ticks_per_s=60 perf_test=off:shadows main_ms=30 fixed_ms=10 render_ms=30 '
            'r_present=5 acquire=1 gpu_ms=35 lap_dt=6.0/9/3\n')
    with tempfile.TemporaryDirectory() as d:
        a = os.path.join(d, 'a.txt')
        with open(a, 'w', encoding='utf-8') as f:
            f.write(''.join(f'[ {i}.0]   ' + line for i in range(5)) + '\x1b[2mnoise\x1b[0m\n' + line)
        got = state_medians([a], 3)
        n, m = got['shadows']
        assert n == 3 and abs(m['tick_ms'] - 10 / 2.4) < 1e-9 and abs(m['render_busy'] - 24.0) < 1e-9, got
        assert abs(m['dt_per_tick'] - 2.5) < 1e-9, m
        # report: two run folders (repeat logs pooled), a compare delta and the moved counters.
        for side, gpu in (('base', 35), ('change', 45)):
            for s in STATES:
                folder = os.path.join(d, side, s)
                os.makedirs(folder)
                text = line.replace('off:shadows', LABELS[s]).replace('gpu_ms=35', f'gpu_ms={gpu}')
                for name in ('stdout.log', 'stdout-2.log'):
                    with open(os.path.join(folder, name), 'w', encoding='utf-8') as f:
                        f.write(text * 4)
        calp = os.path.join(d, 'cal.json')
        with open(calp, 'w', encoding='utf-8') as f:
            json.dump({'states': list(STATES), 'gap_ms': 0.0, 'noise_ms': 1.0,
                       'components': {c: {'a': 1.0, 'b': 0.0} for c in COMPONENTS}}, f)
        import contextlib
        import io
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = report(argparse.Namespace(run=os.path.join(d, 'base'), compare=os.path.join(d, 'change'),
                                             calibration=calp, skip=3))
        text = buf.getvalue()
        assert code == 0 and 'VERDICT normal=35.0ms/gpu->45.0ms(+10.0)' in text, text
        assert 'gpu_ms 35.00->45.00 (+29%)' in text and '2 windows' in text, text
    print('VERDICT selftest PASS')
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--skip', type=int, default=3, help='race windows dropped at the start of each log')
    sub = ap.add_subparsers(dest='cmd', required=True)
    r = sub.add_parser('report')
    r.add_argument('run')
    r.add_argument('--compare')
    r.add_argument('--calibration', default=os.path.join(os.path.dirname(__file__), 'phone_proxy_calibration.json'))
    f = sub.add_parser('fit')
    f.add_argument('--phone', action='append', required=True, help='STATE=LOG[,LOG...]')
    f.add_argument('--proxy', required=True)
    f.add_argument('--out')
    f.add_argument('--note')
    f.add_argument('--noise', type=float)
    s = sub.add_parser('states')
    s.add_argument('logs', nargs='+')
    sub.add_parser('selftest')
    args = ap.parse_args()
    if args.cmd == 'report':
        return report(args)
    if args.cmd == 'fit':
        return cmd_fit(args)
    if args.cmd == 'states':
        return cmd_states(args)
    return selftest()


if __name__ == '__main__':
    sys.exit(main())
