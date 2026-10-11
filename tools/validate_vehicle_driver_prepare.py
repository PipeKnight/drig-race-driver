"""Joined selected pre-suspension driver interval versus whole original helpers."""
import argparse
import collections
import json
import math
import struct
import subprocess
from pathlib import Path
import validate_vehicle_prepared as seeds
import validate_vehicle_propulsion as base

ROOT=base.ROOT
RECORD=base.RECORD_SIZE
BODY=36
VIEW=BODY+0x2600
MATERIAL=VIEW+0x300+0xdd0
NATIVE=ROOT/'local/behavior/vehicle-driver-prepare-probe.exe'
MAGIC=b'DF01'

def cases():
    result=[]
    for i,raw in enumerate(seeds.cases()):
        r=bytearray(raw)
        def number(o,x):struct.pack_into('<f',r,BODY+o,x)
        def integer(o,x):struct.pack_into('<I',r,BODY+o,x)
        for o in [0x24a,0xde4,0xdfc,0x1640]:r[BODY+o]=0
        for o in [0x1970,0x1971]:r[BODY+o]=1
        for o in [0x1108,0x1f78,0x21f2,0xb10,0x18c8,0x1f79,0x21f0]:r[BODY+o]=(i//(1+o%3))%2
        integer(0x2278,0);integer(0x1598,1);integer(0x196c,1+i%9);integer(0x3c0,i%5);integer(0x2e0,i%10);integer(0x1850,i%5);integer(0x1f74,i%2)
        for o,x in [(0x1568,0),(0x159c,0),(0x2e4,.2),(0x1f14,.1),(0x1f34,[0,.5,2][i%3]),(0x1648,[0,2,5,15,40][i%5]),
                    (0x15a0,1200),(0x15a4,1/1200),(0x15a8,600),(0x15ac,1000),(0x15b0,600),(0x17a0,.22),(0x17a4,900),
                    (0x185c,[0,.5,1][i%3]),(0x187c,[0,.3,1][i%3]),(0x1890,100+i%10),(0x1894,[100,200,600][i%3]),
                    (0x1f94,.1),(0x1fa0,[.0001,0,100][i%3]),(0x110c,.3),(0x226c,0),
                    (0x1ee4,.1),(0x1ee8,.2),(0x1eec,0),(0x1ef0,10),
                    (0xe08,1),(0xe0c,2),(0xe10,.5),(0xe14,.5),(0x1f04,[1,5,10][i%3]),(0x1f08,[0,3][i%2]),
                    (0x1f18,[0,.5][i%2]),(0x1f1c,-1),(0x1f64,.4),(0x1f68,.8),(0x139c,.2),(0x21c4,1.2),(0x2214,.4),(0x2268,1),
                    (0x2ec,[0,.5,1,1.5,2][i%5]),(0x2f0,[0,.5,1,1.5,2][(i//5)%5]),(0x2f4,[0,.1,1][i%3]),(0x2f8,.2),
                    (0x2fc,[0,.1,.5][i%3]),(0x300,[0,1,3][i%3])]:number(o,x)
        yaw=(i%11-5)*.1;pitch=(i%7-3)*.1
        side=[math.cos(yaw),0,-math.sin(yaw),0];up=[-math.sin(yaw)*math.sin(pitch),math.cos(pitch),-math.cos(yaw)*math.sin(pitch),0]
        forward=[math.sin(yaw)*math.cos(pitch),math.sin(pitch),math.cos(yaw)*math.cos(pitch),0]
        if i%13==0:up=[-x for x in up]
        for o,row in [(0x15e0,side),(0x15f0,up),(0x1600,forward)]:struct.pack_into('<4f',r,BODY+o,*row)
        velocity=[[-3,0,10,0],[0,0,0,0],[1,2,40,0],[1,-2,-8,0],[0,0,.5,0]][i%5]
        struct.pack_into('<4f',r,BODY+0x15c0,*velocity);struct.pack_into('<4f',r,BODY+0x15d0,0,[0,4,9][i%3],.2,0)
        for j in range(4):
            number(0x1f4c+j*4,(i+j)%40);r[BODY+0x1f48+j]=(i+j)%2
            for o,x in [(0,-.8 if j%2==0 else .8),(8,-1.3 if j<2 else 1.3),(0x78,0 if (i+j)%3==0 else 1),
                        (0x104,(-1 if j%2 else 1)*(i+j)%100),(0x10c,[-1,0,1][(i+j)%3]),(0xe0,.3),(0x9c,i*10),(0xa0,(i%5-2)*100),(0x12c,.005)]:number(0x1990+j*0x150+o,x)
            r[BODY+0x1990+j*0x150+0x130]=(i+j)%2;r[MATERIAL+j*0x140+0x75]=int((i+j)%7==0)
        result.append(bytes(r))
    return result

def request(records):return MAGIC+struct.pack('<I',len(records))+b''.join(struct.pack('<I',i)+r[4:] for i,r in enumerate(records))

def run_set(out,profile,branches,name):
    outputs={};executions=[]
    for kind,records in branches.items():
        source=out/f'{name}-{kind}-input.bin';target=out/f'{name}-{kind}-output.bin';source.write_bytes(request(records))
        exe=ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[source,target] if kind=='rust' else [base.REFERENCE,source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        item=dict(kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,sha256=base.digest(exe));executions.append(item)
        (out/f'{name}-{kind}-execution.json').write_text(json.dumps(item,indent=2));assert run.returncode==0 and not run.stderr,item
        outputs[kind]=target.read_bytes()
    a,repeat,b=[outputs[k] for k in ['original','repeat','rust']];assert len(a)==len(repeat)==len(b)==8+len(branches['original'])*RECORD
    differences=[dict(case=(i-8)//RECORD,offset=hex((i-8)%RECORD),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
    coverage=collections.Counter()
    for i,before in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for label,start,end in [('velocity',0x15c0,0x15e0),('previous_velocity',0xb20,0xb40),('fuel',0x1fa0,0x1fa4),('periodic',0x2ec,0x304),('aero_force',0x1f10,0x1f14),('rpm_history',0x1890,0x1894),('speed_rings',0x290,0x2e0)]:coverage[label]+=before[BODY+start:BODY+end]!=after[BODY+start:BODY+end]
        coverage['friction']+=before[VIEW+0x298:VIEW+0x2a8]!=after[VIEW+0x298:VIEW+0x2a8]
        for j in range(4):
            o=BODY+0x1990+j*0x150+0x104;coverage['wheel_spin']+=before[o:o+4]!=after[o:o+4]
        assert before[BODY+0x15e0:BODY+0x1620]==after[BODY+0x15e0:BODY+0x1620]
        assert before[VIEW+0x300:]==after[VIEW+0x300:]
    report=dict(calls=len(branches['original']),bytes=len(a)-8,repeat_exact=a==repeat,differences=differences,coverage=dict(coverage),executions=executions)
    (out/f'{name}-validation.json').write_text(json.dumps(report,indent=2));assert a==repeat and not differences,dict(differing_words=len(differences),first=differences[:20])
    return report,{kind:[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[kind]))] for kind,data in outputs.items()}

def guards(out,profile,first):
    tests=[('zero_delta',4,0),('nan_delta',4,0x7fc00000),('large_delta',4,0x3e800000)]
    tests += [(name,BODY+offset,value) for name,offset,value in [
        ('disabled',0x24a,1),('disabled_wheel',0x2278,1),('nitro',0xde4,1),('respawn',0xdfc,1),
        ('mode',0x1598,0),('neutral',0x196c,0),('gear',0x196c,10),('clutch_transition',0x1640,1),
        ('automatic_off',0x1970,0),('delta_ring',0x3c0,5),('speed_ring',0x2e0,10),
        ('mass',0x15a0,0),('inverse_mass',0x15a4,0),('inertia',0x15a8,0),('max_rpm',0x17a4,0),
        ('radius',0x1990+0xe0,0),('wheel_contact_nan',0x1990+0x78,0x7fc00000),
        ('velocity_nan',0x15c0,0x7fc00000),('gravity_axis_nan',0x15f4,0x7fc00000),
        ('friction_range',0x1ef0,0),('aero_alignment',0xe14,0x3f800000),
        ('phase_negative',0x2f0,0xbf800000),('phase_large',0x2f0,0x40400000),
        ('gravity_overflow',0x15c4,0x7f7fffff),('wheel_spin_overflow',0x1990+0x104,0x7f800000),
    ]]
    results=[]
    for name,offset,value in tests:
        raw=bytearray(first);struct.pack_into('<I',raw,offset,value)
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(request([bytes(raw)]))
            exe=NATIVE if kind=='original' else ROOT/f'target/{profile}/grid-vehicle-force.exe'
            args=[base.REFERENCE,source,target] if kind=='original' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
            passed=(0<run.returncode<=80 and not run.stderr and 'code_unchanged=1' in run.stdout) if kind=='original' else run.returncode==1
            results.append(dict(name=name,kind=kind,passed=passed and not target.exists(),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
    # Arithmetic failure after earlier writes must also reject, not publish infinities.
    raw=bytearray(first)
    for o,value in [(0x2f4,0),(0x2fc,0),(0x2f8,1),(0x300,1)]:struct.pack_into('<I',raw,BODY+o,value)
    for kind in ['original','rust']:
        source=out/f'guard-periodic-overflow-{kind}-input.bin';target=out/f'guard-periodic-overflow-{kind}-output.bin';source.write_bytes(request([bytes(raw)]))
        exe=NATIVE if kind=='original' else ROOT/f'target/{profile}/grid-vehicle-force.exe'
        args=[base.REFERENCE,source,target] if kind=='original' else [source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        passed=(run.returncode==51 and not run.stderr and 'code_unchanged=1' in run.stdout) if kind=='original' else run.returncode==1
        results.append(dict(name='periodic_overflow',kind=kind,passed=passed and not target.exists(),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
    (out/'guards.json').write_text(json.dumps(results,indent=2));assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);current=cases();reports=[]
    for start in range(0,len(current),48):
        batch=current[start:start+48];r,_=run_set(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}');reports.append(r)
    branches={k:current[1:33] for k in ['original','repeat','rust']}
    for step in range(12):r,branches=run_set(out,a.profile,branches,f'carry-{step}');reports.append(r)
    coverage=collections.Counter()
    for r in reports:coverage.update(r['coverage'])
    checks=guards(out,a.profile,current[1])
    report=dict(profile=a.profile,guards=len(checks),calls=sum(r['calls'] for r in reports),bytes=sum(r['bytes'] for r in reports),coverage=dict(coverage),reports=reports,scope='Selected engaged assisted-automatic earlyA03670 schedule around unchanged original helpers, gravity/drag/downforce included; respawn/nitro inactive, no source suspension/whole driver/world/driving claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k!='reports'}))
if __name__=='__main__':main()
