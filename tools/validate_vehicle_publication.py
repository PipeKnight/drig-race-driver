"""Whole A03220 owned car/physical comparison, and reusable publication boundary."""
import argparse
import collections
import json
import random
import struct
from pathlib import Path
from validate_vehicle_propulsion import ROOT, REFERENCE, digest
import validate_vehicle_contact_force as execution

CAR=0x21000;PHYSICAL=0x424;RECORD=4+CAR+PHYSICAL
NATIVE=ROOT/'local/behavior/vehicle-publication-probe.exe'


def request(rows):return b'VP01'+struct.pack('<I',len(rows))+b''.join(rows)


def configure(car):
    # Authored stable setup; no setup-rebuild or visual/UI objects.
    for o in [0xdb0,0xdb8,0xdbc,0xdc0,0xdc4,0xdd4,0xc7a0,0x1554]:struct.pack_into('<I',car,o,0)
    car[4]=0;car[0xc7f2]=0;car[0x20fb4]=1
    struct.pack_into('<I',car,0x1620,4);struct.pack_into('<I',car,0xba0,2)
    return car


def compare(out,profile,branches,prefix):
    outputs={};runs=[]
    for kind,rows in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin'
        source.write_bytes(request(rows));exe=ROOT/f'target/{profile}/grid-vehicle-publication.exe' if kind=='rust' else NATIVE
        args=[source,target] if kind=='rust' else [REFERENCE,source,target]
        run=execution.execute(exe,args,out,f'{prefix}-{kind}');runs.append(run)
        if kind!='rust':assert 'code_unchanged=1 globals_restored=1 error=0' in run['stdout']
        outputs[kind]=target.read_bytes();assert len(outputs[kind])==8+len(rows)*RECORD
    a,repeat,rust=[outputs[k] for k in ['original','repeat','rust']]
    differences=[dict(case=(o-8)//RECORD,offset=hex((o-8)%RECORD),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(8,len(a),4) if a[o:o+4]!=rust[o:o+4]]
    coverage=collections.Counter()
    for i,row in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for name,o,n in [('timer',0xc798,4),('decay',0xc790,4),('decay_state',0xc780,8),('linear_w',0x15cc,4),('angular_w',0x15dc,4)]:coverage[name]+=row[4+o:4+o+n]!=after[4+o:4+o+n]
        coverage['physical_velocity']+=row[4+CAR+0x210:4+CAR+0x230]!=after[4+CAR+0x210:4+CAR+0x230]
        coverage['zero_delta']+=struct.unpack_from('<f',row)[0]==0
    report=dict(calls=len(branches['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert a==repeat and not differences,dict(count=len(differences),first=differences[:30])
    return report,{k:[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}


def cases():
    rng=random.Random(2008186);rows=[]
    for i in range(128):
        car=configure(bytearray(rng.randbytes(CAR)));physical=bytearray(rng.randbytes(PHYSICAL))
        if i%2:
            struct.pack_into('<I',car,0x1554,17);struct.pack_into('<I',car,0xba0,1);car[0x20fb4]=0
        for o,x in [(0xc798,[-1,-0.,0,.001,.016,.1,1][i%7]),(0xc790,[-2,0,.001,.0019999999,.002,.0020000003,1,2][i%8])]:struct.pack_into('<f',car,o,x)
        for base in [0x15c0,0x15d0]:
            struct.pack_into('<3f',car,base,*[rng.uniform(-100,100) for _ in range(3)])
            struct.pack_into('<I',car,base+12,[0,0x80000000,0x7fc00000,0x7f800000,0xff800000,0xffffffff][i%6])
        rows.append(struct.pack('<f',[0,.001,.016,.04,.1][i%5])+car+physical)
    return rows


def guards(out,profile,first):
    mutations=[('nan_delta',0,0x7fc00000),('negative_delta',0,0xbf800000),('large_delta',0,0x3e800000)]
    mutations += [(name,4+o,value) for name,o,value in [('foreign_wrapper',0xdb0,1),('wheel_count',0x1620,5),('render_wheel',0xdb8,1),('render_body',0xdd4,1),('visual_update',4,1),('ui',0xc7a0,1),('setup_rebuild',0xc7f0,0x10000),('setup_mode',0xba0,1),('setup_flag',0x20fb4,0),('nan_timer',0xc798,0x7fc00000),('nan_decay',0xc790,0x7fc00000),('nan_velocity',0x15c0,0x7fc00000)]]
    checks=[]
    for name,o,value in mutations:
        row=bytearray(first);struct.pack_into('<I',row,o,value)
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(request([row]))
            exe=ROOT/f'target/{profile}/grid-vehicle-publication.exe' if kind=='rust' else NATIVE
            args=[source,target] if kind=='rust' else [REFERENCE,source,target]
            run=execution.execute(exe,args,out,f'guard-{name}-{kind}',True)
            passed=(0<run['exit']<=32 and not run['stderr'] and 'completed=0 code_unchanged=1 globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(x['passed'] for x in checks),[x for x in checks if not x['passed']];return checks


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows=cases();reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),32):
        batch=rows[start:start+32];r,_=compare(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}');reports.append(r);coverage.update(r['coverage'])
    branches={k:rows[16:32] for k in ['original','repeat','rust']}
    for frame in range(8):r,branches=compare(out,a.profile,branches,f'carry-{frame}');reports.append(r);coverage.update(r['coverage'])
    checks=guards(out,a.profile,rows[0]);result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),coverage=dict(coverage),guards=len(checks),reports=reports,scope='Whole A03220 with owned21000car/424physical, stable setup, empty manager/render/UI, notification0, linked velocity setters; no pose setter/advance/correction/world claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='reports'}))
if __name__=='__main__':main()
