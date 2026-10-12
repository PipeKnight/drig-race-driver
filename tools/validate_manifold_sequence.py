"""Retained original manifold arena versus Rust, with source-input lineage.

Triangle order, normals, witnesses, depth and publication flags are authored
boundary controls. This is not fresh collision detection or a packed traversal.
"""
import argparse
import json
import math
import struct
from pathlib import Path

import validate_triangle_manifold as cold
from validate_triangle_manifold import ROOT, IMAGE, digest, run

NATIVE = ROOT/'local/behavior/manifold-sequence-probe.exe'
STRIDE = 0xa20


def u(b,o): return struct.unpack_from('<I',b,o)[0]


def cases(previous, max_base=1225, controls=24):
    for row in previous['results'][:max_base]:
        yield dict(source_index=row['index'],kind='source',steps=8,normal=0,capacity=16)
    for k in range(controls):
        for normal in range(8):
            for capacity in [0,1,2,3,4,16]:
                yield dict(source_index=k*1225//24,kind='control',steps=16 if k%2 else 64,
                           normal=normal,capacity=capacity)


def request(raw, info):
    size=u(raw,4); chunk=raw[60+size:]
    nt=-struct.unpack_from('<i',chunk,24)[0]; start=u(raw,40)
    center=list(struct.unpack_from('<4f',raw,24)); normal=cold.NORMALS[info['normal']]
    records=[]
    for i in range(info['steps']):
        index=(start+[0,0,1,0,2,1,0,2][i%8])%nt
        point=center.copy();point[0]+=[0.,0.,.02,0.,-.02,.02,0.,-.02][i%8]
        depth=[.25,.75,-.25,.5,0.,1.,.125,.875][i%8]
        # Mode zero retains the prior normal despite another valid supplied one.
        mode=int(i%4!=3); supplied=normal if mode else cold.NORMALS[(info['normal']+1)%8]
        records.append(struct.pack('<II4f4ffI',index,mode,*supplied,*point,depth,i%2))
    b=bytearray(b'PC09'+raw[4:40]+struct.pack('<I',len(records))+b''.join(records)+raw[60:])
    struct.pack_into('<I',b,12,info['capacity'])
    return bytes(b)


def projection(answer, raw):
    base=0x44f8+u(raw,20)*8
    return answer[base+0x220:]


def check(answer, raw):
    steps=u(raw,40);s=44+steps*48;limit=u(raw,12);base=0x44f8+u(raw,20)*8
    assert len(answer)==base+0x220+4+steps*STRIDE
    records=projection(answer,raw);assert u(records,0)==steps
    previous=None;prior=0;direction=None;merged=added=full=preserved=0
    for i in range(steps):
        r=raw[44+i*48:92+i*48];record=records[4+i*STRIDE:4+(i+1)*STRIDE]
        arena=record[:0x800];q=record[0x800:0x8a0];features=record[0x8a0:]
        if u(r,4):direction=r[8:24]
        used=u(q,4);new=u(q,8)
        assert prior<=used<=limit and new==used-prior and q[:4]==r[:4]
        assert q[12:16]==bytes(4) and q[0x10:0x20]==direction and q[0x20:0x38]==r[24:48]
        assert q[0x38:0x40]==bytes([0xcd])*8 and q[0x90:]==bytes(16) and u(q,0x40)==0x1000
        points=u(raw,s+0x500)
        for v in struct.unpack_from('<7I',q,0x44):
            assert v>=0x30060 and (v-0x30060)%32==0 and (v-0x30060)//32<points
        for j in range(16):
            c=arena[j*128:(j+1)*128];assert u(c,0)==0x7200
            if j<used:
                assert c[0x71] in [0,1] and c[0x73] in [0,1]
                assert all(math.isfinite(x) for x in struct.unpack_from('<14f',c,0x38))
                if j>=prior:assert c[0x73]==u(r,44)
                elif previous is not None:
                    old=previous[j*128:(j+1)*128]
                    assert c[:0x38]==old[:0x38] and c[0x3c:]==old[0x3c:]
                    depth=struct.unpack_from('<f',c,0x38)[0];old_depth=struct.unpack_from('<f',old,0x38)[0]
                    assert depth>=old_depth;merged+=depth>old_depth;preserved+=1
            else:
                constructor=bytearray(128);struct.pack_into('<I',constructor,0,0x7200)
                constructor[0xf]=1;struct.pack_into('<3I',constructor,0x1c,0xffffffff,0xffffffff,1)
                assert c==constructor
        for j in range(2):
            f=features[j*0xc0:(j+1)*0xc0]
            assert all(math.isfinite(x) for x in struct.unpack_from('<20f',f))
            assert f[0x50:0xa0]==bytes([0xcd])*80 and f[0xb8:]==bytes([0xcd])*8
        if prior==limit:
            full+=1;assert not new and (previous is None or arena==previous)
            if i:assert q[0x40:0x60]==records[4+(i-1)*STRIDE+0x840:4+(i-1)*STRIDE+0x860]
        previous=arena;prior=used;added+=new
    # Reuse the complete cold-arena structural checker on the final state, with
    # diagnostic appended count cleared and flags normalized only in this copy.
    q=record[0x800:0x8a0]
    final_raw=b'PC07'+raw[4:40]+q[:4]+q[0x10:0x38]+raw[s:]
    final=bytearray(answer[:base+0x220]);final[base+8:base+12]=bytes(4)
    for j in range(prior):final[0xfe8+j*128+0x73]=u(r,44)
    cold.check(final,final_raw)
    assert answer[0xfe8:0x17e8]==arena and answer[base:base+0x220]==record[0x800:]
    return dict(intervals=steps,contacts=prior,appended=added,depth_merges=merged,
                preserved_records=preserved,full_intervals=full)


