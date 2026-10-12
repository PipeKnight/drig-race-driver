"""Complete unchanged original impact dispatcher versus owned Rust state/RNG."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess
from validate_damage_region import record as region_record, expected as region_expected

SOURCE=Path('local/behavior/damage-boxes-20261013-final-01/source.state.bin')
IMAGE=Path('local/compatibility/grid-unpacked.exe')
CAR=0x21000
PHYSICAL=0x424
SIZE=32+CAR+PHYSICAL
OUTSIZE=8+CAR+PHYSICAL
M=0xcdf0
BOXES=[0x60,0x90,0xc0,0xf0,0x120,0x150,0x180,0x1b0,0x1e0]
COEFFICIENTS=[0x218,0x22c,0x240,0x304,0x32c,0x340]


def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def f(b,o):return struct.unpack_from('<f',b,o)[0]
def u(b,o):return struct.unpack_from('<I',b,o)[0]
def put(b,o,*values):struct.pack_into('<'+'f'*len(values),b,o,*values)


def authored(car,rng):
    v=bytearray(car)
    put(v,M+0x430,*[rng.uniform(.005,2.7) for _ in range(3)])
    for at in COEFFICIENTS:put(v,M+at,rng.uniform(0,1),rng.random(),rng.random())
    # Activating originally disabled channels also requires coherent effect
    # thresholds. A source threshold of one reaches 0/0 at terminal damage.
    for at in [0x228,0x23c,0x254,0x2f8,0x314,0x324,0x328,0x33c,0x354]:put(v,M+at,.5)
    for at in BOXES:
        put(v,M+at,*[rng.uniform(-2,2) for _ in range(3)],1.,*[rng.uniform(.1,2) for _ in range(3)],1.)
        v[M+at+32]=int(rng.random()>.2)
    for at in [0,4,8,0x10,0x14,0x28,0x2c,0x30,0x34]:put(v,M+at,rng.choice([0.,.1,.5,.99,1.,1.2]))
    put(v,M+0x370,rng.choice([0.,.1,1.,2.]),rng.uniform(0,3),rng.uniform(0,3))
    put(v,M+0x42c,rng.choice([0.,.5,1.]));v[0x104d]=rng.randrange(2)
    return v


def fixtures():
    raw=SOURCE.read_bytes();assert raw[:4]==b'DV01'
    assert sha(SOURCE)=='f515fc1ad6c2e18700159875f15b4616745c40e54d8863004c0d39b2beba2765'
    car=raw[4:4+CAR];physical=raw[4+CAR:4+CAR+PHYSICAL]
    rng=random.Random(200681)
    for label,count in [('source',128),('authored',384),('strict-threshold',128),('authored-basis',128)]:
        for index in range(count):
            v=bytearray(car) if label=='source' else authored(car,rng)
            p=bytearray(physical);mode=index%2
            if label=='authored-basis':
                for row in range(3):put(p,0x1d0+row*16,*[rng.uniform(-1,1) for _ in range(3)],0.)
                put(p,0x200,*[rng.uniform(-100,100) for _ in range(3)],1.)
            point=tuple(f32(f(p,0x200+j*4)+rng.uniform(-2,2)) for j in range(3))+(1.,)
            threshold=f32(f(p,0x3f0)*f(v,M+(0x368 if mode else 0x360)))
            divisor=f32(f(p,0x3f0)*f(v,M+(0x36c if mode else 0x364)))
            if label=='strict-threshold':
                bits=struct.unpack('<I',struct.pack('<f',threshold))[0]
                impact=[struct.unpack('<f',struct.pack('<I',bits-1))[0],threshold,
                        struct.unpack('<f',struct.pack('<I',bits+1))[0],f32(divisor*2)][(index//2)%4]
                v[0x104d]=(index//8)%2;put(v,M+0x42c,float((index//16)%2))
            else:impact=f32(rng.choice([0.,threshold*.5,threshold,threshold*1.01,divisor*.5,divisor,divisor*2]))
            state=rng.getrandbits(32)
            yield label,struct.pack('<IIfI4f',state,mode,impact,0,*point)+v+p


def projection(r):
    """Independent caller arithmetic/channel scheduling plus F241 Python oracle."""
    v=r[32:32+CAR];p=r[32+CAR:];b=v[M:M+0x460];state=u(r,0)
    levels={at:b[at:at+4] for at in [0,4,8,0x10,0x14,0x28,0x2c,0x30,0x34]}
    at=0x368 if u(r,4) else 0x360
    if f(r,8)<=f32(f(p,0x3f0)*f(b,at)):return False,state,levels
    impact=min(1.,f32(f(r,8)/f32(f(p,0x3f0)*f(b,at+4))))
    delta=[f32(f(r,16+j*4)-f(p,0x200+j*4)) for j in range(4)]
    local=[]
    for j in range(3):
        products=[f32(delta[k]*f(p,0x1d0+j*16+k*4)) for k in range(4)]
        local.append(f32(f32(products[1]+products[0])+f32(products[3]+products[2])))
    local.append(1.)
    main=f32(f32(f(b,0x438)*f(b,0x430))*f(b,0x434))
    secondary=f32(f32(f(b,0x438)*f(b,0x434))*f(b,0x430))
    channels=[(0x28+i*4,0x60+i*48,0x240,main) for i in range(4)]+[(0,0x150,0x218,main)]
    if not u(r,4) and v[0x104d]:channels.append((4,0x180,0x22c,secondary))
    if not u(r,4):channels.append((8,0x1b0,0x304,main))
    channels.append((0x10,0x1e0,0x32c,main))
    if f(b,0x42c)>0:channels.append((0x14,0x120,0x340,main))
    for level,box,coeff,gain in channels:
        q=region_record(previous=f(b,level),point=local,impact=impact,gain=gain,
                        center=struct.unpack_from('<4f',b,box),half=struct.unpack_from('<4f',b,box+16),
                        coefficients=struct.unpack_from('<3f',b,coeff),enabled=b[box+32],
                        radius=f(b,0x370),factors=(f(b,0x374),f(b,0x378)),state=state)
        result,_=region_expected(q);levels[level]=result[:4];state=u(result,4)
    return True,state,levels


def run(command,success=True):
    p=subprocess.run([str(v) for v in command],capture_output=True,text=True,timeout=60)
    assert (p.returncode==0)==success,(command,p.returncode,p.stdout,p.stderr)
    return dict(exit=p.returncode,stdout=p.stdout,stderr=p.stderr)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.output.resolve().is_relative_to(Path('local').resolve());a.output.mkdir(exist_ok=False)
    native=Path('local/behavior/collision-damage-probe.exe');debug=Path('target/debug/grid-collision-damage.exe');release=Path('target/release/grid-collision-damage.exe')
    pinned={str(p):sha(p) for p in [native,debug,release,IMAGE,SOURCE,Path(__file__)]}
    items=list(fixtures());assert len(items)==768
    rows=[];applied=0;random_changes=0;level_changes=0;total=0
    for batch in range(0,len(items),32):
        group=items[batch:batch+32];request=b'DI01'+struct.pack('<I',len(group))+b''.join(r for _,r in group)
        inp=a.output/f'{batch//32:03}.input';inp.write_bytes(request)
        outputs={};answers={}
        for name,exe in [('original',native),('repeat',native),('debug',debug),('release',release)]:
            out=a.output/f'{batch//32:03}-{name}.bin'
            e=run([exe,IMAGE,inp,out] if name in ['original','repeat'] else [exe,inp,out])
            if name in ['original','repeat']:assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
            answers[name]=out.read_bytes();outputs[name]=dict(path=str(out),sha256=sha(out),execution=e)
        original=answers['original'];assert len(original)==8+len(group)*OUTSIZE and original[:8]==request[:8]
        for name,data in answers.items():
            diff=next((i for i,(x,y) in enumerate(zip(data,original)) if x!=y),None)
            assert data==original,(batch,name,diff,hex((diff-8)%OUTSIZE) if diff is not None else '')
        for j,(label,r) in enumerate(group):
            answer=original[8+j*OUTSIZE:8+(j+1)*OUTSIZE]
            changed=any(answer[8+M+at:8+M+at+4]!=r[32+M+at:32+M+at+4] for at in [0,4,8,0x10,0x14,0x28,0x2c,0x30,0x34])
            active,final_state,levels=projection(r)
            assert u(answer,0)==active and u(answer,4)==final_state
            assert all(answer[8+M+at:8+M+at+4]==word for at,word in levels.items())
            level_changes+=changed;applied+=u(answer,0);random_changes+=u(answer,4)!=u(r,0)
            assert answer[8+CAR:]==r[32+CAR:]
            assert answer[8:8+M]==r[32:32+M] and answer[8+M+0x460:8+CAR]==r[32+M+0x460:32+CAR]
            if label=='source':assert not changed
            if not u(answer,0):assert answer[8:]==r[32:] and u(answer,4)==u(r,0)
        total+=len(original);rows.append(dict(batch=batch//32,labels=[label for label,_ in group],request=str(inp),request_sha256=sha(inp),outputs=outputs,bytes=len(original)))
        if batch%128==96:print(json.dumps(dict(cases_complete=batch+len(group))),flush=True)
    # Each malformed fixture is preflighted in native/Rust before any output.
    base=next(r for label,r in items if label=='authored');request=b'DI01'+struct.pack('<I',1)+base
    variants=[('short',request[:-1]),('long',request+b'\0')]
    for label,at,word in [('tag',0,0),('count',4,0),('mode',8+4,2),('reserved',8+12,1),
                         ('point-nan',8+16,0x7fc00000),('point-w',8+28,0),
                         ('mass',8+32+CAR+0x3f0,0),('basis-w',8+32+CAR+0x1dc,0x3f800000),
                         ('basis-extent',8+32+CAR+0x1d0,0x40000000),
                         ('region-enable',8+32+M+0x60+32,2),('region-half',8+32+M+0x60+16,0xbf800000),
                         ('probability',8+32+M+0x244,0x40000000),('settings-nan',8+32+M+0x218,0x7fc00000),
                         ('gain',8+32+M+0x430,0x7f7fffff),('owner-link',8+32+M+0x214,1)]:
        v=bytearray(request);struct.pack_into('<I',v,at,word);variants.append((label,v))
    for label,threshold,level in [('terminal-gear-effect',0x354,0x14),('terminal-wheel-effect',0x254,0x28)]:
        v=bytearray(request);struct.pack_into('<f',v,8+8,1.e6)
        for at in BOXES:v[8+32+M+at+32]=0
        for at in [0x430,0x434,0x438]:put(v,8+32+M+at,0.)
        put(v,8+32+M+threshold,1.);put(v,8+32+M+level,1.)
        variants.append((label,v))
    guards=[]
    for label,req in variants:
        inp=a.output/f'guard-{label}.input';inp.write_bytes(req)
        for name,exe in [('native',native),('debug',debug),('release',release)]:
            out=a.output/f'guard-{label}-{name}.bin'
            run([exe,IMAGE,inp,out] if name=='native' else [exe,inp,out],False)
            assert not out.exists();guards.append(name+':'+label)
    existing=a.output/'existing.bin';existing.write_bytes(b'KEEP');valid=a.output/'000.input'
    for name,exe in [('native',native),('debug',debug),('release',release)]:
        run([exe,IMAGE,valid,existing] if name=='native' else [exe,valid,existing],False)
        assert existing.read_bytes()==b'KEEP';guards.append(name+':existing')
    assert all(sha(p)==v for p,v in pinned.items())
    report=dict(passed=True,cases=len(items),bytes_per_run=total,applied=applied,random_changes=random_changes,
                level_changes=level_changes,inputs=pinned,rows=rows,guards=guards,runtime_integration=False,
                source_scope='current five NED regions; source gains still zero; active gains/wheel regions/coefficient variations explicitly authored')
    (a.output/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['passed','cases','bytes_per_run','applied','random_changes','level_changes']}))


if __name__=='__main__':main()
