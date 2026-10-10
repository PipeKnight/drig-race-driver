"""Whole source-backed suspension/history/grip update; all evidence private."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

from validate_binxml import decode
from validate_suspension_contact import ROOT, TRACK, SURFACE, WORLD, MECHANICS, put, ui, digest, execute
from validate_suspension_vehicle import cases as suspension_cases

NATIVE = ROOT/'local/behavior/vehicle-grip-probe.exe'
RECORD = 0x2908


def mechanics_fixture(out):
    """Independently decode source XML into the observed original initializer."""
    _, root, _ = decode(SURFACE.read_bytes())
    previous = MECHANICS.read_bytes()
    nodes = root.findall('MATERIAL')
    assert previous[:4] == b'MEC1' and len(nodes) == struct.unpack_from('<I', previous, 4)[0]
    records = []
    fields = [(0,'grip_per_ms',0), (4,'ms_for_max_effect',1), (8,'grip',1),
              (0xc,'camera_factor',0), (0x10,'roughness',0), (0x14,'minimumSpeedForSlowdown',20),
              (0x18,'speedForMaximumSlowdown',40), (0x1c,'slowdown',0), (0x20,'roll_induce',0),
              (0x64,'hardness',1), (0x68,'heat',1), (0x6c,'wear',1), (0x94,'bumpswavelength',.05),
              (0x98,'bumpsmagnitude',0), (0x9c,'bumpsrealproportion',0),
              (0xa0,'bumpsproportionCarDependent',0), (0xa4,'depth',0)]
    for i, node in enumerate(nodes):
        row = previous[8+i*32:40+i*32]
        assert row[:4] == node.get('name').encode('ascii')
        record = bytearray(0xb0)
        m = node.find('MECHANICS')
        assert bool(struct.unpack_from('<I',row,4)[0]) == (m is not None)
        if m is not None:
            for offset, key, default in fields:
                value = float(m.get(key, default))
                if offset == 0x9c:
                    value = min(1,max(0,value))
                put(record,offset,value)
            name=m.get('name').encode('ascii')
            record[0x24:0x24+len(name)]=name
            record[0x70]=1
            for offset,key,default in [(0x71,'curved',False),(0x72,'dirt_build_up',False),
                                       (0x75,'high_downforce',False),(0x90,'bumpstimebased',True)]:
                text=m.get(key,'').lower()
                record[offset]=int(text=='true' if text in ['true','false'] else default)
            record[0xa8:0xac]=m.get('tyreTable','DEFA').encode('ascii')[:4]
            record[0xac]=1
        assert record[0x90] == struct.unpack_from('<I',row,8)[0] and record[0x94:0xa8] == row[12:32]
        records.append(record)
    path=out/'mechanics-full.private.bin'
    path.write_bytes(b'MEX1'+struct.pack('<I',len(records))+b''.join(records))
    return path


def cases():
    rng=random.Random(2008170)
    result=[]
    for i,(delta,b) in enumerate(suspension_cases()):
        v=bytearray(rng.randbytes(0x300))
        for o,x in [(0xfcc,.53),(0xfe4,.3),(0xe1c,[0,.95,.4,1][i%4]),(0xe20,.7),(0xe74,i%3),
                    (0x1160,5),(0x1164,.3),(0x1168,1),(0x116c,.1),(0x1170,.12),(0x1174,.08),
                    (0x11f4,.25),(0x12a8,[.85,1,0,2][i%4]),(0x1354,.2),(0x137c,0),(0x187c,.15),
                    (0x1efc,1),(0x1f10,1),(0x1f20,1),(0x1f24,.95),(0x1f3c,1/2400),(0x1f64,.55),
                    (0x1f8c,.8),(0x1f90,1.1),(0x1fa4,.1),(0x1fa8,.2),(0x1ffc,.5),
                    (0x21f8,2),(0x21fc,6),(0x2200,.35),(0x185c,.96)]: put(b,o,x)
        put(b,0x11fc,[rng.uniform(.8,1.2) for _ in range(10)])
        put(b,0x1648,[0,1.5,3,4,5.5,6,12,24,48,100][i%10])
        ui(b,0xb10,((i%2)<<8)|(((i//2)%2)<<16))
        ui(b,0xe38,i%2);ui(b,0x1558,(i%2)|(((i//3)%2)<<16));ui(b,0x1640,i%2)
        ui(b,0x1620,4);ui(b,0x227c,sum(((i>>j)&1)<<(j*8) for j in range(4)));ui(b,0x196c,[0,1,9][i%3]);ui(b,0x2204,0x100|(i%2));ui(b,0x21ec,i%5)
        if i%4==0:
            forward=struct.unpack_from('<4f',b,0x1600)
            speed=struct.unpack_from('<f',b,0x1648)[0]
            put(b,0x15c0,[x*speed for x in forward])
            put(b,0x15d0,[0]*4)
        for j in range(4):
            w=0x1990+j*0x150
            ui(b,0xdb8+j*4,(i+j)%2)
            for o,x in [(0xb0,.0001),(0xb4,-.0002),(0xb8,.7 if i%2 else .1),(0xbc,.15),
                        (0x80,rng.uniform(.8,1.2)),(0x104,rng.uniform(-10,150)),(0xe4,1),
                        (0xec,rng.uniform(-.03,.03)),(0x10c,0 if (i+j)%5==0 else (1 if j%2 else -1)),
                        (0x11c,.6),(0x120,.1),(0x124,.2)]: put(b,w+o,x)
            ui(b,w+0xfc,1|((i%2)<<8));ui(b,w+0x128,1 if i%3 else 0)
            put(v,0x190+j*16,[rng.uniform(-5,5),0,rng.uniform(0,20),0])
            put(v,0x1d0+j*16,[0,0,1,0]);put(v,0x210+j*16,[1,0,0,0]);put(v,0x298+j*4,rng.uniform(.8,1.2))
        result.append((delta,b,v))
    return result


def request_bytes(current):
    return b'VG01'+struct.pack('<I',len(current))+b''.join(struct.pack('<If',i,d)+b+v for i,(d,b,v) in enumerate(current))


def arguments(kind,profile,request,target,full):
    if kind=='rust':return ROOT/f'target/{profile}/grid-suspension-contact.exe',[TRACK,SURFACE,request,target]
    return NATIVE,[ROOT/'local/compatibility/grid-unpacked.exe',WORLD,request,target,MECHANICS,full]


def compare(data, outputs):
    original,repeat,rust=[outputs[k] for k in ['original','repeat','rust']]
    assert len(data)==len(original)==len(repeat)==len(rust)
    differences=[]
    coverage=dict(nonzero_grip=0,history_changes=0,notifications=0,shape_publications=0,low_speed_flag=0,spin_zeroing=0)
    count=(len(data)-8)//RECORD
    assert original[:8]==repeat[:8]==rust[:8]==data[:8]
    for i in range(count):
        start=16+i*RECORD
        assert original[start-8:start]==rust[start-8:start]==data[start-8:start]
        for o in range(0,0x2900,4):
            a,b=original[start+o:start+o+4],rust[start+o:start+o+4]
            if a!=b:differences.append(dict(case=i,offset=f'{o:x}',original=a.hex(),rust=b.hex()))
        before,after=data[start:start+0x2600],original[start:start+0x2600]
        # Inertia plus grip/alignment are the only force-view output fields.
        for o in range(0,0x300,4):
            if o not in [0x48,0x4c,0x50] and not 0x5c<=o<0x7c:
                assert data[start+0x2600+o:start+0x2604+o]==original[start+0x2600+o:start+0x2604+o]
        coverage['notifications']+=after[0x21ec:0x21f0]!=before[0x21ec:0x21f0]
        coverage['low_speed_flag']+=bool(after[0x2204])
        for j in range(4):
            w=0x1990+j*0x150
            value=original[start+w+0x78:start+w+0x7c]
            assert value==original[start+0x2600+0x5c+j*4:start+0x2600+0x60+j*4]
            assert after[w+0xfd]==0
            coverage['nonzero_grip']+=struct.unpack('<f',value)[0]!=0
            coverage['history_changes']+=before[w+0x11c:w+0x12c]!=after[w+0x11c:w+0x12c]
            coverage['shape_publications']+=before[0x250+j*16:0x260+j*16]!=after[0x250+j*16:0x260+j*16]
            coverage['spin_zeroing']+=before[w+0x104:w+0x108]!=bytes(4) and after[w+0x104:w+0x108]==bytes(4)
    return dict(calls=count,words=count*0x2900//4,repeat_exact=original==repeat,differing_words=len(differences),differences=differences,coverage=coverage)


def run_set(out,profile,branches,prefix,full):
    outputs={};requests={};runs=[]
    for kind,current in branches.items():
        request=out/f'{prefix}-{kind}-input.bin';requests[kind]=request_bytes(current);request.write_bytes(requests[kind])
        target=out/f'{prefix}-{kind}.bin';exe,args=arguments(kind,profile,request,target,full)
        runs.append(execute(exe,args,out,prefix+'-'+kind));outputs[kind]=target.read_bytes()
    report=compare(requests['original'],outputs)
    report.update(executions=runs,request_sha256=hashlib.sha256(requests['original']).hexdigest(),
                  hashes={k:hashlib.sha256(v).hexdigest() for k,v in outputs.items()})
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2))
    return report,outputs


def guards(out,profile,first,full):
    tests=[('index',0,9),('delta_zero',4,0),('delta_nan',4,0x7fc00000),('delta_large',4,0x3e800000),
           ('delta_tiny',4,1),('view_pointer',8,1),('wheel_count',8+0x1620,5),
           ('context_count',8+0x3d0+0x5d8,5),('extra_axes',8+0x3d0+0x70c,1),
           ('body_pointer',8+0x3d0+0x5d4,1),('table_pointer',8+0x3d0+0x608,1),('table_index',8+0x3d0+0x60c,1),
           ('wheel_pointer',8+0x3d0+0x178,1),('object_pointer',8+0x3d0+0x17c,1),
           ('disabled_wheel',8+0x2278,1),('event_count',8+0x21ec,5),('mass_zero',8+0x15a0,0),
           ('inertia_zero',8+0x15a8,0),('load_fraction',8+0xfcc,0x3f800000),
           ('speed_negative',8+0x1648,0xbf800000),('speed_index_overflow',8+0x1648,0x7f000000),
           ('alignment_one',8+0x137c,0x3f800000),('grip_scale_zero',8+0x1f3c,0),
           ('history_scale_negative',8+0x1ffc,0xbf800000),('modifier_zero_divisor',8+0x1160,0),
           ('equal_low_speed_bounds',8+0x21fc,0x40000000),('negative_power',8+0x12a8,0xbf800000),
           ('nan_power',8+0x12a8,0x7fc00000),('nan_table',8+0x11fc,0x7fc00000),
           ('nan_history',8+0x1990+0x11c,0x7fc00000),('unowned_history',8+0x1990+0x128,0xffffffff),
           ('nan_velocity_view',0x2608+0x190,0x7fc00000),('nan_axis_view',0x2608+0x210,0x7fc00000),
           ('nan_modifier_view',0x2608+0x298,0x7fc00000)]
    records=[]
    for name,offset,value in tests:
        record=bytearray(first);ui(record,offset,value);records.append((name,b'VG01'+struct.pack('<I',1)+record))
    valid=b'VG01'+struct.pack('<I',1)+first
    records += [('truncated',valid[:-1]),('trailing',valid+b'!'),('zero_records',b'VG01'+bytes(4)),('too_many',b'VG01'+struct.pack('<I',129))]
    results=[]
    for name,data in records:
        request=out/f'guard-{name}-input.bin';request.write_bytes(data)
        for kind in ['original','rust']:
            target=out/f'guard-{name}-{kind}.bin';exe,args=arguments(kind,profile,request,target,full)
            run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            passed=(run.returncode in [63,65,66,67,68,69,78] and 'records=0' in run.stdout and 'code_unchanged=1' in run.stdout) if kind=='original' else run.returncode==1
            results.append(dict(name=name,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=passed and not target.exists()))
    (out/'guards-validation.json').write_text(json.dumps(results,indent=2))
    assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results


def carried(out,profile,seed,full):
    branches={k:[(d,bytearray(b),bytearray(v)) for d,b,v in seed] for k in ['original','repeat','rust']}
    reports=[]
    for step in range(12):
        report,outputs=run_set(out,profile,branches,f'carry-{step}',full);reports.append(report)
        assert report['repeat_exact'] and not report['differences'],f'carried step {step} differs'
        for kind,current in branches.items():
            following=[]
            for i,(delta,previous,_) in enumerate(current):
                start=16+i*RECORD;body=bytearray(outputs[kind][start:start+0x2600]);view=bytearray(outputs[kind][start+0x2600:start+0x2900])
                body[0xb40:0xb80]=previous[0x15e0:0x1620];body[0xb80:0xba0]=previous[0xb20:0xb40];body[0xb20:0xb40]=body[0x15c0:0x15e0]
                # Authored inter-frame gravity/history transfer, not a world tick.
                put(body,0x15c4,struct.unpack_from('<f',body,0x15c4)[0]-9.81*delta)
                following.append((delta,body,view))
            branches[kind]=following
    return dict(sequences=len(seed),steps=12,calls=len(seed)*12,words=len(seed)*12*0x2900//4,
                coverage={key:sum(r['coverage'][key] for r in reports) for key in reports[0]['coverage']},reports=reports)


def shared_regression(out,profile,full):
    reports=[]
    for tag,prior in [('SP01','suspension-contact-20261010-debug-final-05'),('SF01','suspension-vehicle-20261010-debug-final-04')]:
        directory=ROOT/'local/behavior'/prior
        pairs=[(directory/'request.private.bin',directory/'original.bin')] if tag=='SP01' else [(directory/'initial-input.bin',directory/'initial-original.bin')]
        for step in range(8 if tag=='SP01' else 12):
            pairs.append((directory/f'carry-{step}-original-in.bin',directory/f'carry-{step}-original-out.bin') if tag=='SP01' else
                         (directory/f'carry-{step}-original-input.bin',directory/f'carry-{step}-original-out.bin'))
        for index,(request,expected) in enumerate(pairs):
            prefix=f'regression-{tag}-{index}'
            target=out/f'{prefix}-original.bin'
            run=execute(NATIVE,[ROOT/'local/compatibility/grid-unpacked.exe',WORLD,request,target,MECHANICS,full],out,prefix+'-original')
            rust_target=out/f'{prefix}-rust.bin'
            rust_run=execute(ROOT/f'target/{profile}/grid-suspension-contact.exe',[TRACK,SURFACE,request,rust_target],out,prefix+'-rust')
            exact=target.read_bytes()==rust_target.read_bytes()==expected.read_bytes()
            calls=struct.unpack_from('<I',request.read_bytes(),4)[0]
            reports.append(dict(tag=tag,calls=calls,words=calls*(0x2600 if tag=='SP01' else 0x260c)//4,request_sha256=digest(request),expected_sha256=digest(expected),original_sha256=digest(target),rust_sha256=digest(rust_target),exact=exact,executions=[run,rust_run]))
            assert exact
    return reports


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',required=True,choices=['debug','release'])
    a=parser.parse_args();out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    assert digest(SURFACE)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    assert digest(TRACK)=='6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'
    assert digest(WORLD)=='b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7'
    assert digest(MECHANICS)=='71a97e7a6f5fc65ec52a7a94e7e6b5dbbea054fecf8f3dccaf8edfd6785a5897'
    full=mechanics_fixture(out);current=cases();branches={k:current for k in ['original','repeat','rust']}
    report,_=run_set(out,a.profile,branches,'initial',full)
    if report['repeat_exact'] and not report['differences']:
        report['guards']=guards(out,a.profile,request_bytes(current)[8:8+RECORD],full)
        report['carried']=carried(out,a.profile,current[8:24],full)
        report['shared_regression']=shared_regression(out,a.profile,full)
    report.update(profile=a.profile,full_mechanics_sha256=digest(full),scope='Whole unchanged A00E10, real source contacts and mechanics; authored body/view schedule, four active wheels, extra axes and tyre table inactive. No live frame or driving claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report[k] for k in ['profile','calls','words','repeat_exact','differing_words','coverage']}|dict(guards=len(report.get('guards',[])),carried_calls=report.get('carried',{}).get('calls',0))))
    assert report['repeat_exact'] and not report['differences']


if __name__=='__main__':main()
