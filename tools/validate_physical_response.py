"""Whole normal-row normalization/iteration and correction-to-response handoff.

Constraints, batch/body roles and timing are authored. No live-frame claim.
"""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_steering_control import digest
from validate_contact_bias import PROBE as BIAS_PROBE, compare

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31524e50
PROBE = '34c4e9b65c422ae421ea7a1598217080f9d99fb1a85bdadc05024ffc3cbed598'


def fput(row, offset, value): struct.pack_into('<f', row, offset, value)
def uput(row, offset, value): struct.pack_into('<I', row, offset, value)
def fget(row, offset): return struct.unpack_from('<f', row, offset)[0]


def row_for(rng, a, b):
    row = bytearray(struct.pack('<48f', *(rng.uniform(-2, 2) for _ in range(48))))
    for offset,value in [(0x84,.1),(0x88,-10.),(0x8c,10.),(0x90,rng.uniform(-20,20)),
                         (0x94,rng.uniform(-1,1)),(0x98,-5.),(0x9c,.2),(0xac,1.),(0xb0,2.)]:
        fput(row,offset,value)
    for offset,value in [(0xa0,a),(0xa4,b),(0xa8,0),(0xb4,1)]: uput(row,offset,value)
    return row


def case(rows,two,bodies,iterations=4,normalize=1,relaxation=1.):
    return dict(rows=rows,two=two,bodies=bodies,iterations=iterations,normalize=normalize,
                relaxation=relaxation,initial=[.125]*(bodies*8))


def simple_row(bias, lower=0., upper=2.):
    r=bytearray(192)
    for offset,value in [(0x88,lower),(0x8c,upper),(0x90,bias),(0xac,1.),(0xb0,1.)]: fput(r,offset,value)
    uput(r,0xb4,1)
    return r


def controls():
    rng=random.Random(200810100346)
    rows=[case([],0,2)]
    r=simple_row(2.,upper=100.);fput(r,0,1.);fput(r,0x40,1.)
    rows.append(case([r],0,1,iterations=2))
    r=simple_row(3.);s=simple_row(1.);uput(r,0xb4,2)
    for c in [r,s]:
        for i in range(8): fput(c,0x40+i*4,1.)
    rows.append(case([r,s],0,1,iterations=1))
    r=simple_row(3.);uput(r,0xa4,1);fput(r,0x40,1.);fput(r,0x60,-1.)
    rows.append(case([r],1,2,iterations=2))
    r=copy.deepcopy(r);uput(r,0xa4,0)
    rows.append(case([r],1,1,iterations=1))
    r=simple_row(1.);rows.append(case([r],0,1))
    for index in range(512):
        n=256 if index==511 else rng.choice([0,1,2,3,7,16,31,64])
        two=rng.randrange(n+1);bodies=rng.choice([1,2,4,8,16,64]);constraints=[]
        for start,end in [(0,two),(two,n)]:
            cursor=start
            while cursor<end:
                length=rng.randint(1,min(4,end-cursor));a=rng.randrange(bodies);b=rng.randrange(bodies)
                for j in range(length):
                    r=row_for(rng,a,b)
                    uput(r,0xb4,length if j==0 else rng.randrange(5))
                    if index%13==0: r[:128]=bytes(128)
                    if index%13==1: fput(r,0x90,-0.);fput(r,0x94,-0.)
                    if index%13==2: fput(r,0x88,0.);fput(r,0x8c,0.)
                    if index%13==3: fput(r,0x90,100.)
                    if index%13==4: fput(r,0x90,-100.)
                    if index%13==5:
                        for i in range(16): fput(r,i*4,2**-126)
                    constraints.append(r)
                cursor+=length
        rows.append(case(constraints,two,bodies,iterations=rng.choice([0,1,2,4,8,32]),
                         normalize=index%2,relaxation=rng.choice([0.,.5,1.,1.5,-.5])))
    return rows


def pack(cases):
    parts=[struct.pack('<II',MAGIC,len(cases))]
    for c in cases:
        parts.append(struct.pack('<5If',len(c['rows']),c['two'],c['bodies'],c['iterations'],c['normalize'],c['relaxation']))
        parts.extend(c['rows']);parts.append(struct.pack('<'+'f'*len(c['initial']),*c['initial']))
    return b''.join(parts)


