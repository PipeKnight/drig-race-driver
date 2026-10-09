"""Authored complete-callback cases; never presented as live callback returns."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from check_impact_producer_controls import controls as producer_controls
from check_impact_generation_controls import floats
from validate_collision_callback import ROOT,PROBE_SHA,pack,compare
from validate_steering_control import digest


def controls(producer,callback):
    rng=random.Random(20081009);rows,bodies=producer_controls(producer);policy=callback['cases'][0]
    for i,r in enumerate(rows):
        for key in ['body_d16c_hex','body_d22c_hex','global_d1676c_hex','preparation_scale_hex','preparation_table_hex']:r[key]=policy[key]
        mode=i%8;r['callback_global_disable']=int(mode==0)
        d=bytearray.fromhex(r['descriptor_before_hex']);d[0xa4af]=int(mode==1);struct.pack_into('<f',d,0xa494,rng.choice([0.,1.,30.,100.,1000.]));struct.pack_into('<I',d,0xa4a0,rng.choice([0,1,2]))
        linked=bytearray.fromhex(r['linked_before_hex']);linked[0x3f0:0x3f4]=bytes.fromhex(policy['linked_before_hex'])[0x3f0:0x3f4]
        if mode==2:struct.pack_into('<f',d,0xa494,0.)
        if mode==3:struct.pack_into('<I',d,0xa4a0,1)
        if mode>=4:
            r['body_d16c_hex']=r['body_d22c_hex']=r['global_d1676c_hex']=struct.pack('<f',1.).hex();struct.pack_into('<f',d,0xa494,1000.);struct.pack_into('<f',linked,0x3f0,1.);struct.pack_into('<I',d,0xa4a0,0)
            r['preparation_scale_hex']=struct.pack('<f',1.).hex();r['preparation_table_hex']=struct.pack('<16f',*([1.]*16)).hex()
        # Table's last two floats alias the first two captured solver-global words.
        sg=bytearray.fromhex(r['solver_globals_hex']);sg[:8]=bytes.fromhex(r['preparation_table_hex'])[-8:];r['solver_globals_hex']=sg.hex()
        r.update(descriptor_before_hex=d.hex(),linked_before_hex=linked.hex())
    for i in range(16):
        r=copy.deepcopy(rows[i]);r['contact_count']=0;r['contacts']=[];r['response_before_hex']='';r['callback_global_disable']=0
        d=bytearray.fromhex(r['descriptor_before_hex']);d[0xa4af]=0;r['descriptor_before_hex']=d.hex();rows.append(r);bodies.append(copy.deepcopy(bodies[i]))
    return rows,bodies


def inspect(data,rows,bodies):
    cursor=8;counts=dict(handled_cases=0,guard_bypass_cases=0,empty_contact_bypass_cases=0,activation_cases=0,rng_advanced_cases=0,angular_changed_cases=0,response_changed_cases=0)
    for r,b in zip(rows,bodies):
        d=data[cursor:cursor+0xa500];cursor+=0xa500;linked=data[cursor:cursor+0x424];cursor+=0x424;state=data[cursor:cursor+48];cursor+=48
        response=data[cursor:cursor+r['contact_count']*4];cursor+=len(response);handled=struct.unpack_from('<I',data,cursor)[0];cursor+=4
        if handled not in (0,1):raise ValueError('Callback AL outside contract')
        incoming=bytes.fromhex(r['linked_before_hex']);incoming_d=bytearray.fromhex(r['descriptor_before_hex']);incoming_d[0xa4f8:0xa4fc]=bytes(4)
        counts['handled_cases']+=handled;counts['guard_bypass_cases']+=not handled
        counts['empty_contact_bypass_cases']+=r['contact_count']==0 and not handled
        counts['activation_cases']+=struct.unpack_from('<I',d,0xa4a0)[0]==1 and struct.unpack_from('<I',incoming_d,0xa4a0)[0]!=1
        counts['rng_advanced_cases']+=struct.unpack_from('<I',state,44)[0]!=b['seed'];counts['angular_changed_cases']+=linked[0x210:0x220]!=incoming[0x210:0x220]
        counts['response_changed_cases']+=response!=bytes.fromhex(r['response_before_hex'])
        if not handled and (d!=incoming_d or linked!=incoming or state!=b['maximum']+b['extra']+struct.pack('<I',b['seed']) or response!=bytes.fromhex(r['response_before_hex'])):raise ValueError('Callback guard modified state')
    if cursor!=len(data):raise ValueError('Callback controls trailing output')
    return counts


def main():
    parser=argparse.ArgumentParser();parser.add_argument('producer_capture',type=Path);parser.add_argument('callback_capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    p,c,out=private(args.producer_capture),private(args.callback_capture),private(args.output)
    captures=[]
    for path in [p,c]:
        if path.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
        capture=json.loads(path.read_text(encoding='utf-8'))
        if not(capture['completed'] and capture['debugger_exit']==0 and capture['code_exact'] and capture['hardware_breakpoints'] and capture['session']['ExeSHA256']==REFERENCE):raise ValueError('Unverified source capture')
        captures.append(capture)
    rows,bodies=controls(*captures);image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/collision-callback-probe.exe';native=ROOT/f'target/{args.profile}/grid-collision-callback.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Reference/probe changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(rows,bodies));original=None
    report=dict(live_verification=False,scope='Authored whole callback contacts/grid/body/policy. Two prior captures provide templates; these hybrid inputs are not an observed simultaneous state.',seed=20081009,cases=len(rows),producer_capture_sha256=digest(p),callback_capture_sha256=digest(c),probe_sha256=digest(probe),native_sha256=digest(native),profile=args.profile,runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if original is None:original=actual
            r.update(compare(actual,original,len(rows)));r.update(inspect(actual,rows,bodies))
        report['runs'][label]=r
    good=pack([rows[6]],[bodies[6]]);bad={'trailing':good+b'\0','truncated':good[:-1],'zero_cases':good[:4]+bytes(4)+good[8:]}
    for name,key,offset,values in [('zero_spacing','descriptor_before_hex',0xa420,[0.]),('nan_node','descriptor_before_hex',0x2580,[float('nan')]),('nan_intensity','descriptor_before_hex',0xa494,[float('nan')]),('zero_linked_mass','linked_before_hex',0x3f0,[0.]),('zero_factor','body_d16c_hex',0,[0.]),('nan_table','preparation_table_hex',0,[float('nan')])]:
        row=copy.deepcopy(rows[6]);floats(row,key,offset,values);bad[name]=pack([row],[bodies[6]])
    row=copy.deepcopy(rows[6]);row['callback_global_disable']=2;bad['invalid_global_disable']=pack([row],[bodies[6]])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);checks={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            checks[label]=dict(exit=run.returncode,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed']=all(r.get('passed',False) for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values()) and all(report['runs']['original'].get(k,0)>0 for k in ('handled_cases','guard_bypass_cases','activation_cases','rng_advanced_cases','angular_changed_cases','response_changed_cases'))
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole callback controls failed; inspect private report')


if __name__=='__main__':main()
