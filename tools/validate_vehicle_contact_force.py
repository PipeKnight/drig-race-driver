"""Source-backed contact/grip through post-grip forces; private composed reference."""
import argparse
import collections
import hashlib
import json
import struct
import subprocess
from pathlib import Path
import validate_vehicle_grip as grip
import validate_vehicle_prepared as prepared
from validate_suspension_contact import ROOT,TRACK,SURFACE,WORLD,MECHANICS,put,ui,digest

RECORD=15384
BODY=36
VIEW=BODY+0x2600
SOLVER=VIEW+0x300
MATERIAL=SOLVER+0xdd0
MAGIC=b'JF01'
GRIP_TAG=b'VG01'
GRIP_NATIVE=grip.NATIVE
REFERENCE=ROOT/'local/compatibility/grid-unpacked.exe'
POST_FORCE=False
POST_CHASSIS=False
DRIVER_PREPARE=False


def request(records,tag=None):
    if tag is None:tag=MAGIC
    return tag+struct.pack('<I',len(records))+b''.join(struct.pack('<I',i)+r[4:] for i,r in enumerate(records))


def cases():
    seeds=prepared.cases();result=[]
    for i,(delta,b,v) in enumerate(grip.cases()):
        r=bytearray(seeds[i]);body=bytearray(r[BODY:VIEW]);view=bytearray(r[VIEW:SOLVER])
        body[0x30c:0xdb8]=b[0x30c:0xdb8]
        for start,end in [(0xdb8,0xdc8),(0x15a0,0x15b4),(0x15c0,0x1620),(0x1648,0x164c),
                          (0x1650,0x1690),(0x2198,0x21c4),(0x2278,0x2280)]:body[start:end]=b[start:end]
        put(body,0x15a4,1/struct.unpack_from('<f',body,0x15a0)[0])
        for o in [0xfcc,0xfe4,0xe74,0x1160,0x1164,0x1168,0x116c,0x1170,0x1174,0x11f4,0x12a8,
                  0x1354,0x137c,0x187c,0x1efc,0x1f10,0x1f20,0x1f24,0x1f3c,0x1f64,0x1f8c,
                  0x1f90,0x1fa4,0x1fa8,0x1ffc,0x21f8,0x21fc,0x2200]:body[o:o+4]=b[o:o+4]
        body[0x11fc:0x1224]=b[0x11fc:0x1224]
        for j in range(4):
            w=0x1990+j*0x150
            for start,end in [(0,0x20),(0x3c,0x84),(0xb0,0xc0),(0xe0,0xe8),(0xec,0xf0),
                              (0x104,0x108),(0x110,0x118),(0x11c,0x12c),(0x138,0x144)]:body[w+start:w+end]=b[w+start:w+end]
            ui(body,w+0xc0,0);ui(body,w+0xc4,0);ui(body,w+0xc8,0)
            ui(r,20+j*4,0)
        for start,end in [(0x190,0x250),(0x298,0x2a8)]:view[start:end]=v[start:end]
        r[BODY:VIEW]=body;r[VIEW:SOLVER]=view
        r[MATERIAL:MATERIAL+4*0x140]=bytes(4*0x140)
        struct.pack_into('<f',r,4,delta)
        result.append(bytes(r))
    return result


