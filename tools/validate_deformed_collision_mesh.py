"""Whole active source adjustment/sampler/mesh construction on supplied grids."""
import argparse
import csv
import json
import math
import struct
from pathlib import Path
import validate_collision_mesh as mesh
import validate_deformation_layout as layout
from validate_binxml import decode
from validate_wheel_geometry import layout as wheel_layout
from compare_source_deformation_layout import STEAM,CAPTURES
from summarize_deformation_capture import load

ROOT=layout.ROOT
PROBE=ROOT/'local/behavior/deformed-collision-mesh-probe.exe'


def cases():
    rows=[];identities={str(layout.IMAGE):layout.digest(layout.IMAGE)}
    baseline=ROOT/'local/behavior/deformation-source-debug-20261013-final-01'
    for stem in ['lac','bmw']:
        ned=STEAM/f'cars/{stem}/{stem}.ned';csvpath=ned.with_suffix('.csv')
        tree=decode(ned.read_bytes())[1];collision=[e for e in tree.iter('COLLISION')];assert len(collision)==1
        geometry=[e for e in collision[0] if e.tag=='GEOMETRY' and e.get('id')=='collision_main'];assert len(geometry)==1
        points=[[*map(layout.f32,map(float,e.get('pos').split())),1.] for e in geometry[0] if e.tag=='POINT']
        faces=[list(map(int,e.get('points').split())) for e in geometry[0] if e.tag=='TRIANGLE']
        wheels=list(tree.iter('WHEELS'));assert len(wheels)==1
        csvrows=[r for r in csv.reader(csvpath.read_text().splitlines()) if r]
        parameters=wheel_layout(wheels[0].attrib,csvrows[0])[2]+[min(p[1] for p in points),max(p[1] for p in points)]
        path=baseline/f'{stem}-rust.bin';report=json.loads((baseline/'validation.json').read_text())
        assert layout.digest(path)==report['files'][path.name]['sha256']
        identities.update({str(p):layout.digest(p) for p in [ned,csvpath,path]})
        base=path.read_bytes()[44+2*layout.SIZE:44+3*layout.SIZE]
        dims=struct.unpack_from('<3I',base,0xa414);count=math.prod(dims)
        for variant in range(16):
            grid=bytearray(base)
            for i in range(count):
                at=0x2580+i*80;x,y,z,w=struct.unpack_from('<4f',grid,at)
                if variant==1:x,y,z=layout.f32(x+.03),layout.f32(y-.02),layout.f32(z+.05)
                elif 2<=variant<8:x,y,z=layout.f32(x*(.88+variant*.02)),layout.f32(y*(1.-variant*.01)),layout.f32(z*.95)
                elif variant>=8:
                    nose=max(z,0.);x=layout.f32(x+nose*.002*(variant-7));y=layout.f32(y+nose*.003*(variant-7));z=layout.f32(z-nose*.005*(variant-7));w=layout.f32(w-(variant-7)*.002)
                struct.pack_into('<4f',grid,at,x,y,z,w)
            for fill in [0,0xcd]:rows.append(dict(name=f'{stem}-{variant}-{fill}',points=points,faces=faces,parameters=parameters,grid=bytes(grid),fill=fill))
        if stem=='lac':
            for name,sha in CAPTURES.items():
                capture=ROOT/'local/behavior'/name;assert layout.digest(capture)==sha;identities[str(capture)]=sha
                _,blocks,_=load(capture);grid=bytearray(base);descriptor=blocks['car_0-deformation_grid_descriptor']
                grid[0xa414:0xa424]=descriptor[:16];grid[0xa430:0xa440]=descriptor[28:44]
                grid[0x2580:0x2580+32000]=blocks['car_0-deformation_grid_nodes']+blocks['car_0-deformation_grid_padding_nodes']
                for fill in [0,0xcd]:rows.append(dict(name=name+'-'+str(fill),points=points,faces=faces,parameters=parameters,grid=bytes(grid),fill=fill))
    return rows,identities


