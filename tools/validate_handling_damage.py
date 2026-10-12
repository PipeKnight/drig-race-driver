"""Whole original handling producer, joined damage/wear and owned global RNG."""
import argparse
import json
from pathlib import Path
import random
import struct
from validate_collision_damage import SOURCE,IMAGE,CAR,PHYSICAL,M,authored,sha,f32,f,u,put,projection,run

PREFIX=528
SIZE=PREFIX+CAR+PHYSICAL


def fixtures():
    source=SOURCE.read_bytes();assert sha(SOURCE)=='f515fc1ad6c2e18700159875f15b4616745c40e54d8863004c0d39b2beba2765'
    car=source[4:4+CAR];physical=source[4+CAR:4+CAR+PHYSICAL];rng=random.Random(200682)
    for label,count in [('source',128),('authored',256),('authored-basis',128),('threshold',128)]:
        for index in range(count):
            v=bytearray(car) if label=='source' else authored(car,rng)
            p=bytearray(physical)
            if label=='authored-basis':
                for j in range(3):put(p,0x1d0+j*16,*[rng.uniform(-1,1) for _ in range(3)],0.)
                put(p,0x200,*[rng.uniform(-30,30) for _ in range(3)],1.)
            delta=[0.,.001,.016,.02,.05][index%5];step=min(f32(delta),f32(.02))
            threshold=f32(f(p,0x3f0)*f(v,M+0x368));divisor=f32(f(p,0x3f0)*f(v,M+0x36c))
            for j in range(4):
                if label!='source':
                    # Both homogeneous-center policies occur before full factory recovery.
                    put(v,M+0x60+j*48+12,float((index+j)%2))
                    put(v,M+0x60+j*48,*[rng.uniform(-1,1) for _ in range(3)])
                impact=rng.choice([0.,threshold*.5,threshold*1.01,divisor*.5,divisor,divisor*2])
                if label=='threshold':
                    force=f32(threshold/step) if step else 0.
                    bits=struct.unpack('<I',struct.pack('<f',force))[0]
                    force=struct.unpack('<f',struct.pack('<I',bits+[-1,0,1,2][j]))[0] if bits else 0.
                else:force=f32(impact/step) if step else rng.uniform(0,1.e6)
                put(v,0x1ac8+j*0x150,force)
                if label!='source':put(v,0x1aa8+j*0x150,rng.choice([0.,.5,2.,20.]))
            if label!='source':
                put(v,M+0x2f0,1.,10.)
                put(v,M+0x280,0.) # No notification events in this scope.
                v[M+0x442]=index%2
            records=[[],[],[(0x1f,5),(0x45,4)],[(0x1f,3),(0x1f,5),(0x46,5)],[(0x45,2),(0x46,4)]][index%5]
            prefix=struct.pack('<fIII',delta,rng.getrandbits(32),int(index%5!=0),len(records))
            prefix+=b''.join(struct.pack('<II',*q) for q in records)+bytes((64-len(records))*8)
            yield label,prefix+v+p


def damage_projection(r):
    """Independent four forward transforms, dispatch order and global RNG."""
    v=bytearray(r[PREFIX:PREFIX+CAR]);p=r[PREFIX+CAR:];state=u(r,4);delta=min(f(r,0),f32(.02));reached=0
    for i in range(4):
        local=[f(v,M+0x60+i*48+j*4) for j in range(4)]
        point=[]
        for j in range(4):
            wy=f32(f32(local[3]*f(p,0x200+j*4))+f32(local[1]*f(p,0x1e0+j*4)))
            zx=f32(f32(local[2]*f(p,0x1f0+j*4))+f32(local[0]*f(p,0x1d0+j*4)))
            point.append(f32(wy+zx))
        impact=f32(f(v,0x1ac8+i*0x150)*delta)
        q=struct.pack('<IIfI4f',state,1,impact,0,*point)+v+p
        active,state,levels=projection(q);reached+=active
        for at,bits in levels.items():v[M+at:M+at+4]=bits
    return state,v,reached


