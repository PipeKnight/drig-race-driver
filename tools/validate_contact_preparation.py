"""Whole original descriptor preparation, captured inactive calls and controls."""
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
PROBE_SHA='cb4a7c38aa3130f91567aae87ceef5067e3ea9578f3f070ba2aafbbde99cc268'
SIZE=0xa500+0x424+16+64+16+4


def from_live(c):
    return dict(d=blob(c,'descriptor_before_hex',0xa500),s=blob(c,'settings_hex',0x1050),linked=blob(c,'linked_before_hex',0x424),
                direction=blob(c,'direction_before_hex',16),inverse=blob(c,'inverse_hex',64),point=blob(c,'point_before_hex',16),
                factor_a=blob(c,'body_d16c_hex',4),factor_b=blob(c,'body_d22c_hex',4),factor_c=blob(c,'global_d1676c_hex',4),
                scale=blob(c,'preparation_scale_hex',4),table=blob(c,'preparation_table_hex',64))


def pack(rows):
    return struct.pack('<II',0x31504349,len(rows))+b''.join(b''.join(r[k] for k in ['d','s','linked','direction','inverse','point','factor_a','factor_b','factor_c','scale','table']) for r in rows)


def controls(template):
    rng=random.Random(20081009);rows=[]
    for i in range(256):
        r=copy.deepcopy(template);d=bytearray(r['d']);s=bytearray(r['s']);linked=bytearray(r['linked']);mode=i%16
        struct.pack_into('<f',d,0xa494,rng.choice([0.,1.,10.,20.,30.,50.,65.,100.,1000.]));struct.pack_into('<I',d,0xa4a0,rng.choice([0,1,2]));d[0xa4af]=int(mode==2)
        struct.pack_into('<f',linked,0x3f0,rng.uniform(.5,50));r['direction']=struct.pack('<4f',rng.uniform(-200,200),rng.uniform(-200,200),rng.uniform(-200,200),rng.choice([0.,1.,-1.]))
        r['point']=struct.pack('<4f',*(rng.uniform(-10,10) for _ in range(4)))
        r['inverse']=struct.pack('<16f',*(rng.uniform(-3,3) for _ in range(16)))
        r['factor_a']=struct.pack('<f',rng.uniform(.1,10));r['factor_b']=struct.pack('<f',rng.uniform(.1,3));r['factor_c']=struct.pack('<f',rng.uniform(.1,3));r['scale']=struct.pack('<f',rng.uniform(.01,2));r['table']=struct.pack('<16f',*(rng.uniform(.01,2) for _ in range(16)))
        if mode==0:struct.pack_into('<f',d,0xa494,0.)
        if mode==1:struct.pack_into('<I',d,0xa4a0,1)
        if mode==3:r['direction']=bytes(16)
        if mode==4:struct.pack_into('<f',d,0xa494,-20.)
        if mode==5:struct.pack_into('<f',d,0xa494,1000.)
        if mode==6:r['factor_a']=struct.pack('<f',1.);r['factor_b']=struct.pack('<f',1.);r['factor_c']=struct.pack('<f',1.);struct.pack_into('<f',d,0xa494,1.)
        if mode==7:r['direction']=struct.pack('<4f',0.,100.,0.,0.);struct.pack_into('<16f',linked,0x1d0,*[float(j//4==j%4) for j in range(16)])
        if mode==8:r['direction']=struct.pack('<4f',0.,-100.,0.,0.);struct.pack_into('<16f',linked,0x1d0,*[float(j//4==j%4) for j in range(16)])
        if mode==9:r['direction']=struct.pack('<4f',-0.,0.,-0.,-0.)
        if mode==10:r['scale']=struct.pack('<f',0.)
        if mode==11:r['table']=bytes(64)
        # Exercise original9A26F0 mode1, with bounded displaced nodes and edges.
        if mode>=12:
            struct.pack_into('<I',d,0xa4a0,0);d[0xa4af]=0;struct.pack_into('<f',d,0xa494,1000.);r['direction']=struct.pack('<4f',1000.,-500.,800.,0.)
            r['factor_a']=r['factor_b']=r['factor_c']=struct.pack('<f',1.);struct.pack_into('<f',linked,0x3f0,1.)
            r['scale']=struct.pack('<f',1.);r['table']=struct.pack('<16f',*([1.]*16))
            dimensions=struct.unpack_from('<3I',d,0xa414);count=dimensions[0]*dimensions[1]*dimensions[2]
            for j in range(count):
                rest=struct.unpack_from('<4f',d,0x2580+j*80+16)
                struct.pack_into('<4f',d,0x2580+j*80,*[rest[k]+rng.uniform(-2,2) for k in range(4)])
            struct.pack_into('<2f',s,0x1024,rng.uniform(0,1),rng.uniform(0,1))
        r.update(d=d,s=s,linked=linked);rows.append(r)
    def adjacent(value,delta):
        return struct.unpack('<f',struct.pack('<I',struct.unpack('<I',struct.pack('<f',value))[0]+delta))[0]
    for i in range(256):
        r=copy.deepcopy(template);d=bytearray(r['d']);linked=bytearray(r['linked'])
        r['factor_a']=r['factor_b']=r['factor_c']=struct.pack('<f',1.);r['scale']=struct.pack('<f',1.)
        struct.pack_into('<I',d,0xa4a0,0);d[0xa4af]=0;struct.pack_into('<16f',linked,0x1d0,*[float(j//4==j%4) for j in range(16)])
        r['inverse']=linked[0x1d0:0x210];r['point']=struct.pack('<4f',1.,-2.,3.,-5.)
        if i<128:
            struct.pack_into('<f',d,0xa494,adjacent(1.,rng.choice([-1,0,1])))
            struct.pack_into('<f',linked,0x3f0,adjacent(1.,rng.choice([-1,0,1])))
            r['table']=struct.pack('<16f',*([1.]*16));r['direction']=struct.pack('<4f',adjacent(1.,rng.choice([-1,0,1])),-0.,0.,0.)
        else:
            value=rng.choice([0.,10.,20.,30.,70.,140.,150.])/struct.unpack('<f',struct.pack('<I',0x400ed917))[0]
            if value:value=adjacent(value,rng.choice([-1,0,1]))
            struct.pack_into('<f',d,0xa494,value);struct.pack_into('<f',linked,0x3f0,1.)
            r['table']=struct.pack('<16f',*(rng.uniform(-2,2) for _ in range(16)));r['direction']=struct.pack('<4f',20.,0.,-3.,0.)
        r.update(d=d,linked=linked);rows.append(r)
    return rows


def compare(actual,wanted,count,skip_scalar=False):
    if len(actual)!=8+count*SIZE or actual[:8]!=struct.pack('<II',0x31504349,count) or len(actual)!=len(wanted):raise ValueError('Preparation framing mismatch')
    offsets=[i for i in range(8,len(actual),4) if not(skip_scalar and (i-8)%SIZE==SIZE-4)]
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in offsets if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_words=len(offsets),mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def expected(cases):
    data=[struct.pack('<II',0x31504349,len(cases))]
    for c in cases:
        d=bytearray(blob(c,'descriptor_after_hex',0xa500));d[0xa4f8:0xa4fc]=bytes(4)
        data.extend([d,blob(c,'linked_after_hex',0x424),blob(c,'direction_after_hex',16),blob(c,'inverse_after_hex',64),blob(c,'point_after_hex',16),bytes(4)])
    return b''.join(data)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'));parser.add_argument('--controls',action='store_true');args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    c=json.loads(source.read_text(encoding='utf-8'));cases=c['cases']
    if not(c['completed'] and c['debugger_exit']==0 and c['code_exact'] and c['hardware_breakpoints'] and c['boundary']=='preparation' and c['session']['ExeSHA256']==REFERENCE and 1<=len(cases)<=32):raise ValueError('Unverified preparation capture')
    rows=controls(from_live(cases[0])) if args.controls else [from_live(r) for r in cases];wanted=None if args.controls else expected(cases)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/contact-preparation-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows));original=None
    report=dict(scope='Whole originalA06050 and helpers including mode1 settling; no imports/stubs/skips/code patches. Live x87 return absent, compared separately as storedf32 only between replay implementations.',authored_controls=args.controls,cases=len(rows),profile=args.profile,capture_sha256=digest(source),probe_sha256=digest(probe),runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)])]
    if args.profile:
        native=ROOT/f'target/{args.profile}/grid-contact-preparation.exe';report['native_sha256']=digest(native);commands.append(('rust',[str(native)]))
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if original is None:original=actual
            r.update(compare(actual,original,len(rows)))
            r['active_cases']=sum(struct.unpack_from('<I',actual,8+i*SIZE+0xa4a0)[0]==1 and struct.unpack_from('<I',rows[i]['d'],0xa4a0)[0]!=1 for i in range(len(rows)))
            r['node_change_cases']=sum(actual[8+i*SIZE+0x2580:8+i*SIZE+0xa280]!=rows[i]['d'][0x2580:0xa280] for i in range(len(rows)))
            r['edge_change_cases']=sum(actual[8+i*SIZE:8+i*SIZE+0x2580]!=rows[i]['d'][:0x2580] for i in range(len(rows)))
            if wanted is not None:r['live']=compare(actual,wanted,len(rows),True);r['passed'] &= r['live']['passed']
        report['runs'][label]=r
    good=pack([from_live(cases[0])]);bad={'trailing':good+b'\0','truncated':good[:-1],'zero_cases':good[:4]+bytes(4)+good[8:],'excess_cases':good[:4]+struct.pack('<I',1025)+good[8:]}
    for name,key,offset,fmt,value in [('zero_factor','factor_a',0,'f',0.),('zero_mass','linked',0x3f0,'f',0.),('nan_up','linked',0x1e0,'f',float('nan')),('nan_table','table',0,'f',float('nan')),('infinite_direction','direction',0,'f',float('inf')),('negative_bound','d',0x25a0,'f',-1.),('invalid_mode','d',0xa4a0,'I',3),('invalid_disable','d',0xa4af,'B',2),('excess_edges','d',0xa410,'I',1201),('invalid_dimension','d',0xa414,'I',1),('invalid_endpoint','d',0,'H',400)]:
        r=from_live(cases[0]);r[key]=bytearray(r[key]);struct.pack_into('<'+fmt,r[key],offset,value);bad[name]=pack([r])
    for name,data in bad.items():
        guard=out/f'guard-{name}.bin';guard.write_bytes(data);checks={}
        for label,command in [command for command in commands if command[0]!='repeat']:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(guard),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    if report['passed'] and args.profile and not args.controls:
        inverse_request=out/'inverse-input.bin';inverse_target=out/'inverse-rust.bin';inverse_native=ROOT/f'target/{args.profile}/grid-contact-inverse.exe'
        inverse_request.write_bytes(struct.pack('<II',0x31494349,len(rows))+b''.join(r['linked'][0x1d0:0x210] for r in rows))
        run=subprocess.run([str(inverse_native),str(inverse_request),str(inverse_target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        pipeline=dict(inverse_exit=run.returncode,inverse_native_sha256=digest(inverse_native),inverse_stdout=run.stdout,inverse_stderr=run.stderr)
        if run.returncode==0:
            inverses=inverse_target.read_bytes()
            if inverses[:8]!=struct.pack('<II',0x31494349,len(rows)) or len(inverses)!=8+64*len(rows):raise ValueError('Inverse pipeline framing')
            derived=copy.deepcopy(rows)
            for i,row in enumerate(derived):row['inverse']=inverses[8+i*64:8+(i+1)*64]
            pipeline_request=out/'pipeline-input.bin';pipeline_target=out/'pipeline-rust.bin';pipeline_request.write_bytes(pack(derived))
            run=subprocess.run([str(native),str(pipeline_request),str(pipeline_target)],cwd=ROOT,capture_output=True,text=True,timeout=60);pipeline.update(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
            if run.returncode==0:pipeline.update(compare(pipeline_target.read_bytes(),original,len(rows)))
        report['inverse_pipeline']=pipeline;report['passed'] &= pipeline.get('passed',False)
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Contact preparation failed; inspect private report')


if __name__=='__main__':main()
