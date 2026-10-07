"""Independent source-triangle checks and limited post-start comparison for placement."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path
from validate_ground_queries import prepare, closest


def value(word):
    return struct.unpack('<f', struct.pack('<I', int(word, 16)))[0]


def bits(number):
    return struct.pack('<f', number).hex()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('output_directory', type=Path)
    parser.add_argument('--reports', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    out = args.output_directory.resolve()
    assert out.is_relative_to((root / 'local').resolve())
    out.mkdir(exist_ok=False)
    source = args.game_root / 'tracks/europe/donington/route_0/track.jpk'
    triangles = prepare(source.read_bytes())
    results, poses, inputs = [], {}, []
    for car in ['lac', 'bmw']:
        path = (args.reports or root / 'local/behavior') / f'{car}-spawn-prototype.tsv'
        inputs.append({'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        rows = [line.split('\t') for line in path.read_text().splitlines()]
        assert rows[0][0] == 'C' and rows[0][1] == car
        reference_math = len(rows[0]) > 6 and rows[0][6] == 'reference'
        queries = {}
        for row in rows[1:]:
            name = bytes.fromhex(row[1]).decode('ascii')
            if row[0] == 'Q':
                ordinal = int(row[2])
                x, z, upper, lower = map(value, row[3:7])
                hit = closest(triangles, [x, upper, z], [x, lower, z])
                if hit is None:
                    assert row[7] == 'none'
                else:
                    assert row[7] != 'none'
                    height = hit['position'][1]
                    if reference_math:
                        f32 = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
                        height = f32(upper - f32(f32(upper - lower) * f32(hit['fraction'])))
                    assert bits(height) == bits(value(row[7])), (car, name, ordinal)
                queries.setdefault(name, []).append({'ordinal': ordinal, 'hit': hit is not None,
                    'chunk': hit['chunk'] if hit else None, 'triangle': hit['triangle'] if hit else None,
                    'height': value(row[7]) if hit else None})
                if sum(len(v) for v in queries.values()) % 30 == 0:
                    print(car, 'queries checked', sum(len(v) for v in queries.values()), flush=True)
            elif row[0] == 'P':
                center = list(map(value, row[2:5]))
                matrix = list(map(value, row[5:21]))
                assert all(math.isfinite(v) for v in center + matrix)
                assert matrix[3::4] == [0., 0., 0., 1.]
                axes = [matrix[i:i + 3] for i in (0, 4, 8)]
                for axis in axes:
                    assert abs(sum(v * v for v in axis) - 1.) < 2e-6
                for i, j in [(0, 1), (0, 2), (1, 2)]:
                    assert abs(sum(a * b for a, b in zip(axes[i], axes[j]))) < 2e-6
                assert len(queries[name]) == 9 and [q['ordinal'] for q in queries[name]] == list(range(9))
                assert int(row[21]) == sum(not q['hit'] for q in queries[name][1:5])
                assert int(row[22]) == sum(q['hit'] for q in queries[name][5:9])
                poses[(car, name)] = matrix
                results.append({'car': car, 'marker': name, 'center': center, 'matrix': matrix, 'queries': queries[name]})
            else:
                raise ValueError('Unexpected report record')
    snapshot = json.loads(args.snapshot.read_text(encoding='utf-8-sig'))
    assert snapshot['read_only'] and not snapshot['input_sent'] and not snapshot['process_suspended']
    player = [c for c in snapshot['managers'][0]['cars'] if c['kind'] == 0]
    assert len(player) == 1
    player = player[0]
    block = next(b for b in snapshot['blocks'] if b['name'] == f"car_{player['index']}-frame_fields")
    assert block['success'] and block['read'] == block['length']
    live = list(struct.unpack_from('<16f', bytes.fromhex(block['hex']), 0x10))
    pose = poses[('lac', f"slot_{player['grid_index']:02d}")]
    differences = [a - b for a, b in zip(pose, live)]
    comparison = {'player_index': player['index'], 'grid_index': player['grid_index'], 'prototype': pose,
        'post_start_live': live, 'prototype_minus_live': differences,
        'horizontal_distance': math.hypot(differences[12], differences[14]),
        'vertical_difference': differences[13], 'maximum_basis_difference': max(abs(v) for v in differences[:12]),
        'meaning': 'Post-start settled pose only. Not a spawn-boundary or handling-fidelity check; no acceptance threshold asserted.'}
    report = {'track_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'inputs': inputs,
        'placements': results, 'placement_count': len(results), 'source_query_count': sum(len(r['queries']) for r in results),
        'source_height_words_exact': True, 'snapshot_sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
        'limited_player_comparison': comparison,
        'limits': 'Source triangle geometry only; original filters, candidate traversal, f32 height reconstruction, SIMD normalization, initialization boundary and suspension state remain unverified.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({key: report[key] for key in ('placement_count', 'source_query_count', 'source_height_words_exact', 'limited_player_comparison')}))


if __name__ == '__main__':
    main()
