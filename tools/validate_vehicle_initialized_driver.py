"""Source engine/gear initialization once, then independently carried full driver."""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_driver as driver
import validate_vehicle_gearing as gearing
import vehicle_source_gearing as sources


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    b=driver.base;out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile)
    records=[]
    for i,raw in enumerate(driver.cases()):
        r=bytearray(raw);t=tables[i%len(tables)];gearing.apply_source_shift(r,t)
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9])
        struct.pack_into('<I',r,b.BODY+0x196c,1+i%count);struct.pack_into('<I',r,b.BODY+0xe48,10);r[b.BODY+0x1972]=0
        records.append(bytes(r))
    b.MAGIC=b'EF01';b.GRIP_TAG=b'VU01';b.GRIP_NATIVE=b.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    b.DRIVER_PREPARE=True;b.POST_FORCE=True;b.POST_CHASSIS=True
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    full=b.grip.mechanics_fixture(out);initial=[];reports=[];coverage=collections.Counter()
    for start in range(0,len(records),40):
        batch=records[start:start+40]
        init,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(init)
        report,_=b.run_set(out,a.profile,branches,f'initial-{start}',full);reports.append(report);coverage.update(report['coverage'])
    init,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(init)
    for step in range(12):
        report,outputs=b.run_set(out,a.profile,branches,f'carry-{step}',full);reports.append(report);coverage.update(report['coverage'])
        for kind,data in outputs.items():
            following=[]
            for i,previous in enumerate(branches[kind]):
                r=bytearray(data[8+i*b.RECORD:8+(i+1)*b.RECORD]);o=b.BODY
                r[o+0xb40:o+0xb80]=previous[o+0x15e0:o+0x1620]
                r[o+0xb80:o+0xba0]=previous[o+0xb20:o+0xb40];r[o+0xb20:o+0xb40]=r[o+0x15c0:o+0x15e0]
                following.append(bytes(r))
            branches[kind]=following
    # Existing whole-driver guards exercise the resulting initialized frame.
    _,valid=gearing.initialize(out,a.profile,{k:[records[1]] for k in ['original','repeat','rust']},'init-guards')
    checks=b.guards(out,a.profile,valid['rust'][0],full)
    assert all(coverage[k]>0 for k in ['contacts','spin_changes','post_angular','chassis_matrix','driver_velocity'])
    result=dict(profile=a.profile,initialization_calls=sum(r['calls'] for r in initial)+1,driver_calls=sum(r['calls'] for r in reports),driver_record_bytes=sum(r['record_bytes'] for r in reports),source_files=len(tables),source_shift_words=len(tables)*34,initialization_reports=initial,reports=reports,coverage=dict(coverage),guards=len(checks),scope='Source max engine speed/torque curve/second-downshift scale/sparse ratios -> whole9AD6B0 initialization once -> selected early driver/source contact/suspension/controls/forces/assists/chassis. Other engine configuration/setup/history and world timing remain supplied; no complete constructor/world/driving claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k not in ['initialization_reports','reports']}))


if __name__=='__main__':main()
