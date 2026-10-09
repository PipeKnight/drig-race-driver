"""Whole original inverse-pose replay, actual callback operands and controls."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from validate_impact_generation import blob
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='520ceb2b848d10e7cb923fe1034b6324c949ca6afabbc7f22e156998b0f0fafc'


def pack(rows):
    return struct.pack('<II',0x31494349,len(rows))+b''.join(struct.pack('<16f',*r) for r in rows)


def controls():
    rng=random.Random(20081009);rows=[]
    for i in range(512):
        r=[rng.uniform(-100,100) for _ in range(16)]
        if i%8==0:r=[float(j//4==j%4) for j in range(16)]
        if i%8==1:r=[rng.choice([0.,-0.]) for _ in r]
        if i%8==2:r[12:]=[0.,-0.,0.,1.]
        if i%8==3:r=[rng.choice([1.,-1.,0.,-0.]) for _ in r]
        if i%8==4:r[12:]=[1.,-1.,1.,0.]
        if i%8==5:r=[rng.choice([-1.,1.])*2.**rng.randrange(-20,21) for _ in r]
        if i%8==6:r[3]=2.;r[7]=-3.;r[11]=4.;r[15]=-5.
        if i%8==7:r[12:]=[struct.unpack('<f',struct.pack('<I',1))[0],-0.,0.,1.]
        rows.append(r)
    return rows


def compare(actual,wanted,count):
    if len(actual)!=8+count*64 or actual[:8]!=struct.pack('<II',0x31494349,count) or len(actual)!=len(wanted):raise ValueError('Inverse framing mismatch')
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_words=count*16,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);parser.add_argument('--controls',action='store_true');args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    c=json.loads(source.read_text(encoding='utf-8'));cases=c['cases']
    if not(c['completed'] and c['debugger_exit']==0 and c['code_exact'] and c['hardware_breakpoints'] and c['boundary']=='preparation' and c['session']['ExeSHA256']==REFERENCE and 1<=len(cases)<=32):raise ValueError('Unverified preparation capture')
    rows=controls() if args.controls else [struct.unpack('<16f',blob(r,'linked_before_hex',0x424)[0x1d0:0x210]) for r in cases]
    wanted=None if args.controls else struct.pack('<II',0x31494349,len(rows))+b''.join(blob(r,'inverse_hex',64) for r in cases)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/contact-inverse-probe.exe';native=ROOT/f'target/{args.profile}/grid-contact-inverse.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows));original=None
    report=dict(scope='Whole original9A15D0 pose transpose/translation; no imports, helper stubs or code patches. Original assumes rigid basis; authored nonrigid matrices test arithmetic only.',authored_controls=args.controls,cases=len(rows),profile=args.profile,capture_sha256=digest(source),probe_sha256=digest(probe),native_sha256=digest(native),runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if original is None:original=actual
            r.update(compare(actual,original,len(rows)))
            if wanted is not None:r['live']=compare(actual,wanted,len(rows));r['passed'] &= r['live']['passed']
        report['runs'][label]=r
    good=pack([rows[0]]);bad={'trailing':good+b'\0','truncated':good[:-1],'zero_cases':good[:4]+bytes(4)+good[8:],'excess_cases':good[:4]+struct.pack('<I',1025)+good[8:]}
    for name,index,value in [('nan_basis',0,float('nan')),('infinite_translation',12,float('inf')),('nan_ignored_w',3,float('nan'))]:
        row=list(copy.deepcopy(rows[0]));row[index]=value;bad[name]=pack([row])
    row=[float(j//4==j%4) for j in range(16)];row[0]=3e38;row[12]=3e38;bad['overflow']=pack([row])
    for name,data in bad.items():
        guard=out/f'guard-{name}.bin';guard.write_bytes(data);checks={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(guard),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Contact inverse failed; inspect private report')


if __name__=='__main__':main()
