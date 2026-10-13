"""Complete original layout against Rust on authored bounds-bearing records."""
import argparse
import collections
import json
import random
import struct
import subprocess
from pathlib import Path
from inspect_installation import inspect_pe
from validate_steering_control import digest, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
PROBE = ROOT / 'local/behavior/deformation-layout-probe.exe'
SIZE = 0xa500


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def parameters():
    assert digest(IMAGE) == PE_SHA
    raw, pe = IMAGE.read_bytes(), inspect_pe(IMAGE)
    def word(address):
        for s in pe['sections']:
            at = address - 0x400000 - s['rva']
            if 0 <= at <= s['raw_size'] - 4:
                return struct.unpack_from('<I', raw, s['raw_offset'] + at)[0]
        raise AssertionError(hex(address))
    values = [struct.unpack('<f', struct.pack('<I', word(a)))[0]
              for a in [0xcb0de4, 0xcafe5c, 0xce2bb4, 0xcb0948, 0xcf7718]]
    return values, word(0xe4fc34) & 255


def cases():
    rng = random.Random(0x2008254)
    default, flag = parameters()
    result = []
    def add(name, bounds, p=None, token=None, selected_flag=None):
        descriptor = bytearray(rng.randbytes(SIZE))
        result.append(dict(name=name, bounds=bounds, parameters=p or default,
                           token=rng.getrandbits(32) if token is None else token,
                           flag=flag if selected_flag is None else selected_flag,
                           descriptor=bytes(descriptor)))
    for i in range(240):
        n = [0, 1, 2, 4, 16, 32][i % 6]
        bounds = []
        for _ in range(n):
            center = [rng.uniform(-5, 5) for _ in range(3)]
            extent = [rng.uniform(.01, 4) for _ in range(3)]
            w0 = struct.unpack('<f', struct.pack('<I', rng.choice([0, 0x80000000, 0x7fc01234, 0x7f800000, 0xff800000])))[0]
            w1 = rng.uniform(-100, 100)
            bounds.append([*[f32(c - e) for c, e in zip(center, extent)], w0,
                           *[f32(c + e) for c, e in zip(center, extent)], w1])
        p = [f32([0, .01, .5][i % 3]), f32([.005, .01, .025, .1, 1][i % 5]),
             f32([1.001, 1.02, 1.5, 2][i % 4]),
             f32([-.5, -1, -2][i % 3]), f32([-1, -3, -4][i % 3])]
        add(f'authored-{i}', bounds, p, selected_flag=[0, 1, 7, 255][i % 4])
    for i, axis in enumerate(range(3)):
        for zero in [0., -0.]:
            for reverse in [False, True]:
                a, b = [-0., -0., -1., 999., 0., 0., 1., -999.], [0., 0., -1., float('nan'), -0., -0., 1., float('inf')]
                a[axis] = zero
                p = list(default); p[0] = 0.
                add(f'signed-order-{i}-{zero}-{reverse}', [b, a] if reverse else [a, b], p)
    for token in [0, 1, 0xdeadbeef, 0xffffffff]:
        for selected_flag in [0, 255]:
            add(f'empty-token-{token}-{selected_flag}', [], token=token, selected_flag=selected_flag)
    add('inverted-x-fallback', [[2., 20., 30., -0., -2., -20., -30., 0.]])
    add('ignored-inverted-record-aggregate-valid', [[2., 1., 1., 0., -2., -1., -1., 0.], [-3., -3., -3., 0., 3., 3., 3., 0.]])
    for length in [.125, 1., 10., 100., 1000.]:
        for axis in range(3):
            lo, hi = [0.] * 4, [0.] * 4
            hi[axis] = length
            p = list(default); p[0] = 0.
            add(f'thin-axis-{axis}-{length}', [lo + hi], p)
    for word in [0x7fa00001, 0xffa00001, 0x7f800000, 0xff800000]:
        add(f'opaque-w-{word:08x}', [[-1., -1., -1., 0., 1., 1., 1., 0.]])
        result[-1]['bound_overrides'] = [(12, word), (28, word ^ 0x80000000)]
    return result


