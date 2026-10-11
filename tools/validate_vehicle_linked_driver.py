"""Owned sync -> initialized source driver -> no-advance physical publication.

Explicit composed contract, not whole A09A80/A06A90/A03220/world execution.
"""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_driver as driver
import validate_vehicle_gearing as gearing
import validate_vehicle_sync as sync
import vehicle_source_gearing as sources

DESTINATIONS=[0x2264,0x2268,0x2280,0x22b0,0x2284,0x2288,0x228c,0x2290,0x2294,0x22a0,0x22a4,0x22a8,0x22ac,0x22b4,0x22b8,0x22bc,0x22c0,0x22c4,0x22c8,0x2298,0x229c]


def owner(record):
    car=bytearray(sync.CAR);car[:0x2600]=record[36:36+0x2600];physical=bytearray(sync.PHYSICAL)
    for source,to in zip(DESTINATIONS,sync.SETTINGS,strict=True):car[to:to+4]=car[source:source+4]
    for to,source in [(0xfa8,0x15a0),(0xfac,0x15a8),(0xfb0,0x15ac),(0xfb4,0x15b0)]:car[to:to+4]=car[source:source+4]
    struct.pack_into('<f',car,0x1560,1);struct.pack_into('<I',car,0xd234,7);struct.pack_into('<I',car,0xdb0,0)
    car[0x22e4:0x22e8]=car[0x2278:0x227c];car[0x22e0:0x22e4]=car[0x227c:0x2280]
    car[0xd1b4]=max(0,min(255,int(struct.unpack_from('<f',car,0x2274)[0])))
    for to,source in [(0xd1cc,0x22cc),(0xd1e4,0x22cd),(0xd1fc,0x22ce),(0xd214,0x22cf)]:car[to]=car[source]
    for to,source,n in [(0x1d0,0x15e0,64),(0x210,0x15d0,16),(0x220,0x15c0,16)]:physical[to:to+n]=car[source:source+n]
    struct.pack_into('<4f',physical,0x230,.2,-.3,.4,0)
    return bytes(car+physical)


