"""Compare whole preparation and source-backed wheel contacts, no helper stubs."""
import argparse
import hashlib
import json
import math
import random
import struct
import subprocess
from pathlib import Path

from validate_vcqtc import chunks

ROOT = Path(__file__).resolve().parents[1]
PYTHON = Path(__import__('sys').executable)
TRACK = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/tracks/europe/donington/route_0/track.jpk')
SURFACE = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
NATIVE = ROOT / 'local/behavior/suspension-prepare-probe.exe'
WORLD = ROOT / 'local/behavior/source-world-20261007-01/world.private.bin'
MECHANICS = ROOT / 'local/behavior/source-contact-20261007-01/mechanics.private.bin'
RECORD = 0x2608


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def put(body, offset, values):
    if isinstance(values, (int, float)):
        values = [values]
    struct.pack_into('<' + 'f' * len(values), body, offset, *values)


def ui(body, offset, value):
    struct.pack_into('<I', body, offset, value)


def build_cases():
    rng = random.Random(2008167)
    positions = []
    for _, vertices, triangles, _, _ in chunks(TRACK.read_bytes()):
        for indices, _, _, _ in triangles:
            p = [vertices[i] for i in indices]
            a = [p[1][i]-p[0][i] for i in range(3)]
            b = [p[2][i]-p[0][i] for i in range(3)]
            n = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
            norm = math.sqrt(sum(v*v for v in n))
            if norm and n[1]/norm > .75:
                positions.append([sum(v[i] for v in p)/3 for i in range(3)])
    cases = []
    for i in range(120):
        center = positions[i*len(positions)//120]
        body = bytearray(0x2600)
        # Retained opaque padding differs per case; no arbitrary pointers are followed.
        for j in range(0, len(body), 4):
            ui(body, j, rng.getrandbits(32))
        delta = [.000001, .001, .006130263675, .01, .016666667, .1][i % 6]
        yaw = rng.uniform(-math.pi, math.pi)
        roll = 0 if i % 3 == 0 else rng.uniform(-.12, .12)
        def orientation(y, r):
            return [[math.cos(y)*math.cos(r), math.sin(r), -math.sin(y)*math.cos(r), 0],
                    [-math.cos(y)*math.sin(r), math.cos(r), math.sin(y)*math.sin(r), 0],
                    [math.sin(y), 0, math.cos(y), 0]]
        basis = orientation(yaw, roll) + [[*center, 1]]
        basis[3][1] += [.3, .4, .6, 1., 4.][i % 5]
        for j, row in enumerate(basis):
            put(body, 0x15e0 + j*16, row)
            put(body, 0xb40 + j*16, row)
        if i % 3:
            for j, row in enumerate(orientation(yaw+.005, roll-.003)):
                put(body, 0xb40 + j*16, row)
        # Carried-frame branch includes actual movement and velocity differences.
        old_position = basis[3].copy()
        old_position[0] -= rng.uniform(-.04, .04)
        old_position[1] -= rng.uniform(-.01, .01)
        put(body, 0xb70, old_position)
        linear = [rng.uniform(-12, 12), rng.uniform(-.5, .5), rng.uniform(-12, 12), 0]
        angular = [rng.uniform(-.1, .1), rng.uniform(-.3, .3), rng.uniform(-.1, .1), 0]
        if i % 10 == 0:
            angular = [-0., 0., -0., 0.]
        put(body, 0x15c0, linear)
        put(body, 0x15d0, angular)
        put(body, 0xb20, [v + rng.uniform(-.01, .01) if j < 3 else 0 for j, v in enumerate(linear)])
        put(body, 0xb30, angular)
        put(body, 0xb80, linear)
        put(body, 0xb90, [v*.99 for v in angular])
        put(body, 0x15a0, rng.uniform(400, 1600))
        put(body, 0x15a8, [700, 1000, 600])
        put(body, 0x3d0 + 0x6f0, [700, 1000, 600])
        put(body, 0x1648, rng.uniform(0, 30))
        put(body, 0x159c, rng.uniform(0, 120))
        put(body, 0x12ac, rng.uniform(-1, 2))
        put(body, 0x3d0 + 0x620, [0]*8)
        for off, val in [(0x5d0, i % 2), (0x5d4, 0), (0x5d8, 4), (0x604, 0), (0x608, 0), (0x60c, 0), (0x70c, 0)]:
            ui(body, 0x3d0 + off, val)
        for w in range(4):
            c = 0x3d0 + w*0x170
            wheel = 0x1990 + w*0x150
            put(body, wheel, [(-.8 if w % 2 else .8), 0, (-1.3 if w < 2 else 1.3), rng.uniform(-3, 3)])
            axle_tilt = rng.uniform(-.08, .08) if i % 3 else 0
            put(body, c + 0x50, [axle_tilt, math.sqrt(1-axle_tilt*axle_tilt), 0, 0])
            put(body, wheel + 0x3c, rng.uniform(-.1, .1))
            put(body, wheel + 0x40, rng.uniform(-.08, .25))
            put(body, wheel + 0x48, -.12)
            put(body, wheel + 0x4c, .2)
            put(body, wheel + 0x5c, rng.uniform(10000, 30000))
            put(body, wheel + 0xe0, .3)
            put(body, wheel + 0x110, rng.uniform(-30, 30))
            put(body, wheel + 0x140, rng.uniform(-.001, .001))
            put(body, 0x1650 + w*16, [rng.uniform(-.01, .01), 0, rng.uniform(-.01, .01), rng.uniform(-4, 4)])
            put(body, c + 0x6c, -1000 if i % 3 == 0 else rng.uniform(-.1, .1))
            put(body, c + 0x7c, rng.uniform(-.001, .001))
            put(body, c + 0x80, rng.uniform(-.001, .001))
            ui(body, c + 0x178, 0)
            ui(body, c + 0x17c, 0)
            for sample in range(5):
                put(body, 0x30c + w*40 + sample*8, [sample*.8, .3 + rng.uniform(-.002, .002)])
        cases.append((delta, body))
    return cases


def execute(exe, args, out, kind):
    result = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True, text=True, timeout=120)
    record = dict(kind=kind, path=str(exe), sha256=digest(exe), exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
    (out / (kind + '-execution.json')).write_text(json.dumps(record, indent=2))
    if result.returncode:
        raise RuntimeError(f'{kind} failed: {record}')
    return record


def guards(out, rust, first):
    """Ordinary rejection before an output is created or original is entered."""
    tests = [('index', 0, 7), ('zero_delta', 4, 0), ('nan_delta', 4, 0x7fc00000), ('tiny_delta', 4, 1),
             ('large_delta', 4, 0x3e800000), ('count', 8+0x3d0+0x5d8, 5),
             ('extra_axes', 8+0x3d0+0x70c, 1), ('preselected', 8+0x3d0+0x604, 1),
             ('body_pointer', 8+0x3d0+0x5d4, 1), ('table_pointer', 8+0x3d0+0x608, 1),
             ('table_index', 8+0x3d0+0x60c, 1), ('wheel_pointer', 8+0x3d0+0x178, 1),
             ('object_pointer', 8+0x3d0+0x17c, 1), ('zero_mass', 8+0x15a0, 0),
             ('zero_inertia', 8+0x3d0+0x6f0, 0), ('nan_basis', 8+0x15e0, 0x7fc00000),
             ('nan_axis', 8+0x3d0+0x50, 0x7fc00000), ('zero_radius', 8+0x1990+0xe0, 0)]
    results = []
    for name, offset, value in tests:
        record = bytearray(first)
        struct.pack_into('<I', record, offset, value)
        request = out / f'guard-{name}.bin'
        request.write_bytes(b'SP01'+struct.pack('<I', 1)+record)
        for kind, exe, arguments in [
            ('original', NATIVE, [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, out/f'guard-{name}-original.bin', MECHANICS]),
            ('rust', rust, [TRACK, SURFACE, request, out/f'guard-{name}-rust.bin']),
        ]:
            run = subprocess.run([str(exe), *map(str, arguments)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            passed = (run.returncode in range(63, 70) and 'records=0' in run.stdout and 'code_unchanged=1' in run.stdout) if kind == 'original' else run.returncode == 1
            passed = passed and not Path(arguments[-2] if kind == 'original' else arguments[-1]).exists()
            results.append(dict(name=name, kind=kind, exit=run.returncode, stdout=run.stdout, stderr=run.stderr, passed=passed))
    assert all(r['passed'] for r in results), results
    return results


def semantic(request, output):
    result = dict(pose_restored=True, previous_velocity_restored=True, clock_zero=True,
                  accumulators_consumed=True, body_rates_exact=True, contact_identity_consistent=True)
    f32 = lambda v: struct.unpack('<f', struct.pack('<f', v))[0]
    for i in range((len(request)-8)//RECORD):
        inp = request[16+i*RECORD:16+i*RECORD+0x2600]
        body = output[16+i*RECORD:16+i*RECORD+0x2600]
        get = lambda offset: struct.unpack_from('<f', body, offset)[0]
        prior = lambda offset: struct.unpack_from('<f', inp, offset)[0]
        delta = struct.unpack_from('<f', request, 12+i*RECORD)[0]
        result['pose_restored'] &= inp[0x15e0:0x1620] == body[0x15e0:0x1620]
        result['previous_velocity_restored'] &= body[0x15c0:0x15e0] == inp[0xb20:0xb40]
        result['clock_zero'] &= get(0x3d0+0x610) == 0
        inverse = f32(1/delta)
        for offset, previous, rates in [(0x15c0, 0xb20, 0x3d0+0x620), (0x15d0, 0xb30, 0x3d0+0x630)]:
            for j in range(4):
                expected = f32(inverse*f32(prior(offset+j*4)-prior(previous+j*4)))
                result['body_rates_exact'] &= struct.pack('<f', expected) == body[rates+j*4:rates+j*4+4]
        for w in range(4):
            c, wheel = 0x3d0+w*0x170, 0x1990+w*0x150
            result['accumulators_consumed'] &= all(get(c+off) == 0 for off in [0x168, 0x16c, 0x170, 0x174])
            tokens = struct.unpack_from('<3I', body, wheel+0xc0)
            result['contact_identity_consistent'] &= bool(tokens[0]) == bool(tokens[1]) == bool(tokens[2])
    assert all(result.values()), result
    return result


def carry_runs(out, rust, cases):
    seed = [(d, bytearray(b)) for d, b in cases[8:24]]
    branches = {kind:[(d, bytearray(b)) for d, b in seed] for kind in ['original', 'repeat', 'rust']}
    executions, all_bytes, differences = [], 0, []
    for step in range(8):
        outputs = {}
        for kind in branches:
            current = branches[kind]
            request = out / f'carry-{step}-{kind}-in.bin'
            data = b'SP01'+struct.pack('<I', len(current))+b''.join(struct.pack('<If', i, d)+b for i, (d, b) in enumerate(current))
            request.write_bytes(data)
            target = out / f'carry-{step}-{kind}-out.bin'
            exe = rust if kind == 'rust' else NATIVE
            arguments = [TRACK, SURFACE, request, target] if kind == 'rust' else [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, target, MECHANICS]
            executions.append(execute(exe, arguments, out, f'carry-{step}-{kind}'))
            outputs[kind] = target.read_bytes()
            semantic(data, outputs[kind])
            next_cases = []
            for i, (delta, old_body) in enumerate(current):
                body = bytearray(outputs[kind][16+i*RECORD:16+i*RECORD+0x2600])
                # Authored caller transfer between preparations: carry each branch's
                # own outputs and advance its position; no captured intermediates.
                body[0xb40:0xb80] = body[0x15e0:0x1620]
                body[0xb80:0xb90] = old_body[0xb20:0xb30]
                body[0xb90:0xba0] = old_body[0xb30:0xb40]
                for j in range(3):
                    position = struct.unpack_from('<f', body, 0x1610+j*4)[0]
                    velocity = struct.unpack_from('<f', body, 0x15c0+j*4)[0]
                    put(body, 0x1610+j*4, position + delta*velocity)
                ui(body, 0x3d0+0x5d0, 1)
                next_cases.append((delta, body))
            branches[kind] = next_cases
        all_bytes += len(outputs['original']) - 8
        if outputs['original'] != outputs['repeat'] or outputs['original'] != outputs['rust']:
            differences.append(step)
    assert not differences, differences
    return dict(sequences=len(seed), steps=8, calls=len(seed)*8, words=len(seed)*8*0x2600//4, record_bytes=all_bytes, exact=True, final_sha256=hashlib.sha256(outputs['original']).hexdigest(), executions=executions)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    world_record = json.loads((WORLD.parent / 'validation.json').read_text())
    assert digest(WORLD) == world_record['original_fixture_sha256']
    assert digest(TRACK) == world_record['source_sha256']
    source_record = json.loads((MECHANICS.parent / 'validation.json').read_text())
    assert digest(MECHANICS) == source_record['mechanics_fixture_sha256']
    assert digest(SURFACE) == source_record['source_mechanics_sha256']
    cases = build_cases()
    request = out / 'request.private.bin'
    request.write_bytes(b'SP01' + struct.pack('<I', len(cases)) + b''.join(struct.pack('<If', i, d)+body for i, (d, body) in enumerate(cases)))
    rust = ROOT / f'target/{args.profile}/grid-suspension-contact.exe'
    executions = []
    for kind, exe, arguments in [
        ('original', NATIVE, [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, out/'original.bin', MECHANICS]),
        ('repeat', NATIVE, [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, out/'repeat.bin', MECHANICS]),
        ('rust', rust, [TRACK, SURFACE, request, out/'rust.bin']),
    ]:
        executions.append(execute(exe, arguments, out, kind))
    original, repeat, native = [(out / name).read_bytes() for name in ['original.bin', 'repeat.bin', 'rust.bin']]
    assert len(original) == len(repeat) == len(native) == len(request.read_bytes())
    differences = []
    accepted = 0
    for i in range(len(cases)):
        start = 8 + i*RECORD + 8
        for w in range(4):
            accepted += struct.unpack_from('<I', original, start + 0x1990 + w*0x150 + 0xc0)[0] != 0
        for off in range(0, 0x2600, 4):
            a, b = original[start+off:start+off+4], native[start+off:start+off+4]
            if a != b:
                differences.append(dict(case=i, offset=f'{off:x}', original=a.hex(), rust=b.hex()))
    report = dict(profile=args.profile, cases=len(cases), words=len(cases)*0x2600//4, accepted_wheels=accepted,
                  repeat_exact=original==repeat, differing_words=len(differences), differences=differences,
                  hashes={p.name:digest(p) for p in [request, out/'original.bin', out/'repeat.bin', out/'rust.bin', TRACK, SURFACE, WORLD, MECHANICS]}, executions=executions,
                  scope='Whole9E41F0/real packed world/contact/material/coupling helpers; authored bounded body/history inputs, zero material-table offset, extra tyre axes off/static world; normalized contact identities, not a live whole suspension wrapper/driving comparison.')
    if original == repeat and not differences:
        report['semantic_checks'] = semantic(request.read_bytes(), original)
        report['guards'] = guards(out, rust, request.read_bytes()[8:8+RECORD])
        report['carried'] = carry_runs(out, rust, cases)
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k:report[k] for k in ['profile', 'cases', 'words', 'accepted_wheels', 'repeat_exact', 'differing_words']}))
    assert original == repeat and not differences


if __name__ == '__main__':
    main()
