"""Audit saved world comparisons against their current executable provenance."""
import argparse, ast, json
from pathlib import Path
from capture_reference_spawn import ANALYSIS, private
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]

def audit(path):
    report=json.loads(path.read_text());assert report['passed']
    assert report.get('analysis_sha256',ANALYSIS)==ANALYSIS
    checks=report['checks'];assert checks and all(v is True for v in checks.values())
    for run in report['runs'].values():assert run['passed']
    for pair in report.get('guards',{}).values():
        for run in pair.values():assert run['passed']
    if 'executables' in report:
        for name,expected in report['executables'].items():assert digest(ROOT/name)==expected
    else:
        name='carried-physical-world' if report.get('carried') else ('whole-physical-world' if report.get('whole_world') else ('shaped-physical-tick' if report['advance'] else 'shaped-physical-frame'))
        assert digest(ROOT/f'local/behavior/{name}-probe.exe')==report['probe_sha256']
        assert digest(ROOT/f'target/{report["profile"]}/grid-physical-tick.exe')==report['rust_sha256']
    for item in report.get('upstream',[]):assert digest(ROOT/item['path'])==item['sha256']
    for item in report.get('producer_reports',[]):
        assert item['repeat_exact'] and item['differing_words']==0 and not item['differences']
        for run in item['executions']:
            assert run['exit']==0 and not run['stderr'] and digest(Path(run['exe']))==run['sha256']
    return dict(path=str(path.relative_to(ROOT)),sha256=digest(path),cases=report['cases'],bytes=report['bytes'],checks=len(checks),guards=sum(len(p) for p in report.get('guards',{}).values()))

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',type=Path,nargs='+');a=p.parse_args()
    assert digest(ROOT/'local/compatibility/grid-unpacked.exe')==ANALYSIS
    rows=[audit(q.resolve()) for q in a.reports]
    scripts=['validate_shaped_physical_frame.py','validate_wheel_physical_handoff.py','audit_physical_world.py']
    for name in scripts:ast.parse((ROOT/'tools'/name).read_text())
    result=dict(passed=True,reports=rows,report_count=len(rows),checks=sum(r['checks'] for r in rows),guards=sum(r['guards'] for r in rows),scripts=scripts)
    private(a.output).write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='reports'}))
if __name__=='__main__':main()
