"""Whole original motion feedback versus independently authored Rust."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_force_view import SNAPSHOTS

ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value):return struct.unpack('<I',struct.pack('<f',value))[0]
def words(data,offset,count):return list(struct.unpack_from('<'+'I'*count,data,offset))
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    cases,provenance=[],[]
    identity=[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,1000.,-2000.,3000.,1.]
    base=[5.,.5,.3]+identity+[1.,0.,5.,0.]+[0.,0.,0.,0.]+[.8,.3,1.2,1.,-.8,.3,1.2,1.]
    def add(name,row):
        assert len(row)==35;cases.append((name,[word(v) for v in row]))
    for i,speed in enumerate([-10.,-4.0000005,-4.,-3.,-2.0000002,-2.,-0.,0.,2.,2.0000002,3.,4.,4.0000005,10.]):
        for j in range(8):
            v=base.copy();v[0]=speed
            v[19:23]=[[1.,0.,5.,0.],[-1.,0.,-5.,0.],[1.,0.,-0.,0.],[-1.,0.,0.,0.],
                       [0.,0.,-5.,0.],[-0.,0.,5.,-0.],[10.,3.,1.,2.],[-10.,-3.,1.,-2.]][j]
            if j>=6:v[23:27]=[.1,-.2,.3,1e20]
            add(f'boundary_{i}_{j}',v)
    for i in range(16):
        v=base.copy();v[0]=0. if i%2 else 2.
        v[19:27]=[0.]*8;v[1:3]=[-0. if i%4 else .5,-0. if i%3 else .3]
        add(f'inactive_zero_velocity_{i}',v)
    for s,(name,sha) in enumerate(SNAPSHOTS):
        path=ROOT/'local/behavior'/name;assert digest(path)==sha
        data=json.loads(path.read_text(encoding='utf-8-sig'));assert data['read_only'] and data['live_count_code_exact']
        blocks={b['name']:bytes.fromhex(b['hex']) for b in data['blocks'] if b['success'] and b['read']==b['length']}
        for car in data['managers'][0]['cars']:
            assert car['wheel_count']==4
            prefix=f"car_{car['index']}"
            frame,mass,wheels=(blocks[prefix+'-'+kind] for kind in ('frame_fields','mass_inertia_fields','wheel_records'))
            row=words(frame,0x78,1)+[word(.5),word(.3)]+words(frame,0x10,16)+words(mass,0x20,4)+words(frame,0,4)
            row+=words(wheels,2*0x150,4)+words(wheels,3*0x150,4)
            assert len(row)==35;cases.append((f'retained_{s}_{car["index"]}',row))
        provenance.append(dict(snapshot=name,sha256=sha,non_atomic=True,command_and_scale_supplied=True,original_live_feedback_not_captured=True))
    rng=random.Random(120)
    for i in range(1024):
        v=base.copy();v[:3]=[rng.uniform(-10.,10.),rng.uniform(.1,1.),rng.uniform(-1.,1.)]
        v[19:27]=[rng.uniform(-10.,10.) for _ in range(8)]
        v[27:]=[rng.uniform(-3.,3.) for _ in range(8)]
        if i>=256:v[3:19]=[rng.uniform(-2.,2.) for _ in range(16)]
        if i>=512:v[15:19]=[rng.uniform(-1e5,1e5) for _ in range(4)]
        add(f'authored_{i}',v)
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{v:08x}' for v in row)+'\n' for name,row in cases))
    executions=[]
    for kind,exe,argv in [
        ('original',ROOT/'local/behavior/steering-feedback-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-steering-feedback.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Steering feedback execution failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert len(cells)==3 and cells[0]=='P' and cells[1] not in result;result[cells[1]]=cells[2]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native.keys()=={name for name,_ in cases}
    differences=[dict(case=name,original=original[name],native=native[name]) for name in original if original[name]!=native[name]]
    guards=[]
    for label,active in [('zero_motion',True),('overflow_requested',False)]:
        v=base.copy();v[0]=5. if active else 0.;v[19:27]=[0.]*8
        if not active:v[1:3]=[3e38,3e38]
        source=out/f'guard-{label}.tsv';source.write_text(label+'\t'+'\t'.join(f'{word(x):08x}' for x in v)+'\n')
        for kind,exe,argv in [
            ('original',ROOT/'local/behavior/steering-feedback-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',source,out/f'guard-{label}-original.tsv']),
            ('native',ROOT/f'target/{args.profile}/grid-steering-feedback.exe',[source,out/f'guard-{label}-native.tsv'])]:
            run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards.append(dict(case=label,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr));assert run.returncode!=0
    result=dict(profile=args.profile,cases=len(cases),words=len(cases),exact_cases=len(cases)-len(differences),differences=differences,guards=guards,
                executions=executions,input_sha256=digest(requests),provenance=provenance,
                limits='Whole original9ae5a0/realcross, original x87division/fpatan under PC53/nearest, observed caller f32 return conversion. RX code/read-only data, no patches/stubs/live injection. '
                       'Supplied matrix/wheel/motion/speed/command/scale, bounded finite cases, no universal atan math equivalence or live cadence/complete assistance claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ('profile','cases','words','exact_cases')}));print('different_words='+str(len(differences)));assert not differences
if __name__=='__main__':main()
