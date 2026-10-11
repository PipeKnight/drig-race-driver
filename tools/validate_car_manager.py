"""Whole single-car A02B70 with real original dispatch/completion/wait code."""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_query_car_job as job
b=job.b
RECORD=job.RECORD
NATIVE=b.ROOT/'local/behavior/car-manager-probe.exe'
at=job.car_offset

def configure(row,index):
    r=bytearray(row)
    for o in [0xcd8,0xda8,0xc0c,0xd40]:struct.pack_into('<I',r,at(o),0)
    r[at(0x248):at(0x24c)]=bytes([index%2,index%2,0,index%2])
    zero=struct.unpack_from('<f',r,4)[0]==0
    struct.pack_into('<I',r,at(0xdc8),int(zero and index%2));r[at(0x1ee0)]=index%2
    # Zero-driver cases retain an installed producer with no other-body wrapper.
    # Positive contact cases explicitly omit DC8 until production is supported.
    if zero:
        for i in range(4):struct.pack_into('<I',r,at(0x1990+i*0x150+0xc4),0)
    return bytes(r)

def compare(out,profile,branches,prefix,full,producer=False):
    answers={};runs=[]
    record_size=RECORD+(520 if producer else 0)
    for kind,rows in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin'
        source.write_bytes(b.request(rows,b'TF02' if producer else b'TF01'))
        exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else b.ROOT/'local/behavior/wheel-car-manager-probe.exe' if producer else NATIVE
        args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
        run=b.execute(exe,args,out,f'{prefix}-{kind}');runs.append(run)
        if kind!='rust':
            assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout']
            assert f'car_manager_intervals={len(rows)} original_dispatch_wait=1 owned_events_consumed=1' in run['stdout']
        raw=target.read_bytes();assert len(raw)==8+record_size*len(rows)
        answers[kind]=[raw[8+i*record_size:8+(i+1)*record_size] for i in range(len(rows))]
    ranges=[(0,b.VIEW),(b.MATERIAL,record_size)];differences=[];coverage=collections.Counter()
    for i,old in enumerate(branches['original']):
        original,repeat,rust=[answers[k][i] for k in ['original','repeat','rust']]
        for start,end in ranges:
            assert original[start:end]==repeat[start:end]
            differences.extend(dict(case=i,offset=hex(o),original=original[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(start,end,4) if original[o:o+4]!=rust[o:o+4])
        assert original[at(0xb40):at(0xb80)]==original[at(0x15e0):at(0x1620)]
        assert original[at(0xb80):at(0xba0)]==original[at(0x15c0):at(0x15e0)]
        assert original[at(0x24b)]==0
        assert original[at(0x21c4):at(0x21c8)]==struct.pack('<f',1)
        assert original[job.OWNER_RECORD:RECORD]==old[job.OWNER_RECORD:RECORD]
        if producer:
            cap,before=struct.unpack_from('<II',old,RECORD);after=struct.unpack_from('<I',original,RECORD+4)[0]
            assert before<=after<=cap
            assert original[RECORD+8:RECORD+8+before*128]==old[RECORD+8:RECORD+8+before*128]
            assert original[RECORD+8+after*128:]==old[RECORD+8+after*128:]
            for j in range(before,after):
                c=original[RECORD+8+j*128:RECORD+8+(j+1)*128]
                assert struct.unpack_from('<II',c,4)==(1,0) and c[0xc]==0
                assert struct.unpack_from('<I',c,0x28)[0]==1 and struct.unpack_from('<I',c,0x2c)[0]>=2
                assert c[0x70:0x74]==bytes([0,0,1,1]) and struct.unpack_from('<f',c,0x3c)[0]>1e38
                assert struct.unpack_from('<f',c,0x38)[0]>0
            coverage['produced_contacts']+=after-before;coverage['production_intervals']+=after>before
            coverage['capacity_skips']+=cap==before;coverage['retained_records']+=before
        po=b.RECORD+job.EXTRA
        assert original[po+0x190:po+0x210]==old[po+0x190:po+0x210]
        for name,o,n in [('velocity',0x15c0,32),('basis',0x15e0,48),('pose_history',0xb40,64),('velocity_history',0xb80,32),('factor',0x21c4,4),('pending_cleared',0x24b,1),('mode',0x1598,4),('engine',0x1894,4),('handling',0xcdf0+0x3a0,0x88)]:coverage[name]+=old[at(o):at(o)+n]!=original[at(o):at(o)+n]
        coverage['zero_delta']+=struct.unpack_from('<f',old,4)[0]==0
        coverage['producer_installed']+=bool(old[at(0x1ee0)] and struct.unpack_from('<I',old,at(0xdc8))[0])
        coverage['queries_present']+=struct.unpack_from('<I',old,job.OWNER_RECORD)[0]!=0
        coverage['notifications_on']+=bool(old[at(0xcdf0+0x442)])
        coverage['contacts']+=sum(struct.unpack_from('<I',original,20+j*4)[0]!=0 for j in range(4))
    report=dict(calls=len(branches['original']),owner_bytes=len(branches['original'])*(0x21000+0x424),observable_bytes=len(branches['original'])*sum(z-a for a,z in ranges),repeat_exact=True,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert not differences,dict(count=len(differences),first=differences[:25]);return report,answers

def guards(out,profile,first,full):
    tests=[]
    for name,o,value in [('deferred_cd8',0xcd8,1),('deferred_da8',0xda8,1),('deferred_c0c',0xc0c,1),('deferred_d40',0xd40,1),('activation_change',0x248,0x01000001),('bad_flags',0x248,2),('foreign_constraint_wrapper',0xdc8,2),('producer_mode',0x1ee0,2),('gear_oob',0x196c,10),('speed_nan',0x1648,0x7fc00000),('query_count',None,65),('zero_forward',0x1600,0)]:
        r=bytearray(first)
        if name=='zero_forward':
            b.put(r,4,0)
            # Synchronization replaces the local pose from physical authority.
            for j in range(4):struct.pack_into('<I',r,b.RECORD+job.EXTRA+0x1f0+j*4,0)
        elif o is None:struct.pack_into('<I',r,job.OWNER_RECORD+4,value)
        else:struct.pack_into('<I',r,at(o),value)
        tests.append((name,bytes(r)))
    r=bytearray(job.healthy(first));struct.pack_into('<I',r,at(0xdc8),1);r[at(0x1ee0)]=1
    b.put(r,at(0x19d0),2);b.put(r,at(0x19e0),1);struct.pack_into('<I',r,at(0x1a54),1)
    tests.append(('active_wheel_constraint',bytes(r)))
    valid=b.request([first],b'TF01')
    frames=[('bad_tag',b'NOPE'+valid[4:]),('empty',b'TF01'+bytes(4)),('count65',b'TF01'+struct.pack('<I',65)),('truncated',valid[:-1]),('trailing',valid+b'!')]
    checks=[]
    for name,data in [(n,b.request([r],b'TF01')) for n,r in tests]+frames:
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(data)
            exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
            args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
            run=b.execute(exe,args,out,f'guard-{name}-{kind}',True)
            passed=(0<run['exit']<=80 and not run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']];return checks

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    tables=job.sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=job.step.cases(tables,True);full=b.grip.mechanics_fixture(out)
    reports=[];initial=[];coverage=collections.Counter();first=None
    def tick(branches,prefix):
        nonlocal first
        if first is None:first=branches['original'][0]
        report,answers=compare(out,a.profile,branches,prefix,full);reports.append(report);coverage.update(report['coverage']);return answers
    for start in range(0,len(records),32):
        batch=records[start:start+32];report,branches=job.gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(report);branches=job.step.runtime_modes(branches,True)
        tick({k:[configure(job.extend(r,start+i),start+i) for i,r in enumerate(rows)] for k,rows in branches.items()},f'initial-{start}')
    zeros=job.step.zero_cases(records,full)
    for start in range(0,len(zeros),32):tick({k:[configure(job.extend(r,start+i),start+i) for i,r in enumerate(zeros[start:start+32])] for k in ['original','repeat','rust']},f'zero-{start}')
    report,branches=job.gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(report);branches=job.step.runtime_modes(branches,True)
    branches={k:[configure(job.extend(r,8+i),8+i) for i,r in enumerate(rows)] for k,rows in branches.items()}
    for frame in range(12):
        after=tick(branches,f'carry-{frame}');following={}
        for kind,rows in after.items():
            following[kind]=[]
            for row in rows:
                r=bytearray(row)
                # Keep manager-authored history; only external controls change.
                inactive=int(frame%4<2);r[at(0xe38)]=inactive;b.put(r,at(0xe1c),0 if inactive else .5);b.put(r,at(0xe20),0)
                following[kind].append(bytes(r))
        branches=following
    checks=guards(out,a.profile,first,full)
    assert all(coverage[k]>0 for k in ['basis','pose_history','velocity_history','factor','pending_cleared','zero_delta','queries_present','notifications_on','contacts'])
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),owner_bytes=sum(r['owner_bytes'] for r in reports),observable_bytes=sum(r['observable_bytes'] for r in reports),source_files=len(tables),guards=len(checks),coverage=dict(coverage),reports=reports,initialization_reports=initial,executables={str(p.relative_to(b.ROOT)):b.digest(p) for p in [NATIVE,b.ROOT/f'target/{a.profile}/grid-vehicle-force.exe']},scope='Whole originalA02B70, single car, unchanged activation/empty deferred edits, original9B2330 factor, originalA7D710 direct task dispatch/A0A290/SetEvent/ResetEvent/A7D530 real wait-all, installed original20 job/completion tables/owned auto-reset events. Conditional9BAB10 constraint production inactive, exact original basis repair/history copies. Healthy handling/query bounds asKF02; authored ownership/control/history; not worker pool/inter-car/constraint production/world scheduler/source construction/driving.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))
if __name__=='__main__':main()
