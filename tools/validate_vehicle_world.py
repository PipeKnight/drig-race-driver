"""Repeated whole car-manager -> selected serial physical world, owned buffers.

Private cold configuration precedes the first manager call. All later car,
physical, contact pool, group and solver state is retained without resets.
"""
import argparse,json,random,re,struct
from pathlib import Path
import validate_wheel_physical_handoff as hand
from validate_main_contact_response import uget,uput,fget,fput
w=hand.w;b=w.b;PO=hand.PO;TAIL=w.RECORD;RECORD=TAIL+3500
NATIVE=b.ROOT/'local/behavior/vehicle-world-probe.exe'

def configure(row,index,steps):
    r=bytearray(row);delta=fget(r,4)
    assert delta>0
    wire=bytearray(3500);uput(wire,0,steps)
    struct.pack_into('<11f',wire,4,delta,0,-9.8100004196167,0,0,.01,.2,.125,.8,.2,.2)
    uput(wire,48,4);struct.pack_into('<4f',wire,52,.01,1,0,.8)
    struct.pack_into('<13f',wire,68,0,.3,.4,.01,.5,.2,2,.5,3,.75,20,0,.125)
    rng=random.Random(2008200+index)
    struct.pack_into('<768f',wire,120,*(rng.uniform(-2,2) for _ in range(768)))
    struct.pack_into('<II',wire,3192,0xffffffff,0xffffffff)
    wire[3268:3272]=r[PO+0x3f4:PO+0x3f8];wire[3272:3336]=r[PO+0x150:PO+0x190]
    return bytes(r+wire)

