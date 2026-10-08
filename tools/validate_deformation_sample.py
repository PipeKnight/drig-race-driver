"""Compare ordered supplied-grid sampling with whole original9a39d0, privately."""
import argparse
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '8ce8358aeccf155bd5e0b4e4e0cfbf5bbf9dd78a83f0b60e7bdd05aeb7a7073c'


def f32(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def rows(path):
    result = {}
    for line in path.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == 6 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--snapshot', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformation-sample-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-deformation-sample.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    out.mkdir(exist_ok=False)
    rng, cases = random.Random(134), []
    for dimensions, spacing in [([2, 2, 2], 1.), ([3, 4, 5], .25), ([6, 5, 12], .4224487245082855)]:
        minimum = [-1., -.5, -2., 1.]
        nodes = []
        for z in range(dimensions[2]):
            for y in range(dimensions[1]):
                for x in range(dimensions[0]):
                    nodes += [minimum[0] + x * spacing + .1 * y * z,
                              minimum[1] + y * spacing - .2 * x * z,
                              minimum[2] + z * spacing + .3 * x * y, 1. + .01 * x * y * z]
        for axis in range(3):
            for fraction in [-1., -0., 0., .001, .5, .999, 1., 2.]:
                point = [minimum[i] + .5 * (dimensions[i] - 1) * spacing for i in range(3)] + [-0.]
                point[axis] = minimum[axis] + fraction * (dimensions[axis] - 1) * spacing
                cases.append((f'boundary_{len(cases)}', list(map(float, dimensions)) + [spacing] + minimum + point + nodes))
    for i in range(512):
        dimensions = [rng.randint(2, 6), rng.randint(2, 6), rng.randint(2, 8)]
        spacing = rng.uniform(.05, 2.)
        minimum = [rng.uniform(-5., 5.) for _ in range(4)]
        point = [minimum[a] + rng.uniform(-1., float(dimensions[a])) * spacing for a in range(3)] + [rng.uniform(-2., 2.)]
        nodes = [rng.uniform(-5., 5.) for _ in range(4 * dimensions[0] * dimensions[1] * dimensions[2])]
        cases.append((f'authored_{i}', list(map(float, dimensions)) + [spacing] + minimum + point + nodes))
    capture_report, retained_cases = None, 0
    if args.snapshot:
        path = args.snapshot.resolve()
        assert path.is_relative_to((ROOT / 'local').resolve()) and path.stat().st_size <= 8 * 1024 * 1024
        capture = json.loads(path.read_text(encoding='utf-8-sig'))
        assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
        assert capture['live_count_code_exact'] and len(capture['managers']) == 1
        assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
        blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
        assert len(blocks['steering_code_9a39d0']) == len(blocks['steering_code_9bb1e0']) == 32
        grids = []
        for car in capture['managers'][0]['cars']:
            i = car['index']
            header, raw = blocks[f'car_{i}-deformation_grid_descriptor'], blocks[f'car_{i}-deformation_grid_nodes']
            assert car['wheel_count'] == 4 and len(header) == 0x2c
            dimensions = list(struct.unpack_from('<3i', header))
            spacing = struct.unpack_from('<f', header, 12)[0]
            minimum = list(struct.unpack_from('<4f', header, 28))
            count = dimensions[0] * dimensions[1] * dimensions[2]
            assert count <= 400 and len(raw) == count * 80
            nodes = [v for offset in range(0, len(raw), 80) for v in struct.unpack_from('<4f', raw, offset)]
            wheels, frame = blocks[f'car_{i}-wheel_records'], blocks[f'car_{i}-frame_fields']
            center = struct.unpack_from('<4f', frame, 0x60)
            for wheel in range(4):
                point = [f32(a - b) for a, b in zip(struct.unpack_from('<4f', wheels, wheel * 0x150), center)]
                radius = struct.unpack_from('<f', wheels, wheel * 0x150 + 0xe0)[0]
                queries = [point.copy()]
                for axis in (2, 1):
                    for sign in (1., -1.):
                        query = point.copy()
                        query[axis] = f32(query[axis] + sign * radius)
                        queries.append(query)
                for q, query in enumerate(queries):
                    cases.append((f'retained_{i}_{wheel}_{q}', list(map(float, dimensions)) + [spacing] + minimum + query + nodes))
                    retained_cases += 1
            grids.append(dict(car=i, dimensions=dimensions, spacing_word=f'{word(spacing):08x}', nodes=count))
        assert len(grids) == 12
        capture_report = dict(path=str(path), sha256=digest(path), utc=capture['utc'], pid=capture['pid'],
                              created_utc=capture['created_utc'], blocks=len(blocks), grids=grids)
    assert len(cases) <= 1024
    request = out / 'input.tsv'
    request.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n' for name, row in cases))
    assert request.stat().st_size <= 16 * 1024 * 1024
    executions = []
    for kind, exe, argv in [('original', probe, [image, request, out / 'original.tsv']),
                           ('native', native, [request, out / 'native.tsv'])]:
        run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
        (out / 'executions.json').write_text(json.dumps(executions, indent=2))
        if run.returncode:
            raise RuntimeError('Deformation sampler execution failed')
    original, actual = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert original.keys() == actual.keys() == {name for name, _ in cases}
    differences = [dict(case=name, field=i, original=a, native=b) for name in original
                   for i, (a, b) in enumerate(zip(original[name], actual[name])) if a != b]
    guards = []
    for slot, value in [(0, 1.), (3, 0.), (8, float('nan')), (4, 1e30)]:
        row = cases[0][1].copy()
        row[slot] = value
        guards.append(row)
    guards.append(cases[0][1][:-1])
    guard_results = []
    for i, row in enumerate(guards):
        guard = out / f'guard-{i}.tsv'
        guard.write_text('guard\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n')
        target = out / f'guard-{i}-native.tsv'
        a = subprocess.run([str(probe), str(image), str(guard), str(out / f'guard-{i}-original.tsv')], capture_output=True, text=True, timeout=30)
        b = subprocess.run([str(native), str(guard), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        guard_results.append(dict(original_exit=a.returncode, native_exit=b.returncode, native_stderr=b.stderr, output_created=target.exists()))
        assert a.returncode and b.returncode and not target.exists()
    report = dict(profile=args.profile, cases=len(cases), fields=len(cases) * 4, retained_grid_cases=retained_cases,
                  differences=differences, executions=executions, guards=guard_results, capture=capture_report,
                  input_sha256=digest(request), analysis_sha256=PE_SHA,
                  limits='Whole original9a39d0/real pointer setters with supplied descriptor positions; RX code/read-only mapped data. '
                         'Retained grids are non-atomic snapshots and wheel queries are constructed, not captured sampler outputs. '
                         'Grid creation/deformation, whole wheel geometry and collision production remain unexecuted.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(profile=args.profile, cases=len(cases), fields=len(cases) * 4,
                         retained_grid_cases=retained_cases, different_fields=len(differences))))
    assert not differences


if __name__ == '__main__':
    main()
