"""Source sparse gear tables through unchanged whole controls and wheel forces."""
import argparse
import collections
import json
import struct
import subprocess
from pathlib import Path
import validate_vehicle_controls as controls
import validate_vehicle_propulsion as base
import vehicle_source_gearing as sources

TABLES=[]


def cases():
    seeds=controls.cases();result=[]
    for ordinal,t in enumerate(TABLES):
        count=sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9]);assert count>=4
        for mode in range(12):
            r=bytearray(seeds[(ordinal*12+mode)%len(seeds)]);b=36
            for i,w in enumerate(t['words']):struct.pack_into('<I',r,b+0x18f4+i*4,w)
            gear=[1,count-1,count,9][mode%4]
            for o,x in [(0x196c,gear),(0xe48,10),(0xe2c,0)]:struct.pack_into('<I',r,b+o,x)
            for o,x in [(0x1970,int(mode>=4)),(0x1971,1),(0x1972,0),(0x1640,0),(0x18c8,int(mode%3==0)),(0xe24,int(mode%4==2)),(0xe28,int(mode%4==1))]:r[b+o]=x
            for o,x in [(0x226c,0.),(0x2280,float(mode>=8)),(0x1974,.5)]:struct.pack_into('<f',r,b+o,x)
            result.append(bytes(r))
    return result+seeds


def guards(out,profile,first):
    checks=controls.guards(out,profile,first)
    for name in ['zero_transition','downshift_underflow']:
        r=bytearray(controls.cases()[1]);b=36
        for o,x in [(0x196c,1),(0xe48,7 if name=='zero_transition' else 10),(0xe2c,0)]:struct.pack_into('<I',r,b+o,x)
        for o,x in [(0x1970,0),(0x1971,1),(0x1640,0),(0x18c8,0),(0xe24,0),(0xe28,int(name=='downshift_underflow'))]:r[b+o]=x
        for o,x in [(0x226c,0.),(0x2280,float(name=='downshift_underflow'))]:struct.pack_into('<f',r,b+o,x)
        for i in range(2,9):struct.pack_into('<I',r,b+0x18f4+i*4,0)
        for kind in ['native','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(base.request([bytes(r)]))
            exe=base.NATIVE if kind=='native' else base.ROOT/f'target/{profile}/grid-vehicle-force.exe'
            args=[base.REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=base.ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(42 if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and not run.stderr and 'completed=0 code_unchanged=1' in run.stdout
            checks.append(dict(name=name,kind=kind,passed=passed,exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
    (out/'sparse-guards.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']]
    return checks


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',required=True,choices=['debug','release']);args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((base.ROOT/'local').resolve())
    TABLES=sources.verify(out.with_name(out.name+'-source'),args.profile)
    base.MAGIC=b'CF01';base.NATIVE=base.ROOT/'local/behavior/vehicle-controls-probe.exe';base.cases=cases;base.guards=guards;base.main()
    path=out/'validation.json';r=json.loads(path.read_text());coverage=collections.Counter()
    for item in r['reports']+r['carried']:coverage.update(item['coverage'])
    for ordinal,t in enumerate(TABLES):
        index=ordinal*12+2;batch=(index//48)*48;slot=index%48
        data=(out/f'initial-{batch}-original.bin').read_bytes()
        gear=struct.unpack_from('<I',data,8+slot*base.RECORD_SIZE+36+0x196c)[0]
        assert gear==sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9])
    r['top_gear_upshift_controls']=len(TABLES)
    r['coverage']=dict(coverage);r['source_files']=len(TABLES);r['scope']='Source decoded sparse gear tables and constructor alias through whole9B40D0/propulsion/forces; authored contacts/thresholds and other tuning, search1348inactive. No full constructor/world/driving claim.'
    path.write_text(json.dumps(r,indent=2))
