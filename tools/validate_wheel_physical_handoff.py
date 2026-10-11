"""Fresh whole-manager contacts/physical owner -> whole grouped serial solving.

Cold body/pool configuration precedes production. Geometry/flags/velocity/mass
are not authored after production. Physical-stage timestep/material remain
external, so this is a stage handoff, not whole AD8D70/frame equivalence.
"""
import argparse,copy,json,random,struct
from pathlib import Path
import validate_wheel_car_manager as w
import validate_shaped_physical_frame as s
from validate_main_contact_response import uget,uput,fput,fget
from validate_contact_bias import compare,execute
from validate_steering_control import digest
ROOT=w.b.ROOT
PO=w.b.RECORD+w.job.EXTRA

def seed(row,index):
    row=bytearray(row);p=bytearray(row[PO:PO+0x424])
    # Previously absent cold physical configuration is supplied BEFORE the job.
    p[0x190:0x1d0]=p[0x1d0:0x210];p[0x230:0x240]=bytes(16)
    for o in [0x3a0,0x3a4,0x3a8,0x3ac,0x3b0,0x3b4,0x3b8,0x3bc,0x3c0,0x3c4]:fput(p,o,s.g.MAX)
    fput(p,0x3a0,0);fput(p,0x3f8,1);fput(p,0x3fc,1)
    for o in [0x110]:
        for j in range(16):fput(p,o+j*4,float(j%5==0))
    p[0x410]=index%2
    row[PO:PO+0x424]=p
    tail=bytearray(struct.pack('<II',4,0)+bytes(512))
    for j in range(4):tail[8+j*128+0xf]=1
    row[w.m.RECORD:]=tail
    return bytes(row)

def prior_rows(profile):
    root=ROOT/f'local/behavior/wheel-car-manager-{profile}-20261011-final-01'
    report=json.loads((root/'validation.json').read_text());assert report['calls']==428 and report['coverage']['produced_contacts']==155
    rows=[];evidence=[]
    for p in sorted(root.glob('*-original-output.bin')):
        if p.name.startswith('guard-'):continue
        old=p.with_name(p.name.replace('output','input'));raw=p.read_bytes();data=old.read_bytes()
        assert len(raw)==len(data)==8+uget(raw,4)*w.RECORD
        repeat=p.with_name(p.name.replace('-original-','-repeat-')).read_bytes();rust=p.with_name(p.name.replace('-original-','-rust-')).read_bytes()
        ranges=[(0,w.b.VIEW),(w.b.MATERIAL,w.RECORD)]
        for i in range(uget(raw,4)):
            at=8+i*w.RECORD;r=raw[at:at+w.RECORD];entry=data[at:at+w.RECORD]
            assert all(r[a:b]==repeat[at+a:at+b]==rust[at+a:at+b] for a,b in ranges)
            if uget(r,w.m.RECORD+4)>uget(entry,w.m.RECORD+4):rows.append(seed(entry,len(rows)))
        evidence.extend(dict(path=str(q.relative_to(ROOT)),sha256=digest(q)) for q in [p,old])
    assert len(rows)==66
    return rows,evidence