def request(row,shape=False):
    raw=mesh.request(row['points'],row['faces'],row['fill'],row['parameters'])
    shape_bytes=b''
    if shape:
        p,t=len(row['points']),len(row['faces']);span=0x60+p*48+t*32+((p*40+15)&~15)+((t*12+15)&~15)
        shape_bytes=struct.pack('<IfII',span,0.,0,0)
    return (b'CM07' if shape else b'CM06')+raw[4:40]+row['grid']+shape_bytes+raw[40:]


def guards(out,row,shape,rust,probe):
    valid=request(row,shape);header=40+layout.SIZE+(16 if shape else 0)
    mutations=[('point_count',4,81,6),('bad_fill',12,1,6),('offset_w',28,0x3f800000,22),('nan_source',header,0x7fc00000,7),
               ('dimension_one',40+0xa414,1,36),('dimension_large',40+0xa414,17,36),('nan_spacing',40+0xa420,0x7fc00000,36),('nan_minimum',40+0xa430,0x7fc00000,36),('nan_node',40+0x2580,0x7fc00000,36),('triangle_index',header+len(row['points'])*16,999,8)]
    if shape:mutations.extend([('zero_span',40+layout.SIZE,0,25),('nan_shape',44+layout.SIZE,0x7fc00000,25),('shape_flag',52+layout.SIZE,2,25)])
    tests=[('truncated',valid[:-1],6),('trailing',valid+b'x',6)]
    for name,at,value,error in mutations:
        raw=bytearray(valid);struct.pack_into('<I',raw,at,value);tests.append((name,bytes(raw),error))
    checks=[]
    for name,raw,error in tests:
        source=out/f'guard-{name}-input.bin';source.write_bytes(raw);runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'guard-{name}-{kind}.bin';exe=probe if kind!='rust' else rust
            run=mesh.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,output_exists=target.exists())
            assert run['exit']==(1 if kind=='rust' else error) and not target.exists(),(name,kind,runs[kind])
        checks.append(dict(name=name,expected_original_exit=error,executions=runs))
    for kind in ['original','rust']:
        source=out/f'existing-{kind}-input.bin';source.write_bytes(valid);target=out/f'existing-{kind}.bin';target.write_bytes(b'owned sentinel')
        exe=probe if kind=='original' else rust;run=mesh.run(exe,([layout.IMAGE] if kind=='original' else [])+[source,target])
        assert run['exit']==(19 if kind=='original' else 1) and target.read_bytes()==b'owned sentinel'
        checks.append(dict(name='existing-'+kind,preserved=True,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));return checks


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],required=True);parser.add_argument('--shape',action='store_true')
    args=parser.parse_args();out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows,identities=cases();reports=[];total=0
    rust=ROOT/f'target/{args.profile}'/('grid-mesh-shape.exe' if args.shape else 'grid-collision-mesh.exe')
    probe=ROOT/'local/behavior/deformed-mesh-shape-probe.exe' if args.shape else PROBE
    for i,row in enumerate(rows):
        source=out/f'{i}-input.bin';source.write_bytes(request(row,args.shape));answers={};runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'{i}-{kind}.bin';exe=probe if kind!='rust' else rust
            run=mesh.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe))
            (out/f'{i}-execution.json').write_text(json.dumps(runs,indent=2));assert run['exit']==0 and not run['stderr'],(row['name'],run)
            if kind!='rust':assert 'code_unchanged=1 code_replacements=0 globals_restored=1 error=0' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];diffs=[dict(offset=hex(i),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(0,len(a),4) if a[i:i+4]!=b[i:i+4]]
        item=dict(name=row['name'],bytes=len(a),differences=diffs,repeat_exact=a==answers['repeat'],executions=runs)
        (out/f'{i}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert a==answers['repeat'] and not diffs,(row['name'],diffs[:10]);total+=len(a)
    guard_report=guards(out,rows[0],args.shape,rust,probe)
    report=dict(profile=args.profile,cases=len(rows),bytes=total,shape=args.shape,identities=identities,reports=reports,guards=guard_report,
                files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                scope='Whole unchanged9B8B70->9B8890->9A39D0 active source preparation then B44E80 mesh on supplied source/retained/authored grids; complete mesh compared. No dirty factory/registration/runtime deformation integration.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','bytes']}))


if __name__=='__main__':main()
