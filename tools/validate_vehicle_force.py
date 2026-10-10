"""Replay complete captured wheel-force calls with the unchanged original code.

Private numeric evidence only. This does not yet claim Rust vehicle equivalence.
"""
import argparse
import hashlib
import json
import random
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SIZES = {'body': 0x2600, 'view': 0x300, 'solver': 0xdd0}
RECORD_SIZE = 36 + sum(SIZES.values()) + 4 * 0x140 + 4 + 32


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def private(path):
    resolved = path.resolve()
    if not resolved.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('evidence must stay under ignored local/')
    return resolved


def exact_hex(value, size):
    data = bytes.fromhex(value)
    if len(data) != size:
        raise ValueError(f'expected {size} bytes, got {len(data)}')
    return data


def record(case, phase):
    blocks = [exact_hex(case[f'{key}_{phase}_hex'], size) for key, size in SIZES.items()]
    body, view, _ = blocks
    addresses = [int(case[key], 16) for key in ('body', 'view', 'solver')]
    if struct.unpack_from('<I', body)[0] != addresses[1] or struct.unpack_from('<I', view)[0] != addresses[2]:
        raise ValueError('unstable body/view/solver pointers')
    if addresses[2] - addresses[1] != 0x2b0 or view[0x2b0:] != blocks[2][:0x50]:
        raise ValueError('need the observed view/solver overlap, with equal aliased bytes')
    delta_word = int(case['delta_word'], 16)
    delta = struct.unpack('<f', struct.pack('<I', delta_word))[0]
    if not 0 < delta <= .1 or case['mode'] != 1 or not 1 <= case['driven_count'] <= 4:
        raise ValueError('unsupported live mode/delta/driven count')
    materials = sorted(case['materials'], key=lambda item: item['wheel'])
    if [m['wheel'] for m in materials] != list(range(4)):
        raise ValueError('need exactly four wheel material slots')
    presence = [int(bool(int(m['address'], 16))) for m in materials]
    data = struct.pack('<9I', case['index'], delta_word, *addresses, *presence) + b''.join(blocks)
    for material, present in zip(materials, presence):
        data += exact_hex(material[f'{phase}_hex'], 0x140) if present else bytes(0x140)
    data += exact_hex(case['engine_limit_hex'], 4)
    data += exact_hex(case[f'friction_diagnostics_{phase}_hex'], 32)
    assert len(data) == RECORD_SIZE
    return data


def malformed_runs(output, probe, reference, first):
    """Reject malformed owned fixtures before entering original instructions."""
    mutations = [('bad_index', 0, 9, 10), ('nan_delta', 4, 0x7fc00000, 11),
                 ('zero_delta', 4, 0, 11), ('mode', 36+0x1598, 0, 11),
                 ('axes', 36+0xadc, 1, 11), ('wheel_count', 36+0x1620, 5, 11),
                 ('drive_count', 36+0x18f0, 5, 11), ('gear', 36+0x196c, 16, 11),
                 ('view_pointer', 36, 0, 11), ('solver_pointer', 36+0x2600, 0, 11),
                 ('overlap', 36+0x2600+0x2b0, 0xffffffff, 11),
                 ('wheel_owner', 36+0x1990+0x34, 0, 12), ('material_presence', 20, 0, 12),
                 ('foreign_driven', 36+0x18d8, 1, 13),
                 ('duplicate_driven', 36+0x18dc, struct.unpack_from('<I', first, 36+0x18d8)[0], 13)]
    result = []
    for name, offset, value, expected in mutations:
        data = bytearray(first)
        struct.pack_into('<I', data, offset, value)
        source, target = output / f'guard-{name}.bin', output / f'guard-{name}-out.bin'
        source.write_bytes(struct.pack('<2I', 0x31304657, 1) + data)
        run = subprocess.run([str(probe), str(reference), str(source), str(target)],
                             capture_output=True, text=True, timeout=15)
        passed = run.returncode == expected and 'completed=0 code_unchanged=1' in run.stdout and not run.stderr
        result.append({'name': name, 'exit': run.returncode, 'expected': expected, 'passed': passed})
    return result


