"""Whole A06A90 observable car state, excluding its expired temporary view/solver."""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_driver as driver
import validate_vehicle_gearing as gearing
import vehicle_source_gearing as sources
b=driver.base
NATIVE=b.ROOT/'local/behavior/whole-vehicle-step-probe.exe'


def baseline(out,records,prefix,full):
    rows=[]
    for row in records:
        r=bytearray(row);view=bytearray(0x300);struct.pack_into('<I',view,0,struct.unpack_from('<I',r,16)[0])
        f=lambda o:struct.unpack_from('<f',r,b.BODY+o)[0]
        r[b.BODY+0x1640]=int(bool(r[b.BODY+0xe38]) or (f(0x1648)<.5 and
            ((f(0xe1c)==0 and f(0xe20)==0) or struct.unpack_from('<I',r,b.BODY+0x196c)[0]==0)))
        for o in range(0x298,0x2a8,4):struct.pack_into('<f',view,o,1)
        r[b.VIEW:b.SOLVER]=view;r[b.SOLVER:b.MATERIAL]=bytes(0xdd0);rows.append(bytes(r))
    source=out/f'{prefix}-driver-baseline-input.bin';target=out/f'{prefix}-driver-baseline-output.bin';source.write_bytes(b.request(rows,b'EF01'))
    run=b.execute(b.ROOT/'local/behavior/whole-vehicle-driver-probe.exe',[b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full],out,prefix+'-driver-baseline')
    assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout'];return target.read_bytes(),run


def compare(out,profile,branches,prefix,full):
    outputs={};runs=[]
    for kind,records in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin';source.write_bytes(b.request(records,b'QF01'))
        exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
        run=b.execute(exe,args,out,f'{prefix}-{kind}');runs.append(run)
        if kind!='rust':assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout']
        outputs[kind]=target.read_bytes();assert len(outputs[kind])==8+len(records)*b.RECORD
    ranges=[(0,b.VIEW),(b.MATERIAL,b.RECORD)];differences=[];coverage=collections.Counter()
    positive=all(struct.unpack_from('<f',r,4)[0]>0 for r in branches['original'])
    if positive:prior,run=baseline(out,branches['original'],prefix,full);runs.append(run)
    for i,row in enumerate(branches['original']):
        a,repeat,rust=[outputs[k][8+i*b.RECORD:8+(i+1)*b.RECORD] for k in ['original','repeat','rust']]
        for start,end in ranges:
            assert a[start:end]==repeat[start:end]
            differences.extend(dict(case=i,offset=hex(o),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(start,end,4) if a[o:o+4]!=rust[o:o+4])
        for label,start,end in [('pose',0x15e0,0x1620),('velocity',0x15c0,0x15e0),('chassis',0x10,0x68),('timer',0x2e8,0x2ec),('clutch_flag',0x1640,0x1644),('fuel',0x1fa0,0x1fa4),('gearing',0x196c,0x1970),('mode_transition',0x1598,0x159c),('engine_speed',0x1894,0x1898)]:coverage[label]+=row[b.BODY+start:b.BODY+end]!=a[b.BODY+start:b.BODY+end]
        coverage['mode_two']+=struct.unpack_from('<I',a,b.BODY+0x1598)[0]==2
        for j in range(4):
            w=b.BODY+0x1990+j*0x150
            coverage['contacts']+=struct.unpack_from('<I',a,20+j*4)[0]!=0;coverage['spin']+=row[w+0x104:w+0x108]!=a[w+0x104:w+0x108]
        coverage['zero_delta']+=struct.unpack_from('<f',row,4)[0]==0
        if positive:
            previous=prior[8+i*b.RECORD:8+(i+1)*b.RECORD];allowed=set(range(0x2e8,0x2ec,4))|set(range(0x15c0,0x15e0,4))|set(range(0x1610,0x1620,4))|{0x1990+j*0x150+0x104 for j in range(4)}
            assert all(a[b.BODY+o:b.BODY+o+4]==previous[b.BODY+o:b.BODY+o+4] for o in range(0,0x2600,4) if o not in allowed)
            for label,start,end in [('tail_angular',0x15d0,0x15e0),('tail_braking',0x15c0,0x15d0),('tail_timer',0x2e8,0x2ec),('tail_pose',0x1610,0x1620)]:coverage[label]+=a[b.BODY+start:b.BODY+end]!=previous[b.BODY+start:b.BODY+end]
    report=dict(calls=len(branches['original']),observable_bytes=sum(e-s for s,e in ranges)*len(branches['original']),body_bytes=0x2600*len(branches['original']),repeat_exact=True,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert not differences,dict(count=len(differences),first=differences[:30]);return report,outputs


def cases(tables,idle=False):
    records=[]
    for i,row in enumerate(driver.cases()):
        r=bytearray(row);t=tables[i%len(tables)];gearing.apply_source_shift(r,t)
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9]);struct.pack_into('<I',r,b.BODY+0x196c,1+i%count);struct.pack_into('<I',r,b.BODY+0xe48,10);r[b.BODY+0x1972]=0;r[b.BODY+0xe38]=0
        b.put(r,b.BODY+0xe1c,.4+i%4*.1)
        for o,v in [(0x2e8,i%7*.5),(0x11f0,(i%4)*.3),(0x1224,.01),(0x1228,.6),(0x122c,(i%5)*.2),(0x1230,(i%3)*.3),(0x1234,0),(0x1238,20)]:b.put(r,b.BODY+o,v)
        if idle:
            r[b.BODY+0xe38]=int(i%4==2)
            if i%4<2:
                for o in [0xe1c,0xe20,0x1648]:b.put(r,b.BODY+o,0)
                b.put(r,b.BODY+0x15c0,(0,0,0,0))
        records.append(bytes(r))
    return records


