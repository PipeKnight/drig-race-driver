"""Whole unchanged A03670 versus the owned Rust driver and source queries."""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_driver as driver
import validate_vehicle_gearing as gearing
import vehicle_source_gearing as sources

b=driver.base
NATIVE=b.ROOT/'local/behavior/whole-vehicle-driver-probe.exe'


def native(out,records,name,full,allow_failure=False):
    source=out/f'{name}-input.bin';target=out/f'{name}-output.bin';source.write_bytes(b.request(records,b'EF01'))
    run=b.execute(NATIVE,[b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full],out,name,allow_failure)
    if run['exit']:return None,[run]
    assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout']
    return target.read_bytes(),[run]


def compare(out,profile,branches,prefix,full):
    outputs={};runs=[]
    for kind,records in branches.items():
        name=prefix+'-'+kind
        if kind=='rust':
            source=out/f'{name}-input.bin';target=out/f'{name}-output.bin';source.write_bytes(b.request(records,b'EF01'))
            runs.append(b.execute(b.ROOT/f'target/{profile}/grid-vehicle-force.exe',[b.TRACK,b.SURFACE,source,target],out,name));outputs[kind]=target.read_bytes()
        else:outputs[kind],items=native(out,records,name,full);runs.extend(items)
    a,repeat,rust=[outputs[k] for k in ['original','repeat','rust']];assert len(a)==len(repeat)==len(rust)==8+len(branches['original'])*b.RECORD
    differences=[dict(case=(o-8)//b.RECORD,offset=hex((o-8)%b.RECORD),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(8,len(a),4) if a[o:o+4]!=rust[o:o+4]]
    coverage=collections.Counter()
    for i,row in enumerate(branches['original']):
        after=a[8+i*b.RECORD:8+(i+1)*b.RECORD]
        for label,start,end in [('pose',0x15e0,0x1620),('velocity',0x15c0,0x15e0),('chassis',0x10,0x68),('fuel',0x1fa0,0x1fa4),('history',0xb20,0xb40),('gearing',0x196c,0x1970)]:coverage[label]+=row[b.BODY+start:b.BODY+end]!=after[b.BODY+start:b.BODY+end]
        for j in range(4):
            w=b.BODY+0x1990+j*0x150
            coverage['contacts']+=struct.unpack_from('<I',after,20+j*4)[0]!=0
            coverage['spin']+=row[w+0x104:w+0x108]!=after[w+0x104:w+0x108]
            coverage['grip']+=struct.unpack_from('<f',after,b.VIEW+0x5c+j*4)[0]!=0
    report=dict(calls=len(branches['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert report['repeat_exact'] and not differences,dict(count=len(differences),first=differences[:24])
    return report,outputs


def configure():
    b.MAGIC=b'EF01';b.DRIVER_PREPARE=True;b.POST_FORCE=True;b.POST_CHASSIS=True
    b.native=native;b.run_set=compare


def framing_guards(out,first,full):
    valid=b.request([first],b'EF01');tests=[('header',b'BAD!'+valid[4:]),('empty',b'EF01'+bytes(4)),('count',b'EF01'+struct.pack('<I',65)),('truncated',valid[:-1]),('trailing',valid+b'x')]
    for name,o,v in [('identity',0,1),('view_owner',b.BODY,0),('view_alias',b.VIEW,0),('overlap',b.VIEW+0x2b0,0xffffffff),('wheel_owner',b.BODY+0x1990+0x34,0),('foreign_drive',b.BODY+0x18d8,1),('foreign_steering',b.BODY+0x21cc,1),('limit_nan',b.RECORD-36,0x7fc00000),('diagnostic_nan',b.RECORD-32,0x7fc00000)]:
        r=bytearray(first);struct.pack_into('<I',r,o,v);tests.append((name,b'EF01'+struct.pack('<I',1)+r))
    checks=[]
    for name,payload in tests:
        source=out/f'native-framing-{name}-input.bin';target=out/f'native-framing-{name}-output.bin';source.write_bytes(payload)
        run=b.execute(NATIVE,[b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full],out,'native-framing-'+name,True)
        checks.append(dict(name=name,passed=0<run['exit']<=80 and not run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] and not target.exists(),execution=run))
    source=out/'native-existing-input.bin';target=out/'native-existing-output.bin';source.write_bytes(valid);target.write_bytes(b'owned sentinel')
    run=b.execute(NATIVE,[b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full],out,'native-existing',True)
    checks.append(dict(name='existing_output',passed=run['exit']==73 and target.read_bytes()==b'owned sentinel',execution=run))
    (out/'native-framing-guards.json').write_text(json.dumps(checks,indent=2));assert all(x['passed'] for x in checks),[x for x in checks if not x['passed']]
    return checks


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False);configure()
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=[]
    for i,row in enumerate(driver.cases()):
        r=bytearray(row);t=tables[i%len(tables)];gearing.apply_source_shift(r,t)
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9]);struct.pack_into('<I',r,b.BODY+0x196c,1+i%count)
        struct.pack_into('<I',r,b.BODY+0xe48,10);r[b.BODY+0x1972]=0;records.append(bytes(r))
    full=b.grip.mechanics_fixture(out);reports=[];initial=[];coverage=collections.Counter()
    for start in range(0,len(records),40):
        batch=records[start:start+40];r,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(r)
        r,_=compare(out,a.profile,branches,f'initial-{start}',full);reports.append(r);coverage.update(r['coverage'])
    r,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(r)
    for step in range(12):
        r,outputs=compare(out,a.profile,branches,f'carry-{step}',full);reports.append(r);coverage.update(r['coverage']);following={}
        for kind,data in outputs.items():
            rows=[]
            for i,previous in enumerate(branches[kind]):
                row=bytearray(data[8+i*b.RECORD:8+(i+1)*b.RECORD]);o=b.BODY
                row[o+0xb40:o+0xb80]=previous[o+0x15e0:o+0x1620];row[o+0xb80:o+0xba0]=previous[o+0xb20:o+0xb40];row[o+0xb20:o+0xb40]=row[o+0x15c0:o+0x15e0];rows.append(bytes(row))
            following[kind]=rows
        branches=following
    checks=b.guards(out,a.profile,records[0],full);native_checks=framing_guards(out,records[0],full)
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),source_files=len(tables),coverage=dict(coverage),guards=len(checks),reports=reports,initialization_reports=initial,scope='Whole original A03670 one call against Rust driver with real source-world contacts. Selected assisted-automatic mode1, authored earlier construction/interframe history. No A06A90tail/A03220/world/Bevy-driving claim.')
    result['native_framing_guards']=len(native_checks)
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))


if __name__=='__main__':main()
