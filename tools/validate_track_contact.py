"""Whole original four-wheel contact with actual packed world queries; supplied mechanics."""
import argparse, hashlib, json, math, struct, subprocess
from pathlib import Path
from validate_vcqtc import chunks
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
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
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{v:08x}' for v in values)+'\n' for name,values in cases));executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/track-contact-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',fixture,requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-track-contact.exe',[source,requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=60);executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Track contact comparison execution failed')
    original=(out/'original.tsv').read_text().splitlines();native=(out/'native.tsv').read_text().splitlines();assert len(original)==len(native)==len(cases);differences=[];accepted=words=0
    for a,b,c in zip(original,native,cases):
        fields=a.split('\t');assert fields[1]==b.split('\t')[1]==c[0] and len(fields)==58;words+=56
        accepted+=sum(int(fields[2+w*14+11],16)!=0 for w in range(4))
        if a!=b:differences.append(dict(case=c[0],original=a,native=b))
    assert digest(source)==record['source_sha256'];report=dict(profile=args.profile,cases=len(cases),wheels=len(cases)*4,accepted_wheels=accepted,words=words,exact=len(cases)-len(differences),source_sha256=record['source_sha256'],fixture_sha256=digest(fixture),retained_inputs_sha256=digest(retained),executions=executions,differences=differences,limits='Whole original9c1660 with unstubbed96b7f0/collector/dispatch/packed geometry/normal/material helpers. Source-built world; one supplied mechanics record applied uniformly per case, source material-specific mechanics mapping not claimed. Incoming state supplied, no synchronized live query/tyre forces/driving equivalence.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','wheels','accepted_wheels','words','exact']}));assert not differences
if __name__=='__main__':main()
