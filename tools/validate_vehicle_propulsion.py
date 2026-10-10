"""Compare actual post-control engine/differential/traction/force segment."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_vehicle_force import ROOT, RECORD_SIZE, record, authored_records, digest

CAPTURE=ROOT/'local/behavior/wheel-force-live-20261010-01'
NATIVE=ROOT/'local/behavior/vehicle-propulsion-probe.exe'
REFERENCE=ROOT/'local/compatibility/grid-unpacked.exe'


def cases():
    live=[json.loads(p.read_text()) for p in sorted(CAPTURE.glob('case-*.json'))]
    metadata=json.loads((CAPTURE/'capture.json').read_text())
    assert metadata['completed'] and metadata['code_exact'] and metadata['debugger_exit']==0
    result=[record(c,'before') for c in live]
    rng=random.Random(2008171)
    for i,raw in enumerate(authored_records(live)):
        b=bytearray(raw)
        def number(o,x):struct.pack_into('<f',b,36+o,x)
        def integer(o,x):struct.pack_into('<I',b,36+o,x)
        number(0x17a4,900);number(0x17a8,105)
        speeds=[-50,0,20,32,94.5,105,120,450,890,900,1500,1800,1675.5159912109375,1675.51611328125,1675.5162353515625]
        number(0x1894,speeds[i%len(speeds)])
        integer(0x1644,rng.getrandbits(32));integer(0x1988,i%3)
        number(0x185c,[0,.2,.95,1][i%4]);integer(0x1558,(i%2)|(((i//2)%2)<<16))
        number(0x1868,0.03 if i%7==0 else 0);b[36+0x1860]=i%2
        number(0x1864,.7);number(0x1fa0,0 if i%11==0 else rng.uniform(100,3600))
        # Independently varied tables retain their zero-node and interpolation branches.
        if i%3==0:
            for node in range(32):number(0x17ac+node*4,0 if (i+node)%11==0 else rng.uniform(.05,1.5))
        number(0x22c4,1 if i%4==0 else 0);number(0x22c8,rng.uniform(.8,1.2))
        number(0x1898,[-3142,-3141.5927734375,0,3141.5927734375,3142][i%5])
        number(0x189c,rng.uniform(-.02,.02));number(0x18a0,rng.uniform(-.02,.02))
        number(0x18a8,rng.uniform(-.02,.02));number(0x18ac,rng.uniform(-.02,.02))
        integer(0x1588,(i//4)%2);integer(0x164c,i%2)
        for o,x in [(0x1570,.3),(0x1574,5),(0x1578,7),(0x157c,.5),(0x1580,9),(0x1584,11),(0x158c,.01),(0x1590,1 if i%5==0 else 0),(0x1594,2),(0x2264,.85),(0x1f70,.1)]:number(o,x)
        driven=struct.unpack_from('<I',b,36+0x18f0)[0]
        address=struct.unpack_from('<I',b,8)[0]
        ids=[(struct.unpack_from('<I',b,36+0x18d8+j*4)[0]-address-0x1990)//0x150 for j in range(driven)]
        for j in range(4):
            w=0x1990+j*0x150;number(w+0x10c,1/driven if j in ids else 0)
            number(w+0x108,rng.uniform(0,500));number(w+0x110,[-315,-314,0,314,315][(i+j)%5])
            number(w+0x94,rng.uniform(-10,10));number(w+0x98,rng.uniform(-10,10));number(w+0xac,rng.uniform(-10,10))
        result.append(bytes(b))
    return result


def request(records):
    return b'PF01'+struct.pack('<I',len(records))+b''.join(struct.pack('<I',i)+r[4:] for i,r in enumerate(records))


def run_set(out,profile,current,prefix):
    outputs={};runs=[]
    for kind,records in current.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}.bin';source.write_bytes(request(records))
        exe=ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[source,target] if kind=='rust' else [REFERENCE,source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        item=dict(kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,exe_sha256=digest(exe));runs.append(item)
        (out/f'{prefix}-{kind}-execution.json').write_text(json.dumps(item,indent=2))
        assert run.returncode==0,item
        outputs[kind]=target.read_bytes()
    a,repeat,b=[outputs[k] for k in ['original','repeat','rust']]
    assert len(a)==len(repeat)==len(b)==len(request(current['original']))
    differences=[dict(case=(i-8)//RECORD_SIZE,offset=hex((i-8)%RECORD_SIZE),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
    coverage=dict(torque_nonzero=0,rng_changes=0,traction_active=0,body_velocity_changes=0,wheel_spin_changes=0)
    for i,record in enumerate(current['original']):
        start=8+i*RECORD_SIZE;after=a[start:start+RECORD_SIZE];body=36;view=body+0x2600
        assert after[view+0x250:view+0x290]==record[view+0x250:view+0x290]
        assert after[body+0x15a8:body+0x15b4]==record[view+0x48:view+0x54]
        coverage['torque_nonzero']+=struct.unpack_from('<f',after,view+0x58)[0]!=0
        coverage['rng_changes']+=after[body+0x1644:body+0x1648]!=record[body+0x1644:body+0x1648]
        coverage['traction_active']+=bool(after[body+0x198c])
        coverage['body_velocity_changes']+=after[body+0x15c0:body+0x15e0]!=record[body+0x15c0:body+0x15e0]
        for j in range(4):
            o=body+0x1990+j*0x150+0x104;coverage['wheel_spin_changes']+=after[o:o+4]!=record[o:o+4]
    report=dict(calls=len(current['original']),record_bytes=len(current['original'])*RECORD_SIZE,repeat_exact=a==repeat,differing_words=len(differences),differences=differences,coverage=coverage,executions=runs,hashes={k:hashlib.sha256(v).hexdigest() for k,v in outputs.items()})
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2))
    return report,outputs


def guards(out,profile,first):
    mutations=[('identity',0,9,10),('nan_delta',4,0x7fc00000,11),('zero_delta',4,0,11),
        ('mode',36+0x1598,0,11),('axes',36+0xadc,1,11),('wheel_count',36+0x1620,5,11),
        ('drive_count',36+0x18f0,5,11),('gear',36+0x196c,16,11),('view_pointer',36,0,11),
        ('solver_pointer',36+0x2600,0,11),('overlap',36+0x2600+0x2b0,0xffffffff,11),
        ('wheel_owner',36+0x1990+0x34,0,12),('presence',20,0,12),('foreign_driven',36+0x18d8,1,13),
        ('duplicate_driven',36+0x18dc,struct.unpack_from('<I',first,36+0x18d8)[0],13),
        ('zero_scale',36+0x1f5c,0,30),('zero_rpm_limit',36+0x1f98,0,30),
        ('huge_speed',36+0x1894,0x7f7fffff,30),('huge_phase',36+0x1898,0x7f7fffff,30),
        ('traction_mode',36+0x1988,3,30),('material_roll',36+0x2600+0x300+0xdd0+0x20,0x7fc00000,30)]
    gear=struct.unpack_from('<I',first,36+0x196c)[0]
    mutations.append(('ratio',36+0x18f4+gear*4,0x7fc00000,31))
    mutations.append(('inconsistent_material_alias',36+0x2600+0x300+0xdd0+0x20,0x7fc00000,12))
    for o in [0x17a0,0x17a4,0x17a8,0x1848,0x1854,0x185c,0x1864,0x1868,0x1870,0x1874,
              0x187c,0x1880,0x1884,0x1888,0x188c,0x1894,0x1898,0x189c,0x18a0,0x18a4,0x18a8,0x18ac,
              0x17ac,0x1828,0x1570,0x1580,0x1f70,0x22c8,0x1990+0x108,0x1990+0x110]:
        mutations.append((f'nan_{o:x}',36+o,0x7fc00000,30))
    payloads=[]
    for name,o,value,expected in mutations:
        b=bytearray(first);struct.pack_into('<I',b,o,value)
        if name=='material_roll':
            pointer=struct.unpack_from('<I',first,36+0x1990+0xc0)[0]
            for j in range(1,4):
                if struct.unpack_from('<I',first,36+0x1990+j*0x150+0xc0)[0]==pointer:
                    struct.pack_into('<I',b,36+0x2600+0x300+0xdd0+j*0x140+0x20,value)
        payloads.append((name,request([bytes(b)]) if o!=0 else b'PF01'+struct.pack('<I',1)+bytes(b),expected))
    payloads.extend([('header',b'BAD!'+struct.pack('<I',1)+first,7),('empty',b'PF01'+bytes(4),7),
                     ('oversize',b'PF01'+struct.pack('<I',65),7),('truncated',request([first])[:-1],10)])
    results=[]
    for name,payload,expected in payloads:
        source=out/f'guard-{name}.bin';source.write_bytes(payload)
        for kind in ['native','rust']:
            target=out/f'guard-{name}-{kind}-out.bin';exe=NATIVE if kind=='native' else ROOT/f'target/{profile}/grid-vehicle-force.exe'
            args=[REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(expected if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and 'completed=0 code_unchanged=1' in run.stdout and not run.stderr
            results.append(dict(name=name,kind=kind,exit=run.returncode,passed=passed,stdout=run.stdout,stderr=run.stderr))
    # Exclusive outputs must preserve existing files even when valid input executes.
    source=out/'guard-existing-input.bin';source.write_bytes(request([first]))
    for kind in ['native','rust']:
        target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel')
        exe=NATIVE if kind=='native' else ROOT/f'target/{profile}/grid-vehicle-force.exe'
        args=[REFERENCE,source,target] if kind=='native' else [source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        results.append(dict(name='existing_output',kind=kind,exit=run.returncode,passed=run.returncode==(8 if kind=='native' else 1) and target.read_bytes()==b'owned sentinel'))
    (out/'guards.json').write_text(json.dumps(results,indent=2))
    assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    current=cases();reports=[]
    for start in range(0,len(current),48):
        batch=current[start:start+48];report,_=run_set(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}',);reports.append(report)
        if not report['repeat_exact'] or report['differences']:break
    assert all(r['repeat_exact'] and not r['differences'] for r in reports)
    # Each implementation carries its own full output, without copying native state to Rust.
    carry={k:current[6:38] for k in ['original','repeat','rust']};carried=[]
    for step in range(12):
        r,outputs=run_set(out,a.profile,carry,f'carry-{step}');carried.append(r)
        if not r['repeat_exact'] or r['differences']:break
        carry={k:[v[8+i*RECORD_SIZE:8+(i+1)*RECORD_SIZE] for i in range(32)] for k,v in outputs.items()}
    guard_results=guards(out,a.profile,current[0])
    report=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),repeat_exact=all(r['repeat_exact'] for r in reports),differing_words=sum(r['differing_words'] for r in reports),reports=reports,
                carried_calls=sum(r['calls'] for r in carried),carried_record_bytes=sum(r['record_bytes'] for r in carried),carried=carried,guards=len(guard_results),
                scope='Authored actual post-control A03670 segment around whole9B1B90/996290/990060/9DF8B0; inherited later-boundary observations are supplied inputs, not live captures of this segment. Controls/contact geometry remain supplied; no complete vehicle/world/driving claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','calls','record_bytes','repeat_exact','differing_words']}))
    assert report['calls']==len(current) and report['repeat_exact'] and report['differing_words']==0
    assert report['carried_calls']==384 and all(r['repeat_exact'] and not r['differences'] for r in carried)


if __name__=='__main__':main()
