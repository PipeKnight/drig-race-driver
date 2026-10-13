"""Recheck complete private evidence and current executable/source identities."""
import argparse
import json
from pathlib import Path
import validate_active_grid_sync as active
import validate_vehicle_sync as sync
import validate_query_car_job as job


def audit(path):
    path = path.resolve()
    assert path.is_relative_to((active.ROOT / 'local').resolve())
    report = json.loads(path.read_text())
    out = path.parent
    for name, expected in report['files'].items():
        p = (out / name).resolve()
        assert p.parent == out and p.stat().st_size == expected['size']
        assert sync.digest(p) == expected['sha256'], str(p)
    for name, expected in report['inputs'].items():
        assert sync.digest(active.ROOT / name) == expected, name
    for name, expected in report['executables'].items():
        assert sync.digest(active.ROOT / name) == expected, name
    grids, baseline = active.fixtures(), sync.cases()
    records = [active.install(baseline[i % len(baseline)], dims, pool, variant)
               for i, (dims, pool, _, variant) in enumerate(grids)]
    for start in range(0, len(records), 48):
        expected = sync.request(records[start:start + 48])
        assert (out / f'active-{start}-original-input.bin').read_bytes() == expected
    for start in range(0, len(baseline), 48):
        assert (out / f'zero-{start}-original-input.bin').read_bytes() == sync.request(baseline[start:start + 48])
    fixture = (active.ROOT / list(active.PINNED)[2]).read_bytes()
    job_rows = []
    for i, (dims, pool, _, variant) in enumerate(grids):
        at = 8 + (i % 32) * job.RECORD
        source = fixture[at:at + job.RECORD]
        car = active.install(active.owned_car(source), dims, pool, variant)
        job_rows.append(source[:job.b.BODY] + car[:0x2600] + source[job.b.VIEW:job.b.RECORD]
                        + car[0x2600:] + source[job.b.RECORD + job.EXTRA:])
    for start in range(0, len(job_rows), 32):
        assert (out / f'job-active-{start}-original-input.bin').read_bytes() == job.b.request(job_rows[start:start + 32], b'KF02')
    for kind in ['original', 'repeat', 'rust']:
        expected_sync = sync.request(records[:16])
        expected_job = job.b.request(job_rows[:16], b'KF02')
        for frame in range(12):
            assert (out / f'carry-{frame}-{kind}-input.bin').read_bytes() == expected_sync
            assert (out / f'job-carry-{frame}-{kind}-input.bin').read_bytes() == expected_job
            expected_sync = (out / f'carry-{frame}-{kind}.bin').read_bytes()
            expected_job = (out / f'job-carry-{frame}-{kind}-output.bin').read_bytes()
    cases = byte_count = 0
    for original in sorted(out.glob('*-original.bin')):
        if original.name.startswith('guard-'):
            continue
        label = original.name.removesuffix('-original.bin')
        raw = original.read_bytes()
        assert raw == (out / f'{label}-repeat.bin').read_bytes() == (out / f'{label}-rust.bin').read_bytes()
        request = (out / f'{label}-original-input.bin').read_bytes()
        before = request[8:]
        for at in range(0, len(before), sync.RECORD):
            active.oracle(before[at:at + sync.RECORD], raw[8 + at:8 + at + sync.RECORD])
            cases += 1
        byte_count += len(raw) - 8
    jobs = owner_bytes = 0
    ranges = [(0, job.b.VIEW), (job.b.MATERIAL, job.RECORD)]
    for original in sorted(out.glob('job-*-original-output.bin')):
        label = original.name.removesuffix('-original-output.bin')
        raw = original.read_bytes()
        repeat = (out / f'{label}-repeat-output.bin').read_bytes()
        rust = (out / f'{label}-rust-output.bin').read_bytes()
        request = (out / f'{label}-original-input.bin').read_bytes()
        for at in range(8, len(raw), job.RECORD):
            for start, end in ranges:
                assert raw[at + start:at + end] == repeat[at + start:at + end] == rust[at + start:at + end]
            active.oracle(active.owned_car(request[at:at + job.RECORD]), active.owned_car(raw[at:at + job.RECORD]))
            jobs += 1
            owner_bytes += 0x21000 + 0x424
    assert cases == report['sync_calls'] and byte_count == report['sync_bytes']
    assert jobs == report['job_calls'] and owner_bytes == report['job_owner_bytes']
    checks = []
    for name in ['guards.json', 'active-guards.json', 'framing.json']:
        checks += json.loads((out / name).read_text())
    assert len(checks) == report['guards'] + report['framing'] and all(c['passed'] for c in checks)
    return dict(report=str(path.relative_to(active.ROOT)), sha256=sync.digest(path),
                files=len(report['files']), current_identities=len(report['inputs']) + len(report['executables']),
                sync_calls=cases, sync_bytes=byte_count, job_calls=jobs, job_owner_bytes=owner_bytes,
                checks=len(checks))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('reports', type=Path, nargs='+')
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to((active.ROOT / 'local').resolve())
    result = dict(reports=[audit(p) for p in args.reports])
    with output.open('x') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
