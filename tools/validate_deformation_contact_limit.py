"""Whole original static-peer9A0C90 and complete independent Rust buffers."""
import argparse
import collections
import csv
import json
import math
import random
import struct
from pathlib import Path
import validate_deformation_layout as layout
import validate_collision_mesh as execution
from inspect_installation import inspect_pe
from compare_source_deformation_layout import STEAM
from validate_deformation_volume import guard_checks

ROOT=layout.ROOT
LINKED=0x424
ROW=layout.SIZE+LINKED+128+16+96
ANSWER=layout.SIZE+LINKED+128+8
PROBE=ROOT/'local/behavior/deformation-contact-limit-probe.exe'


def parameters():
    assert layout.digest(layout.IMAGE)=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    raw=layout.IMAGE.read_bytes();pe=inspect_pe(layout.IMAGE)
    def f(address):
        s=next(s for s in pe['sections'] if 0<=address-0x400000-s['rva']<=s['raw_size']-4)
        return struct.unpack_from('<f',raw,s['raw_offset']+address-0x400000-s['rva'])[0]
    return [f(a) for a in [0xcafbdc,0xcb093c,0xe4fc2c,0xe4fc3c,0xcb3ba8,0xe51d40,0xe4fc30]]+[f(0xe4fc48+i*4) for i in range(16)]