def check(request,answer):
    assert len(answer)==len(request) and answer[:8]==request[:8]
    reached=drawn=wear=changed=0
    for at in range(8,len(request),SIZE):
        r=request[at:at+SIZE];o=answer[at:at+SIZE];state,v,active=damage_projection(r)
        assert u(o,4)==state;reached+=active;drawn+=state!=u(r,4)
        assert o[:4]==r[:4] and o[8:PREFIX]==r[8:PREFIX]
        assert o[PREFIX+CAR:]==r[PREFIX+CAR:]
        assert o[PREFIX:PREFIX+0x1644]==r[PREFIX:PREFIX+0x1644]
        assert o[PREFIX+0x1648:PREFIX+M]==r[PREFIX+0x1648:PREFIX+M]
        assert o[PREFIX+M+0x460:PREFIX+CAR]==r[PREFIX+M+0x460:PREFIX+CAR]
        wear+=u(o,PREFIX+0x1644)!=u(r,PREFIX+0x1644)
        for level in [0,4,8,0x28,0x2c,0x30,0x34]:assert o[PREFIX+M+level:PREFIX+M+level+4]==v[M+level:M+level+4]
        changed+=any(o[PREFIX+M+level:PREFIX+M+level+4]!=r[PREFIX+M+level:PREFIX+M+level+4] for level in [0,4,8,0x28,0x2c,0x30,0x34])
    return dict(reached_calls=reached,global_changes=drawn,wear_changes=wear,damage_changes=changed)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.output.resolve().is_relative_to(Path('local').resolve());a.output.mkdir(exist_ok=False)
    native=Path('local/behavior/handling-damage-probe.exe');debug=Path('target/debug/grid-handling-damage.exe');release=Path('target/release/grid-handling-damage.exe')
    paths=[native,debug,release,IMAGE,SOURCE,Path(__file__),Path('tools/validate_collision_damage.py'),Path('tools/validate_damage_region.py'),Path('tools/probe_handling_damage.c'),Path('tools/vehicle_handling_probe.h'),Path('tools/handling_query_probe.h'),Path('crates/grid-world/src/collision_damage.rs'),Path('crates/grid-world/src/vehicle_handling.rs'),Path('crates/grid-world/src/vehicle_publication.rs')]
    pinned={str(p):sha(p) for p in paths};items=list(fixtures());assert len(items)==640
    rows=[];total=0;stats=dict(reached_calls=0,global_changes=0,wear_changes=0,damage_changes=0)
    for batch in range(0,len(items),32):
        group=items[batch:batch+32];request=b'VH02'+struct.pack('<I',len(group))+b''.join(r for _,r in group)
        inp=a.output/f'{batch//32:03}.input';inp.write_bytes(request);outputs={};answers={}
        for name,exe in [('original',native),('repeat',native),('debug',debug),('release',release)]:
            out=a.output/f'{batch//32:03}-{name}.bin';e=run([exe,IMAGE,inp,out] if name in ['original','repeat'] else [exe,inp,out])
            if name in ['original','repeat']:assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
            answers[name]=out.read_bytes();outputs[name]=dict(path=str(out),sha256=sha(out),execution=e)
        original=answers['original']
        for name,data in answers.items():
            diff=next((i for i,(x,y) in enumerate(zip(data,original)) if x!=y),None)
            if data!=original:
                (a.output/'mismatch.json').write_text(json.dumps(dict(batch=batch,profile=name,offset=diff,record=(diff-8)//SIZE,field=hex((diff-8)%SIZE-PREFIX)),indent=2))
            assert data==original,(batch,name,diff,hex((diff-8)%SIZE-PREFIX) if diff else '')
        found=check(request,original)
        for k,value in found.items():stats[k]+=value
        total+=len(original);rows.append(dict(labels=[label for label,_ in group],request=str(inp),request_sha256=sha(inp),outputs=outputs,bytes=len(original)))
        if batch%128==96:print(json.dumps(dict(cases_complete=batch+len(group))),flush=True)
    base=next(r for label,r in items if label=='authored' and f(r,0)==f32(.016));request=b'VH02'+struct.pack('<I',1)+base
    variants=[('short',request[:-1]),('long',request+b'\0')]
    for label,at,word in [('tag',0,0),('count',4,0),('delta',8,0x7fc00000),('present',16,2),('query-count',20,65),('query-tail',8+PREFIX-4,1),('owner-link',8+PREFIX+M+0x214,1),('query-link',8+PREFIX+M+0x210,1),('impact-nan',8+PREFIX+0x1ac8,0x7fc00000),('mass',8+PREFIX+CAR+0x3f0,0)]:
        v=bytearray(request);struct.pack_into('<I',v,at,word);variants.append((label,v))
    guards=[]
    for label,data in variants:
        inp=a.output/f'guard-{label}.input';inp.write_bytes(data)
        for name,exe in [('original',native),('debug',debug),('release',release)]:
            out=a.output/f'guard-{label}-{name}.bin';run([exe,IMAGE,inp,out] if name=='original' else [exe,inp,out],False);assert not out.exists();guards.append(name+':'+label)
    existing=a.output/'existing.bin';existing.write_bytes(b'KEEP');inp=Path(rows[0]['request'])
    for name,exe in [('original',native),('debug',debug),('release',release)]:
        run([exe,IMAGE,inp,existing] if name=='original' else [exe,inp,existing],False);assert existing.read_bytes()==b'KEEP';guards.append(name+':existing')
    result=dict(passed=True,cases=len(items),bytes_per_run=total,inputs=pinned,rows=rows,guards=guards,**stats,runtime_enabled=False)
    with (a.output/'validation.json').open('x') as file:json.dump(result,file,indent=2)
    print(json.dumps({k:v for k,v in result.items() if k not in ['inputs','rows','guards']}))


if __name__=='__main__':main()
