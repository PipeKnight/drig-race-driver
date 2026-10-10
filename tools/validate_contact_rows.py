"""Whole initial contact construction, grouped friction and material-cache controls."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_steering_control import digest
from validate_contact_bias import compare, execute
from validate_contact_bias import PROBE as BIAS_PROBE

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31575243
PROBE = '82bc4dacde0a7f37fb446ff21824f96d99f7c98737c04ca7da47a39942bd0ad3'
MAX = struct.unpack('<f', bytes.fromhex('ffff7f7f'))[0]


def pack(cases):
    result = bytearray(struct.pack('<II', MAGIC, len(cases)))
    for c in cases:
        result += struct.pack('<III6fII13f', len(c['bodies']), len(c['materials']), len(c['contacts']),
                              *c['parameters'], *c['pair'], *c['cache'])
        for b in c['bodies']:
            result += struct.pack('<12fII', *b['position'], *b['angular'], *b['linear'], *b['flags'])
        for m in c['materials']: result += struct.pack('<13f', *m)
        for contact in c['contacts']:
            result += struct.pack('<10I10f', *contact['flags'], *contact['values'])
        for pool in c['rows']:
            for row in pool: result += row
    return bytes(result)


def case(rng, number):
    nb = 4
    nc = [1, 2, 3, 4, 7, 8, 13, 16, 32, 64, 0][number % 11]
    bodies = [dict(position=[rng.uniform(-10, 10) for _ in range(3)]+[1.],
                   angular=[rng.uniform(-5, 5) for _ in range(4)],
                   linear=[rng.uniform(-30, 30) for _ in range(4)],
                   flags=[(number+j) % 2, (number//2+j) % 2]) for j in range(nb)]
    materials = [[float(j), .25*(j+1), .3*(j+1), .01*(j+1), 10.*(j+1), .1*j,
                  2.*(j+1), .5*(j+1), 5.*(j+1), 1.*(j+1), 100.*(j+1),
                  float(j % 2), .123*(j+1)] for j in range(4)]
    mode = number % 16
    if mode in [0, 1, 2, 3, 4]:
        for m in materials:
            m[6:10] = {0: [0.]*4, 1: [MAX, 0., MAX, 0.], 2: [0., 1., 0., 1.],
                       3: [1., 0., 1., 0.], 4: [1., 2., 1., 2.]}[mode]
    if mode == 5:
        for m in materials: m[5] = -1.
    if mode == 6:
        for m in materials: m[1] = 0.
    if mode == 7:
        for m in materials: m[3] = 0.
    if mode == 8:
        for m in materials: m[10] = 1.
    if mode == 9:
        for m in materials: m[2] = 0.
    if mode == 10:
        for m in materials: m[11] = 0.
    if mode == 11:
        for m in materials: m[7] = 0.
    if mode == 12:
        for m in materials: m[6] = 0.
    if mode == 13:
        for b in bodies: b['linear'] = [0.]*4
    cache = [float(i)*.125 for i in range(13)]
    contacts = []
    group = 0
    for j in range(nc):
        length = 1 + number % 4
        end = (j+1) % length == 0 or (j == nc-1 and number % 5 != 0)
        two = (number + group) % 2
        a = group % nb
        b = (a+1) % nb if number % 7 else a
        material_a = (j + number) % 4
        material_b = material_a if number % 6 == 0 else (material_a+1) % 4
        if number % 6 == 1: material_a, material_b = (0, 1) if j % 2 == 0 else (1, 0)
        normal = [rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1), 0.]
        if number % 12 == 0: normal = [0., 1., 0., 0.]
        if number % 12 == 1: normal = [1., 0., 0., 0.]
        if number % 12 == 2: normal = [0.]*4
        if number % 12 == 3: normal = [0., -1., 0., 0.]
        if number % 12 == 4: normal[3] = .125
        if number % 12 == 5: normal = [1e-5, 1e-5, 0., 0.]
        previous = [MAX, 0., .1, 10., -1.][(j+number) % 5]
        accumulated = [-0., 0., .125, 1., -1., 10.][(number+j) % 6]
        contacts.append(dict(flags=[two, a, b, material_a, material_b,
                                   (j+number) % 3, (j+number//3) % 3,
                                   (j+number//2) % 2, int(end), (j+number//4) % 2],
                             values=[accumulated, previous,
                                     *[rng.uniform(-15, 15) for _ in range(3)], 1., *normal]))
        if end: group += 1
    rows = [[bytearray(struct.pack('<48f', *[rng.uniform(-3, 3) for _ in range(48)]))
             for _ in range(max(nc, 1)*(3 if p % 2 else 1))] for p in range(4)]
    pair = [0, 1] if number % 17 == 0 else [0xffffffff]*2
    parameters = [[1., 30., 60., 120.][number % 4], .2, .125,
                  1. if mode == 1 else [0., .25, 1., 2.][(number//4) % 4],
                  [.2, 0., 1., 2.][(number//7) % 4], [.2, 0., 1.][(number//9) % 3]]
    # MAX compliance sentinel is meaningful only before overflowing scale multiplication.
    if mode == 1: parameters[3] = 1.
    return dict(bodies=bodies, materials=materials, contacts=contacts, rows=rows,
                cache=cache, pair=pair, parameters=parameters)


def decode(data, cases):
    at = 8
    decoded = []
    for c in cases:
        context = struct.unpack_from('<23I', data, at)
        cache = struct.unpack_from('<15I', data, at+92)
        at += 152
        pools = []
        for pool in c['rows']:
            pools.append([data[at+i*192:at+(i+1)*192] for i in range(len(pool))])
            at += len(pool)*192
        decoded.append(dict(context=context, cache=cache, rows=pools))
    if at != len(data): raise ValueError('Output framing')
    return decoded


def semantic(cases, decoded):
    checks = dict(normal_counts=True, cursor_counts=True, open_group_count=True,
                  opaque_responses=True, tangent_links=True, final_batches_unflushed=True,
                  disabled_controls_present=False, same_material_controls_present=False,
                  cached_pair_controls_present=False, single_directional_controls_present=False,
                  empty_case=True)
    activity = dict(normals=0, tangent_rows=0, single=0, two=0, groups=0, open_groups=0,
                    alias_contacts=0, special_contacts=0, disabled_contacts=0, cache_changes=0,
                    batch_flushes=0, changed_bias=0, changed_softness=0, asymmetric_friction_rows=0)
    for c, o in zip(cases, decoded):
        ctx = o['context']; counts = [0]*4; last_group = 0; normal_positions = [0]*4
        checks['normal_counts'] &= ctx[4] == len(c['contacts'])
        for v in c['contacts']:
            two, a, b, ma, mb, sa, sb, special, end, disabled = v['flags']
            p = 2 if two else 0
            normal_positions[p] = counts[p]
            counts[p] += 1; last_group += 1
            activity['two' if two else 'single'] += 1
            activity['normals'] += 1; activity['alias_contacts'] += two and a == b
            activity['disabled_contacts'] += disabled; activity['special_contacts'] += special
            checks['disabled_controls_present'] |= bool(disabled)
            checks['same_material_controls_present'] |= ma == mb
            if end:
                activity['groups'] += 1; last_group = 0
                for r in o['rows'][p+1][counts[p+1]:counts[p+1]+3]:
                    checks['tangent_links'] &= struct.unpack_from('<I', r, 0xa8)[0] == ((p+1)<<24)|(normal_positions[p]*192+0x98)
                counts[p+1] += 3; activity['tangent_rows'] += 3
        checks['cursor_counts'] &= list(ctx[5:9]) == [n*192 for n in counts]
        checks['open_group_count'] &= ctx[22] == last_group
        activity['open_groups'] += bool(last_group)
        checks['cached_pair_controls_present'] |= c['pair'] != [0xffffffff]*2
        checks['single_directional_controls_present'] |= any(b['flags'][1] for b in c['bodies'])
        activity['cache_changes'] += list(o['cache'][2:]) != list(struct.unpack('<13I', struct.pack('<13f', *c['cache'])))
        for p in range(4):
            for i, (before, after) in enumerate(zip(c['rows'][p], o['rows'][p])):
                checks['opaque_responses'] &= before[0x40:0x80] == after[0x40:0x80]
                if i < counts[p]:
                    activity['batch_flushes'] += before[0xb4:0xb8] != after[0xb4:0xb8]
                    activity['changed_bias'] += p % 2 == 0 and before[0x80:0x84] != after[0x80:0x84]
                    activity['changed_softness'] += p % 2 == 0 and before[0x84:0x88] != after[0x84:0x88]
                    lo, hi = struct.unpack_from('<2f', after, 0x88)
                    activity['asymmetric_friction_rows'] += p % 2 == 1 and lo != -hi
            head = ctx[9+p]
            if head != 0xffffffff:
                i = head//192
                checks['final_batches_unflushed'] &= o['rows'][p][i][0xb4:0xb8] == c['rows'][p][i][0xb4:0xb8]
        if not c['contacts']:
            checks['empty_case'] &= all(before == after for before, after in zip(c['rows'], [[bytearray(r) for r in pool] for pool in o['rows']]))
    return checks, activity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    probe = ROOT/'local/behavior/contact-rows-probe.exe'; image = ROOT/'local/compatibility/grid-unpacked.exe'
    rust = ROOT/f'target/{args.profile}/grid-contact-rows.exe'
    if digest(probe) != PROBE or digest(image) != ANALYSIS: raise ValueError('Image/probe identity')
    out = private(args.output); out.mkdir(parents=True, exist_ok=False)
    rng = random.Random(200810100456)
    cases = [case(rng, i) for i in range(528)]
    source = out/'input.bin'
    # Replay the prior joined callback's supplied accumulators afresh. Its contact
    # roles/materials/geometry were authored and remain so in this continuation.
    previous = ROOT/f'local/behavior/contact-bias-20261010-{args.profile}-final-02'
    upstream = previous/'input.bin'
    if digest(upstream) != 'b3c6fa0b4885b1f973a2ac49266acce1a2fc4750161f3bf41a0ca67b14164c54':
        raise ValueError('Upstream request identity')
    baseline = (previous/'original.bin').read_bytes()
    if any((previous/f'{label}.bin').read_bytes() != baseline for label in ['repeat', 'rust', 'expected']):
        raise ValueError('Upstream outputs disagree')
    bias_probe = ROOT/'local/behavior/contact-bias-probe.exe'
    if digest(bias_probe) != BIAS_PROBE: raise ValueError('Bias probe identity')
    upstream_runs = {}
    for label, command in [('original', [str(bias_probe), str(image)]),
                           ('rust', [str(ROOT/f'target/{args.profile}/grid-contact-bias.exe')])]:
        data, run = execute(command, upstream, out/f'upstream-{label}.bin')
        run.update(compare(data, baseline)); upstream_runs[label] = run
        if not run['passed']: raise ValueError('Fresh upstream mismatch')
    raw = upstream.read_bytes(); handoff = []
    for index in range(512, 6352):
        two, disabled = struct.unpack_from('<II', raw, 8+index*220)
        accumulated = struct.unpack_from('<f', raw, 8+index*220+12)[0]
        handoff.append((two, disabled, accumulated))
    authored = len(cases)
    for start in range(0, len(handoff), 32):
        group = handoff[start:start+32]
        c = case(rng, 9+11*(start//32))  # always reserves 64 supplied contacts
        c['contacts'] = c['contacts'][:len(group)]
        for j, (two, disabled, accumulated) in enumerate(group):
            contact = c['contacts'][j]
            contact['flags'][0] = two; contact['flags'][9] = disabled
            contact['flags'][8] = int(j == len(group)-1 or j % 4 == 3 or group[j+1][0] != two)
            contact['values'][0] = accumulated
        c['rows'] = [pool[:len(group)*(3 if p % 2 else 1)] for p, pool in enumerate(c['rows'])]
        cases.append(c)
    source.write_bytes(pack(cases))
    report = dict(profile=args.profile, analysis_sha256=ANALYSIS, probe_sha256=PROBE,
                  rust_sha256=digest(rust), input_sha256=digest(source), cases=len(cases),
                  scope='Whole B0D1B0/B0E040, original material/compliance/cross helpers; supplied authored contacts, no live contact production or physical frame.',
                  authored_controls=authored, handoff_contacts=len(handoff), upstream_runs=upstream_runs,
                  bias_probe_sha256=BIAS_PROBE, upstream_input_sha256=digest(upstream),
                  live_verification=False, runs={}, guards={})
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    original = None
    for label, command in commands:
        data, run = execute(command, source, out/f'{label}.bin')
        if original is None: original = data
        run.update(compare(data, original)); report['runs'][label] = run
    checks, activity = semantic(cases, decode(original, cases))
    report.update(semantic_checks=checks, activity=activity)
    base = case(random.Random(42), 0); good = pack([base])
    bad = dict(truncated=good[:-1], trailing=good+b'\0', zero_cases=good[:4]+bytes(4)+good[8:],
               excess_cases=good[:4]+struct.pack('<I', 1025)+good[8:])
    mutations = [
        ('bad_time', lambda c: c['parameters'].__setitem__(0, 0.)),
        ('nan_parameter', lambda c: c['parameters'].__setitem__(4, float('nan'))),
        ('bad_cache_pair', lambda c: c.update(pair=[99, 0])),
        ('partial_cache_pair', lambda c: c.update(pair=[0xffffffff, 0])),
        ('nan_cache', lambda c: c['cache'].__setitem__(3, float('nan'))),
        ('nan_body', lambda c: c['bodies'][0]['linear'].__setitem__(0, float('nan'))),
        ('bad_body_flag', lambda c: c['bodies'][0]['flags'].__setitem__(0, 2)),
        ('nan_material', lambda c: c['materials'][0].__setitem__(3, float('nan'))),
        ('bad_contact_flag', lambda c: c['contacts'][0]['flags'].__setitem__(0, 2)),
        ('unknown_body', lambda c: c['contacts'][0]['flags'].__setitem__(1, 99)),
        ('unknown_material', lambda c: c['contacts'][0]['flags'].__setitem__(3, 99)),
        ('bad_shape_flag', lambda c: c['contacts'][0]['flags'].__setitem__(5, 3)),
        ('bad_group_flag', lambda c: c['contacts'][0]['flags'].__setitem__(8, 2)),
        ('bad_disabled_flag', lambda c: c['contacts'][0]['flags'].__setitem__(9, 2)),
        ('nan_point', lambda c: c['contacts'][0]['values'].__setitem__(2, float('nan'))),
        ('nan_accumulator', lambda c: c['contacts'][0]['values'].__setitem__(0, float('nan'))),
        ('nan_previous_impulse', lambda c: c['contacts'][0]['values'].__setitem__(1, float('nan'))),
    ]
    for name, mutate in mutations:
        c = copy.deepcopy(base); mutate(c); bad[name] = pack([c])
    for name, request in bad.items():
        guard = out/f'guard-{name}.bin'; guard.write_bytes(request); report['guards'][name] = {}
        for label, command in [commands[0], commands[2]]:
            target = out/f'guard-{name}-{label}.bin'
            run = subprocess.run(command+[str(guard), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=20)
            report['guards'][name][label] = dict(exit=run.returncode, stderr=run.stderr,
                passed=0 < run.returncode < 256 and 'contact_rows_fault=' not in run.stderr and (label == 'original' or not target.exists()))
    report['passed'] = bool(all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values())
                            and all(r['passed'] for r in upstream_runs.values())
                            and all(r['passed'] for guard in report['guards'].values() for r in guard.values()))
    (out/'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']: raise RuntimeError('Contact-row validation failed; inspect private evidence')


if __name__ == '__main__': main()
