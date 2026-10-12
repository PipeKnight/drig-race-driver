"""Whole original outer/penetration queries versus source-input Rust.

Sampling data is independently prepared from identified private/source PE data.
All original, repeat and Rust outputs stay under ignored local/.
"""
import argparse
import json
import math
import struct
import subprocess
from pathlib import Path

import validate_triangle_mesh_sequence as sequence
from validate_triangle_mesh_sequence import ROOT, digest, distance

NATIVE_SEQUENCE = ROOT/'local/behavior/triangle-mesh-outer-sequence-probe.exe'
COLD = ROOT/'local/behavior/triangle-mesh-distance-debug-20261012-final-01/validation.json'
RETAINED = ROOT/'local/behavior/triangle-mesh-sequence-debug-20261012-final-01/validation.json'
SAMPLING = ROOT/'local/behavior/contact-directions-source-20261012-final-02/validation.json'


def run(exe, inp, out, native=True, sampling=None):
    command = [str(exe), str(distance.IMAGE), str(inp), str(out)] if native else [str(exe), str(inp), str(out), str(sampling or SAMPLING.parent/'directions.bin')]
    e = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=e.returncode, stdout=e.stdout, stderr=e.stderr)


def projection(answer, raw):
    if raw[:4] == b'PC06':
        count = struct.unpack_from('<I', raw, 40)[0]
        return answer[-4-count*608:]
    return distance.projection(answer)+answer[0x1c58+0x4d0:0x1c58+0x570]


def check_record(record):
    assert len(record) == 608
    simplex, q, extras = record[:0x120], record[0x120:0x1c0], record[0x1c0:]
    hit = struct.unpack_from('<I', q, 4)[0]
    assert hit in [0, 1] and q[8:16] == bytes(8) and q[0x90:] == bytes(16)
    assert simplex[0] <= 4 and all(v <= 1 for v in simplex[0x11c:0x11f])
    assert all(math.isfinite(v) for v in struct.unpack_from('<4f', q, 0x10))
    assert struct.unpack_from('<f', q, 0x1c)[0] == 0.
    if hit:
        assert all(math.isfinite(v) for v in struct.unpack_from('<8f', q, 0x20))
        assert struct.unpack_from('<f', q, 0x2c)[0] == struct.unpack_from('<f', q, 0x3c)[0] == 1.
    else:
        assert q[0x20:0x40] == bytes([0xcd])*32
    assert all(math.isfinite(v) for v in struct.unpack('<40f', extras))
    assert all(struct.unpack_from('<f', extras, i*16+12)[0] == 0. for i in range(10))
    return dict(hit=hit, simplex_count=simplex[0], extra_data=extras != bytes(160))


