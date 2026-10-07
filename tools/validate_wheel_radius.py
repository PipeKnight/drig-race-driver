"""Original angular radius selector with supplied five samples."""
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
    def add(name, angle, radius, samples):
        values=[angle,radius]+sum(samples,[])
        assert len(values)==12 and all(math.isfinite(v) for v in values)
        cases.append((name,[struct.unpack('<I',struct.pack('<f',v))[0] for v in values]))
    pi=struct.unpack('<f',struct.pack('<I',0x40490fdb))[0];turn=2*pi
    for i in range(12):
        samples=[[turn, .4],[0.,.5],[pi,.6],[-pi,.7],[pi/2,.8]]
        add(f'analytic_{i}',pi*(i-4),.3,samples)
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
            wheels=read('wheel_records',4*0x150);samples=read('wheel_radius_samples',0xa0)
            for w in range(4):
                angle=struct.unpack_from('<f',wheels,w*0x150+0x110)[0]
                radius=struct.unpack_from('<f',wheels,w*0x150+0xe0)[0]
                pairs=[list(struct.unpack_from('<2f',samples,w*40+k*8)) for k in range(5)]
                add(f'{label}_{index:02d}_{w}',angle,radius,pairs)
        snapshots.append({'path':str(path),'sha256':expected})
    rng=random.Random(2008)
    for i in range(144):
        add(f'synthetic_{i:03d}',rng.uniform(-100000,100000),rng.uniform(.1,1),
            [[rng.uniform(-100000,100000),rng.uniform(.1,1)] for _ in range(5)])
    requests = out/'input.tsv'
    with requests.open('x',encoding='ascii') as stream:
        for name,words in cases:
            stream.write(name+'\t'+'\t'.join(f'{v:08x}' for v in words)+'\n')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions=[]
    for executable, arguments in [
        (ROOT/'local/behavior/wheel-radius-probe.exe',[image,requests,out/'original.tsv']),
        (ROOT/f'target/{args.profile}/grid-wheel-radius.exe',[requests,out/'native.tsv'])]:
        result=subprocess.run([str(executable),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(executable),'sha256':digest(executable),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Wheel radius comparison failed; evidence retained')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==3 and row[1] not in results
            results[row[1]]=row[2:]
        return results
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    total=len(cases)
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
            'snapshots':snapshots,'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
            'compared_float_words':total,'exact_float_words':total-sum(len(c['differences']) for c in comparisons)},
            'limits':'Whole original998470 and original CRT modulo without stubs. Supplied five angular/radius samples, '
            'wheel angle and radius, output stored f32. Code RX/data RO; x87 explicitly53-bit nearest, '
            'original CRT selects/restores its own arithmetic control. No damage sample generation or synchronized integration claim.'}
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        json.dump(report,stream,indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases']==len(cases)

if __name__=='__main__':
    main()
