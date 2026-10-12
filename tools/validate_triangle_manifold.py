"""Original whole feature/manifold boundary versus independently rebuilt Rust.

Normals, fallback witnesses/depths and capacities are authored boundary controls,
not a claim to reproduce the packed dispatcher's preceding hit/reorientation.
"""
import argparse
import json
import math
import struct
import subprocess
from pathlib import Path

import validate_triangle_mesh_distance as distance
from validate_triangle_mesh_distance import ROOT, IMAGE, digest, geometry

REFERENCE = ROOT/'local/behavior/triangle-mesh-distance-debug-20261012-final-01/validation.json'
NATIVE = ROOT/'local/behavior/triangle-manifold-probe.exe'
NORMALS = [(0.,1.,0.,0.),(0.,-1.,0.,0.),(1.,0.,0.,0.),(-1.,0.,0.,0.),
           (0.,0.,1.,0.),(0.,0.,-1.,0.),(.6,.8,0.,0.),(.8,0.,-.6,0.)]


def run(exe, inp, out, native=True):
    command = [str(exe),str(IMAGE),str(inp),str(out)] if native else [str(exe),str(inp),str(out)]
    e = subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=30)
    return dict(exit=e.returncode,stdout=e.stdout,stderr=e.stderr)


def request(raw, normal=NORMALS[0], capacity=16, depth=.25, margin=None, flag=1):
    b = bytearray(b'PC07'+raw[4:44]+struct.pack('<4f',*normal)+raw[24:40]+struct.pack('<fI',depth,flag)+raw[60:])
    struct.pack_into('<I',b,12,capacity)
    if margin is not None: struct.pack_into('<f',b,84+0x220,margin)
    return bytes(b)


def cases(previous, max_base=1225, controls=24):
    for row in previous['results'][:max_base]:
        yield dict(source_index=row['index'],kind='base'), dict()
    for k in range(controls):
        index = k*1225//24
        for n, normal in enumerate(NORMALS):
            for capacity in [0,1,2,3,4,16]:
                yield dict(source_index=index,kind='control',normal=n,capacity=capacity), dict(normal=normal,capacity=capacity,depth=[-.25,0.,.25][k%3],flag=k%2)
        for margin in [.05,.5]:
            yield dict(source_index=index,kind='margin',margin=margin), dict(margin=margin)


def projection(answer):
    return answer[0xfe8:0x17e8]+answer[-0x220:]


def check(answer, raw):
    u=lambda b,o:struct.unpack_from('<I',b,o)[0]
    seed_size,chunk_size,limit,cache_limit,index_limit=struct.unpack_from('<5I',raw,4)
    base_size=0x44f8+index_limit*8
    assert len(answer)==base_size+0x220 and cache_limit==1
    q=answer[base_size:base_size+0xa0]; features=answer[-0x180:]
    count=u(q,4)
    assert count<=limit and q[:4]==raw[40:44] and q[8:16]==bytes(8)
    assert q[0x10:0x38]==raw[44:84] and q[0x38:0x40]==bytes([0xcd])*8
    assert q[0x90:]==bytes(16) and u(q,0x40)==0x1000
    assert answer[0x4218+0x240:0x4218+0x270]==q[0x60:0x90]
    points=u(raw,84+0x500)
    for v in struct.unpack_from('<7I',q,0x44):
        assert v>=0x30060 and (v-0x30060)%32==0 and (v-0x30060)//32<points
        if limit==0: assert v==0x30060
    orientations=[]
    for i in range(2):
        f=features[i*0xc0:(i+1)*0xc0]
        assert all(math.isfinite(v) for v in struct.unpack_from('<20f',f))
        assert f[0x10:0x20]==bytes(16) and f[0x50:0xa0]==bytes([0xcd])*80
        assert f[0xa0:0xa4]==bytes([3,0xcd,0xcd,0xcd]) and f[0xa4:0xac]==bytes(8)
        assert f[0xac:0xb0]==(raw[84+0x220:84+0x224] if i==0 else bytes(4))
        assert f[0xb0] in [0,1] and f[0xb1:0xb4]==bytes([0,0xcd,0xcd]) and u(f,0xb4)==1
        assert f[0xb8:]==bytes([0xcd])*8
        orientations.append(f[0xb0])
    base=bytearray(answer[:base_size])
    assert u(base,0xfa0+0x30)==0 and u(base,0x21c8+0x40)==0
    assert u(base,0xfa0+0x10)==u(base,0xf50+0x24)==count
    # The direct boundary does not invoke the packed candidate cache. Assert its
    # untouched state above, then normalize only those two fields for the shared
    # complete source arena checker. Captured answers are never modified.
    struct.pack_into('<I',base,0xfa0+0x30,int(bool(limit)))
    struct.pack_into('<I',base,0x21c8+0x40,0xc000 if limit else 0)
    flipped=positive=negative=0
    for i in range(count):
        c=base[0xfe8+i*128:0x1068+i*128]
        assert c[0x73]==u(raw,80)
        base[0xfe8+i*128+0x73]=1
        flipped+=c[0x71]; depth=struct.unpack_from('<f',c,0x38)[0]
        positive+=depth>0.; negative+=depth<0.
    seed=raw[84:84+seed_size];chunk=raw[84+seed_size:]
    assert len(chunk)==chunk_size
    geometry.check(base,seed,chunk,struct.unpack_from('<4f',raw,24),limit,1,index_limit)
    return dict(contacts=count,flipped=flipped,positive=positive,negative=negative,orientations=orientations)


