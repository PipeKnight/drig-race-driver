"""Verify current native mesh-contact evidence, full outputs and upstream framing."""
import argparse
import json
from pathlib import Path
from validate_mesh_contact import ROOT, IMAGE, NATIVE, check, digest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('report', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    report_path = args.report.resolve()
    report = json.loads(report_path.read_text())
    assert report['passed'] and report['native_reference_only']
    assert digest(IMAGE) == report['image_sha256'] and digest(NATIVE) == report['native_sha256']
    assert digest(ROOT/'tools/validate_mesh_contact.py') == report['script_sha256']
    upstream = ROOT/report['upstream']
    assert digest(upstream) == report['upstream_sha256']
    previous = json.loads(upstream.read_text())
    count = 0
    for row in report['results']:
        i = row['index']
        inp = report_path.parent/f'{i:04}-input.bin'
        assert digest(inp) == row['input_sha256']
        data = inp.read_bytes()
        old = upstream.parent/f'{i:04}-native-input.bin'
        assert digest(old) == previous['results'][i]['native_input_sha256']
        assert data[:4] == b'MC02' and data[4:32] == old.read_bytes()[4:32]
        assert data[40:] == old.read_bytes()[32:]
        baseline = upstream.parent/f'{i:04}-original.bin'
        assert digest(baseline) == previous['results'][i]['output_sha256']
        for kind in ['original', 'repeat']:
            p = report_path.parent/f'{i:04}-{kind}.bin'
            assert digest(p) == row['output_sha256']
            answer = p.read_bytes()
            assert len(answer) == row['bytes']
            result = check(answer, baseline.read_bytes(), data[40:], row['contact_capacity'], row['contact_limit'])
            assert result['contact_count'] == row['contact_count']
            count += 1
    assert report['positive_cases'] == sum(r['contact_count'] > 0 for r in report['results'])
    assert report['contacts'] == sum(r['contact_count'] for r in report['results'])
    assert report['guard_count'] == len(report['guards']) == 19
    for row in report['guards']:
        assert 0 < row['execution']['exit'] < 256 and not row['execution']['stderr']
        assert row.get('no_output') or row.get('existing_output_preserved')
    output = args.output.resolve()
    assert output.is_relative_to((ROOT/'local').resolve())
    with output.open('x') as handle:
        json.dump(dict(passed=True, complete_outputs=count, report_sha256=digest(report_path),
                       native_sha256=digest(NATIVE), upstream_framing=True, current_hashes=True), handle, indent=2)
    print(f'passed complete_outputs={count} current_hashes=1 upstream_framing=1')


if __name__ == '__main__':
    main()
