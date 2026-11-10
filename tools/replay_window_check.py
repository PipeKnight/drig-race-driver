"""Interactive window check of the replay and Flashback: records a race, plays it back with the replayed effects, jumps in and races on (F-entry of the replay effects).

Usage: py -I tools/replay_window_check.py EXE SETTINGS OUT_DIR [--data-dir local] [--drive-ticks 900]
       [--replay S] [--flashback S] [--slow] [--timeout S] [--shots S,S,...]

Starts EXE silently (GRID_AUDIO=off) with the autopilot (`--drive-ticks`, GRID_AUTOPILOT=1) in a
1280x800 window, the interactive branch with tick interpolation forced on (GRID_TICK_INTERPOLATION=1).
When the scripted drive ends, diagnostics `replay_capture` starts an Instant Replay and
`flashback_capture` jumps in FLASHBACK seconds into it; the race goes on from the jump-in frame and
reaches the script's end again (a second Instant Replay). `--slow` (GRID_REPLAY_SLOW=1) plays every
replay in slow motion. No key presses, no focus change.

Verdict from the game's log lines:
- `drive_race_replay version=1 event=start` (and `slow=true` with --slow);
- `replay_effects version=1 event=start`: the race's skid strips, debris and smoke when the replay began;
- `drive_flashback version=1 event=jump_in`, `event=restored` and `event=resume`;
- `replay_effects version=1 event=adopt` at the resume tick, after it stepped the effects from the
  recorded cars (`stepped` > 0): the abandoned future's effects are gone;
- the race goes on: a second `drive_race_replay event=start` at a later tick;
- no panic.
PASS when all hold. The log and `result.txt` stay in OUT_DIR; `--shots` saves window shots at those
game times (GRID_WINDOW_SHOTS, real seconds after loading). Windows only.
"""
import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path


def fields(line):
    return dict(re.findall(r'(\w+)=(\S+)', line))


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('exe')
    ap.add_argument('settings')
    ap.add_argument('out')
    ap.add_argument('--data-dir', default='local')
    ap.add_argument('--drive-ticks', type=int, default=900)
    ap.add_argument('--replay', type=float, default=4.)
    ap.add_argument('--flashback', type=float, default=1.)
    ap.add_argument('--slow', action='store_true')
    ap.add_argument('--timeout', type=float, default=300.)
    ap.add_argument('--shots', default='')
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    data = Path(args.data_dir)
    shots = [float(s) for s in args.shots.split(',') if s.strip()]
    names = [f'window-shot-{s:05.1f}.png' for s in shots]
    for old in data.glob('window-shot-*.png'):
        old.unlink()
    env = dict(os.environ, GRID_AUDIO='off', GRID_AUTOPILOT='1', GRID_TICK_INTERPOLATION='1',
               GRID_REPLAY_CAPTURE=str(args.replay), GRID_FLASHBACK_CAPTURE=str(args.flashback))
    if args.slow:
        env['GRID_REPLAY_SLOW'] = '1'
    if shots:
        env['GRID_WINDOW_SHOTS'] = args.shots
    log_path = out / 'grid-app.log'
    log = open(log_path, 'w')
    proc = subprocess.Popen([str(Path(args.exe).resolve()), '--data-dir', str(data), '--settings', args.settings,
                             '--resolution', '1280x800', '--drive-ticks', str(args.drive_ticks)],
                            stdout=log, stderr=subprocess.STDOUT, env=env)
    start = time.time()

    def done(text):
        starts = len(re.findall(r'^drive_race_replay version=1 event=start', text, re.M))
        return (starts >= 2 and all((data / n).exists() for n in names)) or 'panicked' in text

    try:
        # One check a second of the game's own log file (no output to the console).
        while time.time() - start < args.timeout and proc.poll() is None:
            if done(log_path.read_text(errors='replace')):
                time.sleep(1)
                break
            time.sleep(1)
    finally:
        proc.kill()
        log.close()
    elapsed = time.time() - start
    for n in names:
        if (data / n).exists():
            (data / n).replace(out / n)
    text = log_path.read_text(errors='replace')
    lines = text.splitlines()

    def find(prefix):
        return [fields(l) for l in lines if l.startswith(prefix)]

    replays = find('drive_race_replay version=1 event=start')
    fx_start = find('replay_effects version=1 event=start')
    jump = find('drive_flashback version=1 event=jump_in')
    restored = find('drive_flashback version=1 event=restored')
    resume = find('drive_flashback version=1 event=resume')
    adopt = find('replay_effects version=1 event=adopt')
    exits = find('replay_effects version=1 event=exit')
    problems = []
    if 'panicked' in text:
        problems.append('panic')
    if not replays:
        problems.append('no replay started')
    elif args.slow and replays[0].get('slow') != 'true':
        problems.append('replay not in slow motion')
    if not fx_start:
        problems.append('no replay_effects start')
    if not (jump and restored and resume):
        problems.append('no jump-in/restored/resume')
    if not adopt:
        problems.append('no replay_effects adopt')
    elif resume and adopt[0].get('tick') != resume[0].get('tick'):
        problems.append(f'adopt tick {adopt[0].get("tick")} != resume tick {resume[0].get("tick")}')
    elif int(adopt[0].get('stepped', 0)) == 0:
        problems.append('replay effects never stepped')
    if len(replays) < 2:
        problems.append('race did not reach the script end again after the jump-in')
    summary = []
    if fx_start:
        f = fx_start[0]
        summary.append(f'at replay start: strips={f.get("skid_strips")} debris={f.get("debris")} smoke={f.get("smoke")} '
                       f'checkpoints={f.get("checkpoints")} bytes={f.get("checkpoint_bytes")}')
    if jump:
        summary.append(f'jump-in from tick {jump[0].get("from")} to {jump[0].get("target")}')
    if adopt:
        a = adopt[0]
        summary.append(f'adopt tick={a.get("tick")} strips={a.get("skid_strips")} debris={a.get("debris")} '
                       f'smoke={a.get("smoke")} stepped={a.get("stepped")} restores={a.get("restores")}')
    if len(fx_start) > 1:
        f = fx_start[1]
        summary.append(f'second replay: strips={f.get("skid_strips")} debris={f.get("debris")} smoke={f.get("smoke")}')
    if exits:
        summary.append(f'exits={len(exits)}')
    verdict = ('PASS' if not problems else 'FAIL: ' + ', '.join(problems)) + f' ({elapsed:.0f} s)'
    for s in summary:
        print(s)
    missing = [n for n in names if not (out / n).exists()]
    if missing:
        verdict += f' (shots missing: {", ".join(missing)})'
    print(verdict)
    print(f'log: {log_path}')
    (out / 'result.txt').write_text('\n'.join(summary + [verdict]) + '\n')
    sys.exit(0 if not problems else 1)


if __name__ == '__main__':
    main()
