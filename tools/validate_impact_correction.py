"""Whole optional correction manager with impact pools, bias/solve/pose joined.

All rows, roles, parameters and timing are authored; reused accumulator words
do not establish synchronized live frames. Other/multirow pools are empty.
"""
import argparse
import copy
import json
import math
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_steering_control import digest
from validate_contact_bias import compare, PROBE as BIAS_PROBE, f32
from validate_physical_pose import case as pose_case, floats, words, get
from validate_physical_response import simple_row, fput, uput

ROOT=Path(__file__).resolve().parents[1]
PROBE='b1248de7fa8fb655d3598609da110e805099c88ba18f7762dc9ae36175885f57'
MAGIC=0x31435049


def row(rng,a=0,b=0):
    r=simple_row(3.,lower=-2.,upper=2.)
    for i in range(16):fput(r,i*4,rng.uniform(-.1,.1));fput(r,0x40+i*4,rng.uniform(-.01,.01))
    for o,v in [(0x80,7.),(0x84,2.),(0x94,4.),(0x98,5.),(0x9c,6.),(0xac,1.),(0xb0,2.)]:fput(r,o,v)
    uput(r,0xa0,a);uput(r,0xa4,b);return r


def case(rng,n=0,two=0,bodies=2):
    return dict(rows=[row(rng,rng.randrange(bodies),rng.randrange(bodies)) for _ in range(n)],two=two,
        poses=[pose_case(rng,head=i%2,shapes=i%4) for i in range(bodies)],
        contacts=[(0,1.) for _ in range(n)],workspace=[.125]*(bodies*8),spans=[13,17],
        iterations=4,softness=.1,scale=1.,threshold=0.,relaxation=1.)


def controls():
    rng=random.Random(200810100435);cases=[case(rng)]
    for disabled,accumulated,iterations in [(1,2.,4),(0,-1.,4),(0,0.,0)]:
        c=case(rng,n=1,bodies=1);c['contacts']=[(disabled,accumulated)];c['iterations']=iterations;cases.append(c)
    c=case(rng,n=1,bodies=1);r=simple_row(9.,upper=10.);fput(r,0,1.);fput(r,0x40,1.)
    c.update(rows=[r],softness=0.,iterations=1);cases.append(c)
    c=case(rng,n=1,two=1,bodies=2);r=simple_row(9.,upper=10.);uput(r,0xa4,1);fput(r,0x40,1.);fput(r,0x60,-1.)
    c.update(rows=[r],softness=0.,iterations=1);cases.append(c)
    c=copy.deepcopy(c);uput(c['rows'][0],0xa4,0);cases.append(c)
    c=case(rng,n=3,two=1);c['contacts']=[(1,2.),(0,-1.),(0,1.)];cases.append(c)
    cases.append(case(rng,bodies=0))
    c=case(rng,n=1,bodies=1);c['contacts']=[(0,-0.)];cases.append(c)
    c=case(rng,n=1,two=1);c['contacts']=[(1,2.)];cases.append(c)
    c=case(rng,n=2,bodies=1);uput(c['rows'][0],0xb4,2);cases.append(c)
    # Singular normalization is deliberately unused on the no-active bypass.
    c=case(rng,n=1,bodies=1);c['contacts']=[(0,-1.)];fput(c['rows'][0],0xac,0.);fput(c['rows'][0],0xb0,-c['softness']);cases.append(c)
    for index in range(512):
        n=256 if index==511 else rng.choice([0,1,2,3,7,16,32,64]);two=rng.randrange(n+1);bodies=rng.choice([1,2,4,8,16])
        c=case(rng,n,two,bodies);c.update(iterations=rng.choice([0,1,2,4,8,32]),softness=rng.choice([0.,.1,.2]),
            scale=rng.choice([0.,-0.,-1.,.25,1.]),threshold=rng.uniform(-.5,.5),relaxation=rng.choice([0.,-.25,.5,1.]))
        c['contacts']=[(int(rng.random()<.25),rng.uniform(-4,4)) for _ in range(n)]
        if index%11==0:c['contacts']=[(1,2.) for _ in range(n)]
        if index%13==0:c.update(contacts=[(0,-1.) for _ in range(n)],threshold=0.,scale=1.)
        for p in c['poses']:
            basis=list(get(p['body'],0x190));angle=rng.uniform(-math.pi,math.pi);co,si=math.cos(angle),math.sin(angle)
            basis[:12]=[co,0.,-si,0.,0.,1.,0.,0.,si,0.,co,0.]
            basis[12:]=[rng.uniform(-500,500) for _ in range(3)]+[1.];floats(p['body'],0x190,basis)
            floats(p['body'],0x230,[rng.uniform(-1,1) for _ in range(3)]+[0.])
        for start,end in [(0,two),(two,n)]:
            cursor=start
            while cursor<end:
                length=min(rng.choice([1,1,2,3]),end-cursor);a=rng.randrange(bodies);b=rng.randrange(bodies)
                for i in range(cursor,cursor+length):uput(c['rows'][i],0xa0,a);uput(c['rows'][i],0xa4,b)
                uput(c['rows'][cursor],0xb4,length);cursor+=length
        cases.append(c)
    return cases


