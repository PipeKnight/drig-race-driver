"""Rebuild source requests and audit every selected collector byte and guard."""
import argparse
import json
import struct
from pathlib import Path

import validate_packed_candidates as primary

ROOT, digest = primary.ROOT, primary.digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('debug', type=Path); p.add_argument('release', type=Path); p.add_argument('output', type=Path)
    a = p.parse_args()
    reference = json.loads(primary.REFERENCE.read_text())
    assert reference['passed'] and reference['cases'] == 2642
    reports = [(path.resolve(), json.loads(path.read_text())) for path in [a.debug, a.release]]
    for path, report in reports:
        assert report['passed'] and report['cases'] == 2642 and not report['fresh_original_execution']
        assert report['reference_sha256'] == digest(primary.REFERENCE)
        assert report['rust_sha256'] == digest(Path(report['rust_path']))
        assert report['script_sha256'] == digest(ROOT/'tools/validate_packed_candidates.py')
    files = 0
    totals = [dict(accepted=0, visited=0, stamps=0, observable_bytes=0) for _ in reports]
    for index, (meta, request) in enumerate(primary.source.inputs()):
        native = reference['results'][index]
        inp = primary.REFERENCE.parent/f'{index:04}-input.bin'
        assert inp.read_bytes() == request and digest(inp) == native['input_sha256']
        original = primary.REFERENCE.parent/f'{index:04}-original.bin'
        repeat = primary.REFERENCE.parent/f'{index:04}-repeat.bin'
        assert digest(original) == digest(repeat) == native['output_sha256']
        full = original.read_bytes(); assert full == repeat.read_bytes(); files += 2
        capacity = struct.unpack_from('<I', request, 20)[0]
        # Independent offsets, without the validator's projection helper.
        expected = full[0x21c8:0x21e8]+full[0x21f4:0x21fc]+full[0x220c:0x4210]+full[0xfd0:0xfd4]+full[0x44f8:-0x40]
        assert len(expected) == 8240+8*capacity
        for j, (path, report) in enumerate(reports):
            row = report['results'][index]
            assert row['index'] == index and all(row[k] == v for k, v in meta.items())
            assert row['input_sha256'] == digest(inp) and row['original_sha256'] == digest(original)
            target = path.parent/f'{index:04}-rust.bin'
            assert digest(target) == row['output_sha256'] and target.read_bytes() == expected
            assert row['execution']['exit'] == 0 and not row['execution']['stderr']
            assert row['bytes'] == len(expected)
            accepted, visited = struct.unpack_from('<2I', expected, 32)
            stamps = sum(x != 0 for x in struct.unpack_from('<4096H', expected, 40))
            assert accepted == row['accepted'] and visited == row['visited'] and stamps == row['stamps']
            assert accepted <= capacity and visited <= capacity
            if struct.unpack_from('<I', request, 12)[0] and struct.unpack_from('<I', request, 16)[0]:
                assert struct.unpack_from('<2I', expected, 8232) == (1, 1)
            else: assert expected == bytes(len(expected))
            for key in ['accepted', 'visited', 'stamps']: totals[j][key] += row[key]
            totals[j]['observable_bytes'] += row['bytes']; files += 1
    guards = 0
    for j, (path, report) in enumerate(reports):
        assert all(report[k] == v for k, v in totals[j].items())
        assert len(report['guards']) == 12
        for g in report['guards']:
            assert 0 < g['execution']['exit'] < 256
            if g['label'] == 'existing': assert digest(path.parent/'guard-existing.bin') == g['unchanged_sha256']
            else:
                inp = path.parent/f"guard-{g['label']}-input.bin"
                assert digest(inp) == g['input_sha256'] and g['no_output']
                assert not (path.parent/f"guard-{g['label']}.bin").exists()
            guards += 1
    target = a.output.resolve(); assert target.is_relative_to((ROOT/'local').resolve()) and not target.exists()
    result = dict(passed=True, cases=2642, complete_files=files, guard_records=guards, totals_per_profile=totals,
                  reports={str(path.relative_to(ROOT)): digest(path) for path, _ in reports}, script_sha256=digest(Path(__file__)))
    target.write_text(json.dumps(result, indent=2)); print(json.dumps(result), flush=True)


if __name__ == '__main__': main()
