"""Investigate source bounds candidates against retained layout/rest nodes.

Candidate selection is explicit exploratory policy, not the original factory.
Original/repeat/Rust construct their own layout from each supplied candidate.
"""
import argparse
import itertools
import json
import math
import struct
from pathlib import Path
from survey_pssg_payloads import survey
from summarize_deformation_capture import load
from validate_binxml import decode
import validate_deformation_layout as layout

ROOT = layout.ROOT
STEAM = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
CAPTURES = {
    'general-vehicle-manager-20261008T2213398744933Z.json':
        'fdacccfec8fb847cf086129adf7cd5a8c8add585bf036163f16eb463e9532aa2',
    'general-vehicle-manager-20261008T2215472748164Z.json':
        'a71e5409c25dc6841906b0f2e65aa2b9ea89b9f9d86fce6ab788a75ba14c55c3',
}


def point(matrix, xyz):
    f = layout.f32
    return [f(f(f(f(xyz[0] * matrix[a]) + f(xyz[1] * matrix[4 + a]))
                + f(xyz[2] * matrix[8 + a])) + matrix[12 + a]) for a in range(3)]


def union(records):
    assert records
    # Candidate preaggregation: original source object's complete list is not
    # reconstructed here. Its final extrema are supplied explicitly instead.
    return [*[min(v[a] for v in records) for a in range(3)], 1.,
            *[max(v[4 + a] for v in records) for a in range(3)], 1.]


