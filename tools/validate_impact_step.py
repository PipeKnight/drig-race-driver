"""Whole impact-only main manager, repeat and joined Rust comparison."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_contact_bias import compare, execute
from validate_contact_rows import MAX
from validate_contact_bias import PROBE as BIAS_PROBE
from validate_main_contact_response import fput, uput, fget, uget
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31545350
PROBE = 'e4ec364c12dc223a9fdd89c0927b0c11ad78c458862be59ac8cf511b81ea3974'


def pack(cases):
    data = bytearray(struct.pack('<II', MAGIC, len(cases)))
    for c in cases:
        data += struct.pack('<6I11f4I2I13f',len(c['bodies']),len(c['materials']),len(c['contacts']),
                            len(c['rows']),c['capacity'],c['iterations'],*c['parameters'],*c['ends'],*c['pair'],*c['cache'])
        for b in c['bodies']:data += b['raw']+struct.pack('<25f',*b['properties'],*b['workspace'])
        for m in c['materials']:data += struct.pack('<13f',*m)
        for mode,raw in c['contacts']:data += struct.pack('<I',mode)+raw
        data += b''.join(c['rows'])
    return bytes(data)


def body(rng,number):
    b=bytearray(rng.randbytes(0x424))
    for offset,count in [(0x110,16),(0x1c0,4),(0x210,24)]:
        for i in range(count):fput(b,offset+4*i,rng.uniform(-2,2))
    uput(b,0x334,0x1c0);fput(b,0x3f4,.5+number%3*.25)
    for o in range(0x40c,0x411):b[o]=(number+o)%2
    b[0x40d]=(number//2)%2;b[0x40e]=(number//4)%2
    inertia=[.015*(i+1) for i in range(16)]
    for i in range(4):inertia[i*4+i]=1.+.25*i
    return dict(raw=b,properties=[1.,*inertia],workspace=[rng.uniform(-2,2) for _ in range(8)])


def authored(rng,number):
    nb=1+number%8;nc=[0,1,2,3,4,7,16,32,64][number%9]
    materials=[[float(i),.3,.4,.01,10.,.2,2.,.5,3.,.75,20.,float(i%2),.125] for i in range(4)]
    contacts=[]
    for j in range(nc):
        group=j//(1+number%4);a=group%nb;b=a if number%7==0 else (a+1)%nb
        c=bytearray(rng.randbytes(128));two=(group+number)%2
        uput(c,4,a);uput(c,8,b);uput(c,0x1c,a);uput(c,0x20,b)
        uput(c,0x30,(j+number)%4);uput(c,0x34,(j+number//2)%4)
        uput(c,0x2c,(number+j)%3);uput(c,0x28,(number//3+j)%3)
        c[0xe]=(number+j)%2;c[0x70]=0;c[0x73]=(number//2+j)%2
        fput(c,0x38,[-0.,0.,.125,1.,-1.,10.][(number+j)%6])
        fput(c,0x3c,[MAX,0.,.1,10.][(number+j)%4])
        for k in range(4):fput(c,0x50+4*k,rng.uniform(-5,5))
        normal=[[0.,1.,0.,0.],[1.,0.,0.,0.],[0.,-1.,0.,0.],[0.]*4,[.1,.9,.3,.125]][(number+j)%5]
        for k,v in enumerate(normal):fput(c,0x60+4*k,v)
        contacts.append((two,c))
    slots=max(1,4*nc)+number%3
    slots=min(256,slots)
    rows=[bytearray(struct.pack('<48f',*[rng.uniform(-2,2) for _ in range(48)])) for _ in range(slots)]
    return dict(bodies=[body(rng,number+j) for j in range(nb)],materials=materials,contacts=contacts,rows=rows,
                capacity=0 if number%11==0 else slots,iterations=[0,1,2,4,8,16,32][number%7],
                parameters=[[.008,.016,.03][number%3],[.5,1.,2.][(number//3)%3],.25,1.,[0.,.5,1.,2.][number%4],.01,.2,.125,.8,.2,.2],
                ends=[1,2,3,4],pair=[0,1] if number%17==0 else [0xffffffff]*2,cache=[float(i)*.125 for i in range(13)])


def decode(data,cases):
    if struct.unpack_from('<II',data)!=(MAGIC,len(cases)):raise ValueError('Output header')
    at=8;result=[]
    for c in cases:
        h=struct.unpack_from('<20I',data,at);at+=80
        nb=len(c['bodies']);nc=len(c['contacts']);nr=len(c['rows'])
        workspace=data[at:at+nb*32];at+=nb*32
        bodies=[data[at+i*0x424:at+(i+1)*0x424] for i in range(nb)];at+=nb*0x424
        contacts=[data[at+i*128:at+(i+1)*128] for i in range(nc)];at+=nc*128
        rows=[data[at+i*192:at+(i+1)*192] for i in range(nr)];at+=nr*192
        result.append(dict(success=h[0],ends=h[1:5],cache=h[5:],workspace=workspace,bodies=bodies,contacts=contacts,rows=rows))
    if at!=len(data):raise ValueError('Output framing')
    return result


def semantic(cases,decoded):
    checks=dict(capacity_bypass=True,empty_rows_retained=True,empty_workspace_retained=True,unused_rows_retained=True,
                contact_metadata=True,body_opaque_retained=True,contact_opaque_retained=True)
    activity=dict(success=0,capacity_failures=0,empty=0,zero_iterations=0,normal_rows=0,friction_rows=0,
                  group_ends=0,changed_velocities=0,changed_impulse_accumulators=0,changed_contact_impulses=0,
                  external_force_enabled=0,external_torque_enabled=0,alias_contacts=0,disabled_contacts=0)
    for c,o in zip(cases,decoded):
        nc=len(c['contacts']);success=o['success'];activity['success']+=success
        activity['capacity_failures']+=not success;activity['empty']+=not nc;activity['zero_iterations']+=c['iterations']==0
        if not success:
            checks['capacity_bypass'] &= o['rows']==c['rows'] and o['bodies']==[b['raw'] for b in c['bodies']]
            checks['capacity_bypass'] &= o['workspace']==struct.pack('<'+'f'*len(c['bodies'])*8,*[f for b in c['bodies'] for f in b['workspace']])
        elif not nc:
            checks['empty_rows_retained'] &= o['rows']==c['rows'] and list(o['ends'])==c['ends']
            checks['empty_workspace_retained'] &= o['workspace']==struct.pack('<'+'f'*len(c['bodies'])*8,*[f for b in c['bodies'] for f in b['workspace']])
        else:
            activity['normal_rows']+=o['ends'][1];activity['friction_rows']+=o['ends'][3]-o['ends'][1]
            checks['unused_rows_retained'] &= o['rows'][o['ends'][3]:]==c['rows'][o['ends'][3]:]
        for before,after in zip(c['bodies'],o['bodies']):
            b=before['raw'];activity['changed_velocities']+=b[0x210:0x230]!=after[0x210:0x230]
            activity['changed_impulse_accumulators']+=b[0x240:0x250]!=after[0x240:0x250]
            activity['external_force_enabled']+=bool(b[0x40e]);activity['external_torque_enabled']+=bool(b[0x40d])
            expected=bytearray(b);expected[0x210:0x230]=after[0x210:0x230];expected[0x240:0x250]=after[0x240:0x250];expected[0x40c]=after[0x40c]
            checks['body_opaque_retained'] &= expected==after
        for (mode,before),after in zip(c['contacts'],o['contacts']):
            checks['contact_metadata'] &= uget(after,0x18)==1 and uget(after,0x14)==(4 if after[0x70] else 1)
            activity['group_ends']+=bool(after[0x70]);activity['changed_contact_impulses']+=before[0x3c:0x40]!=after[0x3c:0x40]
            activity['alias_contacts']+=mode and uget(before,0x1c)==uget(before,0x20);activity['disabled_contacts']+=before[0xe]
            expected=bytearray(before);expected[0x14:0x1c]=after[0x14:0x1c];expected[0x70]=after[0x70];expected[0x3c:0x40]=after[0x3c:0x40]
            checks['contact_opaque_retained'] &= expected==after
    return checks,activity


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    probe=ROOT/'local/behavior/impact-step-probe.exe';image=ROOT/'local/compatibility/grid-unpacked.exe';rust=ROOT/f'target/{args.profile}/grid-impact-step.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE:raise ValueError('Image/probe identity')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    rng=random.Random(200810100545);cases=[authored(rng,i) for i in range(512)]
    previous=ROOT/f'local/behavior/contact-bias-20261010-{args.profile}-final-02'
    upstream=previous/'input.bin';baseline=(previous/'original.bin').read_bytes()
    if digest(upstream)!='b3c6fa0b4885b1f973a2ac49266acce1a2fc4750161f3bf41a0ca67b14164c54':raise ValueError('Upstream request identity')
    if any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust','expected']):raise ValueError('Upstream outputs disagree')
    bias_probe=ROOT/'local/behavior/contact-bias-probe.exe'
    if digest(bias_probe)!=BIAS_PROBE:raise ValueError('Bias probe identity')
    upstream_runs={}
    for label,command in [('original',[str(bias_probe),str(image)]),('rust',[str(ROOT/f'target/{args.profile}/grid-contact-bias.exe')])]:
        data,run=execute(command,upstream,out/f'upstream-{label}.bin');run.update(compare(data,baseline));upstream_runs[label]=run
        if not run['passed']:raise ValueError('Fresh upstream mismatch')
    raw=upstream.read_bytes();handoff=[]
    for index in range(512,6352):
        two,disabled=struct.unpack_from('<II',raw,8+index*220)
        accumulated=struct.unpack_from('<f',raw,8+index*220+12)[0]
        handoff.append((two,disabled,accumulated))
    for start in range(0,len(handoff),32):
        c=authored(rng,7+9*(start//32));group=handoff[start:start+32]
        c['contacts']=c['contacts'][:len(group)]
        for j,(two,disabled,accumulated) in enumerate(group):
            _,contact=c['contacts'][j];c['contacts'][j]=(two,contact)
            contact[0xe]=disabled;fput(contact,0x38,accumulated)
        cases.append(c)
    source=out/'input.bin';source.write_bytes(pack(cases))
    report=dict(profile=args.profile,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),
                input_sha256=digest(source),cases=len(cases),authored_controls=512,handoff_cases=len(cases)-512,handoff_contacts=len(handoff),
                upstream_runs=upstream_runs,bias_probe_sha256=BIAS_PROBE,upstream_input_sha256=digest(upstream),
                scope='Whole B7ACC0, impact pools only; handoff retains upstream accumulated operands and disabled flags, roles/geometry/parameters authored; no live physical frame.',runs={},guards={},live_verification=False)
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    original=None
    for label,command in commands:
        data,run=execute(command,source,out/f'{label}.bin')
        if original is None:original=data
        run.update(compare(data,original));report['runs'][label]=run
    checks,activity=semantic(cases,decode(original,cases));report.update(semantic_checks=checks,activity=activity)
    good=pack([authored(random.Random(42),3)])
    bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:],excess_cases=good[:4]+struct.pack('<I',1025)+good[8:])
    for name,offset,value in [('zero_bodies',8,0),('excess_bodies',8,17),('zero_materials',12,0),('excess_materials',12,9),('excess_contacts',16,65),('zero_row_slots',20,0),('excess_row_slots',20,257),('capacity_over_slots',24,257),('excess_iterations',28,33)]:
        data=bytearray(good);uput(data,offset,value);bad[name]=data
    base=authored(random.Random(42),3)
    for name,mutate in [
        ('nan_delta',lambda c:c['parameters'].__setitem__(0,float('nan'))),('zero_delta',lambda c:c['parameters'].__setitem__(0,0.)),
        ('negative_time_factor',lambda c:c['parameters'].__setitem__(1,-1.)),
        ('invalid_pair',lambda c:c['pair'].__setitem__(0,99)),('mixed_null_pair',lambda c:c['pair'].__setitem__(0,0)),
        ('invalid_body_pointer',lambda c:uput(c['bodies'][0]['raw'],0x334,0)),('invalid_force_flag',lambda c:c['bodies'][0]['raw'].__setitem__(0x40e,2)),
        ('nan_force',lambda c:fput(c['bodies'][0]['raw'],0x260,float('nan'))),('nan_mass',lambda c:c['bodies'][0]['properties'].__setitem__(0,float('nan'))),
        ('nan_workspace',lambda c:c['bodies'][0]['workspace'].__setitem__(0,float('nan'))),('invalid_mode',lambda c:c['contacts'].__setitem__(0,(2,c['contacts'][0][1]))),
        ('invalid_contact_body',lambda c:uput(c['contacts'][0][1],0x1c,99)),('invalid_material',lambda c:uput(c['contacts'][0][1],0x30,99)),
        ('invalid_shape',lambda c:uput(c['contacts'][0][1],0x2c,3)),('stale_group_end',lambda c:c['contacts'][0][1].__setitem__(0x70,1)),
        ('invalid_disabled',lambda c:c['contacts'][0][1].__setitem__(0xe,2)),('nan_normal',lambda c:fput(c['contacts'][0][1],0x60,float('nan'))),
        ('negative_previous',lambda c:fput(c['contacts'][0][1],0x3c,-1.))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,request in bad.items():
        guard=out/f'guard-{name}.bin';guard.write_bytes(request);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(guard),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,passed=0<run.returncode<256 and 'impact_step_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for r in upstream_runs.values()) and all(r['passed'] for guard in report['guards'].values() for r in guard.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Impact step comparison failed; inspect private evidence')


if __name__=='__main__':main()
