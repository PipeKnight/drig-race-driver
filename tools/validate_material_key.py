"""Whole original raw packed-material resolver against source and authored keys."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    source = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
    source_sha = '8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    assert digest(source) == source_sha
    _, root, _ = decode(source.read_bytes())
    installed = [int.from_bytes(m.get('name').encode('ascii'), 'little') for m in root.findall('MATERIAL')]
    assert len(installed) == 89 and installed[0] == int.from_bytes(b'CAR_', 'little')
    packed = [int.from_bytes(k, 'little') for k in [b'GRAS', b'STDR', b'LGDR', b'RMBL', b'MPDR', b'MUD\0', b'CONC', b'ROCK']]
    rng = random.Random(96)
    datasets = [('installed', installed, installed + packed + [rng.getrandbits(32) for _ in range(64)]),
                ('authored', [0, 0xffffffff, 0x54534554, 0x54534554, 0x2044554d, 0x0044554d],
                 [0, 0xffffffff, 0x54534554, 0x2044554d, 0x0044554d, 0xdeadbeef]),
                ('empty', [], [0, 0xffffffff, 0x54534554])]
    executions = []
    comparisons = []
    for label, keys, codes in datasets:
        fixture = out / f'{label}.private.bin'
        fixture.write_bytes(b'KEY1' + struct.pack('<I', len(keys)) + b''.join(struct.pack('<I', k) for k in keys))
        cases = [(f'q{i}_{active}', code, active) for i, code in enumerate(codes) for active in [0, 1]]
        requests = out / f'{label}-input.tsv'
        requests.write_text(''.join(f'{name}\t{code:08x}\t{active:08x}\n' for name, code, active in cases))
        independent = []
        for name, code, active in cases:
            present = bool(keys) and bool(active)
            index = keys.index(code) if present and code in keys else 0
            independent.append(f'P\t{name}\t{int(present):08x}\t{index:08x}')
        for kind, exe, arguments in [
            ('original', ROOT / 'local/behavior/material-key-probe.exe',
             [ROOT / 'local/compatibility/grid-unpacked.exe', fixture, requests, out / f'{label}-original.tsv']),
            ('native', ROOT / f'target/{args.profile}/grid-material-key.exe',
             [fixture, requests, out / f'{label}-native.tsv'])]:
            result = subprocess.run([str(exe), *map(str, arguments)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            executions.append(dict(kind=kind, label=label, path=str(exe), sha256=digest(exe),
                                   exit=result.returncode, stdout=result.stdout, stderr=result.stderr))
            if result.returncode:
                (out / 'failure.json').write_text(json.dumps(executions, indent=2))
                raise RuntimeError('Material key execution failed')
        original = (out / f'{label}-original.tsv').read_text().splitlines()
        native = (out / f'{label}-native.tsv').read_text().splitlines()
        assert len(original) == len(native) == len(independent)
        differences = [dict(case=cases[i][0], original=a, native=b, independent=c)
                       for i, (a, b, c) in enumerate(zip(original, native, independent)) if not a == b == c]
        comparisons.append(dict(label=label, keys=len(keys), cases=len(cases), exact=len(cases)-len(differences),
                                input_sha256=digest(fixture), differences=differences))
    assert packed[5] not in installed and int.from_bytes(b'MUD ', 'little') in installed
    assert digest(source) == source_sha
    report = dict(profile=args.profile, source_sha256=source_sha, comparisons=comparisons, executions=executions,
                  limits='Whole original b3b0c0 with supplied ordered raw keys/stride4 and active flag. First exact DWORD match, first record on miss; inactive/empty supplied table absent. Not whole constructor, source loader, live material dereference or mechanics numeric parsing.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps([{k: row[k] for k in ['label', 'keys', 'cases', 'exact']} for row in comparisons]))
    assert all(not row['differences'] for row in comparisons)


if __name__ == '__main__':
    main()