def runtime_modes(branches,idle):
    if not idle:return branches
    # Mode is an authored runtime input, applied after mode-one source setup.
    result={}
    for kind,rows in branches.items():
        result[kind]=[]
        for i,row in enumerate(rows):
            r=bytearray(row);struct.pack_into('<I',r,b.BODY+0x1598,1+i%2);result[kind].append(bytes(r))
    return result


def guards(out,profile,first,full):
    tests=[]
    for name,o,w in [('negative_delta',4,0xbf800000),('nan_delta',4,0x7fc00000),('large_delta',4,0x3e800000),('tiny_delta',4,1),('unsupported_mode',b.BODY+0x1598,0),('angle_span',b.BODY+0x1228,struct.unpack_from('<I',first,b.BODY+0x1224)[0])]:
        row=bytearray(first);struct.pack_into('<I',row,o,w)
        if name=='angle_span':b.put(row,b.BODY+0x122c,1)
        tests.append((name,bytes(row)))
    for o in [0x2e8,0x11f0,0x1224,0x1228,0x122c,0x1230,0x1234,0x1238]:
        row=bytearray(first);struct.pack_into('<I',row,b.BODY+o,0x7fc00000);tests.append((f'nan_{o:x}',bytes(row)))
    row=bytearray(first);row[b.BODY+0x1971]=0
    tests.append(('manual_clutch',bytes(row)))
    row=bytearray(first);struct.pack_into('<I',row,b.BODY+0x17a0,0)
    tests.append(('zero_engine_inertia',bytes(row)))
    row=bytearray(first);struct.pack_into('<I',row,b.BODY+0x1598,2)
    row[b.BODY+0xe38]=0;row[b.BODY+0x1640]=0;b.put(row,b.BODY+0xe1c,.5)
    gear=struct.unpack_from('<I',row,b.BODY+0x196c)[0];b.put(row,b.BODY+0x18f4+4*gear,1.e-30)
    tests.append(('engagement_overflow',bytes(row)))
    row=bytearray(first);b.put(row,4,0)
    b.put(row,b.BODY+0x15c0,(1,0,0,0))
    for o in [0x11f0,0xe20]:struct.pack_into('<I',row,b.BODY+o,0x7f7fffff)
    tests.append(('braking_overflow',bytes(row)));checks=[]
    for name,row in tests:
        for kind in ['native','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(b.request([row],b'QF01'))
            exe=NATIVE if kind=='native' else b.ROOT/f'target/{profile}/grid-vehicle-force.exe';args=[b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full] if kind=='native' else [b.TRACK,b.SURFACE,source,target]
            run=b.execute(exe,args,out,f'guard-{name}-{kind}',True)
            passed=(0<run['exit']<=80 and not run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] if kind=='native' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(x['passed'] for x in checks),[x for x in checks if not x['passed']];return checks


def zero_cases(records,full):
    data=full.read_bytes();result=[]
    for i in range(96):
        r=bytearray(records[i%len(records)]);struct.pack_into('<f',r,4,0);o=b.BODY
        for offset,value in [(0xe1c,0),(0xe20,0),(0x2e8,[0,1,3][i%3]),(0x1648,[0,.01,.1,5,5.01,10][i%6])]:b.put(r,o+offset,value)
        r[o+0xe38]=int(i%2==0)
        b.put(r,o+0x15c0,([0,.001,.05,1,6,12][i%6],.00001,-.00002,0))
        b.put(r,o+0x15d0,(.00003,-.00004,.00005,0))
        for j in range(4):
            w=o+0x1990+j*0x150;token=int(j<=i%4)
            for offset,value in [(0xc0,token),(0xc4,token),(0xc8,token)]:struct.pack_into('<I',r,w+offset,value)
            struct.pack_into('<I',r,20+j*4,token);m=b.MATERIAL+j*0x140;r[m:m+0x140]=bytes(0x140)
            if token:
                for offset in [0x10,0x14,0x18,0x1c,0x20,0x64]:r[m+offset:m+offset+4]=data[8+offset:12+offset]
                r[m+0x75]=data[8+0x75]
        result.append(bytes(r))
    return result


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--idle',action='store_true');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=cases(tables,a.idle);full=b.grip.mechanics_fixture(out);reports=[];init=[];coverage=collections.Counter()
    zeros=zero_cases(records,full)
    for start in range(0,len(zeros),40):
        batch=zeros[start:start+40];r,_=compare(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'zero-{start}',full);reports.append(r);coverage.update(r['coverage'])
    for start in range(0,len(records),40):
        batch=records[start:start+40];r,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');init.append(r)
        branches=runtime_modes(branches,a.idle)
        r,_=compare(out,a.profile,branches,f'initial-{start}',full);reports.append(r);coverage.update(r['coverage'])
    r,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');init.append(r)
    branches=runtime_modes(branches,a.idle)
    for step in range(12):
        r,outputs=compare(out,a.profile,branches,f'carry-{step}',full);reports.append(r);coverage.update(r['coverage']);following={}
        for kind,data in outputs.items():
            rows=[]
            for i,previous in enumerate(branches[kind]):
                row=bytearray(data[8+i*b.RECORD:8+(i+1)*b.RECORD]);o=b.BODY
                row[o+0xb40:o+0xb80]=previous[o+0x15e0:o+0x1620];row[o+0xb80:o+0xba0]=previous[o+0xb20:o+0xb40];row[o+0xb20:o+0xb40]=row[o+0x15c0:o+0x15e0]
                b.put(row,o+0xe1c,.5)
                if a.idle:
                    # Authored controls only; every branch carries its own dynamics.
                    row[o+0xe38]=int(step%4<2)
                    b.put(row,o+0xe1c,0 if step%4<2 else .5)
                    b.put(row,o+0xe20,0)
                rows.append(bytes(row))
            following[kind]=rows
        branches=following
    checks=guards(out,a.profile,records[0],full)
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),body_bytes=sum(r['body_bytes'] for r in reports),observable_bytes=sum(r['observable_bytes'] for r in reports),source_files=len(tables),coverage=dict(coverage),guards=len(checks),reports=reports,initialization_reports=init,scope='Whole A06A90 including whole driver, compared full2600 car and globals/materials; expired uninitialized native temporary view/solver excluded. Assisted automatic non-neutral modes one/two and clutch transitions. Authored construction/interframe history/input; no A03220/world/driving claim.',idle=a.idle)
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))


if __name__=='__main__':main()