def authored_records(cases):
    """Private varied finite inputs; prepared geometry remains supplied."""
    rng = random.Random(2008166)
    result = []
    for index in range(288):
        data = bytearray(record(cases[index % len(cases)], 'before'))
        def put(offset, value):
            struct.pack_into('<I', data, offset, value)
        def number(offset, value):
            struct.pack_into('<f', data, offset, value)
        put(0, index)
        number(4, [1.e-6, 1.e-5, .001, .006, .016, .04, .08, .1][index % 8])
        body = 36
        view = body + 0x2600
        solver = view + 0x300
        material = solver + 0xdd0
        # Deliberately stale unused counts, flags, row tails and map words.
        for i in range(884):
            put(solver+4*i, rng.getrandbits(32))
        mass = rng.uniform(500., 2200.)
        number(body+0x15a0, mass)
        number(body+0x15a4, 1./mass)
        for i in range(3):
            number(body+0x15a8+4*i, rng.uniform(400., 2400.))
        gear = index % 6
        put(body+0x196c, gear)
        number(body+0x18f4+4*gear, 0. if index % 11 == 0 else rng.uniform(.05, 1.5))
        driven = rng.sample(range(4), 1+index % 4)
        put(body+0x18f0, len(driven))
        address = struct.unpack_from('<I', data, 8)[0]
        for i, wheel in enumerate(driven):
            put(body+0x18d8+4*i, address+0x1990+wheel*0x150)
        number(body+0x1648, [0., 2.999, 3., 15.][index % 4])
        number(body+0xe34, [0., .25, 1.][index % 3])
        number(body+0x1894, rng.uniform(0., 900.))
        number(view+0x58, [0., 100., -100., 3000.][index % 4])
        number(view+0x9c, rng.uniform(0., 5000.))
        number(view+0x2a8, rng.uniform(-30., 30.))
        for i in range(4):
            w = body+0x1990+i*0x150
            number(w+0x104, [0., -0., 1.e-6, -1.e-6, .2, -.2, 10., -10., 100., -100., 250., -250.][(index+i*3) % 12])
            number(w+0x100, rng.uniform(.05, 3.))
            number(w+0xe0, rng.uniform(.2, .6))
            brake = [0., .1, 100., 2000., 15000., -10.][(index+i) % 6]
            number(view+0x28+4*i, brake)
            number(view+0x5c+4*i, [0., .1, 10., 100., 2000., 20000.][(index//6+i) % 6])
            number(view+0x6c+4*i, rng.uniform(.2, 2.))
            number(view+0x7c+4*i, rng.uniform(-1., 1.))
            # Clear optional prior spill for some controls; retain live values otherwise.
            if index % 2 == 0:
                for offset in (0x9c, 0xa0, 0xa4):
                    number(w+offset, 0.)
            present = int((index+i) % 5 != 0)
            put(20+4*i, present)
            put(w+0xc0, 0x07000000+i*0x140 if present else 0)
            if present:
                number(material+i*0x140+0x64, rng.uniform(.1, 2.5))
            else:
                data[material+i*0x140:material+(i+1)*0x140] = bytes(0x140)
        data[view+0x2b0:view+0x300] = data[solver:solver+0x50]
        result.append(bytes(data))
    return result


def authored_comparison(output, probe, reference, rust, cases):
    records = authored_records(cases)
    source = output / 'authored-request.bin'
    source.write_bytes(struct.pack('<2I', 0x31304657, len(records)) + b''.join(records))
    # Native/CLI accept 64 cases per bounded request; preserve all comparisons.
    executions, total, mismatch, outputs = [], 0, [], []
    counts = {'compacted': 0, 'wheel_stop_marked': 0, 'solver_failure': 0, 'velocity_changed': 0}
    for start in range(0, len(records), 48):
        batch = [struct.pack('<I', i) + data[4:] for i, data in enumerate(records[start:start+48])]
        path = output / f'authored-{start:03d}.bin'
        path.write_bytes(struct.pack('<2I', 0x31304657, len(batch)) + b''.join(batch))
        actual = []
        for name, exe, arguments in [('original', probe, [reference, path]), ('repeat', probe, [reference, path]), ('rust', rust, [path])]:
            target = output / f'authored-{start:03d}-{name}.bin'
            run = subprocess.run([str(exe), *map(str, arguments), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
            executions.append({'batch': start, 'name': name, 'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
            if run.returncode:
                return {'passed': False, 'executions': executions, 'failure': 'execution failed'}
            actual.append(target.read_bytes())
        for index, request in enumerate(batch):
            offset = 8+index*RECORD_SIZE
            original, repeat, independent = [data[offset:offset+RECORD_SIZE] for data in actual]
            bad = [i for i in range(RECORD_SIZE) if original[i] != repeat[i] or original[i] != independent[i]]
            if bad:
                mismatch.append({'case': start+index, 'bytes': len(bad), 'first_offsets': [hex(i) for i in bad[:24]]})
            body, view, solver = 36, 36+0x2600, 36+0x2600+0x300
            get = lambda o: struct.unpack_from('<I', original, o)[0]
            counts['compacted'] += get(solver) != get(solver+12)
            counts['wheel_stop_marked'] += any(original[view+0x3d:view+0x41])
            counts['solver_failure'] += original[view+0x54] == 0
            counts['velocity_changed'] += original[body+0x15c0:body+0x15e0] != request[body+0x15c0:body+0x15e0]
            total += RECORD_SIZE
        outputs.append(hashlib.sha256(actual[0]).hexdigest())
    return {'passed': not mismatch, 'cases': len(records), 'compared_bytes': total, 'request_sha256': digest(source),
            'coverage': counts, 'differences': mismatch, 'executions': executions, 'original_batch_sha256': outputs}


def carried_comparison(output, probe, reference, rust, cases):
    seeds = authored_records(cases)[32:64]
    state = struct.pack('<2I', 0x31304657, len(seeds)) + b''.join(
        struct.pack('<I', i) + data[4:] for i, data in enumerate(seeds))
    states = [state, state, state]
    executions = []
    for frame in range(12):
        next_states = []
        for branch, (name, exe) in enumerate([('original', probe), ('repeat', probe), ('rust', rust)]):
            source = output / f'carried-{frame:02d}-{name}-in.bin'
            target = output / f'carried-{frame:02d}-{name}-out.bin'
            source.write_bytes(states[branch])
            arguments = [source] if name == 'rust' else [reference, source]
            run = subprocess.run([str(exe), *map(str, arguments), str(target)], cwd=ROOT,
                                 capture_output=True, text=True, timeout=60)
            executions.append({'frame': frame, 'name': name, 'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
            if run.returncode:
                return {'passed': False, 'executions': executions, 'failure': 'execution failed'}
            next_states.append(target.read_bytes())
        if not next_states[0] == next_states[1] == next_states[2]:
            return {'passed': False, 'executions': executions, 'failure': f'carried mismatch at frame {frame}'}
        states = next_states
    return {'passed': True, 'sequences': len(seeds), 'frames_per_sequence': 12,
            'compared_bytes': len(seeds)*12*RECORD_SIZE, 'executions': executions,
            'final_sha256': hashlib.sha256(states[0]).hexdigest(),
            'limits': 'Prepared geometry/vectors held fixed; no world/contact regeneration or pose advance.'}


def rust_guards(output, rust, first):
    fixtures = sorted(output.glob('guard-*.bin'))
    fixtures = [p for p in fixtures if not p.stem.endswith('-out')]
    for name, offset, value in [('nonfinite_mass', 36+0x15a0, 0x7fc00000),
                                ('zero_inertia', 36+0x15a8, 0),
                                ('zero_radius', 36+0x1990+0xe0, 0),
                                ('nan_direction', 36+0x2600+0x1d0, 0x7fc00000)]:
        data = bytearray(first)
        struct.pack_into('<I', data, offset, value)
        path = output / f'rust-guard-{name}.bin'
        path.write_bytes(struct.pack('<2I', 0x31304657, 1) + data)
        fixtures.append(path)
    checks = []
    for source in fixtures:
        target = output / (source.stem+'-rust-result.bin')
        run = subprocess.run([str(rust), str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=15)
        checks.append({'name': source.stem, 'exit': run.returncode, 'no_output': not target.exists(),
                       'passed': run.returncode != 0 and not target.exists()})
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], help='also compare independent Rust and varied private controls')
    args = parser.parse_args()
    capture, output = private(args.capture), private(args.output)
    metadata_path = capture / 'capture.json'
    metadata = json.loads(metadata_path.read_text())
    if (metadata.get('boundary') != 'wheel-force' or metadata.get('entry') != '0x9df8b0'
            or not metadata.get('completed') or not metadata.get('code_exact')
            or not metadata.get('hardware_stop_resumed_before_detach') or metadata.get('debugger_exit') != 0):
        raise ValueError('need a completed unchanged-code wheel-force capture and clean detach')
    paths = sorted(capture.glob('case-*.json'))
    if not 1 <= len(paths) <= 64:
        raise ValueError('case count must be 1..64')
    cases = [json.loads(path.read_text()) for path in paths]
    if [case['index'] for case in cases] != list(range(len(cases))):
        raise ValueError('case indexes must be contiguous')
    header = struct.pack('<2I', 0x31304657, len(cases))
    request = header + b''.join(record(case, 'before') for case in cases)
    expected = header + b''.join(record(case, 'after') for case in cases)
    output.mkdir(parents=True, exist_ok=False)
    request_path = output / 'request.bin'
    request_path.write_bytes(request)
    expected_path = output / 'expected.bin'
    expected_path.write_bytes(expected)
    probe = ROOT / 'local/behavior/vehicle-force-probe.exe'
    reference = ROOT / 'local/compatibility/grid-unpacked.exe'
    report = {'capture_sha256': digest(metadata_path), 'case_sha256': {p.name: digest(p) for p in paths},
              'probe_sha256': digest(probe), 'reference_sha256': digest(reference),
              'request_sha256': digest(request_path), 'expected_sha256': digest(expected_path),
              'cases': len(cases), 'rust_compared': False, 'runs': []}
    outputs = []
    for name in ('original', 'repeat'):
        target = output / f'{name}.bin'
        run = subprocess.run([str(probe), str(reference), str(request_path), str(target)],
                             capture_output=True, text=True, timeout=60)
        item = {'name': name, 'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr}
        report['runs'].append(item)
        if run.returncode:
            break
        actual = target.read_bytes()
        outputs.append(actual)
        item['sha256'] = digest(target)
        item['size'] = len(actual)
        item['mismatched_bytes'] = sum(a != b for a, b in zip(actual, expected)) + abs(len(actual) - len(expected))
        differences = []
        for index in range(len(cases)):
            start = 8 + index * RECORD_SIZE
            blocks = [('header', 36), *SIZES.items(), ('materials', 4 * 0x140), ('engine_limit', 4), ('diagnostics', 32)]
            for block, size in blocks:
                bad = [i for i in range(size) if start+i >= len(actual) or actual[start+i] != expected[start+i]]
                if bad:
                    differences.append({'case': index, 'block': block, 'bytes': len(bad), 'first_offsets': [hex(i) for i in bad[:24]]})
                start += size
        item['differences'] = differences
    report['repeat_exact'] = len(outputs) == 2 and outputs[0] == outputs[1]
    report['live_exact'] = len(outputs) == 2 and all(data == expected for data in outputs)
    report['malformed_guards'] = malformed_runs(output, probe, reference, record(cases[0], 'before'))
    report['guards_passed'] = all(item['passed'] for item in report['malformed_guards'])
    if args.profile:
        rust = ROOT / f'target/{args.profile}/grid-vehicle-force.exe'
        target = output / 'rust-live.bin'
        run = subprocess.run([str(rust), str(request_path), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        live_exact = run.returncode == 0 and target.read_bytes() == expected
        report['rust_compared'] = True
        report['rust'] = {'profile': args.profile, 'sha256': digest(rust), 'exit': run.returncode, 'stderr': run.stderr, 'live_exact': live_exact}
        report['authored'] = authored_comparison(output, probe, reference, rust, cases)
        report['carried'] = carried_comparison(output, probe, reference, rust, cases)
        report['rust_guards'] = rust_guards(output, rust, record(cases[0], 'before'))
        report['rust_passed'] = live_exact and report['authored']['passed'] and report['carried']['passed'] and all(item['passed'] for item in report['rust_guards'])
    (output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 0 if report['live_exact'] and report['repeat_exact'] and report['guards_passed'] and report.get('rust_passed', True) else 1


if __name__ == '__main__':
    raise SystemExit(main())
