"""Selected real contact append -> carried whole world -> real pool consumption.

Fresh manager outputs supply physical owners. Subsequent local-point/normal/
penetration requests and counts are authored controls, not geometry observations.
"""
import argparse,copy,json,struct,subprocess
from pathlib import Path
import validate_wheel_physical_handoff as w
import validate_shaped_physical_frame as s
from validate_main_contact_response import uget,uput,fget
from validate_contact_bias import compare,execute
from validate_steering_control import digest
ROOT=w.ROOT

def authored(row,index):
    base=w.physical_case(row,index);template=base['contacts'][0][1]
    requests=[];slots=[]
    for j,(x,z) in enumerate([(-.8,1.2),(.8,1.2),(-.8,-1.2),(.8,-1.2)]):
        token=2+(index*17+j*11)%512
        requests.append([x,-.3,z,1.,0.,1.,0.,0.,.001+j*.002,token])
        c=bytearray(template);uput(c,0x2c,token);slots.append((2,c))
    base.update(contacts=slots,requests=requests,counts=[[4,0,1,3,2,0,4,1][(k+index)%8] for k in range(64)],carry_seed=index)
    count=[2,4,8,16,32,64][index%6]
    return [dict(copy.deepcopy(base),steps=steps) for steps in [1,count-1,count]]

