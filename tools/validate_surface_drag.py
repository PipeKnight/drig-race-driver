"""Compare whole original surface-drag body/spin updates with supplied contacts."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    identity=[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]
    body=[.016,10.,1140.,1500.,2000.,1300.]+identity+[10.,1.,2.,0.,.1,.2,-.3,0.]
    wheels=[]
    for i in range(4):wheels.extend([[-.8,-.3,-1.2,0.],[.8,-.3,-1.2,0.],[-.8,-.3,1.3,0.],[.8,-.3,1.3,0.]][i]+[30.,.3,.02,1.,0.,3.,20.,.4])
    cases=[]
    def add(name,v):assert len(v)==78;cases.append((name,[word(x) for x in v]))
    for i in range(24):
        v=body.copy()+wheels.copy();v[1]=[0.,2.99,3.,5.,6.,20.][i%6]
        if i<4:
            for w in range(4):v[37+w*12]=float(w==i)
        if 4<=i<8:
            for w in range(4):v[38+w*12]=1.
        if 8<=i<12:
            for w in range(4):v[34+w*12]=[-20.,-0.,0.00001,30.][w]
        if 12<=i<16:
            for w in range(4):v[37+w*12]=0.
        if i>=16:
            v[0]=0. if i%2 else .08
            for w in range(4):v[38+w*12]=float(w%2);v[36+w*12]=-.5
        add(f'analytic_{i}',v)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
    assert digest(source)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    _,root,_=decode(source.read_bytes())
    for i,q in enumerate(root.findall('MATERIAL/MECHANICS')):
        v=body.copy()+wheels.copy();v[1]=[0.,10.,20.,40.,60.][i%5]
        a=q.attrib;coeffs=[float(a.get(k,d)) for k,d in [('minimumSpeedForSlowdown',20),('speedForMaximumSlowdown',40),('slowdown',0)]]
        for w in range(4):v[39+w*12:42+w*12]=coeffs
        add(f'material_{i}',v)
    rng=random.Random(112)
    for i in range(512):
        v=body.copy()+wheels.copy();v[:6]=[rng.uniform(0,.08),rng.uniform(0,60),rng.uniform(500,2000),*[rng.uniform(500,3000) for _ in range(3)]]
        if i>=128:v[6:22]=[rng.uniform(-1,1) for _ in range(16)]
        v[22:30]=[rng.uniform(-10,10) for _ in range(8)]
        for w in range(4):
            k=30+w*12;minimum=rng.uniform(0,20)
            v[k:k+12]=[rng.uniform(-1.5,1.5),rng.uniform(-.5,.5),rng.uniform(-1.5,1.5),rng.uniform(-.3,.3),rng.uniform(-30,30),rng.uniform(.2,.6),rng.uniform(-.2,.2),float(i%5!=0 or w%2==0),float(i%3==0),minimum,minimum+rng.uniform(.1,25),rng.uniform(-.1,1.)]
        add(f'authored_{i}',v)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words in cases))
    executions=[]
    for kind,exe,arguments in [
        ('original',ROOT/'local/behavior/surface-drag-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-surface-drag.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Surface drag run failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==14 and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native.keys()=={n for n,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,executions=executions,cases=len(cases),words=len(cases)*12,
                exact_cases=len(cases)-len({d['case'] for d in differences}),differences=differences,source_sha256=digest(source),
                limits='Whole four-wheel original 9dfc50 including original cross/impulse helpers; no code patches/stubs/live injection. '
                'Private lazy normalization data page writable/restored. Supplied contacts/relative levers/body/speed/additive scalars. '
                '84 source MECH slowdown sets independently decoded; original source loader not invoked here. '
                'Source flag73 initialized false; authored true controls do not establish live activation. '
                'No main cadence/contact generation/full driving equivalence claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','cases','words','exact_cases')}));print('different_words='+str(len(differences)))
    assert not differences
if __name__=='__main__':main()
