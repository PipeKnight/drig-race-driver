"""Replay complete captured wheel-force calls with the unchanged original code.

Private numeric evidence only. This does not yet claim Rust vehicle equivalence.
"""
import argparse
import hashlib
import json
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
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
    (output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 0 if report['live_exact'] and report['repeat_exact'] and report['guards_passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
