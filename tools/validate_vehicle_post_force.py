"""Whole original post-force helpers against Rust, with independently carried state."""
import argparse
import collections
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
REFERENCE=ROOT/'local/compatibility/grid-unpacked.exe'
NATIVE=ROOT/'local/behavior/vehicle-post-force-probe.exe'
MAGIC=b'TA01'
RUST_NAME='grid-vehicle-post-force'
RECORD=0x2604
FIELDS=[0xe1c,0xe20,0xe74,0x15a4,0x17a4,0x17a8,0x185c,0x1894,0x18fc,0x1974,0x1f00,0x1fa0,0x226c,0x2274]
def put(b,o,x):struct.pack_into('<f',b,4+o,x)
def ui(b,o,x):struct.pack_into('<I',b,4+o,x)
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def request(records):return MAGIC+struct.pack('<I',len(records))+b''.join(records)

def cases():
    rng=random.Random(2008176);result=[]
    for i in range(512):
        b=bytearray(rng.randbytes(RECORD))
        struct.pack_into('<f',b,0,[.001,.01,1/60,.033,.1][i%5])
        for o in FIELDS:put(b,o,0)
        for o,x in [(0xe1c,[0,.1,.10001,1][i%4]),(0xe20,[0,.89999,.9,1][i%4]),(0xe74,.3),
                    (0x15a4,[.001,1,3][i%3]),(0x17a4,900),(0x17a8,105),
                    (0x185c,[-1,0,.5,.7,1,2][i%6]),(0x1894,[-10,0,50,99.75,105,106.05,450,900][i%8]),
                    (0x18fc,[0,1,-3][i%3]),(0x1974,[0,.049999,.05,.249999,.25,1][i%6]),
                    (0x1f00,[-10,0,50,100,500][i%5]),(0x1fa0,[0,1000][i%2]),
                    (0x226c,float(i%7==0)),(0x2274,float(i%11==0))]:put(b,o,x)
        ui(b,0x196c,i%4);ui(b,0x21f4,i%4);ui(b,0x1620,4)
        for o,x in [(0x1970,i%2),(0x1971,(i//2)%2),(0x1860,(i//4)%2),(0x1559,(i//8)%2),(0x1f2a,int(i%5!=0))]:b[4+o]=x
        # Authored tilted bases include both assist gates, clipping and signed zeros.
        side=[rng.uniform(-1,1),[-1,-.65,-.50001,-.5,-0.,0,.5,.50001,.6,1][i%10],rng.uniform(-1,1),0]
        forward=[rng.uniform(-1,1),[-1,-.6,-.50001,-.5,0,.5,.50001,.6,1][i%9],rng.uniform(-1,1),0]
        for base,values in [(0x15c0,[0,0,0,0] if i%17==0 else [rng.uniform(-40,40) for _ in range(4)]),
                            (0x15d0,[rng.uniform(-3,3) for _ in range(4)]),(0x15e0,side),(0x15f0,[0,1,0,0]),(0x1600,forward)]:
            for j,x in enumerate(values):put(b,base+j*4,x)
        for j in range(4):
            w=0x1990+j*0x150
            for o,x in [(0,(-1 if j%2==0 else 1)*rng.uniform(.5,1)),(0x40,rng.uniform(-1,1)),
                        (0x138,[-1,0,.5,100,10000][(i+j)%5]),(0x13c,rng.uniform(280,1000))]:put(b,w+o,x)
        result.append(bytes(b))
    for slope in [0.,-0.,1e-38,-1e-38,1e-12,-1e-12,1.,-1.,10.,-10.,1e20,-1e20]:
        b=bytearray(result[17]);ui(b,0x21f4,1);put(b,0x1990,0);put(b,0x1ae0,-1)
        put(b,0x1990+0x40,slope);put(b,0x1ae0+0x40,0);result.append(bytes(b))
    b=bytearray(result[17]);ui(b,0x21f4,0);put(b,0x1990,0);put(b,0x1ae0,0)
    for o in range(0x15c0,0x15d0,4):put(b,o,-0.)
    result.append(bytes(b))
    return result

def execute(out,profile,name,payload,kind,allow=False):
    source=out/f'{name}-{kind}-input.bin';target=out/f'{name}-{kind}-output.bin';source.write_bytes(payload)
    exe=ROOT/f'target/{profile}/{RUST_NAME}.exe' if kind=='rust' else NATIVE
    args=[source,target] if kind=='rust' else [REFERENCE,source,target]
    run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
    report=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,sha256=digest(exe))
    (out/f'{name}-{kind}-execution.json').write_text(json.dumps(report,indent=2))
    if not allow:assert run.returncode==0 and not run.stderr,report
    return (target.read_bytes() if target.exists() else None),report

def run_set(out,profile,name,branches):
    outputs={};runs={}
    for kind,records in branches.items():outputs[kind],runs[kind]=execute(out,profile,name,request(records),kind)
    a,repeat,b=[outputs[k] for k in ['original','repeat','rust']]
    assert len(a)==len(repeat)==len(b)==len(request(branches['original']))
    diff=[dict(case=(i-8)//RECORD,offset=hex((i-8)%RECORD),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
    counts=collections.Counter()
    for i,r in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for label,start,end in [('speed',0x1648,0x164c),('rpm',0x1894,0x1898),('rpm_history',0x1f00,0x1f04),('angular',0x15d0,0x15e0),('brake_timer',0xe74,0xe78)]:counts[label]+=r[4+start:4+end]!=after[4+start:4+end]
        for j in range(4):
            w=4+0x1990+j*0x150
            for label,o in [('camber',0xec),('wheel_response',0x13c)]:counts[label]+=r[w+o:w+o+4]!=after[w+o:w+o+4]
    report=dict(calls=len(branches['original']),bytes=len(a)-8,repeat_exact=a==repeat,differences=diff,coverage=dict(counts),executions=runs)
    (out/f'{name}-validation.json').write_text(json.dumps(report,indent=2))
    assert a==repeat and not diff,dict(differing_words=len(diff),first=diff[:20])
    return report,{k:[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}

def guards(out,profile,first):
    tests=[('zero_delta',0,0),('nan_delta',0,0x7fc00000),('large_delta',0,0x3e800000),('wheel_count',4+0x1620,5)]
    for o in FIELDS+list(range(0x15c0,0x1610,4))+[0x1990,0x1990+0x40,0x1990+0x138,0x1990+0x13c]:tests.append((f'nan_{o:x}',4+o,0x7fc00000))
    tests.extend([('speed_overflow',4+0x15c0,0x7f7fffff),('camber_span',4+0x1ae0,struct.unpack_from('<I',first,4+0x1990)[0])])
    results=[]
    for name,o,x in tests:
        b=bytearray(first);struct.pack_into('<I',b,o,x)
        for kind in ['native','rust']:
            data,run=execute(out,profile,'guard-'+name,request([bytes(b)]),kind,True)
            passed=data is None and run['exit']==(50 if kind=='native' else 1)
            if kind=='native':passed=passed and not run['stderr'] and 'completed=0 code_unchanged=1' in run['stdout']
            results.append(dict(name=name,kind=kind,passed=passed,execution=run))
    for name,payload in [('header',b'BAD!'+request([first])[4:]),('truncated',request([first])[:-1]),('trailing',request([first])+b'!'),('empty',b'TA01'+bytes(4)),('count',b'TA01'+struct.pack('<I',65))]:
        for kind in ['native','rust']:
            data,run=execute(out,profile,'guard-'+name,payload,kind,True)
            results.append(dict(name=name,kind=kind,passed=data is None and 0<run['exit']<80,execution=run))
    b=bytearray(first);struct.pack_into('<f',b,0,.1);put(b,0xe20,3.4028234663852886e38);put(b,0x15a4,1);put(b,0x15c0,10)
    for j in range(4):put(b,0x1990+j*0x150+0x138,10);put(b,0x1990+j*0x150+0x13c,-3.4028234663852886e38)
    for kind in ['native','rust']:
        data,run=execute(out,profile,'guard-late-overflow',request([bytes(b)]),kind,True)
        results.append(dict(name='late_overflow',kind=kind,passed=data is None and run['exit']==(51 if kind=='native' else 1),execution=run))
        target=out/f'guard-existing-{kind}-output.bin';target.write_bytes(b'owned sentinel')
        data,run=execute(out,profile,'guard-existing',request([first]),kind,True)
        results.append(dict(name='existing_output',kind=kind,passed=data==b'owned sentinel' and run['exit']>0,execution=run))
    (out/'guards.json').write_text(json.dumps(results,indent=2));assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    current=cases();reports=[]
    for i in range(0,len(current),64):
        batch=current[i:i+64];r,_=run_set(out,a.profile,f'initial-{i}',{k:batch for k in ['original','repeat','rust']});reports.append(r)
    branches={k:current[1:65] for k in ['original','repeat','rust']}
    for step in range(16):
        r,branches=run_set(out,a.profile,f'carry-{step}',branches);reports.append(r)
    guard_results=guards(out,a.profile,current[3])
    counts=collections.Counter()
    for r in reports:counts.update(r['coverage'])
    assert all(counts[k]>0 for k in ['speed','rpm','rpm_history','angular','brake_timer','camber','wheel_response'])
    report=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),bytes=sum(r['bytes'] for r in reports),guards=len(guard_results),coverage=dict(counts),reference_sha256=digest(REFERENCE),native_sha256=digest(NATIVE),rust_sha256=digest(ROOT/f'target/{a.profile}/grid-vehicle-post-force.exe'),reports=reports,
                scope='Actual post-force caller speed/RPM scheduling, whole990350/990220/996800/9B9500/9BF230. Authored inputs and fixed reference globals; following9BE1E0 and wholeA03670/world/driving not executed.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k!='reports'}))
if __name__=='__main__':main()
