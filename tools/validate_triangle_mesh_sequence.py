"""Compare retained triangle/mesh queries with whole original calls on source geometry.

PC05 preserves the support descriptor and partial-reset simplex between calls.
All executable evidence and source-derived requests must remain under local/.
"""
import argparse
import json
import math
import struct
from pathlib import Path

import validate_triangle_mesh_distance as distance

ROOT, digest = distance.ROOT, distance.digest
NATIVE = ROOT/'local/behavior/triangle-mesh-sequence-probe.exe'
COLD = ROOT/'local/behavior/triangle-mesh-sequence-cold-regression.exe'
UPSTREAM = distance.UPSTREAM


def controls(row, raw):
    seed_size = struct.unpack_from('<I', raw, 4)[0]
    nt = -struct.unpack_from('<i', raw, 40+seed_size+24)[0]
    ids = list(dict.fromkeys([row['triangle'], *row['candidate_triangles'],
                             *(i*nt//16 for i in range(min(16, nt)))]))[:30]
    requests = [(ids[0], 1, (1., 0., 0., 0.)), (ids[0], 0, (0., 0., 0., 0.))]
    for i, ti in enumerate(ids):
        mode = int(i % 3 == 0)
        requests.append((ti, mode, distance.DIRECTIONS[i % len(distance.DIRECTIONS)]))
    requests.extend((ti, 0, (0., 0., 0., 0.)) for ti in reversed(ids))
    assert 1 <= len(requests) <= 64
    return requests


def request(raw, calls, margin=None):
    records = b''.join(struct.pack('<II4f', ti, mode, *d) for ti, mode, d in calls)
    start = 44+len(records)
    b = bytearray(b'PC05'+raw[4:40]+struct.pack('<I', len(calls))+records+raw[40:])
    if margin is not None:
        struct.pack_into('<f', b, start+0x220, margin)
    return bytes(b)


def projection(answer, raw):
    count = struct.unpack_from('<I', raw, 40)[0]
    return answer[-4-count*448:]


def check(answer, raw):
    count = struct.unpack_from('<I', raw, 40)[0]
    seed_size, chunk_size, _, _, index_limit = struct.unpack_from('<5I', raw, 4)
    start = 44+count*24
    assert len(raw) == start+seed_size+chunk_size
    base_size = 0x44f8+index_limit*8+0xa0
    assert len(answer) == base_size+4+count*448
    records = projection(answer, raw)
    assert struct.unpack_from('<I', records)[0] == count
    assert records[-448:-160] == answer[0x1d38:0x1e58]
    assert records[-160:] == answer[base_size-160:base_size]
    last = raw[44+(count-1)*24:44+count*24]
    cold_request = b'PC04'+raw[4:40]+last[:4]+last[8:]+raw[start:]
    distance.check(answer[:base_size], cold_request)
    flags, counts, witnesses, changes = {}, {}, 0, 0
    prior = None
    for i in range(count):
        r = records[4+i*448:4+(i+1)*448]
        simplex, q = r[:0x120], r[0x120:]
        assert q[:4] == raw[44+i*24:48+i*24] and q[4:8] == bytes(4)
        assert q[8] <= 1 and q[9] <= 1 and q[10:16] == bytes(6) and q[0x90:] == bytes(16)
        assert simplex[0] <= 4 and all(v <= 1 for v in simplex[0x11c:0x11f])
        assert struct.unpack_from('<I', q, 0x40)[0] == 0x1000
        points = struct.unpack_from('<I', raw, start+0x500)[0]
        for v in struct.unpack_from('<7I', q, 0x44):
            assert v >= 0x30060 and (v-0x30060)%32 == 0 and (v-0x30060)//32 < points
        assert all(math.isfinite(v) for v in struct.unpack_from('<4f', q, 0x10))
        assert struct.unpack_from('<f', q, 0x1c)[0] == 0.
        if q[8]:
            assert all(math.isfinite(v) for v in struct.unpack_from('<8f', q, 0x20))
            assert struct.unpack_from('<f', q, 0x2c)[0] == struct.unpack_from('<f', q, 0x3c)[0] == 1.
        else:
            assert q[0x20:0x40] == bytes([0xcd])*32
        cache = q[0x44:0x60]
        changes += int(prior is not None and cache != prior)
        prior = cache
        witnesses += int(bool(q[8]))
        key = str(list(q[8:10])); flags[key] = flags.get(key, 0)+1
        counts[str(simplex[0])] = counts.get(str(simplex[0]), 0)+1
    return dict(queries=count, witness_queries=witnesses, cache_changes=changes,
                flags=flags, simplex_counts=counts)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--profile', choices=['debug', 'release'], default='debug')
    p.add_argument('--max-base', type=int, default=1225)
    p.add_argument('--controls', type=int, default=24)
    a = p.parse_args()
    assert 1 <= a.max_base <= 1225 and 0 <= a.controls <= 24
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    old = json.loads(UPSTREAM.read_text()); assert old['passed']
    assert digest(distance.IMAGE) == old['image_sha256']
    assert digest(Path(old['source_path'])) == old['source_sha256']
    rust = ROOT/f'target/{a.profile}/grid-convex-distance.exe'
    cases = [(r, None) for r in old['results'][:a.max_base]]
    selected = old['results'][:a.max_base:max(1, a.max_base//max(1, a.controls))][:a.controls]
    cases.extend((r, margin) for r in selected for margin in [.05, .5])
    rows = []
    for source_row, margin in cases:
        source = UPSTREAM.parent/f"{source_row['index']:04}-input.bin"
        assert digest(source) == source_row['input_sha256']
        raw = request(source.read_bytes(), controls(source_row, source.read_bytes()), margin)
        i = len(rows); inp = out/f'{i:04}-input.bin'; inp.write_bytes(raw)
        row = dict(index=i, source_index=source_row['index'], margin=margin,
                   input_sha256=digest(inp), executions=[])
        answers = []
        for name, exe, native in [('original', NATIVE, True), ('repeat', NATIVE, True), ('rust', rust, False)]:
            target = out/f'{i:04}-{name}.bin'; e = distance.run(exe, inp, target, native)
            row['executions'].append(e)
            if e['exit']:
                (out/'failure.json').write_text(json.dumps(row, indent=2))
                raise RuntimeError((i, name, e))
            answers.append(target.read_bytes())
        if answers[0] != answers[1] or projection(answers[0], raw) != answers[2]:
            (out/'failure.json').write_text(json.dumps(row, indent=2))
            raise RuntimeError(f'sequence mismatch {i}')
        row.update(check(answers[0], raw), bytes=len(answers[0]),
                   output_sha256=digest(out/f'{i:04}-original.bin'), rust_sha256=digest(out/f'{i:04}-rust.bin'))
        rows.append(row)
        if len(rows)%100 == 0: print(f'sequences={len(rows)} queries={sum(r["queries"] for r in rows)}', flush=True)
    raw = (out/'0000-input.bin').read_bytes()
    count = struct.unpack_from('<I', raw, 40)[0]; start = 44+count*24
    variants = [('short', raw[:-1]), ('long', raw+b'\0')]
    edits = [('tag', 0, 0), ('count-zero', 40, 0), ('count-large', 40, 65),
             ('first-carry', 48, 0), ('later-mode', 72, 2), ('later-index', 68, 0xffffffff),
             ('later-nan', 76, 0x7fc00000), ('later-w', 88, 0x3f800000),
             ('margin-negative', start+0x220, 0xbf800000), ('center-nan', 24, 0x7fc00000),
             ('mesh-adjacency', start+0x570, 0xffffffff)]
    for label, o, v in edits:
        b = bytearray(raw); struct.pack_into('<I', b, o, v); variants.append((label, bytes(b)))
    guards = []
    for label, b in variants:
        inp = out/f'guard-{label}-input.bin'; inp.write_bytes(b); executions = []
        for name, exe, native in [('original', NATIVE, True), ('rust', rust, False)]:
            target = out/f'guard-{label}-{name}.bin'; e = distance.run(exe, inp, target, native)
            assert 0 < e['exit'] < 256 and not target.exists(), (label, name, e)
            executions.append(e)
        guards.append(dict(label=label, input_sha256=digest(inp), executions=executions, no_output=True))
    for name, exe, native in [('original', NATIVE, True), ('rust', rust, False)]:
        target = out/f'guard-existing-{name}.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
        e = distance.run(exe, out/'0000-input.bin', target, native)
        assert e['exit'] and digest(target) == before
        guards.append(dict(label=f'existing-{name}', executions=[e], unchanged_sha256=before))
    report = dict(passed=True, profile=a.profile, cases=len(rows), results=rows, guards=guards, guard_count=len(guards),
                  queries=sum(r['queries'] for r in rows), witness_queries=sum(r['witness_queries'] for r in rows),
                  cache_changes=sum(r['cache_changes'] for r in rows), observable_bytes=sum(r['bytes'] for r in rows),
                  base_cases=a.max_base, control_seeds=a.controls, upstream=str(UPSTREAM.relative_to(ROOT)),
                  upstream_sha256=digest(UPSTREAM), image_sha256=digest(distance.IMAGE), native_sha256=digest(NATIVE),
                  rust_sha256=digest(rust), script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k:report[k] for k in ['passed', 'cases', 'queries', 'witness_queries', 'cache_changes', 'observable_bytes']}), flush=True)


if __name__ == '__main__': main()
