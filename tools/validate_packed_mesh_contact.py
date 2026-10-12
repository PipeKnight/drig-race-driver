"""Repeat original car-mesh/packed-track geometry on checked private source data.

Native reference survey only; it is not a Rust generator compatibility claim.
"""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path

from validate_vcqtc import chunks

ROOT = Path(__file__).resolve().parents[1]
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
NATIVE = ROOT / 'local/behavior/packed-mesh-contact-probe.exe'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(request, answer):
    result = subprocess.run([str(NATIVE), str(IMAGE), str(request), str(answer)],
                            cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=result.returncode, stdout=result.stdout, stderr=result.stderr)


def check(answer, seed, chunk, center, limit, cache_limit, index_limit):
    u = lambda b, o: struct.unpack_from('<I', b, o)[0]
    f = lambda b, o: struct.unpack_from('<f', b, o)[0]
    assert len(answer) == 0x44f8+index_limit*8
    first = answer[:0x250]
    track = answer[0x250:0x4d0]
    root = answer[0x4d0:0xf50]
    pair = answer[0xf50:0xfa0]
    worker = answer[0xfa0:0xfe8]
    cache = answer[0x21c8:0x4218]
    header = answer[0x4498:0x44f8]
    count = u(worker, 0x10)
    candidates, seen = u(cache, 0x2c), u(cache, 0x30)
    assert count <= limit <= 16 and candidates <= index_limit and seen <= index_limit
    assert u(worker, 0) == 0xf000 and u(worker, 4) == u(worker, 0x14) == limit
    assert u(worker, 0xc) == 0xe000 and u(worker, 0x40) == 0xe000+count*128
    assert u(worker, 0x30) == int(bool(limit and cache_limit))
    assert u(worker, 0x34) == cache_limit and u(worker, 0x38) == 0x40000
    assert u(pair, 0x20) == (0xe000 if count else 0) and u(pair, 0x24) == count
    assert u(pair, 0x28) == 0x1000 and u(pair, 0x2c) == 0xc000
    expected_root = bytearray(0xa80)
    struct.pack_into('<3I', expected_root, 0x5f0, 0x7100, 0x7101, 0x7102)
    assert root == expected_root
    body_start = len(seed)-0x1120
    body = bytearray(seed[body_start:body_start+0x430])
    for j in range(3):
        for offset in [0x1c0, 0x2a0]:
            struct.pack_into('<f', body, offset+j*4, center[j])
    struct.pack_into('<I', body, 0x334, 0x1c0)
    assert answer[0x17e8:0x1c18] == body
    assert answer[0x1c18:0x1c58] == bytes(0x40)
    assert first[0x14:0x16] == track[0x14:0x16] == b'\0\1'
    assert u(first, 0) == 0x7000 and u(first, 0x10) == 7 and u(track, 0x10) == 11
    assert u(track, 0x170) == 0 and u(track, 0x244) == 0x60000
    assert u(cache, 0x20) == 0x400000 and u(cache, 0x24) == 0x500000
    assert u(cache, 0x28) == index_limit and u(cache, 0x38) == u(cache, 0x3c) == 0x50000
    assert u(cache, 0x40) == (0xc000 if limit and cache_limit else 0)
    nt, nv, nm = [-v for v in struct.unpack_from('<3i', chunk, 24)]
    assert header[:24] == chunk[:24]
    assert struct.unpack_from('<3I', header, 0x18) == (nt, nv, nm)
    offsets = struct.unpack_from('<4I', chunk, 36)
    assert struct.unpack_from('<4I', header, 0x24) == tuple(0x100000+v for v in offsets)
    assert struct.unpack_from('<'+'I'*nm, header, 0x34) == (0x1420,)*nm
    tri_base = 0x100000+offsets[2]
    ids = []
    for start, number in [(0x44f8, candidates), (0x44f8+index_limit*4, seen)]:
        values = struct.unpack_from('<'+'I'*number, answer, start)
        assert len(set(values)) == len(values)
        assert all(v >= tri_base and (v-tri_base) % 4 == 0 and (v-tri_base)//4 < nt for v in values)
        assert answer[start+number*4:start+index_limit*4] == bytes((index_limit-number)*4)
        ids.append([(v-tri_base)//4 for v in values])
    assert set(ids[0]) <= set(ids[1]) or seen == index_limit
    template = bytearray(128)
    struct.pack_into('<I', template, 0, 0x7200)
    template[0xf] = 1
    struct.pack_into('<3I', template, 0x1c, 0xffffffff, 0xffffffff, 1)
    assert answer[0xfe8+count*128:0x17e8] == template*(16-count)
    contacts = []
    for i in range(count):
        c = answer[0xfe8+i*128:0x1068+i*128]
        assert [u(c, o) for o in [4, 8, 0x28, 0x2c, 0x30, 0x34]] == [0xa000, 0, 0x1000, 0xc000, 0x1420, 0x1420]
        assert u(c, 0) == 0x7200 and c[0xc:0x10] == bytes([0, 0, 0, 1])
        assert c[0x10:0x1c] == bytes(12)
        assert struct.unpack_from('<3I', c, 0x1c) == (0xffffffff, 0xffffffff, 1)
        assert f(c, 0x3c) == struct.unpack('<f', bytes.fromhex('ffff7f7f'))[0]
        assert c[0x40:0x50] == bytes(16) and c[0x74:] == bytes(12)
        assert c[0x70] == c[0x72] == 0 and c[0x71] in [0, 1] and c[0x73] == 1
        point, normal = struct.unpack_from('<4f', c, 0x50), struct.unpack_from('<4f', c, 0x60)
        assert all(math.isfinite(v) for v in (*point, *normal, f(c, 0x38)))
        assert point[3] == 1 and normal[3] == 0
        assert math.isclose(sum(v*v for v in normal[:3]), 1, rel_tol=3e-4, abs_tol=3e-4)
        contacts.append(dict(point=point, normal=normal, penetration=f(c, 0x38)))
    if not limit or not cache_limit or not index_limit:
        assert count == candidates == 0
    return dict(contacts=count, candidates=candidates, seen=seen, contact_records=contacts,
                candidate_triangles=ids[0], visited_triangles=ids[1])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--max-shapes', type=int, default=245)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    assert 1 <= args.max_shapes <= 245
    out.mkdir(exist_ok=False)
    # Reuse a checked source-derived mesh/body; position is an authored control.
    upstream = ROOT / 'local/behavior/mesh-contact-20261012-slot-final-02/validation.json'
    previous = json.loads(upstream.read_text())
    assert previous['passed'] and previous['contact_slot_constructor']
    overlap = ROOT / previous['upstream']
    assert digest(overlap) == previous['upstream_sha256']
    overlap_report = json.loads(overlap.read_text())
    attachment = ROOT / overlap_report['upstream']
    assert digest(attachment) == overlap_report['upstream_sha256']
    attachment_report = json.loads(attachment.read_text())
    attachment_row = next(r for r in attachment_report['results']
                          if r.get('source') == r'cars\lac\lac.ned' and r['fill'] == 0)
    game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source', 'csv']:
        assert digest(game / attachment_row[key]) == attachment_row[key+'_sha256']
    seed_path = attachment.parent / f"{attachment_row['index']:03}-original.bin"
    assert digest(seed_path) == attachment_row['output_sha256']
    contact_row = next(r for r in previous['results']
                       if r['upstream_index'] == attachment_row['index'])
    seed_input = upstream.parent / f"{contact_row['index']:04}-input.bin"
    assert digest(seed_input) == contact_row['input_sha256']
    seed = seed_input.read_bytes()[40:]
    assert seed == seed_path.read_bytes()
    linkage = ROOT / 'local/behavior/runtime-grid-20261007-01/validation.json'
    link = json.loads(linkage.read_text())
    source = Path(link['source_path'])
    assert digest(source) == link['source_sha256']
    assert digest(IMAGE) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    raw = source.read_bytes()
    sections = {}
    for i in range(struct.unpack_from('<I', raw, 8)[0]):
        w = struct.unpack_from('<8I', raw, 32 + i*32)
        name = raw[w[0]:raw.index(0, w[0])].decode()
        assert w[1] == w[3]
        sections[name] = raw[w[2]:w[2]+w[3]]
    parsed = {name: (positions, triangles, refs)
              for name, positions, triangles, nodes, refs in chunks(raw)}
    rows = []
    for shape_index, record in enumerate(link['results'][:args.max_shapes]):
        name = record['source_chunk']
        positions, triangles, refs = parsed[name]
        # Prefer an upward facing source triangle for the initial floor survey.
        triangle_index = 0
        for ti, (ids, material, flags, code) in enumerate(triangles):
            a, b, c = [positions[k] for k in ids]
            ab = [b[k]-a[k] for k in range(3)]
            ac = [c[k]-a[k] for k in range(3)]
            n = [ab[1]*ac[2]-ab[2]*ac[1], ab[2]*ac[0]-ab[0]*ac[2],
                 ab[0]*ac[1]-ab[1]*ac[0]]
            if n[1] > .5 and n[1] > abs(n[0])+abs(n[2]):
                triangle_index = ti
                break
        a, b, c = [positions[k] for k in triangles[triangle_index][0]]
        centroid = [sum(v[k] for v in (a, b, c))/3 for k in range(3)]
        chunk = sections[name]
        for height in [10., 2., 0., -.5, -1.]:
            center = centroid.copy()
            center[1] += height
            index = len(rows)
            request = (struct.pack('<6I4f', 0x31304350, len(seed), len(chunk),
                                   16, 1, 4096, *center, 1.) + seed + chunk)
            inp = out / f'{index:04}-input.bin'
            inp.write_bytes(request)
            executions = []
            row = dict(index=index, shape_index=shape_index, chunk=name,
                       triangle=triangle_index, height=height, center=center,
                       input_sha256=digest(inp), executions=executions)
            for kind in ['original', 'repeat']:
                target = out / f'{index:04}-{kind}.bin'
                execution = run(inp, target)
                executions.append(execution)
                if execution['exit']:
                    (out / 'failure.json').write_text(json.dumps(row, indent=2))
                    raise RuntimeError(f'Original packed mesh contact failed: {row}')
            answer = (out / f'{index:04}-original.bin').read_bytes()
            assert answer == (out / f'{index:04}-repeat.bin').read_bytes()
            row.update(check(answer, seed, chunk, struct.unpack_from('<4f', request, 24), 16, 1, 4096), bytes=len(answer),
                       output_sha256=digest(out / f'{index:04}-original.bin'))
            rows.append(row)
        print(json.dumps(dict(shapes=shape_index+1, cases=len(rows),
                              contacts=sum(r['contacts'] for r in rows))), flush=True)
    # Reuse actual positive placements; capacity controls must come from execution.
    selected = [r for r in rows if r['contacts']][:24]
    for base in selected:
        sample = (out / f"{base['index']:04}-input.bin").read_bytes()
        seed_size, chunk_size = struct.unpack_from('<2I', sample, 4)
        seed_part, chunk_part = sample[40:40+seed_size], sample[40+seed_size:]
        assert len(chunk_part) == chunk_size
        for limit, cache_limit, index_limit in [(0, 1, 4096), (1, 1, 4096),
                                               (2, 1, 4096), (4, 1, 4096),
                                               (16, 0, 4096), (16, 1, 0),
                                               (16, 1, 1), (16, 1, 4)]:
            data = bytearray(sample)
            struct.pack_into('<3I', data, 12, limit, cache_limit, index_limit)
            index = len(rows)
            inp = out / f'{index:04}-input.bin'
            inp.write_bytes(data)
            executions = []
            for kind in ['original', 'repeat']:
                target = out / f'{index:04}-{kind}.bin'
                execution = run(inp, target)
                executions.append(execution)
                assert execution['exit'] == 0, execution
            answer = (out / f'{index:04}-original.bin').read_bytes()
            assert answer == (out / f'{index:04}-repeat.bin').read_bytes()
            observation = check(answer, seed_part, chunk_part,
                                struct.unpack_from('<4f', data, 24), limit, cache_limit, index_limit)
            rows.append(dict(index=index, base_index=base['index'],
                             contact_capacity=limit, cache_capacity=cache_limit,
                             candidate_capacity=index_limit, bytes=len(answer),
                             input_sha256=digest(inp),
                             output_sha256=digest(out / f'{index:04}-original.bin'),
                             executions=executions, **observation))
    sample = (out / '0000-input.bin').read_bytes()
    seed_size = struct.unpack_from('<I', sample, 4)[0]
    mesh_size = seed_size-0x1620
    chunk_start = 40+seed_size
    variants = [('short', sample[:-1]), ('long', sample+b'\0')]
    edits = [('tag', 0, 0), ('seed-size', 4, 0), ('chunk-size', 8, 0),
             ('contact-capacity', 12, 17), ('cache-capacity', 16, 2),
             ('candidate-capacity', 20, 32769), ('center-nan', 24, 0x7fc00000),
             ('center-inf', 28, 0x7f800000), ('center-bound', 32, 0x46200001),
             ('center-w', 36, 0), ('mesh-kind', 40+0x10, 6),
             ('mesh-reference', 40+0x240, 1), ('callback', 40+0x188, 1),
             ('special-geometry', 40+0x184, 1), ('linked-shape', 40+0xdc, 1),
             ('body-joint', 40+0x500+mesh_size+0x320, 1),
             ('mesh-points', 40+0x500, 81),
             ('mesh-adjacency', 40+0x500+0x60+0x10, 0xffffffff),
             ('body-center-nan', 40+0x500+mesh_size+0x1c0, 0x7fc00000),
             ('chunk-bounds-nan', chunk_start, 0x7fc00000),
             ('chunk-triangle-count', chunk_start+24, 0),
             ('chunk-vertex-offset', chunk_start+36, 0xffffffff)]
    for label, offset, value in edits:
        data = bytearray(sample)
        struct.pack_into('<I', data, offset, value)
        variants.append((label, data))
    guards = []
    for label, data in variants:
        inp, target = out / f'guard-{label}-input.bin', out / f'guard-{label}-output.bin'
        inp.write_bytes(data)
        execution = run(inp, target)
        assert execution['exit'] and not target.exists(), (label, execution)
        assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in execution['stdout']
        guards.append(dict(label=label, execution=execution, no_output=True))
    target = out / 'guard-existing-output.bin'
    target.write_bytes(b'preserve existing evidence')
    before = digest(target)
    execution = run(out / '0000-input.bin', target)
    assert execution['exit'] == 18 and digest(target) == before
    guards.append(dict(label='existing-valid-output', execution=execution, unchanged_sha256=before))
    assert digest(source) == link['source_sha256']
    report = dict(passed=True, native_reference_only=True, cases=len(rows),
                  contacts=sum(r['contacts'] for r in rows),
                  positive_cases=sum(r['contacts'] > 0 for r in rows), results=rows,
                  guards=guards, guard_count=len(guards),
                  observable_bytes=sum(r['bytes'] for r in rows),
                  source_path=str(source), source_sha256=digest(source),
                  upstream=str(upstream.relative_to(ROOT)), upstream_sha256=digest(upstream),
                  attachment=str(attachment.relative_to(ROOT)),
                  attachment_sha256=digest(attachment),
                  attachment_index=attachment_row['index'], seed_sha256=digest(seed_path),
                  image_sha256=digest(IMAGE), native_sha256=digest(NATIVE),
                  script_sha256=digest(Path(__file__)),
                  scope='Whole original B39500/AEA1F0, source packed descriptors, owned triangle cache and B7C5A0, B0ECE0 slots and scratch/startup. Source Lacetti mesh/body with authored translated poses, one direct prepared pair, shared zero material scalars. No broadphase/static constructor/full world, Rust generator, native movement or runtime barrier proof.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'positive_cases', 'contacts']}))


if __name__ == '__main__':
    main()
