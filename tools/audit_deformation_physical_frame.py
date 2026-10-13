"""Reconstruct whole physical-frame preparation proof and guard files."""
import argparse
import collections
import json
from pathlib import Path
import validate_deformation_physical_frame as validation
import validate_deformation_layout as layout


def audit(path):
    path=path.resolve();assert path.is_relative_to((layout.ROOT/'local').resolve());out=path.parent;report=json.loads(path.read_text())
    rows,identities=validation.cases();assert identities==report['identities'] and len(rows)==report['cases']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    rust=layout.ROOT/f"target/{report['profile']}/grid-deformation-physical-frame.exe";coverage=collections.Counter();total=0;seen=0
    for item in report['reports']:
        start=item['start'];batch=rows[start:start+32];assert start==seen and item['cases']==len(batch);seen+=len(batch)
        assert (out/f'{start}-input.bin').read_bytes()==validation.request(batch)
        answer=(out/f'{start}-original.bin').read_bytes();assert answer==(out/f'{start}-repeat.bin').read_bytes()==(out/f'{start}-rust.bin').read_bytes()
        assert item['bytes']==len(answer)-8 and item['repeat_exact'] and not item['differences']
        assert json.loads((out/f'{start}-execution.json').read_text())==item['executions']
        for kind,run in item['executions'].items():
            assert run['exit']==0 and not run['stderr'] and layout.digest(rust if kind=='rust' else validation.PROBE)==run['executable_sha256']
            if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
        for row,(result,d) in zip(batch,validation.decode(answer,batch)):validation.check_answer(row,result,d,coverage)
        total+=len(answer)-8
    assert seen==len(rows) and total==report['bytes'] and dict(coverage)==report['coverage']
    good,bad=validation.guard_payloads(rows);assert json.loads((out/'guards.json').read_text())==report['guards'];outcomes=0
    for guard in report['guards']:
        name=guard['name']
        if 'executions' in guard:
            assert (out/f'guard-{name}-input.bin').read_bytes()==bad.pop(name)
            for kind,run in guard['executions'].items():
                assert (run['exit']==1 if kind=='rust' else 0<run['exit']<256) and 'fault=' not in run['stderr']
                assert not run['output_exists'] and not (out/f'guard-{name}-{kind}.bin').exists();outcomes+=1
        else:
            kind=name.split('-')[1];assert guard['preserved'] and guard['execution']['exit']==(1 if kind=='rust' else 6)
            assert (out/f'guard-existing-{kind}-input.bin').read_bytes()==good and (out/f'guard-existing-{kind}.bin').read_bytes()==b'owned sentinel';outcomes+=1
    assert not bad
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),cases=len(rows),bytes=total,complete_files=len(report['files']),identities=len(identities),guard_cases=len(report['guards']),guard_outcomes=outcomes,coverage=dict(coverage))


def main():
    p=argparse.ArgumentParser();p.add_argument('reports',type=Path,nargs='+');p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    result=[audit(path) for path in args.reports];target=args.output.resolve();assert target.is_relative_to((layout.ROOT/'local').resolve())
    with target.open('x') as f:json.dump(dict(reports=result),f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