def pack(cases):
    d=bytearray(struct.pack('<II',MAGIC,len(cases)))
    for c in cases:
        d.extend(struct.pack('<6I4f',len(c['rows']),c['two'],len(c['poses']),c['iterations'],*c['spans'],
            c['softness'],c['scale'],c['threshold'],c['relaxation']))
        for r in c['rows']:d.extend(r)
        for disabled,accumulated in c['contacts']:d.extend(struct.pack('<If',disabled,accumulated))
        d.extend(struct.pack(f'<{len(c["workspace"])}f',*c['workspace']))
        for p in c['poses']:
            d.extend(struct.pack('<II',p['head'],len(p['shapes'])));d.extend(p['body'])
            for s in p['shapes']:d.extend(s)
    return d


def decode(data,cases):
    if data[:8]!=struct.pack('<II',MAGIC,len(cases)):raise ValueError('Output header')
    cursor=8;results=[]
    for c in cases:
        n,b,c0,c8=struct.unpack_from('<4I',data,cursor);cursor+=16
        if n!=len(c['rows']) or b!=len(c['poses']):raise ValueError('Output counts')
        rows=[data[cursor+i*192:cursor+(i+1)*192] for i in range(n)];cursor+=n*192
        workspace=data[cursor:cursor+b*32];cursor+=b*32;poses=[]
        for p in c['poses']:
            if struct.unpack_from('<II',data,cursor)!=(p['head'],len(p['shapes'])):raise ValueError('Pose header')
            cursor+=8;body=data[cursor:cursor+0x424];cursor+=0x424;shapes=[]
            for _ in p['shapes']:shapes.append(data[cursor:cursor+0x220]);cursor+=0x220
            poses.append(dict(body=body,shapes=shapes))
        results.append(dict(rows=rows,workspace=workspace,spans=[c0,c8],poses=poses))
    if cursor!=len(data):raise ValueError('Output length')
    return results


def active(c):
    return any(not disabled and f32(f32(f32(accumulated)-f32(c['threshold']))*f32(c['scale']))>=0. for disabled,accumulated in c['contacts'])


