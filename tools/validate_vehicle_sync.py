"""Whole linked getters/mass/control synchronization, including independent carry."""
import argparse
import collections
import json
import random
import struct
import subprocess
from pathlib import Path
import validate_vehicle_driver as driver
from validate_vehicle_propulsion import ROOT,REFERENCE,digest

CAR=0xd240;PHYSICAL=0x424;RECORD=CAR+PHYSICAL
NATIVE=ROOT/'local/behavior/vehicle-sync-probe.exe'
SETTINGS=[0xd198,0xd1ac,0xd1b0,0xce04,0xd1c8,0xd1e0,0xd1f8,0xd210,0xd19c,0xd1c4,0xd1dc,0xd1f4,0xd20c,0xd1bc,0xd1d4,0xd1ec,0xd204,0xd1a8,0xd194,0xd1a0,0xd1a4]


def cases():
    rng=random.Random(2008181);seeds=driver.cases();result=[]
    for i in range(180):
        r=bytearray(struct.pack('<I',rng.getrandbits(32))* (RECORD//4));raw=seeds[i%len(seeds)]
        r[:0x2600]=raw[36:36+0x2600]
        def num(o,x):struct.pack_into('<f',r,o,x)
        def ui(o,x):struct.pack_into('<I',r,o,x)
        for o in SETTINGS:num(o,rng.uniform(-2,2))
        for o,x in [(0xfa8,rng.uniform(300,3000)),(0xfac,rng.uniform(1,20000)),(0xfb0,rng.uniform(1,20000)),(0xfb4,rng.uniform(1,20000)),(0x1560,rng.uniform(.1,3))]:num(o,x)
        for o in [0xdb0,0xc714,0xc718,0xc71c]:ui(o,0)
        ui(0xc7ec,i%10);ui(0xd234,7 if i%2 else i%12)
        for j in range(10):num(0xc7c4+j*4,[0,.49,.5,3,3.01,4][(i+j)%6] if i%3 else 0)
        for j in range(4):num(0xc7b4+j*4,rng.uniform(0,2))
        r[0xc7b1]=i%2;r[0xb13]=i%2;r[0xb14]=(i//2)%2
        for j in range(8):r[0x22e0+j]=(i+j)%3
        r[0xd1b4]=[0,1,255][i%3]
        for j in range(4):r[[0xd1cc,0xd1e4,0xd1fc,0xd214][j]]=(i+j)%3
        for target,source,size in [(0x1d0,0x15e0,64),(0x210,0x15d0,16),(0x220,0x15c0,16)]:r[CAR+target:CAR+target+size]=r[source:source+size]
        for base in [0x210,0x220]:
            for j in range(3):
                value=[0,0x80000000,0x29e12e12,0x29e12e13,0xa9e12e12,0x3f800000][(i+j//(1+i%2))%6]
                struct.pack_into('<I',r,CAR+base+j*4,value)
            num(CAR+base+12,[-3,0,1,7][i%4])
        # Old synchronized pose/velocity are discarded before any arithmetic.
        num(0x1610,struct.unpack_from('<f',r,CAR+0x200)[0]+10)
        if i%8==0:
            for o in [0x15c0,0x15d0,0x15e0]:ui(o,0x7fc00000)
        result.append(bytes(r))
    return result


def request(records):return b'VS01'+struct.pack('<I',len(records))+b''.join(records)


def compare(out,profile,branches,prefix):
    outputs={};runs=[]
    for kind,records in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}.bin';source.write_bytes(request(records))
        exe=ROOT/f'target/{profile}/grid-vehicle-sync.exe' if kind=='rust' else NATIVE
        args=[source,target] if kind=='rust' else [REFERENCE,source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        item=dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr);runs.append(item)
        (out/f'{prefix}-{kind}-execution.json').write_text(json.dumps(item,indent=2));assert run.returncode==0 and not run.stderr,item
        if kind!='rust':assert 'code_unchanged=1 globals_restored=1 error=0' in run.stdout
        outputs[kind]=target.read_bytes();assert len(outputs[kind])==8+len(records)*RECORD
    a,repeat,rust=[outputs[k] for k in ['original','repeat','rust']]
    differences=[dict(case=(o-8)//RECORD,offset=hex((o-8)%RECORD),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(8,len(a),4) if a[o:o+4]!=rust[o:o+4]]
    coverage=collections.Counter()
    for i,r in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for label,o,n in [('pose_copy',0x15e0,64),('mass',CAR+0x3f0,8),('inverse_inertia',CAR+0x150,64),('strain_ring',0xc7c4,44),('settings',0x2264,0x6c)]:coverage[label]+=r[o:o+n]!=after[o:o+n]
        coverage['strain_activation']+=after[0xc7b1]!=0 and not r[0xc7b1]
        coverage['strain_reset']+=after[0xc7b1]==0 and r[0xc7b1]!=0
        for p,v in [(0x210,0x15d0),(0x220,0x15c0)]:
            before=r[CAR+p:CAR+p+12];coverage['tiny_cleanup']+=before!=after[v:v+12] and after[v:v+12]==bytes(12)
            assert r[CAR+p+12:CAR+p+16]==after[v+12:v+16]
    report=dict(calls=len(branches['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert report['repeat_exact'] and not differences,dict(first=differences[:30],count=len(differences))
    return report,{k:[v[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[k]))] for k,v in outputs.items()}


def guards(out,profile,first):
    checks=[]
    tests=[('foreign_wrapper',0xdb0,1,30),('ring_index',0xc7ec,10,30),('active_grid',0xc714,2,30),('nan_pose',CAR+0x1d0,0x7fc00000,30),('nan_linear',CAR+0x220,0x7fc00000,30),('nan_strain',0xc7c4,0x7fc00000,30),('nan_settings',0xd198,0x7fc00000,31),('nan_scale',0x1560,0x7fc00000,31),('zero_mass',0xfa8,0,32),('zero_inertia',0xfac,0,32),('huge_inertia',0xfac,0x7f7fffff,32),('tiny_inertia',0xfac,1,33)]
    for name,o,w,expected in tests:
        r=bytearray(first);struct.pack_into('<I',r,o,w);payload=request([bytes(r)])
        for kind in ['native','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}.bin';source.write_bytes(payload)
            exe=NATIVE if kind=='native' else ROOT/f'target/{profile}/grid-vehicle-sync.exe';args=[REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(expected if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and not run.stderr and 'completed=0 code_unchanged=1' in run.stdout
            checks.append(dict(name=name,kind=kind,exit=run.returncode,passed=passed,stdout=run.stdout,stderr=run.stderr))
    valid=request([first])
    for name,payload,expected in [('header',b'BAD!'+valid[4:],7),('empty',b'VS01'+bytes(4),7),('high_count',b'VS01'+struct.pack('<I',65),7),('truncated',valid[:-1],10),('trailing',valid+b'x',23)]:
        for kind in ['native','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}.bin';source.write_bytes(payload)
            exe=NATIVE if kind=='native' else ROOT/f'target/{profile}/grid-vehicle-sync.exe';args=[REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(expected if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and not run.stderr and 'code_unchanged=1' in run.stdout
            checks.append(dict(name=name,kind=kind,exit=run.returncode,passed=passed,stdout=run.stdout,stderr=run.stderr))
    for kind in ['native','rust']:
        source=out/f'guard-existing-{kind}-input.bin';target=out/f'guard-existing-{kind}.bin';source.write_bytes(valid);target.write_bytes(b'owned sentinel')
        exe=NATIVE if kind=='native' else ROOT/f'target/{profile}/grid-vehicle-sync.exe';args=[REFERENCE,source,target] if kind=='native' else [source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        checks.append(dict(name='existing_output',kind=kind,exit=run.returncode,passed=run.returncode==(8 if kind=='native' else 1) and target.read_bytes()==b'owned sentinel'))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']]
    return checks


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);records=cases();reports=[]
    for start in range(0,len(records),48):
        batch=records[start:start+48];r,_=compare(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}');reports.append(r)
    carry={k:records[:16] for k in ['original','repeat','rust']}
    for step in range(12):
        r,carry=compare(out,a.profile,carry,f'carry-{step}');reports.append(r)
    coverage=collections.Counter()
    for r in reports:coverage.update(r['coverage'])
    checks=guards(out,a.profile,records[0]);result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),reports=reports,coverage=dict(coverage),guards=len(checks),scope='Whole9BD7F0 linked getters/tiny motion cleanup/strain history/diagonal mass-inverse/control settings. Deformation dimensions zero; no A03220/whole car/world claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='reports'}))


if __name__=='__main__':main()
