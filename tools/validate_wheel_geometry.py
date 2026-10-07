"""Independent .ned/CSV f32 reconstruction and native/live wheel-layout checks."""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def word(value):
    return f'{struct.unpack("<I", struct.pack("<f", value))[0]:08x}'


def layout(attributes, row):
    rear = [f32(float(attributes[f'm_rear_{axis}'])) for axis in 'xyzr']
    front = [f32(float(attributes[f'm_front_{axis}'])) for axis in 'xyzr']
    bias = f32(float(row[194]))
    points = [[axle[0] * side, f32(axle[1] - bias), axle[2], 1.0]
              for axle in (rear, front) for side in (1, -1)]
    radii = [rear[3], rear[3], front[3], front[3]]
    span = f32(f32(f32(points[0][0] - points[1][0]) + f32(points[2][0] - points[3][0])) * 0.5)
    height = f32(f32(span * f32(float(row[7]))) + bias)
    bottoms = [f32(point[1] - radius) for point, radius in zip(points, radii)]
    bottom_average = f32(f32(f32(f32(bottoms[2] + bottoms[1]) + bottoms[3]) + bottoms[0]) * 0.25)
    rear_z = f32(f32(points[0][2] + points[1][2]) * 0.5)
    front_z = f32(f32(points[2][2] + points[3][2]) * 0.5)
    sum_x = f32(f32(f32(points[0][0] + points[1][0]) + points[2][0]) + points[3][0])
    offset = [f32(-0.0 - f32(sum_x * 0.25)), f32(f32(-0.0 - height) - bottom_average),
              f32(-0.0 - f32(f32(f32(front_z - rear_z) * f32(float(row[8]))) + rear_z)), 0.0]
    shifted = [[f32(point[axis] + offset[axis]) for axis in range(3)] + [1.0] for point in points]
    return rear, front, offset, shifted, radii


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('source_links', type=Path)
    parser.add_argument('output_directory', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    out = args.output_directory.resolve()
    assert out.is_relative_to((root / 'local').resolve())
    out.mkdir(exist_ok=False)
    reports = []
    sources = {}
    skipped = []
    for path in sorted((args.game_root / 'cars').rglob('*.csv')):
        rows = [row for row in csv.reader(path.read_text().splitlines()) if row]
        if not rows or any(len(row) < 360 or row[2] != 'Standard' for row in rows):
            continue
        ned = path.with_suffix('.ned')
        if not ned.is_file():
            skipped.append({'source': str(path.relative_to(args.game_root)),
                            'reason': 'No same-stem .ned; variant-to-base association unverified'})
            continue
        document = decode(ned.read_bytes())[1]
        wheels = [element for element in document.iter() if element.tag == 'WHEELS']
        assert len(wheels) == 1
        native = out / f'{len(reports):02d}-native.tsv'
        subprocess.run([str(root / 'target/debug/grid-wheel-layout.exe'), str(ned), str(path), str(native)],
                       cwd=root, check=True, capture_output=True, text=True)
        expected = []
        layouts = []
        for ordinal, row in enumerate(rows):
            rear, front, offset, positions, radii = layout(wheels[0].attrib, row)
            if ordinal == 0:
                expected.extend([['G', name] + [word(v) for v in axle]
                                 for name, axle in [('rear', rear), ('front', front)]])
            expected.append(['O', str(ordinal)] + [word(v) for v in offset])
            expected.extend([['P', str(ordinal), str(wheel)] + [word(v) for v in position + [radii[wheel]]]
                             for wheel, position in enumerate(positions)])
            layouts.append((rear, front, offset, positions, radii))
        actual = [line.split('\t') for line in native.read_text().splitlines()]
        assert actual == expected, f'Native layout differs: {path}'
        relative = str(path.relative_to(args.game_root))
        sources[relative] = layouts
        reports.append({'source': relative, 'csv_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'ned_sha256': hashlib.sha256(ned.read_bytes()).hexdigest(), 'rows': len(rows),
                        'native_rows_exact': len(actual)})
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    assert snapshot['read_only'] and not snapshot['process_suspended'] and not snapshot['input_sent']
    blocks = {}
    for block in snapshot['blocks']:
        assert block['success'] and block['read'] == block['length']
        blocks[block['name']] = bytes.fromhex(block['hex'])
    links = json.loads(args.source_links.read_text())
    live = []
    for car in links['cars']:
        index = car['index']
        matches = car['wheel_source_candidates']
        assert len(matches) == 1
        source = matches[0]
        rear, front, offset, positions, radii = sources[source['source']][source['row']]
        pack = lambda values: struct.pack('<' + 'f' * len(values), *values)
        assert pack(rear + front) == blocks[f'car_{index}-source_wheel_geometry'][:32]
        assert pack(offset) == blocks[f'car_{index}-frame_fields'][0x60:0x70]
        initialized = blocks[f'car_{index}-initialized_wheel_geometry']
        assert pack([v for position in positions for v in position]) == initialized[:64]
        assert pack(radii) == initialized[64:80]
        records = blocks[f'car_{index}-wheel_records']
        for wheel, position in enumerate(positions):
            assert pack(position) == records[wheel * 0x150:wheel * 0x150 + 16]
        live.append({'index': index, 'source': source['source'], 'exact_words': 48,
                     'offset': offset, 'wheel_positions': positions})
    result = {'sources': reports, 'skipped': skipped, 'snapshot_sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              'live_cars': live, 'exact_live_words': sum(car['exact_words'] for car in live),
              'limits': 'Source/native agreement for supported files; original live agreement for Lacetti/BMW only. No forces or contact simulation.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps({'supported_files': len(reports), 'skipped': skipped, 'live_cars': len(live), 'exact_live_words': result['exact_live_words']}))


if __name__ == '__main__':
    main()
