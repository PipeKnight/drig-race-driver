"""Whole state transfer/advance/correction, fresh suspension handoffs, carried state.

The call schedule and physical bodies/shapes are authored, not a captured frame.
"""
import argparse
import copy
import json
import math
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_steering_control import digest
from validate_contact_bias import compare
from validate_suspension_contact import TRACK, SURFACE, WORLD, MECHANICS
from validate_physical_pose import case, floats, words, POINTERS, IDENTITY

ROOT = Path(__file__).resolve().parents[1]
PROBE = '8de9c4c36249a546fa2c24f156fea8ee958298804a630129625ed44ca988bb63'
SUSPENSION_PROBE = 'fe1e71a2c825d062cbb789aa520adad81861d9bd4d4ec92b6256d6c7a4a75396'
MAGIC = 0x31425656


def base(rng, index):
    c = case(rng, head=index % 2, shapes=[1,0,2,3,7,16,64][index % 7])
    c.update(vehicle=bytearray(rng.randbytes(0x2600)), notify=index % 2,
             advance=int(index % 5 != 0), correction=int(index % 3 != 0), delta=[.000001,.001,.006130263675,.016666667,.25][index % 5])
    words(c['body'],0x400,[index % 3])
    for i,s in enumerate(c['shapes']): words(s,0x170,[(i+index) % 3])
    floats(c['vehicle'],0x15e0,IDENTITY)
    floats(c['vehicle'],0x15c0,[0.]*8)
    return c


def controls():
    rng=random.Random(2008169); result=[]
    for i in range(96):
        c=base(rng,i)
        if i>=8:
            angle=rng.uniform(-math.pi,math.pi);co,si=math.cos(angle),math.sin(angle)
            pose=[co,0.,-si,0.,0.,1.,0.,0.,si,0.,co,0.]+[rng.uniform(-500,500) for _ in range(3)]+[1.]
            if i % 4==0: pose[:12]=[v+rng.uniform(-.05,.05) for v in pose[:12]]
            floats(c['vehicle'],0x15e0,pose)
            floats(c['vehicle'],0x15c0,[rng.uniform(-40,40) for _ in range(4)]+[rng.uniform(-4,4) for _ in range(4)])
            if i % 8==0: floats(c['vehicle'],0x15d0,[0.,-0.,1e-25,0.])
            for off in [0x90,0x150]: floats(c['body'],off,[rng.uniform(-3,3) for _ in range(16)])
            floats(c['body'],0x230,[rng.uniform(-2,2) for _ in range(3)]+[0. if i % 2 else rng.uniform(-.2,.2)])
            c['linear']=[rng.uniform(-1,1) for _ in range(4)]; c['angular']=[rng.uniform(-.5,.5) for _ in range(4)]
            for s in c['shapes']: floats(s,0xf0,[rng.uniform(-2,2) for _ in range(16)])
        # These lanes are overwritten, including nonfinite values, by the setter.
        for r,v in enumerate([0x7fc00001,0xff800000,0x7f800000,0x7f800001] if i % 6==0 else [0x80000000]*4):
            words(c['vehicle'],0x15ec+r*16,[v])
        if i==1:
            floats(c['vehicle'],0x1610,[10.,20.,30.,9.]);floats(c['body'],0x230,[.25,.5,.75,0.])
            floats(c['vehicle'],0x15c0,[2.,4.,6.,0.]);c['delta']=.25;c['correction']=0
        if i==2: c['linear']=[1.,2.,3.,0.];c['angular']=[0.]*4;c['advance']=0;c['head']=0
        result.append(c)
    return result


def pack(cases):
    out=bytearray(struct.pack('<II',MAGIC,len(cases)))
    for c in cases:
        out.extend(struct.pack('<5If',len(c['shapes']),c['notify'],c['advance'],c['correction'],c['head'],c['delta']))
        out.extend(c['body']);out.extend(c['vehicle']);out.extend(struct.pack('<8f',*c['linear'],*c['angular']))
        for s in c['shapes']:out.extend(s)
    return out


