"""Whole original B77770 versus Rust, authored ordered/deduplicated pools."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from validate_response_consumption import ROOT, ANALYSIS, digest
from capture_reference_spawn import private, REFERENCE

PROBE = '26c27384cea06e9c3d71d95093ff7716a1bc2061b8860e08fc9af0fc89bceb82'


def pack(rows):
    data = [struct.pack('<II', 0x31434e4f, len(rows))]
    for r in rows:
        data.append(struct.pack('<4I', len(r['owners']), len(r['pools'][0]), len(r['pools'][1]), r['capacity']))
        data.extend(struct.pack('<2I', *o) for o in r['owners'])
        data.extend(struct.pack('<3I', *c) for pool in r['pools'] for c in pool)
    return b''.join(data)


def controls():
    rows = []
    def row(owners, a, b, cap=32):
        return dict(owners=owners, pools=[a, b], capacity=cap)
    rows.append(row([[1, 1], [2, 1], [3, 0]], [[2, 1, 1], [3, 1, 0], [2, 1, 0]], [[1, 2, 0], [3, 1, 0]]))
    rows.append(row([[1, 1], [2, 1]], [[1, 2, 1]], []))
    rows.append(row([[1, 1], [2, 1]], [], [[2, 1, 0]]))
    rows.append(row([], [], []))
    rows.append(row([[i+1, 1] for i in range(64)], [[i+1, 0, 1] for i in range(64)], [], 0))
    for cap in (0, 1, 32, 0xffffffff):
        r = copy.deepcopy(rows[0]); r['capacity'] = cap; rows.append(r)
    rng = random.Random(20081010)
    while len(rows) < 512:
        n = rng.randrange(1, 65)
        owners = [[1+i, rng.randrange(2)] for i in range(n)]
        a = [[rng.randrange(1, n+1), rng.randrange(n+1), rng.randrange(2)] for _ in range(rng.randrange(513))]
        b = [[rng.randrange(1, n+1), rng.randrange(1, n+1), 0] for _ in range(rng.randrange(513))]
        rows.append(row(owners, a, b, rng.choice([0, 1, 32, 0xffffffff])))
    return rows


def expected(rows):
    result = []
    for row in rows:
        flags = dict(row['owners']); found = []
        candidates = [c[0] for c in row['pools'][0]] + [id for c in row['pools'][1] for id in c[:2]]
        for id in candidates:
            if flags[id] and id not in found: found.append(id)
        result.append(found)
    return result


def output_bytes(rows):
    return struct.pack('<II', 0x31434e4f, len(rows))+b''.join(struct.pack('<'+'I'*(len(r)+1), len(r), *r) for r in rows)


def live_rows(path):
    path = private(path)
    if path.stat().st_size > 8*1024*1024: raise ValueError('Capture exceeds bound')
    c = json.loads(path.read_text(encoding='utf-8'))
    if not (c['completed'] and c['debugger_exit'] == 0 and c['code_exact'] and c['hardware_breakpoints']
            and c['boundary'] == 'owners' and c['session']['ExeSHA256'] == REFERENCE and 1 <= len(c['cases']) <= 32):
        raise ValueError('Unverified owner capture')
    rows, observed = [], []
    for case in c['cases']:
        owners = [[int(o['address'], 16), int(struct.unpack_from('<I', bytes.fromhex(o['registration_hex']))[0] != 0)] for o in case['owners']]
        pools = []
        for pool in case['pools']:
            entries = []
            for e in pool['entries']:
                b = bytes.fromhex(e['before_hex'])
                if len(b) != 0x80: raise ValueError('Contact length mismatch')
                entries.append([*struct.unpack_from('<2I', b, 4), int(b[0xc] != 0)])
            if len(entries) != pool['count']: raise ValueError('Pool count mismatch')
            pools.append(entries)
        if len(pools) != 2 or case['capacity'] != 32 or case['return_address'] not in ('0xb78559', '0xb7872a'):
            raise ValueError('Unverified live owner ABI')
        values = [int(p, 16) for p in case['output_pointers']]
        if len(values) != case['owner_count']: raise ValueError('Live output count mismatch')
        rows.append(dict(owners=owners, pools=pools, capacity=32)); observed.append(values)
    return rows, observed, digest(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path); parser.add_argument('--profile', choices=('debug', 'release'), required=True)
    parser.add_argument('--capture', type=Path, help='Verified live owners boundary; otherwise authored controls')
    args = parser.parse_args(); out = args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()): raise ValueError('Private output required')
    image, probe, rust = ROOT/'local/compatibility/grid-unpacked.exe', ROOT/'local/behavior/owner-collection-probe.exe', ROOT/f'target/{args.profile}/grid-owner-collection.exe'
    if digest(image) != ANALYSIS or digest(probe) != PROBE: raise ValueError('Reference/probe identity mismatch')
    rows = controls(); wanted_rows = expected(rows); capture_hash = None
    if wanted_rows[:5] != [[2, 1], [1], [2, 1], [], list(range(1, 65))] or any(r != [2, 1] for r in wanted_rows[5:9]): raise ValueError('Independent semantic controls failed')
    if args.capture:
        rows, wanted_rows, capture_hash = live_rows(args.capture)
        if expected(rows) != wanted_rows: raise ValueError('Recovered owner ordering disagrees with live output')
    wanted = output_bytes(wanted_rows)
    out.mkdir(parents=True, exist_ok=False); source = out/'input.bin'; source.write_bytes(pack(rows))
    report = dict(live_verification=bool(args.capture), capture_sha256=capture_hash, scope='Whole B77770, valid nonnull first owners and two-body second pool. Unused capacity does not clamp. No manager scheduling claim.', cases=len(rows), seed=None if args.capture else 20081010, profile=args.profile, analysis_sha256=ANALYSIS, probe_sha256=PROBE, rust_sha256=digest(rust), runs={}, guards={})
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    for label, command in commands:
        target = out/f'{label}.bin'; run = subprocess.run(command+[str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        data = target.read_bytes() if target.exists() else b''
        different = [i for i in range(8, min(len(data), len(wanted)), 4) if data[i:i+4] != wanted[i:i+4]]
        report['runs'][label] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr, compared_words=(len(wanted)-8)//4, mismatched_words=len(different), first_differences=different[:20], passed=run.returncode == 0 and data == wanted)
    base = controls()[0]
    good = pack([base]); bad = dict(truncated=good[:-1], trailing=good+b'\0', zero_cases=good[:4]+bytes(4)+good[8:])
    for name, change in [('zero_owner', lambda r: r['owners'][0].__setitem__(0, 0)), ('duplicate_owner', lambda r: r['owners'][1].__setitem__(0, 1)), ('invalid_flag', lambda r: r['owners'][0].__setitem__(1, 2)), ('null_first', lambda r: r['pools'][0][0].__setitem__(0, 0)), ('unknown_first', lambda r: r['pools'][0][0].__setitem__(0, 99)), ('null_second', lambda r: r['pools'][1][0].__setitem__(1, 0)), ('nonbody_second', lambda r: r['pools'][1][0].__setitem__(2, 1))]:
        r = copy.deepcopy(base); change(r); bad[name] = pack([r])
    for name, data in bad.items():
        source = out/f'guard-{name}.bin'; source.write_bytes(data); report['guards'][name] = {}
        for label, command in (commands[0], commands[2]):
            target = out/f'guard-{name}-{label}.bin'; run = subprocess.run(command+[str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=15)
            report['guards'][name][label] = dict(exit=run.returncode, passed=0 < run.returncode < 256 and (label == 'original' or not target.exists()))
    report['passed'] = all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8'); print(json.dumps(report, indent=2))
    if not report['passed']: raise RuntimeError('Owner collection failed; inspect private report')


if __name__ == '__main__': main()
