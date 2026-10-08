"""Whole original wheel history/grip with supplied state, zero wheel angle/table off."""
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
    # 19 globals, 17 wheel scalars/flags and five four-lane vectors.
    baseline=[.016,0.,8.,1140.,300.,.5,1.,.3,.6,.2,1.,1.,1.,0.,1.,.3,1.,1.,1.,
              1.2,.003,3.5,.8,32.,.3,1.3,.6,.05,.02,1.,1.,0.,.2,.3,.5,2.]
    vectors=[.8,3.,1.5,0.,1.,0.,0.,0.,0.,0.,1.,0.,0.,1.,0.,0.,1.,0.,0.,0.]
    cases=[]
    def add(name,values):
        assert len(values)==56;cases.append((name,[word(v) for v in values]))
    for i in range(24):
        v=baseline.copy()+vectors.copy()
        if i<4:v[18]=0.;v[19]=.2;v[20]=[4.99,5.,5.01,-1.][i];v[32:35]=[1.2,-.3,1.3]
        if i==4:v[0]=0.
        if 5<=i<=7:v[19]=[.4999999701976776,.5,.5000000596046448][i-5]
        if i>=8:v[10]=float(i%2);v[11]=float(i%3==0);v[12]=float(i%4!=0);v[13]=float(i%2)
        if i>=12:v[23]=[-10.,0.,12.,100.][i%4];v[27:29]=[0.,0.];v[8]=0. if i%3 else .5
        if i>=16:v[48:52]=[1.,0.,0.,0.];v[9]=0.5
        if i>=20:v[48:52]=[.8,.6,0.,0.];v[9]=[.59,.6,.61,.99][i-20]
        add(f'analytic_{i}',v)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
    assert digest(source)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    _,root,_=decode(source.read_bytes())
    for i,q in enumerate(root.findall('MATERIAL/MECHANICS')):
        v=baseline.copy()+vectors.copy();a=q.attrib
        v[14]=float(a.get('wear','1'));v[15:18]=[float(a.get(k,d)) for k,d in [('grip_per_ms',0),('ms_for_max_effect',1),('grip',1)]]
        v[13]=float(a.get('dirt_build_up','false').lower()=='true');add(f'material_{i}',v)
    rng=random.Random(108)
    for i in range(256):
        v=baseline.copy()+vectors.copy()
        v[:10]=[rng.uniform(0,.08),rng.uniform(0,2),rng.uniform(0,40),rng.uniform(500,2000),rng.uniform(.05,1000),rng.uniform(0,1),rng.uniform(0,2),rng.uniform(0,1),rng.uniform(0,1),rng.uniform(0,.9)]
        v[10:15]=[float(i%2),float(i%3!=0),float(i%5!=0),float(i%2),rng.uniform(0,2)]
        v[15:19]=[rng.uniform(-2,2),rng.uniform(0,10),rng.uniform(-2,2),float(i%4!=0)]
        v[19:30]=[rng.uniform(0,1),rng.uniform(-.01,5.1),rng.uniform(-2,2),rng.uniform(0,2),rng.uniform(-100,100),rng.uniform(.2,.6),rng.uniform(0,2),rng.uniform(0,2),rng.uniform(-.1,.1),rng.uniform(-.1,.1),rng.uniform(-1,1)]
        v[30:36]=[float(i%2),float(i%3==0),rng.uniform(-.2,1.2),rng.uniform(-.2,1.2),rng.uniform(-.2,1.2),float(i%3)]
        v[36:48]=[rng.uniform(-10,10) for _ in range(4)]+[rng.uniform(-1,1) for _ in range(8)]
        x=rng.uniform(-.99,.99);v[48:52]=[x,(1-x*x)**.5,0.,0.]
        add(f'authored_{i}',v)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words in cases))
    executions=[]
    for kind,exe,arguments in [
        ('original',ROOT/'local/behavior/wheel-history-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-wheel-history.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Wheel history run failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==11 and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native.keys()=={n for n,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,executions=executions,cases=len(cases),words=len(cases)*9,
                exact_cases=len(cases)-len({d['case'] for d in differences}),differences=differences,
                source_sha256=digest(source),limits='Whole original 9ba490 with original 99c990 and zero-angle sin/cos calls. '
                'Supplied body/wheel/force-view vectors and mechanics; table inactive/factor1. '
                'No patches/stubs/writable pages/live injection. x87 precision53/nearest. '
                'Opaque material stamp normalized to caller tokens. No nonzero-angle/table-enabled/live force integration claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','cases','words','exact_cases')}));print('different_words='+str(len(differences)))
    assert not differences


if __name__=='__main__':main()