def semantics(cases,outputs):
    checks=s.lifecycle_checks(cases,outputs)
    checks.update(history=True,dirty_membership_retained=True,selector_alternates=True,active_contacts_constructed=True,inactive_slots_retained=True,contact_identity=True,normal_flags_retained=True,first_point=True,finite_motion=True)
    by_seed={(c['carry_seed'],c['steps']):o for c,o in zip(cases,outputs)}
    rounded=lambda v:struct.unpack('<f',struct.pack('<f',v))[0]
    activity=dict(world_steps=0,carried_steps=0,produced_contacts=0,reused_contacts=0,empty_intervals=0,checked_predecessors=0,point_checks=0,inactive_slot_checks=0,pose_changes=0,velocity_changes=0)
    for c,o in zip(cases,outputs):
        steps=c['steps'];counts=c['counts'][:steps];used=counts[-1]
        activity['world_steps']+=steps;activity['carried_steps']+=steps-1
        activity['produced_contacts']+=sum(counts);activity['reused_contacts']+=sum(counts[1:]);activity['empty_intervals']+=counts.count(0)
        first=by_seed[c['carry_seed'],1];checks['dirty_membership_retained'] &= o['world'][4]==first['world'][4] and o['bodies'][0][0x2f0:0x2f8]==first['bodies'][0][0x2f0:0x2f8] and o['bodies'][0][0x412]==1
        checks['selector_alternates'] &= o['world'][5]==steps%2 and o['world'][6]==1-steps%2
        before=c['bodies'][0]['raw'] if steps==1 else by_seed.get((c['carry_seed'],steps-1),{}).get('bodies',[None])[0]
        if before is not None:
            activity['checked_predecessors']+=1
            checks['history'] &= before[0x190:0x210]==o['bodies'][0][0x270:0x2f0]
            oldslots=[r for _,r in c['contacts']] if steps==1 else by_seed[c['carry_seed'],steps-1]['contacts']
            for j in range(used,4):
                activity['inactive_slot_checks']+=1;checks['inactive_slots_retained'] &= o['contacts'][j]==oldslots[j]
            for j in range(used):
                r=c['requests'][j];local=list(map(rounded,r[:4]))
                for k in range(4):
                    world=rounded(rounded(rounded(local[3]*fget(before,0x200+4*k))+rounded(local[1]*fget(before,0x1e0+4*k)))+rounded(rounded(local[0]*fget(before,0x1d0+4*k))+rounded(local[2]*fget(before,0x1f0+4*k))))
                    relative=rounded(world-fget(before,0x1c0+4*k));advanced=rounded(rounded(rounded(c['parameters'][0])*fget(before,0x220+4*k))+fget(before,0x1c0+4*k))
                    checks['first_point'] &= struct.pack('<f',advanced+relative)==o['contacts'][j][0x50+4*k:0x54+4*k];activity['point_checks']+=1
        for j,r in enumerate(o['contacts'][:used]):
            checks['contact_identity'] &= (uget(r,4),uget(r,8),uget(r,0x28),uget(r,0x2c),uget(r,0x30),uget(r,0x34))==(0,0xffffffff,1,c['requests'][j][9],0,0)
            checks['active_contacts_constructed'] &= uget(r,0x14)>=uget(r,0x18)==1 and uget(r,0x1c)==0 and uget(r,0x20)==0xffffffff
            checks['normal_flags_retained'] &= r[0xc:0x10]==bytes([0,0,0,1]) and r[0x71:0x74]==bytes([0,1,1]) and r[0x60:0x70]==struct.pack('<4f',*c['requests'][j][4:8])
        checks['finite_motion'] &= all(abs(fget(o['bodies'][0],offset))<float('inf') for offset in range(0x190,0x230,4))
        activity['pose_changes']+=o['bodies'][0][0x190:0x210]!=c['bodies'][0]['raw'][0x190:0x210]
        activity['velocity_changes']+=o['bodies'][0][0x210:0x230]!=c['bodies'][0]['raw'][0x210:0x230]
    return checks,activity

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--limit',type=int,default=0);a=p.parse_args()
    out=s.private(a.output);out.mkdir(exist_ok=False);rows,evidence=w.prior_rows(a.profile)
    if a.limit:rows=rows[:a.limit]
    producer=out/'producer';producer.mkdir();fixture=w.w.b.grip.mechanics_fixture(producer);reports=[];branches={k:[] for k in ['original','repeat','rust']}
    for start in range(0,len(rows),32):
        result,answers=w.w.m.compare(producer,a.profile,{k:rows[start:start+32] for k in branches},f'producer-{start}',fixture,True);reports.append(result)
        for k in branches:branches[k].extend(answers[k])
    s.MAGIC=s.g.MAGIC=0x36535747;cases={k:[c for i,row in enumerate(rs) for c in authored(row,i)] for k,rs in branches.items()}
    probe=ROOT/'local/behavior/reproduced-physical-world-probe.exe';rust=ROOT/f'target/{a.profile}/grid-physical-tick.exe';reference=w.w.b.REFERENCE;original=None;runs={};outputs={}
    for kind in branches:
        source=out/f'{kind}-input.bin';source.write_bytes(s.pack(cases[kind]));command=[str(rust)] if kind=='rust' else [str(probe),str(reference)]
        raw,run=execute(command,source,out/f'{kind}-output.bin');original=raw if original is None else original;run.update(compare(raw,original));runs[kind]=run;outputs[kind]=s.decode(raw,cases[kind],True,True)
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout'] and 'original_contact_production=1 repeated_pool_consumption=1' in run['stdout']
    checks,activity=semantics(cases['original'],outputs['original'])
    checks['full_physical_owner_from_manager']=all(c['bodies'][0]['raw'][:0x334]+c['bodies'][0]['raw'][0x338:]==row[w.PO:w.PO+0x334]+row[w.PO+0x338:w.PO+0x424] for k in branches for c,row in zip(cases[k],[r for r in branches[k] for _ in range(3)],strict=True))
    base=copy.deepcopy(cases['original'][0]);good=s.pack([base]);bad={'truncated':good[:-1],'trailing':good+b'!','empty':good[:4]+bytes(4)+good[8:]}
    mutations=[('zero_steps',lambda c:c.update(steps=0)),('excess_steps',lambda c:c.update(steps=65,counts=[0]*65)),('excess_count',lambda c:c['counts'].__setitem__(0,5)),('foreign_request_shape',lambda c:c['requests'][0].__setitem__(9,514)),('null_request_shape',lambda c:c['requests'][0].__setitem__(9,0)),('nan_normal',lambda c:c['requests'][0].__setitem__(4,float('nan'))),('nan_inactive_request',lambda c:(c.update(counts=[0]*64),c['requests'][3].__setitem__(4,float('nan')))),('nan_local',lambda c:c['requests'][0].__setitem__(0,float('nan'))),('nonaffine_local',lambda c:c['requests'][0].__setitem__(3,0)),('nan_penetration',lambda c:c['requests'][0].__setitem__(8,float('nan'))),('shape_flags',lambda c:c['shapes'].__setitem__(0,(1,0,1,0))),('deferred_publication',lambda c:c['bodies'][0]['raw'].__setitem__(0x415,1)),('insufficient_capacity',lambda c:c.update(capacity=0)),('nan_basis',lambda c:uput(c['bodies'][0]['raw'],0x190,0x7fc00000))]
    for name,change in mutations:c=copy.deepcopy(base);change(c);bad[name]=s.pack([c])
    guards={}
    for name,data in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(data);guards[name]={}
        for kind,command in [('original',[str(probe),str(reference)]),('rust',[str(rust)])]:
            target=out/f'guard-{name}-{kind}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards[name][kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=0<run.returncode<256 and not target.exists() and 'physical_tick_fault=' not in run.stderr and (kind=='rust' or not run.stderr and 'code_unchanged=1 no_imports=1' in run.stdout))
    report=dict(profile=a.profile,whole_world=True,carried=True,reproduced=True,cases=len(cases['original']),bytes=len(original)-8,producer_calls=sum(r['calls'] for r in reports),producer_reports=reports,upstream=evidence,runs=runs,checks=checks,activity=activity,guards=guards,executables={str(q.relative_to(ROOT)):digest(q) for q in [probe,rust,ROOT/f'target/{a.profile}/grid-vehicle-force.exe',w.w.NATIVE]},scope='Fresh complete manager owners, authored local contact requests/counts -> unchanged B15E70/B0EE80 before every whole serial AD8D70/A727A0 step on retained buffers. Selected car/static metadata/shared material, 0..4 requests, no geometric query or repeated vehicle job/source initialization/driving claim.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values()) and all(r['passed'] for pair in guards.values() for r in pair.values());(out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['runs','producer_reports','upstream','executables','guards']}));assert report['passed']
if __name__=='__main__':main()
