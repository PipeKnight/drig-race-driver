"""Original root cull and whole packed traversal on supplied source/analytic chunks.
Original quadrant, cull, leaf expansion, decoder and triangle kernel execute.
"""
import argparse, hashlib, json, math, struct, subprocess
from pathlib import Path
from validate_vcqtc import chunks
from validate_leaf_query import fixture
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
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
        bounds=struct.unpack_from('<6f',sections[name]);x0,y0,z0,x1,y1,z1=bounds
        for j,(u,v) in enumerate([(0,0),(0,1),(1,0),(1,1),(.5,.5),(.25,.25),(.75,.75),(.25,.75)]):
            cases.append((f'root_{i}_{j}',0,[x0+(x1-x0)*u,y1+10,z0+(z1-z0)*v,1.],[0.,y0-y1-20,0.,0.]))
        jobs.append((f'source_{i:03d}',sections[name],cases,dict(name=name,leaf_offset=offset,leaf_triangles=len(order))))
    authored,offsets=fixture();
    authored=bytearray(authored);no,to,ro=struct.unpack_from("<3I",authored,40);authored=authored[:no]+bytes.fromhex("0001")+authored[no:];struct.pack_into("<II",authored,44,to+2,ro+2);authored=bytes(authored);cases=[]
    for i,offset in enumerate(offsets):
        for j,y in enumerate([0.,1.,2.,-5e-7,1.0000005,3.]):cases.append((f'authored_{i}_{j}',offset,[.25,y,.25,1.],[0.,-4.,0.,0.]))
    jobs.append(('authored',authored,cases,dict(authored=True)))
    executions=[];differences=[];total=hits=words=0;coverage=[]
    for label,data,cases,metadata in jobs:
        directory=out/label;directory.mkdir();chunk=directory/'chunk.vcqtc';chunk.write_bytes(data)
        requests=directory/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{word(v):08x}' for v in start+[f32(start[k]+direction[k]) for k in range(4)])+'\t'+f'{i%4:08x}'+'\n' for i,(name,off,start,direction) in enumerate(cases)))
        for kind,exe,arguments in [('original',ROOT/'local/behavior/packed-query-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',chunk,requests,directory/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-packed-query.exe',[chunk,requests,directory/'native.tsv'])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(kind=kind,label=label,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
            if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Packed traversal comparison failed')
        original=(directory/'original.tsv').read_text().splitlines();native=(directory/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases)
        for a,b,c in zip(original,native,cases):
            assert a.split('\t')[1]==b.split('\t')[1]==c[0];total+=1;hits+=a.startswith('H');words+=8 if a.startswith('H') else 1
            if a!=b:differences.append(dict(case=c[0],original=a,native=b))
        coverage.append(dict(label=label,sha256=digest(chunk),cases=len(cases),**metadata))
    assert digest(source)==recorded
    report=dict(profile=args.profile,cases=total,hits=hits,words=words,exact=total-len(differences),coverage=coverage,source_path=str(source),source_sha256=recorded,executions=executions,differences=differences,limits='Original root cull and whole traversal with original quadrant/leaf/decoder/kernel. Supplied original source nodes and leaf lists; no normal/material construction, transform or live whole-world query claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','hits','words','exact']}));assert not differences
if __name__=='__main__':main()