def cases():
    rows=[];identities={str(layout.IMAGE):layout.digest(layout.IMAGE)};p=parameters();rng=random.Random(2008258)
    source=ROOT/'local/behavior/deformation-source-debug-20261013-final-01';report=json.loads((source/'validation.json').read_text())
    for stem in ['lac','bmw']:
        path=source/f'{stem}-rust.bin';assert layout.digest(path)==report['files'][path.name]['sha256'];identities[str(path)]=layout.digest(path)
        base=path.read_bytes()[44+2*layout.SIZE:44+3*layout.SIZE];dims=struct.unpack_from('<3I',base,0xa414);count=math.prod(dims)
        csvpath=STEAM/f'cars/{stem}/{stem}.csv';csvrows=[r for r in csv.reader(csvpath.read_text().splitlines()) if r];mass=layout.f32(float(csvrows[0][3]));identities[str(csvpath)]=layout.digest(csvpath)
        for index in range(128):
            d=bytearray(base);b=bytearray(LINKED);c=bytearray(rng.randbytes(128));pp=list(p)
            struct.pack_into('<I',c,4,0);struct.pack_into('<I',c,8,0);c[12]=1
            spacing=struct.unpack_from('<f',d,0xa420)[0];minimum=struct.unpack_from('<4f',d,0xa430)
            point=[layout.f32(minimum[a]+spacing*((dims[a]-1)*[0.,.25,.5,.75,1.25][(index+a)%5])) for a in range(3)]+[1.]
            angle=(index%4)*math.pi*.25;pose=[layout.f32(math.cos(angle)),0.,layout.f32(-math.sin(angle)),0.,0.,1.,0.,0.,layout.f32(math.sin(angle)),0.,layout.f32(math.cos(angle)),0.,.25*(index%3),-.125*(index%2),.2*(index%4),1.]
            struct.pack_into('<16f',b,0x1d0,*pose)
            struct.pack_into('<4f',b,0x210,*[layout.f32(rng.uniform(-.5,.5)) for _ in range(3)],0.)
            speed=[0.,.1,1.,3.,5.,10.,25.,60.][index%8]
            struct.pack_into('<4f',b,0x220,speed,layout.f32(speed*.25),layout.f32(-speed*.5),0.)
            struct.pack_into('<4f',b,0x360,0.2*(index%3),-.1,0.3,0.)
            struct.pack_into('<f',b,0x3f0,[mass,20.,100.][index%3]);center=pose[12:16]
            normal=[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[-.6,0.,.8,0.]][index%4]
            struct.pack_into('<4f',c,0x50,*point);struct.pack_into('<4f',c,0x60,*normal);struct.pack_into('<f',c,0x38,[0.,.01,.25,1.][index%4])
            struct.pack_into('<I',d,0xa4ec,index%10)
            for i in range(10):struct.pack_into('<f',d,0xa4c4+i*4,[0.,.25,1.,4.][(index//16+i)%4] if index>=32 else 0.)
            if 32<=index<48:
                for i in range(4):struct.pack_into('<f',d,0xa4b4+i*4,.2*(i+1))
            if 48<=index<64:
                for i in range(count):struct.pack_into('<f',d,0x25c0+i*80,0. if index%2 else -1.)
            if 64<=index<80:
                for i in range(count):
                    at=0x2580+i*80;x,y,z,w=struct.unpack_from('<4f',d,at);struct.pack_into('<4f',d,at,layout.f32(x+.02*y),y,layout.f32(z-.02*x),layout.f32(w-.001*(i%3)))
            if 80<=index<96:
                struct.pack_into('<f',d,0xa490,[.1,1.,100.][index%3]);struct.pack_into('<f',d,0xa494,7.);struct.pack_into('<f',d,0xa49c,2.)
            if index>=96:
                d[0xa4b0]=0 if index%3==0 else 1;d[0xa4af]=int(index%3==1)
            rows.append(dict(name=f'{stem}-{index}',descriptor=bytes(d),body=bytes(b),contact=bytes(c),center=center,parameters=pp,disabled=int(index>=96 and index%3==2)))
    return rows,identities


def request(rows):
    data=bytearray(b'DC01'+struct.pack('<I',len(rows)))
    for row in rows:data.extend(row['descriptor']);data.extend(row['body']);data.extend(row['contact']);data.extend(struct.pack('<4f',*row['center']));data.extend(struct.pack('<23fI',*row['parameters'],row['disabled']))
    assert len(data)==8+len(rows)*ROW;return bytes(data)


def mutations():
    base=8+layout.SIZE;contact_at=base+LINKED;param_at=contact_at+144
    return [('dimension',8+0xa414,17),('ring',8+0xa4ec,10),('nan_window',8+0xa4c4,0x7fc00000),('nan_marker',8+0x25c0,0x7fc00000),('nan_pose',base+0x1d0,0x7fc00000),('nan_response_velocity',base+0x360,0x7fc00000),('foreign_center',base+0x334,1),('zero_mass',base+0x3f0,0),('invalid_static_flag',contact_at+12,2),('foreign_owner',contact_at+4,1),('nan_point',contact_at+0x50,0x7fc00000),('nan_parameter',param_at,0x7fc00000),('bad_unlimited',param_at+20,0),('bad_disable',param_at+92,2)]


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,identities=cases()
    if args.pilot:rows=rows[:32]
    reports=[];coverage=collections.Counter();rust=ROOT/f'target/{args.profile}/grid-deformation-contact-limit.exe'
    for start in range(0,len(rows),32):
        batch=rows[start:start+32];source=out/f'{start}-input.bin';source.write_bytes(request(batch));answers={};runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'{start}-{kind}.bin';exe=PROBE if kind!='rust' else rust
            run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run['exit']==0 and not run['stderr'],runs[kind]
            if kind!='rust':assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+len(batch)*ANSWER and len(b)==len(a)
        diffs=[dict(case=batch[i//ANSWER]['name'],offset=hex(i%ANSWER),original=a[8+i:12+i].hex(),rust=b[8+i:12+i].hex()) for i in range(0,len(a)-8,4) if a[8+i:12+i]!=b[8+i:12+i]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,repeat_exact=a==answers['repeat'],differences=diffs,executions=runs);(out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert a==answers['repeat'] and not diffs,diffs[:12]
        for i,row in enumerate(batch):
            result=a[8+i*ANSWER:8+(i+1)*ANSWER];cap=struct.unpack_from('<d',result,layout.SIZE+LINKED+128)[0]
            assert math.isfinite(cap)
            assert result[layout.SIZE:layout.SIZE+LINKED]==row['body']
            coverage['finite_restricted_caps']+=cap!=row['parameters'][5]
            coverage['window_changed']+=result[0xa4c4:0xa4ec]!=row['descriptor'][0xa4c4:0xa4ec]
            coverage['lookup_state_changed']+=result[0xa490:0xa49c]!=row['descriptor'][0xa490:0xa49c]
            coverage['coefficient_changed']+=result[layout.SIZE+LINKED+0x40:layout.SIZE+LINKED+0x44]!=row['contact'][0x40:0x44]
    guards=guard_checks(out,request([rows[0]]),mutations(),args.profile,'grid-deformation-contact-limit',PROBE,7)
    result=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),identities=identities,reports=reports,guards=guards,coverage=dict(coverage),
                files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                scope='Whole static-first-owner9A0C90, supplied source grids and authored body/contact state; no counterpart/wake graph, preparation manager, full physical group or live cadence.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ['profile','cases','bytes']}))


if __name__=='__main__':main()
