"""Check every outer-query result and earlier distance regression from source."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_mesh_outer as outer
from validate_triangle_mesh_outer import ROOT, digest, distance, sequence
from extract_contact_directions import extract


def main():
    p = argparse.ArgumentParser()
    for name in ['cold_debug', 'cold_release', 'sequence_debug', 'sequence_release', 'regression', 'output']:
        p.add_argument(name, type=Path)
    a = p.parse_args()
    names = ['cold_debug', 'cold_release', 'sequence_debug', 'sequence_release', 'regression']
    paths = [getattr(a, k).resolve() for k in names]
    reports = [json.loads(v.read_text()) for v in paths]
    assert all(r['passed'] for r in reports)
    sampling = json.loads(outer.SAMPLING.read_text()); table_path = outer.SAMPLING.parent/'directions.bin'; table = table_path.read_bytes()
    assert sampling['passed'] and digest(table_path) == sampling['output_sha256'] and extract()[0] == table
    assert sampling['initializer_image_sha256'] == digest(distance.IMAGE)
    old = json.loads(distance.UPSTREAM.read_text()); assert old['passed']
    track = Path(old['source_path']); assert digest(track) == old['source_sha256']
    attachment = ROOT/old['attachment']; assert digest(attachment) == old['attachment_sha256']
    attached = json.loads(attachment.read_text())['results'][old['attachment_index']]
    game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for k in ['source', 'csv']: assert digest(game/attached[k]) == attached[k+'_sha256']
    chunks = {name:(vertices, triangles) for name, vertices, triangles, _, _ in distance.geometry.chunks(track.read_bytes())}
    complete, guards = 0, 0
    totals = {}
    for is_sequence, profile_reports, profile_paths in [(False, reports[:2], paths[:2]), (True, reports[2:4], paths[2:4])]:
        reference = outer.RETAINED if is_sequence else outer.COLD
        previous = json.loads(reference.read_text()); expected_cases = 1273 if is_sequence else 1801
        native = outer.NATIVE_SEQUENCE if is_sequence else distance.OUTER
        assert all(r['reference_sha256'] == digest(reference) and r['cases'] == len(r['results']) == expected_cases for r in profile_reports)
        total_queries, total_hits, total_extra, native_bytes = 0, 0, 0, 0
        for left, right in zip(profile_reports[0]['results'], profile_reports[1]['results'], strict=True):
            i = left['index']; assert i == right['index'] and left['source_index'] == right['source_index']
            source_row = previous['results'][left['source_index']]
            source = reference.parent/f"{source_row['index']:04}-input.bin"; assert digest(source) == source_row['input_sha256']
            expected = (b'PC06' if is_sequence else b'PC03')+source.read_bytes()[4:]
            originals = []
            for profile, report, row, path in zip(['debug', 'release'], profile_reports, [left, right], profile_paths, strict=True):
                assert report['profile'] == profile and report['sequence'] == is_sequence
                assert report['native_sha256'] == digest(native) and report['rust_sha256'] == digest(ROOT/f'target/{profile}/grid-convex-distance.exe')
                assert report['script_sha256'] == digest(ROOT/'tools/validate_triangle_mesh_outer.py') and report['sampling_sha256'] == digest(outer.SAMPLING)
                assert report['image_sha256'] == digest(distance.IMAGE)
                inp = path.parent/f'{i:04}-input.bin'; assert inp.read_bytes() == expected and digest(inp) == row['input_sha256']
                for e in row['executions'][:2]:
                    assert e['exit'] == 0 and not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
                for name in ['original', 'repeat']:
                    target = path.parent/f'{i:04}-{name}.bin'; assert digest(target) == row['output_sha256'] and target.stat().st_size == row['bytes']
                    answer = target.read_bytes(); observed = outer.check(answer, expected, table)
                    for k, v in observed.items(): assert row[k] == v
                    originals.append(answer); complete += 1
                target = path.parent/f'{i:04}-rust.bin'; assert digest(target) == row['rust_sha256']
                assert target.read_bytes() == outer.projection(originals[-1], expected); complete += 1
            assert all(v == originals[0] for v in originals)
            records = outer.projection(originals[0], expected)[4:] if is_sequence else outer.projection(originals[0], expected)
            original_source_row = old['results'][source_row['source_index']]
            vertices, triangles = chunks[original_source_row['chunk']]
            for step in range(left['queries']):
                offset = 44+step*24 if is_sequence else 40
                ti = struct.unpack_from('<I', expected, offset)[0]
                expected_triangle = b''.join(struct.pack('<4f', *vertices[v], 1.) for v in triangles[ti][0])
                assert records[step*608+0x180:step*608+0x1b0] == expected_triangle
            total_queries += left['queries']; total_hits += left['hits']; total_extra += left['extra_data_queries']; native_bytes += left['bytes']
        for report, path in zip(profile_reports, profile_paths, strict=True):
            assert (report['queries'], report['hits'], report['extra_data_queries'], report['observable_bytes']) == (total_queries, total_hits, total_extra, native_bytes)
            assert report['guard_count'] == len(report['guards']) == (23 if is_sequence else 41)
            for g in report['guards']:
                assert all(0 < e['exit'] < 256 for e in g['executions'])
                label = g['label']
                if label.startswith('existing-'):
                    target = path.parent/f'guard-{label}.bin'; assert target.read_bytes() == b'preserve existing evidence' and digest(target) == g['unchanged_sha256']
                elif label.startswith('table-'):
                    assert not (path.parent/f'guard-{label}-output.bin').exists()
                    assert digest(path.parent/f'guard-{label}.bin') == g['sampling_sha256']
                else:
                    assert digest(path.parent/f'guard-{label}-input.bin') == g['input_sha256']
                    assert all(not (path.parent/f'guard-{label}-{name}.bin').exists() for name in ['original', 'rust'])
                if not label.startswith('table-') and label != 'existing-rust':
                    e = g['executions'][0]; assert not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
                guards += 1
        totals['sequence' if is_sequence else 'cold'] = dict(cases=expected_cases, queries=total_queries, hits=total_hits,
            extra_data_queries=total_extra, native_bytes=native_bytes)
    reg, reg_path = reports[4], paths[4]
    assert reg['cold_cases'] == 1801 and reg['retained_cases'] == 1273 and len(reg['results']) == 3074
    assert reg['debug_sha256'] == digest(ROOT/'target/debug/grid-convex-distance.exe') and reg['release_sha256'] == digest(ROOT/'target/release/grid-convex-distance.exe')
    assert reg['native_sha256'] == digest(ROOT/'local/behavior/triangle-mesh-outer-distance-regression.exe')
    assert reg['outer_native_sha256'] == digest(outer.NATIVE_SEQUENCE)
    assert reg['script_sha256'] == digest(ROOT/'tools/replay_triangle_mesh_outer_regression.py')
    assert reg['cold_reference_sha256'] == digest(outer.COLD) and reg['retained_reference_sha256'] == digest(outer.RETAINED)
    refs = {k:json.loads(v.read_text()) for k, v in [('cold', outer.COLD), ('retained', outer.RETAINED)]}
    for row in reg['results']:
        kind, i = row['kind'], row['index']; reference = outer.COLD if kind == 'cold' else outer.RETAINED
        old_row = refs[kind]['results'][i]; inp = reference.parent/f'{i:04}-input.bin'; assert digest(inp) == row['input_sha256'] == old_row['input_sha256']
        original = reference.parent/f'{i:04}-original.bin'; repeat = reference.parent/f'{i:04}-repeat.bin'
        assert digest(original) == digest(repeat) == row['reference_sha256'] == old_row['output_sha256']
        expected = distance.projection(original.read_bytes()) if kind == 'cold' else sequence.projection(original.read_bytes(), inp.read_bytes())
        for name, sha in row['outputs'].items():
            target = reg_path.parent/f'{kind}-{i:04}-{name}.bin'; assert digest(target) == sha
            if name == 'native':
                assert sha == old_row['output_sha256']
                e = row['executions'][name]; assert e['exit'] == 0 and not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
            else: assert target.read_bytes() == expected
            complete += 1
    assert reg['guard_count'] == len(reg['guards']) == 48
    for g in reg['guards']:
        for profile, e in g['executions'].items():
            assert 0 < e['exit'] < 256
            if g['label'].startswith('existing-'):
                target = reg_path.parent/f"guard-{g['kind']}-existing-{profile}.bin"; assert digest(target) == g['unchanged_sha256']
            else: assert not (reg_path.parent/f"guard-{g['kind']}-{g['label']}-{profile}.bin").exists()
        guards += 1
    assert [r['count'] for r in reg['bounds']] == [1, 64]
    for row in reg['bounds']:
        n = row['count']; inp = reg_path.parent/f'bound-{n:02}-input.bin'; assert digest(inp) == row['input_sha256']
        answers = [(reg_path.parent/f'bound-{n:02}-{name}.bin').read_bytes() for name in ['original', 'repeat', 'debug', 'release']]
        assert answers[0] == answers[1] and outer.projection(answers[0], inp.read_bytes()) == answers[2] == answers[3]
        assert digest(reg_path.parent/f'bound-{n:02}-original.bin') == row['output_sha256']
        for k, v in outer.check(answers[0], inp.read_bytes(), table).items(): assert row[k] == v
        complete += 4
    output = a.output.resolve(); assert output.is_relative_to((ROOT/'local').resolve())
    with output.open('x') as f: json.dump(dict(passed=True, complete_outputs=complete, guards=guards, totals=totals,
        current_hashes=True, rebuilt_inputs=True, decoded_triangle_bytes=True, sampling_reconstructed=True,
        reports={str(v.relative_to(ROOT)):digest(v) for v in paths}), f, indent=2)
    print(json.dumps(dict(passed=True, complete_outputs=complete, guards=guards, totals=totals)))


if __name__ == '__main__': main()
