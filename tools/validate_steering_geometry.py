"""Whole original four-wheel steering initialization with source/retained points."""
import argparse
import csv
import hashlib
import itertools
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_force_view import SNAPSHOTS
from validate_wheel_geometry import layout
from validate_binxml import decode

ROOT = Path(__file__).resolve().parents[1]
MANIFEST_SHA = '0a0a2f45301cfe8f45cad722187b8b55cb42c1f6890f2ae456f5c5b9ec70c859'
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def word(v):
    return struct.unpack('<I', struct.pack('<f', v))[0]
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    cases, provenance=[],[]
    manifest_path=ROOT/'local/behavior/native-wheel-geometry-validation-v3/validation.json'
    assert digest(manifest_path)==MANIFEST_SHA
    manifest=json.loads(manifest_path.read_text())
    for i, source in enumerate(manifest['sources']):
        game=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
        csv_path=game/source['source']
        assert digest(csv_path)==source['csv_sha256'] and digest(csv_path.with_suffix('.ned'))==source['ned_sha256']
        csv_rows=[r for r in csv.reader(csv_path.read_text().splitlines()) if r]
        attributes=[e.attrib for e in decode(csv_path.with_suffix('.ned').read_bytes())[1].iter() if e.tag=='WHEELS']
        assert len(attributes)==1 and len(csv_rows)==source['rows']
        capture=manifest_path.parent/f'{i:02d}-native.tsv'
        rows=[line.split('\t') for line in capture.read_text().splitlines()]
        for ordinal in range(source['rows']):
            points=[row for row in rows if row[:2]==['P',str(ordinal)]]
            assert [p[2] for p in points]==['0','1','2','3']
            source_points=layout(attributes[0],csv_rows[ordinal])[3]
            assert [[word(v) for v in p] for p in source_points]==[[int(w,16) for w in p[3:7]] for p in points]
            cases.append((f'source_{i}_{ordinal}',[word(.5)]+[int(w,16) for p in points for w in p[3:7]]))
        provenance.append(dict(source=source['source'],csv_sha256=source['csv_sha256'],ned_sha256=source['ned_sha256'],
                               fixture_sha256=digest(capture),original_wheel_points_previously_verified=True,
                               scale_property_supplied=True))
    for s,(name,sha) in enumerate(SNAPSHOTS):
        path=ROOT/'local/behavior'/name
        assert digest(path)==sha
        data=json.loads(path.read_text(encoding='utf-8-sig'))
        assert data['read_only'] and data['live_count_code_exact'] and len(data['managers'])==1
        blocks={b['name']:bytes.fromhex(b['hex']) for b in data['blocks'] if b['success'] and b['read']==b['length']}
        for car in data['managers'][0]['cars']:
            assert car['wheel_count']==4
            wheels=blocks[f"car_{car['index']}-wheel_records"]
            row=[word(.5)]
            for w in range(4):row+=list(struct.unpack_from('<4I',wheels,w*0x150))
            cases.append((f"retained_{s}_{car['index']}",row))
        provenance.append(dict(snapshot=name,sha256=sha,non_atomic=True,scale_property_supplied=True,
                               live_initializer_output_captured=False))
    base=[[.8,.2,-1.2,1.],[-.8,.2,-1.2,1.],[.75,.3,1.3,1.],[-.75,.3,1.3,1.]]
    for i in range(32):
        points=[p.copy() for p in base]
        for j,p in enumerate(points):
            p[0]+= [-.2,-.1,0.,.1,.2][i%5]
            p[2]+= [-.3,0.,.3][i%3]
            p[1],p[3]=[-100.,-0.,0.,100.][j],[-0.,0.,1.,-1.][j]
        cases.append((f'analytic_{i}',[word(-0. if i%2 else .7)]+[word(v) for p in points for v in p]))
    rng=random.Random(118)
    for i in range(512):
        points=[]
        for j in range(4):
            points.append([rng.uniform(.6,1.)*(1. if j%2==0 else -1.),rng.uniform(-2.,2.),
                           rng.uniform(1.,2.)*(1. if j>=2 else -1.),rng.uniform(-2.,2.)])
        # Bounded asymmetric geometry with both selected sides; no arbitrary malformed kernel calls.
        cases.append((f'authored_{i}',[word(rng.uniform(-1.,2.))]+[word(v) for p in points for v in p]))
    requests=out/'input.tsv'
    requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in row)+'\n' for name,row in cases))
    executions=[]
    for kind,exe,argv in [
        ('original',ROOT/'local/behavior/steering-geometry-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv']),
        ('native',ROOT/f'target/{args.profile}/grid-steering-geometry.exe',[requests,out/'native.tsv'])]:
        run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Steering geometry execution failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t')
            assert len(cells)==7 and cells[0]=='P' and cells[1] not in result
            result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert original.keys()==native.keys()=={name for name,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,cases=len(cases),fields=len(cases)*5,exact_cases=len(cases)-len({d['case'] for d in differences}),
                differences=differences,executions=executions,provenance=provenance,input_sha256=digest(requests),
                source_pair_counts={str(pair):sum(1 for name,row in original.items() if name.startswith('source_') and row[1:3]==list(pair)) for pair in itertools.permutations('0123',2)},
                limits='Whole original 98ebf0, four wheels only; source point fixtures previously verified, retained inputs non-atomic. '
                       'Property340 supplied, not source decoded. No live initializer output capture, six-wheel semantics, assisted steering or cadence claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','cases','fields','exact_cases')}))
    print('different_fields='+str(len(differences)))
    assert not differences
if __name__=='__main__': main()