def candidates(stem, detail):
    result, identities = [], {}
    ned = STEAM / f'cars/{stem}/{stem}.ned'
    identities[str(ned)] = layout.digest(ned)
    tree = decode(ned.read_bytes())[1]
    collision = next(e for e in tree.iter() if e.tag == 'GEOMETRY' and e.get('id') == 'collision_main')
    points = [[layout.f32(float(v)) for v in e.get('pos').split()] for e in collision if e.tag == 'POINT']
    cb = [*[min(v[a] for v in points) for a in range(3)], 1.,
          *[max(v[a] for v in points) for a in range(3)], 1.]
    result.append(dict(name='collision-main-proxy', bounds=cb))
    strength = next(e for e in tree.iter() if e.tag == 'GEOMETRY' and e.get('id') == 'strength_main')
    boxes = []
    for box in strength:
        size = [layout.f32(float(v)) for v in box.get('size').split()]
        matrix = [layout.f32(float(v)) for v in box.get('transform').split()]
        assert box.tag == 'BOX' and len(size) == 3 and len(matrix) == 16
        f = layout.f32
        half = [f(f(f(abs(matrix[a]) * size[0]) + f(abs(matrix[4 + a]) * size[1]))
                  + f(abs(matrix[8 + a]) * size[2])) for a in range(3)]
        boxes.append([*[f(matrix[12 + a] - half[a]) for a in range(3)], 1.,
                      *[f(matrix[12 + a] + half[a]) for a in range(3)], 1.])
    result.append(dict(name='strength-union-proxy', bounds=union(boxes), strength_boxes=len(boxes)))
    path = STEAM / f'cars/{stem}/{stem}_{detail}.pssg'
    identities[str(path)] = layout.digest(path)
    raw, ps = path.read_bytes(), survey(path)
    nodes = ps['nodes']
    children = {i: [] for i in range(len(nodes))}
    for i, node in enumerate(nodes):
        if node['parent'] is not None:
            children[node['parent']].append(i)
    def name(i):
        return nodes[i]['attributes'].get('id', {}).get('string', '')
    def floats(i, count):
        n = nodes[i]
        assert n['payload_size'] == count * 4
        values = struct.unpack_from('>' + str(count) + 'f', raw, n['payload_offset'])
        assert all(math.isfinite(v) for v in values)
        return values
    ids = {name(i): i for i in range(len(nodes)) if name(i)}
    positions, indices = {}, {}
    for i, node in enumerate(nodes):
        if node['name'] != 'RENDERDATASOURCE':
            continue
        for j in children[i]:
            stream = nodes[j]
            if stream['name'] != 'RENDERSTREAM':
                continue
            a = stream['attributes']
            bi = ids[a['dataBlock']['string'][1:]]
            substreams = [k for k in children[bi] if nodes[k]['name'] == 'DATABLOCKSTREAM']
            ds = nodes[substreams[a['subStream']['u32be']]]['attributes']
            if ds['renderType']['string'] != 'Vertex':
                continue
            assert ds['dataType']['string'] == 'float3'
            data = next(k for k in children[bi] if nodes[k]['name'] == 'DATABLOCKDATA')
            count = nodes[bi]['attributes']['elementCount']['u32be']
            positions[name(i)] = [struct.unpack_from('>3f', raw, nodes[data]['payload_offset']
                                 + ds['offset']['u32be'] + k * ds['stride']['u32be']) for k in range(count)]
        ix = next(k for k in children[i] if nodes[k]['name'] == 'RENDERINDEXSOURCE')
        data = next(k for k in children[ix] if nodes[k]['name'] == 'INDEXSOURCEDATA')
        indices[name(i)] = struct.unpack_from('>' + str(nodes[data]['payload_size'] // 2) + 'H', raw, nodes[data]['payload_offset'])
    identity = tuple(1. if i in (0, 5, 10, 15) else 0. for i in range(16))
    bundles = [n['attributes'].get('id', {}).get('string', '') for n in nodes
               if n['name'] == 'MATRIXPALETTEBUNDLENODE']
    assert bundles and all(b.startswith('LOD') for b in bundles)
    for bundle in bundles:
        choices = [i for i, n in enumerate(nodes) if n['name'] == 'MATRIXPALETTEBUNDLENODE' and name(i) == bundle]
        assert len(choices) == 1
        parent = choices[0]
        for ancestor in [parent, nodes[parent]['parent']]:
            transform = next(j for j in children[ancestor] if nodes[j]['name'] == 'TRANSFORM')
            assert floats(transform, 16) == identity
        bounds = []
        vertex_bounds = {'stream': [], 'indexed': []}
        for i in children[parent]:
            if nodes[i]['name'] != 'MATRIXPALETTEJOINTNODE':
                continue
            bb = next(j for j in children[i] if nodes[j]['name'] == 'BOUNDINGBOX')
            tm = next(j for j in children[i] if nodes[j]['name'] == 'TRANSFORM')
            q, matrix = floats(bb, 6), floats(tm, 16)
            assert all(q[a] <= q[3 + a] for a in range(3))
            transformed = [point(matrix, [q[a + 3 * corner[a]] for a in range(3)])
                           for corner in itertools.product([0, 1], repeat=3)]
            value = [*[min(v[a] for v in transformed) for a in range(3)], 1.,
                     *[max(v[a] for v in transformed) for a in range(3)], 1.]
            bounds.append((name(i), value))
            selected = {'stream': [], 'indexed': []}
            for draw_index in children[i]:
                draw = nodes[draw_index]
                if draw['name'] != 'MATRIXPALETTERENDERINSTANCE':
                    continue
                a = draw['attributes']
                assert a['sourceCount']['u32be'] == 1 and a['streamCount']['u32be'] == 0
                source = a['indices']['string'][1:]
                first, count = a['streamOffset']['u32be'], a['elementCountFromOffset']['u32be']
                indexed_first, indexed_count = a['indexOffset']['u32be'], a['indicesCountFromOffset']['u32be']
                values = positions[source]
                selected_indices = indices[source][indexed_first:indexed_first + indexed_count]
                assert count > 0 and first + count <= len(values)
                assert len(selected_indices) == indexed_count and all(first <= v < first + count for v in selected_indices)
                selected['stream'].extend(point(matrix, v) for v in values[first:first + count])
                selected['indexed'].extend(point(matrix, values[k]) for k in set(selected_indices))
            for policy, points_ in selected.items():
                if points_:
                    record = [*[min(v[a] for v in points_) for a in range(3)], 1.,
                              *[max(v[a] for v in points_) for a in range(3)], 1.]
                    vertex_bounds[policy].append((name(i), record))
        policies = [
            ('all', lambda n: True),
            ('no-wheels', lambda n: not any(s in n for s in ['wheel', 'tyre', 'steering'])),
            ('main-body', lambda n: 'main_body' in n),
            ('no-aerial', lambda n: 'aerial' not in n),
        ]
        for label, predicate in policies:
            result.append(dict(name=detail + '-' + bundle + label,
                               bounds=union([v for n, v in bounds if predicate(n)]),
                               selected_joints=[n for n, _ in bounds if predicate(n)],
                               excluded_joints=[n for n, _ in bounds if not predicate(n)]))
        for policy, records in vertex_bounds.items():
            for label, predicate in [('all', policies[0][1]), ('no-aerial', policies[3][1])]:
                result.append(dict(name=detail + '-' + bundle + policy + '-' + label,
                                   bounds=union([v for n, v in records if predicate(n)]),
                                   selected_joints=[n for n, _ in records if predicate(n)],
                                   excluded_joints=[n for n, _ in records if not predicate(n)]))
    return (result if detail == 'highLOD' else result[2:]), identities


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', required=True, choices=['debug', 'release'])
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    p, flag = layout.parameters()
    rows, provenance, identities, source_extents = [], [], {}, {}
    for stem in ['lac', 'bmw']:
        for detail in ['highLOD', 'lowLOD']:
            cases, sources = candidates(stem, detail)
            identities.update(sources)
            if detail == 'highLOD':
                cb = cases[0]['bounds']
                source_extents[stem] = [layout.f32(layout.f32(cb[0] + cb[4]) * 0.5),
                                        cb[1], cb[6], cb[4], cb[0], cb[2], cb[5]]
            for case in cases:
                rows.append(dict(name=stem + '-' + case['name'], bounds=[case['bounds']],
                                 parameters=p, token=0, flag=flag, descriptor=bytes(layout.SIZE)))
                provenance.append(dict(stem=stem, **case))
    outputs, runs = layout.execute(out, 'source-candidates', layout.request(rows), args.profile)
    assert outputs['original'] == outputs['repeat'] == outputs['rust']
    observations = []
    for name, sha in CAPTURES.items():
        path = ROOT / 'local/behavior' / name
        assert layout.digest(path) == sha
        identities[str(path)] = sha
        _, blocks, cars = load(path)
        for car in cars:
            key = f"car_{car['car']}-"
            # Manager kind distinguishes controller classes, not car models.
            # Identify the model from seven independently source-derived NED
            # collision extents, without using the grid being compared.
            dimensions = blocks[key + 'dimension_fields']
            extents = [dimensions[o:o + 4] for o in [0, 4, 8, 12, 16, 36, 48]]
            matches = [stem for stem, values in source_extents.items()
                       if [struct.pack('<f', v) for v in values] == extents]
            assert len(matches) == 1, (car['car'], matches)
            stem = matches[0]
            actual = blocks[key + 'deformation_grid_descriptor']
            pool = blocks[key + 'deformation_grid_nodes']
            for i, case in enumerate(provenance):
                if case['stem'] != stem:
                    continue
                answer = outputs['original'][8 + i * layout.SIZE:8 + (i + 1) * layout.SIZE]
                fields = (answer[0xa414:0xa420] + answer[0xa420:0xa424] + answer[0xa430:0xa440]
                          == actual[:12] + actual[12:16] + actual[28:44])
                count = len(pool) // 80
                rest_words = sum(answer[0x2580 + n * 80 + 16 + lane * 4:0x2580 + n * 80 + 20 + lane * 4]
                                 == pool[n * 80 + 16 + lane * 4:n * 80 + 20 + lane * 4]
                                 for n in range(count) for lane in range(4))
                current_words = sum(answer[0x2580 + n * 80 + lane * 4:0x2580 + n * 80 + 4 + lane * 4]
                                    == pool[n * 80 + lane * 4:n * 80 + 4 + lane * 4]
                                    for n in range(count) for lane in range(4))
                observations.append(dict(snapshot=name, car=car['car'], stem=stem,
                                         source_identity_extent_words=7,
                                         candidate=case['name'], fields_equal=fields,
                                         rest_words_equal=rest_words, current_words_equal=current_words,
                                         total_node_words=count * 4, rest_exact=rest_words == count * 4))
    matches = [r for r in observations if r['fields_equal'] and r['rest_exact']]
    report = dict(profile=args.profile, candidates=provenance, executions=runs, identities=identities,
                  files={p.name: dict(size=p.stat().st_size, sha256=layout.digest(p))
                         for p in sorted(out.iterdir()) if p.is_file()},
                  original_repeat_rust_bytes_equal=True, descriptor_bytes=len(rows) * layout.SIZE,
                  observations=observations, matches=matches,
                  scope='Exploratory source bounding-box unions, independent Python PSSG/NED decoding '
                  'and whole original/Rust supplied layout. Matching retained layout/rest words does not '
                  'prove original part selection/object construction, live initialization or impact-ready '
                  'node limits/markers. No candidate is enabled in prototype startup.')
    (out / 'comparison.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(profile=args.profile, candidates=len(rows), matches=len(matches),
                         matching_candidates=sorted({(r['stem'], r['candidate']) for r in matches}),
                         matching_rest_words=sum(r['rest_words_equal'] for r in matches))))


if __name__ == '__main__':
    main()
