"""Whole original serial wrapper versus staged original oracle and joined Rust.

Inputs are authored hybrids from prior private captures, not live manager frames.
"""
import argparse
import copy
import json
from pathlib import Path
import struct
import subprocess

from capture_reference_spawn import private, REFERENCE, ANALYSIS
from check_collision_callback_controls import controls
from validate_collision_callback import pack as callback_pack, PROBE_SHA as CALLBACK
from validate_response_consumption import ROOT, PROBE as CONSUMER, digest, pack as consumer_pack, decode

MAGIC = 0x31435349
PROBE = '3f8646fbbd425b614c429e25c7de45ee967cfb9e0cfb1f1376ab66b82ff0f24b'


def pack(rows, bodies, extras):
    parts = [struct.pack('<II', MAGIC, len(rows))]
    for row, body, extra in zip(rows, bodies, extras):
        parts.append(callback_pack([row], [body])[8:])
        parts.append(struct.pack('<I', len(extra['pending'])))
        parts.extend(struct.pack('<I', v) for v in extra['registered'])
        parts.extend(struct.pack('<If', *p) for p in extra['pending'])
    return b''.join(parts)


def supplied(rows, bodies):
    result, states, extras = [], [], []
    for index, (source, body) in enumerate(zip(rows, bodies)):
        for variant in range(2):
            row = copy.deepcopy(source)
            n = row['contact_count']
            row['response'] = '0x10000'
            row['response_before_hex'] = bytes(n * 4).hex()
            registered = [int(bool(int(c['counterpart'], 16)) and (i + index) % 3 != 0)
                          for i, c in enumerate(row['contacts'])]
            pending = ([[i, (-1 if i % 2 else 1) * .25] for i in range(n)] if variant else [])
            if variant and n and index % 11 == 0:
                pending = [[i % n, -.5] for i in range(240)]
            result.append(row)
            states.append(copy.deepcopy(body))
            extras.append(dict(registered=registered, pending=pending))
    return result, states, extras


def consume_row(row, extra, responses):
    owners = [[1, 1]]
    contacts = []
    for i, c in enumerate(row['contacts']):
        raw = bytes.fromhex(c['before_hex'])
        has_other = bool(int(c['counterpart'], 16))
        other = i + 2 if has_other else 0
        if other:
            owners.append([other, extra['registered'][i]])
        reverse = raw[0xc] == 0 and struct.unpack_from('<I', raw, 8)[0] == int(row['linked'], 16)
        a, b = (other, 1) if reverse else (1, other)
        contacts.append([i + 1, a, b, int(raw[0xc] != 0), struct.unpack_from('<f', raw, 0x38)[0]])
    return dict(owners=owners, contacts=contacts,
                pending=[[i + 1, v] for i, v in extra['pending']],
                passes=[dict(owner=1, selected=list(range(len(contacts))), response=responses)])


def execute(command, source, target):
    run = subprocess.run(command + [str(source), str(target)], cwd=ROOT,
                         capture_output=True, text=True, timeout=90)
    if run.returncode:
        raise RuntimeError(f'Execution failed {run.returncode}: {run.stdout} {run.stderr}')
    return target.read_bytes(), dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)


