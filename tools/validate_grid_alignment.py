"""Compare isolated original marker alignment, Rust reports and live inputs.

Game-derived inputs and output remain private. Deformation state is not inferred
from a matching base bound or body offset.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import struct
import subprocess
from validate_binxml import decode
from validate_wheel_geometry import f32, layout, word

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def value(bits):
    return struct.unpack('<f', struct.pack('<I', int(bits, 16)))[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('placement_validation', type=Path)
    parser.add_argument('capture', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('source_reports', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    private = (ROOT / 'local').resolve()
    for path in (args.placement_validation, args.capture, args.snapshot,
                 args.source_reports, args.output):
        if not path.resolve().is_relative_to(private):
            raise ValueError('Private evidence only')
    out = args.output.resolve()
    out.mkdir(exist_ok=False)
    validation = json.loads(args.placement_validation.read_text())
    capture = json.loads(args.capture.read_text(encoding='utf-8-sig'))
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    assert validation['capture_sha256'] == digest(args.capture)
    assert validation['snapshot_sha256'] == digest(args.snapshot)
    assert capture['completed'] and capture['code_exact']
    assert len(capture['cases']) == len(validation['cases']) == 12
    assert snapshot['read_only'] and not snapshot['process_suspended']
    marker_path = private / 'behavior/grid-markers-native.tsv'
    marker_check = json.loads((private / 'behavior/grid-markers-validation.json').read_text())
    source_marker = args.game_root / 'tracks/europe/donington/route_0/grids.pssg'
    assert marker_check['exact'] and marker_check['source_sha256'] == digest(source_marker)
    markers = {bytes.fromhex(r[1]).decode('ascii'): r[18:27]
               for line in marker_path.read_text().splitlines()
               if (r := line.split('\t'))[0] == 'M'}
    blocks = {}
    for block in snapshot['blocks']:
        assert block['success'] and block['read'] == block['length']
        assert block['name'] not in blocks
        blocks[block['name']] = bytes.fromhex(block['hex'])
    sources, reports, provenance = {}, {}, []
    for car in ('lac', 'bmw'):
        ned = args.game_root / f'cars/{car}/{car}.ned'
        table = ned.with_suffix('.csv')
        tree = decode(ned.read_bytes())[1]
        wheel = next(e for e in tree.iter() if e.tag == 'WHEELS')
        row = next(r for r in csv.reader(table.read_text().splitlines()) if r)
        offset = layout(wheel.attrib, row)[2]
        geometry = next(e for e in tree.iter() if e.tag == 'GEOMETRY'
                        and e.get('id') == 'collision_main')
        points = [[f32(float(v)) for v in e.get('pos').split()]
                  for e in geometry if e.tag == 'POINT']
        assert points and all(len(p) == 3 for p in points)
        front = f32(max(p[2] for p in points) + offset[2])
        side = max(p[0] for p in points)
        path = args.source_reports / f'{car}-spawn-prototype.tsv'
        rows = [line.split('\t') for line in path.read_text().splitlines()]
        assert rows[0][0] == 'C' and rows[0][2] == word(front)
        assert rows[0][-1] == 'kernel'
        reports[car] = {bytes.fromhex(r[1]).decode('ascii'): r for r in rows if r[0] == 'P'}
        sources[car] = (front, side, offset)
        provenance.append({'car': car, 'ned_sha256': digest(ned), 'csv_sha256': digest(table),
                           'report_sha256': digest(path), 'front_extent_word': word(front),
                           'side_bound_word': word(side)})
    cases = []
    with (out / 'input.tsv').open('x', encoding='ascii') as stream:
        for index, (case, live) in enumerate(zip(validation['cases'], capture['cases'])):
            assert case['name'] == f'car_{index:02d}' and live['index'] == index
            marker = f'slot_{case["grid_index"]:02d}'
            corners = [value(v) for v in markers[marker]]
            front, side, offset = sources[case['car']]
            assert struct.pack('<4f', *offset) == blocks[f'car_{index}-frame_fields'][0x60:0x70]
            heading = [word(f32(corners[a] - corners[6 + a])) for a in range(3)]
            assert bytes.fromhex(live['heading_hex'])[:12] == b''.join(
                struct.pack('<I', int(v, 16)) for v in heading)
            stream.write(case['name'] + '\t' + '\t'.join(repr(v) for v in corners + [front, side]) + '\n')
            native = reports[case['car']][marker]
            cases.append({'name': case['name'], 'car': case['car'], 'grid_index': case['grid_index'],
                          'source_xz_words': [native[2], native[4]], 'source_heading_words': heading,
                          'live_xz_words': case['entry_xz_words'], 'body_offset_words_exact': 4,
                          'raw_heading_words_exact': 3})
    executable = private / 'behavior/grid-alignment-probe.exe'
    image = private / 'compatibility/grid-unpacked.exe'
    run = subprocess.run([str(executable), str(image), str(out / 'input.tsv'), str(out / 'original.tsv')],
                         cwd=ROOT, capture_output=True, text=True, timeout=30)
    execution = {'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr,
                 'executable_sha256': digest(executable), 'image_sha256': digest(image)}
    if run.returncode:
        (out / 'failure.json').write_text(json.dumps(execution, indent=2))
        raise RuntimeError('Isolated alignment failed; evidence retained')
    original = {r[1]: r[2:] for line in (out / 'original.tsv').read_text().splitlines()
                if (r := line.split('\t'))[0] == 'P'}
    assert set(original) == {c['name'] for c in cases}
    for case in cases:
        reference = original[case['name']]
        assert len(reference) == 5
        case['original_words'] = reference
        case['controlled_exact_words'] = sum(a == b for a, b in zip(
            reference, case['source_xz_words'] + case['source_heading_words']))
        case['live_xz_exact'] = reference[:2] == case['live_xz_words']
        case['live_xz_error'] = [value(a) - value(b) for a, b in zip(reference[:2], case['live_xz_words'])]
    report = {'cases': cases, 'sources': provenance, 'execution': execution,
              'capture_sha256': digest(args.capture), 'snapshot_sha256': digest(args.snapshot),
              'marker_report_sha256': digest(marker_path), 'marker_resource_sha256': digest(source_marker),
              'summary': {'cases': len(cases), 'controlled_exact_cases': sum(c['controlled_exact_words'] == 5 for c in cases),
                          'controlled_exact_words': sum(c['controlled_exact_words'] for c in cases),
                          'live_xz_exact_cases': sum(c['live_xz_exact'] for c in cases),
                          'raw_heading_words_exact': sum(c['raw_heading_words_exact'] for c in cases),
                          'body_offset_words_exact': sum(c['body_offset_words_exact'] for c in cases)},
              'limits': 'Isolated mode-one arithmetic with dimensions, speed, reset and placement stubs. '
                        'Live deformation flag/lattice and live marker corners were not captured. '
                        'Matching source headings and base body offsets do not establish live extent equality.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['controlled_exact_cases'] == 12


if __name__ == '__main__':
    main()
