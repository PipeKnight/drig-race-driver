"""Compare scoped mutable steering outputs of whole original 9c5b90."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PE_SHA = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
PROBE_SHA = '865481d38446062734501c7b47878d2a2effae993113f648c78946e0a38cf4ba'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def word(v):
    return struct.unpack('<I', struct.pack('<f', v))[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    original_exe = ROOT / 'local/behavior/steering-control-probe.exe'
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    assert digest(original_exe) == PROBE_SHA and digest(image) == PE_SHA
    cases, guards = [], []
    base = [.1, .25, .5, .75, .5, .75, .2, .8, -.7, .3, -.2, .9,
            1.5, .25, .75, .1, .5, 1.]
    # Each boundary runs both wrapper states. Near-zero, equality and reversed
    # ramps deliberately include unordered derived fractions from 0/0, checked
    # separately for the native nonfinite-output guard.
    levels = [-1., -0., 0., .2499999850988388, .25, .2500000298023224,
              .4999999701976776, .5, .5000000596046448,
              .7499999403953552, .75, .7500000596046448, 1., 2.]
    for start, end in [(.25, .75), (.5, .5), (.75, .25), (0., 0.), (-0., 0.)]:
        for level in levels:
            for active in [0., 1.]:
                row = base.copy()
                row[:6] = [level] * 6
                row[13:15] = [start, end]
                row[17] = active
                cases.append((f'ramp_{len(cases)}', row))
    for threshold in [-1., -0., 0., .5, 1., 1.5]:
        for level in levels:
            row = base.copy()
            row[4:6] = [level, level]
            row[16] = threshold
            cases.append((f'cap_{len(cases)}', row))
    for gain in [-3., -0., 0., 1., 2., 3.]:
        for low in [-.5, -0., 0., .5, 1., 1.5]:
            row = base.copy()
            row[12], row[15] = gain, low
            cases.append((f'gain_{len(cases)}', row))
    rng = random.Random(126)
    for i in range(2048):
        row = [rng.uniform(-1., 2.) for _ in range(6)]
        row += [rng.random(), rng.random()]
        row += [rng.uniform(-2., 2.) for _ in range(4)]
        row += [rng.uniform(-3., 3.), rng.uniform(-.5, 1.5),
                rng.uniform(-.5, 1.5), rng.uniform(-.5, 1.5),
                rng.uniform(-.5, 1.5), float(i % 2)]
        cases.append((f'authored_{i}', row))
    finite_cases = []
    for name, row in cases:
        level = max(row[2:4])
        unordered = (row[13] == row[14] and (level == row[13] or
                     (row[17] != 0. and row[13] in row[:4])))
        unordered |= row[16] == 1. and 1. in row[4:6]
        (guards if unordered else finite_cases).append((name, row))
    cases = finite_cases
    native_exe = ROOT / f'target/{args.profile}/grid-steering-control.exe'
    guard_results = []
    for i, (name, row) in enumerate(guards):
        request = out / f'guard-{i}.tsv'
        request.write_text(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n')
        a = subprocess.run([str(original_exe), str(image), str(request),
                            str(out / f'guard-{i}-original.tsv')], capture_output=True, text=True, timeout=30)
        target = out / f'guard-{i}-native.tsv'
        b = subprocess.run([str(native_exe), str(request), str(target)],
                           cwd=ROOT, capture_output=True, text=True, timeout=30)
        guard_results.append(dict(case=name, original_exit=a.returncode, native_exit=b.returncode,
                                  original_stdout=a.stdout, native_stderr=b.stderr,
                                  native_output_created=target.exists()))
        assert a.returncode == 11 and b.returncode != 0 and not target.exists()
    request = out / 'input.tsv'
    request.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n'
                               for name, row in cases))
    executions = []
    for kind, exe, argv in [
        ('original', original_exe, [image, request, out / 'original.tsv']),
        ('native', native_exe,
         [request, out / 'native.tsv'])]:
        run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT,
                             capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, sha256=digest(exe), exit=run.returncode,
                               stdout=run.stdout, stderr=run.stderr))
        if run.returncode:
            (out / 'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Mutable steering execution failed')

    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            cells = line.split('\t')
            assert len(cells) == 13 and cells[0] == 'P' and cells[1] not in result
            result[cells[1]] = cells[2:]
        return result

    original, native = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert original.keys() == native.keys() == {name for name, _ in cases}
    differences = [dict(case=name, field=i, original=a, native=b)
                   for name in original for i, (a, b) in enumerate(zip(original[name], native[name]))
                   if a != b]
    result = dict(profile=args.profile, cases=len(cases), fields=len(cases) * 11,
                  exact_cases=len(cases) - len({d['case'] for d in differences}),
                  differences=differences, executions=executions, nonfinite_guards=guard_results,
                  input_sha256=digest(request), analysis_sha256=PE_SHA,
                  limits='Whole original 9c5b90, seven steering scalar outputs and four flags only; '
                         'authored inputs, four wheels, component queries inactive. Original reset, RNG, '
                         'constructor, production copier, source parameters and live cadence unexecuted.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ('profile', 'cases', 'fields', 'exact_cases')}))
    print('different_fields=' + str(len(differences)))
    assert not differences


if __name__ == '__main__':
    main()