def oracle(rows, bodies, extras, output, image, callback, consumer):
    source = output / 'oracle-callback-input.bin'
    source.write_bytes(callback_pack(rows, bodies))
    data, cb_run = execute([str(callback), str(image)], source, output / 'oracle-callback.bin')
    cursor = 8
    prefixes, supplied_rows, active_indices = [], [], []
    for index, (row, extra) in enumerate(zip(rows, extras)):
        prefix = data[cursor:cursor + 0xa500 + 0x424 + 48]
        cursor += len(prefix)
        n = row['contact_count']
        response = list(struct.unpack_from('<' + 'f' * n, data, cursor))
        cursor += n * 4
        handled, = struct.unpack_from('<I', data, cursor)
        cursor += 4
        expected_handled = bool(n and not row['callback_global_disable']
                                and not bytes.fromhex(row['descriptor_before_hex'])[0xa4af])
        if handled != expected_handled:
            raise ValueError('Original callback guard disagrees with independent expectation')
        prefixes.append(prefix)
        if handled:
            active_indices.append(index)
            supplied_rows.append(consume_row(row, extra, response))
    if cursor != len(data):
        raise ValueError('Oracle callback output framing')
    source = output / 'oracle-consumer-input.bin'
    source.write_bytes(consumer_pack(supplied_rows))
    data, con_run = execute([str(consumer), str(image)], source, output / 'oracle-consumer.bin')
    decoded = decode(data, supplied_rows)
    active = {index: states[0] for index, states in zip(active_indices, decoded)}
    parts = [struct.pack('<II', MAGIC, len(rows))]
    activity = dict(handled_cases=len(active), guarded_cases=len(rows)-len(active),
                    accumulated_changes=0, pending_changes=0, full_pending_cases=0,
                    body_angular_changes=0, seed_advances=0)
    for index, (row, body, extra, prefix) in enumerate(zip(rows, bodies, extras, prefixes)):
        initial = consume_row(row, extra, [0.] * row['contact_count'])
        before = [c[4] for c in initial['contacts']]
        state = active.get(index, dict(accumulated=before, pending=initial['pending']))
        activity['accumulated_changes'] += state['accumulated'] != before
        activity['pending_changes'] += state['pending'] != initial['pending']
        activity['full_pending_cases'] += len(extra['pending']) == 240
        activity['body_angular_changes'] += prefix[0xa500+0x210:0xa500+0x220] != bytes.fromhex(row['linked_before_hex'])[0x210:0x220]
        activity['seed_advances'] += struct.unpack_from('<I', prefix, len(prefix)-4)[0] != body['seed']
        parts.append(prefix)
        parts.extend(struct.pack('<f', v) for v in state['accumulated'])
        parts.append(struct.pack('<I', len(state['pending'])))
        parts.extend(struct.pack('<If', token-1, value) for token, value in state['pending'])
    return b''.join(parts), activity, dict(callback=cb_run, consumer=con_run)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('producer_capture', type=Path)
    parser.add_argument('callback_capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=('debug', 'release'), required=True)
    args = parser.parse_args()
    paths = [private(args.producer_capture), private(args.callback_capture)]
    captures = []
    for path in paths:
        if path.stat().st_size > 8*1024*1024: raise ValueError('Capture bound')
        capture = json.loads(path.read_text(encoding='utf-8'))
        if not (capture['completed'] and capture['debugger_exit'] == 0 and capture['code_exact']
                and capture['hardware_breakpoints'] and capture['session']['ExeSHA256'] == REFERENCE):
            raise ValueError('Unverified source template capture')
        captures.append(capture)
    rows, bodies = controls(*captures)
    rows, bodies, extras = supplied(rows, bodies)
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    callback = ROOT/'local/behavior/collision-callback-probe.exe'
    consumer = ROOT/'local/behavior/response-consumption-probe.exe'
    probe = ROOT/'local/behavior/serial-callback-probe.exe'
    rust = ROOT/f'target/{args.profile}/grid-serial-callback.exe'
    if digest(image) != ANALYSIS or digest(callback) != CALLBACK or digest(consumer) != CONSUMER or digest(probe) != PROBE:
        raise ValueError('Reference/oracle identity mismatch')
    out = private(args.output)
    out.mkdir(parents=True, exist_ok=False)
    request = out/'input.bin'
    request.write_bytes(pack(rows, bodies, extras))
    wanted, activity, oracle_runs = oracle(rows, bodies, extras, out, image, callback, consumer)
    (out/'expected.bin').write_bytes(wanted)
    report = dict(live_verification=False, profile=args.profile, cases=len(rows),
                  scope='Whole serial B784B0, original registered A064A0 and B77490; authored hybrid callback templates and owner/pending roles. Supplied contacts, controlled RNG seed; no geometric contact production or whole-manager/task claim.',
                  analysis_sha256=ANALYSIS, probe_sha256=digest(probe), rust_sha256=digest(rust),
                  source_sha256=[digest(p) for p in paths], oracle_runs=oracle_runs,
                  activity=activity, runs={}, guards={})
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    for label, command in commands:
        data, result = execute(command, request, out/f'{label}.bin')
        differences = [i for i in range(8, min(len(data), len(wanted)), 4) if data[i:i+4] != wanted[i:i+4]]
        result.update(compared_words=(len(wanted)-8)//4, mismatched_words=len(differences),
                      first_differences=[dict(offset=hex(i), actual=data[i:i+4].hex(), expected=wanted[i:i+4].hex()) for i in differences[:20]],
                      passed=data == wanted)
        report['runs'][label] = result
    base = next(i for i, r in enumerate(rows) if r['contact_count'] > 0 and not r['callback_global_disable'] and not bytes.fromhex(r['descriptor_before_hex'])[0xa4af])
    good = pack([rows[base]], [bodies[base]], [extras[base]])
    bad = dict(truncated=good[:-1], trailing=good+b'\0', zero_cases=good[:4]+bytes(4)+good[8:])
    for name, pending in [('too_many_pending', [[0, 0.]]*241), ('unknown_pending', [[rows[base]['contact_count'], 1.]]), ('nan_pending', [[0, float('nan')]])]:
        extra = copy.deepcopy(extras[base]); extra['pending'] = pending
        bad[name] = pack([rows[base]], [bodies[base]], [extra])
    extra = copy.deepcopy(extras[base]); extra['registered'][0] = 2
    bad['invalid_registration'] = pack([rows[base]], [bodies[base]], [extra])
    for name, offset, value in [('nan_accumulated', 0x38, float('nan'))]:
        row = copy.deepcopy(rows[base]); raw = bytearray.fromhex(row['contacts'][0]['before_hex'])
        struct.pack_into('<f', raw, offset, value); row['contacts'][0]['before_hex'] = raw.hex()
        bad[name] = pack([row], [bodies[base]], [extras[base]])
    row = copy.deepcopy(rows[base]); row['response_before_hex'] = struct.pack('<f', 1.).hex()+row['response_before_hex'][8:]
    bad['nonzero_initial_response'] = pack([row], [bodies[base]], [extras[base]])
    row = copy.deepcopy(rows[base]); row['response'] = '0x0'
    bad['absent_response_operand'] = pack([row], [bodies[base]], [extras[base]])
    for name, data in bad.items():
        source = out/f'guard-{name}.bin'; source.write_bytes(data); report['guards'][name] = {}
        for label, command in (commands[0], commands[2]):
            target = out/f'guard-{name}-{label}.bin'
            run = subprocess.run(command+[str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=20)
            report['guards'][name][label] = dict(exit=run.returncode, stderr=run.stderr,
                passed=0 < run.returncode < 256 and 'producer_fault=' not in run.stderr and (label == 'original' or not target.exists()))
    report['passed'] = all(r['passed'] for r in report['runs'].values()) and all(activity.values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']: raise RuntimeError('Serial callback mismatch; inspect private evidence')


if __name__ == '__main__': main()
