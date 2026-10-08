"""Whole downstream grip multiplier with analytic, retained-history and seeded inputs."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def value(w):return struct.unpack('<f',struct.pack('<I',int(w,16)))[0]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    baseline=[0.,1.,1.,.2,.3,.5,.5,.8,.1,.4,1.,1.3]
    cases=[]
    def add(name,v):
        assert len(v)==12;cases.append((name,[word(x) for x in v]))
    for ordinal in range(4):
        for flags in range(4):
            for i,z in enumerate([-1.,-1e-45,-0.,0.,1e-45,1.]):
                v=baseline.copy();v[:3]=[float(ordinal),float(flags&1),float(flags>>1)];v[10]=z
                add(f'boundary_{ordinal}_{flags}_{i}',v)
    # Feed the verified calculator's actual output histories into this later stage.
    retained=ROOT/'local/behavior/wheel-angle-20261008-01/original.tsv'
    assert digest(retained)=='1c589c15b6d175e80088d62924c1f78a5efbca4ff65d3cee324cbf027fde0350'
    histories=[]
    for i,line in enumerate(retained.read_text().splitlines()):
        cells=line.split('\t');assert len(cells)==11
        v=baseline.copy();v[0]=float(i%4);v[1]=float(i%2);v[2]=float(i%3!=0)
        v[3:6]=[value(x) for x in cells[6:9]];v[10]=[-1.,0.,1.][i%3];v[11]=value(cells[10])
        add(f'computed_{i}',v);histories.append(v[3:6])
    rng=random.Random(110)
    for i in range(512):
        v=[float(i%4),float(i%2),float(i%3!=0),*[rng.uniform(-2,2) for _ in range(3)],
           rng.uniform(-5,5),rng.uniform(-2,2),rng.uniform(-2,2),rng.uniform(-2,2),rng.uniform(-3,3),rng.uniform(-3,3)]
        add(f'authored_{i}',v)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words in cases))
    executions=[]
    for kind,exe,arguments in [
        ('original',ROOT/'local/behavior/grip-history-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-grip-history.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('History grip run failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==4 and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native.keys()=={n for n,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,executions=executions,cases=len(cases),words=len(cases)*2,
                exact_cases=len(cases)-len({d['case'] for d in differences}),differences=differences,
                retained_history_sha256=digest(retained),
                limits='Whole original 98db40, no patches/stubs/writable mapped pages/live injection. '
                'Supplied body scalars/flags and per-wheel Z/history. Retained histories computed offline, not live. '
                'Returned factor and multiplication of supplied already-rounded incoming grip. '
                'No other a00e10 grip multiplier, source initialization or full force-application claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','cases','words','exact_cases')}));print('different_words='+str(len(differences)))
    assert not differences

if __name__=='__main__':main()