def request(rows):
    output = bytearray(b'GL01' + struct.pack('<I', len(rows)))
    for row in rows:
        output += struct.pack('<III5f', len(row['bounds']), row['token'], row['flag'], *row['parameters'])
        output += row['descriptor']
        for bounds in row['bounds']:
            output += struct.pack('<8f', *bounds)
        for at, value in row.get('bound_overrides', []):
            struct.pack_into('<I', output, len(output) - len(row['bounds']) * 32 + at, value)
    return bytes(output)


def execute(out, prefix, payload, profile, allow_failure=False):
    source = out / (prefix + '-input.bin')
    source.write_bytes(payload)
    outputs, runs = {}, {}
    for kind in ['original', 'repeat', 'rust']:
        target = out / f'{prefix}-{kind}.bin'
        exe = ROOT / f'target/{profile}/grid-deformation-layout.exe' if kind == 'rust' else PROBE
        arguments = [source, target] if kind == 'rust' else [IMAGE, source, target]
        run = subprocess.run([str(exe), *map(str, arguments)], cwd=ROOT, capture_output=True, text=True, timeout=120)
        runs[kind] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr, executable_sha256=digest(exe))
        if target.exists():
            outputs[kind] = target.read_bytes()
        if not allow_failure:
            assert run.returncode == 0 and not run.stderr, runs[kind]
            if kind != 'rust':
                assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run.stdout
    (out / (prefix + '-execution.json')).write_text(json.dumps(runs, indent=2))
    return outputs, runs


