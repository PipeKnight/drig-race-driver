"""One unchanged registered A0A290 car job versus the independent Rust owned vehicle call."""
import argparse
import collections
import json
import struct
from pathlib import Path
import validate_vehicle_outer_publication as composed
import validate_whole_vehicle_step as step
import validate_vehicle_gearing as gearing
import vehicle_source_gearing as sources
b=step.b
EXTRA=0x21000-0x2600
RECORD=b.RECORD+EXTRA+0x424
NATIVE=b.ROOT/'local/behavior/car-job-probe.exe'


def compare(out,profile,branches,prefix,full):
    outputs={};runs=[]
    for kind,rows in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin'
        source.write_bytes(b.request(rows,b'KF01'))
        exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
        args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
        run=b.execute(exe,args,out,f'{prefix}-{kind}');runs.append(run)
        if kind!='rust':
            assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout']
            assert f'car_job_completions={len(rows)} owned_completion_identity=1' in run['stdout']
        outputs[kind]=[target.read_bytes()[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(rows))]
        assert target.stat().st_size==8+len(rows)*RECORD
    differences=[];coverage=collections.Counter();ranges=[(0,b.VIEW),(b.MATERIAL,RECORD)]
    for i,previous in enumerate(branches['original']):
        a,repeat,rust=[outputs[k][i] for k in ['original','repeat','rust']]
        for start,end in ranges:
            assert a[start:end]==repeat[start:end]
            differences.extend(dict(case=i,offset=hex(o),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(start,end,4) if a[o:o+4]!=rust[o:o+4])
        for name,o,n in [('velocity',b.BODY+0x15c0,32),('pose',b.BODY+0x15e0,64),('driver_flag_written',b.BODY+0x9a0,1),('mode_transition',b.BODY+0x1598,4),('engine_speed',b.BODY+0x1894,4),('physical_pose',b.RECORD+EXTRA+0x190,128),('physical_mass',b.RECORD+EXTRA+0x3f0,8),('timer',b.RECORD+0xc798-0x2600,4),('decay',b.RECORD+0xc790-0x2600,4)]:coverage[name]+=previous[o:o+n]!=a[o:o+n]
        expected_flag=1 if struct.unpack_from('<f',previous,4)[0]>0 else previous[b.BODY+0x9a0]
        assert a[b.BODY+0x9a0]==expected_flag, 'Job applied an extra synchronous-only clear'
        po=b.RECORD+EXTRA
        assert a[po+0x190:po+0x210]==previous[po+0x190:po+0x210], 'Job performed unsupported physical pose publication'
        for j in range(4):assert a[po+0x210+j*4:po+0x214+j*4]==a[b.BODY+0x15d0+j*4:b.BODY+0x15d4+j*4]
        for j in range(4):assert a[po+0x220+j*4:po+0x224+j*4]==a[b.BODY+0x15c0+j*4:b.BODY+0x15c4+j*4]
        for name,at,n in [('handling_levels',0x18,16),('handling_effects',0x3a0,0x88),('handling_gear_history',0x428,8)]:
            off=b.RECORD+0xcdf0+at-0x2600;coverage[name]+=previous[off:off+n]!=a[off:off+n]
        coverage['zero_delta']+=struct.unpack_from('<f',previous,4)[0]==0
        coverage['mode_two']+=struct.unpack_from('<I',a,b.BODY+0x1598)[0]==2
        for j in range(4):
            coverage['contacts']+=struct.unpack_from('<I',a,20+j*4)[0]!=0
            w=b.BODY+0x1990+j*0x150+0x104;coverage['spin']+=previous[w:w+4]!=a[w:w+4]
    report=dict(calls=len(branches['original']),owner_bytes=(0x21000+0x424)*len(branches['original']),observable_bytes=sum(e-s for s,e in ranges)*len(branches['original']),repeat_exact=True,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert not differences,dict(count=len(differences),first=differences[:30]);return report,outputs


def extend(row,index):
    o=bytearray(composed.owner(row,index));m=0xcdf0
    # Authored handling state; preserve the existing validated sync settings.
    settings=bytes(o[m+0x3a0:m+0x428]);o[m:m+0x460]=bytes(0x460);o[m+0x3a0:m+0x428]=settings
    for at,x in [(0x48,1),(0x4c,1),(0x254,.65),(0x264,.01),(0x268,.01),(0x270,.1),(0x274,.65),(0x278,.95),(0x27c,.1),(0x280,27),(0x284,.45),(0x2ec,.01),(0x2f0,999999),(0x2f4,999999),(0x2f8,.65),(0x310,.1),(0x314,.65),(0x318,.01),(0x31c,.01),(0x320,.01),(0x324,.65),(0x328,.65),(0x338,.01),(0x33c,.65),(0x354,.65),(0x35c,.01),(0x368,10000),(0x430,1),(0x434,1),(0x438,1)]:b.put(o,m+at,x)
    for wheel in range(4):b.put(o,m+0x18+wheel*4,.01*((index+wheel)%7))
    b.put(o,m+0x224,.01);b.put(o,m+0x228,.65);b.put(o,m+0x238,.01);b.put(o,m+0x23c,.65)
    if struct.unpack_from('<f',row,4)[0]==0:
        # The older zero-driver fixtures left these unread slots arbitrary.
        # Handling now consumes them even when the driver skips its update.
        for wheel in range(4):
            b.put(o,0x1aa8+wheel*0x150,0);b.put(o,0x1ac8+wheel*0x150,0)
    o[0xb13]=o[0xb14]=1;struct.pack_into('<I',o,m+0x444,7);struct.pack_into('<I',o,m+0x428,(index+1)%5)
    struct.pack_into('<II',o,0x2118,index%2,(index//2)%3);struct.pack_into('<I',o,0x22e4,0)
    r=bytearray(row[:b.BODY]+o[:0x2600]+row[b.VIEW:]+o[0x2600:]);r[b.BODY+0x9a0]=0x7f
    return bytes(r)


def guards(out,profile,first,full):
    mutations=[('nan_delta',4,0x7fc00000),('negative_delta',4,0xbf800000),('large_delta',4,0x3e800000),('tiny_delta',4,1)]
    mutations += [(n,b.BODY+o,v) for n,o,v in [('foreign_wrapper',0xdb0,1),('wheel_count',0x1620,5),('render',0xdb8,1),('setup_rebuild',0xc7f0,0x10000),('ring_index',0xc7ec,10),('deformation',0xc714,1),('zero_mass',0xfa8,0),('bad_inertia',0xfac,0x7fc00000),('nan_timer',0xc798,0x7fc00000),('nan_settings',0xd198,0x7fc00000)]]
    # Tail storage follows the full DriverRecord, not body2600.
    mutations=[(n,b.RECORD+o-b.BODY-0x2600 if o>=b.BODY+0x2600 else o,v) for n,o,v in mutations]
    mutations += [('foreign_physical',b.RECORD+EXTRA+0x328,1),('reset_group',b.RECORD+EXTRA+0x400,0xffffffff),('nan_authority',b.RECORD+EXTRA+0x220,0x7fc00000),('nan_center_offset',b.RECORD+EXTRA+0x230,0x7fc00000)]
    for name,o,v in [('foreign_mutable_component',0xcdf0+0x214,1),('nan_handling',0xcdf0+0x318,0x7fc00000),('notifications',0xcdf0+0x440,0x10000),('terminal_wheel',0x22e4,1),('style',0x2118,2),('negative_collision_threshold',0xcdf0+0x368,0xbf800000),('nan_impact',0x1ac8,0x7fc00000),('zero_wear_divisor',0xcdf0+0x2f4,0)]:
        mutations.append((name,b.BODY+o if o<0x2600 else b.RECORD+o-0x2600,v))
    checks=[]
    for name,o,v in mutations:
        row=bytearray(first);struct.pack_into('<I',row,o,v)
        if name=='nan_impact':b.put(row,4,0)
        if name=='zero_wear_divisor':b.put(row,b.RECORD+0xcdf0+0x2f0-0x2600,-1000000)
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(b.request([row],b'KF01'))
            exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
            args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
            run=b.execute(exe,args,out,f'guard-{name}-{kind}',True)
            passed=(0<run['exit']<=80 and not run['stderr'] and 'completed' not in run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']];return checks


def framing(out,profile,first,full):
    valid=b.request([first],b'KF01');foreign=bytearray(valid);struct.pack_into('<I',foreign,8,1)
    alias=bytearray(valid);struct.pack_into('<I',alias,8+b.BODY,0)
    tests=[('bad_tag',b'NOPE'+valid[4:]),('empty',b'KF01'+bytes(4)),('count65',b'KF01'+struct.pack('<I',65)),('truncated',valid[:-1]),('trailing',valid+b'!'),('case_identity',bytes(foreign)),('alias',bytes(alias))]
    checks=[]
    for name,data in tests:
        for kind in ['original','rust']:
            source=out/f'framing-{name}-{kind}-input.bin';target=out/f'framing-{name}-{kind}-output.bin';source.write_bytes(data)
            exe=b.ROOT/f'target/{profile}/grid-vehicle-force.exe' if kind=='rust' else NATIVE
            args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
            run=b.execute(exe,args,out,f'framing-{name}-{kind}',True)
            passed=(0<run['exit']<=80 and not run['stderr'] and 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'framing.json').write_text(json.dumps(checks,indent=2));assert all(c['passed'] for c in checks),[c for c in checks if not c['passed']];return checks


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    for path,sha in [(b.TRACK,'6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'),(b.SURFACE,'8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'),(b.WORLD,'b83d245aa380bb0524f45e4b9a323c44cbf536536ec6cc6722bf22765d90d8d7')]:assert b.digest(path)==sha
    tables=sources.verify_shift(out.with_name(out.name+'-source'),a.profile);records=step.cases(tables,True);full=b.grip.mechanics_fixture(out)
    reports=[];initial=[];coverage=collections.Counter();first=None
    def tick(branches,prefix):
        nonlocal first
        if first is None:first=branches['original'][0]
        r,after=compare(out,a.profile,branches,prefix,full);reports.append(r);coverage.update(r['coverage']);return after
    for start in range(0,len(records),32):
        batch=records[start:start+32];r,branches=gearing.initialize(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'init-{start}');initial.append(r);branches=step.runtime_modes(branches,True)
        tick({k:[extend(r,start+i) for i,r in enumerate(rows)] for k,rows in branches.items()},f'initial-{start}')
    zeros=step.zero_cases(records,full)
    for start in range(0,len(zeros),32):
        tick({k:[extend(r,start+i) for i,r in enumerate(zeros[start:start+32])] for k in ['original','repeat','rust']},f'zero-{start}')
    r,branches=gearing.initialize(out,a.profile,{k:records[8:24] for k in ['original','repeat','rust']},'init-carry');initial.append(r);branches=step.runtime_modes(branches,True)
    branches={k:[extend(r,8+i) for i,r in enumerate(rows)] for k,rows in branches.items()}
    for frame in range(12):
        after=tick(branches,f'carry-{frame}');following={}
        for kind,rows in after.items():
            following[kind]=[]
            for i,row in enumerate(rows):
                r=bytearray(row);old=branches[kind][i];o=b.BODY
                r[o+0xb40:o+0xb80]=old[o+0x15e0:o+0x1620];r[o+0xb80:o+0xba0]=old[o+0xb20:o+0xb40];r[o+0xb20:o+0xb40]=r[o+0x15c0:o+0x15e0]
                inactive=int(frame%4<2);r[o+0xe38]=inactive;b.put(r,o+0xe1c,0 if inactive else .5);b.put(r,o+0xe20,0)
                following[kind].append(bytes(r))
        branches=following
    checks=guards(out,a.profile,first,full);frames=framing(out,a.profile,first,full)
    assert all(coverage[k]>0 for k in ['driver_flag_written','handling_levels','handling_effects','handling_gear_history','mode_transition','mode_two','contacts','spin','zero_delta','timer','decay'])
    result=dict(profile=a.profile,calls=sum(r['calls'] for r in reports),owner_bytes=sum(r['owner_bytes'] for r in reports),observable_bytes=sum(r['observable_bytes'] for r in reports),source_files=len(tables),guards=len(checks),framing=len(frames),coverage=dict(coverage),reports=reports,initialization_reports=initial,scope='Whole registeredA0A290 car job: sync -> wholeA06A90 -> wholeA03220(delta,1) -> owned observational completion, full21000car/424physical. Source44tables, assisted automatic modes1/2, supplied mutable settings/history/controls, stable setup/emptymanager/render/UI; query0/442false/no terminal/collision below threshold. No axisprojection/reset9A0/pose setter/advance/world dispatch/live cadence/driving; expired native view/solver excluded.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if not k.endswith('reports')}))
if __name__=='__main__':main()
