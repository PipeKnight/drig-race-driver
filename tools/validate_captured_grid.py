"""Original candidate collector on retained production grid, compared offline.
Cell tokens remain inert IDs; shape traversal/live query behavior is separate.
"""
import argparse, hashlib, json, random, struct, subprocess
from pathlib import Path
from validate_candidate_grid import f32, word
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    snapshot=ROOT/'local/behavior/runtime-collision-20261007T1646001077249Z.json';assert digest(snapshot)=='475fbbaa73cfd25b6bbf33036fcb056b1662db117aa5f95e61d38c468b55befd'
    linkage=ROOT/'local/behavior/runtime-grid-20261007-01/validation.json';record=json.loads(linkage.read_text());assert record['snapshot_sha256']==digest(snapshot) and record['summary']['matched']==245
    fixture=linkage.parent/'grid.private.bin';assert digest(fixture)==record['grid_fixture_sha256']
    raw=fixture.read_bytes();assert raw[:4]==b'GRC1';bounds=list(struct.unpack_from('<4f',raw,4));cursor=20;tiles=[]
    for _ in range(1024):
        exponent,width=struct.unpack_from('<2I',raw,cursor);cursor+=8;assert width==1<<exponent
        n=width*width;cells=list(struct.unpack_from('<'+'I'*n,raw,cursor));cursor+=n*4;tiles.append((exponent,width,cells))
    assert cursor==len(raw)
    def expected(low,high,capacity,active):
        def coordinate(p,axis):return max(0,min(65535,int(f32(f32(f32(p-bounds[axis])/f32(bounds[axis+2]-bounds[axis]))*65536))))
        a=[coordinate(low[i],i) for i in range(2)];b=[coordinate(high[i],i) for i in range(2)];ids=[]
        if active and capacity:
            for tz in range(a[1]//2048,b[1]//2048+1):
                for tx in range(a[0]//2048,b[0]//2048+1):
                    e,width,cells=tiles[tz*32+tx];step=2048//width
                    starts=[max(0,a[0]//step-tx*width),max(0,a[1]//step-tz*width)]
                    ends=[min(width-1,b[0]//step-tx*width),min(width-1,b[1]//step-tz*width)]
                    for z in range(starts[1],ends[1]+1):
                        for x in range(starts[0],ends[0]+1):
                            token=cells[z*width+x]
                            if token and token not in ids:
                                if len(ids)==capacity:return a+b+[len(ids)]+ids
                                ids.append(token)
        return a+b+[len(ids)]+ids
    cases=[];extent=[bounds[i+2]-bounds[i] for i in range(2)]
    def add(name,low,high,capacity=25,active=1):
        low=list(map(f32,low));high=list(map(f32,high));values=bounds+low+high
        cases.append((name,list(map(word,values))+[capacity,0,0,active],expected(low,high,capacity,active)))
    for z in range(32):
        for x in range(32):
            point=[bounds[0]+extent[0]*(x+.5)/32,bounds[1]+extent[1]*(z+.5)/32]
            add(f'tile_{x}_{z}',point,point)
    for i in range(32):
        boundary=[bounds[0]+extent[0]*i/32,bounds[1]+extent[1]*i/32];add(f'boundary_{i}',boundary,boundary)
    for i,capacity in enumerate([0,1,2,24,25]):add(f'whole_{i}',bounds[:2],bounds[2:],capacity)
    add('inactive',bounds[:2],bounds[2:],25,0)
    add('outside_low',[bounds[i]-extent[i] for i in range(2)],bounds[:2]);add('outside_high',bounds[2:],[bounds[i+2]+extent[i] for i in range(2)])
    rng=random.Random(2452008)
    for i in range(128):
        xs=sorted([bounds[0]+extent[0]*rng.uniform(-.1,1.1) for _ in range(2)]);zs=sorted([bounds[1]+extent[1]*rng.uniform(-.1,1.1) for _ in range(2)])
        add(f'rectangle_{i}',[xs[0],zs[0]],[xs[1],zs[1]],rng.randrange(26))
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words,_ in cases))
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/candidate-grid-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv',fixture]),('native',ROOT/f'target/{args.profile}/grid-candidate-grid.exe',[requests,out/'native.tsv',fixture])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Captured-grid comparison failed')
    def rows(path):
        results={}
        for line in path.read_text().splitlines():
            a=line.split('\t');assert a[0]=='P' and a[1] not in results;results[a[1]]=[int(w,16) for w in a[2:]]
        return results
    original=rows(out/'original.tsv');native=rows(out/'native.tsv');assert set(original)==set(native)=={c[0] for c in cases}
    diffs=[dict(name=name,original=original[name],native=native[name],independent=reference) for name,_,reference in cases if not original[name]==native[name]==reference]
    malformed_width=bytearray(raw);struct.pack_into('<I',malformed_width,24,2)
    guard_checks=[]
    for label,data in [('truncated',raw[:-1]),('trailing',raw+b'X'),('width_mismatch',malformed_width)]:
        path=out/f'{label}.bin';path.write_bytes(data)
        for kind,exe,arguments in [('original',ROOT/'local/behavior/candidate-grid-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/f'{label}-original.tsv',path]),('native',ROOT/f'target/{args.profile}/grid-candidate-grid.exe',[requests,out/f'{label}-native.tsv',path])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            assert result.returncode==(21 if kind=='original' else 1) and not arguments[-2].exists()
            guard_checks.append(dict(kind=kind,label=label,exit=result.returncode,stderr=result.stderr))
    report=dict(profile=args.profile,cases=len(cases),words=sum(len(c[2]) for c in cases),exact=len(cases)-len(diffs),executions=executions,differences=diffs,authored_fixture_guard_checks=guard_checks,snapshot_sha256=digest(snapshot),fixture_sha256=digest(fixture),limits='Offline original whole collector with retained production bounds/widths/exponents/cells. Original grid construction, shape intersection, live query timing and material mechanics ownership remain separate.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','words','exact']}));assert not diffs
if __name__=='__main__':main()
