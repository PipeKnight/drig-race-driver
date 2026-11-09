"""Graphics audit of every raceable route: an autopilot lap per route, chase and cockpit shots, a verdict table.

Usage: py -I tools/route_graphics_audit.py [--exe target/release/grid-app.exe] [--out local/route-audit/NAME]
       [--only SUBSTR,...] [--resolution 1280x720] [--fps 30] [--reuse] [--list]

Routes come from `grid-app --menu-routes OUT/profiles`: every layout the event menu offers, each with the
settings profile the menu writes for the first event type offering it at that type's defaults (field,
laps, AI, `[private] ... = derive`; Touge layouts also uphill). Per route it runs `grid-app --settings
PROFILE --drive --drive-ticks N --render DIR/data/END.png --render-fps FPS --resolution WxH --data-dir DIR/data`
offscreen with GRID_AUTOPILOT=1, GRID_AUDIO=off, GRID_SHADOW_TRACE=1 and GRID_ROUTE_AUDIT=DIR/shots
(route_audit.rs): the autopilot drives the first lap and the app saves chase and head-cam shots at fixed
lap fractions with their metrics (`route_audit_metrics`).
The default graphics are the normal playable preset (a fresh data dir has no settings.ini); the camera
starts in chase_close. The verdict flags, per shot: a black region (no channel above 4) larger than
BLOB of the picture (COCKPIT_BLOB in the head-cam, whose dash is legitimately dark), magenta (missing-texture
stand-in), a dark sky band (chase top eighth below SKY), and per route: missing shots, water draws of
land.pssg's water.fx that the scene leaves out (`omitted_water_draws`) but water.rs did not spawn, one-frame
shadow caster losses past the warm-up whose cause can change the picture (`shadow_dropout`, F745) except
those at the audit's own view cuts (`route_audit_cut`: the frames compared belong to two views; counted
apart), panics and error lines in the log. A second offscreen run per route, `--float-audit DIR/materials.tsv`
(float_audit.rs), lists the drawn parts whose material has no loaded diffuse texture (`untextured` rows,
flagged as untextured). It prints one line per route and the totals, and writes
summary.tsv and summary.md to OUT. --reuse judges existing logs without running. Pure Python.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

BLOB = 0.010
COCKPIT_BLOB = 0.060
MAGENTA = 0.001
SKY = 35.0
# shadow_cull.rs: one-frame caster losses past the pipeline warm-up; port_cull (the slice boundary) and
# frustum (outside the cascade volume) cannot change sampled texels (F745, tools/shadow_dropouts.py).
WARMUP_FRAMES = 200
BENIGN_DROPOUTS = ('port_cull', 'frustum')
# A dropout at an audit view cut (`route_audit_cut frame=C`) compares frames of two views: frames
# C-1..C+2 (the new view is placed at C+1; one frame of slack each side for the schedule; AUTHORED).
CUT_WINDOW = (-1, 2)
# ornament_lod.rs drops a track ornament's shadow level (`ornament/1/ID`) beyond its cull distance D, as the
# original does (0x96EC80): while the instance crosses D the part hands its shadow to a fading proxy entity
# (`not_caster`, shadow_proxies, F713) and is then hidden (`hidden`). Both are counted apart
# (`ornament_culls`), not as dropouts.
# Source tree parts (`trees/ID`) are ornament levels too (source_trees.rs).
ORNAMENT = re.compile(r'^(ornament/\d+|trees)/')


def menu_routes(exe, profiles):
    out = subprocess.run([exe, '--menu-routes', str(profiles)], capture_output=True, text=True, timeout=300)
    if out.returncode != 0:
        sys.exit(f'FAIL: --menu-routes exit {out.returncode}: {out.stderr.strip()[-400:]}')
    routes, venues = [], {}
    for line in out.stdout.splitlines():
        m = re.match(r'menu_route route=(\S+) venue="(.*)" events=(\S+)', line)
        if m:
            venues[m.group(1)] = (m.group(2), m.group(3).split(','))
        m = re.match(r'menu_route_profile route=(\S+) type=(\S+) car=(\S+) subroute=(\d+) path=(.+)$', line)
        if m:
            route = m.group(1)
            venue, events = venues.get(route, ('', []))
            routes.append({'route': route, 'venue': venue, 'events': [e.split(':')[0] for e in events],
                           'type': m.group(2), 'car': m.group(3), 'subroute': int(m.group(4)),
                           'profile': m.group(5).strip()})
    return routes


def slug(entry):
    s = entry['route'].removeprefix('tracks/').replace('/', '-')
    return s + ('-up' if entry['subroute'] else '')


def run(exe, entry, folder, args):
    folder.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, GRID_AUTOPILOT='1', GRID_AUDIO='off', GRID_SHADOW_TRACE='1',
               GRID_ROUTE_AUDIT=str(folder / 'shots'))
    cmd = [exe, '--settings', entry['profile'], '--drive', '--drive-ticks', str(args.ticks),
           '--render', str(folder / 'data' / 'end.png'), '--render-fps', str(args.fps), '--resolution', args.resolution,
           '--data-dir', str(folder / 'data')]
    (folder / 'data').mkdir(exist_ok=True)
    # A rerun starts clean: the app refuses to overwrite a render.
    for old in [folder / 'data' / 'end.png', folder / 'materials.tsv', *(folder / 'shots').glob('*.png')]:
        old.unlink(missing_ok=True)
    started = time.time()
    with open(folder / 'run.log', 'w', encoding='utf-8', errors='replace') as log:
        log.write('cmd ' + ' '.join(cmd) + '\n')
        log.flush()
        try:
            code = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, env=env, timeout=args.timeout).returncode
        except subprocess.TimeoutExpired:
            code = 'timeout'
        log.write(f'audit_exit code={code} seconds={time.time() - started:.0f}\n')
    materials = [exe, '--route', entry['route'], '--car', entry['car'], '--scene', 'donington',
                 '--float-audit', str(folder / 'materials.tsv'), '--data-dir', str(folder / 'data')]
    with open(folder / 'materials.log', 'w', encoding='utf-8', errors='replace') as log:
        log.write('cmd ' + ' '.join(materials) + '\n')
        log.flush()
        try:
            subprocess.run(materials, stdout=log, stderr=subprocess.STDOUT, env=env, timeout=600)
        except subprocess.TimeoutExpired:
            log.write('materials timeout\n')


def judge(entry, folder):
    path = folder / 'run.log'
    if not path.exists():
        return {'route': slug(entry), 'verdict': 'NOT RUN', 'flags': ['no log'], 'shots': 0}
    text = path.read_text(encoding='utf-8', errors='replace')
    flags, shots, worst = [], 0, (0.0, '')
    min_sky = 999.0
    # A night route (drive_environment night=true: Haruna's night routes, forced-night events) has a
    # dark sky: a black region touching the top edge and a dark top band are the sky, not a defect.
    night = re.search(r'^drive_environment .*\bnight=true', text, re.M) is not None
    for m in re.finditer(r'route_audit_metrics path=(\S+) black=(\S+) dark=(\S+) black_blob=(\S+) magenta=(\S+) '
                         r'mean=(\S+) top=(\S+) blob_box=(\S+)', text):
        shots += 1
        name = Path(m.group(1)).stem
        blob, magenta, top = float(m.group(4)), float(m.group(5)), float(m.group(7))
        cockpit = name.endswith('cockpit')
        sky_blob = night and m.group(8).split(',')[1] == '0'
        if blob > (COCKPIT_BLOB if cockpit else BLOB) and not sky_blob:
            flags.append(f'black {name} {blob:.3f} box={m.group(8)}')
        if blob > worst[0]:
            worst = (blob, name)
        if magenta > MAGENTA:
            flags.append(f'magenta {name} {magenta:.4f}')
        if not cockpit:
            min_sky = min(min_sky, top)
            if top < SKY and not night:
                flags.append(f'dark-top {name} {top:.0f}')
    expected = 10
    # Coverage notes, not picture defects: the autopilot wrecked the player's car (the race ends
    # before the last shots), or the car never moved (route_audit.rs falls back to the clock).
    notes = ['night'] if night else []
    if re.search(r'^drive_race_result .*\bYou\s+Wrecked', text, re.M):
        notes.append('wrecked')
    if re.search(r'^route_audit_stall ', text, re.M):
        notes.append('stalled')
    if shots < expected:
        (notes if 'wrecked' in notes else flags).append(f'shots {shots}/{expected}')
    causes, at_cut, culls = {}, 0, 0
    cuts = [int(c) for c in re.findall(r'^route_audit_cut frame=(\d+)', text, re.M)]
    for m in re.finditer(r'^shadow_dropout frame=(\d+) cascade=\d+ cause=(\S+) entity=\S+ name="([^"]*)"', text, re.M):
        frame, cause = int(m.group(1)), m.group(2).split('+')[0]
        if frame <= WARMUP_FRAMES or cause in BENIGN_DROPOUTS:
            continue
        if cause in ('hidden', 'not_caster') and ORNAMENT.match(m.group(3)):
            culls += 1
        elif any(CUT_WINDOW[0] <= frame - c <= CUT_WINDOW[1] for c in cuts):
            at_cut += 1
        else:
            causes[cause] = causes.get(cause, 0) + 1
    if causes:
        flags.append('shadow_dropout ' + ','.join(f'{k}={v}' for k, v in sorted(causes.items())))
    # One scene line per loaded scene (track parts, land.pssg, cars): the water draws are their sum.
    omitted = [int(n) for n in re.findall(r'omitted_water_draws=(\d+)', text)]
    spawned = sum(int(n) for n in re.findall(r'^water spawned draws=(\d+)', text, re.M))
    water = f'{spawned}/{sum(omitted) if omitted else "?"}'
    if omitted and sum(omitted) != spawned:
        flags.append(f'water drawn {water}')
    panics = len(re.findall(r'panicked', text))
    errors = [l for l in text.splitlines() if re.search(r'\bERROR\b', l)]
    if panics:
        flags.append(f'panic {panics}')
    if errors:
        flags.append(f'errors {len(errors)}: {errors[0][-120:]}')
    materials = folder / 'materials.tsv'
    if materials.exists():
        untextured = [l.split('	')[2] for l in materials.read_text(encoding='utf-8', errors='replace').splitlines()
                      if l.startswith('untextured	')]
        if untextured:
            flags.append(f'untextured {len(untextured)}: {",".join(untextured[:6])}')
    else:
        flags.append('no materials.tsv')
    exit_m = re.search(r'audit_exit code=(\S+) seconds=(\S+)', text)
    return {'route': slug(entry), 'verdict': 'FLAG' if flags else 'PASS', 'flags': flags, 'shots': shots,
            'worst': f'{worst[0]:.3f} {worst[1]}', 'water': water, 'cut_dropouts': at_cut, 'ornament_culls': culls, 'sky': f'{min_sky:.0f}' if min_sky < 999 else '-',
            'exit': exit_m.group(1) if exit_m else '?', 'seconds': exit_m.group(2) if exit_m else '?',
            'notes': ','.join(notes) or '-'}


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument('--exe', default='target/release/grid-app.exe')
    p.add_argument('--out', default=f'local/route-audit/{time.strftime("%Y%m%d-%H%M")}')
    p.add_argument('--only', default='')
    p.add_argument('--resolution', default='1280x720')
    p.add_argument('--fps', type=int, default=30, help='--render-fps (30..300)')
    p.add_argument('--ticks', type=int, default=25000, help='upper bound of the drive (400 s at 0.016 s)')
    p.add_argument('--timeout', type=int, default=1500)
    p.add_argument('--reuse', action='store_true')
    p.add_argument('--list', action='store_true')
    args = p.parse_args()
    args.exe = str(Path(args.exe).resolve())
    out = Path(args.out)
    profiles = out / 'profiles'
    if args.list:
        profiles = Path(tempfile.mkdtemp(prefix='route-audit-'))
    routes = menu_routes(args.exe, profiles)
    if args.only:
        wanted = args.only.split(',')
        routes = [r for r in routes if any(w in slug(r) for w in wanted)]
    if args.list:
        for r in routes:
            print(slug(r), r['type'], r['car'], ','.join(r['events']))
        print(f'routes={len(routes)}')
        return
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for i, entry in enumerate(routes):
        folder = out / slug(entry)
        if not (args.reuse and (folder / 'run.log').exists()):
            run(args.exe, entry, folder, args)
        r = judge(entry, folder)
        results.append(r)
        print(f'[{i + 1}/{len(routes)}] {r["verdict"]:5} {r["route"]:42} shots={r["shots"]} worst_blob={r.get("worst")} '
              f'min_sky={r.get("sky")} water={r.get("water")} exit={r.get("exit")} t={r.get("seconds")}s'
              + (f' notes={r["notes"]}' if r.get('notes', '-') != '-' else '')
              + (f' | {"; ".join(r["flags"])[:300]}' if r['flags'] else ''), flush=True)
    keys = ('route', 'verdict', 'shots', 'worst', 'sky', 'water', 'cut_dropouts', 'ornament_culls', 'notes', 'exit', 'seconds')
    rows = ['\t'.join(keys) + '\tflags']
    md = ['| Route | Verdict | Shots | Worst black region | Min sky | Water drawn/source | Dropouts at a cut | Ornament culls | Notes | Flags |',
          '|---|---|---|---|---|---|---|---|---|---|']
    for r in results:
        rows.append('\t'.join(str(r.get(k, '')) for k in keys) + '\t' + '; '.join(r['flags']))
        md.append(f'| {r["route"]} | {r["verdict"]} | {r["shots"]} | {r.get("worst", "")} | {r.get("sky", "")} | '
                  f'{r.get("water", "")} | {r.get("cut_dropouts", "")} | {r.get("ornament_culls", "")} | {r.get("notes", "")} | '
                  f'{"; ".join(r["flags"]).replace("|", "/")} |')
    (out / 'summary.tsv').write_text('\n'.join(rows) + '\n', encoding='utf-8')
    (out / 'summary.md').write_text('\n'.join(md) + '\n', encoding='utf-8')
    flagged = sum(r['verdict'] != 'PASS' for r in results)
    print(f'route_graphics_audit routes={len(results)} pass={len(results) - flagged} flagged={flagged} '
          f'summary={out / "summary.tsv"}')


if __name__ == '__main__':
    main()
