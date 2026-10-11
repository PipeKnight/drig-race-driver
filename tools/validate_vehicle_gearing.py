"""Whole shift-threshold initializer, then reusable independent-state composition."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
import validate_vehicle_controls as controls
import validate_vehicle_propulsion as base
import vehicle_source_gearing as sources

NATIVE=base.ROOT/'local/behavior/vehicle-gearing-probe.exe'
TABLES=[]


def cases():
    seeds=controls.cases();rng=random.Random(2008180);result=[]
    for ordinal,t in enumerate(TABLES):
        for mode in range(8):
            r=bytearray(seeds[(ordinal*8+mode)%len(seeds)]);b=36
            for i,w in enumerate(t['words']):struct.pack_into('<I',r,b+0x18f4+i*4,w)
            struct.pack_into('<f',r,b+0x17a4,[0,1,105,900,1675.51611328125,1800,20000,700][mode])
            struct.pack_into('<f',r,b+0x2218,[0,1,.5,1.2][mode%4])
            for i in range(32):
                value=0 if mode in [0,1] or mode==4 and i%4==0 else rng.uniform(-.25,2)
                struct.pack_into('<f',r,b+0x17ac+i*4,value)
            for i in range(4):struct.pack_into('<f',r,b+0x1990+i*0x150+0xe0,[.31,.29,.37,.31][(i+mode)%4])
            for i in range(10):
                for offset in [0x191c,0x1944]:struct.pack_into('<f',r,b+offset+i*4,rng.uniform(-50,50))
            result.append(bytes(r))
        # Source curve, engine limit and defaulted modifier, including inactive slots.
        r=bytearray(result[-1]);apply_source_shift(r,t);result.append(bytes(r))
    return result


def apply_source_shift(r,t):
    for i,w in enumerate(t['words']):struct.pack_into('<I',r,36+0x18f4+i*4,w)
    for offset,w in zip([0x17a4,*range(0x17ac,0x182c,4),0x2218],t['shift_words'],strict=True):
        struct.pack_into('<I',r,36+offset,w)


def initialize(out,profile,current,prefix):
    outputs={};executions=[]
    for kind,records in current.items():
        payload=b'SF01'+struct.pack('<I',len(records))+b''.join(struct.pack('<I',i)+r[4:] for i,r in enumerate(records))
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}.bin';source.write_bytes(payload)
        exe=base.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[source,target] if kind=='rust' else [base.REFERENCE,source,target]
        run=subprocess.run([str(exe),*map(str,args)],cwd=base.ROOT,capture_output=True,text=True,timeout=120)
        item=dict(kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,sha256=base.digest(exe));executions.append(item)
        (out/f'{prefix}-{kind}-execution.json').write_text(json.dumps(item,indent=2));assert run.returncode==0 and not run.stderr,item
        if kind!='rust':assert 'code_unchanged=1 error=0' in run.stdout
        outputs[kind]=target.read_bytes();assert len(outputs[kind])==len(payload)
    a,repeat,rust=[outputs[k] for k in ['original','repeat','rust']]
    diffs=[dict(case=(i-8)//base.RECORD_SIZE,offset=hex((i-8)%base.RECORD_SIZE),original=a[i:i+4].hex(),rust=rust[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=rust[i:i+4]]
    changed=0
    allowed={36+offset+i*4 for offset in [0x191c,0x1944] for i in range(1,10)}
    for i,record in enumerate(current['original']):
        after=a[8+i*base.RECORD_SIZE:8+(i+1)*base.RECORD_SIZE]
        for o in range(4,base.RECORD_SIZE,4):
            if o not in allowed:assert record[o:o+4]==after[o:o+4],(i,hex(o))
            else:changed+=record[o:o+4]!=after[o:o+4]
    report=dict(calls=len(current['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(diffs),differences=diffs,threshold_words_changed=changed,executions=executions,hashes={k:hashlib.sha256(v).hexdigest() for k,v in outputs.items()})
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert report['repeat_exact'] and not diffs,report
    return report,{k:[v[8+i*base.RECORD_SIZE:8+(i+1)*base.RECORD_SIZE] for i in range(len(current[k]))] for k,v in outputs.items()}


def guards(out,profile,first):
    checks=[]
    mutations=[('nan_rpm',0x17a4,0x7fc00000,70),('negative_rpm',0x17a4,0xbf800000,70),('unbounded_scan',0x17a4,0x7f7fffff,70),('nan_curve',0x17ac,0x7fc00000,70),('nan_ratio',0x1900,0x7fc00000,70),('nan_modifier',0x2218,0x7fc00000,70),('nan_retained_threshold',0x1940,0x7fc00000,70),('zero_radius',0x1990+0xe0,0,70),('negative_radius',0x1990+0xe0,0xbf800000,70),('late_overflow',0x2218,0x7f7fffff,72),('nonfinite_lookup_factor',0x18fc,1,71)]
    for name,o,value,expected in mutations:
        r=bytearray(first);struct.pack_into('<I',r,36+o,value)
        # Known driven set and positive second-gear down threshold activate overflow.
        for i in range(2):struct.pack_into('<I',r,36+0x18d8+i*4,struct.unpack_from('<I',r,8)[0]+0x1990+i*0x150)
        struct.pack_into('<I',r,36+0x18f0,2)
        if name=='late_overflow':
            struct.pack_into('<f',r,36+0x194c,10000)
            struct.pack_into('<f',r,36+0x18fc,0)
        if name=='nonfinite_lookup_factor':
            struct.pack_into('<f',r,36+0x17a4,900)
            struct.pack_into('<f',r,36+0x18f8,1)
        payload=b'SF01'+struct.pack('<I',1)+struct.pack('<I',0)+r[4:]
        for kind in ['native','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}.bin';source.write_bytes(payload)
            exe=NATIVE if kind=='native' else base.ROOT/f'target/{profile}/grid-vehicle-force.exe'
            args=[base.REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=base.ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(expected if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and 'completed=0 code_unchanged=1' in run.stdout and not run.stderr
            checks.append(dict(name=name,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=passed))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']]
    return checks


def main():
    global TABLES
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((base.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    TABLES=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=cases();reports=[]
    for start in range(0,len(records),48):
        batch=records[start:start+48];r,_=initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}');reports.append(r)
    carry={k:records[6:38] for k in ['original','repeat','rust']}
    for step in range(4):
        r,carry=initialize(out,a.profile,carry,f'carry-{step}');reports.append(r)
    checks=guards(out,a.profile,records[0]);result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),reports=reports,guards=len(checks),source_gearing_files=len(TABLES),scope='Whole9AD6B0 shift initialization with source sparse gear tables, supplied engine curves/radii/other state. No complete constructor/world/driving claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='reports'}))


if __name__=='__main__':main()
