"""Check inferred neighbor topology against hash-pinned retained GRID pools.

This is a read-only consistency check, not original execution or source setup.
"""
import argparse
import json
import struct
from pathlib import Path
from summarize_deformation_capture import load
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOTS = {
    'general-vehicle-manager-20261008T2213398744933Z.json':
        'fdacccfec8fb847cf086129adf7cd5a8c8add585bf036163f16eb463e9532aa2',
    'general-vehicle-manager-20261008T2215472748164Z.json':
        'a71e5409c25dc6841906b0f2e65aa2b9ea89b9f9d86fce6ab788a75ba14c55c3',
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to((ROOT / 'local').resolve())
    reports = []
    for name, expected_hash in SNAPSHOTS.items():
        path = ROOT / 'local/behavior' / name
        assert digest(path) == expected_hash, name
        _, blocks, rows = load(path)
        for row in rows:
            nx, ny, nz = row['dimensions']
            prefix = f"car_{row['car']}-"
            nodes = (blocks[prefix + 'deformation_grid_nodes']
                     + blocks[prefix + 'deformation_grid_padding_nodes'])
            actual = [(a, b) for a, b, _ in struct.iter_unpack('<hhf', blocks[prefix + 'deformation_edges'])]
            markers = [struct.unpack_from('<f', nodes, i * 80 + 64)[0] for i in range(400)]
            expected = []
            axes = dict(x=0, y=0, z=0)
            for z in range(nz):
                for y in range(ny):
                    for x in range(nx):
                        a = (z * ny + y) * nx + x
                        for axis, exists, b in [('x', x + 1 < nx, a + 1),
                                                ('y', y + 1 < ny, a + nx),
                                                ('z', z + 1 < nz, a + nx * ny)]:
                            if exists and markers[a] > 0 and markers[b] > 0:
                                expected.append((a, b))
                                axes[axis] += 1
            expected += [(399, 399)] * 3
            differences = [dict(index=i, expected=a, actual=b)
                           for i, (a, b) in enumerate(zip(expected, actual)) if a != b]
            reports.append(dict(snapshot=name, snapshot_sha256=expected_hash, car=row['car'],
                                dimensions=row['dimensions'], active_nodes=nx * ny * nz,
                                positive_markers=sum(v > 0 for v in markers),
                                edge_count=len(actual), expected_count=len(expected), axes=axes,
                                terminal_self_edges=actual[-3:], pairs_exact=actual == expected,
                                differences=differences))
    result = dict(grids=len(reports), edge_pairs=sum(r['edge_count'] for r in reports),
                  pair_bytes=sum(r['edge_count'] * 4 for r in reports),
                  passed=all(r['pairs_exact'] for r in reports), reports=reports,
                  scope='Retained snapshots versus independently reconstructed ordered positive-marker '
                  'XYZ neighbor pairs and three399 self-edges. Length words are not compared. '
                  'No original9FED60 execution, source marker construction, new live capture or Rust port.')
    with output.open('x', encoding='utf-8') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k not in ['reports', 'scope']}))
    assert result['passed'], 'Retained edge topology differs; report preserved'


if __name__ == '__main__':
    main()
