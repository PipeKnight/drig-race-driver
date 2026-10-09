"""Whole original collector versus captured order and independent bounded Rust."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,ANALYSIS,REFERENCE
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='df53e31997019def6f067d215bb762db5104754749b8228ea4d8b53645132d1a'


def from_live(case):
    pools=[]
    for pool in case['pools']:
        entries=[]
        for e in pool['entries']:
            b=bytes.fromhex(e['before_hex'])
            if len(b)!=0x80:raise ValueError('Contact byte length mismatch')
            first,second=struct.unpack_from('<2I',b,4)
            entries.append([int(e['address'],16),first,second,int(b[0xc]!=0),int(e['disabled_a']!=0),int(e['disabled_b']!=0)])
        if len(entries)!=pool['count']:raise ValueError('Pool count mismatch')
        pools.append(entries)
    if len(pools)!=2 or case['capacity']!=80 or case['initial_count']!=0 or case['return_address'] not in ('0xb787d6','0xb788f3'):raise ValueError('Unverified collector ABI')
    return dict(owner=int(case['linked'],16),capacity=80,initial=[],pools=pools)


def pack(rows):
    data=[struct.pack('<II',0x31434f43,len(rows))]
    for r in rows:
        data.append(struct.pack('<5I',r['owner'],r['capacity'],len(r['initial']),len(r['pools'][0]),len(r['pools'][1])))
        data.append(struct.pack('<'+'I'*len(r['initial']),*r['initial']))
        data.extend(struct.pack('<6I',*c) for pool in r['pools'] for c in pool)
    return b''.join(data)


def decode(data,rows):
    if data[:8]!=struct.pack('<II',0x31434f43,len(rows)):raise ValueError('Output header mismatch')
    cursor=8;result=[]
    for r in rows:
        count,=struct.unpack_from('<I',data,cursor);cursor+=4
        if not len(r['initial'])<=count<=r['capacity']:raise ValueError('Output count outside bound')
        values=list(struct.unpack_from('<'+'I'*count,data,cursor));cursor+=count*4
        if values[:len(r['initial'])]!=r['initial']:raise ValueError('Output overwrote initial prefix')
        result.append(values)
    if cursor!=len(data):raise ValueError('Trailing output bytes')
    return result


def controls():
    rng=random.Random(20081009);rows=[]
    for i in range(256):
        cap=rng.choice([1,2,8,32,80]);owner=0x10000
        pools=[[[rng.randrange(1,200),rng.choice([0,owner,0x20000]),rng.choice([0,owner,0x20000]),rng.randrange(2),rng.randrange(2),rng.randrange(2)] for _ in range(rng.randrange(129))] for _ in range(2)]
        initial=[0x80000+j for j in range(rng.randrange(cap+1))];mode=i%16
        if mode==0:pools=[[],[]]
        if mode==1:pools=[[[1,owner,0,1,0,0]],[]];initial=[]
        if mode==2:pools=[[],[[2,0,owner,0,0,0]]];initial=[]
        if mode==3:pools=[[],[[3,0,owner,1,0,0]]];initial=[]
        if mode in (4,5):pools=[[[4,owner,0,0,int(mode==4),int(mode==5)]],[]];initial=[]
        if mode==6:cap=1;initial=[];pools=[[[5+j,owner,0,0,0,0] for j in range(80)],[]]
        if mode==7:initial=[0x80000+j for j in range(cap)]
        if mode==8:pools=[[[8,owner,0,0,0,0]]*20,[[8,owner,owner,0,0,0]]*20];initial=[];cap=80
        if mode==9:pools=[[[9,0,owner,0,0,0]],[]];initial=[]
        if mode==10:pools=[[[10+j,owner,0,0,0,0] for j in range(512)],[[10+j,owner,0,0,0,0] for j in range(512)]];cap=80;initial=[]
        rows.append(dict(owner=owner,capacity=cap,initial=initial,pools=pools))
    return rows


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);parser.add_argument('--controls',action='store_true');args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    c=json.loads(source.read_text(encoding='utf-8'))
    if not(c['completed'] and c['debugger_exit']==0 and c['code_exact'] and c['hardware_breakpoints'] and c['boundary']=='collection' and c['session']['ExeSHA256']==REFERENCE and 1<=len(c['cases'])<=32):raise ValueError('Unverified live capture')
    live=[from_live(r) for r in c['cases']];rows=controls() if args.controls else live
    wanted=None if args.controls else [[int(p,16) for p in r['output_pointers']] for r in c['cases']]
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/contact-collection-probe.exe';native=ROOT/f'target/{args.profile}/grid-contact-collection.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows));original=None
    report=dict(scope='Whole originalB775D0, no imports/helper stubs/code patches; ordered supplied contact pools, not geometric detection.',authored_controls=args.controls,profile=args.profile,cases=len(rows),capture_sha256=digest(source),probe_sha256=digest(probe),native_sha256=digest(native),runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes();values=decode(actual,rows)
            if original is None:original=actual
            mismatches=[] if len(actual)==len(original) else [{'length':len(actual),'expected_length':len(original)}]
            mismatches.extend(dict(offset=hex(i)) for i in range(8,min(len(actual),len(original)),4) if actual[i:i+4]!=original[i:i+4])
            live_mismatches=[i for i,(a,b) in enumerate(zip(values,wanted)) if a!=b] if wanted is not None else []
            r.update(compared_words=(len(actual)-8)//4,returned_contacts=sum(map(len,values)),mismatched_words=len(mismatches),first_differences=mismatches[:20],live_order_mismatches=live_mismatches,passed=not mismatches and not live_mismatches)
        report['runs'][label]=r
    good=pack([live[0]]);bad={'truncated':good[:-1],'trailing':good+b'\0','zero_cases':good[:4]+bytes(4)+good[8:]}
    for name,key,value in [('zero_owner','owner',0),('zero_capacity','capacity',0),('excess_capacity','capacity',81),('excess_initial','initial',list(range(81)))]:
        row=copy.deepcopy(live[0]);row[key]=value;bad[name]=pack([row])
    for name,offset,value in [('zero_identity',0,0),('invalid_boolean',3,2)]:
        row=copy.deepcopy(live[0]);row['pools'][0][0][offset]=value;bad[name]=pack([row])
    row=copy.deepcopy(live[0]);row['pools'][0]=[row['pools'][0][0]]*513;bad['excess_pool']=pack([row])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);checks={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Contact collection mismatch; inspect private report')


if __name__=='__main__':main()
