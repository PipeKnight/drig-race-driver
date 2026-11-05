"""Interactive window check of the race OSD after a window resize mid-race (F737): the screen must show the OSD image's current texture.

Usage: py -I tools/hud_window_check.py EXE SETTINGS OUT_DIR [--data-dir local] [--resize-at S]
       [--size WxH] [--shots S,S,...] [--expect-stale]

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
    missing = [n for n in names if not (out / n).exists()]
    if missing:
        verdict += f' (shots missing: {", ".join(missing)})'
    print(verdict)
    (out / 'result.txt').write_text(verdict + '\n')
    sys.exit(0 if verdict.startswith('PASS') else 1)


if __name__ == '__main__':
    main()