def decode(data,cases):
    if data[:8]!=struct.pack('<II',MAGIC,len(cases)):raise ValueError('Header')
    cursor=8;out=[]
    for c in cases:
        expected=struct.pack('<5If',len(c['shapes']),c['notify'],c['advance'],c['correction'],c['head'],c['delta'])
        if data[cursor:cursor+24]!=expected:raise ValueError('Case header')
        cursor+=24; stages=[]
        for _ in range(3):
            p=data[cursor:cursor+0x424];cursor+=0x424;v=data[cursor:cursor+0x2600];cursor+=0x2600
            shapes=[]
            for _ in c['shapes']:shapes.append(data[cursor:cursor+0x220]);cursor+=0x220
            stages.append(dict(body=p,vehicle=v,shapes=shapes))
        out.append(stages)
    if cursor!=len(data):raise ValueError('Output length')
    return out


def execute(command,source,target):
    run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
    record=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
    if run.returncode:raise RuntimeError(f'Execution failed: {record}')
    return target.read_bytes(),record


def checks(cases,result):
    checks=dict(canonical_setter_lanes=True,readback_publication=True,readback_velocities=True,
        unrelated_vehicle_bytes=True,body_history=True,shape_history=True,shape_movement=True,
        notifications_preserved_through_motion=True,disabled_stages_preserve_state=True)
    for c,stages in zip(cases,result):
        written,advanced,corrected=stages
        p=written['body'];checks['canonical_setter_lanes'] &= [struct.unpack_from('<I',p,o)[0] for o in [0x1dc,0x1ec,0x1fc,0x20c]]==[0,0,0,0x3f800000]
        checks['body_history'] &= p[0x40:0x80]==p[0x1d0:0x210]
        for s in stages:
            checks['readback_publication'] &= s['vehicle'][0x15e0:0x1620]==s['body'][0x1d0:0x210]
            checks['readback_velocities'] &= s['vehicle'][0x15c0:0x15d0]==s['body'][0x220:0x230] and s['vehicle'][0x15d0:0x15e0]==s['body'][0x210:0x220]
            checks['unrelated_vehicle_bytes'] &= s['vehicle'][:0x15c0]==c['vehicle'][:0x15c0] and s['vehicle'][0x1620:]==c['vehicle'][0x1620:]
        if c['advance']:
            checks['body_history'] &= advanced['body'][0x270:0x2b0]==p[0x190:0x1d0] and advanced['body'][0x2b0:0x2f0]==p[0x1d0:0x210]
            checks['body_history'] &= advanced['body'][0x360:0x370]==bytes(16)
        else:checks['disabled_stages_preserve_state'] &= written==advanced
        if not c['correction']:checks['disabled_stages_preserve_state'] &= advanced==corrected
        checks['notifications_preserved_through_motion'] &= all(s['body'][0x40:0x80]==p[0x40:0x80] for s in stages)
        for before,w,a,z in zip(c['shapes'],written['shapes'],advanced['shapes'],corrected['shapes']):
            selector=struct.unpack_from('<I',before,0x170)[0]
            expected=w[0x130:0x170] if selector==1 else before[0xf0:0x130] if selector==2 else before[0x70:0xb0]
            checks['shape_history'] &= w[0x70:0xb0]==expected==a[0x70:0xb0]==z[0x70:0xb0]
            checks['shape_movement'] &= a[0x40:0x50]==z[0x40:0x50]
            if c['advance']:
                old=struct.unpack_from('<4f',p,0x1c0);new=struct.unpack_from('<4f',advanced['body'],0x1c0)
                checks['shape_movement'] &= a[0x40:0x50]==struct.pack('<4f',*(x-y for x,y in zip(old,new)))
    return checks


