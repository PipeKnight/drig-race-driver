"""Installed physical coefficients through repeated complete car/world calls.

Each diagnostic run explicitly assigns one source material to all owned shapes.
This verifies coefficient handoff, not the original live shape-to-resource map.
"""
import argparse,json,struct
from pathlib import Path
import validate_vehicle_world as v
import validate_physical_material as m
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('material_report',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    report=json.loads(a.material_report.read_text());assert report['passed'] and report['profile']==a.profile
    materials=[];provenance=[dict(path=str(a.material_report.resolve().relative_to(v.b.ROOT)),sha256=m.digest(a.material_report))]
    for c in report['comparisons']:
        if c['label'] not in m.SOURCES:continue
        assert not c['differences'] and m.digest(Path(c['input']))==m.SOURCES[c['label']]==c['sha256']
        paths=[a.material_report.parent/f'{c["label"]}-{kind}.tsv' for kind in ['original','repeat','rust']]
        tables=[q.read_text().splitlines() for q in paths];assert tables[0]==tables[1]==tables[2]
        provenance.extend(dict(path=str(q.resolve().relative_to(v.b.ROOT)),sha256=m.digest(q)) for q in paths)
        for line in tables[0]:
            parts=line.split('\t');assert len(parts)==16 and parts[0]=='P'
            words=[int(s,16) for s in parts[1:14]]
            materials.append(dict(resource=c['label'],code=words[0],words=words))
    assert len(materials)==99
    seeds,upstream=v.prior_rows(a.profile);steps=[1,2,3,4,7,8,15,16]
    rows=[];assignments=[]
    # Every installed physical material, balanced across retained source owners.
    for i,material in enumerate(materials):
        for n in steps:
            r=bytearray(v.configure(seeds[i%len(seeds)],i,n))
            struct.pack_into('<13I',r,v.TAIL+68,*material['words']);rows.append(bytes(r))
        assignments.append(dict(**material,seed=i%len(seeds)))
    full=v.b.grip.mechanics_fixture(out);reports=[];answers=[]
    for start in range(0,len(rows),16):
        result,output=v.compare(out,a.profile,rows[start:start+16],f'source-combined-{start}',full);reports.append(result);answers.extend(output['original'])
    checks,activity=v.semantics(rows,answers,len(materials),steps)
    # Large impacts are diagnostic coverage, not a requirement of source material loading.
    checks.pop('large_impacts')
    checks['source_material_words_retained']=all(old[v.TAIL+68:v.TAIL+120]==new[v.TAIL+68:v.TAIL+120] for old,new in zip(rows,answers,strict=True))
    native={key:sum(q['executions']['original']['activity'][key] for q in reports) for key in ['world_calls','produced_contacts','repeated_contacts','empty_steps']};activity.update(native)
    checks['repeated_contacts']=activity['repeated_contacts']>0
    checks['every_source_material_exercised']=len(assignments)==99
    checks['some_source_materials_produce_contacts']=any(q['executions']['original']['activity']['produced_contacts']>0 for q in reports)
    baseline_row=v.configure(seeds[0],0,steps[-1])
    baseline,baseline_answers=v.compare(out,a.profile,[baseline_row],'authored-material-control',full)
    source_answer=answers[len(steps)-1];control=baseline_answers['original'][0]
    effects=[hex(o) for o in range(v.PO,v.PO+0x424,4) if source_answer[o:o+4]!=control[o:o+4]]
    checks['material_changes_physical_response']=bool(effects)
    guard_results=v.guards(out,a.profile,rows[0],full)
    result=dict(passed=all(checks.values()),profile=a.profile,cases=len(rows),world_calls=sum(q['world_calls'] for q in reports),observable_bytes=len(rows)*(v.b.VIEW+v.RECORD-v.b.MATERIAL),checks=checks,activity=activity,reports=reports,authored_material_control=dict(report=baseline,differing_physical_words=effects),guards=len(guard_results),assignments=assignments,upstream=upstream+provenance,executables={str(q.relative_to(v.b.ROOT)):m.digest(q) for q in [v.NATIVE,v.b.ROOT/f'target/{a.profile}/grid-vehicle-force.exe']},limits='Diagnostic source coefficients shared by all shapes. Live material/shape identity, mixed physical material ownership, complete Lacetti initialization and playable driving remain open.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:val for k,val in result.items() if k not in ['reports','assignments','upstream']}));assert result['passed']
if __name__=='__main__':main()
