"""Reconstruct complete deformation volume/initialization proofs, read-only."""
import argparse
import json
from pathlib import Path
import validate_deformation_layout as layout
import validate_deformation_volume as volume
import validate_deformation_initialization as initialization


def source_audit(path,report):
    import validate_source_deformation as source
    from compare_source_deformation_layout import candidates
    out=path.parent;rows,identities=source.source_rows();assert identities==report['identities']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    assert (out/'original-input.bin').read_bytes()==initialization.request(rows)
    original=(out/'original.bin').read_bytes();assert original==(out/'repeat.bin').read_bytes() and len(original)==8+len(rows)*initialization.ANSWER
    for i,row in enumerate(rows):
        stem=row['name'].split('-')[0];choices,_=candidates(stem,'highLOD');choice=[c for c in choices if c['name']=='highLOD-LOD1_no-aerial'][0]
        import struct
        expected=b'DS01'+struct.pack('<II',len(choice['selected_joints']),len(choice['excluded_joints']))+struct.pack('<8f',*choice['bounds'])+original[8+i*initialization.ANSWER:8+(i+1)*initialization.ANSWER]
        assert (out/f'{stem}-rust.bin').read_bytes()==expected==(out/f'{stem}-rust-repeat.bin').read_bytes()
        assert report['observations'][i]==dict(model=stem,complete_bytes=len(expected),selected_joints=len(choice['selected_joints']),excluded_aerials=len(choice['excluded_joints']),exact=True)
    for kind,run in report['executions'].items():
        exe=initialization.PROBE if kind in ['original','repeat'] else layout.ROOT/f"target/{report['profile']}/grid-deformation-source.exe"
        assert run['exit']==0 and layout.digest(exe)==run['executable_sha256']
    for guard in report['guards']:
        assert guard['exit']==1
        if guard['name']=='existing':assert guard['preserved'] and (out/'existing.bin').read_bytes()==b'owned sentinel'
        else:assert not guard['output_exists'] and not (out/f"guard-{guard['name']}.bin").exists()
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),source_models=len(rows),source_output_bytes=sum(v['complete_bytes'] for v in report['observations']),complete_files=len(report['files']),identities=len(identities),guards=len(report['guards']))


def audit(path):
    path=path.resolve();assert path.is_relative_to((layout.ROOT/'local').resolve())
    report=json.loads(path.read_text());out=path.parent
    assert layout.digest(layout.IMAGE)=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    if 'observations' in report:return source_audit(path,report)
    is_init='names' in report;module=initialization if is_init else volume
    binary='grid-deformation-initialization' if is_init else 'grid-deformation-volume'
    rows,identities=module.cases();assert len(rows)==report['cases'] and identities==report['identities']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    total=0
    for item in report['reports']:
        start=item['start'];batch=rows[start:start+item['cases']]
        assert (out/f'{start}-input.bin').read_bytes()==module.request(batch)
        answer=(out/f'{start}-original.bin').read_bytes()
        assert answer==(out/f'{start}-repeat.bin').read_bytes()==(out/f'{start}-rust.bin').read_bytes()
        assert len(answer)==8+len(batch)*module.ANSWER and len(answer)-8==item['bytes'] and not item['differences']
        for kind,run in item['executions'].items():
            exe=module.PROBE if kind!='rust' else layout.ROOT/f"target/{report['profile']}/{binary}.exe"
            assert run['exit']==0 and layout.digest(exe)==run['executable_sha256']
        if is_init and start==0:assert json.loads((out/'retained-comparison.json').read_text())==initialization.retained(answer)
        total+=len(answer)-8
    assert total==report['bytes'] and json.loads((out/'guards.json').read_text())==report['guards']
    outcomes=0
    for item in report['guards']:
        if 'executions' in item:
            for kind,run in item['executions'].items():
                assert run['exit']==(1 if kind=='rust' else item['expected_original_exit']) and not run['output_exists']
                assert not (out/f"guard-{item['name']}-{kind}.bin").exists();outcomes+=1
        else:
            assert item['preserved'] and item['exit'] in [1,24]
            assert (out/f"guard-{item['name']}.bin").read_bytes()==b'owned sentinel';outcomes+=1
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),cases=len(rows),bytes=total,
                complete_files=len(report['files']),identities=len(identities),current_executables_and_image=3,guard_cases=len(report['guards']),guard_outcomes=outcomes)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('reports',type=Path,nargs='+');parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();results=[audit(p) for p in args.reports];output=args.output.resolve()
    assert output.is_relative_to((layout.ROOT/'local').resolve())
    with output.open('x') as f:json.dump(dict(reports=results),f,indent=2)
    print(json.dumps(results))


if __name__=='__main__':main()
