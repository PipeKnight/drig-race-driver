"""Audit current mesh-stage reports, source provenance and every complete output."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('reports', nargs='+', type=Path)
    args = parser.parse_args()
    destination = args.output.resolve()
    assert destination.is_relative_to((ROOT / 'local').resolve())
    assert not destination.exists()
    reports = []
    checked_outputs = 0
    for report_path in args.reports:
        report_path = report_path.resolve()
        assert report_path.is_relative_to((ROOT / 'local').resolve())
        report = json.loads(report_path.read_text())
        assert report['passed'] and report['cases'] == len(report['results'])
        overlap = 'upstream' in report
        if overlap:
            upstream_path = ROOT / report['upstream']
            assert digest(upstream_path) == report['upstream_sha256']
            upstream = json.loads(upstream_path.read_text())
            assert upstream['passed'] and upstream['physical_attachment']
        for relative, expected in report['executables'].items():
            assert digest(ROOT / relative) == expected, relative
        observable = 0
        for row in report['results']:
            stem = report_path.parent / format(row['index'], '04' if overlap else '03')
            if overlap:
                source = upstream['results'][row['upstream_index']]
                previous = upstream_path.parent / f"{source['index']:03}"
                for kind, suffix in [('native', '-original.bin'), ('rust', '-input.bin')]:
                    path = Path(str(stem) + f'-{kind}-input.bin')
                    assert digest(path) == row[f'{kind}_input_sha256']
                    assert path.read_bytes()[32:] == Path(str(previous) + suffix).read_bytes()
            else:
                assert digest(Path(str(stem) + '-input.bin')) == row['input_sha256']
            for kind in ['original', 'repeat', 'rust']:
                path = Path(str(stem) + f'-{kind}.bin')
                assert path.stat().st_size == row['bytes']
                assert digest(path) == row['output_sha256'], path
                checked_outputs += 1
            if not overlap and row['kind'] == 'source':
                assert digest(GAME / row['source']) == row['source_sha256']
                if 'csv' in row:
                    assert digest(GAME / row['csv']) == row['csv_sha256']
            observable += row['bytes']
        assert observable == report['observable_bytes']
        assert len(report['guards']) == report['guard_count']
        for guard in [*report['guards'], *report.get('source_rejected', [])]:
            assert 0 < guard['execution']['exit'] < 256
            assert guard.get('no_output') or guard.get('existing_output_preserved')
        reports.append(dict(path=str(report_path.relative_to(ROOT)),
                            sha256=digest(report_path), cases=report['cases'],
                            guards=report['guard_count'], bytes=observable))
    result = dict(passed=True, reports=reports, complete_output_files=checked_outputs)
    with destination.open('x') as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps(dict(passed=True, reports=len(reports), complete_output_files=checked_outputs)))


if __name__ == '__main__':
    main()