def run_set(out,profile,cases,prefix):
    source=out/f'{prefix}-input.bin';source.write_bytes(pack(cases)); outputs={};runs={}
    for label,cmd in [('original',[str(ROOT/'local/behavior/vehicle-bridge-probe.exe'),str(ROOT/'local/compatibility/grid-unpacked.exe')]),
                      ('repeat',[str(ROOT/'local/behavior/vehicle-bridge-probe.exe'),str(ROOT/'local/compatibility/grid-unpacked.exe')]),
                      ('rust',[str(ROOT/f'target/{profile}/grid-vehicle-bridge.exe')])]:
        outputs[label],runs[label]=execute(cmd,source,out/f'{prefix}-{label}.bin')
        runs[label].update(compare(outputs[label],outputs['original']))
    decoded=decode(outputs['original'],cases)
    report=dict(calls=len(cases),words=sum(3*(0x424+0x2600+len(c['shapes'])*0x220)//4 for c in cases),
        runs=runs,checks=checks(cases,decoded),input_sha256=digest(source),hashes={k:__import__('hashlib').sha256(v).hexdigest() for k,v in outputs.items()})
    report['passed']=all(r['passed'] for r in runs.values()) and all(report['checks'].values())
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2))
    return report,{label:decode(data,cases) for label,data in outputs.items()}