def semantic_checks(cases,results):
    r=results
    checks=dict(empty_spans_preserved=r[0]['spans']==[13,17],disabled_workspace_preserved=r[1]['workspace']==struct.pack('<8f',*cases[1]['workspace']),
        negative_bias_bypasses=r[2]['poses'][0]['body']==cases[2]['poses'][0]['body'],
        equality_activates_and_clears=r[3]['workspace']==bytes(32),single_translation=get(r[4]['poses'][0]['body'],0x1c0,4)==(1.,0.,0.,1.),
        opposite_two_body_translation=get(r[5]['poses'][0]['body'],0x1c0,4)[0]==1. and get(r[5]['poses'][1]['body'],0x1c0,4)[0]==-1.,
        alias_second_write_wins=get(r[6]['poses'][0]['body'],0x1c0,4)[0]==-1.,
        mixed_pool_updates=r[7]['workspace']!=struct.pack('<16f',*cases[7]['workspace']),
        empty_zero_body=r[8]['poses']==[],negative_zero_activates=r[9]['workspace']==bytes(32),
        disabled_two_vectors_zero=r[10]['rows'][0][0x40:0x80]==bytes(64),
        bypass_does_not_normalize_singular_row=r[12]['rows'][0][:0x40]==cases[12]['rows'][0][:0x40],
        nonempty_spans_exclude_tangents=True,disabled_rows_keep_impulse_on_bypass=True,
        no_active_body_and_shape_bytes_preserved=True,active_unused_bodies_publish=True,row_reset_before_bias=True,
        unrelated_body_bytes_preserved=True,unrelated_shape_bytes_preserved=True)
    body_changed=set(range(0xd0,0x150))|set(range(0x190,0x210))
    for c,r in zip(cases,results):
        is_active=active(c)
        if c['rows']:
            checks['nonempty_spans_exclude_tangents'] &= r['spans']==[len(c['rows'])]*2
            checks['row_reset_before_bias'] &= all(row[0x80:0x84]==bytes(4) for row in r['rows'])
        if not is_active:
            checks['no_active_body_and_shape_bytes_preserved'] &= all(p['body']==q['body'] and p['shapes']==q['shapes'] for p,q in zip(c['poses'],r['poses']))
            checks['disabled_rows_keep_impulse_on_bypass'] &= all(a[0x94:0x98]==b[0x94:0x98] for a,b in zip(c['rows'],r['rows']))
        for p,q in zip(c['poses'],r['poses']):
            checks['unrelated_body_bytes_preserved'] &= all(a==b for i,(a,b) in enumerate(zip(p['body'],q['body'])) if i not in body_changed)
            if is_active:checks['active_unused_bodies_publish'] &= q['body'][0xd0:0x110]!=p['body'][0xd0:0x110]
            for a,b in zip(p['shapes'],q['shapes']):checks['unrelated_shape_bytes_preserved'] &= all(x==y for i,(x,y) in enumerate(zip(a,b)) if not 0x130<=i<0x170)
    return checks


def execute(command,source,target):
    run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
    if run.returncode:raise RuntimeError(f'Execution failed {run.returncode}: {run.stdout} {run.stderr}')
    return target.read_bytes(),dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)


