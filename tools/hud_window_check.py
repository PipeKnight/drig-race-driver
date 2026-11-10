"""Interactive window check of the race OSD after a window resize mid-race (F737, F742): the OSD and the smoke's reduced depth must follow the new size.

Usage: py -I tools/hud_window_check.py EXE SETTINGS OUT_DIR [--data-dir local] [--resize-at S]
       [--size WxH] [--shots S,S,...] [--expect-stale] [--expect-splits]
       [--expect-penalty [--contact-at S]] [--expect-brake] [--expect-drift]

Starts EXE silently (GRID_AUDIO=off) with the autopilot (`--drive-ticks`, GRID_AUTOPILOT=1) in a
1280x800 window, the interactive branch with tick interpolation forced on (GRID_TICK_INTERPOLATION=1). RESIZE_AT seconds after launch it resizes
the game's own window (found by its process id; SetWindowPos with SWP_NOACTIVATE, so no focus
change and no keys) to WxH, 2560x1369 by default (a maximised 2560x1440 desktop).

Verdict from the game's `osd_display bound=B size=WxH` lines (osd_scene.rs `check_display`, printed
when the binding or the OSD texture size changes): PASS when the last line has the new size and
`bound=true`, that is the UI material that shows the OSD binds the resized texture. Before F737 the
resize left `bound=false`: the screen kept the old texture, so the dial, timers and minimap stood
still. `--expect-stale` inverts the verdict (to show that the check catches the defect).

GRID_WINDOW_SHOTS (frame_pacing.rs) also saves window shots at the --shots game times (real time
after loading) for a look: by default two before and two after the resize, two seconds apart. The
log, the shots and `result.txt` stay in OUT_DIR. The settings file must start a race with the OSD
(`[game] route`, `[drive] enabled = on`). Windows only.

--expect-splits (F763) also requires the split pills in the interactive window: an `osd_splits`
line (race_osd.rs, printed when the shown pills change) with a race clock after the resize, and
one with a signed difference. Choose a race with opponents and a resize before the first split.

--expect-penalty also requires the touge penalty pill: an `osd_penalty` line (race_osd.rs, printed
when the pill is raised) anywhere in the run. Use a pro touge settings file. The autopilot need not
touch the other car, so the check sets GRID_SMOKE_CONTACT (touge_event.rs): one player contact
counted --contact-at seconds after GO (default 30), through the normal penalty path.

--expect-brake (F770) also requires the excessive-speed brake light's table: a `speed_advice ...
loaded=true` line. `osd_brake` lines (printed when the light starts flashing) are counted but not
required, since the autopilot need not overspeed. Use a circuit whose route ships class tables.

--expect-drift also requires the drift OSD groups (drift_osd.rs): an `osd_drift open=true` line (a
combo opened, printed on each combo change) after the resize, with the multiplier's digit scroll
(`scroll=` units of the double, units and tens of the treble). Use a drift settings file.
"""
import argparse
import ctypes
import os
import re
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path