def decode(data,cases):
    if data[:8]!=struct.pack('<II',MAGIC,len(cases)): raise ValueError('Output header')
    cursor=8;result=[]
    for c in cases:
        n,b=struct.unpack_from('<II',data,cursor);cursor+=8
        if n!=len(c['rows']) or b!=c['bodies']: raise ValueError('Output shape')
        rows=[data[cursor+i*192:cursor+(i+1)*192] for i in range(n)];cursor+=n*192
        bodies=list(struct.unpack_from('<'+'f'*(b*8),data,cursor));cursor+=b*32
        result.append(dict(rows=rows,bodies=bodies))
    if cursor!=len(data):raise ValueError('Trailing/missing output')
    return result


def execute(command, source, target):
    run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=90)
    if run.returncode: raise RuntimeError(f'Execution {run.returncode}: {run.stdout} {run.stderr}')
    return target.read_bytes(),dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)


def semantic_checks(cases,decoded):
    return dict(empty_clears=decoded[0]['bodies']==[0.]*16,
                repeated_single_equilibrium=decoded[1]['bodies']==[2.,0.,0.,0.,0.,0.,0.,0.] and fget(decoded[1]['rows'][0],0x94)==2.,
                batch_prefix=[fget(r,0x98) for r in decoded[2]['rows']]==[2.,3.] and decoded[2]['bodies']==[3.]*8,
                opposite_two_body=decoded[3]['bodies']==[2.,0.,0.,0.,0.,0.,0.,0.,-2.,0.,0.,0.,0.,0.,0.,0.],
                aliased_body_second_wins=decoded[4]['bodies']==[-2.,0.,0.,0.,0.,0.,0.,0.],
                zero_response_no_motion=decoded[5]['bodies']==[0.]*8,
                zero_iterations_clear=all(all(v==0. for v in d['bodies']) for c,d in zip(cases,decoded) if c['iterations']==0),
                zero_iterations_preserve_cumulative=all(all(r[0x98:0x9c]==before[0x98:0x9c] for r,before in zip(d['rows'],c['rows'])) for c,d in zip(cases,decoded) if c['iterations']==0))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('bias_evidence',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--profile',choices=('debug','release'),required=True);args=p.parse_args()
    previous=private(args.bias_evidence);prior=json.loads((previous/'validation.json').read_text())
    if not (prior['passed'] and prior['analysis_sha256']==ANALYSIS and prior['probe_sha256']==BIAS_PROBE and prior['cases']==6352 and prior['authored_controls']==512):
        raise ValueError('Unverified correction evidence')
    for name in ['input.bin','original.bin','repeat.bin','rust.bin','expected.bin']:
        if (previous/name).stat().st_size>16*1024*1024:raise ValueError('Correction evidence bound')
    baseline=(previous/'original.bin').read_bytes()
    if any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust','expected']):raise ValueError('Correction evidence mismatch')
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/physical-response-probe.exe'
    bias=ROOT/'local/behavior/contact-bias-probe.exe';rust=ROOT/f'target/{args.profile}/grid-physical-response.exe'
    bias_rust=ROOT/f'target/{args.profile}/grid-contact-bias.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE or digest(bias)!=BIAS_PROBE:raise ValueError('Reference/probe identity')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    report=dict(live_verification=False,profile=args.profile,
        scope='Whole B7A810 normal-row normalization then B791B0 iterative body response, original BF3B70 memory clear. Authored constraint batches and correction-row handoff, no tangent rows/full solver/contact creation/live cadence claim.',
        analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),bias_probe_sha256=BIAS_PROBE,
        bias_rust_sha256=digest(bias_rust),bias_input_sha256=digest(previous/'input.bin'),
        upstream_runs={},runs={},guards={})
    for label,command in [('original',[str(bias),str(image)]),('rust',[str(bias_rust)])]:
        data,r=execute(command,previous/'input.bin',out/f'bias-{label}.bin');r.update(compare(data,baseline));report['upstream_runs'][label]=r
        if not r['passed']:raise RuntimeError('Fresh correction outputs changed')
    cases=controls();control_count=len(cases);rng=random.Random(200810100400)
    request=(previous/'input.bin').read_bytes()
    if len(request)!=8+6352*220 or len(baseline)!=8+6352*200:raise ValueError('Correction framing')
    handoff=[]
    for index in range(512,6352):
        supplied=request[8+index*220:8+(index+1)*220];two,=struct.unpack_from('<I',supplied)
        r=bytearray(baseline[8+index*200+8:8+(index+1)*200])
        for offset,value in [(0x84,.1),(0x94,0.),(0x98,-5.),(0x9c,0.),(0xac,1.),(0xb0,2.)]:fput(r,offset,value)
        # Physical roles/denominators are authored; preserve computed bias,
        # limits and all correction-updated response-vector bits.
        a=rng.randrange(4);b=rng.randrange(4)
        for offset,value in [(0xa0,a),(0xa4,b),(0xa8,0),(0xb4,1)]:uput(r,offset,value)
        handoff.append((two,r))
    for start in range(0,len(handoff),32):
        group=handoff[start:start+32];two_rows=[r for two,r in group if two];single=[r for two,r in group if not two]
        cases.append(case(two_rows+single,len(two_rows),4,iterations=4,normalize=1,relaxation=.5))
    source=out/'input.bin';source.write_bytes(pack(cases));original=None;decoded=None
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    for label,command in commands:
        data,r=execute(command,source,out/f'{label}.bin')
        if original is None:original=data;decoded=decode(data,cases)
        r.update(compare(data,original));report['runs'][label]=r
    report.update(cases=len(cases),authored_controls=control_count,handoff_contacts=len(handoff),
                  constraints=sum(len(c['rows']) for c in cases),semantic_checks=semantic_checks(cases,decoded))
    report['activity']=dict(two_body_rows=sum(c['two'] for c in cases),
        single_body_rows=sum(len(c['rows'])-c['two'] for c in cases),
        zero_iterations=sum(c['iterations']==0 for c in cases),
        normalization_cases=sum(c['normalize']!=0 for c in cases),
        multibatch_cases=sum(any(struct.unpack_from('<I',r,0xb4)[0]>1 for r in c['rows']) for c in cases),
        body_response_cases=sum(any(v!=0 for v in d['bodies']) for d in decoded),
        clamped_rows=sum(fget(r,0x94) in [fget(r,0x88),fget(r,0x8c)] for d in decoded for r in d['rows']))
    base=case([row_for(random.Random(42),0,1)],1,2)
    good=pack([base]);bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:],
                            excess_cases=good[:4]+struct.pack('<I',1025)+good[8:])
    for name,key,value in [('too_many_rows','rows',None),('too_many_bodies','bodies',65),('too_many_iterations','iterations',33),
                           ('invalid_normalize','normalize',2),('invalid_split','two',2),('nan_relaxation','relaxation',float('nan'))]:
        c=copy.deepcopy(base)
        if key=='rows':c[key]=c['rows']*257
        else:c[key]=value
        if key=='bodies':c['initial']=[.125]*(65*8)
        bad[name]=pack([c])
    for name,offset,value,integer in [('zero_batch',0xb4,0,True),('over_batch',0xb4,2,True),('unknown_body',0xa0,2,True),
        ('unknown_second',0xa4,2,True),('nan_coefficient',0,float('nan'),False),('infinite_response',0x40,float('inf'),False),
        ('nan_bias',0x90,float('nan'),False),('zero_normalization',0xac,0.,False),('zero_denominator',0xb0,-.1,False),
        ('reversed_limits',0x88,100.,False)]:
        c=copy.deepcopy(base);(uput if integer else fput)(c['rows'][0],offset,value);bad[name]=pack([c])
    c=copy.deepcopy(base);c['initial'][0]=float('nan');bad['nan_initial_workspace']=pack([c])
    c=copy.deepcopy(base);c['rows']*=2;uput(c['rows'][0],0xb4,2);c['rows'][1]=copy.deepcopy(c['rows'][1]);uput(c['rows'][1],0xa0,1);c['two']=2
    bad['inconsistent_batch_bodies']=pack([c])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);report['guards'][name]={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,
                passed=0<run.returncode<256 and 'physical_response_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=bool(all(report['semantic_checks'].values()) and all(report['activity'].values())
        and all(r['passed'] for r in report['upstream_runs'].values())
        and all(r['passed'] for r in report['runs'].values())
        and all(r['passed'] for g in report['guards'].values() for r in g.values()))
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Physical response validation failed; inspect private evidence')


if __name__=='__main__':main()
