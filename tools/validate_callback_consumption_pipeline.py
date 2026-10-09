"""Compose callback responses with authored owner/pending state, not a live frame."""
import argparse
import json
from pathlib import Path
import struct
import subprocess

from capture_reference_spawn import private, REFERENCE, ANALYSIS
from check_collision_callback_controls import controls
from validate_collision_callback import pack as callback_pack, compare as callback_compare, PROBE_SHA
from validate_response_consumption import ROOT, PROBE, digest, pack, decode


def responses(data, rows):
    if data[:8] != struct.pack('<II', 0x31434349, len(rows)):
        raise ValueError('Callback header mismatch')
    cursor = 8
    result = []
    for row in rows:
        cursor += 0xa500 + 0x424 + 48
        n = row['contact_count']
        result.append(list(struct.unpack_from('<' + 'f' * n, data, cursor)))
        cursor += n * 4
        if struct.unpack_from('<I', data, cursor)[0] != 1:
            raise ValueError('Selected callback was not handled')
        cursor += 4
    if cursor != len(data):
        raise ValueError('Callback trailing output')
    return result


def handoff_rows(values):
    rows = []
    for response in values:
        n = len(response)
        for prefilled in (False, True):
            # Deliberately authored roles and initial scalar state. No claim that
            # these owners ran simultaneously in either source capture.
            contacts = [[10+i, 1 if i % 3 != 2 else 3,
                         2 if i % 3 == 0 else 0, 0, (i % 3-1)*.25] for i in range(n)]
            rows.append(dict(owners=[[1, 1], [2, 1], [3, 0]], contacts=contacts,
                             pending=[[10+i, -.5] for i in range(0, n, 3)] if prefilled else [],
                             passes=[dict(owner=1, selected=list(range(n)), response=response),
                                     dict(owner=2, selected=list(range(n)),
                                          response=[.5 if i % 2 else -.5 for i in range(n)])]))
    return rows


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
        if path.stat().st_size > 8*1024*1024:
            raise ValueError('Capture exceeds bound')
        capture = json.loads(path.read_text(encoding='utf-8'))
        if not (capture['completed'] and capture['debugger_exit'] == 0
                and capture['code_exact'] and capture['hardware_breakpoints']
                and capture['session']['ExeSHA256'] == REFERENCE):
            raise ValueError('Unverified source capture')
        captures.append(capture)
    rows, bodies = controls(*captures)
    pairs = [(r, b) for r, b in zip(rows, bodies) if r['contact_count'] > 0
             and int(r['response'], 16) and not r['callback_global_disable']
             and bytes.fromhex(r['descriptor_before_hex'])[0xa4af] == 0]
    rows, bodies = map(list, zip(*pairs))
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    callback = ROOT/'local/behavior/collision-callback-probe.exe'
    consumer = ROOT/'local/behavior/response-consumption-probe.exe'
    if digest(image) != ANALYSIS or digest(callback) != PROBE_SHA or digest(consumer) != PROBE:
        raise ValueError('Reference/probe identity mismatch')
    output = private(args.output)
    output.mkdir(parents=True, exist_ok=False)
    request = output/'callback-input.bin'
    request.write_bytes(callback_pack(rows, bodies))
    report = dict(live_verification=False,
                  scope='Whole callback outputs feed whole consumer; authored hybrid callback inputs, owner roles, accumulators, pending pairs and second-owner responses. Separate original invocations, no whole-manager or live scheduling claim.',
                  profile=args.profile, callback_cases=len(rows), handoff_cases=len(rows)*2,
                  passes=len(rows)*4, analysis_sha256=ANALYSIS, callback_probe_sha256=PROBE_SHA,
                  consumer_probe_sha256=PROBE, capture_sha256=[digest(p) for p in paths],
                  rust_callback_sha256=digest(ROOT/f'target/{args.profile}/grid-collision-callback.exe'),
                  rust_consumer_sha256=digest(ROOT/f'target/{args.profile}/grid-response-consumption.exe'), runs={})
    original_callback = original_consumer = None
    for label in ('original', 'repeat', 'rust'):
        cb_command = [str(callback), str(image)] if label != 'rust' else [str(ROOT/f'target/{args.profile}/grid-collision-callback.exe')]
        con_command = [str(consumer), str(image)] if label != 'rust' else [str(ROOT/f'target/{args.profile}/grid-response-consumption.exe')]
        cb_result = output/f'{label}-callback.bin'
        cb_run = subprocess.run(cb_command+[str(request), str(cb_result)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        if cb_run.returncode:
            raise RuntimeError(f'{label} callback failed: {cb_run.stderr}')
        cb_data = cb_result.read_bytes()
        if original_callback is None:
            original_callback = cb_data
        check = callback_compare(cb_data, original_callback, len(rows))
        values = responses(cb_data, rows)
        supplied = handoff_rows(values)
        con_request = output/f'{label}-consumer-input.bin'
        con_request.write_bytes(pack(supplied))
        con_result = output/f'{label}-consumer.bin'
        con_run = subprocess.run(con_command+[str(con_request), str(con_result)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        if con_run.returncode:
            raise RuntimeError(f'{label} consumer failed: {con_run.stderr}')
        con_data = con_result.read_bytes()
        states = decode(con_data, supplied)
        if original_consumer is None:
            original_consumer = con_data
        different = [i for i in range(8, min(len(con_data), len(original_consumer)), 4)
                     if con_data[i:i+4] != original_consumer[i:i+4]]
        activity = dict(nonzero_callback_response_cases=sum(any(v != 0 for v in vs) for vs in values),
                        changed_callback_response_cases=sum(struct.pack('<'+'f'*len(vs), *vs) != bytes.fromhex(r['response_before_hex']) for r, vs in zip(rows, values)),
                        changed_accumulator_cases=sum(s[-1]['accumulated'] != [c[4] for c in r['contacts']] for r, s in zip(supplied, states)),
                        first_pass_pending_changes=sum(s[0]['pending'] != r['pending'] for r, s in zip(supplied, states)))
        report['runs'][label] = dict(callback=check, consumer_compared_words=(len(con_data)-8)//4,
                                    consumer_mismatched_words=len(different), first_differences=different[:20],
                                    callback_stdout=cb_run.stdout, consumer_stdout=con_run.stdout, **activity,
                                    passed=check['passed'] and con_data == original_consumer and all(activity.values()))
    report['passed'] = all(r['passed'] for r in report['runs'].values())
    (output/'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']:
        raise RuntimeError('Callback/consumption pipeline failed; inspect private report')


if __name__ == '__main__':
    main()
