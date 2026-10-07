"""Compare guarded placement-boundary captures with exact Rust replay inputs."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
from validate_binxml import decode
from validate_wheel_geometry import layout

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('capture', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--query-mode', choices=['f64', 'kernel', 'packed'], default='f64')
    parser.add_argument('--source-reports', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    for path in (args.capture, args.snapshot, out):
        if not path.resolve().is_relative_to((ROOT / 'local').resolve()):
            raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    capture = json.loads(args.capture.read_text(encoding='utf-8-sig'))
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    assert capture['completed'] and capture['code_exact'] and len(capture['cases']) == 12
    assert capture['session']['Pid'] == snapshot['pid']
    assert snapshot['read_only'] and not snapshot['process_suspended']
    cars = {int(c['body'], 16): c for c in snapshot['managers'][0]['cars']}
    sources = {}
    for car in ('lac', 'bmw'):
        ned = args.game_root / f'cars/{car}/{car}.ned'
        table = ned.with_suffix('.csv')
        attrs = next(e for e in decode(ned.read_bytes())[1].iter() if e.tag == 'WHEELS').attrib
        row = next(r for r in csv.reader(table.read_text().splitlines()) if r)
        _, _, _, positions, radii = layout(attrs, row)
        sources[car] = {'positions': positions, 'radii': radii, 'ned_sha256': digest(ned),
                        'csv_sha256': digest(table)}
    cases = []
    with (out / 'input.tsv').open('x', encoding='ascii') as stream:
        for case in capture['cases']:
            assert case['entry_registers'][0] == 0x9de4e0
            assert case['return_registers'][0] == int(case['return'], 16) == 0x9ef17d
            assert case['return_registers'][1] == case['entry_registers'][1] + 4
            stack = struct.unpack('<7I', bytes.fromhex(case['stack_hex']))
            assert stack[-1] == 0x461c4000  # Supported upper-Y branch: 10000.
            wheel_bytes = bytes.fromhex(case['wheels_hex'])
            heading = struct.unpack('<4I', bytes.fromhex(case['heading_hex']))[:3]
            inputs = [stack[3], 0, stack[4], *heading]
            positions, radii, displacements = [], [], []
            for wheel in range(4):
                offset = wheel * 0x150
                xyz = struct.unpack_from('<3I', wheel_bytes, offset)
                radius, = struct.unpack_from('<I', wheel_bytes, offset + 0xe0)
                dy, = struct.unpack_from('<I', wheel_bytes, offset + 0x40)
                inputs.extend([*xyz, radius, dy])
                positions.append(wheel_bytes[offset:offset + 16])
                radii.append(wheel_bytes[offset + 0xe0:offset + 0xe4])
                displacements.append(dy)
            matches = [car for car, source in sources.items()
                       if positions == [struct.pack('<4f', *p) for p in source['positions']]
                       and radii == [struct.pack('<f', r) for r in source['radii']]]
            assert len(matches) == 1
            assignment = cars[int(case['body'], 16)]
            name = f'car_{case["index"]:02d}'
            stream.write(name + '\t' + '\t'.join(f'{v:08x}' for v in inputs) + '\n')
            cases.append({'name': name, 'car': matches[0], 'grid_index': assignment['grid_index'],
                          'player': assignment['kind'] == 0, 'displacement_words': displacements,
                          'entry_xz_words': [f'{stack[3]:08x}', f'{stack[4]:08x}'],
                          'original_words': [f'{v:08x}' for v in struct.unpack('<16I', bytes.fromhex(case['pose_hex']))]})
    executable = ROOT / f'target/{args.profile}/grid-placement-track.exe'
    result = subprocess.run([str(executable), str(args.game_root), str(out / 'input.tsv'), str(out / 'native.tsv'), args.query_mode],
                            cwd=ROOT, capture_output=True, text=True, timeout=60)
    execution = {'executable_sha256': digest(executable), 'exit': result.returncode,
                 'stdout': result.stdout, 'stderr': result.stderr}
    if result.returncode:
        with (out / 'failure.json').open('x') as stream:
            json.dump(execution, stream, indent=2)
        raise RuntimeError('Rust replay failed; evidence retained')
    rows = [line.split('\t') for line in (out / 'native.tsv').read_text().splitlines()]
    poses = {r[1]: r for r in rows if r[0] == 'P'}
    queries = {c['name']: [r for r in rows if r[0] == 'Q' and r[1] == c['name']] for c in cases}
    value = lambda s: struct.unpack('<f', struct.pack('<I', int(s, 16)))[0]
    for case in cases:
        native = poses[case['name']][2:18]
        assert len(native) == 16 and len(queries[case['name']]) == 9
        differences = [value(a) - value(b) for a, b in zip(native, case['original_words'])]
        case.update(native_words=native, exact_words=sum(a == b for a, b in zip(native, case['original_words'])),
                    max_basis_error=max(abs(v) for v in differences[:12]),
                    horizontal_error=math.hypot(differences[12], differences[14]),
                    vertical_error=differences[13], queries=queries[case['name']])
    source_reports = []
    if args.source_reports:
        reports = {}
        for car in sources:
            path = args.source_reports / f'{car}-spawn-prototype.tsv'
            if not path.resolve().is_relative_to((ROOT / 'local').resolve()):
                raise ValueError('Private source reports only')
            source_reports.append({'path': str(path), 'sha256': digest(path)})
            reports[car] = {bytes.fromhex(r[1]).decode('ascii'): r for line in path.read_text().splitlines()
                            if (r := line.split('\t'))[0] == 'P'}
        for case in cases:
            source_row = reports[case['car']][f'slot_{case["grid_index"]:02d}']
            case['source_entry_xz_exact'] = [source_row[2], source_row[4]] == case['entry_xz_words']
            case['source_pose_words'] = source_row[5:21]
            case['source_pose_exact_words'] = sum(a == b for a, b in zip(source_row[5:21], case['original_words']))
            assert len(source_row[5:21]) == 16
    report = {'query_mode': args.query_mode, 'profile': args.profile, 'cases': cases, 'source_layouts': sources, 'source_reports': source_reports, 'execution': execution,
              'capture_sha256': digest(args.capture), 'snapshot_sha256': digest(args.snapshot),
              'track_sha256': digest(args.game_root / 'tracks/europe/donington/route_0/track.jpk'),
              'summary': {'cases': len(cases), 'exact_matrices': sum(c['exact_words'] == 16 for c in cases),
                          'exact_words': sum(c['exact_words'] for c in cases),
                          'max_basis_error': max(c['max_basis_error'] for c in cases),
                          'max_vertical_error': max(abs(c['vertical_error']) for c in cases),
                          'max_horizontal_error': max(c['horizontal_error'] for c in cases)},
              'limits': ('Original return-boundary comparison for this restart only. Rust uses source-built packed world/candidate/traversal queries and reference f32/SSE placement; no synchronized live query or driving equivalence.' if args.query_mode == 'packed' else 'Original return-boundary comparison for this restart only. Rust uses f64 candidate selection, optional f32 refinement of selected triangle, and reference f32/SSE placement; original whole query filtering remains unverified. No driving equivalence.')}
    if args.source_reports:
        report['summary'].update(source_entry_xz_exact=sum(c['source_entry_xz_exact'] for c in cases),
                                 source_exact_matrices=sum(c['source_pose_exact_words'] == 16 for c in cases),
                                 source_exact_words=sum(c['source_pose_exact_words'] for c in cases))
    with (out / 'validation.json').open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(report['summary']))


if __name__ == '__main__':
    main()