def game_window(pid):
    user32 = ctypes.windll.user32
    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def each(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
        return True

    user32.EnumWindows(each, 0)
    return found[0] if found else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('exe')
    ap.add_argument('settings')
    ap.add_argument('out')
    ap.add_argument('--data-dir', default='local')
    ap.add_argument('--resize-at', type=float, default=45.)
    ap.add_argument('--size', default='2560x1369')
    ap.add_argument('--shots', default='20,22,40,42')
    ap.add_argument('--expect-stale', action='store_true')
    ap.add_argument('--expect-splits', action='store_true')
    ap.add_argument('--expect-penalty', action='store_true')
    ap.add_argument('--contact-at', type=float, default=30.)
    ap.add_argument('--expect-brake', action='store_true')
    ap.add_argument('--expect-drift', action='store_true')
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    data = Path(args.data_dir)
    shots = [float(s) for s in args.shots.split(',') if s.strip()]
    names = [f'window-shot-{s:05.1f}.png' for s in shots]
    for old in data.glob('window-shot-*.png'):
        old.unlink()
    w, h = (int(v) for v in args.size.split('x'))
    env = dict(os.environ, GRID_AUDIO='off', GRID_AUTOPILOT='1', GRID_TICK_INTERPOLATION='1', GRID_WINDOW_SHOTS=args.shots)
    if args.expect_penalty:
        env['GRID_SMOKE_CONTACT'] = str(args.contact_at)
    log_path = out / 'grid-app.log'
    log = open(log_path, 'w')
    proc = subprocess.Popen([str(Path(args.exe).resolve()), '--data-dir', str(data), '--settings', args.settings,
                             '--resolution', '1280x800', '--drive-ticks', '72000'],
                            stdout=log, stderr=subprocess.STDOUT, env=env)
    start = time.time()
    resized = False
    try:
        while time.time() - start < args.resize_at and proc.poll() is None:
            time.sleep(0.2)
        hwnd = game_window(proc.pid)
        resized = bool(hwnd) and bool(ctypes.windll.user32.SetWindowPos(hwnd, None, 0, 0, w, h, 0x0004 | 0x0010))
        print(f'resize to {w}x{h} at {time.time() - start:.1f} s: {"ok" if resized else "FAILED"}', flush=True)
        while time.time() - start < args.resize_at + 90 and proc.poll() is None:
            if all((data / n).exists() for n in names):
                time.sleep(2)  # the last file finishes writing; a few more frames run
                break
            time.sleep(0.5)
    finally:
        proc.kill()
        log.close()
    for n in names:
        if (data / n).exists():
            (data / n).replace(out / n)
    lines = re.findall(r'^osd_display bound=(\w+) size=(\d+)x(\d+)', log_path.read_text(errors='replace'), re.M)
    for b, x, y in lines:
        print(f'osd_display bound={b} size={x}x{y}')
    last = lines[-1] if lines else None
    resized_shown = last is not None and (int(last[1]), int(last[2])) != (1280, 800)
    stale = last is not None and last[0] == 'false'
    if not resized or not resized_shown:
        verdict = 'FAIL: no osd_display line at the new size (no OSD, no resize, or an exe without the check)'
    elif stale != args.expect_stale:
        verdict = f'FAIL: OSD {"stale" if stale else "bound"} after the resize'
    else:
        verdict = f'PASS: OSD {"stale (as expected)" if stale else "bound to the resized texture"} at {last[1]}x{last[2]}'
    # F742: the reduced smoke depth follows the resize and every smoke material is marked to rebind it.
    smoke = re.findall(r'^smoke_depth size=(\d+)x(\d+) rebound=(\d+)', log_path.read_text(errors='replace'), re.M)
    for x, y, n in smoke:
        print(f'smoke_depth size={x}x{y} rebound={n}')
    if verdict.startswith('PASS') and not args.expect_stale:
        if len(smoke) < 2:
            verdict = 'FAIL: no smoke_depth line after the resize (an exe without the check?)'
        elif int(smoke[-1][2]) == 0:
            verdict = f'FAIL: smoke depth resized to {smoke[-1][0]}x{smoke[-1][1]} but no smoke material rebound'
        else:
            verdict += f'; smoke depth {smoke[-1][0]}x{smoke[-1][1]} rebound by {smoke[-1][2]} materials'
    # F763: split pills (leader's race clock, the others' signed differences).
    text = log_path.read_text(errors='replace')
    # The first OSD line at the new size (the client area, smaller than the window).
    moved = [m for m in re.finditer(r'^osd_display bound=true size=(\d+)x(\d+)', text, re.M)
             if (int(m.group(1)), int(m.group(2))) != (1280, 800)]
    at = moved[0].start() if moved else -1
    splits = re.findall(r'^osd_splits shown=(\d+) rows=(.*)$', text[max(at, 0):], re.M)
    clock = [r for n, r in splits if re.search(r':\d\d:\d\d\.\d\d', r)]
    signed = [r for r in clock if re.search(r':[+-]\d\d:\d\d\.\d\d', r)]
    print(f'osd_splits after the resize: {len(splits)} lines, {len(clock)} with a time, {len(signed)} with a difference'
          + (f'; first: {clock[0]}' if clock else ''))
    if args.expect_splits and verdict.startswith('PASS'):
        if at < 0 or not clock or not signed:
            verdict = 'FAIL: no split pills with a race clock and a difference after the resize'
        else:
            verdict += f'; {len(clock)} split pill updates'
    pills = re.findall(r'^osd_penalty (.*)$', text, re.M)
    print(f'osd_penalty: {len(pills)} lines {pills[:4]}')
    if args.expect_penalty and verdict.startswith('PASS'):
        if not pills:
            verdict = 'FAIL: no penalty pill raised'
        else:
            verdict += f'; {len(pills)} penalty pills'
    table = re.findall(r'^speed_advice version=1 event=table (.*)$', text, re.M)
    brakes = re.findall(r'^osd_brake (.*)$', text, re.M)
    print(f'speed_advice: {table[:1]}; osd_brake: {len(brakes)} lines {brakes[:4]}')
    if args.expect_brake and verdict.startswith('PASS'):
        if not any('loaded=true' in t for t in table):
            verdict = 'FAIL: no brake-light speed table loaded'
        else:
            verdict += f'; brake table loaded, {len(brakes)} flashing starts'
    drifts = re.findall(r'^osd_drift (.*)$', text[max(at, 0):], re.M)
    opened = [d for d in drifts if 'open=true' in d]
    print(f'osd_drift after the resize: {len(drifts)} lines, {len(opened)} open; {drifts[:4]}')
    if args.expect_drift and verdict.startswith('PASS'):
        if at < 0 or not opened:
            verdict = 'FAIL: no drift combo opened after the resize'
        else:
            verdict += f'; {len(opened)} drift combo updates'
    missing = [n for n in names if not (out / n).exists()]
    if missing:
        verdict += f' (shots missing: {", ".join(missing)})'
    print(verdict)
    (out / 'result.txt').write_text(verdict + '\n')
    sys.exit(0 if verdict.startswith('PASS') else 1)


if __name__ == '__main__':
    main()
