"""Whole original four-wheel contact with actual packed world queries; supplied mechanics."""
import argparse, hashlib, json, math, struct, subprocess
from pathlib import Path
from validate_vcqtc import chunks
from validate_binxml import decode
from validate_surface_mechanics import expected
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');parser.add_argument('--source-mechanics',action='store_true');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    source_world=ROOT/'local/behavior/source-world-20261007-01';record=json.loads((source_world/'validation.json').read_text());fixture=source_world/'world.private.bin';assert digest(fixture)==record['original_fixture_sha256']
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/tracks/europe/donington/route_0/track.jpk');assert digest(source)==record['source_sha256']
    retained=ROOT/'local/behavior/isolated-contact-material-20261007-06/input.tsv';assert retained.exists();cases=[]
    for line in retained.read_text().splitlines():
        fields=line.split();assert len(fields)==167;cases.append((fields[0],[int(v,16) for v in fields[1:]]))
    assert len(cases)==236
    # Twenty controlled source-centroid contacts, spanning real winding/slope/material geometry.
    baseline=cases[0][1];parsed=list(chunks(source.read_bytes()))
    for i in range(20):
        name,positions,triangles,nodes,refs=parsed[i*12];order=next(order for off,order in sorted(refs.items()) if len(order)<=256)
        points=[positions[k] for k in triangles[order[0]][0]];center=[sum(v[k] for v in points)/3 for k in range(3)]
        values=baseline.copy()
        def set_values(offset,seq):values[offset:offset+len(seq)]=list(map(word,seq))
        set_values(0,[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.])
        for w in range(4):set_values(16+w*4,[center[0],center[1]+.3,center[2],1.]);set_values(32+w*4,[0.]*4);set_values(60+w*4,[0.,1.,0.,0.])
        set_values(48,[.3]*4);set_values(52,[-.1]*4);set_values(56,[.2]*4)
        set_values(124,[float(i%2),2.25,.01,1.,1.,.02]);set_values(130,[i*.5,1.,.8]);set_values(157,[float(i%2)])
        cases.append((f'centroid_{i}',values))
    source_mechanics=None;mechanics_fixture=None;source_rows=[]
    if args.source_mechanics:
        source_mechanics=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
        assert digest(source_mechanics)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
        _,root,_=decode(source_mechanics.read_bytes());source_rows=expected(root)
        mechanics_fixture=out/'mechanics.private.bin'
        mechanics_fixture.write_bytes(b'MEC1'+struct.pack('<I',len(source_rows))+b''.join(struct.pack('<8I',*[int(v,16) for v in row[1:9]]) for row in source_rows))
        eligible={}
        for _,positions,triangles,_,_ in parsed:
            for indices,_,_,code in triangles:
                points=[positions[k] for k in indices];ab=[points[1][k]-points[0][k] for k in range(3)];ac=[points[2][k]-points[0][k] for k in range(3)]
                cross=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];norm=math.sqrt(sum(v*v for v in cross))
                if norm and cross[1]/norm>.65:eligible.setdefault(code,[]).append([sum(p[k] for p in points)/3 for k in range(3)])
        assert len(eligible)==8
        for code,centers in sorted(eligible.items()):
            for j in range(min(32,len(centers))):
                center=centers[j*len(centers)//min(32,len(centers))];values=baseline.copy()
                def put(offset,seq):values[offset:offset+len(seq)]=list(map(word,seq))
                put(0,[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.])
                for w in range(4):put(16+w*4,[center[0],center[1]+.3,center[2],1.]);put(32+w*4,[0.]*4);put(60+w*4,[0.,1.,0.,0.])
                put(48,[.3]*4);put(52,[-.1]*4);put(56,[.2]*4)
                put(124,[0.,2.25,.01,1.,1.,.02]);put(130,[j*.75,j*.13,[-1.,0.,.8,2.][j%4]]);put(157,[float(j%2)])
                cases.append((f'surface_{code}_{j}',values))
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{v:08x}' for v in values)+'\n' for name,values in cases));executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/track-contact-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',fixture,requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-track-contact.exe',[source,requests,out/'native.tsv'])]:
        if args.source_mechanics:arguments.append(mechanics_fixture if kind=='original' else source_mechanics)
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=60);executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Track contact comparison execution failed')
    original=(out/'original.tsv').read_text().splitlines();native=(out/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases);differences=[];accepted=words=0
    stride=15 if args.source_mechanics else 14;coverage={}
    for a,b,c in zip(original,native,cases):
        fields=a.split('\t');assert fields[1]==b.split('\t')[1]==c[0] and len(fields)==2+stride*4;words+=stride*4
        accepted+=sum(int(fields[2+w*stride+11],16)!=0 for w in range(4))
        if args.source_mechanics:
            for w in range(4):
                index=int(fields[2+w*stride+14],16)
                if index:
                    key=source_rows[index-1][1];coverage[key]=coverage.get(key,0)+1
        if a!=b:differences.append(dict(case=c[0],original=a,native=b))
    assert digest(source)==record['source_sha256'];report=dict(profile=args.profile,cases=len(cases),wheels=len(cases)*4,accepted_wheels=accepted,words=words,exact=len(cases)-len(differences),source_sha256=record['source_sha256'],fixture_sha256=digest(fixture),retained_inputs_sha256=digest(retained),executions=executions,differences=differences,limits='Whole original9c1660 with unstubbed96b7f0/collector/dispatch/packed geometry/normal/material helpers. Source-built world; one supplied mechanics record applied uniformly per case, source material-specific mechanics mapping not claimed. Incoming state supplied, no synchronized live query/tyre forces/driving equivalence.')
    if args.source_mechanics:
        report.update(source_mechanics_sha256=digest(source_mechanics),mechanics_fixture_sha256=digest(mechanics_fixture),accepted_source_key_counts=coverage,limits='Whole original9c1660 with realworldqueries and b3b0c0 resolving source-ordered mechanics keys/stride32. Authored mechanics records from independent binary XML fields/defaults; context offset0. Native reads installed track and surface XML. Not whole original source numeric parser, nonzero context offset, synchronized live contact/tyre force/driving equivalence.')
        assert digest(source_mechanics)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','wheels','accepted_wheels','words','exact']}));assert not differences
    if args.source_mechanics:
        required={int.from_bytes(bytes.fromhex(code),'little') for code in eligible}
        installed_keys=[int(row[1],16) for row in source_rows]
        resolved={code if code in installed_keys else installed_keys[0] for code in required}
        assert {int(code,16) for code in coverage}==resolved
if __name__=='__main__':main()
