"""Rebuild packed-contact requests from source and audit complete native outputs."""
import argparse
import json
import struct
from pathlib import Path

from validate_packed_mesh_contact import ROOT, IMAGE, NATIVE, check, digest
from validate_vcqtc import chunks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('report', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    path = args.report.resolve()
    r = json.loads(path.read_text())
    assert r['passed'] and r['native_reference_only']
    assert digest(IMAGE) == r['image_sha256'] and digest(NATIVE) == r['native_sha256']
    assert digest(ROOT/'tools/validate_packed_mesh_contact.py') == r['script_sha256']
    source = Path(r['source_path'])
    assert digest(source) == r['source_sha256']
    attachment = ROOT/r['attachment']
    assert digest(attachment) == r['attachment_sha256']
    a = json.loads(attachment.read_text())['results'][r['attachment_index']]
    assert a['source'] == r'cars\lac\lac.ned'
    game = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source', 'csv']:
        assert digest(game/a[key]) == a[key+'_sha256']
    seed_path = attachment.parent/f"{r['attachment_index']:03}-original.bin"
    assert digest(seed_path) == r['seed_sha256'] == a['output_sha256']
    seed = seed_path.read_bytes()
    raw = source.read_bytes()
    sections = {}
    for i in range(struct.unpack_from('<I', raw, 8)[0]):
        w = struct.unpack_from('<8I', raw, 32+i*32)
        assert w[1] == w[3]
        name = raw[w[0]:raw.index(0, w[0])].decode()
        sections[name] = raw[w[2]:w[2]+w[3]]
    geometry = {name: (positions, triangles)
                for name, positions, triangles, nodes, refs in chunks(raw)}
    complete = 0
    for row in r['results']:
        if 'base_index' in row:
            base = r['results'][row['base_index']]
            assert 'base_index' not in base
            limits = row['contact_capacity'], row['cache_capacity'], row['candidate_capacity']
        else:
            base, limits = row, (16, 1, 4096)
        positions, triangles = geometry[base['chunk']]
        vertices = [positions[v] for v in triangles[base['triangle']][0]]
        center = [sum(v[k] for v in vertices)/3 for k in range(3)]
        center[1] += base['height']
        assert center == base['center']
        chunk = sections[base['chunk']]
        expected = struct.pack('<6I4f', 0x31304350, len(seed), len(chunk), *limits, *center, 1.)+seed+chunk
        inp = path.parent/f"{row['index']:04}-input.bin"
        assert digest(inp) == row['input_sha256'] and inp.read_bytes() == expected
        for kind in ['original', 'repeat']:
            target = path.parent/f"{row['index']:04}-{kind}.bin"
            assert digest(target) == row['output_sha256']
            data = target.read_bytes()
            assert len(data) == row['bytes']
            observed = check(data, seed, chunk, struct.unpack_from('<4f', expected, 24), *limits)
            observed = json.loads(json.dumps(observed))
            assert all(observed[k] == row[k] for k in observed)
            complete += 1
    assert r['cases'] == len(r['results'])
    assert r['contacts'] == sum(row['contacts'] for row in r['results'])
    assert r['positive_cases'] == sum(row['contacts'] > 0 for row in r['results'])
    assert r['observable_bytes'] == sum(row['bytes'] for row in r['results'])
    assert r['guard_count'] == len(r['guards']) == 25
    for guard in r['guards']:
        e = guard['execution']
        assert 0 < e['exit'] < 256 and not e['stderr']
        assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
        if guard['label'] == 'existing-valid-output':
            target = path.parent/'guard-existing-output.bin'
            assert target.read_bytes() == b'preserve existing evidence'
            assert digest(target) == guard['unchanged_sha256']
        else:
            assert guard['no_output'] and not (path.parent/f"guard-{guard['label']}-output.bin").exists()
    target = args.output.resolve()
    assert target.is_relative_to((ROOT/'local').resolve())
    with target.open('x') as handle:
        json.dump(dict(passed=True, complete_outputs=complete, rebuilt_inputs=True,
                       current_hashes=True, guards=r['guard_count'], report_sha256=digest(path)), handle, indent=2)
    print(f'passed complete_outputs={complete} guards={r["guard_count"]} rebuilt_inputs=1 current_hashes=1')


if __name__ == '__main__':
    main()
