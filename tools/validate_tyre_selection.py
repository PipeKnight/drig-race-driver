"""Compare source tyre selection with retained, read-only production contexts.

Original decoder comparison and constructor copy are static evidence. This does
not execute the original CSV decoder or claim full driving equivalence.
"""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def word(value):
    return struct.pack('<f', value)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence must remain private')
    out.mkdir(exist_ok=False)
    previous = json.loads((ROOT / 'local/behavior/native-body-prelude-validation/validation.json').read_text())
    sources = []
    native = ROOT / f'target/{args.profile}/grid-body-tuning.exe'
    for i, entry in enumerate(previous['csv_files']):
        path = args.game_root / entry['source']
        assert digest(path) == entry['sha256']
        rows = [r for r in csv.reader(path.read_text().splitlines()) if r]
        assert len(rows) == 1
        row = rows[0]
        assert row[2] == 'Standard' and (len(row) == 360 or len(row) == 361 and not row[-1])
        selected = float(row[311] or '0') > .5
        report = out / f'{i:03d}.tsv'
        subprocess.run([str(native), str(path), str(report), 'tyre'], cwd=ROOT,
                       check=True, capture_output=True, timeout=30)
        assert report.read_text().splitlines() == [f'T\t0\t{int(selected)}']
        legacy = out / f'{i:03d}-body.tsv'
        subprocess.run([str(native), str(path), str(legacy)], cwd=ROOT,
                       check=True, capture_output=True, timeout=30)
        baseline = ROOT / f'local/behavior/native-body-prelude-validation/{i:03d}.tsv'
        assert legacy.read_text().splitlines() == baseline.read_text().splitlines()
        sources.append(dict(source=entry['source'], sha256=entry['sha256'],
                            column311=row[311], coupled_tyres=selected, row=row))
    assert len(sources) == 44
    observations = []
    snapshots = []
    for name, expected_hash in [
        ('general-vehicle-manager-20261007T0837131885850Z.json',
         '4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
        ('general-vehicle-manager-20261007T0837468621304Z.json',
         'e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5'),
    ]:
        path = ROOT / 'local/behavior' / name
        assert digest(path) == expected_hash
        data = json.loads(path.read_text(encoding='utf-8-sig'))
        assert data['read_only'] and data['live_count_code_exact']
        assert len(data['managers']) == 1 and data['managers'][0]['count'] == 12
        blocks = {b['name']: b for b in data['blocks']}
        assert len(blocks) == len(data['blocks'])
        for car in data['managers'][0]['cars']:
            def read(field, length):
                b = blocks[f'car_{car["index"]}-{field}']
                assert b['success'] and b['length'] == b['read'] == length
                raw = bytes.fromhex(b['hex'])
                assert len(raw) == length
                return raw
            mass = read('mass_inertia_fields', 0x30)
            wheels = read('wheel_records', 4 * 0x150)
            context = read('dynamics_context', 0x740)
            assert struct.unpack_from('<II', context, 0x5d4) == (int(car['body'], 16), 4)
            candidates = []
            for source in sources:
                row = source['row']
                source_mass = word(float(row[3]))
                m = struct.unpack('<f', source_mass)[0]
                body_expected = [source_mass, word(1. / m)] + [word(float(row[c])) for c in (4, 5, 6)]
                if body_expected != [mass[o:o+4] for o in (0, 4, 8, 12, 16)]:
                    continue
                copies = {0xe0: (146, 145), 0x108: (84, 83), 0x100: (86, 85), 0x50: (88, 87),
                          0x48: (90, 89), 0x5c: (96, 95), 0x60: (94, 93), 0x70: (80, 79)}
                if all(word(float(row[columns[0 if w < 2 else 1]])) ==
                       wheels[w*0x150+o:w*0x150+o+4]
                       for w in range(4) for o, columns in copies.items()):
                    candidates.append(source)
            if len(candidates) != 1:
                failure = dict(snapshot=name, car=car['index'],
                               candidates=[s['source'] for s in candidates])
                (out / 'failure.json').write_text(json.dumps(failure, indent=2))
                raise ValueError('Source identity is ambiguous; evidence retained')
            source = candidates[0]
            flag = context[0x70c]
            assert flag in (0, 1) and bool(flag) == source['coupled_tyres']
            observations.append(dict(snapshot=name, car=car['index'], source=source['source'],
                                     column311=source['column311'], context70c=flag, exact=True))
        snapshots.append(dict(path=name, sha256=expected_hash))
    # Nearest f64 boundary values expose an incorrect early f32 conversion.
    controls = ['', '-1', '0', '0.49999999999999994', '0.5', '0.5000000000000001', '1']
    row = sources[0]['row'].copy()
    for i, text in enumerate(controls):
        row[311] = text
        path = out / f'boundary-{i}.csv'
        path.write_text(','.join(row) + '\r\n', encoding='ascii', newline='')
        report = out / f'boundary-{i}.tsv'
        subprocess.run([str(native), str(path), str(report), 'tyre'], cwd=ROOT,
                       check=True, capture_output=True, timeout=30)
        assert report.read_text().splitlines() == [f'T\t0\t{int(float(text or "0") > .5)}']
    result = dict(profile=args.profile, native_sha256=digest(native),
                  sources=[{k: v for k, v in s.items() if k != 'row'} for s in sources],
                  source_rows_exact=len(sources), active_source_rows=sum(s['coupled_tyres'] for s in sources),
                  legacy_body_reports_exact=len(sources),
                  observations=observations, production_flags_exact=len(observations),
                  snapshots=snapshots, authored_boundary_controls=len(controls),
                  limits='Static original decoder/constructor linkage; source and retained flag comparison. '
                         'No original decoder invocation or new live run; downstream tyre forces remain open.')
    with (out / 'validation.json').open('x') as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k not in ('sources', 'observations', 'snapshots')}))


if __name__ == '__main__':
    main()
