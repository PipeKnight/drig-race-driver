"""Audit combined-world comparisons, guards, regressions and file provenance."""
import argparse,ast,json
from pathlib import Path
from validate_steering_control import digest
ROOT=Path(__file__).resolve().parents[1]

def audit(path):
    report=json.loads(path.read_text());checks=report.get('checks',{})
    if checks:assert report['passed'] and all(v is True for v in checks.values())
    entries=report['reports']
    for r in entries:
        assert r['repeat_exact'] and not r['differences']
        executions=r['executions'];executions=executions.values() if isinstance(executions,dict) else executions
        for run in executions:
            assert run['exit']==0 and not run['stderr'] and digest(Path(run['exe']))==run['sha256']
    guards=json.loads((path.parent/'guards.json').read_text());assert len(guards)==report['guards'] and all(g['passed'] for g in guards)
    for name,sha in report['executables'].items():assert digest(ROOT/name)==sha
    for entry in report.get('upstream',[]):assert digest(ROOT/entry['path'])==entry['sha256']
    for entry in report.get('initialization_reports',[]):
        assert not entry.get('differences',[]) and entry.get('differing_words',0)==0
    return dict(path=str(path.relative_to(ROOT)),sha256=digest(path),cases=report.get('cases',report.get('calls')),checks=len(checks),guards=len(guards))

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',type=Path,nargs='+');a=p.parse_args()
    assert a.output.resolve().is_relative_to((ROOT/'local').resolve())
    assert digest(ROOT/'local/compatibility/grid-unpacked.exe')=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    results=[audit(q.resolve()) for q in a.reports]
    scripts=['validate_vehicle_world.py','audit_vehicle_world.py']
    for s in scripts:ast.parse((ROOT/'tools'/s).read_text())
    report=dict(passed=True,reports=results,checks=sum(r['checks'] for r in results),guards=sum(r['guards'] for r in results),scripts=scripts)
    a.output.write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k!='reports'}))
if __name__=='__main__':main()
