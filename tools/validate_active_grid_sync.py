"""Whole synchronization and registered car job with supplied active grids.

Retained node pools are private observations, not source-only initialization.
Authored strain histories exercise the reached settling traversal separately.
"""
import argparse
import collections
import json
import math
import random
import struct
import subprocess
from pathlib import Path
import validate_vehicle_sync as sync
import validate_query_car_job as job
from summarize_deformation_capture import load

ROOT = sync.ROOT
PINNED = {
    'local/behavior/general-vehicle-manager-20261008T2213398744933Z.json':
        'fdacccfec8fb847cf086129adf7cd5a8c8add585bf036163f16eb463e9532aa2',
    'local/behavior/general-vehicle-manager-20261008T2215472748164Z.json':
        'a71e5409c25dc6841906b0f2e65aa2b9ea89b9f9d86fce6ab788a75ba14c55c3',
    'local/behavior/query-car-job-release-20261011-final-01/initial-0-original-input.bin':
        '54e0a8b2e2586a516be1fc7bd310cdc1723bacd0661d7971fec6e8db660c47e2',
    'local/behavior/query-car-job-release-20261011-final-01/mechanics-full.private.bin':
        '2d87e3a804ce11ec401fc6d77326c94e3bf850b6ef979785ab9d4cb216a0167d',
}


def integer(row, offset, value):
    struct.pack_into('<I', row, offset, value)


def number(row, offset, value):
    struct.pack_into('<f', row, offset, value)


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def install(row, dimensions, pool, variant):
    row = bytearray(row)
    struct.pack_into('<3I', row, 0xc714, *dimensions)
    row[0x4880:0xc580] = pool
    assert len(pool) == 400 * 80
    integer(row, 0xc7ec, variant % 10)
    row[0xc7b1] = 0 if variant % 8 == 3 else 1
    row[0xc7c4:0xc7ec] = bytes(40)
    # Clear-next must happen before the strict maximum tests. The odd variants
    # keep their gate in the previous slot; variant7 puts >3 only in the new slot.
    gate = [0., .49, .5, 0., 3., 3.0001, -0., 4.][variant % 8]
    index = (variant % 10 + (1 if variant % 8 == 7 else 0)) % 10
    number(row, 0xc7c4 + index * 4, gate)
    for i in range(4):
        number(row, 0xc7b4 + i * 4, 90. + i)
    return bytes(row)


def fixtures():
    for path, expected in PINNED.items():
        assert sync.digest(ROOT / path) == expected, path
    rng = random.Random(2008253)
    dimensions = [(2, 2, 2), (3, 2, 3), (6, 5, 12), (6, 5, 13),
                  (10, 4, 10), (16, 2, 12), (5, 3, 7), (2, 16, 12)]
    grids = []
    for i in range(144):
        dims = dimensions[i % len(dimensions)]
        # Opaque tail lanes/padding are deliberately nonnumeric; only the first
        # two vec4s of active nodes may be consumed by this branch.
        pool = bytearray(struct.pack('<I', 0x7fc01234) * (400 * 20))
        for node in range(math.prod(dims)):
            rest = [rng.uniform(-2, 2) for _ in range(4)]
            displacement = [rng.uniform(-.8, .8) for _ in range(4)]
            if i % 11 == 0:
                displacement[:3] = [0., -0., 0.]
            struct.pack_into('<8f', pool, node * 80,
                             *[f32(a + b) for a, b in zip(rest, displacement)], *rest)
        grids.append((dims, bytes(pool), f'authored-{i}', i))
    for variant in [2, 3, 4, 5]:
        dims, pool, _, _ = grids[0]
        pool = bytearray(pool)
        integer(pool, 0, 0x7fc00000)
        integer(pool, 16, 0xff800000)
        grids.append((dims, bytes(pool), f'skipped-opaque-nodes-{variant}', variant))
    for path in list(PINNED)[:2]:
        _, blocks, rows = load(ROOT / path)
        for row in rows:
            key = f"car_{row['car']}-"
            pool = blocks[key + 'deformation_grid_nodes'] + blocks[key + 'deformation_grid_padding_nodes']
            for variant in [0, 2, 3, 5]:
                grids.append((row['dimensions'], pool, f"{Path(path).stem}-car{row['car']}-gate{variant}", variant))
    return grids