def oversized_correction(c):
    r=simple_row(0.);fput(r,0x50,100.)
    c.update(rows=[r],contacts=[(0,1.)],softness=0.,iterations=1)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/impact-correction-probe.exe';rust=ROOT/f'target/{args.profile}/grid-impact-correction.exe'
    bias=ROOT/'local/behavior/contact-bias-probe.exe';bias_rust=ROOT/f'target/{args.profile}/grid-contact-bias.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE or digest(bias)!=BIAS_PROBE:raise ValueError('Image/probe identity')
    previous=ROOT/f'local/behavior/contact-bias-20261010-{args.profile}-final-02';request=previous/'input.bin'
    if digest(request)!='b3c6fa0b4885b1f973a2ac49266acce1a2fc4750161f3bf41a0ca67b14164c54':raise ValueError('Upstream request identity')
    baseline=(previous/'original.bin').read_bytes()
    if any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust','expected']):raise ValueError('Upstream comparison')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    report=dict(profile=args.profile,live_verification=False,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),
        bias_probe_sha256=BIAS_PROBE,bias_rust_sha256=digest(bias_rust),bias_input_sha256=digest(request),upstream_runs={},runs={},guards={},
        scope='Complete original B7B5E0 with only impact contact pools, unchanged bias/normalization/iteration/pose dependencies, authored rows/roles/parameters. Multirow/joint pools empty, no live physical frame.')
    for label,command in [('original',[str(bias),str(image)]),('rust',[str(bias_rust)])]:
        data,r=execute(command,request,out/f'bias-{label}.bin');r.update(compare(data,baseline));report['upstream_runs'][label]=r
        if not r['passed']:raise RuntimeError('Fresh correction bias mismatch')
    cases=controls();authored=len(cases);rng=random.Random(200810100445);raw=request.read_bytes();handoff=[];skipped=0
    if len(raw)!=8+6352*220:raise ValueError('Upstream contact framing')
    for index in range(512,6352):
        supplied=raw[8+index*220:8+(index+1)*220];two,disabled,_=struct.unpack_from('<III',supplied);accumulated=struct.unpack_from('<f',supplied,12)[0]
        if abs(accumulated)>10000:skipped+=1;continue
        handoff.append((two,disabled,accumulated,index))
    for start in range(0,len(handoff),32):
        group=handoff[start:start+32];ordered=[v for v in group if v[0]]+[v for v in group if not v[0]]
        # Upstream accumulators are [0,.45], so alternate an authored threshold
        # of 0 (including equality activation) and 1 (the no-active bypass).
        two=sum(v[0] for v in group);c=case(rng,len(group),two,4);c.update(scale=.0001,threshold=float((start//32)%2))
        c['contacts']=[(disabled,accumulated) for _,disabled,accumulated,_ in ordered];cases.append(c)
    source=out/'input.bin';source.write_bytes(pack(cases));commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    original=None;decoded=None
    for label,command in commands:
        data,r=execute(command,source,out/f'{label}.bin')
        if original is None:original=data;decoded=decode(data,cases)
        r.update(compare(data,original));report['runs'][label]=r
    report.update(cases=len(cases),authored_controls=authored,handoff_contacts=len(handoff),handoff_accumulator_bound_skips=skipped,
        constraints=sum(len(c['rows']) for c in cases),input_sha256=digest(source),semantic_checks=semantic_checks(cases,decoded))
    report['activity']=dict(active_cases=sum(active(c) for c in cases),no_active_cases=sum(not active(c) for c in cases),empty_pools=sum(not c['rows'] for c in cases),
        handoff_active_cases=sum(active(c) for c in cases[authored:]),
        handoff_changed_position_cases=sum(any(p['body'][0x1c0:0x1d0]!=q['body'][0x1c0:0x1d0] for p,q in zip(c['poses'],r['poses'])) for c,r in zip(cases[authored:],decoded[authored:])),
        two_body_rows=sum(c['two'] for c in cases),single_body_rows=sum(len(c['rows'])-c['two'] for c in cases),
        body_publications=sum(len(c['poses']) for c in cases if active(c)),shape_publications=sum(len(p['shapes']) for c in cases if active(c) for p in c['poses']),
        changed_position_cases=sum(any(p['body'][0x1c0:0x1d0]!=q['body'][0x1c0:0x1d0] for p,q in zip(c['poses'],r['poses'])) for c,r in zip(cases,decoded)),
        zero_iteration_active_cases=sum(active(c) and c['iterations']==0 for c in cases))
    base=case(random.Random(42),n=1,bodies=1);good=pack([base]);bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:],excess_cases=good[:4]+struct.pack('<I',1025)+good[8:])
    for name,mutate in [
        ('bad_split',lambda c:c.update(two=2)),('too_many_iterations',lambda c:c.update(iterations=33)),
        ('too_many_rows',lambda c:c.update(rows=c['rows']*257,contacts=c['contacts']*257)),
        ('too_many_bodies',lambda c:c.update(poses=c['poses']*17,workspace=c['workspace']*17)),
        ('bad_contact_flag',lambda c:c.update(contacts=[(2,1.)])),('nan_accumulated',lambda c:c.update(contacts=[(0,float('nan'))])),
        ('nan_softness',lambda c:c.update(softness=float('nan'))),('nan_scale',lambda c:c.update(scale=float('nan'))),
        ('nan_threshold',lambda c:c.update(threshold=float('nan'))),('nan_relaxation',lambda c:c.update(relaxation=float('nan'))),
        ('nan_workspace',lambda c:c['workspace'].__setitem__(0,float('nan'))),('zero_batch',lambda c:uput(c['rows'][0],0xb4,0)),
        ('unknown_body',lambda c:uput(c['rows'][0],0xa0,1)),('nan_response',lambda c:fput(c['rows'][0],0x40,float('nan'))),
        ('reversed_bounds',lambda c:fput(c['rows'][0],0x88,10.)),('singular_normalization',lambda c:fput(c['rows'][0],0xac,0.)),
        ('zero_denominator',lambda c:fput(c['rows'][0],0xb0,-c['softness'])),
        ('overflow_bias',lambda c:c.update(contacts=[(0,3e38)],scale=3e38)),
        ('oversized_correction',oversized_correction),
        ('body_pointer',lambda c:words(c['poses'][0]['body'],0x334,[1])),('bad_head',lambda c:c['poses'][0].update(head=2)),
        ('too_many_shapes',lambda c:c['poses'][0].update(shapes=[bytearray(0x220)]*9))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,passed=0<run.returncode<256 and 'impact_correction_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=bool(all(report['semantic_checks'].values()) and all(report['activity'].values()) and all(r['passed'] for r in report['upstream_runs'].values())
        and all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values()))
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole impact correction validation failed; inspect private evidence')


if __name__=='__main__':main()
