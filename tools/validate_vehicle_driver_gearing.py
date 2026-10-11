"""Source gear tables joined to gravity/contact/force/assist/chassis execution."""
import argparse
import json
import struct
from pathlib import Path
import validate_vehicle_driver as driver
import vehicle_source_gearing as sources

TABLES=[]


def cases():
    result=[]
    for i,raw in enumerate(driver.cases()):
        t=TABLES[i%len(TABLES)];r=bytearray(raw);b=driver.base.BODY
        for j,w in enumerate(t['words']):struct.pack_into('<I',r,b+0x18f4+j*4,w)
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9])
        struct.pack_into('<I',r,b+0x196c,1+i%count)
        struct.pack_into('<I',r,b+0xe48,10);r[b+0x1972]=0
        result.append(bytes(r))
    return result


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    b=driver.base;out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve())
    TABLES=sources.verify(out.with_name(out.name+'-source'),a.profile)
    b.MAGIC=b'EF01';b.GRIP_TAG=b'VU01';b.GRIP_NATIVE=b.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    b.DRIVER_PREPARE=True;b.POST_FORCE=True;b.POST_CHASSIS=True;b.cases=cases;b.main()
    path=out/'validation.json';r=json.loads(path.read_text());r['source_gearing_files']=len(TABLES)
    r['scope']='Source decoded sparse gear tables and constructor alias joined to selected early driver/source contact/suspension/controls/forces/assists/chassis; other initialization and shift thresholds supplied. No full constructor/world/driving claim.'
    path.write_text(json.dumps(r,indent=2))
