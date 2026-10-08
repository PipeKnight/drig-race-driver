"""Whole original assisted coefficient writer with actual feedback/math helpers."""
import argparse
import hashlib
import itertools
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    base=[.7,0.,5.,.3,.5,1.6,2.6,1.,1.,0.,.016,3.,2.]
    for i in range(4):base+=[.8 if i%2==0 else -.8,0.,0.,0.,1.,0.,1.,0.,.9,0.]
    base += [1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,1000.,-2000.,3000.,1.]
    base += [2.,0.,10.,0.,0.,.1,0.,0.,.8,.3,1.3,1.,-.8,.3,1.3,1.]
    base += [.1,0.,0.,2.,10.,100.,.02,.3,.1,10.,0.,0.,0.,1.,0.,0.,0.,10.,1.,1.]
    base += [0.]*4
    cases=[]
    def add(name,row):assert len(row)==109;cases.append((name,[word(v) for v in row]))
    for i,(command,deadzone,shape,linear) in enumerate(itertools.product([-1.,-.5,-0.,0.,.5,1.],[0.,.3,.999,1.],[-2.,0.,1.],[0.,1.])):
        v=base.copy();v[0],v[85],v[86],v[98]=command,deadzone,shape,linear
        add(f'shaping_{i}',v)
    for i in range(128):
        v=base.copy();v[2]=[0.,1.,2.,2.0000002,3.,4.,10.,20.][i%8]
        v[87:90]=[.2,2.,10.];v[90]=[1.,5.,100.][i%3]
        v[95:97]=[.8,.1] if i&8 else [0.,0.]
        v[97]=.15 if i&16 else 0.;v[98]=float(bool(i&32))
        v[99:105]=[.1,.4,0.,20.,.5,1.3] if i&64 else [0.,0.,0.,20.,.5,1.3]
        v[105:]=[-.1,.1,-.15,.15]
        v[69:73]=[[2.,0.,10.,0.],[-2.,0.,-10.,0.],[8.,1.,2.,1.],[-8.,-1.,2.,-1.]][i%4]
        add(f'scale_gate_slew_{i}',v)
    rng=random.Random(121)
    pairs=list(itertools.permutations(range(4),2))
    for i in range(2048):
        v=base.copy();v[:11]=[rng.uniform(-1.2,1.2),rng.uniform(-1.,2.),rng.uniform(0.,25.),rng.uniform(-100.,100.),
                              rng.uniform(.2,.8),rng.uniform(1.,2.5),rng.uniform(2.,4.),rng.uniform(0.,1.2),rng.uniform(0.,1.2),float(i%2),rng.uniform(0.,.1)]
        v[11:13]=pairs[i%12]
        for w in range(4):
            k=13+w*10;v[k:k+10]=[rng.uniform(-1.,1.),rng.uniform(-.1,.1),rng.uniform(-.1,.1),float(bool(i&(1<<w))),
                                  rng.uniform(.7,.9),rng.uniform(-.2,.2),rng.uniform(.7,1.),rng.uniform(-.5,.5),rng.uniform(0.,1.),rng.uniform(-.3,.3)]
        v[69:77]=[rng.uniform(-10.,10.) for _ in range(8)]
        v[77:85]=[rng.uniform(-2.,2.) for _ in range(8)]
        v[85:105]=[rng.uniform(0.,1.),rng.uniform(-3.,3.),rng.uniform(.1,.8) if i%3 else 0.,2.,15.,rng.uniform(0.,20.),
                    rng.uniform(0.,.1),rng.uniform(-1.,1.),rng.uniform(-1.,1.),rng.uniform(1.,20.),
                    rng.uniform(.5,1.5) if i&1 else 0.,rng.uniform(0.,.3),rng.uniform(0.,.3),float(bool(i&2)),
                    rng.uniform(.1,.5) if i&4 else 0.,rng.uniform(.5,1.) if i&4 else 0.,0.,20.,rng.uniform(.5,1.5),rng.uniform(.1,2.)]
        v[105:]=[rng.uniform(-.3,.3) for _ in range(4)]
        if i>=1024:
            v[53:69]=[rng.uniform(-1.,1.) for _ in range(16)]
            v[65:69]=[rng.uniform(-1e5,1e5) for _ in range(4)]
        add(f'authored_{i}',v)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in row)+'\n' for name,row in cases))
    executions=[]
    for kind,exe,argv in [
        ('original',ROOT/'local/behavior/assisted-steering-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-assisted-steering.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Assisted steering execution failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert len(cells)==22 and cells[0]=='P' and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native.keys()=={name for name,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,cases=len(cases),words=len(cases)*20,exact_cases=len(cases)-len({d['case'] for d in differences}),differences=differences,
                executions=executions,input_sha256=digest(requests),
                limits='Whole original9b3110 e6c=true, real98f790/9ae5a0/cross/pow/acos/trig. RX code; only private lazy normalization data page writable/restored, no code patches/stubs/live injection. '
                       'Supplied body/wheel/motion/tuning/delta, bounded finite cases. Source initialization, real device input, live cadence/full driving remain open.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ('profile','cases','words','exact_cases')}));print('different_words='+str(len(differences)));assert not differences
if __name__=='__main__':main()
