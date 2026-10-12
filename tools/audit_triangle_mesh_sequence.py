"""Audit complete retained-query outputs, reconstructed sources and current builds."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_mesh_sequence as sequence
from validate_triangle_mesh_sequence import ROOT, digest, distance


def main():
    p = argparse.ArgumentParser()
    p.add_argument('debug', type=Path)
    p.add_argument('release', type=Path)
    p.add_argument('regression', type=Path)
    p.add_argument('cold_debug', type=Path)
    p.add_argument('cold_release', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    paths = [getattr(a, k).resolve() for k in ['debug', 'release', 'regression', 'cold_debug', 'cold_release']]
    debug, release, reg, cold_debug, cold_release = [json.loads(v.read_text()) for v in paths]
    assert all(v['passed'] for v in [debug, release, reg, cold_debug, cold_release])
    assert digest(sequence.NATIVE) == debug['native_sha256'] == release['native_sha256'] == reg['native_sha256']
    assert digest(sequence.COLD) == reg['cold_native_sha256']
    assert digest(distance.IMAGE) == debug['image_sha256'] == release['image_sha256']
    assert digest(ROOT/'target/debug/grid-convex-distance.exe') == debug['rust_sha256'] == cold_debug['rust_sha256'] == reg['debug_sha256']
    assert digest(ROOT/'target/release/grid-convex-distance.exe') == release['rust_sha256'] == cold_release['rust_sha256'] == reg['release_sha256']
    assert digest(ROOT/'tools/validate_triangle_mesh_sequence.py') == debug['script_sha256'] == release['script_sha256']
    assert digest(ROOT/'tools/replay_triangle_mesh_sequence.py') == reg['script_sha256']
    assert digest(paths[0]) == reg['sequence_reference_sha256']
    upstream = ROOT/debug['upstream']; old = json.loads(upstream.read_text())
    assert old['passed'] and digest(upstream) == debug['upstream_sha256'] == release['upstream_sha256']
    attachment = ROOT/old['attachment']; assert digest(attachment) == old['attachment_sha256']
    attached = json.loads(attachment.read_text())['results'][old['attachment_index']]
    game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for k in ['source', 'csv']: assert digest(game/attached[k]) == attached[k+'_sha256']
    track = Path(old['source_path']); assert digest(track) == old['source_sha256']
    chunks = {name:(positions, triangles) for name, positions, triangles, _, _ in distance.geometry.chunks(track.read_bytes())}
    complete, queries, witnesses, changes, guards = 0, 0, 0, 0, 0
    assert debug['cases'] == release['cases'] == len(debug['results']) == len(release['results']) == 1273
    assert debug['base_cases'] == release['base_cases'] == 1225
    flags, counts = {}, {}
    for dr, rr in zip(debug['results'], release['results'], strict=True):
        assert dr['index'] == rr['index'] and dr['source_index'] == rr['source_index'] and dr['margin'] == rr['margin']
        i = dr['index']; source_row = old['results'][dr['source_index']]
        inp = upstream.parent/f"{source_row['index']:04}-input.bin"; assert digest(inp) == source_row['input_sha256']
        source = inp.read_bytes()
        raw = sequence.request(source, sequence.controls(source_row, source), dr['margin'])
        originals = []
        for report, row, path in [(debug, dr, paths[0]), (release, rr, paths[1])]:
            request = path.parent/f'{i:04}-input.bin'; assert request.read_bytes() == raw and digest(request) == row['input_sha256']
            for e in row['executions'][:2]:
                assert e['exit'] == 0 and not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
            for name in ['original', 'repeat']:
                target = path.parent/f'{i:04}-{name}.bin'; assert digest(target) == row['output_sha256'] and target.stat().st_size == row['bytes']
                answer = target.read_bytes(); observed = sequence.check(answer, raw)
                for k, v in observed.items(): assert row[k] == v
                originals.append(answer); complete += 1
            target = path.parent/f'{i:04}-rust.bin'; assert digest(target) == row['rust_sha256']
            assert target.read_bytes() == sequence.projection(originals[-1], raw); complete += 1
        assert all(v == originals[0] for v in originals)
        records = sequence.projection(originals[0], raw)[4:]
        positions, triangles = chunks[source_row['chunk']]
        for step in range(dr['queries']):
            ti = struct.unpack_from('<I', raw, 44+step*24)[0]
            points = b''.join(struct.pack('<4f', *positions[v], 1.) for v in triangles[ti][0])
            assert records[step*448+0x180:step*448+0x1b0] == points
        queries += dr['queries']; witnesses += dr['witness_queries']; changes += dr['cache_changes']
        for k, v in dr['flags'].items(): flags[k] = flags.get(k, 0)+v
        for k, v in dr['simplex_counts'].items(): counts[k] = counts.get(k, 0)+v
    assert queries == debug['queries'] == release['queries'] and witnesses == debug['witness_queries'] == release['witness_queries']
    assert changes == debug['cache_changes'] == release['cache_changes']
    for report, path in [(debug, paths[0]), (release, paths[1])]:
        assert report['guard_count'] == len(report['guards']) == 15
        for g in report['guards']:
            assert all(0 < e['exit'] < 256 for e in g['executions'])
            if not g['label'].startswith('existing-') or g['label'] == 'existing-original':
                e = g['executions'][0]; assert not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
            if g['label'].startswith('existing-'):
                target = path.parent/f"guard-{g['label']}.bin"; assert digest(target) == g['unchanged_sha256'] and target.read_bytes() == b'preserve existing evidence'
            else:
                inp = path.parent/f"guard-{g['label']}-input.bin"; assert digest(inp) == g['input_sha256']
                assert all(not (path.parent/f"guard-{g['label']}-{n}.bin").exists() for n in ['original', 'rust'])
            guards += 1
    cold_path = ROOT/reg['cold_reference']; cold = json.loads(cold_path.read_text())
    assert digest(cold_path) == reg['cold_reference_sha256'] and reg['cold_cases'] == len(reg['cold_results']) == 1801
    for profile, replay, path in [('debug', cold_debug, paths[3]), ('release', cold_release, paths[4])]:
        ref = ROOT/replay['reference']; reference = json.loads(ref.read_text())
        assert digest(ref) == replay['reference_sha256'] and replay['cases'] == 1801
        assert replay['native_sha256'] == digest(distance.NATIVE) and replay['image_sha256'] == digest(distance.IMAGE)
        for row, native_row in zip(replay['results'], reference['results'], strict=True):
            assert row['index'] == native_row['index']
            inp = ref.parent/f"{row['index']:04}-input.bin"; original = ref.parent/f"{row['index']:04}-original.bin"
            repeat = ref.parent/f"{row['index']:04}-repeat.bin"; target = path.parent/f"{row['index']:04}-rust.bin"
            assert digest(inp) == row['input_sha256'] == native_row['input_sha256']
            assert digest(original) == digest(repeat) == native_row['output_sha256']
            assert digest(target) == row['output_sha256'] and target.read_bytes() == distance.projection(original.read_bytes())
            complete += 1
        assert replay['guard_count'] == len(replay['guards']) == 32
        for g in replay['guards']:
            assert 0 < g['execution']['exit'] < 256
            if g['label'] == 'existing-rust':
                target = path.parent/'guard-existing-rust.bin'; assert digest(target) == g['unchanged_sha256']
            else:
                assert not (path.parent/f"guard-{g['label']}-rust.bin").exists() and g['no_output']
            guards += 1
    for row, old_row in zip(reg['cold_results'], cold['results'], strict=True):
        assert row['index'] == old_row['index'] and row['input_sha256'] == old_row['input_sha256']
        target = paths[2].parent/f"{row['index']:04}-cold.bin"
        assert digest(target) == row['output_sha256'] == old_row['output_sha256'] and target.stat().st_size == row['bytes']
        complete += 1
    effects = []
    for row in reg['effects']:
        i, step = row['sequence'], row['step']; prefix = paths[2].parent/f'effect-{i:04}-{step:02}'
        inp = Path(str(prefix)+'-input.bin'); original = Path(str(prefix)+'-native.bin'); rust = Path(str(prefix)+'-rust.bin')
        assert digest(inp) == row['input_sha256'] and digest(original) == row['native_sha256'] and digest(rust) == row['rust_sha256']
        distance.check(original.read_bytes(), inp.read_bytes()); c = distance.projection(original.read_bytes()); assert c == rust.read_bytes()
        seq_input = paths[0].parent/f'{i:04}-input.bin'; seq_original = paths[0].parent/f'{i:04}-original.bin'
        r = sequence.projection(seq_original.read_bytes(), seq_input.read_bytes())[4+step*448:4+(step+1)*448]
        observed = dict(scratch_differs=c[:0x120] != r[:0x120], cache_differs=c[0x164:0x180] != r[0x164:0x180], result_differs=c[0x128:0x160] != r[0x128:0x160])
        for k, v in observed.items(): assert row[k] == v
        effects.append(observed); complete += 2
    assert sum(r['scratch_differs'] for r in effects) == reg['scratch_effects']
    assert sum(r['cache_differs'] for r in effects) == reg['cache_effects']
    assert sum(r['result_differs'] for r in effects) == reg['result_effects']
    assert [r['count'] for r in reg['bounds']] == [1, 64]
    for row in reg['bounds']:
        n = row['count']; inp = paths[2].parent/f'bound-{n:02}-input.bin'; assert digest(inp) == row['input_sha256']
        answers = [(paths[2].parent/f'bound-{n:02}-{name}.bin').read_bytes() for name in ['original', 'repeat', 'debug', 'release']]
        assert answers[0] == answers[1] and sequence.projection(answers[0], inp.read_bytes()) == answers[2] == answers[3]
        assert digest(paths[2].parent/f'bound-{n:02}-original.bin') == row['output_sha256']
        for k, v in sequence.check(answers[0], inp.read_bytes()).items(): assert row[k] == v
        complete += 4
    output = a.output.resolve(); assert output.is_relative_to((ROOT/'local').resolve())
    with output.open('x') as f:
        json.dump(dict(passed=True, complete_outputs=complete, queries=queries, witness_queries=witnesses, cache_changes=changes,
                       guards=guards, current_hashes=True, rebuilt_inputs=True, decoded_triangle_bytes=True,
                       flags=flags, simplex_counts=counts, reports={str(v.relative_to(ROOT)):digest(v) for v in paths}), f, indent=2)
    print(f'passed complete_outputs={complete} queries={queries} witness_queries={witnesses} guards={guards} cache_changes={changes}')


if __name__ == '__main__': main()
