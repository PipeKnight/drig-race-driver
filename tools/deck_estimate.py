"""Steam Deck estimate analysis: per-configuration table of a tools/deck_estimate.ps1 run and its Deck scaling.

Usage:
  py -I tools/deck_estimate.py report OUT_DIR [--skip 3]
  py -I tools/deck_estimate.py selftest

OUT_DIR holds one folder per configuration (p-rtx-desktop, e-uhd-mobile, ...) with stdout.log and run.txt, plus
topology.txt. Per configuration the report checks the facts the estimate rests on, from the logs, not assumed:
  - the adapter: the `frame_split ... adapter="NAME"` line names the intended GPU (Vulkan);
  - the CPU set: run.txt's process affinity equals the mask of the intended efficiency class in topology.txt;
  - the race finished (`drive_script_complete`) and the camera presses ran (`camera_press` lines).
It prints medians over the race windows (frame_stats with sim_ticks_per_s > 0, first --skip dropped): fps (median
of windows and frames-weighted), frame ms, main-thread ms, its non-sim part, render-thread busy ms, GPU ms
(timestamp queries), p99 (median and worst window), and the gap = frame - max(main, render busy, GPU).

Deck model (docs/STEAM_DECK_ESTIMATE.md), each bound as a low..high range:
  - CPU: the E-core run with the RTX (GPU not limiting) gives the main-thread non-sim and per-tick sim costs and the
    render-thread busy time; Deck CPU = E-core ms x CPU_FACTOR, the worst case at least the E-core + UHD run's costs
    (Intel driver CPU time, iGPU sharing memory and power; its render time without the submit phase, which holds
    the wait for the GPU); the sim part follows the Deck's ticks per frame
    (60 Hz sim, at most 4 ticks per frame), solved as a fixed point;
  - GPU: the UHD 770 runs at 800 and 480 rows split the GPU time into fixed + per-pixel parts; Deck GPU =
    fixed / compute ratio + per-pixel / bandwidth ratio, divided by a driver factor (RADV vs Intel's Windows
    Vulkan driver); the mobile preset (480 rows, low shadows) scales its own UHD GPU time with the per-pixel share
    of the desktop 480-row run;
  - frame = max(CPU main, CPU render, GPU) + the gap of the E-core RTX run; p99 = frame x the p99/frame ratio of
    the E-core UHD run.
Ends with one VERDICT line. Read-only on the logs.
"""
import argparse
import os
import re
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from phone_proxy import derived, num, windows  # noqa: E402

CONFIGS = ('p-rtx-desktop', 'p-rtx-mobile', 'e-rtx-desktop', 'e-rtx-mobile', 'p-uhd-desktop', 'p-uhd-desktop480',
           'p-uhd-mobile', 'e-uhd-desktop', 'e-uhd-mobile')
ADAPTER = re.compile(r'frame_split .*?backend=(\S+) adapter="([^"]*)"(?: timestamp_query=(\S+))?')
# Hardware ratios Deck / this PC (docs/STEAM_DECK_ESTIMATE.md "Hardware").
# GPU compute: Deck 8 CU RDNA 2 at 1.0-1.6 GHz = 1.02-1.64 TFLOPS against the UHD 770's 0.79 (32 EU, 1.55 GHz).
COMPUTE = (1.02 / 0.79, 1.64 / 0.79)
# Memory bandwidth: Deck LCD 88 GB/s, OLED 102 GB/s against dual-channel DDR4-3200 (51.2 GB/s), shared with the CPU.
BANDWIDTH = (88 / 51.2, 102 / 51.2)
# Driver: RADV on SteamOS against Intel's Windows Vulkan driver; 1.0 = no gain (the floor), 1.3 = 30 % faster.
DRIVER = (1.0, 1.3)
# CPU: Deck Zen 2 at 2.4-3.5 GHz with SMT and a shared 15 W budget against a Gracemont E-core (3.6 GHz, no SMT).
# < 1 = the Deck faster. Zen 2's IPC is above Gracemont's; under a GPU-heavy load the APU lowers the CPU clock.
CPU_FACTOR = (0.85, 1.3)
SIM_HZ = 60.0
MAX_TICKS = 4.0
PX = {800: 1280 * 800 / 1e6, 480: 768 * 480 / 1e6}


def read_kv(path):
    try:
        with open(path, encoding='utf-8', errors='replace') as f:
            text = f.read()
    except OSError:
        return {}
    return dict(kv.split('=', 1) for kv in text.split() if '=' in kv)


