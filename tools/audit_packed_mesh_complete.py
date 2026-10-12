"""Audit complete nonaliased packed-generator outputs and rebuilt requests."""
import argparse
import json
from pathlib import Path

import validate_packed_mesh_complete as boundary
from validate_packed_mesh_complete import ROOT, digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('report', type=Path)
    p.add_argument('output', type=Path)
    p.add_argument('--regression', type=Path)
    a = p.parse_args()
    path = a.report.resolve(); r = json.loads(path.read_text())
    assert r['passed'] and r['cases'] == 2642 and r['full_descriptor']
    assert not r['rust_generator_equivalence'] and not r['full_factory_equivalence']
    for key, source in [('native', boundary.NATIVE), ('image', boundary.IMAGE), ('upstream', boundary.UPSTREAM), ('script', ROOT/'tools/validate_packed_mesh_complete.py')]:
        assert digest(source) == r[key+'_sha256']
    complete = contacts = size = changed = source_contacts = legacy_differences = 0
    legacy_report = json.loads(boundary.UPSTREAM.read_text())
    base_contacts = {}
    for i, (row, (meta, request)) in enumerate(zip(r['results'], boundary.inputs(), strict=True)):
        assert row['index'] == i and all(row[k] == v for k, v in meta.items())
        inp = path.parent/f'{i:04}-input.bin'
        assert inp.read_bytes() == request and digest(inp) == row['input_sha256']
        answers = []
        for kind, execution in zip(['original', 'repeat'], row['executions'], strict=True):
            assert execution['exit'] == 0 and not execution['stderr']
            assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in execution['stdout']
            target = path.parent/f'{i:04}-{kind}.bin'
            assert digest(target) == row['output_sha256']
            answer = target.read_bytes(); assert len(answer) == row['bytes']
            observed = json.loads(json.dumps(boundary.check(answer, request)))
            assert all(row[k] == v for k, v in observed.items())
            answers.append(answer); complete += 1
        assert answers[0] == answers[1]
        area = answers[0][0xfe8:0x17e8]
        if meta['label'] == 'source':
            base_contacts[meta['upstream_index']] = area
            source_contacts += row['contacts']
            old = legacy_report['results'][meta['upstream_index']]
            old_output = boundary.UPSTREAM.parent/f"{old['index']:04}-original.bin"
            assert digest(old_output) == old['output_sha256']
            legacy_differences += area != old_output.read_bytes()[0xfe8:0x17e8]
        else: changed += area != base_contacts[meta['upstream_index']]
        contacts += row['contacts']; size += row['bytes']
    assert contacts == r['contacts'] and size == r['observable_bytes']
    assert changed == r['source_all_contact_differences'] and changed > 0
    assert len(r['guards']) == 9
    for guard in r['guards']:
        assert 0 < guard['execution']['exit'] < 256 and not guard['execution']['stderr']
        label = guard['label']
        if label == 'existing':
            assert digest(path.parent/'guard-existing.bin') == guard['unchanged_sha256']
        else:
            assert digest(path.parent/f'guard-{label}-input.bin') == guard['input_sha256']
            assert guard['no_output'] and not (path.parent/f'guard-{label}.bin').exists()
    regression_outputs = regression_guards = 0
    if a.regression:
        rp = a.regression.resolve(); reg = json.loads(rp.read_text())
        assert reg['passed'] and reg['complete_outputs'] == 1467
        assert reg['image_sha256'] == digest(boundary.IMAGE)
        assert reg['script_sha256'] == digest(ROOT/'tools/replay_packed_mesh_complete_regression.py')
        assert reg['shared_source_sha256'] == digest(ROOT/'tools/probe_packed_mesh_contact.c')
        assert reg['header_sha256'] == digest(ROOT/'tools/owned_packed_shape_probe.h')
        for spec in reg['specs']:
            reference = ROOT/spec['reference']; assert digest(reference) == spec['reference_sha256']
            kind = spec['kind']; old = json.loads(reference.read_text()); assert old['passed']
            exe = ROOT/f'local/behavior/packed-complete-{kind}-regression.exe'
            assert digest(exe) == spec['native_sha256']
            selected = [r for r in reg['results'] if r['kind'] == kind]
            indices = list(range(len(old['results']))) if kind == 'packed' else sorted({i*(len(old['results'])-1)//9 for i in range(10)})
            assert [row['index'] for row in selected] == indices
            for row in selected:
                i = row['index']; before = old['results'][i]
                inp = reference.parent/f'{i:04}-input.bin'
                previous = reference.parent/f'{i:04}-original.bin'
                target = rp.parent/f'{kind}-{i:04}.bin'
                assert digest(inp) == before['input_sha256'] == row['input_sha256']
                assert digest(previous) == before['output_sha256'] == row['output_sha256'] == digest(target)
                assert target.read_bytes() == previous.read_bytes() and target.stat().st_size == row['bytes']
                assert row['execution']['exit'] == 0 and not row['execution']['stderr']
                regression_outputs += 1
            for g in [g for g in reg['guards'] if g['kind'] == kind]:
                label = g['label']; assert 0 < g['execution']['exit'] < 256 and not g['execution']['stderr']
                if label == 'existing': assert digest(rp.parent/f'guard-{kind}-existing.bin') == g['unchanged_sha256']
                else:
                    assert digest(reference.parent/f'guard-{label}-input.bin') == g['input_sha256']
                    assert g['no_output'] and not (rp.parent/f'guard-{kind}-{label}.bin').exists()
                regression_guards += 1
        assert regression_outputs == reg['complete_outputs'] and regression_guards == reg['guard_count'] == 142
    output = a.output.resolve(); assert output.is_relative_to((ROOT/'local').resolve())
    result = dict(passed=True, complete_outputs=complete, contacts_per_run=contacts, bytes_per_run=size,
                  source_all_contact_differences=changed, guards=9, rebuilt_inputs=True,
                  current_hashes=True, report_sha256=digest(path), regression_outputs=regression_outputs,
                  regression_guards=regression_guards, source_contacts=source_contacts,
                  legacy_source_contacts=legacy_report['contacts'], source_legacy_contact_differences=legacy_differences)
    with output.open('x') as f: json.dump(result, f, indent=2)
    print(json.dumps(result))


if __name__ == '__main__': main()