def guards(raw):
    yield 'short',raw[:43]
    yield 'long',raw+b'\0'
    s=44+u(raw,40)*48
    for label,o,v in [('count-zero',40,0),('count-65',40,65),('capacity',12,17),
                      ('first-retain',48,0),('mode',48,2),('triangle',44,0xffffffff),
                      ('normal-nan',52,0x7fc00000),('normal-zero-y',56,0),('normal-w',64,0x3f800000),
                      ('point-nan',68,0x7fc00000),('point-w',80,0),('depth-nan',84,0x7fc00000),
                      ('flag',88,2),('margin-negative',s+0x220,0xbf800000),('pose-nan',s+0x130,0x7fc00000),
                      ('retained-normal-zero',44+3*48+12,0),('late-point-w',44+7*48+36,0),
                      ('late-depth-nan',44+7*48+40,0x7fc00000),('late-flag',44+7*48+44,2),
                      ('late-mode',44+7*48+4,2)]:
        b=bytearray(raw);struct.pack_into('<I',b,o,v);yield label,bytes(b)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path)
    p.add_argument('--profile',choices=['debug','release'],default='debug')
    p.add_argument('--max-base',type=int,default=1225);p.add_argument('--controls',type=int,default=24)
    a=p.parse_args();assert 1<=a.max_base<=1225 and 0<=a.controls<=24
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    previous=json.loads(cold.REFERENCE.read_text());assert previous['passed'] and digest(IMAGE)==previous['image_sha256']
    rust=ROOT/f'target/{a.profile}/grid-convex-distance.exe';rows=[]
    for info in cases(previous,a.max_base,a.controls):
        source=previous['results'][info['source_index']];inp=cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(inp)==source['input_sha256'];raw=request(inp.read_bytes(),info);i=len(rows)
        target_input=out/f'{i:04}-input.bin';target_input.write_bytes(raw)
        row=dict(index=i,**info,input_sha256=digest(target_input),executions=[]);answers=[]
        for name,exe,native in [('original',NATIVE,True),('repeat',NATIVE,True),('rust',rust,False)]:
            target=out/f'{i:04}-{name}.bin';e=run(exe,target_input,target,native);row['executions'].append(e)
            if e['exit']:
                (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError((i,name,e))
            answers.append(target.read_bytes())
        if answers[0]!=answers[1] or projection(answers[0],raw)!=answers[2]:
            (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError(f'retained manifold mismatch {i}')
        row.update(check(answers[0],raw),bytes=len(answers[0]),output_sha256=digest(out/f'{i:04}-original.bin'),rust_sha256=digest(out/f'{i:04}-rust.bin'));rows.append(row)
        if len(rows)%100==0:print(f'cases={len(rows)} depth_merges={sum(r["depth_merges"] for r in rows)}',flush=True)
    rejected=[]
    for label,raw in guards((out/'0000-input.bin').read_bytes()):
        inp=out/f'guard-{label}-input.bin';inp.write_bytes(raw);executions=[]
        for name,exe,native in [('original',NATIVE,True),('rust',rust,False)]:
            target=out/f'guard-{label}-{name}.bin';e=run(exe,inp,target,native)
            assert 0<e['exit']<256 and not target.exists(),(label,name,e);executions.append(e)
        rejected.append(dict(label=label,input_sha256=digest(inp),executions=executions,no_output=True))
    for name,exe,native in [('original',NATIVE,True),('rust',rust,False)]:
        target=out/f'guard-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
        e=run(exe,out/'0000-input.bin',target,native);assert e['exit'] and digest(target)==before
        rejected.append(dict(label=f'existing-{name}',executions=[e],unchanged_sha256=before))
    totals={k:sum(r[k] for r in rows) for k in ['intervals','contacts','appended','depth_merges','preserved_records','full_intervals','bytes']}
    report=dict(passed=True,profile=a.profile,cases=len(rows),results=rows,guards=rejected,guard_count=len(rejected),
                totals=totals,max_base=a.max_base,controls=a.controls,reference_sha256=digest(cold.REFERENCE),
                image_sha256=digest(IMAGE),native_sha256=digest(NATIVE),rust_sha256=digest(rust),script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(dict(passed=True,**totals,guards=len(rejected))),flush=True)


if __name__=='__main__':main()
