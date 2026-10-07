"""Supplied source/analytic leaf order against whole original leaf consumer.
Original expansion, packed vertex decoder and triangle kernel all execute.
"""
import argparse, hashlib, json, math, struct, subprocess
from pathlib import Path
from validate_vcqtc import chunks
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def fixture():
    # Four coincident triangles, two layers and an independent continuation list.
    bounds=[0.,-2.,0.,2.,2.,2.]
    positions=[(0.,0.,0.),(1.,0.,0.),(0.,0.,1.),(0.,1.,0.),(1.,1.,0.),(0.,1.,1.)]
    packed=b''
    for point in positions:
        integers=[round((point[i]-bounds[i])/(bounds[i+3]-bounds[i])*(65536 if i==1 else 16777216)) for i in range(3)]
        packed+=integers[0].to_bytes(3,'big')+integers[1].to_bytes(2,'big')+integers[2].to_bytes(3,'big')
    tris=bytes([0,0,1,2])*4+bytes([0,0x30,1,2])+bytes([0,0x31,1,2])*255
    lists=[bytes.fromhex('000000ff'),bytes.fromhex('000004ff'),bytes.fromhex('00000400ff'),bytes.fromhex('0000fe01ff')]
    refs=b'';offsets=[]
    for r in lists:offsets.append(len(refs));refs+=r
    nodes=b''.join((0x8000+v).to_bytes(2,'big') for v in offsets)
    vo=60;no=vo+len(packed);to=no+len(nodes);ro=to+len(tris)
    raw=struct.pack('<6f3i4I',*bounds,-len(tris)//4,-len(positions),-2,vo,no,to,ro)+b'TESTTWO2'+packed+nodes+tris+refs
    return raw,offsets
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/tracks/europe/donington/route_0/track.jpk')
    manifest=json.loads((ROOT/'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    recorded=next(r['steam_sha256'] for r in manifest['resources'] if r['relative_path'].replace('\\','/').lower()=='tracks/europe/donington/route_0/track.jpk');assert digest(source)==recorded
    raw=source.read_bytes();sections={}
    for i in range(struct.unpack_from('<I',raw,8)[0]):
        w=struct.unpack_from('<8I',raw,32+i*32);name=raw[w[0]:raw.index(0,w[0])].decode();assert w[1]==w[3];sections[name]=raw[w[2]:w[2]+w[3]]
    jobs=[]
    for i,(name,positions,triangles,nodes,refs) in enumerate(chunks(raw)):
        if i%8:continue
        offset,order=next((off,order) for off,order in sorted(refs.items()) if len(order)<=256)
        # Centroid-normal segments exercise real sloped triangles without authored flattening.
        vertices=[positions[k] for k in triangles[order[0]][0]];a,b,c=vertices
        ab=[b[k]-a[k] for k in range(3)];ac=[c[k]-a[k] for k in range(3)]
        normal=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];length=math.sqrt(sum(v*v for v in normal))
        normal=[v/length if length else (1. if k==1 else 0.) for k,v in enumerate(normal)];center=[sum(v[k] for v in vertices)/3 for k in range(3)]
        cases=[]
        for j,distance in enumerate([1.,3.,10.]):
            start=[center[k]+normal[k]*distance for k in range(3)]+[1.];direction=[-normal[k]*distance*2 for k in range(3)]+[0.]
            cases.append((f'source_{i}_{j}',offset,start,direction))
        cases.append((f'source_{i}_miss',offset,[center[0]+100000,center[1]+10,center[2]+100000,1.],[0.,-20.,0.,0.]))
        jobs.append((f'source_{i:03d}',sections[name],cases,dict(name=name,leaf_offset=offset,leaf_triangles=len(order))))
    authored,offsets=fixture();cases=[]
    for i,offset in enumerate(offsets):
        for j,y in enumerate([0.,1.,2.,-5e-7,1.0000005,3.]):cases.append((f'authored_{i}_{j}',offset,[.25,y,.25,1.],[0.,-4.,0.,0.]))
    jobs.append(('authored',authored,cases,dict(authored=True)))
    executions=[];differences=[];total=hits=words=0;coverage=[]
    for label,data,cases,metadata in jobs:
        directory=out/label;directory.mkdir();chunk=directory/'chunk.vcqtc';chunk.write_bytes(data)
        requests=directory/'input.tsv';requests.write_text(''.join(name+'\t'+f'{off:08x}'+'\t'+'\t'.join(f'{word(v):08x}' for v in start+direction)+'\n' for name,off,start,direction in cases))
        for kind,exe,arguments in [('original',ROOT/'local/behavior/leaf-query-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',chunk,requests,directory/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-leaf-query.exe',[chunk,requests,directory/'native.tsv'])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(kind=kind,label=label,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
            if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Leaf comparison execution failed')
        original=(directory/'original.tsv').read_text().splitlines();native=(directory/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases)
        for a,b,c in zip(original,native,cases):
            assert a.split('\t')[1]==b.split('\t')[1]==c[0];total+=1;hits+=a.startswith('H');words+=7 if a.startswith('H') else 0
            if a!=b:differences.append(dict(case=c[0],original=a,native=b))
        coverage.append(dict(label=label,sha256=digest(chunk),cases=len(cases),**metadata))
    assert digest(source)==recorded
    report=dict(profile=args.profile,cases=total,hits=hits,words=words,exact=total-len(differences),coverage=coverage,source_path=str(source),source_sha256=recorded,executions=executions,differences=differences,limits='Whole original leaf consumer/expansion/decoder/kernel; supplied leaf order and shape bounds. No quadtree traversal, normal/material construction or live query fidelity claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','hits','words','exact']}));assert not differences
if __name__=='__main__':main()
