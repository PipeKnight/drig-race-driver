"""Audit source reconstruction and every orientation/manifold/regression byte."""
import argparse
import json
import struct
from pathlib import Path

import validate_oriented_manifold as manifold
import replay_oriented_manifold_regression as regression
from validate_oriented_manifold import ROOT, digest, u


def execution(e,native):
    assert e['exit']==0 and not e['stderr']
    if native:assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']


def main():
    p=argparse.ArgumentParser()
    for name in ['primary','regression','output']:p.add_argument(name,type=Path)
    a=p.parse_args();path=a.primary.resolve();report=json.loads(path.read_text())
    old=json.loads(manifold.retained.cold.REFERENCE.read_text());assert old['passed']
    upstream,chunks=manifold.source_chunks()
    attachment=ROOT/upstream['attachment'];assert digest(attachment)==upstream['attachment_sha256']
    attached=json.loads(attachment.read_text())['results'][upstream['attachment_index']]
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source','csv']:assert digest(game/attached[key])==attached[key+'_sha256']
    assert report['passed'] and report['max_source']==245 and report['controls']==24 and report['cases']==1421 and report['profiles']==['debug','release']
    assert report['image_sha256']==digest(manifold.IMAGE) and report['native_sha256']==digest(manifold.NATIVE)
    assert report['reference_sha256']==digest(manifold.retained.cold.REFERENCE) and report['upstream_sha256']==digest(manifold.retained.cold.distance.UPSTREAM)
    assert report['script_sha256']==digest(ROOT/'tools/validate_oriented_manifold.py')
    for profile,sha in report['rust_hashes'].items():assert digest(ROOT/f'target/{profile}/grid-convex-distance.exe')==sha
    complete=0;totals={k:0 for k in report['totals']}
    for row,info in zip(report['results'],manifold.cases(),strict=True):
        source=old['results'][info['source_index']];origin=manifold.retained.cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(origin)==source['input_sha256'];raw=manifold.request(origin.read_bytes(),info,upstream,chunks);i=row['index']
        inp=path.parent/f'{i:04}-input.bin';assert inp.read_bytes()==raw and digest(inp)==row['input_sha256']
        for k,v in info.items():assert row[k]==v
        original=None
        for j,name in enumerate(['original','repeat','debug','release']):
            execution(row['executions'][j],j<2);target=path.parent/f'{i:04}-{name}.bin'
            assert digest(target)==row['output_sha256' if j<2 else 'rust_sha256']
            if j<2:
                answer=target.read_bytes();assert len(answer)==row['bytes']
                if original is None:original=answer
                else:assert answer==original
                observed=manifold.check(answer,raw)
                for k,v in observed.items():assert row[k]==v
            else:assert target.read_bytes()==manifold.projection(original,raw)
            complete+=1
        vertices,triangles=chunks[upstream['results'][info['source_index']]['chunk']]
        records=manifold.projection(original,raw)
        for step in range(info['steps']):
            ids=triangles[u(raw,44+step*64)][0];decoded=b''.join(struct.pack('<4f',*vertices[v],1.) for v in ids)
            o=4+step*manifold.retained.STRIDE+0x860;assert records[o:o+48]==decoded
        for k in totals:totals[k]+=row[k]
    assert totals==report['totals'] and totals['intervals']==49576
    assert totals['orientation_flips']>0 and totals['orientation_unchanged']>0 and totals['zero_triangle_displacements']>0
    expected_guards=dict(manifold.guards((path.parent/'0000-input.bin').read_bytes()));assert report['guard_count']==len(report['guards'])==24
    for g in report['guards']:
        label=g['label']
        if label.startswith('existing-'):assert digest(path.parent/f'guard-{label}.bin')==g['unchanged_sha256']
        else:
            inp=path.parent/f'guard-{label}-input.bin';assert inp.read_bytes()==expected_guards[label] and digest(inp)==g['input_sha256'] and g['no_output']
            assert all(not (path.parent/f'guard-{label}-{name}.bin').exists() for name in ['original','debug','release'])
        assert all(0<e['exit']<256 for e in g['executions'])
    rp=a.regression.resolve();reg=json.loads(rp.read_text());assert reg['passed'] and reg['image_sha256']==digest(manifold.IMAGE)
    assert reg['rust_hashes']==report['rust_hashes'] and reg['script_sha256']==digest(ROOT/'tools/replay_oriented_manifold_regression.py')
    expected=[]
    for spec,(kind,reference,native,previous,selected) in zip(reg['specs'],regression.specs(),strict=True):
        assert spec['kind']==kind and spec['reference_sha256']==digest(reference) and spec['native_sha256']==digest(native)
        assert spec['selected']==[r['index'] for r in selected]
        expected.extend((kind,reference,r) for r in selected)
    regression_complete=0
    for row,(kind,reference,source) in zip(reg['results'],expected,strict=True):
        i=source['index'];assert row['kind']==kind and row['index']==i
        inp=reference.parent/f'{i:04}-input.bin';original=reference.parent/f'{i:04}-original.bin'
        assert digest(inp)==row['input_sha256']==source['input_sha256'] and digest(original)==row['reference_sha256']==source['output_sha256']
        answer=original.read_bytes()
        for name,sha in row['outputs'].items():
            execution(row['executions'][name],name=='native');target=rp.parent/f'{kind}-{i:04}-{name}.bin'
            assert digest(target)==sha and target.read_bytes()==(answer if name=='native' else regression.projection(kind,answer,inp.read_bytes()));regression_complete+=1
    previous=json.loads(regression.RETAINED.read_text());known={g['label']:g for g in previous['guards']};assert reg['guard_count']==len(reg['guards'])==25
    for g in reg['guards']:
        label=g['label']
        if label.startswith('existing-'):assert digest(rp.parent/f'guard-{label}.bin')==g['unchanged_sha256']
        else:
            inp=regression.RETAINED.parent/f'guard-{label}-input.bin';assert digest(inp)==known[label]['input_sha256']==g['input_sha256'] and g['no_output']
            assert all(not (rp.parent/f'guard-{label}-{name}.bin').exists() for name in ['native','debug','release'])
        assert all(0<e['exit']<256 for e in g['executions'].values())
    output=a.output.resolve();assert output.is_relative_to((ROOT/'local').resolve()) and not output.exists()
    result=dict(passed=True,primary_complete_files=complete,primary_guards=report['guard_count'],totals=totals,
                regression_native_cases=len(expected),regression_complete_files=regression_complete,regression_guards=reg['guard_count'],
                primary_sha256=digest(path),regression_sha256=digest(rp),script_sha256=digest(Path(__file__)))
    output.write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)


if __name__=='__main__':main()
