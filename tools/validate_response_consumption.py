"""Whole original response handoff against independent Rust, authored sequences."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
ANALYSIS = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
PROBE = '8ec365fc477f27995c9f2a20807ff8acd222651a9d656aad43f36552c22d189e'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pack(rows):
    data = [struct.pack('<II', 0x31435352, len(rows))]
    for r in rows:
        data.append(struct.pack('<4I', len(r['owners']), len(r['contacts']), len(r['pending']), len(r['passes'])))
        data.extend(struct.pack('<2I', *o) for o in r['owners'])
        data.extend(struct.pack('<4If', *c) for c in r['contacts'])
        data.extend(struct.pack('<If', *p) for p in r['pending'])
        for p in r['passes']:
            data.append(struct.pack('<2I', p['owner'], len(p['selected'])))
            data.extend(struct.pack('<If', i, v) for i, v in zip(p['selected'], p['response']))
    return b''.join(data)


def decode(data, rows):
    if data[:8] != struct.pack('<II', 0x31435352, len(rows)):
        raise ValueError('Output header mismatch')
    cursor = 8
    result = []
    for r in rows:
        count, = struct.unpack_from('<I', data, cursor)
        cursor += 4
        if count != len(r['passes']):
            raise ValueError('Pass count mismatch')
        sequence = []
        for p in r['passes']:
            nc, ns, np = struct.unpack_from('<3I', data, cursor)
            cursor += 12
            if nc != len(r['contacts']) or ns != len(p['selected']) or np > 240:
                raise ValueError('State counts mismatch')
            values = list(struct.unpack_from('<' + 'f' * (nc + ns), data, cursor))
            cursor += (nc + ns) * 4
            pending = [list(struct.unpack_from('<If', data, cursor + i * 8)) for i in range(np)]
            cursor += np * 8
            sequence.append(dict(accumulated=values[:nc], response=values[nc:], pending=pending))
        result.append(sequence)
    if cursor != len(data):
        raise ValueError('Trailing output')
    return result


def controls():
    def row(name, first=1, second=2, kind=0, value=0., response=None, pending=None):
        return dict(name=name, owners=[[1, 1], [2, 1], [3, 0]],
                    contacts=[[10, first, second, kind, value]], pending=pending or [],
                    passes=[dict(owner=1, selected=[0], response=response or [2.])])
    rows = [row('direct_positive', second=0, value=-1., response=[3.]),
            row('direct_negative', second=0, response=[-3.]),
            row('second_nonbody', kind=1), row('unregistered', first=3, second=1),
            row('registered_first', first=2, second=1), row('registered_second', first=0),
            row('self_only', second=1), row('empty_selection'),
            row('duplicate_selection'), row('duplicate_pending', pending=[[10, 3.], [10, 4.]]),
            row('negative_zero', value=-0., response=[-0.], pending=[[10, -0.]]),
            row('sum_grouping', value=3.125, response=[-1e20], pending=[[10, 1e20]]),
            row('matched_negative', response=[-3.], pending=[[10, 1.]]),
            row('direct_pending_untouched', second=0, response=[-3.], pending=[[10, 9.]])]
    rows[7]['passes'][0].update(selected=[], response=[])
    rows[8]['passes'][0].update(selected=[0, 0], response=[2., 3.])
    r = row('swap_last')
    r['contacts'] += [[11, 1, 2, 0, 0.], [12, 1, 2, 0, 0.]]
    r['pending'] = [[10, 1.], [11, 2.], [12, 3.]]
    rows.append(r)
    for name, index in [('full_drop', 240), ('full_match', 0)]:
        r = row(name)
        r['contacts'] = [[10 + i, 1, 2, 0, 0.] for i in range(241)]
        r['pending'] = [[10 + i, 1.] for i in range(240)]
        r['passes'][0].update(selected=[index], response=[-2.])
        rows.append(r)
    r = row('eight_owner_passes')
    r['passes'] = [dict(owner=1 + i % 2, selected=[0], response=[float(i - 3)]) for i in range(8)]
    rows.append(r)
    r = row('subnormal_direct', second=0, response=[struct.unpack('<f', struct.pack('<I', 1))[0]])
    rows.append(r)
    rng = random.Random(20081010)
    values = [-0., 0., 1e-40, -1e-40, -3., 2., 1e10, -1e10]
    while len(rows) < 512:
        n = rng.randrange(1, 65)
        r = dict(name=f'authored_{len(rows)}', owners=[[1, rng.randrange(2)], [2, rng.randrange(2)], [3, rng.randrange(2)]],
                 contacts=[[10 + i, rng.randrange(4), rng.randrange(4), rng.randrange(2), rng.choice(values)] for i in range(n)],
                 pending=[[10 + rng.randrange(n), rng.choice(values)] for _ in range(rng.randrange(0, 100))], passes=[])
        for _ in range(rng.randrange(1, 9)):
            count = rng.randrange(81)
            r['passes'].append(dict(owner=rng.randrange(1, 4), selected=[rng.randrange(n) for _ in range(count)], response=[rng.choice(values) for _ in range(count)]))
        rows.append(r)
    return rows


def semantic_checks(states):
    return dict(direct_positive=states[0][0]['accumulated'] == [2.],
                direct_negative=states[1][0]['accumulated'] == [-3.],
                second_kind_ignored=states[2][0]['pending'] == [],
                duplicate_selection=states[8][0]['accumulated'] == [5.] and states[8][0]['pending'] == [],
                negative_zero=struct.pack('<f', states[10][0]['accumulated'][0]) == bytes.fromhex('00000080'),
                grouping=states[11][0]['accumulated'] == [3.125],
                clamp=states[12][0]['accumulated'] == [0.] and states[12][0]['response'] == [-2.],
                direct_pending=states[13][0]['pending'] == [[10, 9.]],
                swap_last=[p[0] for p in states[14][0]['pending']] == [12, 11],
                full_drop=len(states[15][0]['pending']) == 240 and states[15][0]['accumulated'][240] == 0.,
                full_match=len(states[16][0]['pending']) == 239,
                multi_owner=len(states[17]) == 8 and states[17][-1]['pending'] == [])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private output required')
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    probe = ROOT / 'local/behavior/response-consumption-probe.exe'
    rust = ROOT / f'target/{args.profile}/grid-response-consumption.exe'
    if digest(image) != ANALYSIS or digest(probe) != PROBE:
        raise ValueError('Reference/probe identity mismatch')
    rows = controls()
    output.mkdir(parents=True, exist_ok=False)
    request = output / 'input.bin'
    request.write_bytes(pack(rows))
    report = dict(scope='Whole originalB77490, supplied owner/response/pending sequences, no imports/helper stubs/code changes. Active pending slots only; no live scheduling/contact-generation claim.',
                  live_verification=False, cases=len(rows), passes=sum(len(r['passes']) for r in rows),
                  seed=20081010, profile=args.profile, analysis_sha256=ANALYSIS, probe_sha256=PROBE, rust_sha256=digest(rust), runs={}, guards={})
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    reference = None
    for label, command in commands:
        target = output / f'{label}.bin'
        run = subprocess.run(command + [str(request), str(target)], capture_output=True, text=True, cwd=ROOT, timeout=60)
        result = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        if run.returncode == 0:
            data = target.read_bytes()
            states = decode(data, rows)
            if reference is None:
                reference = data
            different = [i for i in range(8, min(len(data), len(reference)), 4) if data[i:i+4] != reference[i:i+4]]
            checks = semantic_checks(states)
            result.update(compared_words=(len(data)-8)//4, mismatched_words=len(different), first_differences=different[:20], semantic_checks=checks, passed=len(data)==len(reference) and not different and all(checks.values()))
        report['runs'][label] = result
    good = pack([rows[0]])
    malformed = {'truncated': good[:-1], 'trailing': good+b'\0', 'zero_cases': good[:4]+bytes(4)+good[8:]}
    for name, change in [
        ('zero_owner', lambda r: r['owners'][0].__setitem__(0, 0)),
        ('duplicate_owner', lambda r: r['owners'][1].__setitem__(0, 1)),
        ('unknown_owner', lambda r: r['contacts'][0].__setitem__(1, 99)),
        ('zero_contact', lambda r: r['contacts'][0].__setitem__(0, 0)),
        ('invalid_kind', lambda r: r['contacts'][0].__setitem__(3, 2)),
        ('nan_contact', lambda r: r['contacts'][0].__setitem__(4, float('nan'))),
        ('unknown_pending', lambda r: r.__setitem__('pending', [[99, 1.]])),
        ('nan_pending', lambda r: r.__setitem__('pending', [[10, float('nan')]])),
        ('invalid_selection', lambda r: r['passes'][0].__setitem__('selected', [1])),
        ('nan_response', lambda r: r['passes'][0].__setitem__('response', [float('nan')])),
        ('overflow', lambda r: (r['contacts'][0].__setitem__(4, 3e38), r['passes'][0].__setitem__('response', [3e38]))),
    ]:
        row = copy.deepcopy(rows[0])
        change(row)
        malformed[name] = pack([row])
    for name, data in malformed.items():
        source = output / f'guard-{name}.bin'
        source.write_bytes(data)
        report['guards'][name] = {}
        for label, command in [commands[0], commands[2]]:
            target = output / f'guard-{name}-{label}.bin'
            run = subprocess.run(command+[str(source), str(target)], capture_output=True, text=True, cwd=ROOT, timeout=15)
            report['guards'][name][label] = dict(exit=run.returncode, passed=run.returncode > 0 and (label == 'original' or not target.exists()))
    report['passed'] = all(r.get('passed', False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (output / 'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']:
        raise RuntimeError('Response consumption comparison failed; inspect private report')


if __name__ == '__main__':
    main()
