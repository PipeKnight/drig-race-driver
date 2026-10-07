"""Whole active wheel substep with neutral differential/brake/cross-wheel responses."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value):return struct.unpack('<I',struct.pack('<f',value))[0]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    cases=[]
    # Pure first/second slips, limiter boundary and low/high normal-load scales.
    for i in range(36):
        wheels=[]
        for w in range(4):
            radius=.3;state=[0.,.015,.03,.045,-.06,1.][i%6]
            lateral=[0.,.1,1.,-1.,10.,-.01][(i//6+w)%6]
            wheels.extend([0. if i<6 else .2,.3,[0.,1.,-1.,20.][w],1.+w,[0.,.0001,.1,1.,100.,-1.][i//6],.75,lateral,state,0.,.8])
        cases.append((f'analytic_{i}',[.001,[0.,.5,1.][i%3],[0.,.25,1.][(i//3)%3]]+wheels))
    for boundary in [word(.1),word(1.)]:
        for offset in [-1,0,1]:
            for sign in [1.,-1.]:
                value=struct.unpack('<f',struct.pack('<I',boundary+offset))[0]*sign
                state,lateral=(value,0.) if boundary==word(.1) else (0.,value)
                wheel=[.2,1.,0.,1.,0.,10.,lateral,state,0.,1.]
                cases.append((f'boundary_{boundary:08x}_{offset+1}_{int(sign>0)}',[.001,1.,.25]+wheel*4))
    rng=random.Random(99)
    for i in range(192):
        wheels=[]
        for w in range(4):
            wheels.extend([rng.uniform(0,5),rng.uniform(.1,.8),rng.uniform(-30,30),rng.uniform(.25,4),rng.uniform(-.1,10),rng.uniform(.1,3),rng.uniform(-2,2),rng.uniform(-.2,.2),rng.uniform(-10,10),rng.uniform(0,2)])
        cases.append((f'synthetic_{i}',[rng.uniform(.0002,.01),rng.uniform(-.5,1.5),rng.uniform(0,1)]+wheels))
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{word(v):08x}' for v in values)+'\n' for name,values in cases))
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/tyre-friction-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-tyre-friction.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Tyre execution failed')
    original=(out/'original.tsv').read_text().splitlines();native=(out/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases)
    differences=[]
    for a,b,(name,_) in zip(original,native,cases):
        fa=a.split('\t');fb=b.split('\t');assert fa[1]==fb[1]==name and len(fa)==len(fb)==46
        if a!=b:differences.append(dict(case=name,fields=[dict(index=i,original=x,native=y) for i,(x,y) in enumerate(zip(fa[2:],fb[2:])) if x!=y]))
    report=dict(profile=args.profile,cases=len(cases),wheel_evaluations=len(cases)*4,words=len(cases)*44,exact=len(cases)-len(differences),executions=executions,input_sha256=digest(requests),differences=differences,limits='Whole original9c1f60 active+70c executes, original gravity/contact impulse supplies load; zero spring/damping/coupling/response matrices, differential strengths/brake/drive increments zero. Supplied friction/spin/inertia/slip/clock coefficients. Compared limiter/impulses/weighted accumulators/rotational response and preceding neutral contact state. No source tyre tables, calibrated live inputs, general coupled active step or driving fidelity.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','wheel_evaluations','words','exact']}));assert not differences

if __name__=='__main__':main()
