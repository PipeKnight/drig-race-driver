"""Whole car manager with active wheel overtravel into an owned contact pool."""
import argparse,collections,json,random,struct
from pathlib import Path
import validate_car_manager as m
job=m.job;b=m.b;at=m.at;RECORD=m.RECORD+520
NATIVE=b.ROOT/'local/behavior/wheel-car-manager-probe.exe'

def configure(row,index):
    r=bytearray(m.configure(row,index));r[at(0x1ee0)]=int(index%7!=0);struct.pack_into('<I',r,at(0xdc8),int(index%11!=0));struct.pack_into('<I',r,at(0xdd8),0)
    zero=struct.unpack_from('<f',r,4)[0]==0
    for i in range(4):
        struct.pack_into('<I',r,at(0x1a54+i*0x150),2)
        if zero:
            limit=struct.unpack_from('<f',r,at(0x19e0+i*0x150))[0]
            b.put(r,at(0x19d0+i*0x150),limit+(.125 if (index+i)%3 else 0))
    capacity=[4,4,4,0,1,2,3,4][index%8];used=capacity if index%11==0 else int(capacity>0 and index%13==0)
    tail=bytearray(struct.pack('<II',capacity,used)+random.Random(2008195+index).randbytes(512))
    for i in range(4):tail[8+i*128+0xf]=1
    return bytes(r+tail)

def guards(out,profile,first,full):
    tests=[]
    for name,o,v in [('capacity',m.RECORD,5),('used',m.RECORD+4,5),('foreign_other',at(0x1a54),1),('foreign_world',at(0xdd8),1),('foreign_car_shape',at(0xdc8),2),('bad_query_count',job.OWNER_RECORD+4,65),('terminal',at(0x22e4),1),('negative_delta',4,0xbf800000)]:
        r=bytearray(first);struct.pack_into('<I',r,o,v);tests.append((name,b.request([r],b'TF02')))
    valid=b.request([first],b'TF02');tests.extend([('bad_tag',b'NOPE'+valid[4:]),('empty',b'TF02'+bytes(4)),('truncated',valid[:-1]),('trailing',valid+b'!')])
    checks=[]
    for name,data in tests:
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
        report,answers=m.compare(out,a.profile,branches,prefix,full,True);reports.append(report);coverage.update(report['coverage']);return answers
    for start in range(0,len(records),32):
        batch=records[start:start+32];report,branches=job.gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(report);branches=job.step.runtime_modes(branches,True)
        tick({k:[configure(job.extend(r,start+i),start+i) for i,r in enumerate(rows)] for k,rows in branches.items()},f'initial-{start}')
    zeros=job.step.zero_cases(records,full)
    for start in range(0,len(zeros),32):tick({k:[configure(job.extend(r,start+i),start+i) for i,r in enumerate(zeros[start:start+32])] for k in ['original','repeat','rust']},f'zero-{start}')
    report,branches=job.gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(report);branches=job.step.runtime_modes(branches,True)
    branches={k:[configure(job.extend(r,8+i),8+i) for i,r in enumerate(rows)] for k,rows in branches.items()}
    for frame in range(12):
        after=tick(branches,f'carry-{frame}');branches={}
        for kind,rows in after.items():
            branches[kind]=[]
            for row in rows:
                r=bytearray(row);inactive=int(frame%4<2);r[at(0xe38)]=inactive;b.put(r,at(0xe1c),0 if inactive else .5);b.put(r,at(0xe20),0)
                if frame%3==0:struct.pack_into('<I',r,m.RECORD+4,0) # External contact consumption/reset, templates retained.
                branches[kind].append(bytes(r))
    checks=guards(out,a.profile,first,full)
    assert all(coverage[k]>0 for k in ['produced_contacts','production_intervals','capacity_skips','retained_records','basis','contacts','zero_delta'])
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),owner_bytes=sum(r['owner_bytes'] for r in reports),observable_bytes=sum(r['observable_bytes'] for r in reports),source_files=len(tables),guards=len(checks),coverage=dict(coverage),reports=reports,initialization_reports=initial,executables={str(p.relative_to(b.ROOT)):b.digest(p) for p in [NATIVE,b.ROOT/f'target/{a.profile}/grid-vehicle-force.exe']},scope='WholeA02B70 direct originaldispatch/wait/job plus actual9BAB10/A72400/B15E70/B0EE80 wheel contacts, owned0..4 contact slots/source static shape identities. Selected recorded car0/static11 metadata/MAX friction/shared materialtoken0, templates externally supplied, cold records still await classification; no shape allocator/world solve/full scheduler/driving.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))
if __name__=='__main__':main()