def independent_checks(before, answer):
    dims = struct.unpack_from('<3I', answer, 0xa414)
    count = dims[0] * dims[1] * dims[2]
    assert all(2 <= d <= 400 for d in dims) and count <= 400
    assert struct.unpack_from('<I', answer, 0xa4a0)[0] == 0xffffffff
    assert struct.unpack_from('<I', answer, 0xa4f8)[0] == before['token']
    assert answer[0xa4ad] == 1 and answer[0xa4b0] == before['flag']
    assert answer[0xa440:0xa450] == bytes(16)
    spacing = struct.unpack_from('<f', answer, 0xa420)[0]
    minimum = struct.unpack_from('<4f', answer, 0xa430)
    allowed = set(range(0xa414, 0xa420)) | set(range(0xa420, 0xa424)) | set(range(0xa430, 0xa450))
    allowed |= set(range(0xa4a0, 0xa4a4)) | set(range(0xa4f8, 0xa4fc)) | {0xa4ad, 0xa4b0}
    for node in range(count):
        x = node % dims[0]
        y = (node // dims[0]) % dims[1]
        z = node // (dims[0] * dims[1])
        position = [f32(f32(float(v) * spacing) + minimum[i]) for i, v in enumerate([x, y, z, 0])]
        at = 0x2580 + node * 80
        assert answer[at:at + 16] == answer[at + 16:at + 32] == struct.pack('<4f', *position)
        assert answer[at + 48:at + 64] == bytes(16) and answer[0xa280 + node] == 255
        allowed.update(range(at, at + 32)); allowed.update(range(at + 48, at + 64)); allowed.add(0xa280 + node)
    assert all(a == b or i in allowed for i, (a, b) in enumerate(zip(before['descriptor'], answer)))
    return dims, count


def guards(out, profile, first):
    payload = bytearray(request([first]))
    mutations = [('flag', 8 + 8, 256), ('negative_margin', 8 + 12, 0xbf800000),
                 ('nan_margin', 8 + 12, 0x7fc00000), ('zero_spacing_factor', 8 + 16, 0),
                 ('growth_one', 8 + 20, 0x3f800000), ('positive_fallback', 8 + 24, 0x3f800000)]
    tests = []
    for name, at, word in mutations:
        r = bytearray(payload); struct.pack_into('<I', r, at, word); tests.append((name, r, 9))
    for name, data, error in [('tag', b'NOPE' + payload[4:], 7), ('empty', b'GL01' + bytes(4), 7),
                              ('count129', b'GL01' + struct.pack('<I', 129), 7),
                              ('truncated', payload[:-1], 8), ('trailing', payload + b'x', 8)]:
        tests.append((name, data, error))
    high = bytearray(payload); struct.pack_into('<I', high, 8, 33)
    tests.append(('bounds_count33', high, 8))
    bounds_case = dict(first, bounds=[[-1., -1., -1., 0., 1., 1., 1., 0.]])
    for name, at, word in [('nan_bound', 8 + 32 + SIZE, 0x7fc00000), ('infinite_bound', 8 + 32 + SIZE + 24, 0x7f800000),
                           ('bound_too_large', 8 + 32 + SIZE, struct.unpack('<I', struct.pack('<f', 1001.))[0])]:
        r = bytearray(request([bounds_case])); struct.pack_into('<I', r, at, word); tests.append((name, r, 9))
    degenerate = dict(bounds_case, bounds=[[0.] * 8], parameters=[0., *first['parameters'][1:]])
    tests.append(('degenerate', request([degenerate]), 9))
    inverted = dict(bounds_case, bounds=[[0., 2., 0., 0., 1., -2., 1., 0.]])
    tests.append(('inverted_y', request([inverted]), 9))
    checks = []
    for name, raw, expected in tests:
        outputs, runs = execute(out, 'guard-' + name, raw, profile, True)
        passed = not outputs and all(r['exit'] == (1 if k == 'rust' else expected) for k, r in runs.items())
        checks.append(dict(name=name, passed=passed, executions=runs))
    # Existing outputs are immutable even when original execution succeeds.
    for kind in ['original', 'rust']:
        source, target = out / f'existing-{kind}-input.bin', out / f'existing-{kind}.bin'
        source.write_bytes(payload); target.write_bytes(b'owned sentinel')
        exe = PROBE if kind == 'original' else ROOT / f'target/{profile}/grid-deformation-layout.exe'
        args = [IMAGE, source, target] if kind == 'original' else [source, target]
        run = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True, text=True, timeout=120)
        checks.append(dict(name='existing-' + kind, passed=run.returncode == (24 if kind == 'original' else 1)
                           and target.read_bytes() == b'owned sentinel', exit=run.returncode,
                           stdout=run.stdout, stderr=run.stderr))
    (out / 'guards.json').write_text(json.dumps(checks, indent=2))
    assert all(c['passed'] for c in checks), [c for c in checks if not c['passed']]
    return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    rows, reports, coverage = cases(), [], collections.Counter()
    for start in range(0, len(rows), 64):
        batch = rows[start:start + 64]
        outputs, runs = execute(out, f'batch-{start}', request(batch), args.profile)
        original, repeat, rust = [outputs[k] for k in ['original', 'repeat', 'rust']]
        differences = [dict(case=i // SIZE, offset=hex(i % SIZE)) for i in range(0, len(original) - 8, 4)
                       if original[8 + i:12 + i] != rust[8 + i:12 + i]]
        item = dict(cases=len(batch), bytes=len(original) - 8, repeat_exact=original == repeat,
                    differences=differences, executions=runs)
        (out / f'batch-{start}-validation.json').write_text(json.dumps(item, indent=2))
        assert len(original) == 8 + len(batch) * SIZE and original == repeat and not differences, item
        for i, row in enumerate(batch):
            dims, count = independent_checks(row, original[8 + i * SIZE:8 + (i + 1) * SIZE])
            coverage['nodes'] += count; coverage['empty'] += not row['bounds']
            coverage['wide_dimension'] += max(dims) > 16
        reports.append(item)
    checks = guards(out, args.profile, rows[0])
    files = {p.name: dict(size=p.stat().st_size, sha256=digest(p)) for p in sorted(out.iterdir()) if p.is_file()}
    executables = {str(p.relative_to(ROOT)): digest(p) for p in [IMAGE, PROBE, ROOT / f'target/{args.profile}/grid-deformation-layout.exe']}
    result = dict(profile=args.profile, cases=len(rows), bytes=sum(r['bytes'] for r in reports),
                  coverage=dict(coverage), guards=checks, names=[r['name'] for r in rows],
                  reports=reports, files=files, executables=executables,
                  scope='Whole original9A2A70/989490/98A7D0 versus independent Rust; authored owned bounds, '
                  'five supplied scalar globals and flag. All descriptor bytes compared; owned canaries and '
                  'complete restored image checked. No source-object construction, impact-ready node markers, '
                  'edge creation, live globals or prototype startup integration.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ['profile', 'cases', 'bytes', 'coverage']}))


if __name__ == '__main__':
    main()
