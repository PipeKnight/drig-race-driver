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
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');parser.add_argument('--coupled',action='store_true');args=parser.parse_args()
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
    matrices=None
    if args.coupled:
        matrices=ROOT/'local/behavior/tyre-coupling-20261007-01/original.tsv'
        assert digest(matrices)=='c5b6e5157e725c87a319adb8ddc874cf3553e523fabbc51ae51494611783977e'
        assert matrices.read_text().splitlines()==(matrices.parent/'native.tsv').read_text().splitlines()
        # Matrices already exactly matched whole preparation; retain hash/input provenance.
        prior=json.loads((matrices.parent/'validation.json').read_text());assert prior['summary']['exact_cases']==192
        rows=[]
        for line in matrices.read_text().splitlines():
            f=line.split('\t');assert len(f)==174
            v=[struct.unpack('<f',struct.pack('<I',int(x,16)))[0] for x in f[2:]]
            rows.append(v[4:20]+v[44:172]+v[20:36])
        for i,(name,values) in enumerate(cases):
            preload=[(-1 if (i+w)%2 else 1)*.05*(w+1) for w in range(4)]
            cases[i]=(name,values+preload+rows[i%len(rows)])
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{word(v):08x}' for v in values)+'\n' for name,values in cases))
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/tyre-friction-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-tyre-friction.exe',[requests,out/'native.tsv'])]:
        if args.coupled:arguments.append('coupled')
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Tyre execution failed')
    original=(out/'original.tsv').read_text().splitlines();native=(out/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases)
    differences=[]
    for a,b,(name,_) in zip(original,native,cases):
        fa=a.split('\t');fb=b.split('\t');assert fa[1]==fb[1]==name and len(fa)==len(fb)==(54 if args.coupled else 46)
        if a!=b:differences.append(dict(case=name,fields=[dict(index=i,original=x,native=y) for i,(x,y) in enumerate(zip(fa[2:],fb[2:])) if x!=y]))
    report=dict(profile=args.profile,cases=len(cases),wheel_evaluations=len(cases)*4,words=len(cases)*44,exact=len(cases)-len(differences),executions=executions,input_sha256=digest(requests),differences=differences,limits='Whole original9c1f60 active+70c executes, original gravity/contact impulse supplies load; zero spring/damping/coupling/response matrices, differential strengths/brake/drive increments zero. Supplied friction/spin/inertia/slip/clock coefficients. Compared limiter/impulses/weighted accumulators/rotational response and preceding neutral contact state. No source tyre tables, calibrated live inputs, general coupled active step or driving fidelity.')
    if args.coupled:
        report.update(words=len(cases)*52,matrices_path=str(matrices),matrices_sha256=digest(matrices),limits='Whole original9c1f60 active+70c with supplied prepared nine response matrices/secondary contacts, nonzero spring/preload and tyre impulses. Native suspension/normal contacts/tyre limiter/cross-wheel responses compose. Differential/brake/drive increments zero; damping/bump/axle coupling zero here; no full drivetrain/tyre source/synchronized live/driving fidelity.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','wheel_evaluations','words','exact']}));assert not differences

if __name__=='__main__':main()