def oracle(before, after):
    """Independent flat-index reference for ring gates and four score words."""
    dims = struct.unpack_from('<3I', before, 0xc714)
    index = (struct.unpack_from('<I', before, 0xc7ec)[0] + 1) % 10
    ring = list(struct.unpack_from('<10f', before, 0xc7c4))
    ring[index] = 0.
    assert after[0xc7c4:0xc7ec] == struct.pack('<10f', *ring)
    maximum = max(ring)
    flag = before[0xc7b1]
    if maximum > 3.:
        flag = 1
    elif maximum < .5 and flag:
        flag = 0
        squared = [0.] * 4
        for node in range(math.prod(dims)):
            x = node % dims[0]
            z = node // (dims[0] * dims[1])
            corner = int(x * 2 >= dims[0]) + 2 * int(z * 2 >= dims[2])
            values = struct.unpack_from('<8f', before, 0x4880 + node * 80)
            d = [f32(values[i] - values[i + 4]) for i in range(4)]
            squares = [f32(v * v) for v in d]
            value = f32(f32(squares[3] + squares[2]) + f32(squares[1] + squares[0]))
            squared[corner] = max(squared[corner], value)
        assert after[0xc7b4:0xc7c4] == struct.pack('<4f', *[math.sqrt(v) for v in squared])
    else:
        assert after[0xc7b4:0xc7c4] == before[0xc7b4:0xc7c4]
    assert after[0xc7b1] == flag
    assert after[0x4880:0xc580] == before[0x4880:0xc580]


def owned_car(row):
    return row[job.b.BODY:job.b.VIEW] + row[job.b.RECORD:job.b.RECORD + job.EXTRA]