def prior_rows(profile):
    root=b.ROOT/f'local/behavior/wheel-car-manager-{profile}-20261011-final-01'
    produced,evidence=hand.prior_rows(profile)
    rows=[r for r in produced if fget(r,4)>0]
    for path in sorted(root.glob('*-original-input.bin')):
        if path.name.startswith('guard-'):continue
        data=path.read_bytes()
        if data[:4]!=b'TF02':continue
        assert len(data)==8+uget(data,4)*w.RECORD
        outputs=[path.with_name(path.name.replace('-original-input','-'+kind+'-output')) for kind in ['original','repeat','rust']]
        raw=[q.read_bytes() for q in outputs]
        assert all(len(x)==len(data) for x in raw)
        evidence.extend(dict(path=str(q.relative_to(b.ROOT)),sha256=b.digest(q)) for q in [path,*outputs])
        for i in range(uget(data,4)):
            r=data[8+i*w.RECORD:8+(i+1)*w.RECORD]
            if fget(r,4)>0:
                assert all(raw[0][8+i*w.RECORD+a:8+i*w.RECORD+z]==x[8+i*w.RECORD+a:8+i*w.RECORD+z] for x in raw[1:] for a,z in [(0,b.VIEW),(b.MATERIAL,w.RECORD)])
                rows.append(hand.seed(r,len(rows)))
    # Preserve the known positive producing owner and sample the other source owners.
    rows=[rows[0]]+[rows[1+i*(len(rows)-2)//23] for i in range(24)]
    return rows,evidence

def compare(out,profile,rows,prefix,full):
    answers={};runs={}
    for kind in ['original','repeat','rust']:
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin'
        source.write_bytes(b.request(rows,b'TF03'))
        exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
        runs[kind]=b.execute(exe,args,out,f'{prefix}-{kind}')
        if kind!='rust':
            assert 'code_unchanged=1 driver_globals_restored=1' in runs[kind]['stdout']
            assert 'common_delta=1 authoritative_physical_feedback=1 no_cold_reset=1' in runs[kind]['stdout']
            metric=re.search(r'vehicle_world_steps=(\d+) contacts=(\d+) repeated_contacts=(\d+) empty_steps=(\d+)',runs[kind]['stdout'])
            assert metric and int(metric[1])==sum(uget(r,TAIL) for r in rows)
            runs[kind]['activity']=dict(zip(['world_calls','produced_contacts','repeated_contacts','empty_steps'],map(int,metric.groups())))
        raw=target.read_bytes();assert len(raw)==8+len(rows)*RECORD
        answers[kind]=[raw[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(rows))]
    differences=[];ranges=[(0,b.VIEW),(b.MATERIAL,RECORD)]
    for i in range(len(rows)):
        original,repeat,rust=[answers[k][i] for k in ['original','repeat','rust']]
        for a,z in ranges:
            assert original[a:z]==repeat[a:z],('native repeat',i,a,z)
            differences.extend(dict(case=i,offset=hex(o),original=original[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(a,z,4) if original[o:o+4]!=rust[o:o+4])
    report=dict(cases=len(rows),world_calls=sum(uget(r,TAIL) for r in rows),repeat_exact=True,differences=differences,executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert not differences,dict(count=len(differences),first=differences[:24])
    return report,answers

def semantics(rows,answers,seed_count,steps):
    checks={key:True for key in ['pool_consumed','world_lifecycle','active_selector_and_heads','manager_history','query_retention','world_configuration_retention','physical_ownership','source_shape_tokens','authoritative_velocity_feedback']}
    counts=dict(history_pairs=0,velocity_changes=0,pose_changes=0,manager_changes=0,large_impacts=0)
    for old,row in zip(rows,answers,strict=True):
        n=uget(row,TAIL);world=struct.unpack_from('<11I',row,TAIL+3368);life=struct.unpack_from('<6I',row,TAIL+3412)
        checks['pool_consumed'] &= uget(row,w.m.RECORD+4)==0
        checks['world_lifecycle'] &= life==(0,uget(row,4),0,0,0,0)
        selector=n%2;checks['active_selector_and_heads'] &= world[:4]==(1,1,1,1) and world[5:7]==(selector,1-selector) and world[7:]==((0,1,0,1) if selector else (1,0,1,0))
        checks['manager_history'] &= row[w.at(0xb40):w.at(0xb80)]==row[w.at(0x15e0):w.at(0x1620)] and row[w.at(0xb80):w.at(0xba0)]==row[w.at(0x15c0):w.at(0x15e0)]
        checks['query_retention'] &= row[w.job.OWNER_RECORD:w.m.RECORD]==old[w.job.OWNER_RECORD:w.m.RECORD]
        checks['world_configuration_retention'] &= row[TAIL:TAIL+120]==old[TAIL:TAIL+120]
        checks['physical_ownership'] &= all(uget(row,PO+o)==0 for o in [0x328,0x32c,0x334,0x338,0x33c,0x340]) and uget(row,PO+0x400)==selector
        for j in range(4):
            c=row[w.m.RECORD+8+j*128:w.m.RECORD+8+(j+1)*128]
            if uget(c,4):checks['source_shape_tokens'] &= uget(c,4)==1 and uget(c,8)==0 and uget(c,0x28)==1 and 2<=uget(c,0x2c)<=513 and c[0x70] in [0,1] and c[0x71:0x74]==bytes([0,1,1])
            counts['large_impacts']+=abs(fget(row,w.at(0x1ac8+j*0x150)))>1_000_000
        counts['velocity_changes']+=row[PO+0x210:PO+0x230]!=old[PO+0x210:PO+0x230]
        counts['pose_changes']+=row[PO+0x190:PO+0x210]!=old[PO+0x190:PO+0x210]
        counts['manager_changes']+=row[TAIL+120:TAIL+3368]!=old[TAIL+120:TAIL+3368]
    for owner in range(seed_count):
        history={n:answers[owner*len(steps)+j] for j,n in enumerate(steps)}
        for n,row in history.items():
            if n-1 not in history:continue
            previous=history[n-1];counts['history_pairs']+=1
            for po,co in [(0x220,0xb20),(0x210,0xb30)]:
                expected=bytearray(previous[PO+po:PO+po+16])
                if all(abs(fget(expected,i*4))<struct.unpack('<f',struct.pack('<I',0x29e12e13))[0] for i in range(3)):expected[:12]=bytes(12)
                checks['authoritative_velocity_feedback'] &= row[w.at(co):w.at(co)+16]==expected
    checks.update({key:counts[key]>0 for key in ['history_pairs','velocity_changes','pose_changes','manager_changes','large_impacts']})
    return checks,counts

def guards(out,profile,first,full):
    cases=[]
    for name,o,value in [('zero_steps',TAIL,0),('excess_steps',TAIL,65),('delta_mismatch',TAIL+4,0x3f000000),('nan_gravity',TAIL+12,0x7fc00000),('correction_iterations',TAIL+48,33),('nan_row',TAIL+120,0x7fc00000),('foreign_cache',TAIL+3192,1),('mixed_cache',TAIL+3192,0),('bad_end',TAIL+3252,17),('nan_mass',TAIL+3268,0x7fc00000),('nan_workspace',TAIL+3336,0x7fc00000),('world_not_cold',TAIL+3368,1),('unfinished_phase',TAIL+3412,1),('row_reference',TAIL+3436,1),('pool_used',w.m.RECORD+4,1),('foreign_physical',PO+0x328,1),('foreign_center',PO+0x334,1),('deferred_publication',PO+0x414,0x100)]:
        row=bytearray(first);uput(row,o,value);cases.append((name,b.request([row],b'TF03')))
    valid=b.request([first],b'TF03');cases.extend([('bad_tag',b'NOPE'+valid[4:]),('empty',b'TF03'+bytes(4)),('truncated',valid[:-1]),('trailing',valid+b'!')])
    result=[]
    for name,data in cases:
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(data)
            exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
            args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
            run=b.execute(exe,args,out,f'guard-{name}-{kind}',True)
            passed=(run['exit']==1 if kind=='rust' else 0<run['exit']<80 and not run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout']) and not target.exists()
            result.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(result,indent=2));assert all(x['passed'] for x in result),[x for x in result if not x['passed']]
    return result

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--steps',type=int,nargs='+',default=[1,2,3,4,7,8,15,16,31,32,63,64]);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    for path,expected in [(b.REFERENCE,'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7'),(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251')]:assert b.digest(path)==expected
    seeds,evidence=prior_rows(a.profile)
    rows=[configure(r,i,n) for i,r in enumerate(seeds) for n in a.steps];full=b.grip.mechanics_fixture(out)
    reports=[];all_answers=[]
    for start in range(0,len(rows),16):
        report,answers=compare(out,a.profile,rows[start:start+16],f'combined-{start}',full);reports.append(report)
        all_answers.extend(answers['original'])
    checks,activity=semantics(rows,all_answers,len(seeds),a.steps)
    guard_results=guards(out,a.profile,rows[0],full)
    native_activity={key:sum(r['executions']['original']['activity'][key] for r in reports) for key in ['world_calls','produced_contacts','repeated_contacts','empty_steps']};activity.update(native_activity)
    checks.update(dict(repeated_contacts=activity['repeated_contacts']>0,empty_steps=activity['empty_steps']>0))
    result=dict(profile=a.profile,cases=len(rows),world_calls=sum(r['world_calls'] for r in reports),observable_bytes=len(rows)*(b.VIEW+RECORD-b.MATERIAL),passed=all(checks.values()),checks=checks,activity=activity,guards=len(guard_results),reports=reports,upstream=evidence,executables={str(q.relative_to(b.ROOT)):b.digest(q) for q in [NATIVE,b.ROOT/f'target/{a.profile}/grid-vehicle-force.exe']})
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k not in ['reports','upstream']}));assert result['passed']
if __name__=='__main__':main()
