"""Whole original substep under explicit four-wheel/extra-axes-inactive conditions."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def floats(raw, offset, count):
    return list(struct.unpack_from(f'<{count}f',raw,offset))

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    cases=[]
    snapshots=[]
    def add(name,wheels,response=None,secondary=None,delta=.001,weight=.75,scale=1.):
        values=[delta,weight,scale]+(response or [0.]*16)+(secondary or [0.]*16)+sum(wheels,[])
        assert len(values)==139
        cases.append((name,[struct.unpack('<I',struct.pack('<f',v))[0] for v in values]))
    wheel=[0.,0.,0.,0.,1.,0.,1.,1.,1.,0.,0., 0.,1.,0.,0., 0.,0.,0.,0., 0.,0.,0.,0.,0., -98.,98.]
    for i in range(12):
        wheels=[wheel.copy() for _ in range(4)]
        for j,w in enumerate(wheels):
            if i%3==0:w[17]=-.1
            if i%3==1:w[17]=.002
            if i>=3:w[0]=(.001 if j%2 else -.001);w[1]=.1*(j-1)
            if i>=6:w[2]=50000.;w[5]=1000.;w[10]=10000.
            if i>=9:w[16]=2.;w[18]=-.1;w[19:23]=[1.,-2.,3.,-4.];w[23]=1.
        response=[.001 if a==b else -.0001 for a in range(4) for b in range(4)] if i>=6 else None
        secondary=[.0002 if a==b else -.00001 for a in range(4) for b in range(4)] if i>=6 else None
        add(f'analytic_{i:02d}',wheels,response,secondary)
    for i in range(16):
        wheels=[wheel.copy() for _ in range(4)]
        for j,w in enumerate(wheels):
            w[0]=(.002 if (i+j)%2 else -.002)
            w[1]=(.5 if (i//2+j)%2 else -.5)
            w[17]=-1.
            w[24:26]=[-.001,.001]
            if i>=8:w[20:23]=[1.,-2.,3.]
        response=[.003 if a==b else -.0002 for a in range(4) for b in range(4)]
        add(f'travel_stop_{i:02d}',wheels,response,weight=0. if i%4==0 else .75)
    for label,file,expected in [
        ('paused','general-vehicle-manager-20261007T0837131885850Z.json','4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
        ('throttle','general-vehicle-manager-20261007T0837468621304Z.json','e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5')]:
        path=ROOT/'local/behavior'/file
        assert digest(path)==expected
        data=json.loads(path.read_text(encoding='utf-8-sig'))
        assert data['read_only'] and data['live_count_code_exact'] and len(data['managers'])==1 and data['managers'][0]['count']==12
        blocks={b['name']:b for b in data['blocks']}
        assert len(blocks)==len(data['blocks'])
        for car in data['managers'][0]['cars']:
            i=car['index']
            def read(name,length):
                b=blocks[f'car_{i}-{name}']
                assert b['success'] and b['length']==b['read']==length
                raw=bytes.fromhex(b['hex']);assert len(raw)==length
                return raw
            context=read('dynamics_context',0x740)
            source=read('wheel_records',4*0x150)
            assert context[0x70c]==0 and struct.unpack_from('<II',context,0x5d4)==(int(car['body'],16),4)
            wheels=[]
            for j in range(4):
                def c(o):return floats(context,j*0x170+o,1)[0]
                def w(o):return floats(source,j*0x150+o,1)[0]
                flag=context[j*0x170+0x84];assert flag in (0,1)
                wheels.append([c(0x64),c(0x74),c(0x60),w(0x54),w(0x4c),w(0x60),w(0x64),w(0x68),w(0x114),w(0x58),w(0x70)]
                    +floats(context,j*0x170+0x30,4)
                    +[c(0x70),floats(context,0x640+j*4,1)[0],c(0x68),c(0x78),c(0x168),c(0x16c),c(0x174),c(0x170),float(flag),w(0x48),w(0x50)])
            response=sum([floats(context,j*0x170+0x88,4) for j in range(4)],[])
            add(f'{label}_{i:02d}',wheels,response,floats(context,0x650,16),scale=floats(context,0,1)[0])
        snapshots.append({'path':str(path),'sha256':expected})
    rng=random.Random(20081)
    for i in range(160):
        wheels=[]
        for j in range(4):
            q=rng.uniform(-.02,.02)
            wheels.append([q,rng.uniform(-.5,.5),rng.uniform(5000,80000),rng.uniform(-100,100),rng.uniform(.3,.6),
                rng.uniform(0,5000),rng.uniform(.05,.3),rng.uniform(.2,1),rng.uniform(.2,1.5),rng.uniform(0,20000),rng.uniform(0,40000)]
                +[rng.uniform(-.2,.2),rng.uniform(.8,1),rng.uniform(-.2,.2),0]
                +[rng.uniform(-2,2),rng.uniform(-20,20),q+rng.uniform(-.003,.003),rng.uniform(-.5,.5)]
                +[rng.uniform(-2,2) for _ in range(4)]+[float(i%2),-.019,.019])
        response=[rng.uniform(.0001,.004) if a==b else rng.uniform(-.001,.001) for a in range(4) for b in range(4)]
        secondary=[rng.uniform(.0001,.001) if a==b else rng.uniform(-.0003,.0003) for a in range(4) for b in range(4)]
        add(f'synthetic_{i:03d}',wheels,response,secondary,delta=rng.uniform(.0002,.0015),weight=rng.uniform(0,1),scale=rng.uniform(.5,1.5))
    requests=out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image=ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable,arguments in [
        (ROOT/'local/behavior/wheel-step-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-wheel-step.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':run.returncode,'stdout':run.stdout,'stderr':run.stderr})
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Substep comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t');assert row[0]=='P' and len(row)==42 and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)*40
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),'snapshots':snapshots,'cases':comparisons,
        'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),'compared_float_words':total,'exact_float_words':total-sum(len(c['differences']) for c in comparisons)},
        'limits':'Whole original 0x9c1f60 and travel-stop 0x998560 unstubbed, four active wheels/extra tyre axes inactive. '
        'Compare q/speed/body speed/floor/contact speed and raw/weighted suspension/contact accumulators plus contact flags. '
        '28 analytic/travel-stop, 24 recorded context replays, 160 synthetic cases; supplied .001 substep for recorded contexts, '
        'not an actual synchronized live substep. File-backed gravity and inverse-wheel-mass constant retained. '
        'No disabled wheel, extra tyre axes, preparation, body finalization, main timing or driving equivalence.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases']==len(cases)
if __name__=='__main__':main()
