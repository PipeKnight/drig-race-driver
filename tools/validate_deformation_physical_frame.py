"""Whole B7B850/B78530/B77850/9A0C90 versus staged Rust physical frame."""
import argparse
import collections
import copy
import json
import random
import struct
import subprocess
from pathlib import Path
import validate_deformation_contact_limit as limit
import validate_deformation_layout as layout
import validate_collision_mesh as execution
import validate_impact_frame as frame
from validate_impact_step import authored
from validate_main_contact_response import fput,uput

ROOT=layout.ROOT
PROBE=ROOT/'local/behavior/deformation-physical-frame-probe.exe'


def cases():
    sources,identities=limit.cases();rows=[];rng=random.Random(2008260)
    for index,s in enumerate(sources):
        c=frame.configure(authored(rng,index),index);c['bodies']=c['bodies'][:1];raw=c['bodies'][0]['raw'];source=s['body']
        raw[0x1d0:0x230]=source[0x1d0:0x230];raw[0x190:0x1d0]=source[0x1d0:0x210]
        raw[0x1c0:0x1d0]=source[0x200:0x210];raw[0x360:0x370]=source[0x360:0x370];raw[0x3f0:0x3f4]=source[0x3f0:0x3f4]
        # Keep the frame's independent force/inertia inputs; source mass and
        # point velocities feed the callback, not a claimed live whole frame.
        count=[0,1,2,8,16,64,4,32][index%8];c['contacts']=[]
        for j in range(count):
            contact=bytearray(s['contact']);uput(contact,4,0);uput(contact,8,0);uput(contact,0x10,0);uput(contact,0x1c,0);uput(contact,0x20,0)
            for at,value in [(0x28,2),(0x2c,1),(0x30,j%4),(0x34,(j+1)%4)]:uput(contact,at,value)
            contact[0xc]=1;contact[0xd]=0;contact[0xe]=j%2;contact[0xf]=1;contact[0x70]=0;contact[0x73]=int(j%3!=2)
            fput(contact,0x3c,[s['parameters'][5],100.,.1,0.][j%4]);fput(contact,0x38,[.001,.1,.5,2.][j%4])
            c['contacts'].append((0,contact))
        c['rows']=[bytearray(struct.pack('<48f',*[rng.uniform(-2,2) for _ in range(48)])) for _ in range(max(1,count*4))];c['capacity']=len(c['rows'])
        c['iterations']=[0,1,2,4,8,16,32][index%7]
        # Explicit sleeping bypass also covers nonzero contact pools. Active
        # controls use MAX duration so the callback cannot be skipped by sleep.
        sleeping=index%17==0
        for i,value in enumerate([0.,frame.MAX,frame.MAX,frame.MAX,frame.MAX,0. if sleeping else frame.MAX]):fput(raw,0x3a0+i*4,value)
        if index%19==0:c['capacity']=0
        c['deformation']=s;rows.append(c)
    return rows,identities


def request(rows):
    data=bytearray(b'PFD1'+struct.pack('<I',len(rows)))
    for row in rows:
        data.extend(frame.pack([row])[8:]);s=row['deformation'];data.extend(s['descriptor']);data.extend(struct.pack('<23fI',*s['parameters'],s['disabled']))
    return bytes(data)


def answer_size(row):
    return 84+68+32+limit.LINKED+128*len(row['contacts'])+192*len(row['rows'])+layout.SIZE


def decode(data,rows):
    assert data[:8]==b'PFD1'+struct.pack('<I',len(rows));at=8;answers=[]
    for row in rows:
        size=answer_size(row);r=data[at:at+size];assert len(r)==size
        old=b'PFR1'+struct.pack('<I',1)+r[:-layout.SIZE]
        answers.append((frame.decode(old,[row])[0],r[-layout.SIZE:]));at+=size
    assert at==len(data);return answers


