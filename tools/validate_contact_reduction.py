"""Original whole contact reduction, live prepared operands and authored controls."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from validate_impact_generation import blob
from validate_impact_producer import pack as producer_pack,expected as producer_expected,compare as producer_compare
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='8ee5000505b2465989624dcf919c95d48bc0deac24f898159cc1b010e7fd8a30'


def from_live(case):
    result=[]
    for c in case['contacts']:
        b=blob(c,'before_hex',0x80);reverse=int(b[0xc]==0 and struct.unpack_from('<I',b,8)[0]==int(case['linked'],16))
        result.append([reverse,*struct.unpack_from('<f',b,0x3c),*struct.unpack_from('<8f',b,0x50)])
    if len(result)!=case['contact_count']:raise ValueError('Contact count mismatch')
    return result


def pack(rows):
    return struct.pack('<II',0x31524349,len(rows))+b''.join(struct.pack('<I',len(r))+b''.join(struct.pack('<I9f',*c) for c in r) for r in rows)


def controls():
    rng=random.Random(20081009);rows=[]
    for i in range(512):
        count=rng.choice([1,2,3,7,16,79,80]);r=[]
        for j in range(count):
            c=[rng.randrange(2),rng.uniform(-100,100),*(rng.uniform(-1000,1000) for _ in range(8))]
            mode=i%16
            if mode==0:c[1]=0.
            if mode==1:c[1]=-0.
            if mode==2:c[2:]=[-0.,0.,-0.,0.]*2
            if mode==3:c[1]=1.;c[6:]=[0.,0.,0.,1.]
            if mode==4:c[0]=1
            if mode==5:c[0]=0
            if mode==6:c[1]=(-1.)**j;c[6:]=[1.,1.,1.,1.]
            if mode==7:c[1]=struct.unpack('<f',struct.pack('<I',1))[0];c[6:]=[1.,-1.,0.,0.]
            if mode==8:c[2:6]=[1.,2.,3.,4.]
            if mode==9:c[1]=2.**rng.randrange(-20,21)
            r.append(c)
        rows.append(r)
    return rows


def compare(actual,wanted,count):
    if len(actual)!=8+count*32 or actual[:8]!=struct.pack('<II',0x31524349,count) or len(wanted)!=len(actual):raise ValueError('Reduction framing mismatch')
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_words=count*8,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);parser.add_argument('--controls',action='store_true');args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    c=json.loads(source.read_text(encoding='utf-8'));cases=c['cases']
    if not(c['completed'] and c['debugger_exit']==0 and c['code_exact'] and c['hardware_breakpoints'] and c['session']['ExeSHA256']==REFERENCE and 1<=len(cases)<=32):raise ValueError('Unverified capture')
    rows=controls() if args.controls else [from_live(r) for r in cases]
    wanted=None if args.controls else struct.pack('<II',0x31524349,len(rows))+b''.join(blob(r,'prepared_a_hex',16)+blob(r,'prepared_b_hex',16) for r in cases)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/contact-reduction-probe.exe';native=ROOT/f'target/{args.profile}/grid-contact-reduction.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows));original=None;rust=None
    report=dict(scope='Whole original9A14D0 ordered point/direction reduction; original helpers/imports/code patches absent. Does not implement interveningA06050 descriptor policy.',authored_controls=args.controls,cases=len(rows),profile=args.profile,capture_sha256=digest(source),probe_sha256=digest(probe),native_sha256=digest(native),runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if original is None:original=actual
            r.update(compare(actual,original,len(rows)))
            if wanted is not None:r['live']=compare(actual,wanted,len(rows));r['passed'] &= r['live']['passed']
            if label=='rust':rust=actual
        report['runs'][label]=r
    good=pack([[from_live(cases[0])[0]]]);bad={'trailing':good+b'\0','truncated':good[:-1],'zero_cases':good[:4]+bytes(4)+good[8:],'zero_contacts':good[:8]+bytes(4)+good[12:],'excess_contacts':good[:8]+struct.pack('<I',81)+good[12:]}
    r=[from_live(cases[0])[0]]
    for name,index,value in [('nan_weight',1,float('nan')),('infinite_point',2,float('inf')),('nan_direction',6,float('nan')),('invalid_reverse',0,2)]:
        row=copy.deepcopy(r);row[0][index]=value;bad[name]=pack([row])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);checks={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    if report['passed'] and not args.controls:
        rows=copy.deepcopy(cases)
        for i,row in enumerate(rows):row['prepared_a_hex']=rust[8+i*32:24+i*32].hex();row['prepared_b_hex']=rust[24+i*32:40+i*32].hex()
        source=out/'producer-input.bin';source.write_bytes(producer_pack(rows));target=out/'producer-rust.bin';binary=ROOT/f'target/{args.profile}/grid-impact-producer.exe'
        run=subprocess.run([str(binary),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);p=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,native_sha256=digest(binary))
        if run.returncode==0:p.update(producer_compare(target.read_bytes(),producer_expected(cases),len(cases)))
        report['producer_pipeline']=p;report['passed'] &= p.get('passed',False)
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Contact reduction failed; inspect private report')


if __name__=='__main__':main()