def summary(log, skip):
    """Medians of the race windows of one log, plus the checks."""
    rows, raw = [], []
    for label_windows in windows(log, skip).values():
        for w in label_windows:
            d = derived(w)
            if d:
                d['p99_ms'] = num(w.get('p99_ms'))
                d['frames'] = num(w.get('frames'))
                d['unfocused'] = num(w.get('unfocused')) or 0
                rows.append(d)
                raw.append(w)
    adapter, backend, tsq, presses, complete = '', '', '', 0, False
    with open(log, encoding='utf-8', errors='replace') as f:
        for line in f:
            if not adapter:
                m = ADAPTER.search(line)
                if m:
                    backend, adapter, tsq = m.group(1), m.group(2), m.group(3) or ''
            presses += 'camera_press ' in line
            complete = complete or 'drive_script_complete' in line
    if not rows:
        return {'adapter': adapter, 'backend': backend, 'tsq': tsq, 'presses': presses, 'complete': complete, 'n': 0}

    def med(k):
        v = [r[k] for r in rows if r.get(k) is not None]
        return statistics.median(v) if v else None

    frames = sum(r['frames'] or 0 for r in rows)
    secs = sum((r['frames'] or 0) / r['fps'] for r in rows if r['fps'])
    s = {k: med(k) for k in ('fps', 'frame_ms', 'main_ms', 'main_nonsim', 'tick_ms', 'fixed_ms', 'render_busy',
                             'gpu_ms', 'p99_ms', 'ticks_per_frame', 'r_present', 'acquire', 'r_submit')}
    s['unfocused'] = sum(1 for r in rows if r['unfocused'])
    s['race_s'] = secs
    s['p99_max'] = max((r['p99_ms'] for r in rows if r['p99_ms'] is not None), default=None)
    s['fps_w'] = frames / secs if secs else None
    s['gap'] = s['frame_ms'] - max(s['main_ms'], s['render_busy'], s['gpu_ms'] or 0)
    s['render'] = raw[-1].get('render', '?')
    s.update(adapter=adapter, backend=backend, tsq=tsq, presses=presses, complete=complete, n=len(rows))
    return s


def ticks(frame_ms):
    return min(MAX_TICKS, max(1.0, frame_ms * SIM_HZ / 1000.0))


def deck_frame(cpu, gpu_ms, gap, factor):
    """Fixed point of the Deck frame: main = non-sim + per-tick x ticks(frame)."""
    nonsim, tick, render = cpu['main_nonsim'] * factor, cpu['tick_ms'] * factor, cpu['render_busy'] * factor
    frame = max(nonsim + tick, render, gpu_ms) + gap
    for _ in range(50):
        main = nonsim + tick * ticks(frame)
        frame = max(main, render, gpu_ms) + gap
    return frame, main, render


def gpu_split(d800, d480):
    """(fixed ms, per-pixel ms per megapixel) from the UHD runs at 800 and 480 rows."""
    per_mp = max(0.0, (d800 - d480) / (PX[800] - PX[480]))
    return max(0.0, d800 - per_mp * PX[800]), per_mp


def deck_gpu(fixed, per_px_part, best):
    i = 1 if best else 0
    return (fixed / COMPUTE[i] + per_px_part / BANDWIDTH[i]) / DRIVER[i]


