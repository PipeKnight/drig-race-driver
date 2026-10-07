"""Controlled original outer wrapper versus independent Rust numeric contract."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def value(bits):
    return struct.unpack('<f', struct.pack('<I', bits))[0]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    cases = [('unchanged', [word(.01)] + [word(v) for v in [1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 1]]
              + [0] * 8 + [word(10)]),
             ('constant_rate', [word(.125)] + [0] * 12 + [word(v) for v in [8, -4, 2, 0, 1, 2, 3, 0, 0]])]
    for index, delta in enumerate([.001, .002, .016, .017, 1 / 60, .25]):
        for adjacent in (-1, 0, 1):
            bits = word(delta) + adjacent
            if value(bits) > .25:
                continue
            cases.append((f'boundary_{index}_{adjacent + 1}', [bits] + [word(v) for v in
                         [1, -2, 3, 0, -4, 5, -6, 0, 80, -10, 120, 1, 1, -3, 5, 0, -2, 4, -6, 0, .5]]))
    rng = random.Random(2008)
    for index in range(120):
        values = [rng.uniform(1e-5, .25)] + [rng.uniform(-100, 100) for _ in range(12)]
        values += [rng.uniform(-300, 300) for _ in range(8)] + [rng.uniform(0, 100)]
        cases.append((f'synthetic_{index:03d}', [word(v) for v in values]))
    requests = out / 'input.tsv'
    with requests.open('x', encoding='ascii') as stream:
        for name, words in cases:
            assert len(words) == 22
            stream.write(name + '\t' + '\t'.join(f'{v:08x}' for v in words) + '\n')
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    programs = [(ROOT / 'local/behavior/dynamics-contribution-probe.exe',
                 [image, requests, out / 'original.tsv']),
                (ROOT / f'target/{args.profile}/grid-dynamics-contribution.exe',
                 [requests, out / 'native.tsv'])]
    executions = []
    for executable, arguments in programs:
        run = subprocess.run([str(executable), *map(str, arguments)], cwd=ROOT,
                             capture_output=True, text=True, timeout=30)
        executions.append({'path': str(executable), 'sha256': digest(executable), 'exit': run.returncode,
                           'stdout': run.stdout, 'stderr': run.stderr})
        if run.returncode:
            (out / 'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Controlled dynamics failed; evidence retained')
    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            row = line.split('\t')
            assert row[0] in ('S', 'P') and (row[0], row[1]) not in result
            assert len(row) == (7 if row[0] == 'S' else 14)
            result[(row[0], row[1])] = row[2:]
        return result
    original, native = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert set(original) == set(native) == {(kind, name) for name, _ in cases for kind in ('S', 'P')}
    comparisons = []
    for name, words in cases:
        first, second = original[('S', name)] + original[('P', name)], native[('S', name)] + native[('P', name)]
        differences = [{'field': i, 'original': a, 'native': b} for i, (a, b) in enumerate(zip(first, second)) if a != b]
        comparisons.append({'name': name, 'delta_word': f'{words[0]:08x}', 'substeps': int(first[0]),
                            'exact': not differences, 'differences': differences})
    report = {'profile': args.profile, 'executions': executions, 'image_sha256': digest(image), 'input_sha256': digest(requests),
              'cases': comparisons, 'summary': {'cases': len(cases),
              'exact_cases': sum(c['exact'] for c in comparisons), 'compared_float_words': len(cases) * 16,
              'exact_float_words': len(cases) * 16 - sum(sum(d['field'] != 0 for d in c['differences']) for c in comparisons)},
              'limits': 'Original 0x9f1e10 outer wrapper only. Inertia preparation, solver preparation, '
                        'substep kernel and finalizer are authored stubs; drive flag is zero. '
                        'No suspension/tyre forces, main frame cadence, collision, live state or driving equivalence.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases'] == len(cases)


if __name__ == '__main__':
    main()
