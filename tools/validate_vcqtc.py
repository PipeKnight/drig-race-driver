"""Independent GRID collision source/bit comparison and vertical grid probe."""
import argparse
import collections
import hashlib
import json
import math
import struct
from pathlib import Path


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def bits(value):
    return struct.pack('>f', value).hex()


def chunks(data):
    count = struct.unpack_from('<I', data, 8)[0]
    for i in range(count):
        words = struct.unpack_from('<8I', data, 32 + 32*i)
        name = data[words[0]:data.index(0, words[0])].decode('ascii')
        assert words[1] == words[3] and name.endswith('.vcqtc')
        d = data[words[2]:words[2]+words[3]]
        bounds = struct.unpack_from('<6f', d)
        nt, nv, nm = map(abs, struct.unpack_from('<3i', d, 24))
        vo, no, to, ro = struct.unpack_from('<4I', d, 36)
        assert vo == 52 + 4*nm and no == vo + nv*8 and ro == to + nt*4
        scale = [f32(f32(bounds[j+3] - bounds[j]) / (65536 if j == 1 else 16777216)) for j in range(3)]
        positions = []
        for v in range(nv):
            packed = d[vo+v*8:vo+(v+1)*8]
            integers = [int.from_bytes(packed[:3], 'big'), int.from_bytes(packed[3:5], 'big'), int.from_bytes(packed[5:], 'big')]
            positions.append([f32(f32(integer*scale[j]) + bounds[j]) for j, integer in enumerate(integers)])
        triangles = []
        for t in range(nt):
            a, b, c, e = d[to+t*4:to+(t+1)*4]
            assert a < 64, 'Upper packed-index bits outside validated subset'
            first = a*16 + (b >> 4)
            indices = [first, first+c, first+e]
            assert max(indices) < nv
            material = b & 15
            assert material < nm
            triangles.append((indices, material, 0, d[52+material*4:56+material*4].hex()))
        nodes = [int.from_bytes(d[p:p+2], 'big') for p in range(no, to, 2)]
        refs = {}
        p = ro
        while p < len(d):
            start = p-ro
            current = int.from_bytes(d[p:p+2], 'big')
            p += 2
            indices = [current]
            while d[p] != 255:
                delta = d[p]
                p += 1
                current += delta
                if delta != 254:
                    indices.append(current)
            p += 1
            assert max(indices) < nt
            refs[start] = indices
        for value in nodes:
            if value & 0x8000 and value != 0xffff:
                assert (value & 0x7fff) in refs
        yield name, positions, triangles, nodes, refs


def probe(positions, triangles, x, y, z):
    hits = []
    for i, (indices, material, flags, code) in enumerate(triangles):
        a, b, c = (positions[j] for j in indices)
        denominator = (b[2]-c[2])*(a[0]-c[0]) + (c[0]-b[0])*(a[2]-c[2])
        if abs(denominator) < 1e-12:
            continue
        u = ((b[2]-c[2])*(x-c[0]) + (c[0]-b[0])*(z-c[2])) / denominator
        v = ((c[2]-a[2])*(x-c[0]) + (a[0]-c[0])*(z-c[2])) / denominator
        w = 1-u-v
        if min(u, v, w) >= -1e-7:
            height = a[1]*u + b[1]*v + c[1]*w
            if height <= y:
                hits.append(dict(triangle=i, height=height, material=bytes.fromhex(code).decode('ascii'), flags=flags))
    return hits


if __name__ == '__main__':
    a = argparse.ArgumentParser()
    a.add_argument('game_root', type=Path)
    a.add_argument('native_report', type=Path)
    a.add_argument('--output', required=True, type=Path)
    args = a.parse_args()
    private = (Path(__file__).resolve().parent.parent/'local').resolve()
    if not args.output.resolve().is_relative_to(private):
        raise SystemExit('Evidence must remain private')
    source = args.game_root/'tracks/europe/donington/route_0/track.jpk'
    data = source.read_bytes()
    actual = iter(args.native_report.read_text().splitlines())
    counts = collections.Counter()
    flags = collections.Counter()
    mismatches = []
    grid_hits = []
    for name, positions, triangles, nodes, refs in chunks(data):
        expected = [['G', name, str(len(positions)), str(len(triangles)), str(len(nodes))]]
        expected += [['V', name, str(i), ','.join(map(bits, v))] for i, v in enumerate(positions)]
        expected += [['T', name, str(i), ','.join(map(str, indices)), str(mat), str(flag), code]
                     for i, (indices, mat, flag, code) in enumerate(triangles)]
        expected += [['N', name, str(i), f'{n:04x}'] for i, n in enumerate(nodes)]
        expected += [['P', name, str(offset), ','.join(map(str, indices))] for offset, indices in refs.items()]
        for row in expected:
            line = '\t'.join(row)
            native = next(actual, None)
            if native != line:
                if len(mismatches) < 20:
                    mismatches.append([line, native])
                counts['mismatches'] += 1
        for hit in probe(positions, triangles, -183.11911, -7.147942, 19.41256):
            grid_hits.append(dict(chunk=name, **hit))
        counts.update(chunks=1, vertices=len(positions), triangles=len(triangles), nodes=len(nodes), reference_lists=len(refs))
        flags.update(t[2] for t in triangles)
    if next(actual, None) is not None:
        raise SystemExit('Extra native report rows')
    grid_hits.sort(key=lambda h: h['height'], reverse=True)
    report = dict(exact=not mismatches, counts=dict(counts), flag_counts=dict(flags), mismatches=mismatches,
                  grid_probe=dict(x=-183.11911, z=19.41256, max_y=-7.147942, hits=grid_hits),
                  source_sha256=hashlib.sha256(data).hexdigest(), native_sha256=hashlib.sha256(args.native_report.read_bytes()).hexdigest())
    with args.output.open('x', encoding='utf-8') as f:
        json.dump(report, f, indent=2)
    print(json.dumps({k: report[k] for k in ('exact', 'counts', 'flag_counts', 'grid_probe')}))
    if not report['exact']:
        raise SystemExit('Collision reader disagrees with independent original-byte decoding')
