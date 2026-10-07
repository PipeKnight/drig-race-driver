"""Conditional two-pass body finalization, without extra tyre axes or contact objects."""
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
    identity = [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]
    def add(name, parameters, basis, linear, angular, delta, inertia, wheels):
        values = parameters+basis+linear+angular+[delta]+inertia+sum(wheels,[])
        assert len(values)==120 and all(math.isfinite(v) for v in values)
        cases.append((name, [struct.unpack('<I', struct.pack('<f', v))[0] for v in values]))
    for i in range(16):
        basis=identity.copy();basis[12:15]=[10*i,-20*i,30*i]
        axis=[0,1,0,0];projection=[.1,1,.2,0] if i%2 else axis
        wheels=[basis[12:16]+axis+projection+[2 if i%3 else 0,1,3,1.5,
            -.3,.2,[-.5,-.2,.5,2][w],-.4,.4,0] for w in range(4)]
        add(f'analytic_{i}',[2,3,4,5],basis,[0,0,0,0],[0,.1*i,0,0],.01,[6,8,10],wheels)
    for label, file, expected in [
        ('paused', 'general-vehicle-manager-20261007T0837131885850Z.json',
         '4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
        ('throttle', 'general-vehicle-manager-20261007T0837468621304Z.json',
         'e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5')]:
        path = ROOT/'local/behavior'/file
        assert digest(path)==expected
        snapshot = json.loads(path.read_text(encoding='utf-8-sig'))
        assert snapshot['read_only'] and snapshot['live_count_code_exact']
        assert len(snapshot['managers'])==1 and snapshot['managers'][0]['count']==12
        blocks = {b['name']: b for b in snapshot['blocks']}
        assert len(blocks)==len(snapshot['blocks'])
        for car in snapshot['managers'][0]['cars']:
            index = car['index']
            def read(name, length):
                block = blocks[f'car_{index}-{name}']
                assert block['success'] and block['length']==block['read']==length
                raw = bytes.fromhex(block['hex'])
                assert len(raw)==length
                return raw
            mass, frame, context = read('mass_inertia_fields',0x30),read('frame_fields',0x90),read('dynamics_context',0x740)
            assert struct.unpack_from('<II',context,0x5d4)==(int(car['body'],16),4) and context[0x70c]==0
            parameters = [struct.unpack_from('<f',mass,o)[0] for o in (0,8,12,16)]
            basis = list(struct.unpack_from('<16f',frame,0x10))
            linear, angular = list(struct.unpack_from('<4f',mass,0x20)),list(struct.unpack_from('<4f',frame))
            source=read('wheel_records',4*0x150)
            wheels=[]
            for w in range(4):
                vectors=sum([list(struct.unpack_from('<4f',context,w*0x170+o)) for o in (0x10,0x30,0x40)],[])
                fields=[struct.unpack_from('<f',context,w*0x170+o)[0] for o in (0x64,0x74,0x68)]
                stored=[struct.unpack_from('<f',source,w*0x150+o)[0] for o in (0x48,0x4c,0x138)]
                wheels.append(vectors+[2+w,1+w*.2,.3,.1]+fields+stored)
            add(f'{label}_{index:02d}',parameters,basis,linear,angular,.00608,
                [struct.unpack_from('<f',context,o)[0] for o in (0x6f0,0x6f4,0x6f8)],wheels)
        snapshots.append({'path':str(path),'sha256':expected})

    rng = random.Random(20084)
    for i in range(160):
        angle=rng.uniform(-math.pi,math.pi);c,sn=math.cos(angle),math.sin(angle)
        position=[rng.uniform(-500,500),rng.uniform(-30,30),rng.uniform(-500,500),1]
        basis=[c,0,-sn,0,0,1,0,0,sn,0,c,0]+position
        parameters=[rng.uniform(100,2500)]+[rng.uniform(50,5000) for _ in range(3)]
        wheels=[]
        for w in range(4):
            point=[position[j]+rng.uniform(-2,2) for j in range(3)]+[1]
            axis=[rng.uniform(-.2,.2),rng.uniform(.7,1),rng.uniform(-.2,.2),0]
            projection=[rng.uniform(-.1,.1),rng.uniform(.8,1.2),rng.uniform(-.1,.1),0]
            wheels.append(point+axis+projection+[rng.uniform(-20,20) for _ in range(4)]
                +[rng.uniform(-.5,.5),rng.uniform(-2,2),rng.uniform(-.6,.6),-.4,.4,rng.uniform(-100,100)])
        add(f'synthetic_{i:03d}',parameters,basis,[rng.uniform(-60,60) for _ in range(3)]+[0],
            [rng.uniform(-2,2) for _ in range(3)]+[0],rng.uniform(.001,.02),[v*1.5 for v in parameters[1:]],wheels)
    requests = out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:
            stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable, arguments in [
        (ROOT/'local/behavior/finalize-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-finalize.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Finalizer comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==63 and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)*61
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
            'snapshots':snapshots,'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
            'compared_float_words':total,'exact_float_words':total-sum(len(c['differences']) for c in comparisons)},
            'limits':'Whole original 0x9e4b10 without stubs, including pose and impulse helpers. Four wheels, extra tyre axes inactive '
            'and contact-object pointers null. Supplied accumulators, delta, floor and inertia overrides. '
            '24 recorded body/point/axis replay inputs with supplied impulses; not synchronized finalization boundaries. '
            'Compare mass/inertia, pose, velocities, wheel displacement/speed/floor difference/force, consumed impulses and clock. '
            'No preparation, active tyre model, contact-object adjustment, drivetrain, timing or driving equivalence.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases']==len(cases)

if __name__=='__main__':
    main()