def guard_requests(raw):
    yield 'short',raw[:43]
    yield 'long',raw+b'\0'
    for label,o,v in [('tag',0,0),('seed-small',4,0),('seed-large',4,22001),('chunk-small',8,0),
                      ('capacity',12,17),('cache',16,2),('index-capacity',20,32769),
                      ('center-nan',24,0x7fc00000),('center-w',36,0),('triangle-index',40,0xffffffff),
                      ('normal-nan',44,0x7fc00000),('normal-zero-y',48,0),('normal-w',56,0x3f800000),
                      ('point-nan',60,0x7fc00000),('point-w',72,0),('depth-nan',76,0x7fc00000),('flag',80,2),
                      ('kind',84+0x10,0),('mesh-owner',84+0x240,0),('mesh-level',84+0x500+8,2),
                      ('margin-negative',84+0x220,0xbf800000),('pose-nan',84+0x130,0x7fc00000)]:
        b=bytearray(raw);struct.pack_into('<I',b,o,v);yield label,bytes(b)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path)
    p.add_argument('--profile',choices=['debug','release'],default='debug')
    p.add_argument('--max-base',type=int,default=1225);p.add_argument('--controls',type=int,default=24)
    a=p.parse_args();assert 1<=a.max_base<=1225 and 0<=a.controls<=24
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    previous=json.loads(REFERENCE.read_text());assert previous['passed']
    upstream=json.loads(distance.UPSTREAM.read_text());assert upstream['passed'] and digest(Path(upstream['source_path']))==upstream['source_sha256']
    attachment=ROOT/upstream['attachment'];assert digest(attachment)==upstream['attachment_sha256']
    attached=json.loads(attachment.read_text())['results'][upstream['attachment_index']]
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    for key in ['source','csv']:assert digest(game/attached[key])==attached[key+'_sha256']
    assert digest(IMAGE)==previous['image_sha256']
    rust=ROOT/f'target/{a.profile}/grid-convex-distance.exe';rows=[]
    for info,controls in cases(previous,a.max_base,a.controls):
        source=previous['results'][info['source_index']]
        inp=REFERENCE.parent/f"{source['index']:04}-input.bin";assert digest(inp)==source['input_sha256']
        raw=request(inp.read_bytes(),**controls);i=len(rows);target_input=out/f'{i:04}-input.bin';target_input.write_bytes(raw)
        row=dict(index=i,**info,input_sha256=digest(target_input),executions=[]);answers=[]
        for name,exe,is_native in [('original',NATIVE,True),('repeat',NATIVE,True),('rust',rust,False)]:
            target=out/f'{i:04}-{name}.bin';e=run(exe,target_input,target,is_native);row['executions'].append(e)
            if e['exit']:
                (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError((i,name,e))
            answers.append(target.read_bytes())
        if answers[0]!=answers[1] or projection(answers[0])!=answers[2]:
            (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError(f'manifold mismatch {i}')
        row.update(check(answers[0],raw),bytes=len(answers[0]),output_sha256=digest(out/f'{i:04}-original.bin'),rust_sha256=digest(out/f'{i:04}-rust.bin'));rows.append(row)
        if len(rows)%100==0:print(f'cases={len(rows)} contacts={sum(r["contacts"] for r in rows)}',flush=True)
    guards=[]
    for label,raw in guard_requests((out/'0000-input.bin').read_bytes()):
        inp=out/f'guard-{label}-input.bin';inp.write_bytes(raw);executions=[]
        for name,exe,is_native in [('original',NATIVE,True),('rust',rust,False)]:
            target=out/f'guard-{label}-{name}.bin';e=run(exe,inp,target,is_native)
            assert 0<e['exit']<256 and not target.exists(),(label,name,e);executions.append(e)
        guards.append(dict(label=label,input_sha256=digest(inp),executions=executions,no_output=True))
    for name,exe,is_native in [('original',NATIVE,True),('rust',rust,False)]:
        target=out/f'guard-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
        e=run(exe,out/'0000-input.bin',target,is_native);assert e['exit'] and digest(target)==before
        guards.append(dict(label=f'existing-{name}',executions=[e],unchanged_sha256=before))
    report=dict(passed=True,profile=a.profile,cases=len(rows),results=rows,guards=guards,guard_count=len(guards),
                contacts=sum(r['contacts'] for r in rows),flipped=sum(r['flipped'] for r in rows),positive=sum(r['positive'] for r in rows),negative=sum(r['negative'] for r in rows),
                observable_bytes=sum(r['bytes'] for r in rows),max_base=a.max_base,controls=a.controls,
                reference_sha256=digest(REFERENCE),upstream_sha256=digest(distance.UPSTREAM),image_sha256=digest(IMAGE),
                native_sha256=digest(NATIVE),rust_sha256=digest(rust),script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report[k] for k in ['passed','cases','contacts','flipped','positive','negative','observable_bytes','guard_count']}),flush=True)


if __name__=='__main__':main()