def execute(exe,args,out,name,allow_failure=False):
    run=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
    item=dict(exe=str(exe),sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
    (out/f'{name}-execution.json').write_text(json.dumps(item,indent=2))
    if not allow_failure:assert run.returncode==0,item
    return item


def native(out,records,name,full,allow_failure=False):
    prefix_runs=[]
    if DRIVER_PREPARE:
        source=out/f'{name}-driver-prepare-input.bin';target=out/f'{name}-driver-prepare-output.bin'
        source.write_bytes(request(records,b'DF01'))
        prefix_runs.append(execute(ROOT/'local/behavior/vehicle-driver-prepare-probe.exe',[REFERENCE,source,target],out,name+'-driver-prepare',allow_failure))
        if prefix_runs[-1]['exit']:return None,prefix_runs
        data=target.read_bytes();records=[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(records))]
    source=out/f'{name}-grip-input.bin';target=out/f'{name}-grip-output.bin'
    vg=[]
    for r in records:
        body=bytearray(r[BODY:VIEW]);ui(body,0,0)
        vg.append(struct.pack('<If',len(vg),struct.unpack_from('<f',r,4)[0])+body+r[VIEW:SOLVER])
    source.write_bytes(GRIP_TAG+struct.pack('<I',len(vg))+b''.join(vg))
    runs=prefix_runs+[execute(GRIP_NATIVE,[REFERENCE,WORLD,source,target,MECHANICS,full],out,name+'-grip',allow_failure)]
    if runs[-1]['exit']:return None,runs
    data=target.read_bytes();mechanics=full.read_bytes();following=[]
    for i,r in enumerate(records):
        r=bytearray(r);start=8+i*grip.RECORD
        body=bytearray(data[start+8:start+0x2608]);ui(body,0,struct.unpack_from('<I',r,12)[0])
        r[BODY:VIEW]=body;r[VIEW:SOLVER]=data[start+0x2608:start+0x2908]
        # Same caller-owned solver storage, including the actual overlapping prefix.
        r[SOLVER:SOLVER+0x50]=r[VIEW+0x2b0:SOLVER]
        for j in range(4):
            token=struct.unpack_from('<I',body,0x1990+j*0x150+0xc0)[0]
            ui(r,20+j*4,int(token!=0));m=MATERIAL+j*0x140;r[m:m+0x140]=bytes(0x140)
            if token:
                s=8+(token-1)*0xb0
                for o in [0x10,0x14,0x18,0x1c,0x20,0x64]:r[m+o:m+o+4]=mechanics[s+o:s+o+4]
                r[m+0x75]=mechanics[s+0x75]
        following.append(bytes(r))
    source=out/f'{name}-prepared-input.bin';target=out/f'{name}-prepared-output.bin'
    source.write_bytes(request(following,b'GF01'))
    runs.append(execute(ROOT/'local/behavior/vehicle-prepared-probe.exe',[REFERENCE,source,target],out,name+'-prepared',allow_failure))
    if runs[-1]['exit']:return None,runs
    result=MAGIC+target.read_bytes()[4:]
    if POST_FORCE:
        source=out/f'{name}-post-force-input.bin';target=out/f'{name}-post-force-output.bin'
        rows=[result[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(records))]
        source.write_bytes(b'TA01'+struct.pack('<I',len(rows))+b''.join(r[4:8]+r[BODY:VIEW] for r in rows))
        runs.append(execute(ROOT/'local/behavior/vehicle-post-force-probe.exe',[REFERENCE,source,target],out,name+'-post-force',allow_failure))
        if runs[-1]['exit']:return None,runs
        tail=target.read_bytes();joined=bytearray(result)
        for i in range(len(rows)):
            start=8+i*0x2604;joined[8+i*RECORD+BODY:8+i*RECORD+VIEW]=tail[start+4:start+0x2604]
        result=bytes(joined)
    if POST_CHASSIS:
        source=out/f'{name}-chassis-input.bin';target=out/f'{name}-chassis-output.bin'
        rows=[result[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(records))]
        source.write_bytes(b'MA01'+struct.pack('<I',len(rows))+b''.join(r[4:8]+r[BODY:VIEW] for r in rows))
        runs.append(execute(ROOT/'local/behavior/vehicle-chassis-probe.exe',[REFERENCE,source,target],out,name+'-chassis',allow_failure))
        if runs[-1]['exit']:return None,runs
        tail=target.read_bytes();joined=bytearray(result)
        for i in range(len(rows)):
            start=8+i*0x2604;joined[8+i*RECORD+BODY:8+i*RECORD+VIEW]=tail[start+4:start+0x2604]
        result=bytes(joined)
    (out/f'{name}-output.bin').write_bytes(result)
    return result,runs