def physical_case(row,index):
    used=uget(row,w.m.RECORD+4);assert used<=4
    physical=bytearray(row[PO:PO+0x424]);uput(physical,0x334,0x1c0) # normalize owned center backlink only
    contacts=[]
    for j in range(used):
        c=bytearray(row[w.m.RECORD+8+j*128:w.m.RECORD+8+(j+1)*128])
        assert uget(c,4)==1 and uget(c,8)==0 and c[0xc]==0 and c[0xf]==1 and c[0x73]==1
        uput(c,4,0);uput(c,8,0xffffffff);uput(c,0x1c,0);uput(c,0x20,0xffffffff)
        contacts.append((2,c))
    rng=random.Random(2008196+index)
    return dict(bodies=[dict(raw=physical,properties=[fget(physical,0x3f4),*struct.unpack_from('<16f',physical,0x150)],workspace=[0.]*8)],
                materials=[[0.,.3,.4,.01,.5,.2,2.,.5,3.,.75,20.,0.,.125]],contacts=contacts,
                rows=[bytearray(struct.pack('<48f',*(rng.uniform(-2,2) for _ in range(48)))) for _ in range(16)],capacity=16,iterations=[1,2,4,8][index%4],
                parameters=[.016,0.,-9.81,0.,0.,.01,.2,.125,.8,.2,.2],ends=[0]*4,pair=[0xffffffff]*2,cache=[0.]*13,
                correction=[4,.01,1.,0.,.8],order=[0],shapes=[(0,0,1,0)]+[(1,0,0,0)]*512)

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--advance',action='store_true');p.add_argument('--whole-world',action='store_true');a=p.parse_args()
    if a.whole_world:a.advance=True
    out=s.private(a.output);out.mkdir(exist_ok=False);rows,evidence=prior_rows(a.profile);producer=out/'producer';producer.mkdir();full=w.b.grip.mechanics_fixture(producer)
    reports=[];branches={k:[] for k in ['original','repeat','rust']}
    for start in range(0,len(rows),32):
        result,answers=w.m.compare(producer,a.profile,{k:rows[start:start+32] for k in branches},f'producer-{start}',full,True);reports.append(result)
        for k in branches:branches[k].extend(answers[k])
    s.MAGIC=0x34535747 if a.whole_world else (0x34535447 if a.advance else 0x34534647);s.g.MAGIC=s.MAGIC
    cases={k:[physical_case(r,i) for i,r in enumerate(rs)] for k,rs in branches.items()}
    probe=ROOT/('local/behavior/whole-physical-world-probe.exe' if a.whole_world else f'local/behavior/shaped-physical-{"tick" if a.advance else "frame"}-probe.exe');rust=ROOT/f'target/{a.profile}/grid-physical-tick.exe';reference=w.b.REFERENCE
    answers={};runs={};baseline=None
    for kind in branches:
        source=out/f'{kind}-input.bin';source.write_bytes(s.pack(cases[kind]));command=[str(rust)] if kind=='rust' else [str(probe),str(reference)]
        raw,run=execute(command,source,out/f'{kind}-output.bin');baseline=raw if baseline is None else baseline;run.update(compare(raw,baseline));runs[kind]=run;answers[kind]=s.decode(raw,cases[kind],a.whole_world)
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
        if a.whole_world and kind!='rust':assert 'whole_serial_world=1 wrapper_pool_consumed=1 owned_workers_retained=1' in run['stdout']
    checks,activity=s.semantics(cases['original'],answers['original'],a.advance)
    checks['generated_contacts_present']=activity['contacts']>0
    checks['all_generated_contacts_enabled']=activity['enabled']==activity['contacts'] and activity['disabled']==0
    checks['all_generated_contacts_special_null_second']=activity['special']==activity['null_second']==activity['contacts']
    checks['physical_velocity_changes']=activity['velocity_changes']>0
    checks['full_physical_owner_from_manager']=all(c['bodies'][0]['raw'][:0x334]+c['bodies'][0]['raw'][0x338:]==r[PO:PO+0x334]+r[PO+0x338:PO+0x424] for k in branches for c,r in zip(cases[k],branches[k]))
    if a.whole_world:checks.update(s.lifecycle_checks(cases['original'],answers['original']))
    report=dict(profile=a.profile,advance=a.advance,cases=len(rows),bytes=len(baseline)-8,producer_calls=sum(r['calls'] for r in reports),producer_contacts=sum(r['coverage']['produced_contacts'] for r in reports),producer_reports=reports,upstream=evidence,runs=runs,checks=checks,activity=activity,executables={str(q.relative_to(ROOT)):digest(q) for q in [probe,rust,ROOT/f'target/{a.profile}/grid-vehicle-force.exe',w.NATIVE]},scope='Fresh whole car manager from cold physical/pool seeds -> generated contact records and full physical owner -> optional whole AD7040 -> whole AD6C40/B77310/B7B850. Independent branches; only identity normalization after production. Authored shared material, physical-stage delta, cold configuration before producer; not whole enclosing frame, persistent world or drivable lap.')
    if a.whole_world:report.update(whole_world=True,scope='Fresh whole manager -> generated contact pool and complete physical owner -> whole serial AD8D70 -> actual A727A0 callback-free pool consumption. Independent branches, identity normalization only after production. Authored cold configuration/material and separate physical timestep; no persistent world/whole frame/source registry/driving claim.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values());(out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['producer_reports','upstream','runs','executables']}));assert report['passed']
if __name__=='__main__':main()
