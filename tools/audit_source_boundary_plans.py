"""Read-only provenance and complete-file replay audit of scheduled driving."""
import argparse
import itertools
import json
import os
from pathlib import Path

from validate_source_boundary_plans import PLANS, inspect, snapshot
from validate_standard_properties import GAME, ROOT, digest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('report', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    assert args.output.resolve().is_relative_to((ROOT / 'local').resolve())
    report = json.loads(args.report.read_text())
    assert report['passed'] and report['cases'] == 24 and report['ticks'] == 14400
    for path, expected in report['inputs'].items():
        assert digest(Path(path)) == expected, path
    checked = 0
    groups = {}
    for row in report['outputs']:
        key = (row['name'], row['profile'], row['repeat'])
        assert key not in groups
        directory = Path(row['command'][4])
        assert row['exit'] == 0 and not row['stderr']
        assert row['command'][:3] == [str(ROOT / f'target/{row["profile"]}/grid-drive-plan.exe'), str(GAME), str(ROOT / 'local/body-contact-directions.bin')]
        plan = ''.join(' '.join(map(str, segment)) + '\n' for segment in PLANS[row['name']]).replace('\n', os.linesep).encode()
        assert (directory / 'plan.txt').read_bytes() == plan
        assert digest(Path(row['command'][3])) == row['plan_sha256']
        event_parameters=report.get('body_events')
        assert len(row['command'])==(6 if event_parameters else 5)
        if event_parameters:
            assert row['command'][5]==event_parameters
            assert (directory/'body-events.bin').read_bytes()==Path(event_parameters).read_bytes()
        for path, expected in row['files'].items():
            assert digest(Path(path)) == expected, path
            checked += 1
        assert row['metrics'] == inspect(directory, PLANS[row['name']])
        groups[key] = ((directory / 'telemetry.tsv').read_bytes(), snapshot(directory / 'initial.bin', 0), snapshot(directory / 'final.bin', 600))
    assert set(groups) == set(itertools.product(PLANS, ['debug', 'release'], range(2)))
    for name in PLANS:
        assert all(groups[name, profile, repeat] == groups[name, 'debug', 0]
                   for profile in ['debug', 'release'] for repeat in range(2))
    if report.get('body_events'):
        assert any(row['metrics']['global_rng_changed'] for row in report['outputs'])
        assert any(row['metrics']['selected_damage_changed'] for row in report['outputs'])
    assert report['guard_count'] == len(report['guards']) == 14
    for guard in report['guards']:
        directory = args.report.parent / f'guard-{guard["profile"]}-{guard["label"]}'
        assert guard['exit'] != 0
        if guard['label'] == 'existing':
            assert list(directory.iterdir()) == [directory / 'keep.txt']
            assert (directory / 'keep.txt').read_bytes() == b'KEEP'
        else:
            assert not directory.exists()
    baseline_path = ROOT / 'local/reference-race/load-20261006-0128/steam-resource-verification.json'
    baseline = json.loads(baseline_path.read_text(encoding='utf-8-sig'))
    sources = {str(GAME / row['relative_path']): digest(GAME / row['relative_path']) for row in baseline['resources']}
    assert all(sources[str(GAME / row['relative_path'])] == row['steam_sha256'] for row in baseline['resources'])
    assert digest(GAME / 'grid.exe') == baseline['original_executable_sha256']
    result = dict(passed=True, cases=24, checked_primary_files=checked, guards=14,
                  report_sha256=digest(args.report), auditor_sha256=digest(Path(__file__)),
                  baseline_sha256=digest(baseline_path), source_resources=sources,
                  executable_sha256=digest(GAME / 'grid.exe'), inputs=report['inputs'],
                  scope='Current hashes, source preservation, complete per-tick TSV/selected buffers, '
                        'request/control reconstruction and guard output preservation. Shared metric '
                        'decoder; not an independent numerical or original-frame oracle.')
    with args.output.open('x') as output:
        json.dump(result, output, indent=2)
    print(json.dumps({key: value for key, value in result.items() if key not in ['inputs', 'source_resources']}))


if __name__ == '__main__':
    main()
