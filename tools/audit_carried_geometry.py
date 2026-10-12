"""Rebuild retained source-contact requests and verify complete profile outputs."""
import argparse
import json
from pathlib import Path

import validate_carried_geometry as boundary
from validate_carried_geometry import ROOT, digest


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); p.add_argument('reports', type=Path, nargs='+')
    p.add_argument('--regression', type=Path); a = p.parse_args()
    upstream = json.loads(boundary.UPSTREAM.read_text())
    legacy = ROOT/upstream['upstream']; assert digest(legacy) == upstream['upstream_sha256']
    old = json.loads(legacy.read_text())
    attachment = ROOT/old['attachment']; assert digest(attachment) == old['attachment_sha256']
    owner = json.loads(attachment.read_text())['results'][old['attachment_index']]
    game = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
    assert owner['source'] == r'cars\lac\lac.ned'
    for key in ['source', 'csv']: assert digest(game/owner[key]) == owner[key+'_sha256']
    seed = attachment.parent/f"{old['attachment_index']:03}-original.bin"
    assert digest(seed) == old['seed_sha256'] == owner['output_sha256']
    source = boundary.cases()
    complete = guards = intervals = 0
    profiles, hashes = [], []
    boundary.shaped.MAGIC = boundary.shaped.g.MAGIC = boundary.MAGIC
    for path in a.reports:
        path = path.resolve(); r = json.loads(path.read_text())
        assert r['passed'] and r['source_placements'] == 608 and r['cases'] == len(source) == 3648
        assert r['profile'] not in profiles; profiles.append(r['profile'])
        for key, target in [('upstream', boundary.UPSTREAM), ('image', boundary.IMAGE), ('native', boundary.NATIVE), ('rust', ROOT/f"target/{r['profile']}/grid-physical-tick.exe"), ('script', ROOT/'tools/validate_carried_geometry.py')]:
            assert digest(target) == r[key+'_sha256']
        assert r['provenance'] == [dict(source_index=c['source_index'], source_sha256=c['source_sha256'], mode=c['mode'], steps=c['steps']) for c in source]
        outputs, total_bytes = [], 0
        assert len(r['batches']) == 31
        for batch, start in zip(r['batches'], range(0, len(source), 120), strict=True):
            cases = source[start:start+120]
            assert batch['start'] == start and batch['cases'] == len(cases)
            inp = path.parent/f'{start:04}-input.bin'
            assert inp.read_bytes() == boundary.pack(cases) and digest(inp) == batch['input_sha256']
            answers = []
            for kind, execution in zip(['original', 'repeat', 'rust'], batch['executions'], strict=True):
                assert execution['exit'] == 0 and not execution['stderr']
                target = path.parent/f'{start:04}-{kind}.bin'
                assert digest(target) == batch['output_sha256']
                raw = target.read_bytes(); assert len(raw) == batch['bytes']
                answers.append(raw); complete += 1
            assert answers[0] == answers[1] == answers[2]
            outputs.extend(boundary.shaped.decode(answers[0], cases))
            total_bytes += len(answers[0])-8
        assert total_bytes == r['observable_bytes']
        assert boundary.semantics(source, outputs) == r['activity']
        assert sum(c['steps'] for c in source) == r['intervals'] == 38912
        intervals += r['intervals']
        assert len(r['guards']) == 20
        for guard in r['guards']:
            label, kind = guard['label'], guard['kind']
            e = guard['execution']; assert 0 < e['exit'] < 256 and 'fault=' not in e['stderr']
            if label == 'existing': assert digest(path.parent/f'guard-existing-{kind}.bin') == guard['unchanged_sha256']
            else:
                assert digest(path.parent/f'guard-{label}-input.bin') == guard['input_sha256']
                assert guard['no_output'] and not (path.parent/f'guard-{label}-{kind}.bin').exists()
            guards += 1
        hashes.append(digest(path))
    assert sorted(profiles) == ['debug', 'release']
    regression_cases = regression_outputs = 0
    if a.regression:
        path = a.regression.resolve(); reg = json.loads(path.read_text()); assert reg['passed']
        assert reg['image_sha256'] == digest(boundary.IMAGE)
        assert reg['script_sha256'] == digest(ROOT/'tools/replay_carried_geometry_regression.py')
        assert reg['shared_source_sha256'] == digest(ROOT/'tools/probe_physical_tick.c')
        for row in reg['results']:
            reference = ROOT/row['reference']; assert digest(reference) == row['reference_sha256']
            before = reference.parent/row['original_name']; inp = reference.parent/row['input_name']
            assert digest(before) == row['output_sha256'] and digest(inp) == row['input_sha256']
            expected = before.read_bytes(); assert len(expected)-8 == row['bytes']
            for execution in row['executions']:
                kind = execution['kind']
                exe = ROOT/f"local/behavior/carried-geometry-{row['kind']}-regression.exe" if kind == 'native' else ROOT/f'target/{kind}/grid-physical-tick.exe'
                assert digest(exe) == execution['sha256']
                assert execution['execution']['exit'] == 0 and not execution['execution']['stderr']
                target = path.parent/f"{row['index']:02}-{kind}.bin"
                assert digest(target) == execution['output_sha256'] == row['output_sha256']
                assert target.read_bytes() == expected
                regression_outputs += 1
            regression_cases += row['cases']
        assert regression_cases == reg['cases_per_run'] == 6122
        assert sum(row['bytes'] for row in reg['results']) == reg['bytes_per_run'] == 207840804
    output = a.output.resolve(); assert output.is_relative_to((ROOT/'local').resolve())
    result = dict(passed=True, complete_outputs=complete, guards=guards, intervals=intervals,
                  profiles=profiles, requests_rebuilt=True, source_provenance=True, current_hashes=True, report_hashes=hashes,
                  regression_cases_per_run=regression_cases, regression_complete_outputs=regression_outputs)
    with output.open('x') as f: json.dump(result, f, indent=2)
    print(json.dumps(result))


if __name__ == '__main__': main()
