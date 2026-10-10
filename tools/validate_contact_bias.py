"""Whole correction helpers and authored callback-to-correction handoff.

The upstream wrapper executes afresh; supplied contact pools and correction
parameters are authored, not a synchronized live physical frame.
"""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, REFERENCE, ANALYSIS
from check_collision_callback_controls import controls
from validate_serial_callback import supplied, pack as serial_pack, PROBE as SERIAL
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31494243
PROBE = 'ed10efdda97dfe8c5b5586bd89a7ea014e754f4c09c72a5440c6c0ba113cd44b'
SOURCES = ['c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519',
           '90e0c43a0ad21299038872d5ddadb83f4f9ead9ef2c4d61d18a643d2cfe5f73c']


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def pack(rows):
    return struct.pack('<II', MAGIC, len(rows)) + b''.join(
        struct.pack('<III4f', *r[:7]) + r[7] for r in rows)


def authored_row(rng):
    return struct.pack('<48f', *(rng.uniform(-100, 100) for _ in range(48)))


def controls_rows():
    rng = random.Random(20081010)
    rows = []
    for i in range(512):
        values = [rng.uniform(-100, 100), rng.uniform(-100, 100),
                  rng.choice([0., -0., 1., -1., .5, 100.]),
                  rng.choice([1000., 0., -1., f32(3.4028234663852886e38)])]
        mode = i % 16
        if mode == 0: values[:3] = [-0., 0., 1.]
        if mode == 1: values[:3] = [0., 0., -1.]
        if mode == 2: values[:3] = [1., 1., 1.]
        if mode == 3: values[:3] = [0., 1., 1.]
        if mode == 4: values[:3] = [2., 1., 1.]
        if mode == 5: values[:3] = [f32(2**-149), 0., 1.]
        if mode == 6: values[:3] = [f32(2**-126), 0., .5]
        rows.append([i % 2, int(i % 7 == 0), (i // 2) % 2, *values, authored_row(rng)])
    return rows


def expected(rows):
    parts = [struct.pack('<II', MAGIC, len(rows))]
    activity = dict(single=0, two=0, disabled=0, negative=0, nonnegative=0,
                    negative_zero=0, activations=0, vector_changes=0, bias_changes=0)
    for two, disabled, active, accumulated, threshold, scale, limit, before in rows:
        row = bytearray(before)
        activity['two' if two else 'single'] += 1
        if disabled:
            activity['disabled'] += 1
            row[0x40:0x80 if two else 0x60] = bytes(64 if two else 32)
        else:
            bias = f32(f32(f32(accumulated) - f32(threshold)) * f32(scale))
            if bias >= 0:
                activity['nonnegative'] += 1
                activity['negative_zero'] += struct.pack('<f', bias) == b'\0\0\0\x80'
                activity['activations'] += not active
                active = 1
            else:
                activity['negative'] += 1
                bias = 0.
            struct.pack_into('<3f', row, 0x88, 0., limit, bias)
        activity['vector_changes'] += row[0x40:0x80] != before[0x40:0x80]
        activity['bias_changes'] += row[0x90:0x94] != before[0x90:0x94]
        parts.append(struct.pack('<II', 192, active) + row)
    return b''.join(parts), activity


def execute(command, source, target):
    run = subprocess.run(command+[str(source), str(target)], cwd=ROOT,
                         capture_output=True, text=True, timeout=90)
    if run.returncode:
        raise RuntimeError(f'Execution failed {run.returncode}: {run.stdout} {run.stderr}')
    return target.read_bytes(), dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)


def compare(actual, wanted):
    differences = [i for i in range(8, min(len(actual), len(wanted)), 4)
                   if actual[i:i+4] != wanted[i:i+4]]
    return dict(compared_words=(len(wanted)-8)//4, mismatched_words=len(differences),
                first_differences=[dict(offset=hex(i), actual=actual[i:i+4].hex(),
                                       expected=wanted[i:i+4].hex()) for i in differences[:20]],
                passed=actual == wanted)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('producer_capture', type=Path)
    parser.add_argument('callback_capture', type=Path)
    parser.add_argument('serial_evidence', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=('debug', 'release'), required=True)
    args = parser.parse_args()
    paths = [private(args.producer_capture), private(args.callback_capture)]
    captures = []
    for path, sha in zip(paths, SOURCES):
        if path.stat().st_size > 8*1024*1024 or digest(path) != sha:
            raise ValueError('Source template identity/bound')
        capture = json.loads(path.read_text(encoding='utf-8'))
        if not (capture['completed'] and capture['debugger_exit'] == 0 and capture['code_exact']
                and capture['hardware_breakpoints'] and capture['session']['ExeSHA256'] == REFERENCE):
            raise ValueError('Unverified template capture')
        captures.append(capture)
    metadata, bodies = controls(*captures)
    metadata, bodies, extras = supplied(metadata, bodies)
    serial_input = serial_pack(metadata, bodies, extras)
    previous = private(args.serial_evidence)
    prior_report = json.loads((previous/'validation.json').read_text(encoding='utf-8'))
    if not (prior_report['passed'] and prior_report['probe_sha256'] == SERIAL
            and prior_report['source_sha256'] == SOURCES and (previous/'input.bin').read_bytes() == serial_input):
        raise ValueError('Unverified serial evidence')
    baseline = (previous/'original.bin').read_bytes()
    if any((previous/f'{label}.bin').read_bytes() != baseline for label in ['repeat', 'rust', 'expected']):
        raise ValueError('Prior serial outputs disagree')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    probe = ROOT/'local/behavior/contact-bias-probe.exe'
    serial = ROOT/'local/behavior/serial-callback-probe.exe'
    rust = ROOT/f'target/{args.profile}/grid-contact-bias.exe'
    serial_rust = ROOT/f'target/{args.profile}/grid-serial-callback.exe'
    if digest(image) != ANALYSIS or digest(probe) != PROBE or digest(serial) != SERIAL:
        raise ValueError('Reference/probe identity changed')
    out = private(args.output); out.mkdir(parents=True, exist_ok=False)
    source = out/'serial-input.bin'; source.write_bytes(serial_input)
    report = dict(live_verification=False, profile=args.profile,
                  scope='Whole original B0CE10/B0CEC0 correction rows; authored controls and fresh whole serial callback accumulated-contact handoff. Supplied contacts and correction context; no full physical frame, geometry, solver or cadence claim.',
                  analysis_sha256=ANALYSIS, probe_sha256=PROBE, rust_sha256=digest(rust),
                  serial_probe_sha256=SERIAL, serial_rust_sha256=digest(serial_rust),
                  source_sha256=SOURCES, upstream_runs={}, runs={}, guards={})
    upstream = None
    for label, command in [('original', [str(serial), str(image)]), ('rust', [str(serial_rust)])]:
        data, r = execute(command, source, out/f'serial-{label}.bin')
        r.update(compare(data, baseline)); report['upstream_runs'][label] = r
        if not r['passed']: raise RuntimeError('Fresh serial wrapper output changed')
        upstream = data
    rows = controls_rows(); control_count = len(rows)
    rng = random.Random(20081011); cursor = 8; changed = 0
    for index, m in enumerate(metadata):
        cursor += 0xa500+0x424+48
        values = struct.unpack_from('<'+'f'*m['contact_count'], upstream, cursor)
        cursor += len(values)*4
        pending, = struct.unpack_from('<I', upstream, cursor); cursor += 4+pending*8
        for j, (c, value) in enumerate(zip(m['contacts'], values)):
            raw = bytes.fromhex(c['before_hex']); initial, = struct.unpack_from('<f', raw, 0x38)
            two = int(raw[0xc] == 0 and bool(int(c['counterpart'], 16)))
            changed += struct.pack('<f', initial) != struct.pack('<f', value)
            rows.append([two, int(raw[0xe] != 0), (index+j)%2,
                         value, initial, [1., 4., .5, -2.][(index+j)%4], 1000., authored_row(rng)])
    if cursor != len(upstream): raise ValueError('Serial output framing')
    source = out/'input.bin'; source.write_bytes(pack(rows))
    wanted, activity = expected(rows); (out/'expected.bin').write_bytes(wanted)
    report.update(cases=len(rows), authored_controls=control_count, handoff_contacts=len(rows)-control_count,
                  changed_handoff_accumulators=changed, activity=activity)
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    for label, command in commands:
        data, r = execute(command, source, out/f'{label}.bin'); r.update(compare(data, wanted)); report['runs'][label] = r
    good = pack([rows[0]])
    bad = dict(truncated=good[:-1], trailing=good+b'\0', zero_cases=good[:4]+bytes(4)+good[8:],
               too_many_cases=good[:4]+struct.pack('<I',16385)+good[8:])
    for i, name in enumerate(['invalid_two', 'invalid_disabled', 'invalid_active']):
        r = copy.deepcopy(rows[0]); r[i] = 2; bad[name] = pack([r])
    for i, name in enumerate(['nan_accumulated', 'nan_threshold', 'infinite_scale', 'nan_limit']):
        r = copy.deepcopy(rows[1]); r[3+i] = float('inf') if i == 2 else float('nan'); bad[name] = pack([r])
    for name, operands in [('overflow_difference', [3.4e38,-3.4e38,1.]), ('overflow_product', [3.4e38,0.,2.])]:
        r = copy.deepcopy(rows[1]); r[1] = 0; r[3:6] = operands; bad[name] = pack([r])
    for name, data in bad.items():
        source = out/f'guard-{name}.bin'; source.write_bytes(data); report['guards'][name] = {}
        for label, command in (commands[0], commands[2]):
            target = out/f'guard-{name}-{label}.bin'
            run = subprocess.run(command+[str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=20)
            report['guards'][name][label] = dict(exit=run.returncode, stderr=run.stderr,
                passed=0 < run.returncode < 256 and 'contact_bias_fault=' not in run.stderr
                       and (label == 'original' or not target.exists()))
    report['passed'] = bool(changed and all(activity.values())
        and all(r['passed'] for r in report['upstream_runs'].values())
        and all(r['passed'] for r in report['runs'].values())
        and all(r['passed'] for g in report['guards'].values() for r in g.values()))
    (out/'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']: raise RuntimeError('Contact bias validation failed; inspect private evidence')


if __name__ == '__main__': main()
