"""Authored whole-original collision-body controls, separate from live calls."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,ANALYSIS
from validate_impact_body import from_live,pack,ROOT,PROBE_SHA
from validate_steering_control import digest


def values(*v):return struct.pack('<'+'f'*len(v),*v)


def controls(capture):
    rng=random.Random(20081009);base=from_live(next(c for c in capture['cases'] if c['body_effect']['random_result'] is not None));rows=[]
    for i in range(256):
        r=copy.deepcopy(base);linked=bytearray(r['linked']);linked[0x210:0x230]=values(0.,0.,0.,0.,.1,.2,.3,0.);r['linked']=bytes(linked)
        r.update(frame=values(1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.),a=values(2.,3.,-4.,1.),b=values(5.,.25,2.,0.),center=values(1.,2.,3.,1.),maximum=values(*range(8,18)),extra=values(5.),seed=rng.randrange(2**32))
        v=i%32
        if v in (1,2,3,20):
            linked=bytearray(r['linked']);linear={1:[6.5,0.,0.,0.],2:[6.4,0.,0.,0.],3:[0.]*4,20:[.1,.2,.3,7.]}[v];linked[0x220:0x230]=values(*linear);r['linked']=bytes(linked)
        elif v in (4,5,6,7,8):r['maximum']=values(*([{4:7.,5:struct.unpack('<f',struct.pack('<I',0x40e00001))[0],6:6.9,7:22.,8:100.}[v]]*10))
        elif v in (9,10,11,12,13,21,22,23,28):
            r['b']=values(*{9:[0.,1.,0.,0.],10:[0.]*4,11:[0.0000003,0.,0.,0.],12:[0.0000004,0.,0.,0.],13:[-5.,-.25,-2.,0.],21:[0.,2.,.1,0.],22:[.7,.71414286,0.,0.],23:[.1,0.,-1.,0.],28:[5.,.25,2.,3.]}[v])
        elif v==14:r['mass']=values(500.)
        elif v==15:r['mass']=values(1000.)
        elif v in (16,17,18,26):
            g=list(struct.unpack('<5f',r['globals']));g[{16:0,17:0,18:1,26:2}[v]]={16:0.,17:-2.,18:0.,26:2.}[v];r['globals']=values(*g)
        elif v==19:r['seed']=0xffffffff
        elif v in (24,25):
            linked=bytearray(r['linked']);linked[0x210:0x220]=values(*( [-20.,20.,20.,0.] if v==24 else [-0.,0.,-0.,-0.]));r['linked']=bytes(linked)
        elif v==27:r['maximum']=values(*([-1.]*10))
        elif v==29:r['a']=r['center']
        elif v==30:r['frame']=values(*([0.]*16))
        elif v==31:r['mass']=values(100000.)
        if i>=64:
            r['a']=values(*(rng.uniform(-20.,20.) for _ in range(3)),1.);r['center']=values(*(rng.uniform(-5.,5.) for _ in range(3)),1.)
            g=list(struct.unpack('<5f',r['globals']));g[0]*=rng.uniform(.2,2.);g[1]*=rng.uniform(-2.,2.);r['globals']=values(*g)
        if i>=128:
            linked=bytearray(r['linked']);linked[0x210:0x220]=values(*(rng.uniform(-2.,2.) for _ in range(4)));r['linked']=bytes(linked)
        if i>=192:
            r['frame']=values(*(rng.uniform(-1.,1.) for _ in range(16)))
        rows.append(r)
    return rows


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    c=json.loads(private(args.capture).read_text(encoding='utf-8'));rows=controls(c);out=private(args.output)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/impact-body-probe.exe';native=ROOT/f'target/{args.profile}/grid-impact-body.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Image/probe identity mismatch')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows));original=None
    report=dict(authored_cases=len(rows),seed=20081009,live_verification=False,scope='Whole original9D4210 and original CRT rand, controlled TLS seeds; no helper stubs.',profile=args.profile,probe_sha256=digest(probe),native_sha256=digest(native),runs={},guards={})
    for label,command in [('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if len(actual)!=8+len(rows)*96 or actual[:8]!=struct.pack('<II',0x31524249,len(rows)):raise ValueError('Output framing mismatch')
            if original is None:original=actual
            differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=original[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=original[i:i+4]]
            r.update(compared_words=len(rows)*24,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)
            r['rng_advanced_cases']=sum(struct.unpack_from('<I',actual,8+i*96+92)[0]!=row['seed'] for i,row in enumerate(rows))
            r['angular_changed_cases']=sum(actual[8+i*96:8+i*96+16]!=row['linked'][0x210:0x220] for i,row in enumerate(rows))
        report['runs'][label]=r
    bad={}
    for name,key,value in [('zero_mass','mass',values(0.)),('zero_range','globals',values(1.,.1,0.,7.,6.4)),('nan_direction','b',values(float('nan'),0.,0.,0.)),('infinite_point','a',values(float('inf'),0.,0.,1.))]:
        row=copy.deepcopy(rows[0]);row[key]=value;bad[name]=pack([row])
    good=pack([rows[0]]);bad['trailing']=good+b'\0';bad['truncated']=good[:-1];bad['zero_cases']=good[:4]+bytes(4)+good[8:]
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);checks={}
        for label,command in [('original',[str(probe),str(image)]),('rust',[str(native)])]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            checks[label]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values()) and report['runs']['original'].get('angular_changed_cases',0)>0
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Authored body controls failed; inspect private report')


if __name__=='__main__':main()
