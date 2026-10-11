"""Audit source runtime comparisons, exact complete outputs and force controls."""
import argparse,ast,json
from pathlib import Path
import validate_vehicle_handling as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',nargs=2,type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False);reports=[];bytes_checked=0
    for path in a.reports:
        r=json.loads(path.read_text());root=path.parent
        assert r['passed'] and r['source_cases']==516 and r['controls']==64 and r['carried_calls']==96 and r['calls']==676 and r['record_bytes']==92092832
        for name,sha in r['executables'].items():assert v.digest(v.ROOT/name)==sha
        for u in r['upstream']:assert v.digest(Path(u['path']))==u['sha256']
        assert len(r['source_owners'])==516 and len(r['force_controls'])==64
        for c in r['source_owners']:
            assert v.digest(Path(c['owner']))==c['owner_sha256'] and v.digest(Path(c['physical_factory']))==c['physical_factory_sha256']
        for c in r['force_controls']:assert (any(x>0 for x in c['levels']) if c['active'] else all(x==0 for x in c['levels']))
        for prefix in [*[f'initial-{i}' for i in range(0,580,32)],*[f'carry-{i}' for i in range(8)]]:
            files=[root/f'{prefix}-{kind}-output.bin' for kind in ['original','repeat','rust']];answers=[q.read_bytes() for q in files];assert answers[0]==answers[1]==answers[2]
            inputs=[root/f'{prefix}-{kind}-input.bin' for kind in ['original','repeat','rust']]
            assert inputs[0].read_bytes()==inputs[1].read_bytes()==inputs[2].read_bytes()
            for kind in ['original','repeat','rust']:
                e=json.loads((root/f'{prefix}-{kind}-execution.json').read_text());assert e['exit']==0 and not e['stderr'] and v.digest(Path(e['exe']))==e['sha256']
            bytes_checked+=len(answers[0])-8
        assert len(r['guards'])==r['guard_count']==12
        for g in r['guards']:assert g['no_output'] and g['execution']['exit']!=0 and not (root/f'guard-{g["label"]}-{g["kind"]}-output.bin').exists()
        reports.append(dict(profile=r['profile'],path=str(path),sha256=v.digest(path),calls=r['calls'],guards=r['guard_count']))
    assert {r['profile'] for r in reports}=={'debug','release'} and bytes_checked==184185664
    for name in ['validate_source_handling.py','audit_source_handling.py']:ast.parse((v.ROOT/'tools'/name).read_text())
    result=dict(passed=True,reports=reports,observable_bytes=bytes_checked,limits='Source-configured owners and physical bodies, explicit source active flags and separate authored wear controls. Whole mutable update only, driver/world excluded.')
    (out/'audit.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
if __name__=='__main__':main()
