"""Original horizontal rejection helper and cache side effect on supplied inputs."""
import argparse, hashlib, json, random, struct, subprocess
from pathlib import Path
from validate_candidate_grid import f32, word
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def expected(v,plane):
    start=v[:2];direction=v[2:4];center=v[4:6];half=v[6:8];origin=[f32(start[i]-center[i]) for i in range(2)]
    near=0.;far=1.;initial=plane
    for _ in range(4):
        axis=plane//2;sign=-1. if plane%2 else 1.;delta=f32(direction[axis]*sign);boundary=f32(f32(f32(origin[axis]*sign)*-1.)-half[axis])
        if delta<=0:
            if delta>=0:
                if boundary>0:return [0,plane]
            else:
                if f32(delta*near)<boundary:return [0,plane]
                if f32(delta*far)<boundary:far=f32(boundary/delta)
        else:
            if f32(delta*far)<boundary:return [0,plane]
            if f32(delta*near)<boundary:near=f32(boundary/delta)
        plane=(plane+1)%4
    return [1,initial]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    cases=[]
    def add(name,values,plane):
        values=list(map(f32,values));cases.append((name,list(map(word,values))+[plane],expected(values,plane)))
    controls=[([0.,0.],[0.,0.]),([1.,0.],[0.,1.]),([-1.,0.],[0.,-1.]),([2.,0.],[0.,1.]),([-2.,0.],[0.,1.]),([0.,2.],[1.,0.]),([0.,-2.],[-1.,0.]),([-2.,0.],[2.,2.]),([-2.,0.],[4.,0.]),([2.,2.],[-4.,-4.]),([1.000000119,0.],[0.,1.]),([-2.,1.],[4.,-0.])]
    for plane in range(4):
        for i,(start,direction) in enumerate(controls):add(f'analytic_{plane}_{i}',start+direction+[0.,0.,1.,1.],plane)
    rng=random.Random(2008)
    for i in range(192):
        center=[rng.uniform(-2000,2000) for _ in range(2)];half=[rng.uniform(.001,100) for _ in range(2)];start=[center[j]+half[j]*rng.uniform(-3,3) for j in range(2)];direction=[half[j]*rng.uniform(-6,6) for j in range(2)]
        add(f'synthetic_{i}',start+direction+center+half,i%4)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words,_ in cases))
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/rectangle-cull-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-rectangle-cull.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Rectangle comparison failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            a=line.split('\t');assert len(a)==4 and a[0]=='P' and a[1] not in result;result[a[1]]=[int(w,16) for w in a[2:]]
        return result
    original=rows(out/'original.tsv');native=rows(out/'native.tsv');assert set(original)==set(native)=={c[0] for c in cases}
    diffs=[dict(name=name,original=original[name],native=native[name],independent=reference) for name,_,reference in cases if not original[name]==native[name]==reference]
    report=dict(profile=args.profile,cases=len(cases),words=2*len(cases),exact=len(cases)-len(diffs),executions=executions,differences=diffs,limits='Whole original rectangle culling and cached rejection plane, supplied finite geometry. No packed-node traversal or complete shape/world query claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','words','exact']}));assert not diffs
if __name__=='__main__':main()
