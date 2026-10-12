"""Whole original witness orientation -> retained manifold versus Rust.

Source geometry/transform anchors with authored witnesses, normals and depth.
No packed normal/depth selection, backface correction or fresh traversal claim.
"""
import argparse
import json
import struct
from pathlib import Path

import validate_manifold_sequence as retained
from validate_manifold_sequence import ROOT, IMAGE, digest, run, u
from validate_vcqtc import f32

NATIVE=ROOT/'local/behavior/oriented-manifold-probe.exe'


def source_chunks():
    report=json.loads(retained.cold.distance.UPSTREAM.read_text());assert report['passed']
    track=Path(report['source_path']);assert digest(track)==report['source_sha256']
    chunks={n:(v,t) for n,v,t,_,_ in retained.cold.geometry.chunks(track.read_bytes())}
    return report,chunks


def cases(max_source=245, controls=24):
    for k in range(max_source):yield dict(source_index=k*5+2,kind='source',steps=8,normal=0,capacity=16)
    for k in range(controls):
        for normal in range(8):
            for capacity in [0,1,2,3,4,16]:
                yield dict(source_index=k*1225//24,kind='control',steps=16 if k%2 else 64,normal=normal,capacity=capacity)
    if controls:
        for normal in range(8):
            for capacity in [0,3,16]:
                yield dict(source_index=2,kind='threshold',steps=64,normal=normal,capacity=capacity)


def request(raw, info, upstream, chunks):
    old=retained.request(raw,info);count=u(old,40);s=44+count*48;size=u(old,4)
    seed=old[s:s+size];mesh_size=size-0x1620;body=0x500+mesh_size
    center=struct.unpack_from('<4f',old,24)
    if info['kind']=='threshold':
        center=tuple(f32(struct.unpack_from('<f',seed,body+0x1c0+j*4)[0]-struct.unpack_from('<f',seed,0x160+j*4)[0]) for j in range(3))+(1.,)
    anchor=[f32(struct.unpack_from('<f',seed,0x160+j*4)[0]+f32(center[j]-struct.unpack_from('<f',seed,body+0x1c0+j*4)[0])) for j in range(3)]+[1.]
    vertices,triangles=chunks[upstream['results'][info['source_index']]['chunk']]
    records=[]
    for i in range(count):
        r=bytearray(old[44+i*48:92+i*48]);v=vertices[triangles[u(r,0)][0][0]]+[1.]
        da=[(.25,1.,.125),(-.25,-1.,-.125),(0.,0.,0.),(0.,1e-6,0.),(0.,-1e-6,0.),(1.,.25,0.),(0.,-2.,0.),(0.,2.,0.)][i%8]
        db=[(0.,2.,0.),(0.,-2.,0.),(0.,1.,0.),(0.,-1.,0.),(0.,0.,0.),(0.,.25,1.),(0.,1.,0.),(0.,-1.,0.)][i%8]
        a=[f32(v[j]+da[j]) for j in range(3)]+[1.]
        b=[f32(anchor[j]+db[j]) for j in range(3)]+[1.]
        if info['kind']=='threshold':
            assert anchor==[0.,0.,0.,1.],anchor
            offsets=[0,0x3727c5ab,0x3727c5ac,0x3727c5ad,0x3727c5ae,0x3727c5aa,0x358637bd,0x38d1b717]
            tiny=struct.unpack('<f',struct.pack('<I',offsets[i%8]))[0]
            a=[f32(v[0]+1.),f32(v[1]-1.),v[2],1.]
            b=[0.,tiny if i%2 else -tiny,0.,1.]
        struct.pack_into('<4f',r,24,*b)
        records.append(bytes(r)+struct.pack('<4f',*a))
    header=bytearray(b'PC10'+old[4:44]);struct.pack_into('<4f',header,24,*center)
    return bytes(header)+b''.join(records)+old[s:]


def projection(answer,raw):return retained.projection(answer,raw)


def check(answer,raw):
    count=u(raw,40);s=44+count*64;base=0x44f8+u(raw,20)*8
    records=projection(answer,raw);assert u(records,0)==count
    normalized=bytearray(answer);requests=[];prior_normal=None;flips=unchanged=short=0
    for i in range(count):
        r=bytearray(raw[44+i*64:108+i*64]);o=base+0x224+i*retained.STRIDE+0x800
        q=answer[o:o+0xa0]
        normal=r[8:24] if u(r,4) else prior_normal
        inverted=b''.join(struct.pack('<f',f32(-x)) for x in struct.unpack('<4f',normal))
        a,b=r[48:64],r[24:40]
        if q[0x10:0x20]==normal:
            assert q[0x90:]==a and q[0x20:0x30]==b;unchanged+=1
        else:
            assert q[0x10:0x20]==inverted and q[0x90:]==b and q[0x20:0x30]==a;flips+=1
        assert q[0x30:0x38]==r[40:48]
        if a==q[0x60:0x70]:short+=1
        prior_normal=q[0x10:0x20]
        struct.pack_into('<I',r,4,1);r[8:24]=q[0x10:0x20];r[24:40]=q[0x20:0x30]
        requests.append(bytes(r[:48]));normalized[o+0x90:o+0xa0]=bytes(16)
    normalized[base+0x90:base+0xa0]=bytes(16)
    virtual=b'PC09'+raw[4:44]+b''.join(requests)+raw[s:]
    observed=retained.check(normalized,virtual)
    observed.update(orientation_flips=flips,orientation_unchanged=unchanged,zero_triangle_displacements=short)
    return observed


def guards(raw):
    yield 'short',raw[:43]
    yield 'long',raw+b'\0'
    s=44+u(raw,40)*64
    for label,o,v in [('count-zero',40,0),('count-65',40,65),('capacity',12,17),('first-retain',48,0),
                      ('mode',48,2),('triangle',44,0xffffffff),('normal-nan',52,0x7fc00000),('normal-zero-y',56,0),
                      ('normal-w',64,0x3f800000),('point-nan',68,0x7fc00000),('point-w',80,0),
                      ('depth-nan',84,0x7fc00000),('flag',88,2),('witness-nan',92,0x7fc00000),('witness-w',104,0),
                      ('late-witness-nan',44+7*64+48,0x7fc00000),('late-flag',44+7*64+44,2),
                      ('margin-negative',s+0x220,0xbf800000),('pose-nan',s+0x130,0x7fc00000)]:
        b=bytearray(raw);struct.pack_into('<I',b,o,v);yield label,bytes(b)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--max-source',type=int,default=245)
    p.add_argument('--controls',type=int,default=24);p.add_argument('--debug-only',action='store_true');a=p.parse_args()
    assert 1<=a.max_source<=245 and 0<=a.controls<=24
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    old=json.loads(retained.cold.REFERENCE.read_text());assert old['passed'] and digest(IMAGE)==old['image_sha256']
    upstream,chunks=source_chunks();profiles=['debug'] if a.debug_only else ['debug','release'];rows=[]
    for info in cases(a.max_source,a.controls):
        source=old['results'][info['source_index']];origin=retained.cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(origin)==source['input_sha256'];raw=request(origin.read_bytes(),info,upstream,chunks);i=len(rows)
        inp=out/f'{i:04}-input.bin';inp.write_bytes(raw);answers=[];executions=[]
        for name,exe,native in [('original',NATIVE,True),('repeat',NATIVE,True)]+[(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in profiles]:
            target=out/f'{i:04}-{name}.bin';e=run(exe,inp,target,native);executions.append(e)
            if e['exit']:
                (out/'failure.json').write_text(json.dumps(dict(index=i,name=name,executions=executions),indent=2));raise RuntimeError((i,name,e))
            answers.append(target.read_bytes())
        if answers[0]!=answers[1] or any(answer!=projection(answers[0],raw) for answer in answers[2:]):
            (out/'failure.json').write_text(json.dumps(dict(index=i,info=info,executions=executions),indent=2));raise RuntimeError(f'oriented manifold mismatch {i}')
        observed=check(answers[0],raw)
        rows.append(dict(index=i,**info,**observed,executions=executions,input_sha256=digest(inp),output_sha256=digest(out/f'{i:04}-original.bin'),rust_sha256=digest(out/f'{i:04}-debug.bin'),bytes=len(answers[0])))
        if len(rows)%100==0:print(f'cases={len(rows)} flips={sum(r["orientation_flips"] for r in rows)}',flush=True)
    rejected=[]
    for label,raw in guards((out/'0000-input.bin').read_bytes()):
        inp=out/f'guard-{label}-input.bin';inp.write_bytes(raw);executions=[]
        for name,exe,native in [('original',NATIVE,True)]+[(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in profiles]:
            target=out/f'guard-{label}-{name}.bin';e=run(exe,inp,target,native);assert 0<e['exit']<256 and not target.exists(),(label,name,e);executions.append(e)
        rejected.append(dict(label=label,input_sha256=digest(inp),executions=executions,no_output=True))
    for name,exe,native in [('original',NATIVE,True)]+[(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in profiles]:
        target=out/f'guard-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
        e=run(exe,out/'0000-input.bin',target,native);assert e['exit'] and digest(target)==before
        rejected.append(dict(label=f'existing-{name}',executions=[e],unchanged_sha256=before))
    totals={k:sum(r[k] for r in rows) for k in ['intervals','contacts','depth_merges','preserved_records','orientation_flips','orientation_unchanged','zero_triangle_displacements','bytes']}
    report=dict(passed=True,cases=len(rows),results=rows,totals=totals,guards=rejected,guard_count=len(rejected),max_source=a.max_source,controls=a.controls,profiles=profiles,
                reference_sha256=digest(retained.cold.REFERENCE),upstream_sha256=digest(retained.cold.distance.UPSTREAM),
                image_sha256=digest(IMAGE),native_sha256=digest(NATIVE),rust_hashes={p:digest(ROOT/f'target/{p}/grid-convex-distance.exe') for p in profiles},script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(dict(passed=True,**totals,guards=len(rejected))),flush=True)


if __name__=='__main__':main()
