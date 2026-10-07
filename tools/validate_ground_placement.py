"""Conditional original numeric placement versus Rust using controlled plane queries."""
import argparse
import csv
import hashlib
import json
import math
import random
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode
from validate_wheel_geometry import layout


def value(word):
    return struct.unpack('<f', struct.pack('<I', int(word, 16)))[0]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('output_directory', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    out = args.output_directory.resolve()
    if not out.is_relative_to((root / 'local').resolve()):
        raise ValueError('Evidence must remain private')
    out.mkdir(exist_ok=False)
    source = root / 'local/compatibility/grid-unpacked.exe'
    assert digest(source) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    wheels = [[1, 0, -2, .5, 0], [-1, 0, -2, .5, 0], [1, 0, 2, .5, 0], [-1, 0, 2, .5, 0]]
    cases = [['flat', 3, 99, 4, 0, 0, 1] + sum(wheels, []) + [0, 0, 2],
             ['slope', 3, 99, 4, 0, 0, 1] + sum(wheels, []) + [.1, .2, 0]]
    rng = random.Random(2008)
    for i in range(120):
        angle = rng.uniform(-math.pi, math.pi)
        rear_x, front_x = rng.uniform(.5, 1), rng.uniform(.5, 1)
        rear_y, front_y = rng.uniform(-.2, .2), rng.uniform(-.2, .2)
        rear_z, front_z = rng.uniform(-2, -1), rng.uniform(1, 2)
        rear_r, front_r = rng.uniform(.25, .4), rng.uniform(.25, .4)
        wheels = [[rear_x, rear_y, rear_z, rear_r, rng.uniform(-.01, .01)],
                  [-rear_x, rear_y, rear_z, rear_r, rng.uniform(-.01, .01)],
                  [front_x, front_y, front_z, front_r, rng.uniform(-.01, .01)],
                  [-front_x, front_y, front_z, front_r, rng.uniform(-.01, .01)]]
        cases.append([f'synthetic_{i:03d}', rng.uniform(-100, 100), 0, rng.uniform(-100, 100),
                      math.sin(angle), rng.uniform(-.1, .1), math.cos(angle)] + sum(wheels, []) +
                     [rng.uniform(-.25, .25), rng.uniform(-.25, .25), rng.uniform(-20, 20)])
    resources = []
    for car in ['lac', 'bmw']:
        ned = args.game_root / f'cars/{car}/{car}.ned'
        csv_path = ned.with_suffix('.csv')
        tree = decode(ned.read_bytes())[1]
        attrs = next(e for e in tree.iter() if e.tag == 'WHEELS').attrib
        row = next(r for r in csv.reader(csv_path.read_text().splitlines()) if r)
        _, _, _, positions, radii = layout(attrs, row)
        resources.append({'ned': str(ned), 'ned_sha256': digest(ned), 'csv_sha256': digest(csv_path)})
        for i, plane in enumerate([[0, 0, -12], [.01, -.02, -12], [-.1, .05, 2], [.2, .1, -3]]):
            wheels = [position[:3] + [radius, 0.] for position, radius in zip(positions, radii)]
            cases.append([f'{car}_{i}', -166.3, 0, 22.52, -.9816687, 0, -.1905951] + sum(wheels, []) + plane)
    requests = out / 'input.tsv'
    with requests.open('x') as handle:
        for case in cases:
            assert len(case) == 30
            handle.write('\t'.join(map(str, case)) + '\n')
    original = out / 'original.tsv'
    programs = [(root / 'local/behavior/ground-placement-probe.exe', [source, requests, original])]
    for mode in ['direct', 'fraction', 'reference']:
        programs.append((root / 'target/debug/grid-placement-plane.exe', [requests, out / f'native-{mode}.tsv', mode]))
    executions = []
    for program, arguments in programs:
        process = subprocess.run([str(program), *map(str, arguments)], cwd=root, capture_output=True, text=True, timeout=30)
        executions.append({'program': str(program), 'sha256': digest(program), 'exit': process.returncode,
                           'stdout': process.stdout, 'stderr': process.stderr})
        if process.returncode:
            with (out / 'failed-run.json').open('x') as handle:
                json.dump(executions, handle, indent=2)
            raise SystemExit('Placement executable failed; evidence retained')
    rows = [line.split('\t') for line in original.read_text().splitlines()]
    assert rows[0][0] == 'C'
    oracle = [row for row in rows if row[0] == 'P']
    assert len(oracle) == len(cases)
    assert all(row[-1] == '9' for row in oracle)
    results, summaries = [], {}
    for mode in ['direct', 'fraction', 'reference']:
        native = [line.split('\t') for line in (out / f'native-{mode}.tsv').read_text().splitlines()]
        assert len(native) == len(oracle)
        comparisons = []
        for reference, actual in zip(oracle, native):
            assert reference[0:2] == actual[0:2] and actual[-1] == '9'
            delta = [abs(value(a) - value(b)) for a, b in zip(reference[2:18], actual[2:18])]
            comparisons.append({'name': reference[1], 'exact_words': sum(a == b for a, b in zip(reference[2:18], actual[2:18])),
                'exact': reference == actual, 'max_basis_error': max(delta[:12]), 'max_position_error': max(delta[12:]),
                'original': reference[2:18], 'native': actual[2:18]})
        summaries[mode] = {'cases': len(comparisons), 'exact_cases': sum(r['exact'] for r in comparisons),
            'exact_words': sum(r['exact_words'] for r in comparisons),
            'max_basis_error': max(r['max_basis_error'] for r in comparisons),
            'max_position_error': max(r['max_position_error'] for r in comparisons)}
        results.append({'mode': mode, 'comparisons': comparisons})
    report = {'summary': summaries, 'environment': rows[0], 'executions': executions, 'resources': resources,
              'reference_sha256': digest(source), 'requests_sha256': digest(requests), 'results': results,
              'limits': 'Original placement/getter/cross/wheel-vector/height-wrapper routines only, with our controlled plane candidate oracle. No original world filtering, live state, entry point, imports, TLS or contact solver equivalence.'}
    with (out / 'validation.json').open('x') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(summaries))


if __name__ == '__main__':
    main()
