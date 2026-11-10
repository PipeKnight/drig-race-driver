"""Interactive window check of the post-race results screen in the front-end art (row 69, docs/RACE_HUD.md section 6.7).

Usage: py -I tools/results_window_check.py EXE OUT_DIR [--data-dir local] [--art-off]
       [--shot-after S] [--laps N] [--opponents N] [--route ROUTE] [--discipline D] [--timeout S]
       [--follow S]

Starts EXE silently (GRID_AUDIO=off, sound volumes 0) in a 1280x800 window with the autopilot
(GRID_AUTOPILOT=1, `--drive-ticks`) on a short race written to OUT_DIR/race.ini. When the car
finishes, GRID_RESULTS_CONTINUE leaves the stats screen; on the results screen the diagnostics
`results_shot` (results_art.rs) saves `window-shot-results.png` SHOT_AFTER seconds after the board
first shows with the art loaded, and prints `results_shot path=... art=on|off rows=N`. The script
then stops the game and copies the shot to OUT_DIR.

Verdict: PASS when the shot exists and, with the art on, a `results_art board rows=N drift=D
drawn=K` line (printed when the art sets up a board) has K > 0 items drawn. `--art-off` runs the
experimental `results_art_off` diagnostics (the plain panel) for the "before" side of a matched
pair; it needs only the shot. `--discipline` (settings `[drive] discipline`, e.g. drift_battle on a
route such as tracks/europe/nurburgring/route_1) shows the drift scoreboard first; the board line
then ends in `screen=DriftScoreboard`. With the art on, each screen's slide-in must also finish:
a `results_art enter screen=X seconds=S done` line with S > 0 (the `enter` event of its own
animation, 0.4 s); a small SHOT_AFTER such as 0.25 shoots the screen mid-slide. `--follow S` keeps
the game running S seconds after the shot, so GRID_RESULTS_CONTINUE (SHOT_AFTER + 6 s) leaves the
screen, and requires a `results_art exit screen=X ... done` line and a second enter: with a drift
discipline the scoreboard slides out and the results slide in. The log and
`result.txt` stay in OUT_DIR. Windows only.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('exe')
    ap.add_argument('out')
    ap.add_argument('--data-dir', default='local')
    ap.add_argument('--art-off', action='store_true')
    ap.add_argument('--shot-after', type=float, default=4.)
    ap.add_argument('--laps', type=int, default=1)
    ap.add_argument('--opponents', type=int, default=7)
    ap.add_argument('--route')
    ap.add_argument('--discipline')
    ap.add_argument('--timeout', type=float, default=900.)
    ap.add_argument('--follow', type=float, default=0.,
                    help='keep running this long after the shot (GRID_RESULTS_CONTINUE leaves the screen) '
                         'and require the exit of the leaving screen and the enter of the next')
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    data = Path(args.data_dir)
    data.mkdir(parents=True, exist_ok=True)
    shot = data / 'window-shot-results.png'
    if shot.exists():
        shot.unlink()
    settings = data / 'settings.ini'
    if not settings.exists():
        settings.write_text('[sound]\neffects = 0\nspeech = 0\nmusic = 0\n')
    race = out / 'race.ini'
    race.write_text((f'[game]\nroute = {args.route}\n' if args.route else '')
                    + f'[drive]\nenabled = on\nlaps = {args.laps}\nopponents = {args.opponents}\n'
                    + (f'discipline = {args.discipline}\n' if args.discipline else '')
                    + 'grid_position = back\n'
                    # Opponents need the derived body contacts (as tools/menu_loop_smoke.ps1).
                    + '[private]\nbody_contacts = derive\nbody_events = derive\ndeformation = derive\n')
    env = dict(os.environ, GRID_AUDIO='off', GRID_AUTOPILOT='1', GRID_RESULTS_SHOT=str(args.shot_after),
               GRID_RESULTS_CONTINUE=str(args.shot_after + 6.))
    env.pop('GRID_RESULTS_ART_OFF', None)
    if args.art_off:
        env['GRID_RESULTS_ART_OFF'] = '1'
    log_path = out / 'grid-app.log'
    with open(log_path, 'w') as log:
        proc = subprocess.Popen([str(Path(args.exe).resolve()), '--data-dir', str(data), '--settings', str(race),
                                 '--resolution', '1280x800', '--drive-ticks', '72000'],
                                stdout=log, stderr=subprocess.STDOUT, env=env)
        start = time.time()
        saved_at = None
        try:
            while time.time() - start < args.timeout and proc.poll() is None:
                time.sleep(0.5)
                if saved_at is None and 'results_shot path=' in log_path.read_text(errors='replace'):
                    saved_at = time.time()
                # The shot is written a frame or two after the line; give it a moment.
                if saved_at is not None and shot.exists() and time.time() - saved_at > max(2., args.follow):
                    break
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(10)
                except subprocess.TimeoutExpired:
                    proc.kill()
    text = log_path.read_text(errors='replace')
    shots = re.findall(r'results_shot path=\S+ art=(\w+) rows=(\d+)', text)
    boards = [(int(r), d, int(k), sc) for r, d, k, sc in
              re.findall(r'results_art board rows=(\d+) drift=(\w+) drawn=(\d+)(?: screen=(\w+))?', text)]
    enters = [(sc, float(t)) for sc, t in re.findall(r'results_art enter screen=(\w+) seconds=([\d.]+) done', text)]
    exits = [(sc, float(t)) for sc, t in re.findall(r'results_art exit screen=(\w+) seconds=([\d.]+) done', text)]
    if shot.exists():
        shutil.copy2(shot, out / 'window-shot-results.png')
    problems = []
    if not shots:
        problems.append('no results_shot line (did the race finish?)')
    if not shot.exists():
        problems.append('no window-shot-results.png')
    if not args.art_off and not any(b[2] > 0 for b in boards):
        problems.append('no results_art board line with drawn > 0')
    if not args.art_off and not any(t > 0 for _, t in enters):
        problems.append('no results_art enter line with seconds > 0 (the slide-in did not run)')
    if args.follow and not args.art_off and not (exits and len(enters) > 1):
        problems.append('no results_art exit followed by a second enter (the screen change did not slide)')
    if args.art_off and boards:
        problems.append('art drawn although results_art_off')
    verdict = 'PASS' if not problems else 'FAIL: ' + '; '.join(problems)
    summary = (f'{verdict}\nshot={shots[-1] if shots else None} boards={boards[-3:]} enters={enters[-3:]} exits={exits[-3:]} '
               f'elapsed_s={time.time() - start:.0f}\nlog={log_path}\n')
    (out / 'result.txt').write_text(summary)
    print(summary, end='')
    sys.exit(0 if not problems else 1)


if __name__ == '__main__':
    main()
