"""Whole original candidate collector on authored bounds, grid cells and IDs.
No original world population, shape hits, material tokens or live boundary claim.
"""
import argparse, hashlib, json, random, struct, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def expected(values,capacity,seed,exponent,active):
    def coordinate(p,axis):
        n=f32(p-values[axis]);d=f32(values[axis+2]-values[axis]);q=f32(f32(n/d)*65536.)
        return max(0,min(65535,int(q)))
    a=[coordinate(values[4+i],i) for i in range(2)];b=[coordinate(values[6+i],i) for i in range(2)]
    ids=[];width=2**exponent;step=2048//width
    if active and capacity:
        for tz in range(a[1]//2048,b[1]//2048+1):
            for tx in range(a[0]//2048,b[0]//2048+1):
                az=max(0,a[1]//step-tz*width);bz=min(width-1,b[1]//step-tz*width)
                ax=max(0,a[0]//step-tx*width);bx=min(width-1,b[0]//step-tx*width)
                for cz in range(az,bz+1):
                    for cx in range(ax,bx+1):
                        key=tx*13+tz*7+cx*5+cz*11+seed
                        token=0 if key%5==0 else key%(seed%31+1)+1
                        if token and token not in ids:
                            if len(ids)==capacity:return a+b+[len(ids)]+ids
                            ids.append(token)
    return a+b+[len(ids)]+ids
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    cases=[]
    def add(name,bounds,low,high,capacity,seed,exponent,active=1):
        values=list(map(f32,bounds+low+high));words=list(map(word,values))+[capacity,seed,exponent,active]
        cases.append((name,words,expected(values,capacity,seed,exponent,active)))
    # Exact edges, clamp outside world, empty state, capacity and fine-cell seams.
    for e in range(4):
        for i,(low,high) in enumerate([([0.,0.],[32.,32.]),([-10.,-10.],[-1.,-1.]),([31.,31.],[40.,40.]),([1.,1.],[1.,1.]),([.125,.5],[2.,2.]),([0.,0.],[0.,0.]),([32.,32.],[32.,32.]),([.99999994,.99999994],[1.00000012,1.00000012])]):
            add(f'edge_{e}_{i}',[0.,0.,32.,32.],low,high,[25,1,2,0,25,25,25,25][i],73,e)
        add(f'inactive_{e}',[0.,0.,32.,32.],[0.,0.],[32.,32.],25,73,e,0)
    rng=random.Random(20081007)
    for i in range(180):
        x,z=rng.uniform(-1000,1000),rng.uniform(-1000,1000);w,h=rng.uniform(10,3000),rng.uniform(10,3000)
        xs=sorted([x+w*rng.uniform(-.2,1.2),x+w*rng.uniform(-.2,1.2)])
        zs=sorted([z+h*rng.uniform(-.2,1.2),z+h*rng.uniform(-.2,1.2)])
        add(f'synthetic_{i:03d}',[x,z,x+w,z+h],[xs[0],zs[0]],[xs[1],zs[1]],rng.randrange(26),rng.randrange(1001),i%4)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words,_ in cases))
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/candidate-grid-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-candidate-grid.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Candidate-grid comparison failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            a=line.split('\t');assert a[0]=='P' and a[1] not in result
            result[a[1]]=[int(w,16) for w in a[2:]]
        return result
    original=rows(out/'original.tsv');native=rows(out/'native.tsv');assert set(original)==set(native)=={c[0] for c in cases}
    diffs=[dict(name=name,original=original[name],native=native[name],independent=reference) for name,_,reference in cases if not original[name]==native[name]==reference]
    words=sum(len(r) for _,_,r in cases)
    report=dict(profile=args.profile,cases=len(cases),exact=len(cases)-len(diffs),words=words,executions=executions,differences=diffs,limits='Whole original collector and original coordinate/subdivision helpers on authored grid; no stubs, live injection, original grid population, shape intersection or material ownership claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','exact','words']}));assert not diffs
if __name__=='__main__':main()
