"""Whole original world collector/dispatch and packed-shape queries, private inputs."""
import argparse, hashlib, json, math, random, struct, subprocess
from pathlib import Path
from validate_vcqtc import chunks
from validate_leaf_query import fixture
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def world(bounds,tiles,sections):
    data=b'WQG1'+struct.pack('<4fI',*bounds,len(sections))
    for exponent,cells in tiles:
        assert len(cells)==1<<(2*exponent)
        data+=struct.pack('<II',exponent,1<<exponent)+struct.pack('<'+'I'*len(cells),*cells)
    return data+b''.join(struct.pack('<I',len(s))+s for s in sections)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    linkage_path=ROOT/'local/behavior/runtime-grid-20261007-01/validation.json';linkage=json.loads(linkage_path.read_text())
    snapshot=Path(linkage['snapshot_path']);assert digest(snapshot)==linkage['snapshot_sha256']=='475fbbaa73cfd25b6bbf33036fcb056b1662db117aa5f95e61d38c468b55befd'
    grid_path=linkage_path.parent/'grid.private.bin';assert digest(grid_path)==linkage['grid_fixture_sha256']
    source=Path(linkage['source_path']);assert digest(source)==linkage['source_sha256'];raw=source.read_bytes();sections={}
    for i in range(struct.unpack_from('<I',raw,8)[0]):
        w=struct.unpack_from('<8I',raw,32+i*32);assert w[1]==w[3];name=raw[w[0]:raw.index(0,w[0])].decode();sections[name]=raw[w[2]:w[2]+w[3]]
    records=linkage['results'];assert len(records)==245 and all(r['transform_pointer']=='00000000' for r in records)
    ids={int(r['wrapper'],16):i+1 for i,r in enumerate(records)}
    captured=grid_path.read_bytes();assert captured[:4]==b'GRC1';bounds=struct.unpack_from('<4f',captured,4);cursor=20;tiles=[]
    for i in range(1024):
        exponent,width=struct.unpack_from('<II',captured,cursor);cursor+=8;n=width*width;cells=struct.unpack_from('<'+'I'*n,captured,cursor);cursor+=n*4;tiles.append((exponent,[ids[v] if v else 0 for v in cells]))
    assert cursor==len(captured)
    parsed={name:(positions,triangles,refs) for name,positions,triangles,nodes,refs in chunks(raw)}
    cases=[]
    for i,r in enumerate(records):
        positions,triangles,refs=parsed[r['source_chunk']];order=next(order for off,order in sorted(refs.items()) if len(order)<=256)
        a,b,c=[positions[k] for k in triangles[order[0]][0]];ab=[b[k]-a[k] for k in range(3)];ac=[c[k]-a[k] for k in range(3)]
        normal=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];length=math.sqrt(sum(v*v for v in normal));normal=[v/length if length else (1. if k==1 else 0.) for k,v in enumerate(normal)];center=[(a[k]+b[k]+c[k])/3 for k in range(3)]
        for j,d in enumerate([1.,3.,10.]):cases.append((f'shape_{i}_{j}',[center[k]+normal[k]*d for k in range(3)]+[1.],[center[k]-normal[k]*d for k in range(3)]+[1.],(i+j)%4,1))
    x0,z0,x1,z1=bounds
    for tile in range(1024):
        x=x0+(x1-x0)*((tile%32)+.5)/32;z=z0+(z1-z0)*((tile//32)+.5)/32
        cases.append((f'tile_{tile}',[x,150.,z,1.],[x,-100.,z,1.],tile%4,1))
    rng=random.Random(2026100791)
    for i in range(192):
        start=[rng.uniform(x0-10,x1+10),rng.uniform(0,160),rng.uniform(z0-10,z1+10),1.]
        end=[start[0]+rng.uniform(-100,100),rng.uniform(-100,20),start[2]+rng.uniform(-100,100),1.]
        cases.append((f'segment_{i}',start,end,i%4,1))
    for i in range(16):cases.append((f'inactive_{i}',cases[i][1],cases[i][2],i%4,0))
    jobs=[('captured',world(bounds,tiles,[sections[r['source_chunk']] for r in records]),cases)]
    # Thirty overlapping authored shapes: tied hits, later closer layers and capacity.
    chunk,offsets=fixture();authored=[]
    for i in range(30):
        data=bytearray(chunk);data[52:56]=f'{i+1:04d}'.encode()
        if i in (1,28):
            b=list(struct.unpack_from('<6f',data));b[1]+=.5;b[4]+=.5;struct.pack_into('<6f',data,0,*b)
        authored.append(bytes(data))
    tiled=[(0,[(i%30)+1]) for i in range(1024)];controls=[]
    for i,(start,end) in enumerate([
        ([.2,1.,.2,1.],[.4,-1.,.2,1.]),
        ([.2,1.,.2,1.],[1.2,-3.,.2,1.]),
        ([.1,1.,.1,1.],[31.,-100.,.1,1.]),
        ([.2,0.,.2,1.],[31.,-100.,.1,1.]),
        ([31.,1.,.1,1.],[.1,-100.,.1,1.]),
        ([100.,1.,100.,1.],[100.,-1.,100.,1.]),
    ]):
        for plane in range(4):controls.append((f'control_{i}_{plane}',start,end,plane,1))
    jobs.append(('authored',world([0.,0.,32.,32.],tiled,authored),controls))
    executions=[];differences=[];total=hits=words=0;coverage=[];caps=multi=0
    for label,data,cases in jobs:
        directory=out/label;directory.mkdir();fixture_path=directory/'world.private.bin';fixture_path.write_bytes(data);requests=directory/'input.tsv'
        requests.write_text(''.join(name+'\t'+'\t'.join(f'{word(v):08x}' for v in start+end)+f'\t{plane:08x}\t{active:08x}\n' for name,start,end,plane,active in cases))
        for kind,exe,arguments in [('original',ROOT/'local/behavior/packed-world-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',fixture_path,requests,directory/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-packed-world.exe',[fixture_path,requests,directory/'native.tsv'])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=60)
            executions.append(dict(kind=kind,label=label,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
            if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('World comparison execution failed')
        original=(directory/'original.tsv').read_text().splitlines();native=(directory/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases)
        for a,b,c in zip(original,native,cases):
            aw=a.split('\t');assert aw[1]==b.split('\t')[1]==c[0];total+=1;hit=aw[0]=='H';hits+=hit;words+=len(aw)-2;candidate_count=int(aw[10 if hit else 3],16);caps+=candidate_count==25;multi+=candidate_count>1
            if a!=b:differences.append(dict(case=c[0],original=a,native=b))
        coverage.append(dict(label=label,sha256=digest(fixture_path),cases=len(cases)))
    assert digest(source)==linkage['source_sha256']
    report=dict(profile=args.profile,cases=total,hits=hits,words=words,exact=total-len(differences),capacity25_cases=caps,multiple_candidate_cases=multi,coverage=coverage,source_sha256=linkage['source_sha256'],snapshot_sha256=digest(snapshot),executions=executions,differences=differences,limits='Whole original96b7f0, collector, type11 dispatch, packed query. Supplied retained grid/source geometry and authored address relocation/material code tokens. Nodes/leaves from source, not captured live; no original population, nonzero transforms or synchronized live query fidelity.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','hits','words','exact','capacity25_cases','multiple_candidate_cases']}));assert not differences
if __name__=='__main__':main()
