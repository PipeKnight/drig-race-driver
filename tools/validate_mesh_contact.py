"""Repeat whole original geometry generation on verified private overlap fixtures.

This is a native reference survey, not a Rust narrow-phase compatibility claim.
"""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
NATIVE = ROOT / 'local/behavior/mesh-contact-probe.exe'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(request, target):
    p = subprocess.run([str(NATIVE), str(IMAGE), str(request), str(target)],
                       cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr)


def friction_limit(a, b, normal_y):
    f = lambda shape, o: struct.unpack_from('<f', shape, o)[0]
    single = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
    maximum = struct.unpack('<f', bytes.fromhex('ffff7f7f'))[0]
    limit = maximum
    if a[0x228] or b[0x228] or (a[0x17] and b[0x18]) or (b[0x17] and a[0x18]):
        limit = min(f(a, 0x224), f(b, 0x224))
    for owner, other in [(a, b), (b, a)]:
        if owner[0x229] and other[0x230] and f(other, 0x22c) != maximum:
            cap = single(single(1-abs(normal_y))*f(other, 0x22c))
            cap = max(0., cap)
            if cap <= limit:
                limit = cap
    return limit


def check(answer, baseline, seed, contact_capacity, contact_limit):
    assert len(answer) == 0x21a8 and len(baseline) == 0xf80
    u = lambda b, o: struct.unpack_from('<I', b, o)[0]
    f = lambda b, o: struct.unpack_from('<f', b, o)[0]
    count = u(answer, 0xf90)
    assert count <= contact_capacity <= 16
    assert u(answer, 0xf50) == (0xe000 if count else 0)
    assert u(answer, 0xf54) == count
    assert u(answer, 0xf80) == 0xf000
    assert u(answer, 0xfc0) == 0xe000 + count*0x80
    assert u(answer, 0xf84) == u(answer, 0xf94) == contact_capacity
    # Only these fields of the proved registration boundary may change.
    normalized = bytearray(answer[:0xf80])
    for shape in [0, 0x250]:
        for o, length in [(0x14, 2), (0x1a, 1), (0xc8, 4), (0x224, 4)]:
            normalized[shape+o:shape+o+length] = baseline[shape+o:shape+o+length]
        assert answer[shape+0x14:shape+0x16] == b'\0\1'
        assert answer[shape+0x1a] == 255 and u(answer, shape+0xc8) == 0x1420
    normalized[0x4a0+0x5b0:0x4a0+0x5c0] = baseline[0x4a0+0x5b0:0x4a0+0x5c0]
    assert struct.unpack_from('<4I', answer, 0x4a0+0x5b0) == (0x7100, 0x7101, 0x7102, 0)
    normalized[0xf40:0xf58] = baseline[0xf40:0xf58]
    assert normalized == baseline
    if not u(baseline, 0x4a0+0x910):
        assert count == 0
    template = bytearray(128)
    struct.pack_into('<I', template, 0, 0x7200)
    template[0xf] = 1
    struct.pack_into('<3I', template, 0x1c, 0xffffffff, 0xffffffff, 1)
    assert answer[0xfc8+count*128:0x17c8] == template*(16-count)
    body_start = len(seed)-0x1120
    body = bytearray(seed[body_start:body_start+0x430])
    struct.pack_into('<I', body, 0x334, 0x1c0)
    assert answer[0x17c8:0x1bf8] == body
    assert answer[0x1bf8:0x1c38] == bytes(0x40)
    contacts = []
    for i in range(count):
        c = answer[0xfc8+i*128:0x1048+i*128]
        assert [u(c, o) for o in [4, 8, 0x28, 0x2c, 0x30, 0x34]] == [0xa000, 0, 0x1000, 0xc000, 0x1420, 0x1420]
        assert u(c, 0) == 0x7200 and c[0xc:0x10] == bytes([0, 0, 0, 1])
        assert c[0x10:0x1c] == bytes(12)
        assert struct.unpack_from('<3I', c, 0x1c) == (0xffffffff, 0xffffffff, 1)
        assert f(c, 0x3c) == friction_limit(answer[:0x250], answer[0x250:0x4a0], f(c, 0x64))
        assert c[0x40:0x50] == bytes(16)
        assert c[0x70] == c[0x72] == 0 and c[0x71] in [0, 1] and c[0x73] == 1
        assert c[0x74:] == bytes(12)
        point = struct.unpack_from('<4f', c, 0x50)
        normal = struct.unpack_from('<4f', c, 0x60)
        depth = f(c, 0x38)
        assert all(math.isfinite(v) for v in (*point, *normal, depth))
        assert point[3] == 1 and normal[3] == 0
        assert math.isclose(sum(v*v for v in normal[:3]), 1, rel_tol=3e-4, abs_tol=3e-4)
        contacts.append(dict(point=point, normal=normal, penetration=depth))
    return dict(contact_count=count, contacts=contacts)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--upstream', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    upstream = args.upstream.resolve()
    previous = json.loads(upstream.read_text())
    assert previous['passed'] and previous['cases'] == 720
    attachment = ROOT/previous['upstream']
    assert digest(attachment) == previous['upstream_sha256']
    source_report = json.loads(attachment.read_text())
    assert source_report['passed'] and source_report['physical_attachment']
    assert digest(IMAGE) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    results = []
    for row in previous['results']:
        index = row['index']
        old_request = upstream.parent/f'{index:04}-native-input.bin'
        old_answer = upstream.parent/f'{index:04}-original.bin'
        assert digest(old_request) == row['native_input_sha256']
        assert digest(old_answer) == row['output_sha256']
        request = old_request.read_bytes()
        assert request[:4] == b'MP01'
        source_row = source_report['results'][row['upstream_index']]
        seed_path = attachment.parent/f"{source_row['index']:03}-original.bin"
        source_path = attachment.parent/f"{source_row['index']:03}-input.bin"
        assert digest(seed_path) == source_row['output_sha256']
        assert digest(source_path) == source_row['input_sha256']
        seed = seed_path.read_bytes()
        assert request[32:] == seed
        if source_row['kind'] == 'source':
            assert digest(GAME/source_row['source']) == source_row['source_sha256']
            assert digest(GAME/source_row['csv']) == source_row['csv_sha256']
        inp = out/f'{index:04}-input.bin'
        contact_capacity = [16, 1, 2, 4, 0][index % 5]
        contact_limit = [struct.unpack('<f', bytes.fromhex('ffff7f7f'))[0], 0., 2.5][index % 3]
        inp.write_bytes(b'MC02'+request[4:32]+struct.pack('<fI', contact_limit, contact_capacity)+request[32:])
        executions, answers = [], []
        for kind in ['original', 'repeat']:
            target = out/f'{index:04}-{kind}.bin'
            e = run(inp, target)
            executions.append(e)
            (out/'current-executions.json').write_text(json.dumps(dict(index=index, runs=executions), indent=2))
            assert e['exit'] == 0 and not e['stderr'], (index, e)
            assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
            answers.append(target.read_bytes())
        assert answers[0] == answers[1], index
        try:
            observation = check(answers[0], old_answer.read_bytes(), seed, contact_capacity, contact_limit)
        except AssertionError:
            (out/'failed-check.json').write_text(json.dumps(dict(index=index), indent=2))
            raise
        results.append(dict(index=index, upstream_index=row['upstream_index'], label=row['label'],
                            contact_capacity=contact_capacity, contact_limit=contact_limit,
                            input_sha256=digest(inp), output_sha256=digest(out/f'{index:04}-original.bin'),
                            bytes=len(answers[0]), executions=executions, **observation))
    guards = []
    sample = (out/'0000-input.bin').read_bytes()
    mesh_bytes = struct.unpack_from('<I', sample, 4)[0]
    variants = [('short', sample[:-1]), ('long', sample+b'\0')]
    edits = [('tag', 0, 0), ('shift-nan', 8, 0x7fc00000), ('shift-inf', 12, 0x7f800000),
             ('shift-bound', 16, 0x46200001), ('shift-w', 20, 0x3f800000), ('capacity', 28, 2),
             ('contact-limit-nan', 32, 0x7fc00000), ('contact-limit-negative', 32, 0xbf800000),
             ('contact-capacity', 36, 17), ('callback', 40+0x188, 1), ('linked-shape', 40+0xdc, 1),
             ('body-joint', 40+0x500+mesh_bytes+0x320, 1), ('mesh-kind', 40+0x10, 6),
             ('mesh-reference', 40+0x240, 1), ('mesh-points', 40+0x500, 81),
             ('mesh-adjacency', 40+0x500+0x60+0x10, 0xffffffff)]
    for label, offset, value in edits:
        b = bytearray(sample)
        struct.pack_into('<I', b, offset, value)
        variants.append((label, b))
    for label, data in variants:
        inp = out/f'guard-{label}-input.bin'
        target = out/f'guard-{label}-output.bin'
        inp.write_bytes(data)
        e = run(inp, target)
        assert 0 < e['exit'] < 256 and not e['stderr'] and not target.exists(), (label, e)
        assert 'globals_restored=1' in e['stdout']
        guards.append(dict(label=label, execution=e, no_output=True))
    existing = out/'guard-existing.bin'
    existing.write_bytes(b'preserve existing evidence')
    before = digest(existing)
    e = run(out/'0000-input.bin', existing)
    assert 0 < e['exit'] < 256 and not e['stderr'] and digest(existing) == before
    guards.append(dict(label='existing', execution=e, existing_output_preserved=True))
    report = dict(passed=True, native_reference_only=True, contact_slot_constructor=True, cases=len(results),
                  observable_bytes=sum(r['bytes'] for r in results),
                  positive_cases=sum(r['contact_count'] > 0 for r in results),
                  contacts=sum(r['contact_count'] for r in results), guard_count=len(guards),
                  guards=guards, results=results, upstream=str(upstream.relative_to(ROOT)),
                  upstream_sha256=digest(upstream), image_sha256=digest(IMAGE),
                  native_sha256=digest(NATIVE), script_sha256=digest(Path(__file__)),
                  scope='Whole original startup direction initializer, scratch constructor and all16 B0ECE0 contact-slot constructors, static clone registration and B39500/AE8AD0 mesh7/7 kernel. Owned bodies, zero material scalars, 0/1/2/4/16 contact capacity, explicitly supplied finite contact limit replaces formerly opaque descriptor6C/shape224. Repeated native reference only: no Rust contact generator, world solve, source barrier shape or runtime integration claim.')
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'positive_cases', 'contacts', 'observable_bytes', 'guard_count']}))


if __name__ == '__main__':
    main()
