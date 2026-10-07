"""Standard preload builder at explicitly selected x87 53-bit nearest."""
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
    def add(name, mass, gravity, z, spring, observed=None):
        values=[mass,gravity]+z+spring
        assert len(values)==10 and all(math.isfinite(v) for v in values)
        cases.append((name,[struct.unpack('<I',struct.pack('<f',v))[0] for v in values]))
        if observed is not None:
            observed_cases[name]=[f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}' for v in observed]
    observed_cases={}
    for i in range(12):
        add(f'analytic_{i}',100+100*i,0. if i==0 else 10.,[-1,-1,3,3],
            [100+i*10,100+i*20,100+i*30,100+i*40])
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
            wheels=read('wheel_records',4*0x150)
            mass=struct.unpack_from('<f',mass,0)[0]
            z=[struct.unpack_from('<f',wheels,w*0x150+8)[0] for w in range(4)]
            spring=[struct.unpack_from('<f',wheels,w*0x150+0x5c)[0] for w in range(4)]
            preload=[struct.unpack_from('<f',wheels,w*0x150+0x54)[0] for w in range(4)]
            add(f'{label}_{index:02d}',mass,struct.unpack('<f',struct.pack('<I',0x411cf5c3))[0],z,spring,preload)
        snapshots.append({'path':str(path),'sha256':expected})

    rng=random.Random(2008)
    for i in range(180):
        add(f'synthetic_{i:03d}',rng.uniform(50,5000),rng.uniform(0,25),
            [rng.uniform(-3,-.1),rng.uniform(-3,-.1),rng.uniform(.1,3),rng.uniform(.1,3)],
            [rng.uniform(100,100000) for _ in range(4)])
    requests = out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:
            stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable, arguments in [
        (ROOT/'local/behavior/preload-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-preload.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Preload comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==6 and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)*4
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
            'snapshots':snapshots,'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
            'compared_float_words':total,'exact_float_words':total-sum(len(c['differences']) for c in comparisons)},
            'observed_preload_replays':[{'name':name,'exact':original[name]==observed,'differences':[
                {'wheel':i,'computed':a,'observed':b} for i,(a,b) in enumerate(zip(original[name],observed)) if a!=b]}
                for name,observed in observed_cases.items()],
            'limits':'Whole Standard preload builder/gravity helper without stubs, supplied gravity/local wheel Z/mass/spring. '
            'x87 explicitly 53-bit nearest, running-game control-word not captured. '
            '24 paused state inputs replayed with supplied gravity magnitude 9.8100004196; actual retained preload comparison reported separately. '
            'No synchronized production initialization, deformation or driving claim.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    print('observed_preloads_exact='+str(sum(v['exact'] for v in report['observed_preload_replays'])))
    assert report['summary']['exact_cases']==len(cases)

if __name__=='__main__':
    main()
