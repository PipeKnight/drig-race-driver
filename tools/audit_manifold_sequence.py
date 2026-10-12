"""Audit every retained arena, source triangle, report hash and regression."""
import argparse
import json
import struct
from pathlib import Path

import validate_manifold_sequence as sequence
import replay_manifold_sequence_regression as regression
import validate_manifold_turns as turns
from validate_manifold_sequence import ROOT, digest, u


def execution(e, native):
    assert e['exit']==0 and not e['stderr']
    if native:assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']


def main():
    p=argparse.ArgumentParser()
    for name in ['debug','release','regression','output']:p.add_argument(name,type=Path)
    p.add_argument('--turns',type=Path,required=True)
    a=p.parse_args();paths=[a.debug.resolve(),a.release.resolve()];reports=[json.loads(x.read_text()) for x in paths]
    previous=json.loads(sequence.cold.REFERENCE.read_text());assert previous['passed']
    upstream=json.loads(sequence.cold.distance.UPSTREAM.read_text());assert upstream['passed']
    track=Path(upstream['source_path']);assert digest(track)==upstream['source_sha256']
    attachment=ROOT/upstream['attachment'];assert digest(attachment)==upstream['attachment_sha256']
    attached=json.loads(attachment.read_text())['results'][upstream['attachment_index']]
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source','csv']:assert digest(game/attached[key])==attached[key+'_sha256']
    chunks={name:(v,t) for name,v,t,_,_ in sequence.cold.geometry.chunks(track.read_bytes())}
    expected=list(sequence.cases(previous));assert len(expected)==2377
    complete=guards=intervals=merged=0
    for path,report,profile in zip(paths,reports,['debug','release'],strict=True):
        assert report['passed'] and report['profile']==profile and report['cases']==2377 and report['max_base']==1225 and report['controls']==24
        assert report['reference_sha256']==digest(sequence.cold.REFERENCE) and report['image_sha256']==digest(sequence.IMAGE)
        assert report['native_sha256']==digest(sequence.NATIVE) and report['rust_sha256']==digest(ROOT/f'target/{profile}/grid-convex-distance.exe')
        assert report['script_sha256']==digest(ROOT/'tools/validate_manifold_sequence.py')
        totals={k:0 for k in report['totals']}
        for row,info in zip(report['results'],expected,strict=True):
            source=previous['results'][info['source_index']];origin=sequence.cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
            assert digest(origin)==source['input_sha256'];raw=sequence.request(origin.read_bytes(),info)
            i=row['index'];inp=path.parent/f'{i:04}-input.bin';assert inp.read_bytes()==raw and digest(inp)==row['input_sha256']
            for k,v in info.items():assert row[k]==v
            for e in row['executions'][:2]:execution(e,True)
            execution(row['executions'][2],False)
            for name in ['original','repeat']:
                target=path.parent/f'{i:04}-{name}.bin';assert digest(target)==row['output_sha256'] and target.stat().st_size==row['bytes']
                answer=target.read_bytes();observed=sequence.check(answer,raw)
                for k,v in observed.items():assert row[k]==v
                complete+=1
            target=path.parent/f'{i:04}-rust.bin';assert digest(target)==row['rust_sha256'] and target.read_bytes()==sequence.projection(answer,raw);complete+=1
            values=sequence.projection(answer,raw)
            # Separate archive decoder checks every triangle, including neighbor
            # successive-index selections and repetitions; no native output supplies Rust input.
            vertices,triangles=chunks[upstream['results'][info['source_index']]['chunk']]
            for step in range(info['steps']):
                index=u(raw,44+step*48);ids=triangles[index][0]
                decoded=b''.join(struct.pack('<4f',*vertices[v],1.) for v in ids)
                offset=4+step*sequence.STRIDE+0x860
                assert values[offset:offset+48]==decoded
            for k in totals:totals[k]+=row[k]
        assert totals==report['totals'] and totals['intervals']==55880 and totals['depth_merges']>0 and totals['preserved_records']>0
        intervals+=totals['intervals'];merged+=totals['depth_merges']
        assert len(report['guards'])==report['guard_count']==24
        expected_guards=dict(sequence.guards((path.parent/'0000-input.bin').read_bytes()))
        for g in report['guards']:
            label=g['label']
            if label.startswith('existing-'):assert digest(path.parent/f'guard-{label}.bin')==g['unchanged_sha256']
            else:
                assert digest(path.parent/f'guard-{label}-input.bin')==g['input_sha256'] and g['no_output']
                assert (path.parent/f'guard-{label}-input.bin').read_bytes()==expected_guards[label]
                assert all(not (path.parent/f'guard-{label}-{name}.bin').exists() for name in ['original','rust'])
            assert all(0<e['exit']<256 for e in g['executions']);guards+=1
    for row in reports[0]['results']:
        i=row['index'];assert (paths[0].parent/f'{i:04}-original.bin').read_bytes()==(paths[1].parent/f'{i:04}-original.bin').read_bytes()
    rp=a.regression.resolve();reg=json.loads(rp.read_text());assert reg['passed']
    assert reg['image_sha256']==digest(sequence.IMAGE) and reg['script_sha256']==digest(ROOT/'tools/replay_manifold_sequence_regression.py')
    for profile in ['debug','release']:assert reg[profile+'_sha256']==digest(ROOT/f'target/{profile}/grid-convex-distance.exe')
    expected_rows=[]
    for spec,(kind,reference,native,old,selected) in zip(reg['specs'],regression.selections(),strict=True):
        assert spec['kind']==kind and spec['reference_sha256']==digest(reference) and spec['native_sha256']==digest(native)
        assert spec['selected']==[r['index'] for r in selected]
        expected_rows.extend((kind,reference,r) for r in selected)
    regression_complete=0
    for row,(kind,reference,source) in zip(reg['results'],expected_rows,strict=True):
        i=source['index'];assert row['kind']==kind and row['index']==i
        inp=reference.parent/f'{i:04}-input.bin';original=reference.parent/f'{i:04}-original.bin'
        assert digest(inp)==source['input_sha256']==row['input_sha256'] and digest(original)==source['output_sha256']==row['reference_sha256']
        expected=original.read_bytes()
        for name,sha in row['outputs'].items():
            execution(row['executions'][name],name=='native');target=rp.parent/f'{kind}-{i:04}-{name}.bin'
            assert digest(target)==sha and target.read_bytes()==(expected if name=='native' else regression.old.projection(kind,expected,inp.read_bytes()))
            regression_complete+=1
    tp=a.turns.resolve();tr=json.loads(tp.read_text());assert tr['passed'] and tr['cases']==288
    assert tr['reference_sha256']==digest(sequence.cold.REFERENCE) and tr['native_sha256']==digest(sequence.NATIVE) and tr['image_sha256']==digest(sequence.IMAGE)
    assert tr['script_sha256']==digest(ROOT/'tools/validate_manifold_turns.py') and tr['sequence_script_sha256']==digest(ROOT/'tools/validate_manifold_sequence.py')
    for profile in ['debug','release']:assert tr[profile+'_sha256']==digest(ROOT/f'target/{profile}/grid-convex-distance.exe')
    turn_complete=0;turn_totals={k:0 for k in tr['totals']}
    for row,info in zip(tr['results'],turns.cases(),strict=True):
        source=previous['results'][info['source_index']];origin=sequence.cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(origin)==source['input_sha256'];raw=turns.request(origin.read_bytes(),info);i=row['index']
        inp=tp.parent/f'{i:04}-input.bin';assert inp.read_bytes()==raw and digest(inp)==row['input_sha256']
        for k,v in info.items():assert row[k]==v
        for j,name in enumerate(['original','repeat','debug','release']):
            execution(row['executions'][j],j<2);target=tp.parent/f'{i:04}-{name}.bin'
            assert digest(target)==row['output_sha256' if j<2 else 'rust_sha256']
            if j<2:
                answer=target.read_bytes();observed=sequence.check(answer,raw)
                for k,v in observed.items():assert row[k]==v
            else:assert target.read_bytes()==sequence.projection(answer,raw)
            turn_complete+=1
        for k in turn_totals:turn_totals[k]+=row[k]
    assert turn_totals==tr['totals'] and turn_totals['preserved_records']>0
    output=a.output.resolve();assert output.is_relative_to((ROOT/'local').resolve()) and not output.exists()
    result=dict(passed=True,primary_complete_files=complete,guards=guards,intervals=intervals,depth_merges=merged,
                regression_native_cases=len(expected_rows),regression_complete_files=regression_complete,
                turn_complete_files=turn_complete,turn_intervals=turn_totals['intervals'],turn_totals=turn_totals,
                reports=[dict(path=str(x.relative_to(ROOT)),sha256=digest(x)) for x in [*paths,rp,tp]],script_sha256=digest(Path(__file__)))
    output.write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)


if __name__=='__main__':main()
