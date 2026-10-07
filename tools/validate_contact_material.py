"""Conditional contact material arithmetic; authored query-result fixture."""
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
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    cases, snapshots = [], []
    def add(name,basis,anchors,offsets,radius,low,denominator,axes,angles,samples,height,material,body,fractions,normals,hits,new,oldbump,newbefore):
        values=sum(basis,[])+sum(anchors,[])+sum(offsets,[])+radius+low+denominator+sum(axes,[])+angles+sum(sum(samples,[]),[])+height+material+body+fractions+sum(normals,[])+hits+[new]+oldbump+newbefore
        assert len(values)==166 and all(math.isfinite(v) for v in values)
        cases.append((name,[struct.unpack('<I',struct.pack('<f',v))[0] for v in values]))
    identity=[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[10.,20.,30.,1.]]
    threshold=struct.unpack('<f',struct.pack('<I',0x3ed8616c))[0]
    above=struct.unpack('<f',struct.pack('<I',0x3ed8616d))[0]
    for i in range(32):
        normal=[0.,[0.,threshold,above,1.][i%4],0.,0.]
        add(f'analytic_{i}',identity,[[i*.1,2.,-i*.2,1.]]*4,[[0.]*4]*4,[.3]*4,[-.1]*4,[.2]*4,[identity[1]]*4,
            [0.]*4,[[[0.,.3]]*5]*4,[.01]*4,[float(i%2),2.25,.01,1.,1.,.02],[i*.5,1.,.8],
            [i/31]*4,[normal]*4,[float(i%3!=0)]*4,float(i%2),[.005]*4,[.009]*4)
    for label,file,expected in [
        ('paused','general-vehicle-manager-20261007T1113268018652Z.json','fd93eb3329f44b05b34829999c7e6d10f831ef08d6876add4171a05b01381672'),
        ('throttle','general-vehicle-manager-20261007T1114213119655Z.json','da4f5d871fb14d7d84933c7b7439aee5cf3cfa1873073d8a4f93dddcd7aaf675')]:
        path=ROOT/'local/behavior'/file;assert digest(path)==expected
        snapshot=json.loads(path.read_text(encoding='utf-8-sig'));assert snapshot['read_only'] and snapshot['live_count_code_exact']
        blocks={b['name']:b for b in snapshot['blocks']};assert len(blocks)==len(snapshot['blocks'])
        for car in snapshot['managers'][0]['cars']:
            index=car['index']
            def read(name,length):
                block=blocks[f'car_{index}-{name}'];assert block['success'] and block['length']==block['read']==length
                return bytes.fromhex(block['hex'])
            frame=read('frame_fields',0x90);wheels=read('wheel_records',4*0x150);context=read('dynamics_context',0x740)
            parameters=read('parameter_neighborhood',0x350);rad=read('wheel_radius_samples',0xa0)
            assert struct.unpack_from('<II',context,0x5d4)==(int(car['body'],16),4)
            basis=[list(struct.unpack_from('<4f',frame,0x10+i*16)) for i in range(4)]
            anchors=[list(struct.unpack_from('<4f',context,i*0x170+0x20)) for i in range(4)]
            offsets=[list(struct.unpack_from('<4f',parameters,0x10+i*16)) for i in range(4)]
            at=lambda o:[struct.unpack_from('<f',wheels,i*0x150+o)[0] for i in range(4)]
            axes=[list(struct.unpack_from('<4f',context,i*0x170+0x30)) for i in range(4)]
            ptr=struct.unpack_from('<I',wheels,0xc0)[0];mat=read(f'material_{ptr:x}',0xb0)
            material=[float(mat[0x90]!=0)]+list(struct.unpack_from('<5f',mat,0x94))
            samples=[[list(struct.unpack_from('<2f',rad,w*40+k*8)) for k in range(5)] for w in range(4)]
            speed=struct.unpack_from('<f',parameters,8)[0]
            body=[speed,1.,1.] # clock/car factor supplied controls, not captured source fields
            normals=[list(struct.unpack_from('<4f',wheels,w*0x150+0xd0)) for w in range(4)]
            old=[struct.unpack_from('<f',context,w*0x170+0x7c)[0] for w in range(4)]
            before=[struct.unpack_from('<f',context,w*0x170+0x80)[0] for w in range(4)]
            add(f'{label}_{index:02d}',basis,anchors,offsets,at(0xe0),at(0x48),at(0x4c),axes,at(0x110),samples,at(0x140),material,body,
                [.5]*4,normals,[1.]*4,1.,old,before)
        snapshots.append({'path':str(path),'sha256':expected})
    rng=random.Random(2008)
    for i in range(180):
        basis=[[1.,0.,0.,0.],[rng.uniform(-.1,.1),1.,rng.uniform(-.1,.1),0.],[0.,0.,1.,0.],[10.,20.,30.,1.]]
        add(f'synthetic_{i:03d}',basis,[[rng.uniform(-100,100) for _ in range(3)]+[1.] for _ in range(4)],
            [[rng.uniform(-1,1) for _ in range(3)]+[0.] for _ in range(4)],
            [rng.uniform(.1,1) for _ in range(4)],[rng.uniform(-.3,0) for _ in range(4)],
            [rng.uniform(.1,.5) for _ in range(4)],[basis[1]]*4,
            [rng.uniform(-1000,1000) for _ in range(4)],[[[rng.uniform(-1000,1000),rng.uniform(.1,1)] for _ in range(5)] for _ in range(4)],
            [rng.uniform(-.1,.1) for _ in range(4)],
            [float(i%2),rng.uniform(.1,10),rng.uniform(0,.1),rng.uniform(0,1),rng.uniform(0,1),rng.uniform(-.1,.1)],
            [rng.uniform(-10,30),rng.uniform(0,200),rng.uniform(0,2)],
            [rng.random() for _ in range(4)],[[rng.uniform(-.3,.3),rng.uniform(.1,1),rng.uniform(-.3,.3),0.] for _ in range(4)],
            [float(rng.random()>.2) for _ in range(4)],float(i%3!=0),[rng.uniform(-.1,.1) for _ in range(4)],[rng.uniform(-.1,.1) for _ in range(4)])
    requests = out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:
            stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable, arguments in [
        (ROOT/'local/behavior/contact-material-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-contact-material.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Wheel frame comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==58 and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)*56
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
            'snapshots':snapshots,'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
            'compared_words':total,'exact_words':total-sum(len(c['differences']) for c in comparisons),'compared_float_words':len(cases)*44,'compared_presence_flags':len(cases)*12},
            'limits':'Conditional original9c1660 with authored query-result fixtures, original radius/CRT/normal helpers. '
            'Query call replaced; NO original traversal/hit fidelity claim. Captured geometry/material/radius inputs replayed '
            'with supplied fractions .5 and clock/car-factor1, not synchronized live contact boundaries. x87 explicitly53-bit nearest. '
            'Output includes12 pointer-presence flags per case, other44 words are floats; no proprietary assets published.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases']==len(cases)

if __name__=='__main__':
    main()
