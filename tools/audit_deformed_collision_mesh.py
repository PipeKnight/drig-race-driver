"""Read-only reconstruction and current-file audit of active mesh/shape proofs."""
import argparse
import json
from pathlib import Path
import validate_deformed_collision_mesh as validation
import validate_deformation_layout as layout


def audit(path):
    path=path.resolve();assert path.is_relative_to((layout.ROOT/'local').resolve());report=json.loads(path.read_text());out=path.parent
    rows,identities=validation.cases();assert identities==report['identities'] and len(rows)==report['cases']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    shape=report['shape'];probe=layout.ROOT/'local/behavior'/('deformed-mesh-shape-probe.exe' if shape else 'deformed-collision-mesh-probe.exe')
    rust=layout.ROOT/f"target/{report['profile']}"/('grid-mesh-shape.exe' if shape else 'grid-collision-mesh.exe')
    total=0
    for i,(row,item) in enumerate(zip(rows,report['reports'])):
        assert row['name']==item['name'] and not item['differences'] and item['repeat_exact']
        assert (out/f'{i}-input.bin').read_bytes()==validation.request(row,shape)
        original=(out/f'{i}-original.bin').read_bytes();assert original==(out/f'{i}-repeat.bin').read_bytes()==(out/f'{i}-rust.bin').read_bytes() and len(original)==item['bytes'];total+=len(original)
        for kind,run in item['executions'].items():assert run['exit']==0 and layout.digest(rust if kind=='rust' else probe)==run['executable_sha256']
    assert total==report['bytes'] and json.loads((out/'guards.json').read_text())==report['guards']
    outcomes=0
    for guard in report['guards']:
        if 'executions' in guard:
            for kind,run in guard['executions'].items():
                assert run['exit']==(1 if kind=='rust' else guard['expected_original_exit']) and not run['output_exists'] and not (out/f"guard-{guard['name']}-{kind}.bin").exists();outcomes+=1
        else:
            kind=guard['name'].split('-')[1];assert guard['preserved'] and guard['execution']['exit']==(19 if kind=='original' else 1)
            assert (out/f"{guard['name']}.bin").read_bytes()==b'owned sentinel';outcomes+=1
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),cases=len(rows),bytes=total,complete_files=len(report['files']),identities=len(identities),guard_cases=len(report['guards']),guard_outcomes=outcomes)


def main():
    p=argparse.ArgumentParser();p.add_argument('reports',type=Path,nargs='+');p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    result=[audit(path) for path in args.reports];target=args.output.resolve();assert target.is_relative_to((layout.ROOT/'local').resolve())
    with target.open('x') as f:json.dump(dict(reports=result),f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
