"""Compare supplied whole impact arithmetic and clock-independent repeatability."""
import argparse
import json
import random
import struct
import subprocess
from pathlib import Path
from prepare_deformation_impact import PROBE_SHA, DESCRIPTOR, SETTINGS, record
from validate_steering_control import digest, PE_SHA

ROOT = Path(__file__).resolve().parents[1]


def unpack(data):
    magic, count = struct.unpack_from('<II', data)
    assert magic == 0x314d4947 and 1 <= count <= 1024
    cursor, cases = 8, []
    for _ in range(count):
        records, gain = struct.unpack_from('<If', data, cursor)
        cursor += 8
        d = bytearray(data[cursor:cursor + DESCRIPTOR])
        cursor += DESCRIPTOR
        s = bytearray(data[cursor:cursor + SETTINGS])
        cursor += SETTINGS
        g = bytearray(data[cursor:cursor + 24])
        cursor += 24
        clip, = struct.unpack_from('<I', data, cursor)
        cursor += 4
        r = data[cursor:cursor + records * 48]
        cursor += records * 48
        assert len(d) == DESCRIPTOR and len(s) == SETTINGS and len(r) == records * 48
        cases.append(dict(d=d, s=s, g=g, clip=clip, r=r, gain=gain))
    assert cursor == len(data)
    return cases


