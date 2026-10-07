"""Check Rust body preludes against original CSV bytes and live body/wheel words."""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path


GROUP_COLUMNS = [[146, 84, 86, 88, 90, 96, 94, 80], [145, 83, 85, 87, 89, 95, 93, 79]]
WHEEL_OFFSETS = [0xe0, 0x108, 0x100, 0x50, 0x48, 0x5c, 0x60, 0x70]


def word(value):
    return f'{struct.unpack("<I", struct.pack("<f", value))[0]:08x}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('output_directory', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    assert args.output_directory.resolve().is_relative_to((root / 'local').resolve())
    args.output_directory.mkdir(exist_ok=False)
    native = root / 'target/debug/grid-body-tuning.exe'
    reports = {}
    files = []
    for path in sorted((args.game_root / 'cars').rglob('*.csv')):
        raw = path.read_bytes()
        rows = [row for row in csv.reader(raw.decode('utf-8').splitlines()) if row]
        if not all((len(row) == 360 or len(row) == 361 and not row[-1]) and row[2] == 'Standard' for row in rows):
            continue
        expected = []
        for ordinal, row in enumerate(rows):
            mass = struct.unpack('<f', struct.pack('<f', float(row[3])))[0]
            expected.append(['B', str(ordinal), word(mass), word(1.0 / mass)] + [word(float(row[i])) for i in (4, 5, 6)])
            for group, columns in enumerate(GROUP_COLUMNS):
                expected.append(['W', str(ordinal), str(group)] + [word(float(row[i])) for i in columns])
        output = args.output_directory / f'{len(files):03d}.tsv'
        subprocess.run([str(native), str(path), str(output)], check=True, capture_output=True, cwd=root)
        actual = [line.split('\t') for line in output.read_text().splitlines()]
        assert actual == expected, str(path)
        name = str(path.relative_to(args.game_root))
        reports[name] = actual
        files.append({'source': name, 'sha256': hashlib.sha256(raw).hexdigest(), 'rows': len(rows), 'exact': True})
    assert len(files) == 44
    snapshot = json.loads(args.snapshot.read_text())
    blocks = {block['name']: bytes.fromhex(block['hex']) for block in snapshot['blocks']}
    cars = []
    for car in snapshot['managers'][0]['cars']:
        mass = blocks[f"car_{car['index']}-mass_inertia_fields"]
        wheels = blocks[f"car_{car['index']}-wheel_records"]
        body_words = [f'{struct.unpack_from("<I", mass, offset)[0]:08x}' for offset in (0, 4, 8, 12, 16)]
        wheel_words = [[f'{struct.unpack_from("<I", wheels, wheel * 0x150 + offset)[0]:08x}'
                        for offset in WHEEL_OFFSETS] for wheel in range(4)]
        matches = []
        for name, rows in reports.items():
            if rows[0][2:] != body_words:
                continue
            if all(rows[1 + (0 if wheel < 2 else 1)][3:] == wheel_words[wheel] for wheel in range(4)):
                matches.append(name)
        assert len(matches) == 1
        cars.append({'index': car['index'], 'source': matches[0], 'float_words_exact': 37})
    assert len(cars) == 12
    result = {'csv_files': files, 'csv_files_exact': 44, 'cars': cars,
              'live_float_words_exact': sum(car['float_words_exact'] for car in cars),
              'snapshot_sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              'limits': 'Supported Standard prelude only; no complete vehicle or force-model equivalence.'}
    with (args.output_directory / 'validation.json').open('x', encoding='utf-8') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps({key: value for key, value in result.items() if key not in ('csv_files', 'cars')}))


if __name__ == '__main__':
    main()