def check(answer, raw, table):
    is_sequence = raw[:4] == b'PC06'
    count = struct.unpack_from('<I', raw, 40)[0] if is_sequence else 1
    index_limit = struct.unpack_from('<I', raw, 20)[0]
    base_size = 0x44f8+index_limit*8+0xa0
    assert len(answer) == base_size+(4+count*608 if is_sequence else 0)
    assert answer[0x1c58+0x210:0x1c58+0x4d0] == table[8:]
    base = bytearray(answer[:base_size])
    hit = struct.unpack_from('<I', base, base_size-0xa0+4)[0]
    # Reuse the empty-arena/witness audit after explicit normalization of the
    # outer hit to that checker's first distance flag; captured bytes stay intact.
    struct.pack_into('<I', base, base_size-0xa0+4, 0)
    base[base_size-0xa0+8] = hit
    cold_raw = raw
    if is_sequence:
        start = 44+count*24; last = raw[44+(count-1)*24:44+count*24]
        cold_raw = b'PC04'+raw[4:40]+last[:4]+last[8:]+raw[start:]
    distance.check(base, cold_raw)
    records = projection(answer, raw)
    if is_sequence:
        assert struct.unpack_from('<I', records)[0] == count
        records = records[4:]
        assert records[-608:-320] == answer[0x1d38:0x1e58]
        assert records[-320:-160] == answer[base_size-160:base_size]
        assert records[-160:] == answer[0x1c58+0x4d0:0x1c58+0x570]
    rows = [check_record(records[i*608:(i+1)*608]) for i in range(count)]
    for i in range(count):
        q = records[i*608+0x120:i*608+0x1c0]
        index = raw[44+i*24:48+i*24] if is_sequence else raw[40:44]
        assert q[:4] == index and struct.unpack_from('<I', q, 0x40)[0] == 0x1000
        start = 44+count*24 if is_sequence else 60
        points = struct.unpack_from('<I', raw, start+0x500)[0]
        for v in struct.unpack_from('<7I', q, 0x44):
            assert v >= 0x30060 and (v-0x30060)%32 == 0 and (v-0x30060)//32 < points
    return dict(queries=count, hits=sum(r['hit'] for r in rows), extra_data_queries=sum(r['extra_data'] for r in rows),
                simplex_counts={str(n):sum(r['simplex_count'] == n for r in rows) for n in range(5)})


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path)
    p.add_argument('--profile', choices=['debug', 'release'], default='debug')
    p.add_argument('--sequence', action='store_true'); p.add_argument('--max-cases', type=int)
    a = p.parse_args(); reference = RETAINED if a.sequence else COLD
    previous = json.loads(reference.read_text()); assert previous['passed']
    sampling = json.loads(SAMPLING.read_text()); table_path = SAMPLING.parent/'directions.bin'; table = table_path.read_bytes()
    assert sampling['passed'] and digest(table_path) == sampling['output_sha256']
    assert digest(Path(sampling['source_path'])) == sampling['source_sha256']
    from extract_contact_directions import extract
    assert extract()[0] == table
    native = NATIVE_SEQUENCE if a.sequence else distance.OUTER
    rust = ROOT/f'target/{a.profile}/grid-convex-distance.exe'
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    rows = []
    cases = previous['results'][:a.max_cases] if a.max_cases else previous['results']
    for source in cases:
        i = len(rows); inp = reference.parent/f"{source['index']:04}-input.bin"; assert digest(inp) == source['input_sha256']
        raw = (b'PC06' if a.sequence else b'PC03')+inp.read_bytes()[4:]
        target_input = out/f'{i:04}-input.bin'; target_input.write_bytes(raw)
        row = dict(index=i, source_index=source['index'], input_sha256=digest(target_input), executions=[])
        answers = []
        for name, exe, is_native in [('original', native, True), ('repeat', native, True), ('rust', rust, False)]:
            target = out/f'{i:04}-{name}.bin'; e = run(exe, target_input, target, is_native); row['executions'].append(e)
            if e['exit']:
                (out/'failure.json').write_text(json.dumps(row, indent=2)); raise RuntimeError((i, name, e))
            answers.append(target.read_bytes())
        if answers[0] != answers[1] or projection(answers[0], raw) != answers[2]:
            (out/'failure.json').write_text(json.dumps(row, indent=2)); raise RuntimeError(f'outer output mismatch {i}')
        row.update(check(answers[0], raw, table), bytes=len(answers[0]),
                   output_sha256=digest(out/f'{i:04}-original.bin'), rust_sha256=digest(out/f'{i:04}-rust.bin'))
        rows.append(row)
        if len(rows)%100 == 0: print(f'cases={len(rows)} queries={sum(r["queries"] for r in rows)} hits={sum(r["hits"] for r in rows)}', flush=True)
    guards = []
    for g in previous['guards']:
        label = g['label']
        if label.startswith('existing-'): continue
        inp = reference.parent/f'guard-{label}-input.bin'
        raw = inp.read_bytes()
        if raw[:4] in [b'PC04', b'PC05']: raw = (b'PC06' if a.sequence else b'PC03')+raw[4:]
        target_input = out/f'guard-{label}-input.bin'; target_input.write_bytes(raw)
        executions = []
        for name, exe, is_native in [('original', native, True), ('rust', rust, False)]:
            target = out/f'guard-{label}-{name}.bin'; e = run(exe, target_input, target, is_native)
            assert 0 < e['exit'] < 256 and not target.exists(), (label, name, e); executions.append(e)
        guards.append(dict(label=label, input_sha256=digest(target_input), executions=executions, no_output=True))
    for name, exe, is_native in [('original', native, True), ('rust', rust, False)]:
        target = out/f'guard-existing-{name}.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
        e = run(exe, out/'0000-input.bin', target, is_native); assert e['exit'] and digest(target) == before
        guards.append(dict(label=f'existing-{name}', executions=[e], unchanged_sha256=before))
    variants = [('short', table[:-1]), ('long', table+b'\0')]
    for label, o, v in [('tag', 0, 0), ('count', 4, 45), ('nan', 8, 0x7fc00000), ('w', 20, 0x3f800000), ('zero', 16, 0), ('bound', 8, 0x40400000)]:
        b = bytearray(table); struct.pack_into('<I', b, o, v); variants.append((label, bytes(b)))
    for label, b in variants:
        target_input = out/f'guard-table-{label}.bin'; target_input.write_bytes(b); target = out/f'guard-table-{label}-output.bin'
        e = run(rust, out/'0000-input.bin', target, False, target_input)
        assert 0 < e['exit'] < 256 and not target.exists(), (label, e)
        guards.append(dict(label=f'table-{label}', sampling_sha256=digest(target_input), executions=[e], no_output=True))
    report = dict(passed=True, profile=a.profile, sequence=a.sequence, cases=len(rows), results=rows, guards=guards, guard_count=len(guards),
                  queries=sum(r['queries'] for r in rows), hits=sum(r['hits'] for r in rows), extra_data_queries=sum(r['extra_data_queries'] for r in rows),
                  observable_bytes=sum(r['bytes'] for r in rows), reference=str(reference.relative_to(ROOT)), reference_sha256=digest(reference),
                  sampling=str(SAMPLING.relative_to(ROOT)), sampling_sha256=digest(SAMPLING), image_sha256=digest(distance.IMAGE),
                  native_sha256=digest(native), rust_sha256=digest(rust), script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k:report[k] for k in ['passed', 'cases', 'queries', 'hits', 'extra_data_queries', 'observable_bytes', 'guard_count']}), flush=True)


if __name__ == '__main__': main()