def guards(out,profile,first):
    valid=pack([first]);tests={}
    for name,offset,value in [('magic',0,0),('count_zero',4,0),('count_high',4,129),('shape_count',8,65),('notify',12,2),('advance',16,2),('correction',20,2),('head',24,2),
        ('body_pointer',32+0x328,1),('local_nan',32+0x90,0x7fc00000),('offset_nan',32+0x230,0x7fc00000),('velocity_nan',32+0x424+0x15c0,0x7fc00000),
        ('pose_nan',32+0x424+0x15e0,0x7fc00000),('shape_pointer',32+0x424+0x2600+32+0xe0,1),('shape_selector',32+0x424+0x2600+32+0x170,3),('shape_flag',32+0x424+0x2600+32+0x214,2),('shape_local_nan',32+0x424+0x2600+32+0xf0,0x7fc00000)]:
        data=bytearray(valid);struct.pack_into('<I',data,offset,value);tests[name]=data
    for name,value in [('zero_delta',0),('nan_delta',0x7fc00000),('large_delta',0x3f800000)]:
        data=bytearray(valid);struct.pack_into('<I',data,16,1);struct.pack_into('<I',data,28,value);tests[name]=data
    data=bytearray(valid);struct.pack_into('<I',data,12,1);struct.pack_into('<I',data,32+0x400,0xffffffff);tests['group_reset']=data
    data=bytearray(valid);struct.pack_into('<I',data,20,1);struct.pack_into('<f',data,32+0x424+0x2600+16,65.);tests['angular_bound']=data
    data=bytearray(valid);struct.pack_into('<I',data,16,1);struct.pack_into('<f',data,28,.25);struct.pack_into('<f',data,32+0x424+0x15d0,1000.);tests['spin_bound']=data
    tests.update(truncated=valid[:-1],trailing=valid+b'x')
    results={}
    for name,data in tests.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data)
        for label,cmd in [('original',[str(ROOT/'local/behavior/vehicle-bridge-probe.exe'),str(ROOT/'local/compatibility/grid-unpacked.exe')]),('rust',[str(ROOT/f'target/{profile}/grid-vehicle-bridge.exe')])]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(cmd+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            results[f'{name}-{label}']=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=0<run.returncode<256 and not target.exists())
    return results


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',required=True,choices=['debug','release']);args=parser.parse_args()
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/vehicle-bridge-probe.exe';sp=ROOT/'local/behavior/suspension-prepare-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE or digest(sp)!=SUSPENSION_PROBE:raise ValueError('Reference/probe identity')
    report=dict(profile=args.profile,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(ROOT/f'target/{args.profile}/grid-vehicle-bridge.exe'),
        live_frame=False,scope='Whole linked setters, selected no-reset notification, B13E10 advance with shape displacement, optional B13B00 correction/head normalization, linked getters. Authored schedule/body/shape ownership; fresh F168 suspension outputs.')
    try:
        upstream=ROOT/('local/behavior/suspension-vehicle-20261010-'+('debug-final-04' if args.profile=='debug' else 'release-final-05'))
        request=upstream/'initial-input.bin'; baseline=(upstream/'initial-original.bin').read_bytes()
        if digest(request)!='29b64fd2ac1cd2efbb7846f77249588d7b8a0884309a103ac48c35cde2ee9362':raise ValueError('Suspension input identity')
        report['upstream']={}
        for label,cmd in [('original',[str(sp),str(image),str(WORLD),str(request),str(out/'suspension-original.bin'),str(MECHANICS)]),('rust',[str(ROOT/f'target/{args.profile}/grid-suspension-contact.exe'),str(TRACK),str(SURFACE),str(request),str(out/'suspension-rust.bin')])]:
            run=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True,timeout=60)
            result=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr);result.update(compare((out/f'suspension-{label}.bin').read_bytes(),baseline));report['upstream'][label]=result
            if run.returncode or not result['passed']:raise ValueError('Fresh upstream suspension mismatch')
        cases=controls();rng=random.Random(200816901)
        for i in range(32):
            c=base(rng,i+96);c['vehicle']=bytearray(baseline[16+i*0x2614:16+i*0x2614+0x2600]);c['advance']=1;c['correction']=i%2;c['delta']=.006130263675
            floats(c['body'],0x230,[.2,-.3,.4,0.]);c['linear']=[.01,-.02,.03,0.];c['angular']=[.001,0.,-.002,0.];cases.append(c)
        report['handoff']=dict(source_input_sha256=digest(request),source_output_sha256=__import__('hashlib').sha256(baseline).hexdigest(),cases=list(range(32)),replacement_of_vehicle_pose_or_velocity=False)
        report['coverage']=dict(authored_cases=96,suspension_handoffs=32,advanced_cases=sum(c['advance'] for c in cases),corrected_cases=sum(c['correction'] for c in cases),normalized_cases=sum(c['correction'] and c['head'] for c in cases),shape_nodes=sum(len(c['shapes']) for c in cases),shape_history_sources={str(k):sum(struct.unpack_from('<I',s,0x170)[0]==k for c in cases for s in c['shapes']) for k in range(3)})
        report['initial'],outputs=run_set(out,args.profile,cases,'initial')
        if not report['initial']['passed']:raise ValueError('Initial bridge mismatch')
        report['guards']=guards(out,args.profile,cases[0])
        current={label:[copy.deepcopy(c) for c in cases[32:48]] for label in outputs}
        report['carried']=[]
        for step in range(12):
            branch_results={};branch_outputs={}
            for label in ['original','repeat','rust']:
                prefix=f'carried-{step:02}-{label}';source=out/f'{prefix}-input.bin';source.write_bytes(pack(current[label]))
                cmd=[str(ROOT/f'target/{args.profile}/grid-vehicle-bridge.exe')] if label=='rust' else [str(probe),str(image)]
                data,run=execute(cmd,source,out/f'{prefix}.bin');branch_results[label]=(data,run);branch_outputs[label]=decode(data,current[label])
            comparison={label:compare(data,branch_results['original'][0]) for label,(data,_) in branch_results.items()}
            semantic=checks(current['original'],branch_outputs['original'])
            report['carried'].append(dict(step=step,comparisons=comparison,checks=semantic,calls=16,words=sum(3*(0x424+0x2600+len(c['shapes'])*0x220)//4 for c in current['original'])))
            if not all(r['passed'] for r in comparison.values()) or not all(semantic.values()):raise ValueError('Carried bridge mismatch')
            for label in current:
                for c,stages in zip(current[label],branch_outputs[label]):
                    final=stages[-1];c['body']=bytearray(final['body']);c['vehicle']=bytearray(final['vehicle']);c['shapes']=[bytearray(s) for s in final['shapes']]
        report['passed']=all(g['passed'] for g in report['guards'].values())
        report['total_calls']=128+sum(r['calls'] for r in report['carried']);report['total_words']=report['initial']['words']+sum(r['words'] for r in report['carried'])
    finally:
        (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report.get(k) for k in ['profile','passed','total_calls','total_words']}))
    if not report['passed']:raise SystemExit(1)


if __name__=='__main__':main()
