"""Whole chassis-motion publication vs Rust with complete opaque body preservation."""
import argparse
import collections
import json
import math
import random
import struct
from pathlib import Path
import validate_vehicle_post_force as base

ROOT=base.ROOT
RECORD=base.RECORD
put=base.put
ui=base.ui

def cases():
    rng=random.Random(2008177);result=[]
    for i in range(320):
        b=bytearray(rng.randbytes(RECORD));delta=[.001,.01,1/60,.033,.05][i%5];struct.pack_into('<f',b,0,delta)
        for start,end in [(0x50,0x24c),(0xb20,0xb30),(0x1494,0x14d0),(0x15c0,0x15d0),(0x15e0,0x1610),(0x1630,0x1640)]:
            for o in range(start,end,4):put(b,o,0)
        for o,x in [(0x1494,[-2,0,8,1000][i%4]),(0x1498,10),(0x149c,15),(0x14a0,[0,.005,.03][i%3]),
                    (0x14a4,[0,.004,-.02][i%3]),(0x14a8,30),(0x14ac,5),(0x14b0,.04),(0x14b4,.005),
                    (0x14b8,[.5,1,2][i%3]),(0x14bc,[.5,1,2][(i//3)%3]),(0x14c0,1),(0x14c4,-1),
                    (0x14c8,[0,.5,1,6][i%4]),(0x14cc,[0,.5,1,6][(i//4)%4]),
                    (0x60,[0,.2,59.9,60,59.99999][i%5]),(0x64,[0,.2,59.9,60,59.99999][(i//5)%5]),
                    (0x13d0,.05),(0x13d4,.04),(0x1648,[0,5,20,50][i%4]),(0x1638,.1)]:put(b,o,x)
        for j in range(61):
            put(b,0x68+j*4,math.sin((j+i)*.1))
            put(b,0x158+j*4,math.cos((j+i)*.1))
        # Adjacent table endpoint is intentionally shared/owned at phase exactly 60.
        for j in range(4):
            velocity=rng.uniform(-20,20);put(b,0x15c0+j*4,velocity);put(b,0xb20+j*4,velocity+rng.uniform(-.5,.5));put(b,0x50+j*4,rng.uniform(-1,1))
        yaw=rng.uniform(-1,1);pitch=rng.uniform(-.2,.2)
        side=[math.cos(yaw),0,-math.sin(yaw),0];forward=[math.sin(yaw)*math.cos(pitch),math.sin(pitch),math.cos(yaw)*math.cos(pitch),0]
        if i%17==0:side[3]=.01;forward[3]=-.02
        for start,row in [(0x15e0,side),(0x15f0,[0,1,0,0]),(0x1600,forward)]:
            for j,x in enumerate(row):put(b,start+j*4,x)
        rear,front=[(-.1,-.1),(-.1,.2),(.2,-.1),(.1,.1),(.15,.35),(.4,.15)][i%6]
        for j in range(4):
            w=0x1990+j*0x150
            for o,x in [(0,-.85 if j%2==0 else .85),(8,-1.3 if j<2 else 1.3),(0x40,(rear if j<2 else front)+rng.uniform(-.02,.02))]:put(b,w+o,x)
        result.append(bytes(b))
    return result

def run_set(out,profile,name,branches):
    outputs={};runs={}
    for kind,records in branches.items():outputs[kind],runs[kind]=base.execute(out,profile,name,base.request(records),kind)
    a,repeat,b=[outputs[k] for k in ['original','repeat','rust']]
    assert len(a)==len(repeat)==len(b)==8+len(branches['original'])*RECORD
    diff=[dict(case=(i-8)//RECORD,offset=hex((i-8)%RECORD),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
    counts=collections.Counter()
    for i,r in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for label,start,end in [('matrix',0x10,0x50),('acceleration',0x50,0x60),('phases',0x60,0x68)]:counts[label]+=r[4+start:4+end]!=after[4+start:4+end]
        counts['translation_nonzero']+=any(struct.unpack_from('<f',after,4+0x40+j*4)[0]!=0 for j in range(3))
        assert after[4+0x15e0:4+0x1620]==r[4+0x15e0:4+0x1620]
    report=dict(calls=len(branches['original']),bytes=len(a)-8,repeat_exact=a==repeat,differences=diff,coverage=dict(counts),executions=runs)
    (out/f'{name}-validation.json').write_text(json.dumps(report,indent=2))
    assert a==repeat and not diff,dict(differing_words=len(diff),first=diff[:20])
    return report,{k:[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}

def guards(out,profile,first):
    fields=[0x50,0x60,0x64,0x68,0x248,0xb20,0x13d0,0x13d4,*range(0x1494,0x14d0,4),0x15c0,0x15e0,0x1600,0x1638,0x1648,0x1990,0x1998,0x19d0]
    tests=[('zero_delta',0,0),('nan_delta',0,0x7fc00000),('tiny_delta',0,1),('large_delta',0,0x3e800000)]
    tests.extend((f'nan_{o:x}',4+o,0x7fc00000) for o in fields)
    tests.extend([('speed_span',4+0x14a8,struct.unpack_from('<I',first,4+0x14ac)[0]),('noise_span',4+0x14c0,struct.unpack_from('<I',first,4+0x14c4)[0]),
                  ('negative_cap',4+0x1498,0xbf800000),('wheel_width',4+0x1ae0,struct.unpack_from('<I',first,4+0x1990)[0]),
                  ('wheel_base',4+0x1c38,struct.unpack_from('<I',first,4+0x1998)[0]),('phase_negative',4+0x60,0xc1200000),('phase_large',4+0x60,0x43480000)])
    results=[]
    for name,o,x in tests:
        r=bytearray(first);struct.pack_into('<I',r,o,x)
        for kind in ['native','rust']:
            data,run=base.execute(out,profile,'guard-'+name,base.request([bytes(r)]),kind,True)
            passed=data is None and run['exit']==(50 if kind=='native' else 1)
            if kind=='native':passed=passed and not run['stderr'] and 'completed=0 code_unchanged=1' in run['stdout']
            results.append(dict(name=name,kind=kind,passed=passed,execution=run))
    r=bytearray(first)
    for o,x in [(0x1494,100),(0x15c0,1e30),(0x14a0,3.4028234663852886e38)]:put(r,o,x)
    for kind in ['native','rust']:
        data,run=base.execute(out,profile,'guard-late-overflow',base.request([bytes(r)]),kind,True)
        passed=data is None and run['exit']==(51 if kind=='native' else 1)
        if kind=='native':passed=passed and not run['stderr'] and 'completed=0 code_unchanged=1' in run['stdout']
        results.append(dict(name='late_overflow',kind=kind,passed=passed,execution=run))
    payload=base.request([first])
    for name,data in [('bad_header',b'BAD!'+payload[4:]),('truncated',payload[:-1]),('trailing',payload+b'!'),
                      ('zero_count',base.MAGIC+bytes(4)),('too_many',base.MAGIC+struct.pack('<I',65))]:
        data,run=base.execute(out,profile,'guard-'+name,data,'rust',True)
        results.append(dict(name=name,kind='rust-framing',passed=data is None and run['exit']==1,execution=run))
    target=out/'guard-existing-rust-output.bin';target.write_bytes(b'owned sentinel')
    data,run=base.execute(out,profile,'guard-existing',payload,'rust',True)
    results.append(dict(name='existing_output',kind='rust-output',passed=data==b'owned sentinel' and run['exit']==1,execution=run))
    (out/'guards.json').write_text(json.dumps(results,indent=2));assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    base.NATIVE=ROOT/'local/behavior/vehicle-chassis-probe.exe';base.MAGIC=b'MA01';base.RUST_NAME='grid-vehicle-chassis'
    current=cases();reports=[]
    for start in range(0,len(current),64):
        batch=current[start:start+64];r,_=run_set(out,a.profile,f'initial-{start}',{k:batch for k in ['original','repeat','rust']});reports.append(r)
    branches={k:current[1:33] for k in ['original','repeat','rust']}
    for step in range(16):
        r,branches=run_set(out,a.profile,f'carry-{step}',branches);reports.append(r)
    g=guards(out,a.profile,current[1]);counts=collections.Counter()
    for r in reports:counts.update(r['coverage'])
    assert all(counts[k]>0 for k in ['matrix','acceleration','phases','translation_nonzero'])
    report=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),bytes=sum(r['bytes'] for r in reports),guards=len(g),coverage=dict(counts),native_sha256=base.digest(base.NATIVE),rust_sha256=base.digest(ROOT/f'target/{a.profile}/{base.RUST_NAME}.exe'),reports=reports,
                scope='Whole original9BE1E0/9BDF30/67C200 versus independent Rust; authored finite body state, bounded phases including60; no whole driver/world/driving claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k!='reports'}))
if __name__=='__main__':main()
