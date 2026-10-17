"""Reproduce logged controls and prove sampled motion before a damage stop."""
import argparse
import ast
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from summarize_driving_log import FIELDS

ROOT = Path(__file__).resolve().parents[1]


def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inputs(log, segment, continuation):
    rows = [dict(FIELDS.findall(line)) for line in log.read_text().splitlines()
            if line.startswith('drive_sample ')]
    rows = [r for r in rows if int(r['segment']) == segment]
    failure, = [r for r in rows if r['event'] == 'failure']
    observed = {int(r['tick']): r for r in rows if r['event'] == 'tick'}
    assert len(observed) == sum(r['event'] == 'tick' for r in rows)
    failed_tick = int(failure['tick'])+1
    assert max(observed) == failed_tick-1 and min(observed) == 1
    records, current = [], None
    for tick in range(1, failed_tick+continuation+1):
        row = observed.get(tick)
        if row: current = row
        if tick == failed_tick: current = failure
        assert current is not None
        controls = [float(current[k]) for k in ['throttle', 'brake', 'steering']]
        expected = ast.literal_eval(row['pose'])+ast.literal_eval(row['velocity']) if row else [0.]*20
        assert current['handbrake'] in ['true', 'false']
        records.append(struct.pack('<3fII20f', *controls,
                                   int(current['handbrake'] == 'true'), int(row is not None), *expected))
    return b'DR01'+struct.pack('<I', len(records))+b''.join(records), len(observed), failed_tick


def inspect(path, failed_tick):
    rows = [line.split('\t') for line in path.read_text().splitlines()]
    assert all(len(r) == 40 and r[0] == 'T' and int(r[1]) == i+1 for i, r in enumerate(rows))
    terminal = [r for r in rows if r[2] == '3']
    assert terminal and int(terminal[0][1]) == failed_tick
    assert len(terminal) == len(rows)-failed_tick+1
    assert len({r[3] for r in terminal}) == 1
    assert all(r[36] == '3f800000' and r[38] == '3f800000' and r[39] == '00000000'
               for r in rows[failed_tick:])
    word = lambda x: struct.unpack('<f', struct.pack('<I', int(x, 16)))[0]
    previous = rows[failed_tick-2]
    return dict(ticks=len(rows), first_terminal_tick=failed_tick, terminal_wheel=int(terminal[0][3]),
                continued_ticks=len(rows)-failed_tick, physics_continued=True,
                controls_disabled_next_tick=True,
                pre_terminal_effects_3a0_3bc=[word(w) for w in previous[24:32]],
                pre_terminal_wheel_levels=[word(w) for w in previous[32:36]],
                last_pose=[word(w) for w in rows[-1][4:20]])


def main():
    p = argparse.ArgumentParser()
    p.add_argument('log', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--game-root', type=Path,
                   default=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid'))
    p.add_argument('--continuation', type=int, default=300)
    a = p.parse_args()
    assert a.log.resolve().is_relative_to((ROOT/'local').resolve())
    assert a.output.resolve().is_relative_to((ROOT/'local').resolve()) and 1 <= a.continuation <= 3000
    a.output.mkdir(exist_ok=False)
    log_rows = [dict(FIELDS.findall(line)) for line in a.log.read_text().splitlines()
                if line.startswith('drive_sample ') and 'event=failure ' in line]
    segments = sorted({int(r['segment']) for r in log_rows})
    assert segments
    parameters = [ROOT/'local/body-contact-directions.bin', ROOT/'local/body-impact-parameters.bin',
                  ROOT/'local/deformation-parameters.bin']
    identities = {str(path): digest(path) for path in
                  [a.log, Path(__file__), ROOT/'crates/grid-probes/src/bin/grid-drive-replay.rs', *parameters]}
    results = []
    for segment in segments:
        raw, observed, failed_tick = inputs(a.log, segment, a.continuation)
        request = a.output/f'segment-{segment}.input'
        request.write_bytes(raw)
        outputs, executions = {}, {}
        for profile in ['debug', 'release', 'repeat']:
            exe = ROOT/f'target/{"release" if profile == "repeat" else profile}/grid-drive-replay.exe'
            identities[str(exe)] = digest(exe)
            target = a.output/f'segment-{segment}-{profile}.tsv'
            command = [str(exe), str(a.game_root), *map(str, parameters), str(request), str(target)]
            run = subprocess.run(command, capture_output=True, text=True, timeout=120, cwd=ROOT)
            assert run.returncode == 0, run.stderr
            assert f'sampled_pose_velocity_exact={observed} completed=1' in run.stdout
            outputs[profile] = dict(path=str(target), sha256=digest(target))
            executions[profile] = dict(stdout=run.stdout, stderr=run.stderr, returncode=run.returncode)
        assert len({r['sha256'] for r in outputs.values()}) == 1
        results.append(dict(segment=segment, sampled_pose_velocity_exact=observed,
                            request=str(request), request_sha256=digest(request), outputs=outputs,
                            executions=executions, **inspect(Path(outputs['release']['path']), failed_tick)))
    report = dict(passed=True, log=str(a.log), identities=identities, segments=results,
                  scope='Logged controls held until the next logged change; every recorded successful pose/velocity checked exactly. Continuation after the old stop is an authored experiment, not an original-game capture.')
    (a.output/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(passed=True, segments=len(results),
                         sampled_pose_velocity_exact=sum(r['sampled_pose_velocity_exact'] for r in results),
                         terminal_ticks=[r['first_terminal_tick'] for r in results])))


if __name__ == '__main__': main()
