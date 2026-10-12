"""Whole original triangle/mesh support loop versus source-input Rust.

All original image data and per-case evidence remain private under local/.
"""
import argparse
import json
import math
import struct
import subprocess
from pathlib import Path

import validate_packed_mesh_contact as geometry

ROOT, IMAGE, digest = geometry.ROOT, geometry.IMAGE, geometry.digest
NATIVE = ROOT/'local/behavior/triangle-mesh-distance-probe.exe'
OUTER = ROOT/'local/behavior/triangle-mesh-query-probe.exe'
REGRESSION = ROOT/'local/behavior/packed-mesh-query-regression-probe.exe'
UPSTREAM = ROOT/'local/behavior/packed-mesh-contact-lacetti-20261012-final-01/validation.json'
DIRECTIONS = [(0.,0.,0.,0.),(-1.,0.,0.,0.),(0.,1.,0.,0.),(0.,-1.,0.,0.),
              (0.,0.,1.,0.),(0.,0.,-1.,0.),(1.,2.,-3.,0.),(100.,0.,0.,0.)]


def run(exe, inp, out, native=True):
    command = [str(exe), str(IMAGE), str(inp), str(out)] if native else [str(exe),str(inp),str(out)]
    e = subprocess.run(command,cwd=ROOT,capture_output=True,text=True,timeout=30)
    return dict(exit=e.returncode,stdout=e.stdout,stderr=e.stderr)


def request(raw, triangle, direction, margin=None):
    b = bytearray(b'PC04'+raw[4:40]+struct.pack('<I4f',triangle,*direction)+raw[40:])
    if margin is not None:
        struct.pack_into('<f',b,60+0x220,margin)
    return bytes(b)


def projection(answer):
    return answer[0x1d38:0x1e58]+answer[-0xa0:]


def check(answer, raw):
    seed_size,chunk_size,limit,cache_limit,index_limit=struct.unpack_from('<5I',raw,4)
    seed,chunk=raw[60:60+seed_size],raw[60+seed_size:]
    assert len(chunk)==chunk_size and len(answer)==0x44f8+index_limit*8+0xa0
    base=bytearray(answer[:-0xa0])
    # No dispatcher/cache invocation in the direct distance loop. Preserve and
    # assert supplied worker capacities, then reuse the original empty-arena audit.
    assert struct.unpack_from('<I',base,0xfa0+4)[0]==limit
    assert struct.unpack_from('<I',base,0xfa0+0x14)[0]==limit
    assert struct.unpack_from('<I',base,0xfa0+0x34)[0]==cache_limit
    for o in [0xfa0+4,0xfa0+0x14,0xfa0+0x34]:struct.pack_into('<I',base,o,0)
    geometry.check(base,seed,chunk,struct.unpack_from('<4f',raw,24),0,0,index_limit)
    simplex,query=answer[0x1d38:0x1e58],answer[-0xa0:]
    assert simplex[0]<=4 and simplex[0x11c]<=1 and simplex[0x11d]<=1 and simplex[0x11e]<=1
    assert query[:4]==raw[40:44] and query[4:8]==bytes(4) and query[8]<=1 and query[9]<=1
    assert query[10:16]==bytes(6) and query[0x90:]==bytes(16)
    assert struct.unpack_from('<I',query,0x40)[0]==0x1000
    points=struct.unpack_from('<I',seed,0x500)[0]
    for v in struct.unpack_from('<7I',query,0x44):assert v>=0x30060 and (v-0x30060)%32==0 and (v-0x30060)//32<points
    assert all(math.isfinite(v) for v in struct.unpack_from('<4f',query,0x10))
    assert struct.unpack_from('<f',query,0x1c)[0]==0.
    if query[8]:
        assert all(math.isfinite(v) for v in struct.unpack_from('<8f',query,0x20))
        assert struct.unpack_from('<f',query,0x2c)[0]==struct.unpack_from('<f',query,0x3c)[0]==1.
    else:assert query[0x20:0x40]==bytes([0xcd])*32
    # Decoded packed triangle must be exactly what the original setter retained.
    assert answer[0x4218+0x240:0x4218+0x270]==query[0x60:0x90]
    return dict(simplex_count=simplex[0],flags=list(query[8:10]),witnesses=bool(query[8]),
                direction=list(struct.unpack_from('<4f',query,0x10)))


