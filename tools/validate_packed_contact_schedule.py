"""Original active-body serial pair scheduling, including order/static gates.

The supplied pair already exists. No broad-phase update or full-world claim.
"""
import argparse
import json
import struct
import subprocess
from pathlib import Path

import validate_packed_mesh_contact as geometry

ROOT, IMAGE, digest = geometry.ROOT, geometry.IMAGE, geometry.digest
SCHEDULED = ROOT/'local/behavior/packed-contact-schedule-probe.exe'
REGRESSION = ROOT/'local/behavior/packed-mesh-contact-regression-probe.exe'


def run(executable, request, target):
    e = subprocess.run([str(executable), str(IMAGE), str(request), str(target)],
                       cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=e.returncode, stdout=e.stdout, stderr=e.stderr)


def scheduled_request(raw, flags):
    tag = b'PC02' if raw[:4] == b'PC01' else raw[:4]
    return tag+raw[4:40]+struct.pack('<I', flags)+raw[40:]


def normalize(answer, direct, flags):
    """Check queue state, then remove only explicitly authored scope differences."""
    u = lambda b, o: struct.unpack_from('<I', b, o)[0]
    put = lambda b, o, v: struct.pack_into('<I', b, o, v)
    extent = len(direct)
    assert len(answer) == extent+0x1cc
    tail = answer[extent:]
    expected_tail = bytearray(0x1cc)
    put(expected_tail, 0xc20-0xa80, 0xd000 if flags&1 else 0)
    struct.pack_into('<3I', expected_tail, 0x1c0, 0x80000, 0, 0xd000 if flags&1 else 0)
    assert tail == expected_tail
    assert u(answer, 0xfa0+8) == 0  # B184F0 clears heads after primary dispatch.
    assert u(answer, 0x4d0+0xa78) == int(bool(flags&1))
    assert u(answer, 0x4d0+0x928) == 0x70000
    assert u(answer, 0x4d0+0x92c) == 1
    assert u(answer, 0x4d0+0x930) == (0xd000 if flags&1 else 0)
    assert u(answer, 0x17e8+0x328) == 0x1000
    assert u(answer, 0x17e8+0x400) == 0
    assert answer[0x17e8+0x416] == flags&1
    reverse = bool(flags&2)
    assert u(answer, 0xd4 if reverse else 0xd0) == 0xd000
    assert u(answer, 0x250+(0xd0 if reverse else 0xd4)) == 0xd000
    assert struct.unpack_from('<2I', answer, 0xf50+0x28) == ((0xc000, 0x1000) if reverse else (0x1000, 0xc000))
    b = bytearray(answer[:extent])
    for shape in [0, 0x250]:
        put(b, shape+0xd0, 0)
        put(b, shape+0xd4, 0)
    b[0x4d0+0x8b0:0x4d0+0x8c0] = bytes(16)
    for offset in [0x928, 0x92c, 0x930, 0xa78]:
        put(b, 0x4d0+offset, 0)
    put(b, 0xf50+0x28, 0x1000)
    put(b, 0xf50+0x2c, 0xc000)
    put(b, 0x17e8+0x400, u(direct, 0x17e8+0x400))
    b[0x17e8+0x416] = direct[0x17e8+0x416]
    if not flags&1:
        # Direct zero-capacity control has the same constructed arena and caches.
        put(b, 0xfa0+4, 0)
        put(b, 0xfa0+0x14, 0)
    assert b == direct
    return bytes(b)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--upstream', type=Path, required=True)
    p.add_argument('--regression-only', action='store_true')
    p.add_argument('--limit', type=int, default=0)
    args = p.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    upstream = args.upstream.resolve()
    r = json.loads(upstream.read_text())
    assert r['passed'] and r['native_reference_only'] and r['attachment_index'] == 48
    assert digest(geometry.NATIVE) == r['native_sha256'] and digest(IMAGE) == r['image_sha256']
    rows = []
    if args.regression_only:
        for old in r['results'][:args.limit or None]:
            i = old['index']
            inp = upstream.parent/f'{i:04}-input.bin'
            assert digest(inp) == old['input_sha256']
            target = out/f'{i:04}-regression.bin'
            execution = run(REGRESSION, inp, target)
            assert execution['exit'] == 0 and not execution['stderr'], execution
            assert digest(target) == old['output_sha256']
            rows.append(dict(index=i, execution=execution, output_sha256=digest(target), bytes=target.stat().st_size))
        report = dict(passed=True, regression_only=True, cases=len(rows), results=rows,
                      observable_bytes=sum(v['bytes'] for v in rows),
                      native_sha256=digest(REGRESSION), upstream_sha256=digest(upstream),
                      upstream=str(upstream.relative_to(ROOT)), script_sha256=digest(Path(__file__)))
    else:
        selected = [(v, range(4)) for v in r['results'] if 'base_index' not in v and v['height'] == 0]
        selected += [(v, [1, 3]) for v in r['results'] if 'base_index' in v]
        if args.limit:
            selected = selected[:args.limit]
        for old, flag_values in selected:
            source_index = old['index']
            inp = upstream.parent/f'{source_index:04}-input.bin'
            assert digest(inp) == old['input_sha256']
            raw = inp.read_bytes()
            seed_size, chunk_size, limit, cache_limit, index_limit = struct.unpack_from('<5I', raw, 4)
            seed, chunk = raw[40:40+seed_size], raw[40+seed_size:]
            assert len(chunk) == chunk_size
            direct_path = upstream.parent/f'{source_index:04}-original.bin'
            assert digest(direct_path) == old['output_sha256']
            direct = direct_path.read_bytes()
            if 0 in flag_values:
                zero_request = bytearray(raw)
                struct.pack_into('<I', zero_request, 12, 0)
                zero_input, zero_target = out/f'{source_index:04}-zero-input.bin', out/f'{source_index:04}-zero.bin'
                zero_input.write_bytes(zero_request)
                e = run(geometry.NATIVE, zero_input, zero_target)
                assert e['exit'] == 0 and not e['stderr'], e
                zero = zero_target.read_bytes()
                geometry.check(zero, seed, chunk, struct.unpack_from('<4f', raw, 24), 0, cache_limit, index_limit)
            for flags in flag_values:
                i = len(rows)
                request = out/f'{i:04}-input.bin'
                request.write_bytes(scheduled_request(raw, flags))
                executions = []
                reference = direct if flags&1 else zero
                for kind in ['original', 'repeat']:
                    target = out/f'{i:04}-{kind}.bin'
                    e = run(SCHEDULED, request, target)
                    executions.append(e)
                    if e['exit']:
                        (out/'failure.json').write_text(json.dumps(dict(index=i, source_index=source_index, flags=flags, executions=executions), indent=2))
                        raise RuntimeError(e)
                    assert not e['stderr'], e
                    normalized = normalize(target.read_bytes(), reference, flags)
                    geometry.check(normalized, seed, chunk, struct.unpack_from('<4f', raw, 24),
                                   limit if flags&1 else 0, cache_limit, index_limit)
                assert (out/f'{i:04}-original.bin').read_bytes() == (out/f'{i:04}-repeat.bin').read_bytes()
                answer = (out/f'{i:04}-original.bin').read_bytes()
                rows.append(dict(index=i, source_index=source_index, flags=flags,
                                 input_sha256=digest(request), output_sha256=digest(out/f'{i:04}-original.bin'),
                                 reference_sha256=digest(direct_path) if flags&1 else digest(zero_target),
                                 bytes=len(answer), contacts=struct.unpack_from('<I', answer, 0xfb0)[0], executions=executions))
            if len(rows) % 40 == 0:
                print(f'scheduled_cases={len(rows)}', flush=True)
        guards = []
        for guard in r['guards']:
            if guard['label'] == 'existing-valid-output':
                continue
            label = guard['label']
            raw = (upstream.parent/f'guard-{label}-input.bin').read_bytes()
            request, target = out/f'guard-{label}-input.bin', out/f'guard-{label}-output.bin'
            request.write_bytes(scheduled_request(raw, 1))
            e = run(SCHEDULED, request, target)
            assert 0 < e['exit'] < 256 and not e['stderr'] and not target.exists(), (label, e)
            assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
            guards.append(dict(label=label, execution=e, no_output=True))
        request, target = out/'guard-flags-input.bin', out/'guard-flags-output.bin'
        request.write_bytes(scheduled_request((upstream.parent/'0000-input.bin').read_bytes(), 4))
        e = run(SCHEDULED, request, target)
        assert e['exit'] == 7 and not target.exists()
        guards.append(dict(label='flags', execution=e, no_output=True))
        target = out/'guard-existing-output.bin'
        target.write_bytes(b'preserve existing evidence')
        before = digest(target)
        e = run(SCHEDULED, out/'0000-input.bin', target)
        assert e['exit'] == 18 and digest(target) == before
        guards.append(dict(label='existing-valid-output', execution=e, unchanged_sha256=before))
        report = dict(passed=True, regression_only=False, cases=len(rows), results=rows,
                      contacts=sum(v['contacts'] for v in rows),
                      positive_cases=sum(v['contacts'] > 0 for v in rows),
                      guard_count=len(guards), guards=guards,
                      observable_bytes=sum(v['bytes'] for v in rows),
                      native_sha256=digest(SCHEDULED), upstream_sha256=digest(upstream),
                      upstream=str(upstream.relative_to(ROOT)), script_sha256=digest(Path(__file__)),
                      scope='Whole original serial B152E0/B18300/B184F0/B187D0 on one active body/prepared pair; all prior packed generation runs unchanged. Explicit active400=0/static416 gate, both ownership orders. No moving broadphase/world advance/sleeping or inactive-group finish/async/full world/Rust generator/runtime collision proof.')
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'observable_bytes']}))


if __name__ == '__main__':
    main()
