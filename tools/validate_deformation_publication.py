"""Whole A03220 with supplied rebuilt state and active owned presentation."""
import argparse
import collections
import json
import random
import struct
from pathlib import Path
import validate_deformation_layout as layout
import validate_collision_mesh as execution
from validate_binxml import decode
from compare_source_deformation_layout import STEAM

ROOT=layout.ROOT;CAR=0x21000;PHYSICAL=0x424;RECORD=4+CAR+PHYSICAL
PROBE=ROOT/'local/behavior/deformation-publication-probe.exe'
RUST_BIN='grid-deformation-publication'
SCOPE='Whole A03220 with supplied post-shape/wheel-rebuild C7F2=0, owned empty graphical collection and authored light/wheel records; no A01960 or live source view construction claim.'


def cases():
    rows=[];identities={str(layout.IMAGE):layout.digest(layout.IMAGE)};rng=random.Random(2008263)
    source=ROOT/'local/behavior/deformation-source-debug-20261013-final-01';report=json.loads((source/'validation.json').read_text())
    for stem in ['lac','bmw']:
        path=source/f'{stem}-rust.bin';assert layout.digest(path)==report['files'][path.name]['sha256'];identities[str(path)]=layout.digest(path);data=path.read_bytes();d=data[44+2*layout.SIZE:44+3*layout.SIZE];settings=data[44+3*layout.SIZE:]
        ned=STEAM/f'cars/{stem}/{stem}.ned';identities[str(ned)]=layout.digest(ned);parts=list(decode(ned.read_bytes())[1].iter('PARTS'));assert len(parts)==1;cluster=layout.f32(float(parts[0].get('m_smashLightCluster','.1')))
        dims=struct.unpack_from('<3I',d,0xa414);count=dims[0]*dims[1]*dims[2]
        for index in range(128):
            car=bytearray([0 if index%2 else 0xcd])*CAR;physical=bytearray(rng.randbytes(PHYSICAL));car[0x2300:0xc800]=d;car[0xd240:0xd240+len(settings)]=settings
            for at,value in [(0xdb0,0),(0xdb8,0),(0xdbc,0),(0xdc0,0),(0xdc4,0),(0xdd4,0),(0x1620,4),(0xc8a4,0),(0xd004,0),(0xc7f8,0xd240),(0xc7a0,[0,1,0xffffffff][index%3]),(0x1554,0 if index%2 else 17),(0xba0,2 if index%2 else 1)]:struct.pack_into('<I',car,at,value)
            car[4]=int(index%8!=0);car[0xc7f2]=0;car[0x20fb4]=int(index%2!=0);car[0xc7ad]=int(index%9!=0)
            for at,value in [(0xee40,cluster),(0xc798,[-1.,0.,.001,.1][index%4]),(0xc790,[0.,.5,2.][index%3])]:struct.pack_into('<f',car,at,value)
            for at in [0x15c0,0x15d0]:struct.pack_into('<4f',car,at,*[layout.f32(rng.uniform(-50,50)) for _ in range(3)],float('nan'))
            for i in range(4):struct.pack_into('<f',car,0xcd90+i*4,[.1,.2,.25,.5][(index+i)%4] if index>=64 else .1)
            for i in range(count):
                at=0x4880+i*80;x,y,z,w=struct.unpack_from('<4f',car,at);shift=[0.,.05,.1,.2,-.1,-.3,.5,.9][index%8]
                struct.pack_into('<4f',car,at,layout.f32(x+shift),layout.f32(y+.02*shift*z),layout.f32(z-.03*shift*x),layout.f32(w+.001*(index%3)))
            for i in range(12):
                struct.pack_into('<I',car,0xcc10+i*32,[0,1,2][(index+i)%3]);struct.pack_into('<4f',car,0xcc20+i*32,(-.7 if i%2 else .7),.25,(i%3-1)*1.5,1.)
            for i in range(4):struct.pack_into('<4f',car,0xcda0+i*16,(-.7 if i%2 else .7),0.,(-1.5 if i<2 else 1.5),1.)
            rows.append(dict(name=f'{stem}-{index}',record=struct.pack('<f',[0.,.001,.016,.1][index%4])+bytes(car)+bytes(physical)))
    return rows,identities


def request(rows):return b'VD01'+struct.pack('<I',len(rows))+b''.join(r['record'] for r in rows)


