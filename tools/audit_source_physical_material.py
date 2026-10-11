"""Current binary, input, repeat, guard and source-coefficient handoff audit."""
import argparse,ast,json
from pathlib import Path
import validate_physical_material as m
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',type=Path,nargs='+');a=p.parse_args()
    assert a.output.resolve().is_relative_to((m.ROOT/'local').resolve()) and not a.output.exists()
    rows=[]
    for path in a.reports:
        d=json.loads(path.read_text());assert d['passed']
        if 'comparisons' in d:
            for q in d['comparisons']:assert not q['differences'] and m.digest(Path(q['input']))==q['sha256']
            for q in d['executions']:assert q['exit']==0 and not q['stderr'] and m.digest(Path(q['exe']))==q['sha256']
            assert all(q['passed'] for q in d['guards']);guard_count=len(d['guards'])
        else:
            assert all(d['checks'].values())
            for q in [*d['reports'],d['authored_material_control']['report']]:
                assert q['repeat_exact'] and not q['differences']
                for run in q['executions'].values():assert run['exit']==0 and not run['stderr'] and m.digest(Path(run['exe']))==run['sha256']
            for name,sha in d['executables'].items():assert m.digest(m.ROOT/name)==sha
            for q in d['upstream']:assert m.digest(m.ROOT/q['path'])==q['sha256']
            guards=json.loads((path.parent/'guards.json').read_text());guard_count=len(guards)
            assert guard_count==d['guards'] and all(q['passed'] for q in guards)
        rows.append(dict(path=str(path.resolve().relative_to(m.ROOT)),sha256=m.digest(path),guards=guard_count))
    for name,sha in m.SOURCES.items():assert m.digest(m.INSTALL/name)==sha
    scripts=['validate_physical_material.py','validate_source_vehicle_world.py','audit_source_physical_material.py','inspect_reference_outer.py']
    for name in scripts:ast.parse((m.ROOT/'tools'/name).read_text())
    result=dict(passed=True,reports=rows,guards=sum(q['guards'] for q in rows),scripts=scripts)
    a.output.write_text(json.dumps(result,indent=2));print(json.dumps(dict(passed=True,reports=len(rows),guards=result['guards'],scripts=len(scripts))))
if __name__=='__main__':main()
