"""Source-only startup, retained original world and independent driving semantics.

No captured car supplies startup. Native comparison uses the Rust source-built
initial owner, with explicit owned pointer tokens and authored serial settings.
This proves the update contract; source construction stages have separate proofs.
"""
import argparse, csv, json, math, struct
from pathlib import Path
import validate_vehicle_world as vw
import validate_handling_configuration as v

b = vw.b
NATIVE = b.ROOT / 'local/behavior/source-vehicle-world-probe.exe'

def wire(initial, throttle, steering, ticks):
    assert initial[:4] == b'DV01' and len(initial) == 4 + 0x21000 + 0x424 + 0x2b0 + 0xdd0
    car = initial[4:4+0x21000]
    physical = initial[4+0x21000:4+0x21000+0x424]
    view = initial[4+0x21000+0x424:4+0x21000+0x424+0x2b0]
    solver = initial[-0xdd0:]
    row = bytearray(vw.RECORD)
    struct.pack_into('<IfIII', row, 0, 0, .016, 0, 0, 0x2b0)
    row[b.BODY:b.BODY+0x2600] = car[:0x2600]
    row[b.RECORD:b.RECORD+0x21000-0x2600] = car[0x2600:]
    row[vw.PO:vw.PO+0x424] = physical
    row[b.VIEW:b.VIEW+0x2b0] = view
    row[b.VIEW+0x300:b.VIEW+0x300+0xdd0] = solver
    row[b.VIEW+0x2b0:b.VIEW+0x300] = solver[:0x50]
    struct.pack_into('<I', row, b.VIEW, 0x2b0)
    struct.pack_into('<f', row, b.RECORD-36, 8000.)
    for o in [0x328,0x32c,0x334,0x338,0x33c,0x340]:
        struct.pack_into('<I', row, vw.PO+o, 0)
    struct.pack_into('<f', row, b.BODY+0xe1c, throttle)
    struct.pack_into('<f', row, b.BODY+0xe30, steering)
    struct.pack_into('<I', row, vw.w.m.RECORD, 4)
    struct.pack_into('<I11fI4f13f', row, vw.TAIL, ticks, .016, 0., -9.8100004196167, 0., 0., .01, .2, .125, .8, .2, .2, 4, .01, 1., 0., .8, 0., .3, .4, .01, .5, .2, 2., .5, 3., .75, 20., 0., .125)
    struct.pack_into('<2I', row, vw.TAIL+3192, 0xffffffff, 0xffffffff)
    row[vw.TAIL+3268:vw.TAIL+3272] = physical[0x3f4:0x3f8]
    row[vw.TAIL+3272:vw.TAIL+3336] = physical[0x150:0x190]
    return row

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((b.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    full=b.grip.mechanics_fixture(out)
    rust=b.ROOT/f'target/{a.profile}/grid-vehicle-force.exe';drive=b.ROOT/f'target/{a.profile}/grid-drive.exe'
    reports=[];finals={};rows=[]
    for name,t,s,ticks in [('rest',0.,0.,1200),('power',1.,0.,600),('left',1.,.2,160),('right',1.,-.2,160)]:
        telemetry=out/f'{name}.tsv';run=v.run(drive,[b.TRACK.parents[4],telemetry,ticks,t,s]);assert run['exit']==0 and not run['stderr'],run
        source=out/f'{name}-input.bin';initial=telemetry.with_suffix('.state.bin').read_bytes();row=wire(initial,t,s,ticks);source.write_bytes(b.request([row],b'TF04'));rows.append(row)
        executions=[dict(kind='source-runtime',**run)];answers=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{name}-{kind}.bin';exe=rust if kind=='rust' else NATIVE
            args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full]
            run=v.run(exe,args);executions.append(dict(kind=kind,**run));(out/'current-executions.json').write_text(json.dumps(executions,indent=2));assert run['exit']==0 and not run['stderr'],run
            if kind!='rust':assert 'code_unchanged=1 driver_globals_restored=1' in run['stdout'] and f'vehicle_world_steps={ticks}' in run['stdout']
            answers.append(target.read_bytes()[8:])
        assert all(answers[0][x:y]==answers[1][x:y]==answers[2][x:y] for x,y in [(0,b.VIEW),(b.MATERIAL,vw.RECORD)]),name
        final=telemetry.with_suffix('.final.bin').read_bytes()[4:];car=answers[0][b.BODY:b.BODY+0x2600]+answers[0][b.RECORD:b.RECORD+0x21000-0x2600]
        assert final[:0x21000]==car,(name,'runtime complete car')
        physical=bytearray(final[0x21000:0x21000+0x424]);struct.pack_into('<I',physical,0x334,0)
        assert physical==answers[0][vw.PO:vw.PO+0x424],(name,'runtime complete body')
        runtime_view=bytearray(final[0x21000+0x424:0x21000+0x424+0x2b0]);assert struct.unpack_from('<I',runtime_view)[0]==0
        struct.pack_into('<I',runtime_view,0,0x2b0)
        assert runtime_view==answers[2][b.VIEW:b.VIEW+0x2b0]
        assert final[-0xdd0:]==answers[2][b.VIEW+0x300:b.VIEW+0x300+0xdd0]
        with telemetry.open() as f:data=list(csv.DictReader(f,delimiter='\t'))
        assert len(data)==ticks+1 and all(math.isfinite(float(r[k])) for r in data for k in ['x','y','z','speed','engine_speed'])
        assert all(int(r['wheel_hits'])==4 for r in data[32:]),name
        start=[float(data[0][k]) for k in ['x','y','z']];end=[float(data[-1][k]) for k in ['x','y','z']];distance=math.dist(start,end)
        assert distance < .02 if name=='rest' else distance>5
        finals[name]=struct.unpack_from('<16f',physical,0x1d0)
        reports.append(dict(name=name,ticks=ticks,displacement=distance,observable_bytes=b.VIEW+vw.RECORD-b.MATERIAL,executions=executions,initial_sha256=b.digest(telemetry.with_suffix('.state.bin')),native_sha256=b.digest(out/f'{name}-original.bin'),runtime_sha256=b.digest(telemetry.with_suffix('.final.bin'))))
    # Opposite inputs must turn the source car to distinct sides of straight.
    cross=lambda p:finals['power'][8]*p[10]-finals['power'][10]*p[8]
    assert cross(finals['left'])*cross(finals['right'])<0
    guards=[]
    for name,o,x in [('zero_steps',vw.TAIL,0),('step_bound',vw.TAIL,4097),('missing_root',vw.PO+8,0),('missing_active',vw.PO+0x2c,0),('missing_group',vw.PO+0x20,0),('foreign_center',vw.PO+0x334,1),('delta_mismatch',vw.TAIL+4,0x3f000000),('nan_gravity',vw.TAIL+12,0x7fc00000)]:
        row=bytearray(rows[0]);struct.pack_into('<I',row,o,x)
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(b.request([row],b'TF04'));exe=rust if kind=='rust' else NATIVE;args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full];run=v.run(exe,args)
            passed=(run['exit']==1 if kind=='rust' else 0<run['exit']<80 and 'code_unchanged=1 driver_globals_restored=1' in run['stdout']) and not target.exists();guards.append(dict(name=name,kind=kind,passed=passed,execution=run));assert passed,guards[-1]
    # Old context inertia is overwritten by 9F1E10 before it can be consumed.
    for i,x in enumerate([0,0x7fc01234,0x7f800000,0xff800000]):
        row=bytearray(rows[0]);struct.pack_into('<I',row,vw.TAIL,1)
        for o in [0xac0,0xac4,0xac8]:struct.pack_into('<I',row,b.BODY+o,x)
        source=out/f'cache-{i}-input.bin';source.write_bytes(b.request([row],b'TF04'));results=[]
        for kind in ['original','rust']:
            target=out/f'cache-{i}-{kind}.bin';exe=rust if kind=='rust' else NATIVE;args=[b.TRACK,b.SURFACE,source,target] if kind=='rust' else [b.REFERENCE,b.WORLD,source,target,b.MECHANICS,full];run=v.run(exe,args);assert run['exit']==0,run;results.append(target.read_bytes())
        assert all(results[0][8+x:8+y]==results[1][8+x:8+y] for x,y in [(0,b.VIEW),(b.MATERIAL,vw.RECORD)])
    result=dict(passed=True,profile=a.profile,cases=4,world_calls=sum(r['ticks'] for r in reports),guards=guards,overwritten_cache_controls=4,reports=reports,executables={str(q.relative_to(b.ROOT)):b.digest(q) for q in [NATIVE,rust,drive]})
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps(dict(passed=True,profile=a.profile,cases=4,world_calls=result['world_calls'],guards=len(guards))))
if __name__=='__main__':main()
