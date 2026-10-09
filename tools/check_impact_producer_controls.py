"""Authored whole-collision controls, kept separate from live-return evidence."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,ANALYSIS
from check_impact_generation_controls import controls as generation_controls,floats
from check_impact_body_controls import controls as body_controls
from validate_impact_producer import pack,ROOT,PROBE_SHA
from validate_steering_control import digest


def controls(capture):
    rng=random.Random(20081009);generated=generation_controls(capture['cases']);bodies=body_controls(capture);rows=[]
    for index,body in enumerate(bodies):
        row=copy.deepcopy(generated[index%len(generated)])
        linked=bytearray.fromhex(row['linked_before_hex']);linked[0x210:0x230]=body['linked'][0x210:0x230]
        if index%5==0:struct.pack_into('<4f',linked,0x360,*(rng.uniform(-2,2) for _ in range(3)),0.)
        struct.pack_into('<4f',linked,0x380,*(rng.uniform(-.1,.1) for _ in range(4)))
        linked[0x390]=index%2;row['linked_before_hex']=linked.hex()
        row['prepared_a_hex']=body['a'].hex();row['prepared_b_hex']=body['b'].hex()
        row['response']='0x10000' if (index+index//16)%4 else '0x0'
        values=[rng.uniform(-2,2) for _ in range(row['contact_count'])]
        row['response_before_hex']=struct.pack('<'+'f'*row['contact_count'],*(values if int(row['response'],16) else [0.]*row['contact_count'])).hex()
        floats(row,'producer_globals_hex',0,[rng.choice([0.,.25,1.,2.])])
        g=bytearray.fromhex(row['producer_globals_hex']);g[4]=(index//4)%2;row['producer_globals_hex']=g.hex()
        row['response_scale_hex']=struct.pack('<f',rng.choice([0.,.5,1.,2.])).hex()
        sg=bytearray.fromhex(row['solver_globals_hex']);struct.pack_into('<I',sg,12,rng.choice([1,2,4,10]));row['solver_globals_hex']=sg.hex()
        row['solver_clip_hex']=bytes([index%2]).hex();rows.append(row)
    return rows,bodies


def inspect(data,rows,bodies):
    if data[:8]!=struct.pack('<II',0x31574349,len(rows)):raise ValueError('Output header mismatch')
    cursor=8;counts=dict(rng_advanced_cases=0,angular_changed_cases=0,accumulator_changed_cases=0,response_changed_cases=0,empty_bypass_cases=0)
    for index,(row,body) in enumerate(zip(rows,bodies)):
        d=data[cursor:cursor+0xa500];cursor+=0xa500
        linked=data[cursor:cursor+0x424];cursor+=0x424
        state=data[cursor:cursor+48];cursor+=48
        response=data[cursor:cursor+row['contact_count']*4];cursor+=len(response)
        if len(d)!=0xa500 or len(linked)!=0x424 or len(state)!=48 or len(response)!=row['contact_count']*4:raise ValueError('Output row mismatch')
        incoming=bytes.fromhex(row['linked_before_hex']);seed=struct.unpack_from('<I',state,44)[0]
        counts['rng_advanced_cases']+=seed!=body['seed']
        counts['angular_changed_cases']+=linked[0x210:0x220]!=incoming[0x210:0x220]
        counts['accumulator_changed_cases']+=linked[0x380:0x390]!=incoming[0x380:0x390]
        counts['response_changed_cases']+=response!=bytes.fromhex(row['response_before_hex'])
        # Generation mode0 disables every contact; the original returns without body/solver/response.
        if (index%192)%16==0:
            if linked!=incoming or state!=body['maximum']+body['extra']+struct.pack('<I',body['seed']) or response!=bytes.fromhex(row['response_before_hex']):raise ValueError('Empty-record bypass changed body/response')
            counts['empty_bypass_cases']+=1
    if cursor!=len(data):raise ValueError('Output trailing bytes')
    return counts


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    capture=json.loads(source.read_text(encoding='utf-8'))
    if not(capture['completed'] and capture['debugger_exit']==0 and capture['body_code_exact'] and capture['solver_code_exact']):raise ValueError('Unverified capture')
    rows,bodies=controls(capture);image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/impact-producer-probe.exe';native=ROOT/f'target/{args.profile}/grid-impact-producer.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows,bodies));original=None
    report=dict(authored_cases=len(rows),seed=20081009,live_verification=False,scope='Whole original9EA6E0 versus single Rust call path; supplied authored contacts/body/response and controlled random seeds; no helper stubs.',capture_sha256=digest(source),profile=args.profile,probe_sha256=digest(probe),native_sha256=digest(native),runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes();r.update(inspect(actual,rows,bodies))
            if original is None:original=actual
            if len(actual)!=len(original):raise ValueError('Comparison length mismatch')
            differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=original[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=original[i:i+4]]
            r.update(compared_words=(len(actual)-8)//4,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)
        report['runs'][label]=r
    good=pack([rows[1]],[bodies[1]]);bad={'truncated':good[:-1],'trailing':good+b'\0','zero_cases':good[:4]+bytes(4)+good[8:]}
    for name,field,offset,value in [('zero_spacing','descriptor_before_hex',0xa420,[0.]),('nan_node','descriptor_before_hex',0x2580,[float('nan')]),('infinite_direction','prepared_b_hex',0,[float('inf')])]:
        row=copy.deepcopy(rows[1]);floats(row,field,offset,value);bad[name]=pack([row],[bodies[1]])
    for name,key,value in [('zero_mass','mass',struct.pack('<f',0.)),('zero_range','globals',struct.pack('<5f',1.,.1,0.,7.,6.4))]:
        body=copy.deepcopy(bodies[1]);body[key]=value;bad[name]=pack([rows[1]],[body])
    row=copy.deepcopy(rows[1]);row['solver_clip_hex']='02';bad['invalid_clip']=pack([row],[bodies[1]])
    row=copy.deepcopy(rows[1]);row['contacts']=[];row['contact_count']=0;row['response_before_hex']='';bad['zero_contacts']=pack([row],[bodies[1]])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);checks={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values()) and all(report['runs']['original'].get(k,0)>0 for k in ('rng_advanced_cases','angular_changed_cases','accumulator_changed_cases','response_changed_cases','empty_bypass_cases'))
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole-producer authored controls failed; inspect private report')


if __name__=='__main__':main()
