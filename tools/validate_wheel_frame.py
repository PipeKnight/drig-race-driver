"""Whole four-wheel geometry helper, optionally with supplied active tyre axes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--tyre-axes', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    cases, snapshots = [], []
    def add(name,basis,points,axes,displacement,coefficients=None):
        values=sum(basis,[])+sum(points,[])+sum(axes,[])+displacement
        if args.tyre_axes:
            assert coefficients is not None
            values+=sum(coefficients,[])
        assert len(values)==(60 if args.tyre_axes else 52) and all(math.isfinite(v) for v in values)
        cases.append((name,[struct.unpack('<I',struct.pack('<f',v))[0] for v in values]))
    identity=[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[10.,20.,30.,1.]]
    for i in range(12):
        add(f'analytic_{i}',identity,[[1.,2.,3.,float(i)]]*4,
            [[0.,1.,0.,0.] if i<6 else [1.,0.,0.,.5]]*4,[i*.025]*4,
            [[(-0. if i==0 else i*.025),1.-i*.02],[-i*.025,1.],[0.,-0.],[-0.,0.]])
    for label,file,expected in [
        ('paused','general-vehicle-manager-20261007T0837131885850Z.json','4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
        ('throttle','general-vehicle-manager-20261007T0837468621304Z.json','e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5')]:
        path=ROOT/'local/behavior'/file;assert digest(path)==expected
        snapshot=json.loads(path.read_text(encoding='utf-8-sig'));assert snapshot['read_only']
        blocks={b['name']:b for b in snapshot['blocks']};assert len(blocks)==len(snapshot['blocks'])
        for car in snapshot['managers'][0]['cars']:
            index=car['index']
            def read(name,length):
                block=blocks[f'car_{index}-{name}'];assert block['success'] and block['length']==block['read']==length
                return bytes.fromhex(block['hex'])
            frame=read('frame_fields',0x90);wheels=read('wheel_records',4*0x150);context=read('dynamics_context',0x740)
            assert struct.unpack_from('<II',context,0x5d4)==(int(car['body'],16),4) and context[0x70c]==0
            basis=[list(struct.unpack_from('<4f',frame,0x10+i*16)) for i in range(4)]
            points=[list(struct.unpack_from('<4f',wheels,i*0x150)) for i in range(4)]
            axes=[list(struct.unpack_from('<4f',context,i*0x170+0x50)) for i in range(4)]
            displacement=[struct.unpack_from('<f',context,i*0x170+0x64)[0] for i in range(4)]
            coefficients=[list(struct.unpack_from('<2f',wheels,i*0x150+0x20)) for i in range(4)]
            add(f'{label}_{index:02d}',basis,points,axes,displacement,coefficients)
        snapshots.append({'path':str(path),'sha256':expected})
    rng=random.Random(2008)
    for i in range(180):
        add(f'synthetic_{i:03d}',[[rng.uniform(-10,10) for _ in range(4)] for _ in range(4)],
            [[rng.uniform(-5,5) for _ in range(4)] for _ in range(4)],
            [[rng.uniform(-2,2) for _ in range(4)] for _ in range(4)],
            [rng.uniform(-.3,.3) for _ in range(4)],
            [[rng.uniform(-2,2),rng.uniform(-2,2)] for _ in range(4)] if args.tyre_axes else None)
    requests = out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:
            stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable, arguments in [
        (ROOT/'local/behavior/wheel-frame-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-wheel-frame.exe',[requests,out/'native.tsv'])]:
        if args.tyre_axes:arguments.append('tyre')
        result=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Wheel frame comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==(82 if args.tyre_axes else 50) and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)*(80 if args.tyre_axes else 48)
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
            'snapshots':snapshots,'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
            'compared_float_words':total,'exact_float_words':total-sum(len(c['differences']) for c in comparisons)},
            'limits':'Whole four-wheel geometry helper without stubs or writable mapped pages; extra tyre axes inactive. '
            '24 recorded local points/axes/displacement/basis replays, not synchronized original geometry boundaries. '
            'No production contact/query or driving claim.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        if args.tyre_axes:
            report['limits']='Whole original9c13d0 active+70c branch; supplied wheel+20/+24 coefficients, including24retained inactive-context inputs replayed with branch enabled. No synchronized active reference/input calibration, tyre force or driving claim. No stubs/code patches/writable mapped pages.'
        json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases']==len(cases)

if __name__=='__main__':
    main()
