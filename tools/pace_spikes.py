"""Refresh pacing and spikes in a frame trace (one log with frame_stats and refresh_pace lines): pace changes with the 3 windows before each, per-pace medians, top spike windows with the worst render frame split and the events between windows. Usage: py -I tools/pace_spikes.py LOG..."""
import re, sys, statistics as st
from collections import Counter
NUM = re.compile(r'(\w+)=(-?[0-9.]+)(?=\s|$)')
QUIET = ('drive_sample', 'drive_lap_sample', 'drive_handling_sample', 'drive_deformation_sample',
         'drive_input', 'tyre_smoke', 'frame_gpu_views', 'pipeline_user', 'smoke_', 'crowd_')
RW = 'tot meshes views acq queue sort prep bind enc submit present clean'.split()

def p90(hist):
    tot = sum(c for _, c in hist); acc = 0
    for b, c in sorted(hist):
        acc += c
        if acc >= 0.9 * tot: return b
    return float('nan')

for path in sys.argv[1:]:
    lines = open(path, encoding='utf-8', errors='replace').read().splitlines()
    wins, events, phase = [], [], 'load'
    for i, l in enumerate(lines):
        k = l.split(' ', 1)[0]
        if l.startswith('screen_transition') and 'reason=race_start' in l: phase = 'count'
        elif 'remaining=GO' in l: phase = 'race'
        elif k == 'drive_script_complete': phase = 'end'
        if k == 'frame_stats':
            d = {a: float(b) for a, b in NUM.findall(l)}
            m = re.search(r'ft_hist=(\S+)', l)
            d['hist'] = [(float(a), int(b)) for a, b in (x.split(':') for x in m.group(1).split(','))] if m else []
            m = re.search(r'rworst=(\S+)', l)
            d['rw'] = [float(x) for x in m.group(1).split('/')] if m else []
            m = re.search(r'pace_hz=(\S+)', l); d['pace'] = m.group(1) if m else '-'
            d['line'], d['phase'], d['n'] = i, phase, len(wins)
            wins.append(d)
        elif not k.startswith(QUIET):
            events.append((i, k, l))
    print(f"== {path.replace(chr(92), '/').split('/')[-1][:34]}  windows {len(wins)}")
    race = [w for w in wins if w['phase'] == 'race']
    # 1. pacing: what made each change.
    print(' pace decisions (window n: 3 windows before -> pace, interval p90, acquire, main, render CPU, outside, max):')
    for i, k, l in events:
        if k == 'refresh_pace' and 'pace_hz=' in l:
            w = [x for x in wins if x['line'] < i][-3:]
            print('  ' + re.sub(r'display_hz=\S+ |cap_hz=\S+', '', l[13:]).strip())
            for x in w:
                print(f"     n={x['n']:3d} {x['phase']:5s} pace={x['pace']:5s} p90={p90(x['hist']):5.1f} acq={x.get('acquire',0):5.2f} main={x.get('main_ms',0):5.2f} rcpu={x.get('render_ms',0)-x.get('acquire',0):5.2f} out={x.get('outside_ms',0):5.2f} max={x.get('max_ms',0):5.1f} ivcsw={x.get('ivcsw',0):.0f} cpu={x.get('cpu_load',0):.2f}")
    pc = Counter(w['pace'] for w in race)
    print(' race windows by pace:', dict(pc))
    for pace in pc:
        R = [w for w in race if w['pace'] == pace]
        g = lambda k: st.median([w.get(k, 0) for w in R])
        print(f"  {pace:5s} med fps={g('fps'):.1f} p90int={st.median([p90(w['hist']) for w in R]):.1f} main={g('main_ms'):.2f} outside={g('outside_ms'):.2f} render={g('render_ms'):.2f} acq={g('acquire'):.2f} gpu={g('gpu_ms'):.2f} >=20ms={100*sum(c for w in R for b,c in w['hist'] if b>=20)/max(1,sum(c for w in R for b,c in w['hist'])):.1f}%")
    # 2. spikes.
    top = sorted([w for w in wins if w['n'] >= 6 and w['phase'] in ('race', 'count')], key=lambda w: -w.get('max_ms', 0))[:8]
    print(' top spike windows (max frame) with the worst render frame split and events since the previous window:')
    for w in top:
        rw = ' '.join(f'{a}={b}' for a, b in zip(RW, w['rw']) if b >= 1.0)
        prev = wins[w['n'] - 1]['line'] if w['n'] else 0
        ev = Counter(k for i, k, l in events if prev < i < w['line'])
        print(f"  n={w['n']:3d} {w['phase']} pace={w['pace']} max={w['max_ms']:.1f} hist>=20:{','.join(f'{b:g}:{c}' for b,c in w['hist'] if b>=20)} main={w['main_ms']:.2f} fixed_max={w.get('fixed_max_ms',0):.1f} out={w['outside_ms']:.2f} gpu={w.get('gpu_ms',0):.2f} rworst[{rw}] slow14 n={w.get('slow14_frames',0):.0f} main={w.get('slow14_main',0):.1f} fixed={w.get('slow14_fixed',0):.1f} out={w.get('slow14_outside',0):.1f} render={w.get('slow14_render',0):.1f} ticks={w.get('slow14_ticks',0):.1f} img_kb_s={w.get('img_kb_s',0):.0f} mesh_up={w.get('mesh_kb_s',0):.0f} faults={w.get('faults',0):.0f} ivcsw={w.get('ivcsw',0):.0f}")
        print('     events:', dict(ev) if ev else '-')
        for i, k, l in events:
            if prev < i < w['line'] and k in ('pipeline_new', 'asset_churn', 'transform_burst', 'drive_camera', 'car_batch', 'texture', 'render_scale', 'render_dynamic', 'loading_stats', 'drive_lap_event', 'speech_audio', 'drive_damage_part', 'opponent_dents'):
                print('       ' + l[:170])
