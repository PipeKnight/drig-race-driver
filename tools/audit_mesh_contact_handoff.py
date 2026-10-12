"""Audit current handoff binaries, complete outputs and native mesh provenance."""
import argparse
import json
from pathlib import Path
import validate_mesh_contact_handoff as handoff
from validate_mesh_contact import ROOT, digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('reports', type=Path, nargs='+')
    args = p.parse_args()
    complete = seeds = guard_count = 0
    hashes = []
    for path in args.reports:
        path = path.resolve()
        r = json.loads(path.read_text())
        assert r['passed'] and r['world_points_unchanged'] and r['correction_flag_retains_main_response']
        assert digest(ROOT/'local/behavior/geometric-physical-tick-probe.exe') == r['native_sha256']
        assert digest(ROOT/f"target/{r['profile']}/grid-physical-tick.exe") == r['rust_sha256']
        assert digest(ROOT/'tools/validate_mesh_contact_handoff.py') == r['script_sha256']
        upstream = ROOT/r['upstream']
        assert digest(upstream) == r['upstream_sha256']
        producer = json.loads(upstream.read_text())
        assert producer['passed']
        if r.get('packed_complete', False):
            assert producer['full_descriptor'] and not producer['rust_generator_equivalence']
            original_path = ROOT/producer['upstream']
            assert digest(original_path) == producer['upstream_sha256']
            assert json.loads(original_path.read_text())['attachment_index'] == 48
            assert all(producer['results'][row['source_index']]['label'] == 'source' for row in r['provenance'])
        elif r.get('packed_track', False):
            assert producer['native_reference_only'] and producer['attachment_index'] == 48
        else:
            assert producer['contact_slot_constructor']
        cases = []
        for row in r['provenance']:
            old = producer['results'][row['source_index']]
            source = upstream.parent/f"{row['source_index']:04}-original.bin"
            assert digest(source) == old['output_sha256'] == row['source_sha256']
            c = handoff.fixture(source.read_bytes(), row['source_index'], r.get('packed_track', False))
            if row['kind'] == 'correction_disabled':
                c['shapes'][1] = (1, 1, 0, 0)
            elif row['kind'] == 'empty':
                c['contacts'] = []
            else:
                assert row['kind'] == 'enabled'
            cases.append(c)
        handoff.shaped.MAGIC = handoff.grouped.MAGIC = handoff.MAGIC
        request = path.parent/'input.bin'
        assert digest(request) == r['input_sha256'] and request.read_bytes() == handoff.shaped.pack(cases)
        assert r['cases'] == len(cases) and r['positive_mesh_seeds']*3 == len(cases)
        for kind in ['original', 'repeat', 'rust']:
            answer = path.parent/f'{kind}.bin'
            assert digest(answer) == r['output_sha256']
            assert len(answer.read_bytes())-8 == r['observable_bytes']
            decoded = handoff.shaped.decode(answer.read_bytes(), cases)
            assert all(before[0x50:0x70] == after[0x50:0x70]
                       for c, o in zip(cases, decoded) for (_, before), after in zip(c['contacts'], o['contacts']))
            complete += 1
        assert r['guard_count'] == len(r['guards']) == 24
        for g in r['guards']:
            assert 0 < g['exit'] < 256
            if g['label'] == 'existing':
                assert (path.parent/f"guard-existing-{g['kind']}.bin").read_bytes() == b'preserve existing evidence'
            else:
                assert g['no_output'] and not (path.parent/f"guard-{g['label']}-{g['kind']}.bin").exists()
        guard_count += r['guard_count']
        seeds += r['positive_mesh_seeds']
        hashes.append(digest(path))
    target = args.output.resolve()
    assert target.is_relative_to((ROOT/'local').resolve())
    with target.open('x') as f:
        json.dump(dict(passed=True, complete_outputs=complete, mesh_seed_uses=seeds,
                       guards=guard_count, current_hashes=True, rebuilt_inputs=True, report_hashes=hashes), f, indent=2)
    print(f'passed complete_outputs={complete} mesh_seed_uses={seeds} guards={guard_count} rebuilt_inputs=1')


if __name__ == '__main__':
    main()