def main():
    p=argparse.ArgumentParser()
    p.add_argument('output',type=Path)
    p.add_argument('--profile',choices=['debug','release'],default='debug')
    p.add_argument('--max-base',type=int,default=1225)
    p.add_argument('--controls',type=int,default=24)
    p.add_argument('--regression',action='store_true')
    args=p.parse_args()
    assert 1<=args.max_base<=1225 and 0<=args.controls<=24
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rust=ROOT/f'target/{args.profile}/grid-convex-distance.exe'
    old=json.loads(UPSTREAM.read_text());assert old['passed'] and digest(IMAGE)==old['image_sha256']
    assert digest(geometry.NATIVE)==old['native_sha256']
    assert digest(Path(old['source_path']))==old['source_sha256']
    rows=[]
    if args.regression:
        for row in old['results']:
            inp=UPSTREAM.parent/f"{row['index']:04}-input.bin";assert digest(inp)==row['input_sha256']
            target=out/f"{row['index']:04}-regression.bin";e=run(REGRESSION,inp,target)
            assert e['exit']==0 and digest(target)==row['output_sha256'],(row['index'],e)
            rows.append(dict(index=row['index'],bytes=target.stat().st_size,output_sha256=digest(target),execution=e))
        report=dict(passed=True,regression_only=True,cases=len(rows),results=rows,native_sha256=digest(REGRESSION))
    else:
        cases=[(r,[1.,0.,0.,0.],None) for r in old['results'][:args.max_base]]
        for r in old['results'][:args.max_base:max(1,args.max_base//max(1,args.controls))][:args.controls]:
            cases.extend((r,list(d),margin) for margin in [0.,.05,.5] for d in DIRECTIONS)
        for r,direction,margin in cases:
            source=UPSTREAM.parent/f"{r['index']:04}-input.bin";assert digest(source)==r['input_sha256']
            raw=request(source.read_bytes(),r['triangle'],direction,margin)
            i=len(rows);inp=out/f'{i:04}-input.bin';inp.write_bytes(raw)
            row=dict(index=i,source_index=r['index'],direction_input=direction,margin=margin,input_sha256=digest(inp),executions=[])
            answers=[]
            for name,exe,native in [('original',NATIVE,True),('repeat',NATIVE,True),('rust',rust,False)]:
                target=out/f'{i:04}-{name}.bin';e=run(exe,inp,target,native);row['executions'].append(e)
                if e['exit']:
                    (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError((i,name,e))
                answers.append(target.read_bytes())
            if answers[0]!=answers[1] or projection(answers[0])!=answers[2]:
                (out/'failure.json').write_text(json.dumps(row,indent=2));raise RuntimeError(f'distance output mismatch case {i}')
            row.update(check(answers[0],raw),bytes=len(answers[0]),output_sha256=digest(out/f'{i:04}-original.bin'),rust_sha256=digest(out/f'{i:04}-rust.bin'))
            rows.append(row)
            if len(rows)%100==0:print(json.dumps(dict(cases=len(rows),witnesses=sum(v['witnesses'] for v in rows))),flush=True)
        raw=(out/'0000-input.bin').read_bytes();seed_size=struct.unpack_from('<I',raw,4)[0];mesh_size=seed_size-0x1620;chunk_start=60+seed_size
        variants=[('short',raw[:-1]),('long',raw+b'\0')]
        edits=[('tag',0,0),('seed-size',4,0),('chunk-size',8,0),('contact-capacity',12,17),('cache-capacity',16,2),('index-capacity',20,32769),
               ('center-nan',24,0x7fc00000),('center-inf',28,0x7f800000),('center-bound',32,0x46200001),('center-w',36,0),
               ('triangle-index',40,32768),('direction-nan',44,0x7fc00000),('direction-bound',48,0x46200001),('direction-w',56,0x3f800000),
               ('mesh-kind',60+0x10,6),('mesh-reference',60+0x240,1),('callback',60+0x188,1),('special-geometry',60+0x184,1),('linked-shape',60+0xdc,1),
               ('body-joint',60+0x500+mesh_size+0x320,1),('mesh-points',60+0x500,81),('mesh-levels',60+0x500+8,2),
               ('mesh-adjacency',60+0x500+0x70,0xffffffff),('margin-negative',60+0x220,0xbf800000),('margin-nan',60+0x220,0x7fc00000),
               ('body-center-nan',60+0x500+mesh_size+0x1c0,0x7fc00000),('chunk-bounds-nan',chunk_start,0x7fc00000),
               ('chunk-triangle-count',chunk_start+24,0),('chunk-vertex-offset',chunk_start+36,0xffffffff)]
        for label,o,v in edits:
            b=bytearray(raw);struct.pack_into('<I',b,o,v);variants.append((label,bytes(b)))
        guards=[]
        for label,data in variants:
            inp=out/f'guard-{label}-input.bin';inp.write_bytes(data);executions=[]
            for name,exe,native in [('original',NATIVE,True),('rust',rust,False)]:
                target=out/f'guard-{label}-{name}.bin';e=run(exe,inp,target,native);assert 0<e['exit']<256 and not target.exists(),(label,name,e);executions.append(e)
            guards.append(dict(label=label,executions=executions,no_output=True))
        for name,exe,native in [('original',NATIVE,True),('rust',rust,False)]:
            target=out/f'guard-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
            e=run(exe,out/'0000-input.bin',target,native);assert e['exit'] and digest(target)==before
            guards.append(dict(label=f'existing-{name}',executions=[e],unchanged_sha256=before))
        report=dict(passed=True,regression_only=False,profile=args.profile,cases=len(rows),base_cases=args.max_base,control_seeds=args.controls,
                    results=rows,guards=guards,guard_count=len(guards),witness_cases=sum(r['witnesses'] for r in rows),
                    native_sha256=digest(NATIVE),rust_sha256=digest(rust),outer_native_sha256=digest(OUTER))
    report.update(upstream=str(UPSTREAM.relative_to(ROOT)),upstream_sha256=digest(UPSTREAM),image_sha256=digest(IMAGE),
                  script_sha256=digest(Path(__file__)),observable_bytes=sum(r['bytes'] for r in rows))
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report[k] for k in ['passed','cases','observable_bytes']}),flush=True)


if __name__=='__main__':main()