def estimate(s):
    """Deck ranges per preset from the configuration summaries; None where a run is missing."""
    out = {}
    need = ('p-uhd-desktop', 'p-uhd-desktop480')
    if not all(s.get(c, {}).get('n') and s[c].get('gpu_ms') for c in need):
        return out, None
    fixed, per_mp = gpu_split(s['p-uhd-desktop']['gpu_ms'], s['p-uhd-desktop480']['gpu_ms'])
    share480 = per_mp * PX[480] / s['p-uhd-desktop480']['gpu_ms'] if s['p-uhd-desktop480']['gpu_ms'] else 0.0
    model = {'fixed': fixed, 'per_mp': per_mp, 'share480': share480}
    for preset in ('desktop', 'mobile'):
        cpu, uhd, comb = s.get('e-rtx-' + preset, {}), s.get('p-uhd-' + preset, {}), s.get('e-uhd-' + preset, {})
        if not (cpu.get('n') and uhd.get('n') and uhd.get('gpu_ms')):
            continue
        g = uhd['gpu_ms']
        if preset == 'desktop':
            parts = (fixed, per_mp * PX[800])
        else:
            parts = (g * (1 - share480), g * share480)
        gap = max(0.0, cpu['gap'])
        p99r = (comb['p99_ms'] / comb['frame_ms']) if comb.get('n') else (cpu['p99_ms'] / cpu['frame_ms'])
        row = {}
        keys = ('main_nonsim', 'tick_ms', 'render_busy')
        # Worst CPU: the slower of the scaled E-core + RTX costs and the measured E-core + UHD costs (Intel's driver
        # CPU time and the iGPU sharing memory and power with the cores show in the latter).
        cpus = {'best': {k: cpu[k] * CPU_FACTOR[0] for k in keys},
                'worst': {k: max(cpu[k] * CPU_FACTOR[1], comb.get(k) or 0) for k in keys}}
        if comb.get('n'):
            # The UHD run's submit phase holds its wait for the GPU (it follows gpu_ms); count the RTX run's instead.
            busy = comb['render_busy'] - (comb['r_submit'] or 0) + (cpu['r_submit'] or 0)
            cpus['worst']['render_busy'] = max(cpu['render_busy'] * CPU_FACTOR[1], busy)
        for name, best in (('best', True), ('worst', False)):
            gpu = deck_gpu(*parts, best)
            frame, main, render = deck_frame(cpus[name], gpu, gap, 1.0)
            row[name] = {'gpu': gpu, 'main': main, 'render': render, 'frame': frame, 'fps': 1000 / frame,
                         'p99': frame * p99r}
        out[preset] = row
    return out, model


def holds(row, target_ms):
    if row['worst']['frame'] <= target_ms:
        return 'holds'
    if row['best']['frame'] <= target_ms:
        return 'plausible'
    return 'no'


def fmt(v, nd=1):
    return '-' if v is None else f'{v:.{nd}f}'


