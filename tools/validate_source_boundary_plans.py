"""Scheduled source driving regressions; no whole-original-frame oracle."""
import argparse
import csv
import json
import math
from pathlib import Path
import struct
import subprocess

from validate_standard_properties import GAME, ROOT, digest

PLANS = {
    'immediate-quarter': [(600, 1, 0, .25, 0)],
    'immediate-full': [(600, 1, 0, 1, 0)],
    '180-left': [(180, 1, 0, 0, 0), (420, 1, 0, 1, 0)],
    '300-left': [(300, 1, 0, 0, 0), (300, 1, 0, 1, 0)],
    '300-right': [(300, 1, 0, 0, 0), (300, 1, 0, -1, 0)],
    '240-left-brake': [(240, 1, 0, 0, 0), (240, 1, 0, .5, 0), (120, 0, 1, 0, 0)],
}


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def from_hex(value):
    return struct.unpack('<f', struct.pack('<I', int(value, 16)))[0]


def snapshot(path, ticks):
    data = path.read_bytes()
    assert data[:4] == b'DP01'
    sizes = struct.unpack_from('<4I', data, 4)
    assert sizes == (0x21000 // 4, 0x424 // 4, 0x2b0 // 4, 884)
    assert len(data) == 36 + 4 * sum(sizes)
    assert struct.unpack_from('<QI', data, 20) == (ticks, 1)
    return data


def inspect(directory, expected_plan):
    with (directory / 'telemetry.tsv').open() as file:
        rows = list(csv.DictReader(file, delimiter='\t'))
    assert len(rows) == 601 and [int(r['tick']) for r in rows] == list(range(601))
    expected = [(segment, row[1:]) for segment, row in enumerate(expected_plan) for _ in range(row[0])]
    for row, (segment, controls) in zip(rows[1:], expected, strict=True):
        assert int(row['segment']) == segment
        assert tuple(float(row[key]) for key in ['throttle', 'brake', 'steering', 'handbrake']) == tuple(controls)
        assert all(math.isfinite(float(row[key])) for key in ['x', 'y', 'z', 'vx', 'vy', 'vz'])
        # Wheel contacts consume a variable prefix of the stable 20-slot pool.
        assert 0 <= int(row['body_contacts']) <= 20
        assert len(row['damage_words'].split(',')) == 9
    hits = [row for row in rows if int(row['body_contacts'])]
    assert hits and float(rows[-1]['y']) > -30
    ratios = []
    for row in rows:
        mass, coefficient = map(from_hex, row['damage_threshold_words'].split(','))
        threshold = f32(mass * coefficient)
        assert threshold > 0 and math.isfinite(threshold)
        impacts = [f32(force * f32(.016)) for force in map(from_hex, row['wheel_impact_words'].split(','))]
        assert len(impacts) == 4 and all(math.isfinite(force) for force in impacts)
        ratios.append(max(impacts) / threshold)
    first = int(hits[0]['tick'])
    before = rows[first - 1]
    speed = math.sqrt(sum(float(before[key]) ** 2 for key in ['vx', 'vy', 'vz']))
    return dict(ticks=600, contacts=sum(int(row['body_contacts']) for row in rows),
                first_contact=first, approach_speed_kmh=speed * 3.6,
                max_wheel_impact_threshold_ratio=max(ratios),
                global_rng_changed=any(row['global_rng'] != rows[0]['global_rng'] for row in rows),
                selected_damage_changed=any(row['damage_words'] != rows[0]['damage_words'] for row in rows),
                final_position=[float(rows[-1][key]) for key in ['x', 'y', 'z']])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--body-events',type=Path)
    args = parser.parse_args()
    assert args.output.resolve().is_relative_to((ROOT / 'local').resolve())
    args.output.mkdir(exist_ok=False)
    table = ROOT / 'local/body-contact-directions.bin'
    if args.body_events:
        args.body_events=args.body_events.resolve()
        assert args.body_events.is_relative_to((ROOT/'local').resolve())
    pinned = {str(path): digest(path) for path in [table, Path(__file__),
              ROOT / 'crates/grid-tools/src/bin/grid-drive-plan.rs',
              ROOT / 'crates/grid-tools/src/source_vehicle.rs']}
    if args.body_events:
        for path in [args.body_events,ROOT/'crates/grid-tools/src/body_impact_parameters.rs',ROOT/'crates/grid-world/src/body_impact.rs',ROOT/'crates/grid-world/src/vehicle_world.rs']:
            pinned[str(path)]=digest(path)
    outputs = []
    for name, plan in PLANS.items():
        source = args.output / (name + '.txt')
        source.write_text(''.join(' '.join(map(str, row)) + '\n' for row in plan))
        baseline = None
        for profile in ['debug', 'release']:
            exe = ROOT / f'target/{profile}/grid-drive-plan.exe'
            pinned[str(exe)] = digest(exe)
            for repeat in range(2):
                output = args.output / f'{name}-{profile}-{repeat}'
                command = list(map(str, [exe, GAME, table, source, output]))
                if args.body_events:command.append(str(args.body_events))
                execution = subprocess.run(command, capture_output=True, text=True, timeout=120)
                assert execution.returncode == 0 and not execution.stderr, (command, execution)
                current = ((output / 'telemetry.tsv').read_bytes(), snapshot(output / 'initial.bin', 0), snapshot(output / 'final.bin', 600))
                if baseline is None:
                    baseline = current
                else:
                    assert current == baseline, (name, profile, repeat)
                assert (output / 'plan.txt').read_bytes() == source.read_bytes()
                if args.body_events:assert (output/'body-events.bin').read_bytes()==args.body_events.read_bytes()
                metrics = inspect(output, plan)
                hashes = {str(path): digest(path) for path in sorted(output.iterdir())}
                outputs.append(dict(name=name, profile=profile, repeat=repeat, command=command,
                                    stdout=execution.stdout, stderr=execution.stderr, exit=execution.returncode,
                                    plan_sha256=digest(source), files=hashes, metrics=metrics))
                print(json.dumps(dict(name=name, profile=profile, repeat=repeat, **metrics)), flush=True)
    guards = []
    for profile in ['debug', 'release']:
        exe = ROOT / f'target/{profile}/grid-drive-plan.exe'
        for name, text in [('empty', '# empty\n'), ('nonfinite', '1 NaN 0 0 0\n'),
                           ('zero', '0 1 0 0 0\n'), ('total', '36000 1 0 0 0\n1 1 0 0 0\n'),
                           ('handbrake', '1 1 0 0 2\n'), ('steering', '1 1 0 2 0\n')]:
            source = args.output / f'guard-{profile}-{name}.txt'
            source.write_text(text)
            output = args.output / f'guard-{profile}-{name}'
            execution = subprocess.run(list(map(str, [exe, GAME, table, source, output])), capture_output=True, text=True, timeout=30)
            assert execution.returncode != 0 and not output.exists()
            guards.append(dict(profile=profile, label=name, exit=execution.returncode, stderr=execution.stderr, no_output=True))
        output = args.output / f'guard-{profile}-existing'
        output.mkdir()
        marker = output / 'keep.txt'
        marker.write_bytes(b'KEEP')
        execution = subprocess.run(list(map(str, [exe, GAME, table, args.output / 'immediate-quarter.txt', output])), capture_output=True, text=True, timeout=30)
        assert execution.returncode != 0 and marker.read_bytes() == b'KEEP' and len(list(output.iterdir())) == 1
        guards.append(dict(profile=profile, label='existing', exit=execution.returncode, stderr=execution.stderr, preserved=True))
    report = dict(passed=True, cases=len(outputs), ticks=600 * len(outputs), inputs=pinned,
                  body_events=str(args.body_events) if args.body_events else None,
                  outputs=outputs, guards=guards, guard_count=len(guards),
                  scope='Authored input schedules on source Lacetti/Donington; both profiles/repeats '
                        'match full per-tick TSV and complete selected initial/final buffers. '
                        'Threshold ratios observe post-tick wheel operands, not every intermediate '
                        'handling input. No original-frame, arbitrary crash or lap equivalence.')
    (args.output / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({key: value for key, value in report.items() if key not in ['inputs', 'outputs', 'guards']}))


if __name__ == '__main__':
    main()