def guard_payloads(rows):
    good=request([rows[5]]);bad={'magic':b'BAD!'+good[4:],'zero_count':good[:4]+bytes(4)+good[8:],'truncated':good[:-1],'trailing':good+b'x'}
    for name,at,value in [('nan_delta',0,0x7fc00000),('negative_delta',0,0xbf800000),('large_delta',0,0x3e800000)]:
        b=bytearray(good);struct.pack_into('<I',b,8+at,value);bad[name]=bytes(b)
    for name,at,value in [('wrapper',0xdb0,1),('wheel_count',0x1620,5),('render_wheel',0xdb8,1),('render_body',0xdd4,1),('part_collection',0xc8a4,1),('notification',0xd004,1),('settings',0xc7f8,1),('pending_rebuild',0xc7f0,0x10000),('body_flag',4,2),('dimension',0xc714,17),('zero_spacing',0xc720,0),('nan_node',0x4880,0x7fc00000),('light_state',0xcc10,3),('nan_light',0xcc20,0x7fc00000),('zero_range',0xcd94,0),('nan_wheel',0xcda0,0x7fc00000),('nan_cluster',0xee40,0x7fc00000),('nan_timer',0xc798,0x7fc00000)]:
        b=bytearray(good);struct.pack_into('<I',b,12+at,value);bad[name]=bytes(b)
    return good,bad


def check_answer(row,answer,coverage,extra_allowed=frozenset()):
    assert len(answer)==RECORD and answer[:4]==row['record'][:4]
    original=row['record'][4:4+CAR];r=answer[4:4+CAR]
    coverage['light_states_changed']+=sum(r[0xcc10+j*32:0xcc14+j*32]!=original[0xcc10+j*32:0xcc14+j*32] for j in range(12))
    coverage['wheel_colours_changed']+=r[0xcde0:0xcde4]!=original[0xcde0:0xcde4];coverage['mode_one']+=struct.unpack_from('<I',original,0xc7a0)[0]==1
    assert all(r[at:at+4]==original[at:at+4] for at in range(0x2300,0xc800,4) if at not in {0xc780,0xc784,0xc790,0xc794,0xc798}|extra_allowed) and r[0xd240:0xef60]==original[0xd240:0xef60]
    allowed={0xc780,0xc784,0xc790,0xc794,0xc798,0x15cc,0x15dc,0xcdf4,0xcdfc,0xcde0}|{0xcc10+i*32 for i in range(12)}|extra_allowed
    assert all(r[at:at+4]==original[at:at+4] for at in range(0,CAR,4) if at not in allowed)
    before=row['record'][4+CAR:];after=answer[4+CAR:]
    assert before[:0x210]==after[:0x210] and before[0x230:]==after[0x230:]
    for at,source in [(0x210,0x15d0),(0x220,0x15c0)]:assert after[at:at+12]==r[source:source+12] and after[at+12:at+16]==bytes(4)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,identities=cases()
    if args.pilot:rows=rows[:32]
    rust=ROOT/f'target/{args.profile}/{RUST_BIN}.exe';reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),32):
        batch=rows[start:start+32];source=out/f'{start}-input.bin';source.write_bytes(request(batch));runs={};answers={}
        for kind in ['original','repeat','rust']:
            exe=rust if kind=='rust' else PROBE;target=out/f'{start}-{kind}.bin';run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run['exit']==0 and not run['stderr'],runs[kind]
            if kind!='rust':assert 'code_unchanged=1 globals_restored=1 error=0' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+len(batch)*RECORD and len(b)==len(a)
        differences=[dict(case=batch[(i-8)//RECORD]['name'],offset=hex((i-8)%RECORD),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,repeat_exact=a==answers['repeat'],differences=differences,executions=runs);(out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert item['repeat_exact'] and not differences,differences[:15]
        for i,row in enumerate(batch):
            check_answer(row,a[8+i*RECORD:8+(i+1)*RECORD],coverage)
    good,bad=guard_payloads(rows);guards=[]
    for name,payload in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(payload);runs={}
        for kind in ['original','repeat','rust']:
            exe=rust if kind=='rust' else PROBE;target=out/f'guard-{name}-{kind}.bin';run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,output_exists=target.exists())
            assert (run['exit']==1 if kind=='rust' else 0<run['exit']<256) and 'fault=' not in run['stderr'] and not target.exists(),runs[kind]
        guards.append(dict(name=name,executions=runs))
    for kind in ['original','rust']:
        source=out/f'guard-existing-{kind}-input.bin';source.write_bytes(good);target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel');exe=rust if kind=='rust' else PROBE
        run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);assert run['exit']==(1 if kind=='rust' else 8) and target.read_bytes()==b'owned sentinel';guards.append(dict(name='existing-'+kind,execution=run,preserved=True))
    (out/'guards.json').write_text(json.dumps(guards,indent=2))
    result=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),reports=reports,identities=identities,guards=guards,coverage=dict(coverage),files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},scope=SCOPE)
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
