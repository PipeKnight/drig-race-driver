"""Audit every original/repeat/Rust distance output and legacy regeneration."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_mesh_distance as distance

ROOT,digest=distance.ROOT,distance.digest


def main():
    p=argparse.ArgumentParser()
    p.add_argument('reference',type=Path)
    p.add_argument('debug_replay',type=Path)
    p.add_argument('release',type=Path)
    p.add_argument('regression',type=Path)
    p.add_argument('output',type=Path)
    p.add_argument('--release-replay',type=Path)
    a=p.parse_args()
    paths=[a.reference.resolve(),a.debug_replay.resolve(),a.release.resolve(),a.regression.resolve()]
    r,debug,release,legacy=[json.loads(v.read_text()) for v in paths]
    release_replay_path=a.release_replay.resolve() if a.release_replay else None
    release_replay=json.loads(release_replay_path.read_text()) if release_replay_path else None
    assert all(v['passed'] for v in [r,debug,release,legacy])
    upstream=ROOT/r['upstream'];assert digest(upstream)==r['upstream_sha256']==release['upstream_sha256']==legacy['upstream_sha256']
    old=json.loads(upstream.read_text());assert digest(distance.IMAGE)==old['image_sha256']==r['image_sha256']==release['image_sha256']
    assert digest(distance.NATIVE)==r['native_sha256']==debug['native_sha256']==release['native_sha256']
    assert digest(distance.REGRESSION)==legacy['native_sha256']
    assert digest(ROOT/'target/debug/grid-convex-distance.exe')==debug['rust_sha256']
    assert digest(ROOT/'target/release/grid-convex-distance.exe')==(release_replay or release)['rust_sha256']
    if release_replay:
        assert release_replay['passed'] and release_replay['cases']==1801
        assert release_replay['reference_sha256']==digest(paths[2]) and release_replay['native_sha256']==r['native_sha256']
    assert digest(paths[0])==debug['reference_sha256']
    assert digest(ROOT/'tools/validate_triangle_mesh_distance.py')==r['script_sha256']==release['script_sha256']==legacy['script_sha256']
    assert digest(ROOT/'tools/replay_triangle_mesh_distance.py')==debug['script_sha256']
    assert r['cases']==debug['cases']==release['cases']==1801 and r['base_cases']==release['base_cases']==1225
    attachment=ROOT/old['attachment'];assert digest(attachment)==old['attachment_sha256']
    attached=json.loads(attachment.read_text())['results'][old['attachment_index']]
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for k in ['source','csv']:assert digest(game/attached[k])==attached[k+'_sha256']
    assert digest(Path(old['source_path']))==old['source_sha256']
    complete=0;counts={};flags={};witnesses=0;byte_count=0
    for row,rr,dr in zip(r['results'],release['results'],debug['results'],strict=True):
        i=row['index'];assert i==rr['index']==dr['index']
        previous=old['results'][row['source_index']]
        source=upstream.parent/f"{row['source_index']:04}-input.bin";assert digest(source)==previous['input_sha256']
        expected=distance.request(source.read_bytes(),previous['triangle'],row['direction_input'],row['margin'])
        originals=[]
        for report,path,current in [(r,paths[0],row),(release,paths[2],rr)]:
            for e in current['executions'][:2]:
                assert e['exit']==0 and not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']
            inp=path.parent/f'{i:04}-input.bin';assert inp.read_bytes()==expected and digest(inp)==current['input_sha256']
            for name in ['original','repeat']:
                target=path.parent/f'{i:04}-{name}.bin';assert digest(target)==current['output_sha256'] and target.stat().st_size==current['bytes']
                answer=target.read_bytes();observed=distance.check(answer,expected)
                for k,v in observed.items():assert current[k]==v
                originals.append(answer);complete+=1
            target=path.parent/f'{i:04}-rust.bin';assert digest(target)==current['rust_sha256']
            assert target.read_bytes()==distance.projection(originals[-1]);complete+=1
        assert all(b==originals[0] for b in originals)
        target=paths[1].parent/f'{i:04}-rust.bin';assert digest(target)==dr['output_sha256'] and dr['input_sha256']==row['input_sha256']
        assert target.read_bytes()==distance.projection(originals[0]);complete+=1
        if release_replay:
            replayed=release_replay['results'][i];target=release_replay_path.parent/f'{i:04}-rust.bin'
            assert replayed['index']==i and replayed['input_sha256']==row['input_sha256'] and digest(target)==replayed['output_sha256']
            assert target.read_bytes()==distance.projection(originals[0]);complete+=1
        counts[row['simplex_count']]=counts.get(row['simplex_count'],0)+1
        f=str(row['flags']);flags[f]=flags.get(f,0)+1;witnesses+=row['witnesses'];byte_count+=row['bytes']
    assert r['observable_bytes']==release['observable_bytes']==byte_count
    assert r['witness_cases']==release['witness_cases']==witnesses
    guards=0
    guarded=[(r,paths[0]),(release,paths[2]),(debug,paths[1])]
    if release_replay:guarded.append((release_replay,release_replay_path))
    for report,path in guarded:
        assert report['guard_count']==len(report['guards'])
        for g in report['guards']:
            executions=g.get('executions',[g.get('execution')]);assert all(0<e['exit']<256 for e in executions)
            if report is r or report is release:
                if not g['label'].startswith('existing-') or g['label']=='existing-original':
                    assert not executions[0]['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0' in executions[0]['stdout']
            if g['label'].startswith('existing-'):
                target=path.parent/f"guard-{g['label']}.bin";assert target.read_bytes()==b'preserve existing evidence' and digest(target)==g['unchanged_sha256']
            else:
                for name in ['original','rust']:assert not (path.parent/f"guard-{g['label']}-{name}.bin").exists()
                assert g['no_output']
            guards+=1
    assert legacy['regression_only'] and legacy['cases']==len(legacy['results'])==old['cases']==1417
    for row in legacy['results']:
        previous=old['results'][row['index']];target=paths[3].parent/f"{row['index']:04}-regression.bin"
        assert digest(target)==row['output_sha256']==previous['output_sha256'] and target.stat().st_size==row['bytes'];complete+=1
    output=a.output.resolve();assert output.is_relative_to((ROOT/'local').resolve())
    if release_replay_path:paths.append(release_replay_path)
    with output.open('x') as f:json.dump(dict(passed=True,complete_outputs=complete,guards=guards,current_hashes=True,rebuilt_inputs=True,
        simplex_counts=counts,termination_flags=flags,witness_cases=witnesses,observable_native_bytes=byte_count,
        reports={str(v.relative_to(ROOT)):digest(v) for v in paths}),f,indent=2)
    print(f'passed complete_outputs={complete} guards={guards} witness_cases={witnesses} simplex_counts={counts} flags={flags}')


if __name__=='__main__':main()
