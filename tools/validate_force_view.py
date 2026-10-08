"""Whole original four-wheel vector preparation versus independent Rust."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOTS = [
    ('general-vehicle-manager-20261007T0837131885850Z.json', '4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
    ('general-vehicle-manager-20261007T0837468621304Z.json', 'e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5'),
]
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]
def words(data, offset, count):
    return list(struct.unpack_from('<' + 'I' * count, data, offset))
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    cases, provenance = [], []
    identity = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.]
    base = [1., 0., 0., 0., 0., 0., 1., 0., 10., 1., 2., 0., .1, .2, -.3, 0.]
    for i in range(4):
        base += [[-.8, .2, -1.2, 1.], [.8, .2, -1.2, 1.], [-.8, .2, 1.3, 1.], [.8, .2, 1.3, 1.]][i] + [.1, .3, 1., 0.] + identity
    def add(name, row):
        assert len(row) == 112
        cases.append((name, [word(x) for x in row]))
    for i in range(24):
        v = base.copy()
        for w in range(4):
            k = 16 + w * 24
            v[k+6:k+8] = [[0., 0.], [1., 0.], [0., 1.], [-1., 0.], [.3, -.7], [-0., -0.]][i % 6]
            v[k+3] = [-0., 0., 1., -1.][i % 4]
            if i >= 12:
                v[k+20:k+24] = [100., -200., 300., 1.]
                v[11] = -0.
            if i >= 18:
                v[k+4] = -0.
                v[k+5] = 0.
                v[k+1] = [-0., 0., 1e-20, -1e-20][w]
        add(f'analytic_{i}', v)
    v = base.copy()
    v[15] = 1e30
    for w in range(4):
        for j in range(4):
            v[16+w*24+8+j*4+3] = 1e28
    add('unused_cross_w_overflow', v)
    for s, (name, expected) in enumerate(SNAPSHOTS):
        path = ROOT / 'local/behavior' / name
        assert digest(path) == expected
        data = json.loads(path.read_text(encoding='utf-8-sig'))
        assert data['read_only'] and data['live_count_code_exact']
        assert len(data['managers']) == 1 and data['managers'][0]['count'] == 12
        blocks = {b['name']: bytes.fromhex(b['hex']) for b in data['blocks'] if b['success'] and b['read'] == b['length']}
        aliases = {}
        for car in data['managers'][0]['cars']:
            index, address = car['index'], int(car['body'], 16)
            assert car['wheel_count'] == 4
            prefix = f'car_{index}'
            header, frame, mass, wheels = (blocks[prefix + '-' + kind] for kind in ('body_header', 'frame_fields', 'mass_inertia_fields', 'wheel_records'))
            scratch = words(header, 0, 1)[0]
            assert scratch != address + 0x3d0
            aliases.setdefault(hex(scratch), []).append(index)
            row = words(frame, 0x10, 4) + words(frame, 0x30, 4) + words(mass, 0x20, 4) + words(frame, 0, 4)
            matrix = words(frame, 0x10, 16)
            for w in range(4):
                k = w * 0x150
                assert words(wheels, k+0x34, 1)[0] == address + 0x15a0
                row += words(wheels, k, 4) + words(wheels, k+0x40, 1) + words(wheels, k+0xe0, 1) + words(wheels, k+0x20, 2) + matrix
            assert len(row) == 112
            cases.append((f'retained_{s}_{index}', row))
        assert len(aliases) == (2 if s == 0 else 3)
        provenance.append(dict(path=str(path.relative_to(ROOT)), sha256=expected, force_pointer_aliases=aliases,
                               matrix_links_verified=48, non_atomic=True))
    rng = random.Random(115)
    for i in range(512):
        v = base.copy()
        v[:16] = [rng.uniform(-5, 5) for _ in range(16)]
        for w in range(4):
            k = 16 + w*24
            v[k:k+8] = [rng.uniform(-3, 3) for _ in range(8)]
            if i >= 128:
                v[k+8:k+24] = [rng.uniform(-2, 2) for _ in range(16)]
        add(f'authored_{i}', v)
    requests = out/'input.tsv'
    requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in row)+'\n' for name, row in cases))
    executions = []
    for kind, exe, argv in [
        ('original', ROOT/'local/behavior/force-view-probe.exe', [ROOT/'local/compatibility/grid-unpacked.exe', requests, out/'original.tsv']),
        ('native', ROOT/f'target/{args.profile}/grid-force-view.exe', [requests, out/'native.tsv'])]:
        run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Force-view comparison failed')
    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            cells = line.split('\t')
            assert cells[0] == 'P' and len(cells) == 66 and cells[1] not in result
            result[cells[1]] = cells[2:]
        return result
    original, native = rows(out/'original.tsv'), rows(out/'native.tsv')
    assert original.keys() == native.keys() == {name for name, _ in cases}
    differences = [dict(case=name, field=i, original=a, native=b) for name in original for i, (a,b) in enumerate(zip(original[name],native[name])) if a != b]
    result = dict(profile=args.profile, executions=executions, cases=len(cases), words=len(cases)*64,
                  exact_cases=len(cases)-len({d['case'] for d in differences}), differences=differences, provenance=provenance,
                  limits='Whole original 9b2790 with original cross helper; no code/data patches or stubs/live injection. '
                         '24 retained non-atomic body/wheel states with all 96 matrix links verified to body+15a0. '
                         'Original isolated outputs versus Rust, not captured original live temporary-buffer contents. '
                         'No synchronized update timing, contact identity or complete driving claim.')
    (out/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k:result[k] for k in ('profile','cases','words','exact_cases')}))
    print('different_words='+str(len(differences)))
    assert not differences
if __name__ == '__main__':
    main()
