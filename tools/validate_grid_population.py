"""Original name parsing/initialization/insertion and exact captured-grid reconstruction."""
import argparse, hashlib, json, random, struct, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def expected(names):
    paths=[];exponents=[0]*1024
    for name in names:
        parts=name[3:-6].split('_');x=z=0
        for i,pair in enumerate(parts):x+=int(pair[0])*(32768>>i);z+=int(pair[1])*(32768>>i)
        depth=len(parts);paths.append((x,z,depth));tile=(z//2048)*32+x//2048;exponents[tile]=max(exponents[tile],depth-5)
    cells=[[0]*(1<<(2*e)) for e in exponents]
    for id,(x,z,depth) in enumerate(paths,1):
        high=[x+(65536>>depth)-1,z+(65536>>depth)-1]
        for tz in range(z//2048,high[1]//2048+1):
            for tx in range(x//2048,high[0]//2048+1):
                tile=tz*32+tx;width=1<<exponents[tile];shift=11-exponents[tile]
                low=[max(0,(x>>shift)-tx*width),max(0,(z>>shift)-tz*width)];hi=[min(width-1,(high[0]>>shift)-tx*width),min(width-1,(high[1]>>shift)-tz*width)]
                for cz in range(low[1],hi[1]+1):
                    for cx in range(low[0],hi[0]+1):cells[tile][cz*width+cx]=id
    result=b'GBL1'+struct.pack('<I',len(paths))+b''.join(struct.pack('<3I',*p) for p in paths)
    for e,c in zip(exponents,cells):result+=struct.pack('<II',e,1<<e)+struct.pack('<'+'I'*len(c),*c)
    return result,cells,exponents
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    linkage=json.loads((ROOT/'local/behavior/runtime-grid-20261007-01/validation.json').read_text());source=Path(linkage['source_path']);assert digest(source)==linkage['source_sha256'];raw=source.read_bytes();names=[]
    for i in range(struct.unpack_from('<I',raw,8)[0]):
        w=struct.unpack_from('<8I',raw,32+i*32);names.append(raw[w[0]:raw.index(0,w[0])].decode());assert raw[w[2]:w[2]+24]==raw[struct.unpack_from('<8I',raw,32)[2]:struct.unpack_from('<8I',raw,32)[2]+24]
    jobs=[('source',names)]
    for depth in range(1,12):
        path='qt_'+'_'.join('10' if i%2 else '01' for i in range(depth))+'.vcqtc';jobs.append((f'depth_{depth}',[path]))
    parent='qt_10.vcqtc';child='qt_10_00_00_00_00_00.vcqtc';jobs.extend([('overlap',[parent,child]),('overlap_reverse',[child,parent])])
    rng=random.Random(2026100792)
    for i in range(64):
        count=rng.randrange(2,25);paths=[]
        for _ in range(count):paths.append('qt_'+'_'.join(rng.choice(['00','01','10','11']) for _ in range(rng.randrange(1,12)))+'.vcqtc')
        jobs.append((f'random_{i}',paths))
    executions=[];differences=[];words=paths_count=0;coverage=[];source_cells=None
    for label,names in jobs:
        directory=out/label;directory.mkdir();input_path=directory/'names.txt';input_path.write_text('\n'.join(names)+'\n');wanted,cells,exponents=expected(names);(directory/'expected.bin').write_bytes(wanted)
        for kind,exe,arguments in [('original',ROOT/'local/behavior/grid-population-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',input_path,directory/'original.bin']),('native',ROOT/f'target/{args.profile}/grid-population.exe',[input_path,directory/'native.bin'])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30);executions.append(dict(kind=kind,label=label,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
            if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Population comparison execution failed')
        a=(directory/'original.bin').read_bytes();b=(directory/'native.bin').read_bytes();words+=(len(wanted)-4)//4;paths_count+=len(names)
        if a!=b or a!=wanted:differences.append(dict(label=label,original_size=len(a),native_size=len(b),expected_size=len(wanted),original_sha256=hashlib.sha256(a).hexdigest(),native_sha256=hashlib.sha256(b).hexdigest()))
        coverage.append(dict(label=label,paths=len(names),cells=sum(map(len,cells)),max_exponent=max(exponents)))
        if label=='source':source_cells=cells;source_names=names
    # Compare all populated source cells to captured identities, normalized by source name.
    captured_path=ROOT/'local/behavior/runtime-grid-20261007-01/grid.private.bin';assert digest(captured_path)==linkage['grid_fixture_sha256'];captured=captured_path.read_bytes();cursor=20
    mapped={int(r['wrapper'],16):source_names.index(r['source_chunk'])+1 for r in linkage['results']};captured_differences=[]
    for tile in range(1024):
        exponent,width=struct.unpack_from('<II',captured,cursor);cursor+=8;ids=struct.unpack_from('<'+'I'*(width*width),captured,cursor);cursor+=width*width*4
        actual=[mapped[v] if v else 0 for v in ids]
        if actual!=source_cells[tile]:captured_differences.append(tile)
    assert digest(source)==linkage['source_sha256'];report=dict(profile=args.profile,datasets=len(jobs),paths=paths_count,words=words,exact_datasets=len(jobs)-len(differences),source_chunks=len(source_names),captured_tiles_exact=1024-len(captured_differences),source_nonempty_cells=sum(bool(v) for c in source_cells for v in c),source_sha256=linkage['source_sha256'],captured_fixture_sha256=digest(captured_path),executions=executions,coverage=coverage,differences=differences,captured_differences=captured_differences,limits='Whole original name parser, grid initializer and insertion with host allocation virtual method. Tile exponent selection is authored from static loader evidence, not execution of whole resource loader. Source cell identities match retained capture; no nonzero transforms, all-game content or synchronized physics boundary claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','datasets','paths','words','exact_datasets','captured_tiles_exact','source_nonempty_cells']}));assert not differences and not captured_differences
if __name__=='__main__':main()