def run_set(out,profile,branches,prefix,full):
    outputs={};executions=[]
    for kind,records in branches.items():
        name=prefix+'-'+kind
        if kind=='rust':
            source=out/f'{name}-input.bin';target=out/f'{name}-output.bin';source.write_bytes(request(records))
            executions.append(execute(ROOT/f'target/{profile}/grid-vehicle-force.exe',[TRACK,SURFACE,source,target],out,name))
            outputs[kind]=target.read_bytes()
        else:outputs[kind],runs=native(out,records,name,full);executions.extend(runs)
    a,repeat,b=[outputs[k] for k in ['original','repeat','rust']]
    assert len(a)==len(repeat)==len(b)==8+len(branches['original'])*RECORD
    diff=[dict(case=(i-8)//RECORD,offset=hex((i-8)%RECORD),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
    counts=collections.Counter()
    for i,r in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        if MAGIC in [b'UF01',b'HF01',b'MF01',b'EF01']:
            f32=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
            scale=struct.unpack_from('<f',r,BODY+0x3d0)[0];delta=struct.unpack_from('<f',r,4)[0]
            amount=f32(delta*.25);expected=min(f32(scale+amount),1) if scale<1 else max(f32(scale-amount),1)
            assert after[BODY+0x3d0:BODY+0x3d4]==struct.pack('<f',expected)
            counts['scale_changes']+=after[BODY+0x3d0:BODY+0x3d4]!=r[BODY+0x3d0:BODY+0x3d4]
            stage=(out/f'{prefix}-original-grip-output.bin').read_bytes();v=8+i*grip.RECORD+0x2608
            assert after[VIEW+0x190:VIEW+0x290]==stage[v+0x190:v+0x290]
            counts['prepared_vector_changes']+=after[VIEW+0x190:VIEW+0x290]!=r[VIEW+0x190:VIEW+0x290]
        if POST_FORCE:
            prior=(out/f'{prefix}-original-prepared-output.bin').read_bytes()[8+i*RECORD:8+(i+1)*RECORD]
            for label,o,size in [('speed_refresh',0x1648,4),('rpm_history',0x1f00,4),('post_angular',0x15d0,16)]:
                counts[label]+=after[BODY+o:BODY+o+size]!=prior[BODY+o:BODY+o+size]
            for j in range(4):
                for label,o in [('camber',0xec),('wheel_response',0x13c)]:
                    o=BODY+0x1990+j*0x150+o;counts[label]+=after[o:o+4]!=prior[o:o+4]
        counts['pose_changes']+=r[BODY+0x15e0:BODY+0x1620]!=after[BODY+0x15e0:BODY+0x1620]
        if DRIVER_PREPARE:
            prior=(out/f'{prefix}-original-driver-prepare-output.bin').read_bytes()[8+i*RECORD:8+(i+1)*RECORD]
            assert prior[BODY+0xb20:BODY+0xb40]==r[BODY+0x15c0:BODY+0x15e0]
            for label,start,end in [('driver_velocity',0x15c0,0x15e0),('driver_fuel',0x1fa0,0x1fa4),('driver_history',0xb20,0xb40),('driver_aero',0x1f10,0x1f14)]:counts[label]+=prior[BODY+start:BODY+end]!=r[BODY+start:BODY+end]
        if POST_CHASSIS:
            prior=(out/f'{prefix}-original-post-force-output.bin').read_bytes()[8+i*0x2604:8+(i+1)*0x2604]
            assert after[BODY+0x15e0:BODY+0x1620]==prior[4+0x15e0:4+0x1620]
            for label,start,end in [('chassis_matrix',0x10,0x50),('chassis_acceleration',0x50,0x60),('chassis_phases',0x60,0x68)]:
                counts[label]+=after[BODY+start:BODY+end]!=prior[4+start:4+end]
        counts['velocity_changes']+=r[BODY+0x15c0:BODY+0x15e0]!=after[BODY+0x15c0:BODY+0x15e0]
        for j in range(4):
            counts['contacts']+=struct.unpack_from('<I',after,20+j*4)[0]!=0
            o=BODY+0x1990+j*0x150
            counts['grip_nonzero']+=struct.unpack_from('<f',after,VIEW+0x5c+j*4)[0]!=0
            counts['spin_changes']+=r[o+0x104:o+0x108]!=after[o+0x104:o+0x108]
    report=dict(calls=len(branches['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(diff),differences=diff,coverage=dict(counts),executions=executions,hashes={k:hashlib.sha256(v).hexdigest() for k,v in outputs.items()})
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2))
    assert report['repeat_exact'] and not diff,dict(differing_words=len(diff),first=diff[:15])
    return report,outputs


def guards(out,profile,first,full):
    tests=[('zero_delta',4,0),('nan_delta',4,0x7fc00000),('tiny_delta',4,1),('large_delta',4,0x3e800000),
        ('context_count',BODY+0x3d0+0x5d8,5),('extra_axes',BODY+0x3d0+0x70c,1),('preselected',BODY+0x3d0+0x604,1),
        ('body_pointer',BODY+0x3d0+0x5d4,1),('table_pointer',BODY+0x3d0+0x608,1),('table_index',BODY+0x3d0+0x60c,1),
        ('wheel_pointer',BODY+0x3d0+0x178,1),('object_pointer',BODY+0x3d0+0x17c,1),
        ('disabled_wheel',BODY+0x2278,1),('event_count',BODY+0x21ec,5),('zero_mass',BODY+0x15a0,0),
        ('zero_inertia',BODY+0x15a8,0),('zero_radius',BODY+0x1990+0xe0,0),('negative_speed',BODY+0x1648,0xbf800000),
        ('grip_nan',BODY+0x12a8,0x7fc00000),('history_token',BODY+0x1990+0x128,0xffffffff),
        ('view_nan',VIEW+(0x298 if MAGIC in [b'UF01',b'HF01',b'MF01',b'EF01'] else 0x190),0x7fc00000),('late_mode',BODY+0x1598,0),('late_search',BODY+0x1348,1),
        ('late_override',BODY+0xe48,11),('late_engine',BODY+0x17a4,0),('late_timer',BODY+0x1f6c,0x7fc00000)]
    if MAGIC in [b'UF01',b'HF01',b'MF01',b'EF01']:tests.extend([('blend_nan',BODY+0x1990+0x20,0x7fc00000),('scale_zero',BODY+0x3d0,0)])
    if POST_FORCE:tests.append(('post_force_nan',BODY+0x2274,0x7fc00000))
    if POST_CHASSIS:tests.extend([('chassis_nan',BODY+0x1494,0x7fc00000),('chassis_phase',BODY+0x60,0x43480000)])
    if DRIVER_PREPARE:
        tests=[t for t in tests if t[0] not in ['negative_speed','view_nan']]
        tests.extend([('driver_previous_speed_nan',BODY+0x1648,0x7fc00000),('driver_friction_nan',BODY+0x1ee4,0x7fc00000),('driver_ring',BODY+0x3c0,5),('driver_respawn',BODY+0xdfc,1)])
    results=[]
    for name,offset,value in tests:
        r=bytearray(first);ui(r,offset,value);r=bytes(r)
        value,runs=native(out,[r],'guard-'+name+'-native',full,True)
        last=runs[-1]
        passed=value is None and 0<last['exit']<=80 and not last['stderr'] and 'code_unchanged=1' in last['stdout'] and not (out/f'guard-{name}-native-output.bin').exists()
        results.append(dict(name=name,kind='native',passed=passed,executions=runs))
        source=out/f'guard-{name}-rust-input.bin';target=out/f'guard-{name}-rust-output.bin';source.write_bytes(request([r]))
        run=execute(ROOT/f'target/{profile}/grid-vehicle-force.exe',[TRACK,SURFACE,source,target],out,'guard-'+name+'-rust',True)
        results.append(dict(name=name,kind='rust',passed=run['exit']==1 and not target.exists(),executions=[run]))
    # Framing and ownership are checked at the single Rust API's CLI boundary.
    payload=request([first])
    extra=[('bad_header',b'BAD!'+payload[4:]),('truncated',payload[:-1]),('trailing',payload+b'!'),
           ('zero_count',MAGIC+bytes(4)),('too_many',MAGIC+struct.pack('<I',65))]
    for name,offset,value in [('identity',0,1),('view_owner',BODY,0),('view_alias',VIEW,0),('overlap',VIEW+0x2b0,0xffffffff),
        ('wheel_owner',BODY+0x1990+0x34,0),('foreign_drive',BODY+0x18d8,1),('foreign_steering',BODY+0x21cc,1)]:
        r=bytearray(first);ui(r,offset,value);extra.append((name,MAGIC+struct.pack('<I',1)+r))
    for name,payload in extra:
        source=out/f'guard-{name}-input.bin';target=out/f'guard-{name}-output.bin';source.write_bytes(payload)
        run=execute(ROOT/f'target/{profile}/grid-vehicle-force.exe',[TRACK,SURFACE,source,target],out,'guard-'+name,True)
        results.append(dict(name=name,kind='rust-only-framing',passed=run['exit']==1 and not target.exists(),executions=[run]))
    source=out/'guard-existing-input.bin';target=out/'guard-existing-output.bin';source.write_bytes(request([first]));target.write_bytes(b'owned sentinel')
    run=execute(ROOT/f'target/{profile}/grid-vehicle-force.exe',[TRACK,SURFACE,source,target],out,'guard-existing',True)
    results.append(dict(name='existing_output',kind='rust-only-output',passed=run['exit']==1 and target.read_bytes()==b'owned sentinel',executions=[run]))
    (out/'guards.json').write_text(json.dumps(results,indent=2))
    assert all(r['passed'] for r in results),[r for r in results if not r['passed']]
    return results


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    assert digest(TRACK)=='6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'
    assert digest(SURFACE)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    assert digest(WORLD)=='b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7'
    full=grip.mechanics_fixture(out);current=cases();reports=[]
    for start in range(0,len(current),40):
        batch=current[start:start+40];r,_=run_set(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}',full);reports.append(r)
    branches={k:current[8:24] for k in ['original','repeat','rust']};carried=[]
    for step in range(12):
        r,outputs=run_set(out,a.profile,branches,f'carry-{step}',full);carried.append(r)
        for kind,data in outputs.items():
            following=[]
            for i,previous in enumerate(branches[kind]):
                r=bytearray(data[8+i*RECORD:8+(i+1)*RECORD]);b=BODY
                # Explicit authored history transfer. No extra pose advancement or external gravity.
                r[b+0xb40:b+0xb80]=previous[b+0x15e0:b+0x1620]
                r[b+0xb80:b+0xba0]=previous[b+0xb20:b+0xb40]
                r[b+0xb20:b+0xb40]=r[b+0x15c0:b+0x15e0]
                following.append(bytes(r))
            branches[kind]=following
    guard_results=guards(out,a.profile,current[1],full)
    report=dict(profile=a.profile,guards=len(guard_results),calls=sum(r['calls'] for r in reports),carried_calls=sum(r['calls'] for r in carried),record_bytes=sum(r['record_bytes'] for r in reports+carried),reports=reports,carried=carried,
        full_mechanics_sha256=digest(full),scope=('Whole original9B2790, actual caller scale recovery, wholeA00E10 then post-grip controls/forces in two composed original processes vs one Rust call. Source geometry/materials; earlier controls/history and material-table binding supplied; no wholeA03670/world/driving claim.' if MAGIC==b'UF01' else 'Whole original A00E10 then authored actual post-grip interval using unchanged helpers in two isolated processes; Rust one call. Source contact geometry/materials, supplied earlier force vectors/history schedule; no whole A03670/world/driving claim.'))
    if POST_FORCE:
        report['scope']='Whole original9B2790/scale recovery/A00E10/post-grip forces then actual post-force interval through whole9BF230 in three composed original adapters vs one Rust call. Source geometry/materials; earlier controls/history/table binding supplied; following9BE1E0, wholeA03670/world/driving open.'
        combined=collections.Counter()
        for item in reports+carried:combined.update(item['coverage'])
        assert all(combined[k]>0 for k in ['speed_refresh','rpm_history','post_angular','camber','wheel_response'])
        report['coverage']=dict(combined)
    if POST_CHASSIS:
        assert all(report['coverage'][k]>0 for k in ['chassis_matrix','chassis_acceleration','chassis_phases'])
        report['scope']='Whole original9B2790/scale/A00E10/post-grip force/post-force/9BE1E0 in four composed original adapters vs one Rust call. Source geometry/materials; earlier controls/history/table binding supplied; physical pose unchanged by chassis stage; wholeA03670/world/driving open.'
    if DRIVER_PREPARE:
        assert report['coverage']['driver_velocity']>0 and report['coverage']['driver_history']>0
        report['scope']='Selected engaged assisted-automatic earlyA03670 preparation, source vector/contact/suspension/grip, controls/forces/assists/chassis in five composed original adapters vs one Rust call. Gravity and full source mechanics supplied; early respawn/nitro inactive; no wholeA03670/world/driving claim.'
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['reports','carried']}))

if __name__=='__main__':main()