def pack(cases):
    result = bytearray(struct.pack('<II', 0x314d4947, len(cases)))
    for c in cases:
        result += struct.pack('<If', len(c['r']) // 48, c['gain'])
        result += c['d'] + c['s'] + c['g'] + struct.pack('<I', c['clip']) + c['r']
    return result


def authored(seed, index, rng):
    c = {k: bytearray(v) if isinstance(v, (bytes, bytearray)) else v for k, v in seed.items()}
    dims = [(2, 2, 2), (3, 3, 3), (2, 3, 3), (6, 5, 13), (5, 5, 16), (4, 4, 16)][index % 6]
    count = dims[0] * dims[1] * dims[2]
    spacing = [0.125, 0.5, 1., 2.][index % 4]
    struct.pack_into('<3if', c['d'], 0xa414, *dims, spacing)
    struct.pack_into('<i', c['d'], 0xa4a0, index % 3)
    struct.pack_into('<i', c['d'], 0xa450, [0, 1, 10, 64][index % 4])
    c['d'][0xa4f0] = index % 2
    struct.pack_into('<2f', c['s'], 0x1024, [0., 0.01, -0.2][index % 3], [0., 0.5, 20.][index % 3])
    struct.pack_into('<f', c['s'], 0x1048, [0., 0.01, 0.05, 1.][(index // 7) % 4])
    struct.pack_into('<fI2f', c['g'], 8, [0., 0.1, -0.01, 0.25][(index // 11) % 4],
                     [1, 2, 10, 64][(index // 3) % 4], [0., 0.857, 1., -0.5][(index // 13) % 4], [0., 1., -2.][index % 3])
    c['clip'], c['gain'] = index % 2, [-2., 0., 0.25, 1., 2.][(index // 5) % 5]
    for j in range(400):
        rest = [float(j % dims[0]) * spacing, float((j // dims[0]) % dims[1]) * spacing,
                float(j // (dims[0] * dims[1])) * spacing, 1.]
        position = [v + rng.uniform(-0.02, 0.02) for v in rest]
        # Nonoverlapping XYZ boxes keep iterative edge normalization finite.
        bounds = [rng.choice([0., 0.05, 0.1, 0.2]) * spacing for _ in range(3)] + [0.2]
        motion = [rng.uniform(-0.01, 0.01) for _ in range(4)]
        marker = 0. if index % 13 == 0 or j % 7 == 0 else float(j % 10 + 1)
        if j == 0 and index % 8 == 0:
            rest[:2], position[:2] = [-0., 0.], [0., -0.]
        struct.pack_into('<17f', c['d'], 0x2580 + j * 80, *(position + rest + bounds + motion + [marker]))
    edges = [0, 1, 2, 3, 4, 5, 7, 8, 31, 64, 1200][index % 11]
    struct.pack_into('<i', c['d'], 0xa410, edges)
    for j in range(edges):
        a = rng.randrange(400)
        b = (a + rng.randrange(1, 400)) % 400
        if j >= edges // 4 * 4 and index % 3 == 0:
            a = b = 399  # Trailing self-edge is not iterated, but is settled.
        struct.pack_into('<hhf', c['d'], j * 8, a, b, rng.uniform(0.1, 2.) * spacing)
    records = [0, 1, 2, 7, 512][index % 5]
    r = bytearray()
    for j in range(records):
        node = [0, count - 1, 399][j % 3]
        anchor = struct.unpack_from('<4f', c['d'], 0x2580 + node * 80)
        direction = [rng.choice([-1., -0., 0., 0.25, 1.]) for _ in range(4)]
        scalar = [-0.1, 0., 0.0001, struct.unpack('<f', bytes.fromhex('6f12833a'))[0], 0.1][(index // 5 + j) % 5]
        r += record(node, direction, anchor, scalar)
    c['r'] = r
    return c


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--prepared', type=Path, required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out, prepared = args.output.resolve(), args.prepared.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and prepared.is_relative_to((ROOT / 'local').resolve())
    prior = json.loads((prepared / 'execution.json').read_text())
    assert prior['execution']['exit'] == 0 and digest(prepared / 'input.bin') == prior['input_sha256']
    assert digest(prepared / 'original.bin') == prior['output_sha256']
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformation-impact-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-deformation-impact.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    cases = unpack((prepared / 'input.bin').read_bytes())
    retained_count = len(cases)
    rng = random.Random(0x9d4970)
    cases += [authored(cases[i % retained_count], i, rng) for i in range(192)]
    for gate_word in (0x3a83126e, 0x3a83126f, 0x3a831270):
        c = unpack(pack([cases[2]]))[0]
        r = bytearray(c['r'])
        struct.pack_into('<I', r, 0x24, gate_word)
        c['r'] = r
        cases.append(c)
    out.mkdir(exist_ok=False)
    request = out / 'input.bin'
    request.write_bytes(pack(cases))
    execution, outputs = {}, {}
    for label, program in [('original', [str(probe), str(image)]), ('original_repeat', [str(probe), str(image)]), ('native', [str(native)])]:
        target = out / f'{label}.bin'
        run = subprocess.run(program + [str(request), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        execution[label] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        (out / 'execution.json').write_text(json.dumps(execution, indent=2))
        assert run.returncode == 0, execution[label]
        outputs[label] = target.read_bytes()
        assert len(outputs[label]) == 8 + len(cases) * DESCRIPTOR
    a, b = outputs['original'], outputs['native']
    differences = [dict(case=(i - 8) // DESCRIPTOR, offset=hex((i - 8) % DESCRIPTOR),
                        original=a[i:i + 4].hex(), native=b[i:i + 4].hex())
                   for i in range(8, len(a), 4) if a[i:i + 4] != b[i:i + 4]]
    semantic = sum(3206 + struct.unpack_from('<I', c['d'], 0xa410)[0] for c in cases)
    guards = []
    for label, offset, value in [('zero_spacing', 0xa420, 0), ('bad_mode', 0xa4a0, 3),
                                 ('negative_bound', 0x25a0, 0xbf800000), ('nonfinite_motion', 0x25b0, 0x7fc00000),
                                 ('bad_edge', 0, 400), ('oversized_nodes', 0xa414, 16)]:
        case = unpack(pack([cases[2]]))[0]
        struct.pack_into('<I', case['d'], offset, value)
        invalid = out / f'{label}.bin'
        invalid.write_bytes(pack([case]))
        records = {}
        for program, command in [('original', [str(probe), str(image)]), ('native', [str(native)])]:
            target = out / f'{label}-{program}.bin'
            run = subprocess.run(command + [str(invalid), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            assert run.returncode != 0 and (not target.exists() or target.stat().st_size <= 8)
            if program == 'native': assert not target.exists()
            records[program] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        guards.append(dict(case=label, execution=records))
    for label, field, offset, value in [('bad_record_index', 'r', 0x20, 400),
                                       ('nonfinite_direction', 'r', 0, 0x7f800000),
                                       ('zero_iterations', 'g', 12, 0),
                                       ('too_many_iterations', 'g', 12, 65),
                                       ('processed_self_edge', 'd', 0, 0)]:
        case = unpack(pack([cases[2]]))[0]
        if field == 'r': case['r'] = bytearray(case['r'])
        struct.pack_into('<I', case[field], offset, value)
        invalid = out / f'{label}.bin'
        invalid.write_bytes(pack([case]))
        records = {}
        for program, command in [('original', [str(probe), str(image)]), ('native', [str(native)])]:
            target = out / f'{label}-{program}.bin'
            run = subprocess.run(command + [str(invalid), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            assert run.returncode != 0 and (not target.exists() or target.stat().st_size <= 8)
            if program == 'native': assert not target.exists()
            records[program] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        guards.append(dict(case=label, execution=records))
    result = dict(profile=args.profile, cases=len(cases), retained_pool_cases=retained_count, authored_cases=195,
                  semantic_fields=semantic, descriptor_words=len(cases) * DESCRIPTOR // 4,
                  exact=not differences, differences=differences,
                  original_repeat_identical=a == outputs['original_repeat'], execution=execution, guards=guards,
                  analysis_sha256=PE_SHA, probe_sha256=PROBE_SHA, native_sha256=digest(native),
                  input_sha256=digest(request), prepared_report_sha256=digest(prepared / 'execution.json'),
                  limits='Supplied impacts, pools, settings and runtime constants; full descriptors compared. Real clocks do not change retained numerical output in these repeated runs. Collision production, physical units, live clip flag and same-tick update order remain unverified.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ('cases', 'semantic_fields', 'descriptor_words', 'exact', 'original_repeat_identical')}))
    assert not differences, differences[:8]
    assert result['original_repeat_identical']


if __name__ == '__main__':
    main()
