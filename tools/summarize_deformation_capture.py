"""Summarize bounded private deformation captures and paired byte stability."""
import argparse
import json
import math
import struct
from pathlib import Path
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]


def load(path):
    path = path.resolve()
    assert path.is_relative_to((ROOT / 'local').resolve()) and path.stat().st_size <= 8 * 1024 * 1024
    capture = json.loads(path.read_text(encoding='utf-8-sig'))
    assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
    assert capture['live_count_code_exact'] and len(capture['managers']) == 1
    assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
    blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
    assert len(blocks) == len(capture['blocks'])
    rows = []
    for car in capture['managers'][0]['cars']:
        prefix = f"car_{car['index']}-"
        descriptor = blocks[prefix + 'deformation_grid_descriptor']
        dims = struct.unpack_from('<3i', descriptor)
        count = math.prod(dims)
        nodes = blocks[prefix + 'deformation_grid_nodes']
        assert len(nodes) == count * 80 and 0 < count <= 400
        pool = nodes + blocks.get(prefix + 'deformation_grid_padding_nodes', b'')
        assert len(pool) == 400 * 80
        edge_count, = struct.unpack('<i', blocks[prefix + 'deformation_edge_count'])
        assert 0 <= edge_count <= 1200
        edges = list(struct.iter_unpack('<hhf', blocks[prefix + 'deformation_edges']))
        assert len(edges) == edge_count and all(0 <= a < 400 and 0 <= b < 400 for a, b, _ in edges)
        tail_edges = [(a, b) for a, b, _ in edges if a >= count or b >= count]
        degrees = [0] * 400
        for a, b, _ in edges:
            degrees[a] += 1
            degrees[b] += 1
        state = blocks[prefix + 'deformation_grid_state']
        settings, = struct.unpack_from('<I', state, 0x58)
        position_w = [struct.unpack_from('<f', pool, i * 80 + 12)[0] for i in range(count)]
        rows.append(dict(car=car['index'], kind=car['kind'], dimensions=dims, sampled_nodes=count,
                         captured_pool_nodes=400, edges=edge_count, tail_edge_endpoints=tail_edges,
                         maximum_degree=max(degrees), mode=struct.unpack_from('<i', state)[0],
                         settings_pointer=f'{settings:x}', settings_relative_to_body=settings-int(car['body'], 16),
                         strain_settings=struct.unpack('<2f', blocks[prefix + 'deformation_strain_settings']),
                         wheel_radius_thresholds=struct.unpack('<2f', blocks[prefix + 'deformation_radius_thresholds']),
                         tyre_flags=list(blocks[prefix + 'deformation_tyre_flags']),
                         node_w_range=[min(position_w), max(position_w)]))
    return capture, blocks, rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('paired', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to((ROOT / 'local').resolve())
    first, a, rows = load(args.snapshot)
    second, b, _ = load(args.paired)
    assert (first['pid'], first['created_utc'], first['managers']) == (second['pid'], second['created_utc'], second['managers'])
    assert a.keys() == b.keys()
    changed = [dict(block=k, changed_bytes=sum(x != y for x, y in zip(a[k], b[k]))) for k in a if a[k] != b[k]]
    report = dict(snapshot_sha256=digest(args.snapshot), paired_sha256=digest(args.paired),
                  pid=first['pid'], reads=len(a), cars=rows, paired_changed_blocks=changed,
                  limits='Sequential read-only captures while UI paused; byte stability does not prove same-tick synchronization or update cadence.')
    with output.open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(dict(reads=len(a), cars=len(rows), changed_blocks=changed, player=rows[0])))


if __name__ == '__main__':
    main()
