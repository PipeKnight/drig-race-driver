"""Compare whole original input conditioning to independently authored Rust."""
import argparse
import hashlib
import itertools
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    cases = []
    for i, row in enumerate(itertools.product([-2., -1., -0., 0., 1., 2.],
                                                [-1., -0., 0., 1e-30, .49999997, .5, 1., 2.],
                                                [-10., -0., 0., 4.9999995, 5., 10.])):
        cases.append((f'boundary_{i}', [*row, .37]))
    for i, phase in enumerate([-10000., -1000., -10., -1., -1e-30, -0., 0., 1e-30, .1, 1., 10., 1000., 10000.]):
        for p in [.25, .5, 1.]:
            cases.append((f'phase_{i}_{p}', [0., p, 10., phase]))
    rng = random.Random(116)
    for i in range(1024):
        cases.append((f'authored_{i}', [rng.uniform(-1.2, 1.2), rng.uniform(-1., 2.),
                                       rng.uniform(-10., 20.), rng.uniform(-10000., 10000.)]))
    requests = out / 'input.tsv'
    requests.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n' for name, row in cases))
    executions = []
    for kind, exe, argv in [
        ('original', ROOT / 'local/behavior/steering-input-probe.exe', [ROOT / 'local/compatibility/grid-unpacked.exe', requests, out / 'original.tsv']),
        ('native', ROOT / f'target/{args.profile}/grid-steering-input.exe', [requests, out / 'native.tsv'])]:
        run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
        if run.returncode:
            (out / 'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Steering input execution failed')
    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            cells = line.split('\t')
            assert len(cells) == 3 and cells[0] == 'P' and cells[1] not in result
            result[cells[1]] = cells[2]
        return result
    original, native = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert original.keys() == native.keys() == {name for name, _ in cases}
    differences = [dict(case=name, original=original[name], native=native[name]) for name in original if original[name] != native[name]]
    result = dict(profile=args.profile, cases=len(cases), words=len(cases), exact_cases=len(cases)-len(differences),
                  differences=differences, executions=executions, input_sha256=digest(requests),
                  reference_sha256=digest(ROOT / 'local/compatibility/grid-unpacked.exe'),
                  limits='Whole original 98f790 and real trigonometry, RX mapped code, no patches/stubs/data writes/live injection. '
                         'Supplied body scalars; physical semantics/source initialization and whole wheel update remain open. '
                         'Finite bounded controls, phase up to +/-10000, not universal math-library equivalence.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k:result[k] for k in ('profile', 'cases', 'words', 'exact_cases')}))
    print('different_words=' + str(len(differences)))
    assert not differences

if __name__ == '__main__':
    main()
