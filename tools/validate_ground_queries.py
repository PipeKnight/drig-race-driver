"""Independent plane/projected-barycentric checks against original VCQTC bytes.

Native implementation uses a BVH and Moller-Trumbore; this scans decoded source
triangles and intersects planes before choosing a 2D barycentric projection.
All coordinates, derivatives and comparison reports remain private.
"""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode
from validate_vcqtc import chunks


def sub(a, b):
    return [x-y for x, y in zip(a, b)]


def dot(a, b):
    return sum(x*y for x, y in zip(a, b))


def cross(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def prepare(source):
    result = []
    for name, positions, triangles, _, _ in chunks(source):
        for i, (indices, _, flags, material) in enumerate(triangles):
            points = [positions[j] for j in indices]
            a, b, c = points
            normal = cross(sub(b, a), sub(c, a))
            length = math.sqrt(dot(normal, normal))
            bounds = ([min(p[j] for p in points) for j in range(3)],
                      [max(p[j] for p in points) for j in range(3)])
            result.append(dict(name=name, triangle=i, points=points, normal=normal, length=length,
                               bounds=bounds, material=material, flags=flags))
    return result


def closest(triangles, start, end):
    direction = sub(end, start)
    direction_length = math.sqrt(dot(direction, direction))
    low = [min(a, b) for a, b in zip(start, end)]
    high = [max(a, b) for a, b in zip(start, end)]
    best = None
    for ordinal, triangle in enumerate(triangles):
        minimum, maximum = triangle['bounds']
        if any(high[i] < minimum[i] or low[i] > maximum[i] for i in range(3)):
            continue
        normal = triangle['normal']
        denominator = dot(normal, direction)
        if abs(denominator) <= 1e-12*triangle['length']*direction_length:
            continue
        a, b, c = triangle['points']
        fraction = dot(normal, sub(a, start))/denominator
        if fraction < -1e-12 or fraction > 1+1e-12:
            continue
        fraction = max(0., min(1., fraction))
        position = [start[i]+direction[i]*fraction for i in range(3)]
        drop = max(range(3), key=lambda i: abs(normal[i]))
        x, y = [i for i in range(3) if i != drop]
        area = (b[y]-c[y])*(a[x]-c[x])+(c[x]-b[x])*(a[y]-c[y])
        wa = ((b[y]-c[y])*(position[x]-c[x])+(c[x]-b[x])*(position[y]-c[y]))/area
        wb = ((c[y]-a[y])*(position[x]-c[x])+(a[x]-c[x])*(position[y]-c[y]))/area
        wc = 1-wa-wb
        if min(wa, wb, wc) < -1e-10:
            continue
        if best is None or fraction < best['fraction']-1e-12 or (abs(fraction-best['fraction']) <= 1e-12 and ordinal < best['ordinal']):
            best = dict(ordinal=ordinal, fraction=fraction, position=position,
                        normal=[v/triangle['length'] for v in normal], barycentric=[wa, wb, wc],
                        chunk=triangle['name'], triangle=triangle['triangle'],
                        material=triangle['material'], flags=triangle['flags'])
    return best


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('game_root', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    workspace = Path(__file__).resolve().parent.parent
    output = (args.output or workspace/'local/behavior/ground-validation').resolve()
    if not output.is_relative_to((workspace/'local').resolve()):
        raise SystemExit('Evidence must remain private')
    output.mkdir(exist_ok=False)
    source = args.game_root/'tracks/europe/donington/route_0/track.jpk'
    raw = source.read_bytes()
    triangles = prepare(raw)
    queries = []

    def vertical(name, point):
        x, y, z = point
        queries.append((name, [x, y+30., z], [x, y-30., z]))

    vertical('grid-known', [-183.11911, -12.147942, 19.41256])
    marker_path = workspace/'local/behavior/grid-markers-validation.json'
    markers = json.loads(marker_path.read_text())
    for marker in markers['results']:
        vertical(marker['id']+'-origin', marker['raw_origin'])
        vertical(marker['id']+'-center', marker['center'])
    progress_path = args.game_root/'tracks/europe/donington/route_0/progress_track.xml'
    progress_raw = progress_path.read_bytes()
    _, progress, _ = decode(progress_raw)
    for gate in progress.find('gates'):
        left = [f32(float(v)) for v in gate.find('left').text.split()]
        right = [f32(float(v)) for v in gate.find('right').text.split()]
        vertical('gate-'+gate.attrib['id'], [(a+b)/2 for a, b in zip(left, right)])
    for point in progress.find('points'):
        vertical('point-'+point.attrib['id'], [f32(float(v)) for v in point.find('position').text.split()])
    for i in range(20):
        triangle = triangles[i*len(triangles)//20]
        if triangle['length'] == 0:
            continue
        center = [sum(p[j] for p in triangle['points'])/3 for j in range(3)]
        normal = [v/triangle['length'] for v in triangle['normal']]
        start = [center[j]+normal[j]*2 for j in range(3)]
        end = [center[j]-normal[j]*2 for j in range(3)]
        queries.append((f'oblique-{i}', start, end))
        queries.append((f'reverse-{i}', end, start))
    queries += [('outside-down', [1e6, 10., 1e6], [1e6, -10., 1e6]),
                ('outside-flat', [1e6, 0., 1e6], [1e6+10., 0., 1e6]),
                ('zero-length', [0., 0., 0.], [0., 0., 0.])]
    requests = output/'queries.tsv'
    with requests.open('x') as f:
        for name, start, end in queries:
            f.write('\t'.join([name]+[repr(v) for v in start+end])+'\n')
    native = output/'native.tsv'
    process = subprocess.run([str(workspace/'target/debug/grid-ground.exe'), str(args.game_root), str(requests), str(native)], capture_output=True, text=True)
    (output/'native-run.txt').write_text(process.stdout+process.stderr)
    if process.returncode:
        raise SystemExit('native ground report failed: '+process.stderr)
    rows = [row.split('\t') for row in native.read_text().splitlines()]
    stats = rows.pop(0)
    assert stats[0] == 'M' and int(stats[1]) == len(triangles)
    assert len(rows) == len(queries)
    results = []
    for (name, start, end), actual in zip(queries, rows):
        assert actual[1] == name
        expected = closest(triangles, start, end)
        errors = []
        if expected is None:
            if actual[0] != 'N':
                errors.append('expected miss')
        elif actual[0] != 'H':
            errors.append('expected hit')
        else:
            if [actual[2], int(actual[3])] != [expected['chunk'], expected['triangle']]:
                errors.append('triangle identity')
            numbers = [expected['fraction']]+expected['position']+expected['normal']+expected['barycentric']
            if any(abs(float(a)-b) > 1e-8 for a, b in zip(actual[4:14], numbers)):
                errors.append('geometry difference above 1e-8')
            if actual[14] != expected['material'] or int(actual[15]) != expected['flags']:
                errors.append('material/flags')
        results.append(dict(name=name, matched=not errors, errors=errors, expected=expected, native=actual))
    tested = sum(int(row[-1]) for row in rows)
    summary = dict(queries=len(queries), matched=sum(r['matched'] for r in results),
                   hits=sum(row[0]=='H' for row in rows), triangles=len(triangles),
                   hierarchy_nodes=int(stats[2]), skipped_degenerate=int(stats[3]),
                   tested_triangles=tested, brute_force_tests=len(queries)*len(triangles),
                   coordinate_tolerance=1e-8, original_algorithm_equivalence=False)
    report = dict(summary=summary, source_sha256=hashlib.sha256(raw).hexdigest(),
                  progress_sha256=hashlib.sha256(progress_raw).hexdigest(),
                  marker_report_sha256=hashlib.sha256(marker_path.read_bytes()).hexdigest(), results=results)
    with (output/'validation.json').open('x') as f:
        json.dump(report, f, indent=2)
    print(json.dumps(summary))
    assert summary['matched'] == summary['queries']
