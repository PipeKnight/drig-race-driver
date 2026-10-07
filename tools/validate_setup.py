"""Bounded suspension-subset comparison of the unstubbed whole setup consumer."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value): return f'{struct.unpack("<I",struct.pack("<f",value))[0]:08x}'

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()): raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    cases=[]
    rng=random.Random(2008)
    for i in range(200):
        upper=[1.]*30;lower=[0.]*30;sliders=[.5]*30
        # Inert ancillary range controls remain finite; gearing curve span is
        # zero in the supplied body, so the original tail does not query it.
        for j in (4,5):
            lower[j]=rng.uniform(1000,20000);upper[j]=lower[j]*rng.uniform(1.1,2)
        for j in (6,7,8,9,10):
            lower[j]=rng.uniform(50,1000);upper[j]=lower[j]*rng.uniform(1.1,2)
        lower[11]=.001;upper[11]=rng.uniform(.1,600)
        lower[12]=-.15;upper[12]=.25
        for j in (28,29):lower[j]=1000;upper[j]=10000
        sliders=[rng.uniform(0,1) for _ in range(30)]
        if i<12:
            sliders=[(i%3)*.5]*30
            if i==3:sliders[0]=-1.
            if i==4:sliders[28]=-1.
        base_damper=[rng.uniform(100,1000) for _ in range(2)]
        threshold=[400.,400.] if i<12 else [rng.uniform(.01,600) for _ in range(2)]
        ratio=[rng.uniform(.1,.8) for _ in range(2)]
        if i==5: threshold=[.14,.14];upper[11]=.14
        if i==6: threshold=[.001,.001];lower[11]=.001
        height=rng.uniform(-.1,.1);denominator=[rng.uniform(.02,.5) for _ in range(4)]
        y=[rng.uniform(-1,1) for _ in range(4)]
        z=[rng.uniform(-3,-.1),rng.uniform(-3,-.1),rng.uniform(.1,3),rng.uniform(.1,3)]
        values=upper+lower+sliders+base_damper+threshold+ratio+[height]+denominator+y+z+[rng.uniform(100,3000),9.81]
        assert len(values)==111
        cases.append((f'control_{i:03d}',[word(v) for v in values]))
    requests=out/'input.tsv'
    requests.write_text(''.join(name+'\t'+'\t'.join(values)+'\n' for name,values in cases),encoding='ascii')
    image=ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for exe,arguments in [(ROOT/'local/behavior/suspension-prepare-probe.exe',[image,requests,out/'original.tsv']),
                          (ROOT/f'target/{args.profile}/grid-suspension-prepare.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(exe),'sha256':digest(exe),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Setup execution failed; evidence retained')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==39 and cells[1] not in result
            result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    summary={'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),'compared_float_words':len(cases)*37,
             'exact_float_words':len(cases)*37-sum(len(c['differences']) for c in comparisons)}
    report={'profile':args.profile,'summary':summary,'cases':comparisons,'executions':executions,
            'image_sha256':digest(image),'input_sha256':digest(requests),
            'limits':'Whole original setup consumer/helpers without stubs, supplied ranges/sliders/base fields and inert ancillary gearing/brake state. '
            'Only 37 suspension/height output words compared. Extra axle layouts, complete gearing/tyre setup, source parser/default ranges '
            'and synchronized live setup boundary are not validated. x87 explicitly 53-bit nearest.'}
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps(summary))
    assert summary['exact_cases']==len(cases)

if __name__=='__main__':main()