def guard_payloads(rows):
    row=rows[5];good=request([row]);guards={'magic':b'BAD!'+good[4:],'zero_count':good[:4]+bytes(4)+good[8:],'truncated':good[:-1],'trailing':good+b'x'}
    for name,mutate in [
        ('zero_delta',lambda c:c['parameters'].__setitem__(0,0.)),
        ('nan_basis',lambda c:fput(c['bodies'][0]['raw'],0x190,float('nan'))),
        ('foreign_callback',lambda c:uput(c['bodies'][0]['raw'],0x418,1)),
        ('foreign_user',lambda c:uput(c['bodies'][0]['raw'],0x420,1)),
        ('null_shape',lambda c:uput(c['contacts'][0][1],0x28,0)),
        ('dynamic_contact',lambda c:c['contacts'].__setitem__(0,(1,c['contacts'][0][1]))),
        ('nan_old_cap',lambda c:fput(c['contacts'][0][1],0x3c,float('nan'))),
        ('ring',lambda c:c['deformation'].__setitem__('descriptor',mutated(c['deformation']['descriptor'],0xa4ec,10))),
        ('dimension',lambda c:c['deformation'].__setitem__('descriptor',mutated(c['deformation']['descriptor'],0xa414,17))),
        ('nan_marker',lambda c:c['deformation'].__setitem__('descriptor',mutated(c['deformation']['descriptor'],0x25c0,0x7fc00000))),
        ('bad_disable',lambda c:c['deformation'].__setitem__('disabled',2)),
        ('bad_unlimited',lambda c:c['deformation']['parameters'].__setitem__(5,0.)),
    ]:
        c=copy.deepcopy(row);mutate(c);guards[name]=request([c])
    return good,guards


def mutated(raw,at,value):
    b=bytearray(raw);struct.pack_into('<I',b,at,value);return bytes(b)


def check_answer(row,result,d,coverage):
    bypass=not result['started'];coverage['unstarted']+=bypass;coverage['finished']+=result['started'] and result['status']==0;coverage['capacity_failed']+=result['started'] and result['status']!=0
    old=row['deformation']['descriptor'];coverage['window_changed']+=d[0xa4c4:0xa4ec]!=old[0xa4c4:0xa4ec]
    if bypass:assert d==old
    allowed={0xa490,0xa494,0xa498,0xa4c4+struct.unpack_from('<I',old,0xa4ec)[0]*4}
    assert all(d[at:at+4]==old[at:at+4] for at in range(0,layout.SIZE,4) if at not in allowed)
    checks,_=frame.semantic([row],[result]);assert all(checks.values()),checks


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,identities=cases()
    if args.pilot:rows=rows[:32]
    rust=ROOT/f'target/{args.profile}/grid-deformation-physical-frame.exe';reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),32):
        batch=rows[start:start+32];source=out/f'{start}-input.bin';source.write_bytes(request(batch));answers={};runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'{start}-{kind}.bin';exe=rust if kind=='rust' else PROBE
            run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run['exit']==0 and not run['stderr'],runs[kind]
            if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+sum(answer_size(r) for r in batch) and len(b)==len(a)
        differences=[dict(offset=hex(i),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,repeat_exact=a==answers['repeat'],differences=differences,executions=runs);(out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert item['repeat_exact'] and not differences,differences[:10]
        for row,(result,d) in zip(batch,decode(a,batch)):
            check_answer(row,result,d,coverage)
    good,bad=guard_payloads(rows);guards=[]
    for name,payload in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(payload);runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'guard-{name}-{kind}.bin';exe=rust if kind=='rust' else PROBE
            run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,output_exists=target.exists())
            assert run['exit']==1 if kind=='rust' else 0<run['exit']<256
            assert not target.exists() and 'fault=' not in run['stderr'],runs[kind]
        guards.append(dict(name=name,executions=runs))
    for kind in ['original','rust']:
        source=out/f'guard-existing-{kind}-input.bin';source.write_bytes(good);target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel');exe=rust if kind=='rust' else PROBE
        run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);assert run['exit']==(1 if kind=='rust' else 6) and target.read_bytes()==b'owned sentinel';guards.append(dict(name='existing-'+kind,execution=run,preserved=True))
    (out/'guards.json').write_text(json.dumps(guards,indent=2))
    result=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),identities=identities,reports=reports,guards=guards,coverage=dict(coverage),files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},scope='Whole single-body B7B850 with static first pool, registered9A0C90/A064A0 and first shape230=1 so bulk collection is empty; optional correction disabled, supplied settings; not driving cadence.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
