"""Audit complete manifold outputs, rebuilt source requests and regressions."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_manifold as manifold
import replay_triangle_manifold_regression as regression
from validate_triangle_manifold import ROOT, digest


def execution(e, native):
    assert e['exit']==0 and not e['stderr']
    if native:assert 'code_unchanged=1 globals_restored=1 code_replacements=0' in e['stdout']


def main():
    p=argparse.ArgumentParser()
    for name in ['debug','release','regression','output']:p.add_argument(name,type=Path)
    a=p.parse_args();paths=[a.debug.resolve(),a.release.resolve()];reports=[json.loads(x.read_text()) for x in paths]
    old=json.loads(manifold.REFERENCE.read_text());assert old['passed']
    upstream=json.loads(manifold.distance.UPSTREAM.read_text());assert upstream['passed']
    track=Path(upstream['source_path']);assert digest(track)==upstream['source_sha256']
    attachment=ROOT/upstream['attachment'];assert digest(attachment)==upstream['attachment_sha256']
    attached=json.loads(attachment.read_text())['results'][upstream['attachment_index']]
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source','csv']:assert digest(game/attached[key])==attached[key+'_sha256']
    chunks={name:(vertices,triangles) for name,vertices,triangles,_,_ in manifold.geometry.chunks(track.read_bytes())}
    expected_cases=list(manifold.cases(old));assert len(expected_cases)==2425
    complete=guards=contacts=flipped=positive=negative=native_bytes=0
    for row_pair,(info,controls) in zip(zip(reports[0]['results'],reports[1]['results'],strict=True),expected_cases,strict=True):
        source=old['results'][info['source_index']];inp=manifold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(inp)==source['input_sha256'];raw=manifold.request(inp.read_bytes(),**controls)
        originals=[]
        for profile,report,row,path in zip(['debug','release'],reports,row_pair,paths,strict=True):
            assert report['passed'] and report['profile']==profile and report['cases']==2425 and report['max_base']==1225 and report['controls']==24
            assert report['reference_sha256']==digest(manifold.REFERENCE) and report['upstream_sha256']==digest(manifold.distance.UPSTREAM)
            assert report['image_sha256']==digest(manifold.IMAGE) and report['native_sha256']==digest(manifold.NATIVE)
            assert report['rust_sha256']==digest(ROOT/f'target/{profile}/grid-convex-distance.exe')
            assert report['script_sha256']==digest(ROOT/'tools/validate_triangle_manifold.py')
            for k,v in info.items():assert row[k]==v
            i=row['index'];actual_input=path.parent/f'{i:04}-input.bin';assert actual_input.read_bytes()==raw and digest(actual_input)==row['input_sha256']
            for e in row['executions'][:2]:execution(e,True)
            execution(row['executions'][2],False)
            for name in ['original','repeat']:
                target=path.parent/f'{i:04}-{name}.bin';assert digest(target)==row['output_sha256'] and target.stat().st_size==row['bytes']
                answer=target.read_bytes();observed=manifold.check(answer,raw)
                for k,v in observed.items():assert row[k]==v
                complete+=1
            target=path.parent/f'{i:04}-rust.bin';assert digest(target)==row['rust_sha256'] and target.read_bytes()==manifold.projection(answer);complete+=1
            originals.append(answer)
            # Separate source reader must reproduce every selected triangle.
            source_index=info['source_index'];chunk_name=upstream['results'][source_index]['chunk']
            vertices,triangles=chunks[chunk_name];index=struct.unpack_from('<I',raw,40)[0]
            ids=triangles[index][0]
            decoded=b''.join(struct.pack('<4f',*vertices[v],1.) for v in ids)
            assert answer[-0x220+0x60:-0x220+0x90]==decoded
        assert originals[0]==originals[1]
        contacts+=row_pair[0]['contacts'];flipped+=row_pair[0]['flipped'];positive+=row_pair[0]['positive'];negative+=row_pair[0]['negative'];native_bytes+=row_pair[0]['bytes']
    for path,report in zip(paths,reports,strict=True):
        assert report['contacts']==contacts and report['flipped']==flipped and report['positive']==positive and report['negative']==negative and report['observable_bytes']==native_bytes
        assert len(report['guards'])==report['guard_count']==26
        for g in report['guards']:
            label=g['label']
            if label.startswith('existing-'):
                assert digest(path.parent/f'guard-{label}.bin')==g['unchanged_sha256']
            else:
                target=path.parent/f'guard-{label}-input.bin';assert digest(target)==g['input_sha256']
                assert all(not (path.parent/f'guard-{label}-{name}.bin').exists() for name in ['original','rust']) and g['no_output']
            assert all(0<e['exit']<256 for e in g['executions']);guards+=1
    rp=a.regression.resolve();rr=json.loads(rp.read_text());assert rr['passed'] and len(rr['results'])==9990
    assert rr['debug_sha256']==digest(ROOT/'target/debug/grid-convex-distance.exe') and rr['release_sha256']==digest(ROOT/'target/release/grid-convex-distance.exe')
    assert rr['script_sha256']==digest(ROOT/'tools/replay_triangle_manifold_regression.py') and rr['image_sha256']==digest(manifold.IMAGE)
    by_kind={r['kind']:r for r in rr['specs']};previous={}
    for kind,path,native_name in regression.SPECS:
        assert by_kind[kind]['reference_sha256']==digest(path) and by_kind[kind]['native_sha256']==digest(ROOT/'local/behavior'/native_name)
        previous[kind]=(path,json.loads(path.read_text()))
    for row in rr['results']:
        kind=row['kind'];path,old_report=previous[kind];i=row['index'];source=old_report['results'][i]
        inp=path.parent/f'{i:04}-input.bin';assert digest(inp)==row['input_sha256']==source['input_sha256']
        original=path.parent/f'{i:04}-original.bin';repeat=path.parent/f'{i:04}-repeat.bin'
        assert digest(original)==digest(repeat)==row['reference_sha256']==source['output_sha256']
        native=rp.parent/f'{kind}-{i:04}-native.bin';assert digest(native)==row['outputs']['native']==digest(original)
        execution(row['executions']['native'],True);complete+=1
        if kind not in ['packed','manifold']:
            expected=regression.projection(kind,original.read_bytes(),inp.read_bytes())
            for profile in ['debug','release']:
                target=rp.parent/f'{kind}-{i:04}-{profile}.bin';assert digest(target)==row['outputs'][profile] and target.read_bytes()==expected
                execution(row['executions'][profile],False);complete+=1
    assert len(rr['guards'])==rr['guard_count']
    for g in rr['guards']:
        kind,label=g['kind'],g['label'];path,old_report=previous[kind]
        if label.startswith('existing-'):
            assert digest(rp.parent/f'guard-{kind}-{label}.bin')==g['unchanged_sha256']
        else:
            inp=path.parent/(f'guard-{label}-input.bin' if not label.startswith('table-') else '0000-input.bin')
            assert digest(inp)==g['input_sha256'] and g['no_output']
            if g['sampling_sha256']:assert digest(path.parent/f'guard-{label}.bin')==g['sampling_sha256']
            for name in g['executions']:assert not (rp.parent/f'guard-{kind}-{label}-{name}.bin').exists()
        assert all(0<e['exit']<256 for e in g['executions'].values());guards+=1
    result=dict(passed=True,complete_outputs=complete,guards=guards,cases=2425,contacts=contacts,flipped=flipped,positive=positive,negative=negative,native_bytes=native_bytes,
                current_hashes=True,rebuilt_inputs=True,decoded_triangle_bytes=True,regression_native_cases=9990,
                reports={str(x.relative_to(ROOT)):digest(x) for x in [*paths,rp]})
    output=a.output.resolve();assert output.is_relative_to((ROOT/'local').resolve())
    with output.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)
    print(json.dumps(result,indent=2))


if __name__=='__main__':main()
