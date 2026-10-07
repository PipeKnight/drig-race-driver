"""Compare native car collision geometry, source bounds and saved live description words."""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode


def f32(value):
    return struct.unpack('<f', struct.pack('<f', float(value)))[0]


def word(value):
    return f'{struct.unpack("<I", struct.pack("<f", value))[0]:08x}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('source_links', type=Path)
    parser.add_argument('output_directory', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    out = args.output_directory.resolve()
    if not out.is_relative_to((root / 'local').resolve()):
        raise ValueError('Evidence must remain private')
    out.mkdir(exist_ok=False)
    reports, sources, differences, skipped = [], {}, [], []
    for path in sorted((args.game_root / 'cars').rglob('*.csv')):
        rows = [row for row in csv.reader(path.read_text().splitlines()) if row]
        if not rows or any(len(row) < 360 or row[2] != 'Standard' for row in rows):
            continue
        ned = path.with_suffix('.ned')
        relative = str(path.relative_to(args.game_root))
        if not ned.is_file():
            skipped.append({'source': relative, 'reason': 'No same-stem .ned; variant association unverified'})
            continue
        tree = decode(ned.read_bytes())[1]
        collision = [e for e in tree.iter() if e.tag == 'COLLISION']
        assert len(collision) == 1
        geometry = [e for e in collision[0] if e.tag == 'GEOMETRY' and e.get('id') == 'collision_main']
        assert len(geometry) == 1
        points = [[f32(v) for v in e.get('pos').split()] for e in geometry[0] if e.tag == 'POINT']
        triangles = [[int(v) for v in e.get('points').split()] for e in geometry[0] if e.tag == 'TRIANGLE']
        assert all(len(p) == 3 for p in points + triangles)
        assert 0 < len(points) <= 80 and 0 < len(triangles) <= 110
        assert all(0 <= i < len(points) for t in triangles for i in t)
        minimum = [min(p[a] for p in points) for a in range(3)]
        maximum = [max(p[a] for p in points) for a in range(3)]
        extents = [f32(f32(minimum[0] + maximum[0]) * .5), minimum[1], maximum[2],
                   maximum[0], minimum[0], minimum[2], maximum[1]]
        expected = [['P', str(i)] + [word(v) for v in p] for i, p in enumerate(points)]
        expected += [['T', str(i)] + [str(v) for v in t] for i, t in enumerate(triangles)]
        expected += [['B'] + [word(v) for v in extents]]
        native = out / f'{len(reports):02d}-native.tsv'
        subprocess.run([str(root / 'target/debug/grid-car-collision.exe'), str(ned), str(native)],
                       cwd=root, check=True, capture_output=True, text=True)
        actual = [line.split('\t') for line in native.read_text().splitlines()]
        assert actual == expected, relative
        wheel = [e for e in tree.iter() if e.tag == 'WHEELS']
        assert len(wheel) == 1
        radii = [f32(wheel[0].get('m_rear_r')), f32(wheel[0].get('m_front_r'))]
        for ordinal, row in enumerate(rows):
            copied = [f32(row[i]) for i in (146, 145)]
            if [word(v) for v in copied] != [word(v) for v in radii]:
                differences.append({'source': relative, 'row': ordinal, 'csv_early': copied, 'ned_final': radii})
        sources[relative] = (extents, radii)
        reports.append({'source': relative, 'ned_sha256': hashlib.sha256(ned.read_bytes()).hexdigest(),
                        'points': len(points), 'triangles': len(triangles), 'exact_native_rows': len(actual),
                        'description_extents': extents})
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    assert snapshot['read_only'] and not snapshot['input_sent'] and not snapshot['process_suspended']
    blocks = {}
    for block in snapshot['blocks']:
        assert block['success'] and block['read'] == block['length']
        assert block['name'] not in blocks
        blocks[block['name']] = bytes.fromhex(block['hex'])
    links = json.loads(args.source_links.read_text())
    live = []
    for car in links['cars']:
        index = car['index']
        assert len(car['wheel_source_candidates']) == 1
        source = car['wheel_source_candidates'][0]['source']
        extents, radii = sources[source]
        dimensions = blocks[f'car_{index}-dimension_fields']
        for value, offset in zip(extents, (0, 4, 8, 12, 16, 36, 48)):
            assert struct.pack('<f', value) == dimensions[offset:offset + 4], (index, offset)
        wheels = blocks[f'car_{index}-wheel_records']
        for wheel in range(4):
            radius = radii[0 if wheel < 2 else 1]
            for offset in (0xe0, 0xf4):
                start = wheel * 0x150 + offset
                assert struct.pack('<f', radius) == wheels[start:start + 4], (index, wheel, offset)
        live.append({'index': index, 'source': source, 'extent_words_exact': 7, 'radius_words_exact': 8})
    report = {'sources': reports, 'skipped': skipped, 'csv_ned_radius_differences': differences,
              'live_cars': live, 'extent_words_exact': sum(c['extent_words_exact'] for c in live),
              'radius_words_exact': sum(c['radius_words_exact'] for c in live),
              'snapshot': str(args.snapshot), 'snapshot_sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              'limits': 'Saved live bounds/radii for Lacetti/BMW; other cars have source/native agreement only. Corner extent queries, contacts and deformation are not implemented.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({'files_exact': len(reports), 'radius_difference_files': len({d['source'] for d in differences}),
                      'extent_words_exact': report['extent_words_exact'], 'radius_words_exact': report['radius_words_exact'],
                      'skipped': skipped}))


if __name__ == '__main__':
    main()