def publish(out,profile,branches,owners,prefix,ignore_temporary=False):
    b=driver.base;results={};runs=[]
    for kind,records in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}.bin';rows=[]
        for r,o in zip(records,owners[kind],strict=True):
            vehicle=bytearray(r[b.BODY:b.VIEW]);struct.pack_into('<I',vehicle,0x15cc,0);struct.pack_into('<I',vehicle,0x15dc,0)
            rows.append(struct.pack('<6I',0,0,0,0,0,0)+o[sync.CAR:]+vehicle+bytes(32))
        source.write_bytes(b'VVB1'+struct.pack('<I',len(rows))+b''.join(rows))
        exe=b.ROOT/f'target/{profile}/grid-vehicle-bridge.exe' if kind=='rust' else b.ROOT/'local/behavior/vehicle-bridge-probe.exe'
        args=[source,target] if kind=='rust' else [b.REFERENCE,source,target]
        runs.append(b.execute(exe,args,out,f'{prefix}-{kind}'))
        data=target.read_bytes();assert len(data)==8+len(rows)*(24+3*(0x424+0x2600))
        following=[];newowners=[]
        for i,(r,old) in enumerate(zip(records,owners[kind],strict=True)):
            start=8+i*(24+3*(0x424+0x2600))+24;stage=data[start:start+0x424+0x2600]
            assert data[start:start+3*len(stage)]==stage*3 # No second advance/correction.
            physical=stage[:0x424];car=bytearray(old[:sync.CAR]);car[:0x2600]=stage[0x424:]
            row=bytearray(r);row[b.BODY:b.VIEW]=car[:0x2600]
            following.append(bytes(row));newowners.append(bytes(car+physical))
        results[kind]=(following,newowners)
    if ignore_temporary:
        assert results['original'][1]==results['repeat'][1]==results['rust'][1]
    else:assert results['original']==results['repeat']==results['rust']
    report=dict(calls=len(branches['original']),whole_buffers_exact=True,advance_calls=0,correction_calls=0,executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));return report,results


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);mode=p.add_mutually_exclusive_group();mode.add_argument('--whole-driver',action='store_true');mode.add_argument('--whole-step',action='store_true');p.add_argument('--idle',action='store_true');a=p.parse_args()
    if a.idle and not a.whole_step:p.error('--idle requires --whole-step')
    b=driver.base;out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=[]
    for i,raw in enumerate(driver.cases()):
        r=bytearray(raw);t=tables[i%len(tables)];gearing.apply_source_shift(r,t)
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9]);struct.pack_into('<I',r,b.BODY+0x196c,1+i%count)
        struct.pack_into('<I',r,b.BODY+0xe48,10);struct.pack_into('<I',r,b.BODY+0xdb0,0);r[b.BODY+0x1972]=0
        records.append(bytes(r))
    b.MAGIC=b'EF01';b.GRIP_TAG=b'VU01';b.GRIP_NATIVE=b.ROOT/'local/behavior/vehicle-contact-update-probe.exe';b.DRIVER_PREPARE=True;b.POST_FORCE=True;b.POST_CHASSIS=True
    if a.whole_driver:
        import validate_whole_vehicle_driver as whole
        whole.configure()
    if a.whole_step:
        import validate_whole_vehicle_step as step
        records=step.cases(tables,a.idle);b.MAGIC=b'QF01';b.run_set=step.compare
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    full=b.grip.mechanics_fixture(out);reports=[];sync_reports=[];publication=[];initial=[];coverage=collections.Counter()
    def tick(branches,owners,prefix):
        report,owners=sync.compare(out,a.profile,owners,prefix+'-sync');sync_reports.append(report)
        for kind,rows in branches.items():
            branches[kind]=[r[:b.BODY]+o[:0x2600]+r[b.VIEW:] for r,o in zip(rows,owners[kind],strict=True)]
        report,outputs=b.run_set(out,a.profile,branches,prefix,full);reports.append(report);coverage.update(report['coverage'])
        after={k:[data[8+i*b.RECORD:8+(i+1)*b.RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}
        for kind,rows in after.items():
            for i,row in enumerate(rows):
                r=bytearray(row);previous=branches[kind][i];o=b.BODY
                r[o+0xb40:o+0xb80]=previous[o+0x15e0:o+0x1620];r[o+0xb80:o+0xba0]=previous[o+0xb20:o+0xb40];r[o+0xb20:o+0xb40]=r[o+0x15c0:o+0x15e0]
                if a.whole_step:
                    b.put(r,o+0xe1c,.5)
                    if a.idle:
                        inactive=int(len(reports)%4<2);r[o+0xe38]=inactive
                        b.put(r,o+0xe1c,0 if inactive else .5);b.put(r,o+0xe20,0)
                rows[i]=bytes(r)
        report,result=publish(out,a.profile,after,owners,prefix+'-publish',a.whole_step);publication.append(report)
        return {k:v[0] for k,v in result.items()},{k:v[1] for k,v in result.items()}
    for start in range(0,len(records),40):
        batch=records[start:start+40];r,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(r)
        if a.whole_step:branches=step.runtime_modes(branches,a.idle)
        owners={k:[owner(r) for r in rows] for k,rows in branches.items()};tick(branches,owners,f'initial-{start}')
    r,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(r)
    if a.whole_step:branches=step.runtime_modes(branches,a.idle)
    owners={k:[owner(r) for r in rows] for k,rows in branches.items()}
    for step in range(12):branches,owners=tick(branches,owners,f'carry-{step}')
    required=['contacts','spin','chassis','velocity'] if a.whole_driver or a.whole_step else ['contacts','spin_changes','chassis_matrix','driver_velocity']
    assert all(coverage[k]>0 for k in required)
    result=dict(profile=a.profile,driver_calls=sum(r['calls'] for r in reports),driver_record_bytes=sum(r.get('record_bytes',r.get('observable_bytes',0)) for r in reports),sync_calls=sum(r['calls'] for r in sync_reports),sync_bytes=sum(r['record_bytes'] for r in sync_reports),source_files=len(tables),coverage=dict(coverage),initialization_reports=initial,sync_reports=sync_reports,driver_reports=reports,publication_reports=publication,scope='Whole9BD7F0 -> selected initialized source driver -> linked setters/readback without advance/correction, independent carried states. Authored history/ownership, zero deformation dimensions/shape lists, omitted A06A90 tail/A03220/world dispatch; no actual whole synchronous wrapper or driving claim.')
    if a.whole_driver:result['scope']='Whole9BD7F0 -> whole original A03670 versus Rust -> linked setters/readback without advance/correction; independent carried state, authored history/ownership, zero deformation/emptyshapes, omitted A06A90tail/A03220/world dispatch. No whole synchronous wrapper/driving claim.'
    if a.whole_step:
        result['driver_observable_bytes']=result.pop('driver_record_bytes');result['driver_body_bytes']=sum(r['body_bytes'] for r in reports)
        result['scope']='Whole9BD7F0 -> whole original A06A90 versus Rust -> linked publication without advance/correction, each branch carrying its own state. Full car and physical state compare; expired native temporary view/solver excluded. Authored input/history/ownership, assisted automatic modes one/two with clutch transitions, zero deformation/emptyshapes; omitted A03220/world dispatch, no driving claim.'
        result['idle']=a.idle
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))


if __name__=='__main__':main()
