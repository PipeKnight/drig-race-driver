"""Compose whole original geometry/input/coefficient routines versus Rust."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    geometry_dir=ROOT/'local/behavior/steering-geometry-20261008-01'
    coeff_dir=ROOT/'local/behavior/steering-coefficients-20261008-03'
    inputs=[]
    provenance=[]
    for directory in [geometry_dir,coeff_dir]:
        result=json.loads((directory/'validation.json').read_text())
        assert not result['differences'] and result['cases']==result['exact_cases']
        source=directory/'input.tsv'
        assert digest(source)==result['input_sha256']
        inputs.append([line.split('\t') for line in source.read_text().splitlines()])
        provenance.append(dict(input=str(source.relative_to(ROOT)),sha256=digest(source),validation_sha256=digest(directory/'validation.json')))
    cases=[]
    for i,geometry in enumerate(inputs[0]):
        for j in range(3):
            row=inputs[1][(i*3+j)%len(inputs[1])][1:].copy()
            # Wrong but valid placeholders demonstrate actual derived spans/pair/X are consumed.
            row[5:7]=['3f000000','3f800000']
            row[11:13]=['00000000','3f800000']
            for w in range(4):row[13+w*10]='00000000'
            row+=geometry[2:]
            assert len(row)==69
            cases.append((f'{geometry[0]}_condition_{j}',row))
    requests=out/'input.tsv'
    requests.write_text(''.join(name+'\t'+'\t'.join(row)+'\n' for name,row in cases))
    executions=[]
    for kind,exe,argv in [
        ('original',ROOT/'local/behavior/steering-coefficients-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv','--geometry']),
        ('native',ROOT/f'target/{args.profile}/grid-steering-coefficients.exe',[requests,out/'native.tsv','--geometry'])]:
        run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Steering pipeline execution failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t')
            assert len(cells)==27 and cells[0]=='P' and cells[1] not in result
            result[cells[1]]=cells[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert original.keys()==native.keys()=={name for name,_ in cases}
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result=dict(profile=args.profile,cases=len(cases),fields=len(cases)*25,exact_cases=len(cases)-len({d['case'] for d in differences}),
                differences=differences,executions=executions,provenance=provenance,input_sha256=digest(requests),
                limits='Whole original 98ebf0 ->9b3110/98f790 with assistance disabled. Geometry from source/retained/authored fixtures, property340 and other tuning/state/delta supplied. '
                       'Deliberately different placeholder geometry ignored; final coefficients and derived pair/spans compared. No live/assisted steering or original source decoder claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','cases','fields','exact_cases')}))
    print('different_fields='+str(len(differences)))
    assert not differences
if __name__=='__main__': main()