def active_guards(out, profile, sync_row, job_row, full):
    checks = []
    mutations = [('dimension_zero', 0xc714, 0), ('dimension_one', 0xc714, 1),
                 ('dimension_high', 0xc714, 17), ('dimension_negative', 0xc714, 0xffffffff),
                 ('node_nan', 0x4880, 0x7fc00000), ('rest_nan', 0x4890, 0x7fc00000),
                 ('node_infinite', 0x488c, 0x7f800000),
                 ('rest_negative_infinite', 0x489c, 0xff800000),
                 ('node_too_large', 0x4880, struct.unpack('<I', struct.pack('<f', 1000001.))[0])]
    for name, offset, word in mutations + [('product_high', None, None)]:
        for mode, base in [('sync', sync_row), ('job', job_row)]:
            row = bytearray(base)
            if offset is None:
                for o in [0xc714, 0xc718, 0xc71c]:
                    integer(row, o if mode == 'sync' else job.car_offset(o), 16)
            else:
                integer(row, offset if mode == 'sync' else job.car_offset(offset), word)
            for kind in ['native', 'rust']:
                label = f'active-guard-{name}-{mode}-{kind}'
                source, target = out / (label + '-input.bin'), out / (label + '-output.bin')
                source.write_bytes(sync.request([row]) if mode == 'sync' else job.b.request([row], b'KF02'))
                if mode == 'sync':
                    exe = sync.NATIVE if kind == 'native' else ROOT / f'target/{profile}/grid-vehicle-sync.exe'
                    args = [sync.REFERENCE, source, target] if kind == 'native' else [source, target]
                else:
                    exe = job.NATIVE if kind == 'native' else ROOT / f'target/{profile}/grid-vehicle-force.exe'
                    args = ([job.b.REFERENCE, job.b.WORLD, source, target, job.b.MECHANICS, full]
                            if kind == 'native' else [job.b.TRACK, job.b.SURFACE, source, target])
                run = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True,
                                     text=True, timeout=60)
                passed = run.returncode == (30 if kind == 'native' else 1) and not target.exists()
                if kind == 'native':
                    passed = passed and not run.stderr and 'code_unchanged=1' in run.stdout
                checks.append(dict(name=label, passed=passed, exit=run.returncode,
                                   stdout=run.stdout, stderr=run.stderr))
    (out / 'active-guards.json').write_text(json.dumps(checks, indent=2))
    assert all(c['passed'] for c in checks), [c for c in checks if not c['passed']]
    return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', required=True, choices=['debug', 'release'])
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    grids = fixtures()
    sync.NATIVE = ROOT / 'local/behavior/active-grid-sync-probe.exe'
    job.NATIVE = ROOT / 'local/behavior/active-grid-car-job-probe.exe'
    records = sync.cases()
    active = [install(records[i % len(records)], dims, pool, variant)
              for i, (dims, pool, _, variant) in enumerate(grids)]
    reports, coverage = [], collections.Counter()

    def compare_sync(branches, label):
        report, answers = sync.compare(out, args.profile, branches, label)
        for before, after in zip(branches['original'], answers['original'], strict=True):
            oracle(before, after)
        reports.append(report)
        coverage.update(report['coverage'])
        return answers

    for start in range(0, len(active), 48):
        batch = active[start:start + 48]
        compare_sync({k: batch for k in ['original', 'repeat', 'rust']}, f'active-{start}')
    carry = {k: active[:16] for k in ['original', 'repeat', 'rust']}
    for frame in range(12):
        carry = compare_sync(carry, f'carry-{frame}')
    # Baseline zero-grid coverage retains the full old domain and guards.
    for start in range(0, len(records), 48):
        batch = records[start:start + 48]
        compare_sync({k: batch for k in ['original', 'repeat', 'rust']}, f'zero-{start}')
    checks = sync.guards(out, args.profile, records[0])
    # Whole registered job uses the existing independently initialized source
    # driver request. Only grids and authored ring/gate state are replaced.
    path = ROOT / list(PINNED)[2]
    raw = path.read_bytes()
    assert raw[:4] == b'KF02' and len(raw) == 8 + 32 * job.RECORD
    source_rows = [raw[8 + i * job.RECORD:8 + (i + 1) * job.RECORD] for i in range(32)]
    full = ROOT / list(PINNED)[3]
    rows = []
    for i, (dims, pool, _, variant) in enumerate(grids):
        source = source_rows[i % 32]
        car = install(owned_car(source), dims, pool, variant)
        rows.append(source[:job.b.BODY] + car[:0x2600] + source[job.b.VIEW:job.b.RECORD]
                    + car[0x2600:] + source[job.b.RECORD + job.EXTRA:])
        assert len(rows[-1]) == job.RECORD
    job_reports = []

    def compare_job(branches, label):
        report, answers = job.compare(out, args.profile, branches, label, full)
        for before, after in zip(branches['original'], answers['original'], strict=True):
            oracle(owned_car(before), owned_car(after))
        job_reports.append(report)
        return answers

    for start in range(0, len(rows), 32):
        batch = rows[start:start + 32]
        compare_job({k: batch for k in ['original', 'repeat', 'rust']}, f'job-active-{start}')
    carry = {k: rows[:16] for k in ['original', 'repeat', 'rust']}
    for frame in range(12):
        carry = compare_job(carry, f'job-carry-{frame}')
    active_checks = active_guards(out, args.profile, active[0], rows[0], full)
    frames = job.framing(out, args.profile, rows[0], full)
    files = {p.name: dict(size=p.stat().st_size, sha256=sync.digest(p))
             for p in sorted(out.iterdir()) if p.is_file()}
    executables = [sync.REFERENCE, sync.NATIVE, job.NATIVE,
                   ROOT / f'target/{args.profile}/grid-vehicle-sync.exe',
                   ROOT / f'target/{args.profile}/grid-vehicle-force.exe']
    result = dict(profile=args.profile, active_fixtures=len(grids), fixture_names=[g[2] for g in grids],
                  sync_calls=sum(r['calls'] for r in reports),
                  sync_bytes=sum(r['record_bytes'] for r in reports),
                  job_calls=sum(r['calls'] for r in job_reports),
                  job_owner_bytes=sum(r['owner_bytes'] for r in job_reports),
                  job_observable_bytes=sum(r['observable_bytes'] for r in job_reports),
                  coverage=dict(coverage), guards=len(checks) + len(active_checks), framing=len(frames),
                  reports=reports, job_reports=job_reports,
                  inputs=PINNED, executables={str(p.relative_to(ROOT)): sync.digest(p) for p in executables},
                  files=files, scope='Whole9BD7F0 and registeredA0A290 with bounded supplied active grid, '
                  'unchanged original code, authored strain histories and retained nodes. Independent carried '
                  'state and Python corner oracle. Stable setup/healthy notification/query scope retained. '
                  'No source grid constructor, new impact producer, dirty rebuild, deformation rendering, '
                  'live scheduling or full driving-world equivalence claim.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: v for k, v in result.items() if k not in
                      ['reports', 'job_reports', 'files', 'fixture_names', 'inputs', 'executables']}))


if __name__ == '__main__':
    main()
