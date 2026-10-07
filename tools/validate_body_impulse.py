"""Compare supplied impulses with unstubbed original angular/application helpers."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def word(v):
    return struct.unpack('<I', struct.pack('<f', v))[0]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--point-response', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    cases = []

    def add(name, parameters, basis, linear, angular, impulse, lever):
        values = parameters + basis + linear + angular + impulse + lever
        assert len(values) == 36
        cases.append((name, [word(v) for v in values]))

    parameters = [2, 3, 4, 5]
    zero = [0] * 4
    add('zero', parameters, identity, zero, zero, zero, zero)
    add('center', parameters, identity, zero, zero, [2, -4, 6, 0], zero)
    for axis in range(3):
        for sign in (-1, 1):
            impulse = zero.copy()
            impulse[axis] = sign * 12
            lever = [2, 3, -5, 0]
            add(f'axis_{axis}_{sign+1}', parameters, identity, [1, -2, 3, 0],
                [-3, 2, -1, 0], impulse, lever)
    add('signed_zero', parameters, identity, [-0.0] * 4, [0] * 4, [-0.0] * 4, [-0.0] * 4)

    table = args.game_root / 'cars/lac/lac.csv'
    row = next(r for r in csv.reader(table.read_text().splitlines()) if r)
    source_parameters = [float(row[i]) for i in range(3, 7)]
    capture_path = ROOT / 'local/behavior/live-spawn-20261007-08/capture.json'
    capture = json.loads(capture_path.read_text(encoding='utf-8-sig'))
    assert capture['completed'] and capture['code_exact'] and len(capture['cases']) == 12
    assert digest(capture_path) == '7007c2feb3efc141673788634446af80251bd8835c83da1b3b82b253cd83d2c4'
    for index, case in enumerate(capture['cases']):
        basis = list(struct.unpack('<16f', bytes.fromhex(case['pose_hex'])))
        for axis in range(3):
            impulse = zero.copy()
            impulse[axis] = 1200
            lever = [.7, -.25, 1.2, 0]
            if args.point_response:
                lever = [basis[12+i]+lever[i] for i in range(4)]
            add(f'recorded_pose_{index:02d}_{axis}', source_parameters, basis, [3, -4, 5, 0],
                [.2, -.3, .4, 0], impulse, lever)

    rng = random.Random(2008)
    # Arbitrary finite matrices deliberately test arithmetic beyond rigid poses;
    # they are not asserted to be physically valid solver orientations.
    for index in range(160):
        p = [rng.uniform(50, 2000)] + [rng.uniform(10, 5000) for _ in range(3)]
        matrix = identity if index < 40 else [rng.uniform(-2, 2) for _ in range(16)]
        vectors = [[rng.uniform(-20, 20) for _ in range(4)] for _ in range(4)]
        add(f'synthetic_{index:03d}', p, matrix, *vectors)
    snapshot_path = None
    if args.point_response:
        snapshot_path = ROOT/'local/behavior/general-vehicle-manager-20261007T0837131885850Z.json'
        assert digest(snapshot_path) == '4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'
        snapshot = json.loads(snapshot_path.read_text(encoding='utf-8-sig'))
        assert snapshot['read_only'] and snapshot['live_count_code_exact']
        blocks = {b['name']: b for b in snapshot['blocks']}
        assert len(blocks) == len(snapshot['blocks'])
        for car in snapshot['managers'][0]['cars']:
            i = car['index']
            assert car['wheel_count'] == 4
            def read(name, length):
                b = blocks[f'car_{i}-{name}']
                assert b['success'] and b['read'] == b['length'] == length
                raw = bytes.fromhex(b['hex'])
                assert len(raw) == length
                return raw
            mass = read('mass_inertia_fields', 0x30)
            frame = read('frame_fields', 0x90)
            context = read('dynamics_context', 0x740)
            params = [struct.unpack_from('<f', mass, offset)[0] for offset in (0, 8, 12, 16)]
            basis = list(struct.unpack_from('<16f', frame, 0x10))
            linear = list(struct.unpack_from('<4f', mass, 0x20))
            angular = list(struct.unpack_from('<4f', frame, 0))
            for wheel in range(4):
                point = list(struct.unpack_from('<4f', context, 0x10+wheel*0x170))
                # Recorded state and wheel point; this test impulse is supplied.
                add(f'live_state_{i:02d}_{wheel}', params, basis, linear, angular,
                    [0, 1200, 0, 0], point)
    requests = out / 'input.tsv'
    with requests.open('x', encoding='ascii') as stream:
        for name, words in cases:
            stream.write(name + '\t' + '\t'.join(f'{v:08x}' for v in words) + '\n')
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    programs = [(ROOT / 'local/behavior/body-impulse-probe.exe', [image, requests, out / 'original.tsv']),
                (ROOT / f'target/{args.profile}/grid-body-impulse.exe', [requests, out / 'native.tsv'])]
    if args.point_response:
        for _, arguments in programs:
            arguments.append('point')
    executions = []
    for executable, arguments in programs:
        run = subprocess.run([str(executable), *map(str, arguments)], cwd=ROOT,
                             capture_output=True, text=True, timeout=30)
        executions.append({'path': str(executable), 'sha256': digest(executable), 'exit': run.returncode,
                           'stdout': run.stdout, 'stderr': run.stderr})
        if run.returncode:
            (out / 'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Impulse comparison failed; evidence retained')

    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            row = line.split('\t')
            assert row[0] == 'P' and len(row) == 14 and row[1] not in result
            result[row[1]] = row[2:]
        return result

    original, native = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert set(original) == set(native) == {name for name, _ in cases}
    comparisons = []
    for name, _ in cases:
        differences = [{'field': i, 'original': a, 'native': b}
                       for i, (a, b) in enumerate(zip(original[name], native[name])) if a != b]
        comparisons.append({'name': name, 'exact': not differences, 'differences': differences})
    report = {'profile': args.profile, 'executions': executions, 'image_sha256': digest(image),
              'point_response': args.point_response,
              'input_sha256': digest(requests), 'capture_sha256': digest(capture_path),
              'csv_sha256': digest(table), 'source_parameter_words': [f'{word(v):08x}' for v in source_parameters],
              'cases': comparisons, 'summary': {'cases': len(cases),
              'exact_cases': sum(c['exact'] for c in comparisons), 'compared_float_words': len(cases) * 12,
              'exact_float_words': len(cases) * 12 - sum(len(c['differences']) for c in comparisons)},
              'limits': 'Original 0x9cc3d0 / 0x9cc6e0 and cross-product helper run without stubs. '
              'Supplied impulses and lever arms; recorded placement poses are reused as controlled inputs, '
              'not live impulse observations. Lacetti CSV mass/inertia used for all recorded-pose cases. '
              'Arbitrary matrix cases test arithmetic only. No tyre/suspension/contact generation, '
              'temporary drivetrain overrides, simulation timing or driving equivalence.'}
    if args.point_response:
        report['snapshot_sha256'] = digest(snapshot_path)
        report['limits'] = ('Unstubbed original point velocity 0x99c6f0 and world-point impulse '
                            '0x9cc750 plus their original helpers. Supplied impulses, points and '
                            'states, including 48 recorded wheel points with original state. '
                            'No live impulse measurement, coefficient preparation, tyre/contact '
                            'generation, timing or driving equivalence.')
    with (out / 'validation.json').open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases'] == len(cases)


if __name__ == '__main__':
    main()
