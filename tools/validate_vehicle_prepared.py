"""Actual post-grip interval into whole control and selected force processing."""
import struct
import subprocess
import json
import validate_vehicle_controls as controls
import validate_vehicle_propulsion as base

control_cases=controls.cases
control_guards=controls.guards


def cases():
    result=[]
    for i,raw in enumerate(control_cases()):
        b=bytearray(raw)
        def number(o,x):struct.pack_into('<f',b,36+o,x)
        for o,x in [(0x2198,1.1),(0x219c,1.1),(0x21a0,1.2),(0x21a4,1.3),(0x21a8,1.4),(0x21ac,1.5),(0x21b0,1.6),
                    (0x21b4,-.1),(0x21b8,.4),(0x21bc,1.2),(0x21c0,.9),(0x1f6c,[0,.25,1,-.1,1.1][i%5]),(0x1f7c,[0,.01,1,2,2.1][i%5])]:number(o,x)
        for j in range(4):
            number(0x22b4+j*4,[0,.2,-.1][(i+j)%3])
            m=36+0x2600+0x300+0xdd0+j*0x140
            for o,x in [(0x10,[0,.5,10][(i+j)%3]),(0x14,2),(0x18,30),(0x1c,[0,.5,2][(i+j)%3])]:struct.pack_into('<f',b,m+o,x)
            b[m+0x73]=int(i%4==0)
        if i%41==0:
            for o in range(0x15c0,0x15e0,4):number(o,0)
            number(0x1648,0)
            for j in range(4):
                number(0x22b4+j*4,0)
                for o in [0x10,0x1c]:struct.pack_into('<f',b,36+0x2600+0x300+0xdd0+j*0x140+o,0)
        result.append(bytes(b))
    return result


def guards(out,profile,first):
    controls.cases=cases
    # Extend the existing controls guard fixture, retaining its valid shared aliases.
    old=controls.original_guards
    def extended(out,profile,record,extra):
        body=36;material=36+0x2600+0x300+0xdd0
        record=bytearray(record)
        # The overflow control must activate roughness before testing its conversion bound.
        struct.pack_into('<f',record,body+0x1648,40)
        record=bytes(record)
        extra=list(extra)+[('zero_mass',body+0x15a0,0,50),('zero_inertia',body+0x15a8,0,50),
            ('zero_radius',body+0x1990+0xe0,0,50),('inertia_intervals',body+0x21b8,struct.unpack_from('<I',record,body+0x21b4)[0],50),
            ('equal_drag_speeds',material+0x18,struct.unpack_from('<I',record,material+0x14)[0],50),
            ('roughness_overflow',material+0x10,0x7f7fffff,50)]
        for o in [0x1f6c,0x1f7c,*range(0x2198,0x21c4,4),0x22b4]:extra.append((f'prelude_nan_{o:x}',body+o,0x7fc00000,50))
        for o in [0x10,0x14,0x18,0x1c]:extra.append((f'material_nan_{o:x}',material+o,0x7fc00000,50))
        results=old(out,profile,record,extra)
        b=bytearray(record)
        for o in range(0x15c0,0x15e0,4):struct.pack_into('<f',b,body+o,0)
        for j in range(4):struct.pack_into('<f',b,material+j*0x140+0x1c,1)
        source=out/'guard-zero-point-velocity.bin';source.write_bytes(base.request([bytes(b)]))
        for kind in ['native','rust']:
            target=out/f'guard-zero-point-velocity-{kind}-out.bin';exe=base.NATIVE if kind=='native' else base.ROOT/f'target/{profile}/grid-vehicle-force.exe'
            args=[base.REFERENCE,source,target] if kind=='native' else [source,target]
            run=subprocess.run([str(exe),*map(str,args)],cwd=base.ROOT,capture_output=True,text=True,timeout=30)
            passed=run.returncode==(51 if kind=='native' else 1) and not target.exists()
            if kind=='native':passed=passed and 'completed=0 code_unchanged=1' in run.stdout and not run.stderr
            results.append(dict(name='zero_point_velocity',kind=kind,passed=passed,exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        (out/'guards.json').write_text(json.dumps(results,indent=2))
        assert all(r['passed'] for r in results),results[-2:]
        return results
    controls.original_guards=extended
    try:return control_guards(out,profile,first)
    finally:controls.original_guards=old


if __name__=='__main__':
    base.MAGIC=b'GF01';base.NATIVE=base.ROOT/'local/behavior/vehicle-prepared-probe.exe';base.cases=cases;base.guards=guards
    base.main()
