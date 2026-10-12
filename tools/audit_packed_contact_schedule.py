"""Audit complete scheduled outputs, direct-reference framing and legacy replay."""
import argparse
import json
import struct
from pathlib import Path

import validate_packed_contact_schedule as schedule

ROOT, digest = schedule.ROOT, schedule.digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('scheduled', type=Path)
    p.add_argument('regression', type=Path)
    p.add_argument('output', type=Path)
    args = p.parse_args()
    path, legacy_path = args.scheduled.resolve(), args.regression.resolve()
    r, legacy = json.loads(path.read_text()), json.loads(legacy_path.read_text())
    assert r['passed'] and not r['regression_only']
    assert legacy['passed'] and legacy['regression_only']
    assert digest(schedule.SCHEDULED) == r['native_sha256']
    assert digest(schedule.REGRESSION) == legacy['native_sha256']
    assert digest(ROOT/'tools/validate_packed_contact_schedule.py') == r['script_sha256'] == legacy['script_sha256']
    upstream = ROOT/r['upstream']
    assert digest(upstream) == r['upstream_sha256'] == legacy['upstream_sha256']
    old = json.loads(upstream.read_text())
    assert digest(schedule.geometry.NATIVE) == old['native_sha256']
    assert digest(schedule.IMAGE) == old['image_sha256']
    zero_cases = set()
    complete = total_bytes = 0
    coverage = {}
    for row in r['results']:
        original = old['results'][row['source_index']]
        source_input = upstream.parent/f"{row['source_index']:04}-input.bin"
        assert digest(source_input) == original['input_sha256']
        raw = source_input.read_bytes()
        request = path.parent/f"{row['index']:04}-input.bin"
        assert digest(request) == row['input_sha256']
        assert request.read_bytes() == schedule.scheduled_request(raw, row['flags'])
        seed_size, chunk_size, limit, cache_limit, index_limit = struct.unpack_from('<5I', raw, 4)
        seed, chunk = raw[40:40+seed_size], raw[40+seed_size:]
        assert len(chunk) == chunk_size
        coverage.setdefault(row['source_index'], []).append(row['flags'])
        if row['flags']&1:
            direct_path = upstream.parent/f"{row['source_index']:04}-original.bin"
            assert digest(direct_path) == original['output_sha256']
        else:
            direct_path = path.parent/f"{row['source_index']:04}-zero.bin"
            zero_input = path.parent/f"{row['source_index']:04}-zero-input.bin"
            expected = bytearray(raw)
            struct.pack_into('<I', expected, 12, 0)
            assert zero_input.read_bytes() == expected
            zero_cases.add(row['source_index'])
        assert digest(direct_path) == row['reference_sha256']
        direct = direct_path.read_bytes()
        for kind in ['original', 'repeat']:
            target = path.parent/f"{row['index']:04}-{kind}.bin"
            assert digest(target) == row['output_sha256'] and target.stat().st_size == row['bytes']
            answer = target.read_bytes()
            normalized = schedule.normalize(answer, direct, row['flags'])
            observed = schedule.geometry.check(normalized, seed, chunk,
                                               struct.unpack_from('<4f', raw, 24),
                                               limit if row['flags']&1 else 0,
                                               cache_limit, index_limit)
            assert observed['contacts'] == row['contacts']
            complete += 1
        total_bytes += row['bytes']
    selected = {v['index']: [0, 1, 2, 3] for v in old['results']
                if 'base_index' not in v and v['height'] == 0}
    selected.update({v['index']: [1, 3] for v in old['results'] if 'base_index' in v})
    assert coverage == selected and len(zero_cases) == 245
    assert r['cases'] == len(r['results']) == 1364
    assert r['observable_bytes'] == total_bytes
    assert r['contacts'] == sum(v['contacts'] for v in r['results'])
    assert r['positive_cases'] == sum(v['contacts'] > 0 for v in r['results'])
    assert r['guard_count'] == len(r['guards']) == 26
    for g in r['guards']:
        e = g['execution']
        assert 0 < e['exit'] < 256 and not e['stderr']
        assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
        if g['label'] == 'existing-valid-output':
            target = path.parent/'guard-existing-output.bin'
            assert target.read_bytes() == b'preserve existing evidence' and digest(target) == g['unchanged_sha256']
        else:
            assert g['no_output'] and not (path.parent/f"guard-{g['label']}-output.bin").exists()
    for row in legacy['results']:
        source = old['results'][row['index']]
        target = legacy_path.parent/f"{row['index']:04}-regression.bin"
        assert digest(target) == row['output_sha256'] == source['output_sha256']
        assert target.stat().st_size == row['bytes']
        complete += 1
    assert legacy['cases'] == len(legacy['results']) == old['cases'] == 1417
    assert legacy['observable_bytes'] == sum(v['bytes'] for v in legacy['results']) == old['observable_bytes']
    target = args.output.resolve()
    assert target.is_relative_to((ROOT/'local').resolve())
    with target.open('x') as f:
        json.dump(dict(passed=True, complete_outputs=complete, guarded_zero_references=len(zero_cases),
                       guards=26, rebuilt_inputs=True, current_hashes=True,
                       report_sha256=digest(path), regression_sha256=digest(legacy_path)), f, indent=2)
    print(f'passed complete_outputs={complete} zero_references={len(zero_cases)} guards=26 rebuilt_inputs=1 current_hashes=1')


if __name__ == '__main__':
    main()
