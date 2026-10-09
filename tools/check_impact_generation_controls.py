"""Authored boundary controls; kept distinct from the actual live comparisons."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private
from validate_impact_generation import pack, decode, ROOT, PROBE_SHA
from validate_steering_control import digest, PE_SHA


def floats(row,key,offset,values):
    data=bytearray.fromhex(row[key]);struct.pack_into('<'+'f'*len(values),data,offset,*values);row[key]=data.hex()


def controls(base):
    rng=random.Random(20081009)
    rows=[]
    for index in range(192):
        row=copy.deepcopy(base[index%len(base)])
        mode=index%16
        for contact in row['contacts']:
            data=bytearray.fromhex(contact['before_hex'])
            if mode==0: data[0x73]=0
            if mode==12: struct.pack_into('<f',data,0x3c,0.)
            if mode==7:
                data[0xc]=0;struct.pack_into('<II',data,4,0x10000,int(row['linked'],16))
                struct.pack_into('<2f',data,0x40,0.,rng.uniform(100,10000))
                contact['counterpart']='0x10000'
                contact['counterpart_360_hex']=struct.pack('<4f',*(rng.uniform(-2,2) for _ in range(3)),0.).hex()
            if mode==5:
                data[0xc]=0;struct.pack_into('<I',data,8,0x10000)
                struct.pack_into('<2f',data,0x40,rng.uniform(100,10000),0.)
                contact['counterpart']='0x10000';contact['counterpart_360_hex']=bytes(16).hex()
            if mode==11:
                struct.pack_into('<4f',data,0x60,*(rng.uniform(-1,1) for _ in range(3)),rng.choice([0.,0.25]))
            if mode==15: struct.pack_into('<4f',data,0x60,-0.,0.,-0.,0.)
            contact['before_hex']=data.hex()
        if mode==1:
            for i in range(400):floats(row,'descriptor_before_hex',0x25c0+i*80,[0.])
        if mode==2:floats(row,'descriptor_before_hex',0xa4b4,[10.]*4)
        if mode==3:
            floats(row,'descriptor_before_hex',0xa4b4,[0.]*4)
            floats(row,'descriptor_before_hex',0xa4c4,[rng.uniform(-.1,.1) for _ in range(10)])
        if mode==4:
            floats(row,'settings_hex',0x1030,[0.,rng.choice([-.2,.01,.95,2.]),rng.uniform(-.5,1.5)])
            data=bytearray.fromhex(row['descriptor_before_hex']);data[0xa280:0xa410]=bytes(rng.randrange(256) for _ in range(400));row['descriptor_before_hex']=data.hex()
        if mode==6:
            floats(row,'descriptor_before_hex',0xa480,[-1.])
            row['scalar_word']=struct.pack('>f',rng.choice([-.25,0.,.25,1.,2.])).hex()
        if mode in (8,10,15):
            floats(row,'linked_before_hex',0x1d0,[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.])
            floats(row,'body_1630_hex',0,[0.]*4)
            for contact in row['contacts']:
                floats(contact,'before_hex',0x50,[rng.choice([-.30000001192092896,0.,.30000001192092896]),.0,rng.choice([-.30000001192092896,0.,.30000001192092896]),1.])
                if mode==8:floats(contact,'before_hex',0x60,[rng.choice([-1.,0.,1.]),rng.choice([-1.,0.,1.]),rng.choice([-1.,0.,1.]),rng.choice([0.,.25])])
        if mode==9:
            row['contacts']=[copy.deepcopy(row['contacts'][0]) for _ in range(80)];row['contact_count']=80
            floats(row,'producer_globals_hex',16,[.001,10000.])
            floats(row,'descriptor_before_hex',0xa4b4,[0.]*4)
            for i in range(400):floats(row,'descriptor_before_hex',0x25c0+i*80,[1.])
        if mode==11:floats(row,'body_b20_hex',0,[rng.uniform(-3,3) for _ in range(4)])
        if mode==13:
            floats(row,'body_b20_hex',0,[0.]*4);floats(row,'settings_hex',0x1030,[0.,1.,.15])
        if mode==14:
            data=bytearray.fromhex(row['descriptor_before_hex']);struct.pack_into('<3i',data,0xa414,2,2,2);row['descriptor_before_hex']=data.hex()
        row.pop('solver',None)
        rows.append(row)
    return rows


def expected(data,rows):
    if data[:8]!=struct.pack('<II',0x31475049,len(rows)):raise ValueError('Original header mismatch')
    cursor=8;result=copy.deepcopy(rows)
    for row in result:
        count,=struct.unpack_from('<I',data,cursor);cursor+=4
        d=data[cursor:cursor+0xa500];cursor+=0xa500
        r=data[cursor:cursor+count*48];cursor+=count*48
        row['solver']=dict(record_count=count,records_hex=r.hex(),descriptor_before_hex=d.hex())
    if cursor!=len(data):raise ValueError('Original output length mismatch')
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    capture=json.loads(source.read_text(encoding='utf-8'))
    if not capture['completed'] or capture['debugger_exit']!=0:raise ValueError('Unverified capture')
    rows=controls(capture['cases']);out.mkdir(parents=True,exist_ok=False);(out/'input.bin').write_bytes(pack(rows))
    probe=ROOT/'local/behavior/impact-generation-probe.exe';image=ROOT/'local/compatibility/grid-unpacked.exe'
    native=ROOT/f'target/{args.profile}/grid-impact-generation.exe'
    if digest(image)!=PE_SHA or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe changed')
    report=dict(scope='192 authored controls derived from retained input pools; not additional live observations',
                capture_sha256=digest(source),probe_sha256=digest(probe),native_sha256=digest(native),profile=args.profile,runs={})
    expected_rows=None
    for label,command in [('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(out/'input.bin'),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        record=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr);report['runs'][label]=record
        if run.returncode:break
        if expected_rows is None:expected_rows=expected(target.read_bytes(),rows)
        comparison=decode(target.read_bytes(),expected_rows)
        record.update(cases=len(comparison),records=sum(c['records'] for c in comparison),
                      record_words=sum(c['compared_record_words'] for c in comparison),descriptor_words=len(comparison)*0xa500//4,
                      passed=all(c['passed'] for c in comparison),failures=[dict(index=i,**c) for i,c in enumerate(comparison) if not c['passed']][:20])
    report['passed']=len(report['runs'])==3 and all(r.get('passed',False) for r in report['runs'].values())
    report['guards']=[]
    for index in range(8):
        row=copy.deepcopy(capture['cases'][0])
        if index==0:floats(row,'descriptor_before_hex',0xa420,[0.])
        if index==1:
            d=bytearray.fromhex(row['descriptor_before_hex']);struct.pack_into('<I',d,0xa414,1);row['descriptor_before_hex']=d.hex()
        if index==2:floats(row,'descriptor_before_hex',0x2580,[float('nan')])
        if index==3:floats(row,'body_b20_hex',0,[float('inf')])
        if index==4:floats(row,'settings_hex',0x1038,[float('nan')])
        if index==5:floats(row,'producer_static_mass_hex',0,[float('nan')])
        if index==6:floats(row['contacts'][0],'before_hex',0x50,[float('nan')])
        if index==7:row['contacts']=[];row['contact_count']=0
        request=out/f'guard-{index}.bin';request.write_bytes(pack([row]));guard=dict(index=index)
        for label,command in [('original',[str(probe),str(image)]),('rust',[str(native)])]:
            target=out/f'guard-{index}-{label}.bin'
            run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            guard[label]=dict(exit=run.returncode,output_exists=target.exists())
        guard['passed']=guard['original']['exit']!=0 and guard['rust']['exit']!=0 and not guard['rust']['output_exists']
        report['guards'].append(guard)
    report['passed'] &= all(g['passed'] for g in report['guards'])
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Authored control comparison failed')


if __name__=='__main__':main()
