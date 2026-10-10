"""Whole B7B850 impact-only frame with naturally empty registered callbacks."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private, ANALYSIS
from validate_contact_bias import compare, execute
from validate_impact_step import authored as step_authored, pack as step_pack, decode as step_decode, PROBE as STEP_PROBE
from validate_main_contact_response import fput, uput, fget, uget
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
MAGIC=0x31524650
PROBE='5c42ca441037f5bcad3e99c18320c7c9f2aeec19a4cd9d9bb23b2b2a81d96895'
MAX=struct.unpack('<f',bytes.fromhex('ffff7f7f'))[0]


def pack(cases):
    d=bytearray(step_pack(cases));uput(d,0,MAGIC);return d


def configure(c,number):
    c=copy.deepcopy(c);c['parameters'][1:5]=[0.,-9.81,0.,0.]
    for j,b in enumerate(c['bodies']):
        raw=b['raw'];mode=number+j
        for offset in [0x2fc,0x320,0x324,0x328,0x32c,0x338,0x33c,0x340,0x418,0x41c,0x420,0x400]:uput(raw,offset,0)
        matrix=[.8,.6,0.,0.,-.6,.8,0.,0.,0.,0.,1.,0.,*struct.unpack_from('<4f',raw,0x1c0)]
        for i,v in enumerate(matrix):fput(raw,0x190+4*i,v)
        for offset in [0x90,0x110,0x150]:
            for i in range(16):fput(raw,offset+4*i,(1.+.25*(i//4)) if i%5==0 else .005*(i+1))
        for i,v in enumerate([.1,-.2,.3,0.]):fput(raw,0x230+4*i,v)
        for i,v in enumerate([.1*(j+1),-.125,.25,0.]):fput(raw,0x380+4*i,v)
        raw[0x390]=mode%2
        sleep=[0.,MAX,MAX,MAX,MAX,[0.,.01,.5][number%13]] if number%13<3 else [0.,.01,.01,.01,.01,MAX]
        for i,v in enumerate(sleep):fput(raw,0x3a0+4*i,v)
        for i,v in enumerate(([MAX]*4,[.5,1.,2.,1.],[0.,0.,0.,0.],[2.,MAX,1.,10.])[mode%4]):fput(raw,0x3b8+4*i,v)
        for i,v in enumerate(([0.]*4,[.1,.2,.3,.5],[10.,0.,1.,100.])[mode%3]):fput(raw,0x3c8+4*i,v)
        for i in range(6):fput(raw,0x3d8+4*i,0. if mode%3==0 else .1*(i+1)*(10. if mode%7==0 else 1.))
        fput(raw,0x3f0,2.);fput(raw,0x3f8,0. if mode%4==0 else 1.);fput(raw,0x3fc,1.)
    for mode,craw in c['contacts']:
        craw[0xc]=0 if mode else 1;craw[0xd]=0;craw[0xf]=1;uput(craw,0x10,0);craw[0x70]=0
    return c


def decode(data,cases):
    if struct.unpack_from('<II',data)!=(MAGIC,len(cases)):raise ValueError('Output header')
    at=8;result=[]
    for c in cases:
        h=struct.unpack_from('<21I',data,at);at+=84;nb=len(c['bodies']);nc=len(c['contacts']);nr=len(c['rows'])
        properties=data[at:at+nb*68];at+=nb*68;workspace=data[at:at+nb*32];at+=nb*32
        bodies=[data[at+i*0x424:at+(i+1)*0x424] for i in range(nb)];at+=nb*0x424
        contacts=[data[at+i*128:at+(i+1)*128] for i in range(nc)];at+=nc*128
        rows=[data[at+i*192:at+(i+1)*192] for i in range(nr)];at+=nr*192
        result.append(dict(status=h[0],started=h[1],ends=h[2:6],cache=h[6:],properties=properties,workspace=workspace,bodies=bodies,contacts=contacts,rows=rows))
    if at!=len(data):raise ValueError('Output framing')
    return result


def layouts(data):
    at=8;result=[]
    for _ in range(uget(data,4)):
        nb,nm,nc,nr,capacity,iterations=struct.unpack_from('<6I',data,at);at+=24
        parameters=list(struct.unpack_from('<11f',data,at));at+=44
        ends=list(struct.unpack_from('<4I',data,at));at+=16
        pair=list(struct.unpack_from('<2I',data,at));at+=8;cache=list(struct.unpack_from('<13f',data,at));at+=52
        bodies=[]
        for _ in range(nb):
            raw=bytearray(data[at:at+0x424]);at+=0x424;values=struct.unpack_from('<25f',data,at);at+=100
            bodies.append(dict(raw=raw,properties=list(values[:17]),workspace=list(values[17:])))
        materials=[]
        for _ in range(nm):materials.append(list(struct.unpack_from('<13f',data,at)));at+=52
        contacts=[]
        for _ in range(nc):mode=uget(data,at);at+=4;contacts.append((mode,bytearray(data[at:at+128])));at+=128
        rows=[bytearray(data[at+i*192:at+(i+1)*192]) for i in range(nr)];at+=nr*192
        result.append(dict(bodies=bodies,materials=materials,contacts=contacts,rows=rows,capacity=capacity,iterations=iterations,parameters=parameters,ends=ends,pair=pair,cache=cache))
    if at!=len(data):raise ValueError('Upstream request framing')
    return result


def semantic(cases,decoded):
    checks=dict(zero_iterations_retained=True,finished_forces_cleared=True,finished_shift_consumed=True,unstarted_contact_rows_retained=True,unused_rows_retained=True,opaque_body_retained=True)
    activity=dict(finished=0,capacity_failed=0,sleep_bypasses=0,zero_iterations=0,shift_publications=0,velocity_changes=0,
                  force_changes=0,sleep_timer_changes=0,normal_rows=0,friction_rows=0,empty_contacts=0)
    for c,o in zip(cases,decoded):
        zero=c['iterations']==0;finished=o['started'] and o['status']==0
        activity['finished']+=bool(finished);activity['capacity_failed']+=o['started'] and o['status']==1
        activity['sleep_bypasses']+=not o['started'] and o['status']==1;activity['zero_iterations']+=zero;activity['empty_contacts']+=not c['contacts']
        if zero:checks['zero_iterations_retained'] &= o['bodies']==[b['raw'] for b in c['bodies']] and o['contacts']==[v for _,v in c['contacts']] and o['rows']==c['rows']
        if not o['started']:checks['unstarted_contact_rows_retained'] &= o['contacts']==[v for _,v in c['contacts']] and o['rows']==c['rows']
        if finished and c['contacts']:
            activity['normal_rows']+=o['ends'][1];activity['friction_rows']+=o['ends'][3]-o['ends'][1]
            checks['unused_rows_retained'] &= o['rows'][o['ends'][3]:]==c['rows'][o['ends'][3]:]
        for b,after in zip(c['bodies'],o['bodies']):
            before=b['raw'];activity['velocity_changes']+=before[0x210:0x230]!=after[0x210:0x230]
            activity['force_changes']+=before[0x250:0x270]!=after[0x250:0x270];activity['sleep_timer_changes']+=before[0x3a0:0x3a4]!=after[0x3a0:0x3a4]
            activity['shift_publications']+=bool(finished and before[0x390])
            if finished:
                checks['finished_forces_cleared'] &= after[0x250:0x270]==bytes(32) and not after[0x40d] and not after[0x40e]
                if before[0x390]:checks['finished_shift_consumed'] &= not after[0x390] and after[0x380:0x390]==bytes(16)
            expected=bytearray(before)
            for lo,hi in [(0xd0,0x150),(0x190,0x230),(0x240,0x270),(0x380,0x391),(0x3a0,0x3a4),(0x40c,0x40f)]:expected[lo:hi]=after[lo:hi]
            checks['opaque_body_retained'] &= expected==after
    return checks,activity


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/impact-frame-probe.exe';rust=ROOT/f'target/{args.profile}/grid-physical-frame.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE:raise ValueError('Image/probe identity')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False);rng=random.Random(200810100600)
    cases=[configure(step_authored(rng,i),i) for i in range(512)]
    previous=ROOT/f'local/behavior/impact-step-20261010-{args.profile}-final-03';source=previous/'input.bin';baseline=(previous/'original.bin').read_bytes()
    if digest(source)!='d468ba745e5843d4edfe6a5d002a97767390e82ed933b515fef6ae51de08f841' or any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust']):raise ValueError('Upstream identity/agreement')
    step_probe=ROOT/'local/behavior/impact-step-probe.exe'
    if digest(step_probe)!=STEP_PROBE:raise ValueError('Upstream probe identity')
    upstream_runs={}
    for label,command in [('original',[str(step_probe),str(image)]),('rust',[str(ROOT/f'target/{args.profile}/grid-impact-step.exe')])]:
        data,run=execute(command,source,out/f'upstream-{label}.bin');run.update(compare(data,baseline));upstream_runs[label]=run
        if not run['passed']:raise ValueError('Fresh upstream mismatch')
    supplied=layouts(source.read_bytes());outputs=step_decode(baseline,supplied)
    for number,(c,o) in enumerate(zip(supplied[512:],outputs[512:])):
        for b,raw in zip(c['bodies'],o['bodies']):b['raw']=bytearray(raw)
        c['contacts']=[(mode,bytearray(raw)) for (mode,_),raw in zip(c['contacts'],o['contacts'])];c['rows']=[bytearray(raw) for raw in o['rows']]
        cases.append(configure(c,512+number))
    source=out/'input.bin';source.write_bytes(pack(cases))
    report=dict(profile=args.profile,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),input_sha256=digest(source),cases=len(cases),authored_controls=512,handoff_cases=183,
                upstream_runs=upstream_runs,scope='Whole B7B850, impact contacts only, no registered callbacks, optional correction disabled, no body links/shapes/wakeup; authored settings; no live frame.',runs={},guards={},live_verification=False)
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])];original=None
    for label,command in commands:
        data,run=execute(command,source,out/f'{label}.bin')
        if original is None:original=data
        run.update(compare(data,original));report['runs'][label]=run
    checks,activity=semantic(cases,decode(original,cases));report.update(semantic_checks=checks,activity=activity)
    base=configure(step_authored(random.Random(42),3),3);good=pack([base]);bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:])
    for name,mutate in [('zero_delta',lambda c:c['parameters'].__setitem__(0,0.)),('excess_delta',lambda c:c['parameters'].__setitem__(0,.251)),('nan_gravity',lambda c:c['parameters'].__setitem__(2,float('nan'))),
                        ('invalid_capacity',lambda c:c.__setitem__('capacity',257)),('excess_iterations',lambda c:c.__setitem__('iterations',33)),
                        ('body_link',lambda c:uput(c['bodies'][0]['raw'],0x320,1)),('body_callback',lambda c:uput(c['bodies'][0]['raw'],0x418,1)),('body_wakeup',lambda c:uput(c['bodies'][0]['raw'],0x400,0xffffffff)),
                        ('nan_drag',lambda c:fput(c['bodies'][0]['raw'],0x3d8,float('nan'))),('nan_basis',lambda c:fput(c['bodies'][0]['raw'],0x190,float('nan'))),('invalid_shift_flag',lambda c:c['bodies'][0]['raw'].__setitem__(0x390,2)),
                        ('contact_link',lambda c:uput(c['contacts'][0][1],0x10,1)),('other_pool_contact',lambda c:c['contacts'][0][1].__setitem__(0xf,0)),('contact_type',lambda c:c['contacts'][0][1].__setitem__(0xc,2)),('stale_metadata',lambda c:c['contacts'][0][1].__setitem__(0x70,1))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,request in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(request);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,passed=0<run.returncode<256 and 'impact_frame_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for r in upstream_runs.values()) and all(r['passed'] for guard in report['guards'].values() for r in guard.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole frame comparison failed; inspect private evidence')


if __name__=='__main__':main()
