"""Read-only reconstruction/current-hash audit of owned callback projection."""
import argparse
import json
from pathlib import Path
import validate_owned_deformation_callback as validation
import validate_deformation_layout as layout


def audit(path):
    path=path.resolve();assert path.is_relative_to((layout.ROOT/'local').resolve());out=path.parent;report=json.loads(path.read_text())
    rows,bodies,extras,identities=validation.cases();assert identities==report['identities'] and len(rows)==report['cases']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    assert (out/'input.bin').read_bytes()==validation.pack(rows,bodies,extras)
    original=(out/'original.bin').read_bytes();assert len(original)-8==report['bytes']
    assert json.loads((out/'execution.json').read_text())==report['runs'] and not any(json.loads((out/'differences.json').read_text()).values())
    for kind,run in report['runs'].items():
        exe=validation.PROBE if kind in ['original','repeat'] else layout.ROOT/f"target/{report['profile']}/grid-{'serial-callback' if kind=='typed' else 'owned-deformation-callback'}.exe"
        assert layout.digest(exe)==run['executable_sha256'] and run['exit']==0 and not run['stderr'] and (out/f'{kind}.bin').read_bytes()==original
        if kind in ['original','repeat']:assert 'code_unchanged=1 real_imports=7 original_rand=1' in run['stdout']
    good,bad=validation.guard_cases(rows,bodies,extras);bad={n:(b,o) for n,b,o in bad};outcomes=0
    assert json.loads((out/'guards.json').read_text())==report['guards']
    for guard in report['guards']:
        name=guard['name']
        if 'executions' in guard:
            payload,owned_only=bad.pop(name);assert guard['owned_only']==owned_only and (out/f'guard-{name}-input.bin').read_bytes()==payload
            assert set(guard['executions'])==({'owned'} if owned_only else {'original','typed','owned'})
            for kind,run in guard['executions'].items():
                assert 0<run['exit']<256 and 'fault=' not in run['stderr']
                if kind!='original':assert not run['output_exists'] and not (out/f'guard-{name}-{kind}.bin').exists()
                outcomes+=1
        else:
            kind=name.split('-')[1];assert guard['preserved'] and guard['execution']['exit']==(9 if kind=='original' else 1)
            assert (out/f'guard-existing-{kind}-input.bin').read_bytes()==good and (out/f'guard-existing-{kind}.bin').read_bytes()==b'owned sentinel';outcomes+=1
    assert not bad
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),cases=len(rows),bytes=report['bytes'],complete_files=len(report['files']),identities=len(identities),guard_cases=len(report['guards']),guard_outcomes=outcomes,coverage=report['coverage'])


def main():
    p=argparse.ArgumentParser();p.add_argument('reports',type=Path,nargs='+');p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    result=[audit(path) for path in args.reports];target=args.output.resolve();assert target.is_relative_to((layout.ROOT/'local').resolve())
    with target.open('x') as f:json.dump(dict(reports=result),f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
