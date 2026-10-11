"""Independent carried sync -> whole vehicle -> whole A03220 -> pose publication.

The original world scheduler and complete A09A80 are not executed here.
"""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_linked_driver as linked
import validate_vehicle_sync as sync
import validate_whole_vehicle_step as step
import validate_vehicle_publication as publication
import validate_vehicle_gearing as gearing
import vehicle_source_gearing as sources
b=step.b


def narrow(owner):return owner[:sync.CAR]+owner[publication.CAR:]


def merge(old,small):return small[:sync.CAR]+old[sync.CAR:publication.CAR]+small[sync.CAR:]


def owner(record,index):
    small=linked.owner(record)
    car=publication.configure(bytearray(small[:sync.CAR]+bytes(publication.CAR-sync.CAR)))
    struct.pack_into('<f',car,0xc798,[0,.016,.08,.2][index%4])
    struct.pack_into('<f',car,0xc790,[0,.001,.002,1][index%4])
    return bytes(car)+small[sync.CAR:]


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=step.cases(tables,True);full=b.grip.mechanics_fixture(out)
    reports=[];sync_reports=[];outer=[];poses=[];initial=[];coverage=collections.Counter();outer_coverage=collections.Counter()
    def tick(branches,owners,prefix,frame):
        r,small=sync.compare(out,a.profile,{k:[narrow(o) for o in rows] for k,rows in owners.items()},prefix+'-sync');sync_reports.append(r)
        owners={k:[merge(o,n) for o,n in zip(owners[k],small[k],strict=True)] for k in owners}
        for kind,rows in branches.items():branches[kind]=[r[:b.BODY]+o[:0x2600]+r[b.VIEW:] for r,o in zip(rows,owners[kind],strict=True)]
        r,outputs=step.compare(out,a.profile,branches,prefix+'-driver',full);reports.append(r);coverage.update(r['coverage'])
        after={k:[data[8+i*b.RECORD:8+(i+1)*b.RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}
        requests={}
        for kind,rows in after.items():
            requests[kind]=[]
            for row,old in zip(rows,owners[kind],strict=True):
                car=bytearray(old[:publication.CAR]);car[:0x2600]=row[b.BODY:b.VIEW]
                requests[kind].append(row[4:8]+car+old[publication.CAR:])
        r,published=publication.compare(out,a.profile,requests,prefix+'-outer');outer.append(r);outer_coverage.update(r['coverage'])
        for kind,rows in published.items():
            owners[kind]=[row[4:] for row in rows]
            after[kind]=[row[:b.BODY]+o[:0x2600]+row[b.VIEW:] for row,o in zip(after[kind],owners[kind],strict=True)]
        r,result=linked.publish(out,a.profile,after,{k:[narrow(o) for o in rows] for k,rows in owners.items()},prefix+'-pose',True);poses.append(r)
        following={};newowners={}
        for kind,(rows,small) in result.items():
            following[kind]=[];newowners[kind]=[]
            for i,(row,n) in enumerate(zip(rows,small,strict=True)):
                r=bytearray(row);previous=branches[kind][i];o=b.BODY
                r[o+0xb40:o+0xb80]=previous[o+0x15e0:o+0x1620];r[o+0xb80:o+0xba0]=previous[o+0xb20:o+0xb40];r[o+0xb20:o+0xb40]=r[o+0x15c0:o+0x15e0]
                inactive=int(frame%4<2);r[o+0xe38]=inactive;b.put(r,o+0xe1c,0 if inactive else .5);b.put(r,o+0xe20,0)
                wide=bytearray(merge(owners[kind][i],n));wide[:0x2600]=r[b.BODY:b.VIEW]
                following[kind].append(bytes(r));newowners[kind].append(bytes(wide))
        assert newowners['original']==newowners['repeat']==newowners['rust']
        return following,newowners
    for start in range(0,len(records),32):
        batch=records[start:start+32];r,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(r);branches=step.runtime_modes(branches,True)
        owners={k:[owner(row,start+i) for i,row in enumerate(rows)] for k,rows in branches.items()}
        tick(branches,owners,f'initial-{start}',0)
    r,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(r);branches=step.runtime_modes(branches,True)
    owners={k:[owner(row,8+i) for i,row in enumerate(rows)] for k,rows in branches.items()}
    for frame in range(12):branches,owners=tick(branches,owners,f'carry-{frame}',frame)
    assert all(coverage[k]>0 for k in ['contacts','mode_transition','mode_two','spin'])
    assert all(outer_coverage[k]>0 for k in ['timer','decay','decay_state'])
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),source_files=len(tables),driver_car_bytes=sum(r['body_bytes'] for r in reports),driver_observable_bytes=sum(r['observable_bytes'] for r in reports),sync_bytes=sum(r['record_bytes'] for r in sync_reports),outer_bytes=sum(r['record_bytes'] for r in outer),coverage=dict(coverage),outer_coverage=dict(outer_coverage),driver_reports=reports,sync_reports=sync_reports,outer_reports=outer,pose_reports=poses,initialization_reports=initial,scope='Whole9BD7F0 -> wholeA06A90 -> wholeA03220 -> pose/velocity setters/readback, each branch carries its own full21000car and424physical. Authored construction/history/controls, stable setup, empty manager/render/UI/shapes, no advance/correction; expired native driver view/solver excluded. Not wholeA09A80/world/driving.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))
if __name__=='__main__':main()