def report(out_dir, skip):
    topo = read_kv(os.path.join(out_dir, 'topology.txt'))
    print(f'topology: {open(os.path.join(out_dir, "topology.txt")).read().strip()}'
          if topo else 'topology: missing')
    exe = os.path.join(out_dir, 'exe.txt')
    if os.path.exists(exe):
        print(open(exe).read().strip())
    s, problems, notes = {}, [], []
    print(f'{"config":17} {"fps":>5} {"fps_w":>5} {"frame":>5} {"main":>5} {"nonsim":>6} {"render":>6} {"gpu":>5} '
          f'{"p99":>5} {"p99max":>6} {"gap":>5} {"tpf":>4} {"rows":>9} cam adapter')
    for c in CONFIGS:
        log = os.path.join(out_dir, c, 'stdout.log')
        if not os.path.exists(log):
            continue
        run = read_kv(os.path.join(out_dir, c, 'run.txt'))
        r = summary(log, skip)
        s[c] = r
        want_gpu ='rtx 4070' if '-rtx-' in c else 'uhd graphics 770'
        want_mask = topo.get('E_mask' if c.startswith('e-') else 'P_mask')
        if want_gpu not in r['adapter'].lower():
            problems.append(f'{c}: adapter "{r["adapter"]}" is not {want_gpu}')
        if r['backend'] and r['backend'].lower() != 'vulkan':
            problems.append(f'{c}: backend {r["backend"]}')
        if not want_mask or run.get('process_affinity', '').upper() != want_mask.upper():
            problems.append(f'{c}: affinity {run.get("process_affinity")} is not {want_mask}')
        if not r['n']:
            problems.append(f'{c}: no race windows')
            continue
        # A run cut by the deadline still counts when it covers 90 % of the race (ticks at the measured tick rate).
        want_s = num(run.get('ticks')) / statistics.median(
            x for x in [r.get('ticks_per_frame') and r['ticks_per_frame'] * r['fps']] if x) if run.get('ticks') else 0
        if not r['complete'] and r['race_s'] < 0.9 * want_s:
            problems.append(f'{c}: no drive_script_complete ({run.get("state")}), race {r["race_s"]:.0f} of {want_s:.0f} s')
        elif not r['complete']:
            notes.append(f'{c}: cut by the deadline after {r["race_s"]:.0f} of {want_s:.0f} race s')
        # An unfocused window may be frame-capped by the driver's background limit (60 fps seen with the RTX).
        if r['unfocused'] > 0.1 * r['n']:
            problems.append(f'{c}: window unfocused in {r["unfocused"]} of {r["n"]} windows')
        if r['presses'] < 4:
            problems.append(f'{c}: {r["presses"]} camera presses')
        print(f'{c:17} {fmt(r["fps"]):>5} {fmt(r["fps_w"]):>5} {fmt(r["frame_ms"]):>5} {fmt(r["main_ms"]):>5} '
              f'{fmt(r["main_nonsim"]):>6} {fmt(r["render_busy"]):>6} {fmt(r["gpu_ms"]):>5} {fmt(r["p99_ms"]):>5} '
              f'{fmt(r["p99_max"]):>6} {fmt(r["gap"]):>5} {fmt(r["ticks_per_frame"], 2):>4} {r["render"]:>9} '
              f'{r["presses"]:>3} {r["adapter"]} ts={r["tsq"]} n={r["n"]} unfocused={r["unfocused"]}')
    # Cross-adapter presentation: the UHD renders, the RTX drives the monitor; the copy is outside the GPU timestamps.
    for preset in ('desktop', 'mobile'):
        a, b = s.get('p-rtx-' + preset, {}), s.get('p-uhd-' + preset, {})
        if a.get('n') and b.get('n'):
            print(f'present {preset}: gap rtx {fmt(a["gap"])} uhd {fmt(b["gap"])} ms; r_present+acquire rtx '
                  f'{fmt((a["r_present"] or 0) + (a["acquire"] or 0))} uhd {fmt((b["r_present"] or 0) + (b["acquire"] or 0))} ms')
    est, model = estimate(s)
    if model:
        print(f'uhd gpu model: fixed {model["fixed"]:.1f} ms + {model["per_mp"]:.1f} ms/MP '
              f'(per-pixel share at 480 rows {model["share480"]:.0%})')
    verdict = []
    for preset, row in est.items():
        b, w = row['best'], row['worst']
        print(f'deck {preset}: frame {b["frame"]:.1f}..{w["frame"]:.1f} ms = {w["fps"]:.0f}..{b["fps"]:.0f} fps; '
              f'gpu {b["gpu"]:.1f}..{w["gpu"]:.1f}, main {b["main"]:.1f}..{w["main"]:.1f}, render '
              f'{b["render"]:.1f}..{w["render"]:.1f}, p99 {b["p99"]:.1f}..{w["p99"]:.1f} ms; '
              f'60 Hz {holds(row, 1000 / 60)}, 30 Hz {holds(row, 1000 / 30)}')
        verdict.append(f'{preset} {w["fps"]:.0f}-{b["fps"]:.0f} fps (60:{holds(row, 1000 / 60)} '
                       f'30:{holds(row, 1000 / 30)})')
    for n in notes:
        print('note: ' + n)
    for p in problems:
        print('problem: ' + p)
    ok = not problems and len(est) == 2
    print(f'VERDICT {"PASS" if ok else "FAIL"}: deck ' + ('; '.join(verdict) or 'no estimate')
          + f'; {len(s)} configs, {len(problems)} problems; {out_dir}')
    return 0 if ok else 1


def selftest():
    fixed, per_mp = gpu_split(30.0, 30.0 - (PX[800] - PX[480]) * 20)
    assert abs(per_mp - 20) < 1e-6 and abs(fixed - (30 - 20 * 1.024)) < 1e-6
    cpu = {'main_nonsim': 4.0, 'tick_ms': 2.0, 'render_busy': 5.0}
    frame, main, _ = deck_frame(cpu, 10.0, 1.0, 1.0)
    assert abs(frame - 11.0) < 1e-6 and abs(main - 6.0) < 1e-6, (frame, main)
    frame, main, _ = deck_frame(cpu, 40.0, 0.0, 1.0)
    assert abs(main - (4 + 2 * 2.4)) < 1e-6, main
    assert holds({'best': {'frame': 15}, 'worst': {'frame': 20}}, 1000 / 60) == 'plausible'
    assert holds({'best': {'frame': 15}, 'worst': {'frame': 16}}, 1000 / 60) == 'holds'
    m = ADAPTER.search('frame_split version=1 backend=Vulkan adapter="Intel(R) UHD Graphics 770" timestamp_query=on')
    assert m and m.group(2) == 'Intel(R) UHD Graphics 770' and m.group(3) == 'on'
    print('deck_estimate.py selftest PASS')
    return 0


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='cmd', required=True)
    r = sub.add_parser('report')
    r.add_argument('out_dir')
    r.add_argument('--skip', type=int, default=3)
    sub.add_parser('selftest')
    a = ap.parse_args()
    return report(a.out_dir, a.skip) if a.cmd == 'report' else selftest()


if __name__ == '__main__':
    sys.exit(main())
