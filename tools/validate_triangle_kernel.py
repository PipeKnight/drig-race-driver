"""Compare independent f32 queries with isolated, identity-checked original code."""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path
from validate_vcqtc import chunks


def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]


def value(bits):
    return struct.unpack('<f',struct.pack('<I',int(bits,16)))[0]


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('game_root',type=Path)
    args=parser.parse_args()
    root=Path(__file__).resolve().parent.parent
    output=root/'local/behavior/triangle-kernel-validation'
    output.mkdir(exist_ok=False)
    source=root/'local/compatibility/grid-unpacked.exe'
    source_hash=hashlib.sha256(source.read_bytes()).hexdigest()
    assert source_hash=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    track=args.game_root/'tracks/europe/donington/route_0/track.jpk'
    raw=track.read_bytes()
    geometry={}
    for name,positions,triangles,_,_ in chunks(raw):
        for i,(indices,*_) in enumerate(triangles):
            geometry[(name,i)]=[positions[j] for j in indices]
    requests=[]
    for line in (root/'local/behavior/triangle-kernel-cases.tsv').read_text().splitlines():
        fields=line.split()
        requests.append((fields[0],[float(v) for v in fields[1:]]))
    ground=root/'local/behavior/ground-validation-edge'
    inputs={r[0]:[float(v) for v in r[1:]] for line in (ground/'queries.tsv').read_text().splitlines() if (r:=line.split('\t'))}
    for line in (ground/'native.tsv').read_text().splitlines():
        fields=line.split('\t')
        if fields[0]!='H':
            continue
        name=fields[1]
        coordinates=inputs[name]
        start=[f32(v) for v in coordinates[:3]]
        direction=[f32(coordinates[i+3]-coordinates[i]) for i in range(3)]
        triangle=geometry[(fields[2],int(fields[3]))]
        requests.append(('track-'+name,start+direction+[v for p in triangle for v in p]))
    triangle=[[0.,0.,0.],[1.,0.,0.],[0.,0.,1.]]
    for x in [-2e-6,-5e-7,0.,0.25,0.75,1.,1.0000005,1.000002]:
        requests.append((f'edge-{x}',[f32(x),1.,0.25,0.,-2.,0.]+[v for p in triangle for v in p]))
    for y in [-2e-6,-5e-7,0.,1.,1.0000005,1.000002]:
        requests.append((f'fraction-{y}',[0.25,f32(y),0.25,0.,-1.,0.]+[v for p in triangle for v in p]))
    for scale in [1e-3,1e-4,1e-5]:
        requests.append((f'small-{scale}',[scale/4,1.,scale/4,0.,-2.,0.]+[v*scale for p in triangle for v in p]))
    inputs_file=output/'cases.tsv'
    with inputs_file.open('x') as f:
        for name,coordinates in requests:
            f.write('\t'.join([name]+[format(f32(v),'.17g') for v in coordinates])+'\n')
    original=output/'original.tsv'
    native=output/'native.tsv'
    executions=[]
    for executable,arguments in [
        (root/'local/behavior/triangle-kernel-probe.exe',[source,inputs_file,original]),
        (root/'target/debug/grid-triangle.exe',[inputs_file,native])]:
        process=subprocess.run([str(executable),*map(str,arguments)],capture_output=True,text=True,timeout=30)
        executions.append(dict(executable=str(executable),sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                               exit=process.returncode,stdout=process.stdout,stderr=process.stderr))
        if process.returncode:
            (output/'failed-run.json').write_text(json.dumps(executions,indent=2))
            raise SystemExit('numeric comparison process failed')
    oracle=[r.split('\t') for r in original.read_text().splitlines()]
    environment=oracle.pop(0)
    ours=[r.split('\t') for r in native.read_text().splitlines()]
    assert len(oracle)==len(ours)==len(requests)
    results=[]
    for reference,actual in zip(oracle,ours):
        assert reference[1]==actual[1]
        same_hit=reference[0]==actual[0]
        differences=[abs(value(a)-value(b)) for a,b in zip(reference[2:],actual[2:])] if same_hit and reference[0]=='H' else []
        results.append(dict(name=reference[1],same_hit=same_hit,exact=reference==actual,
                            max_absolute_error=max(differences,default=0.),original=reference,native=actual))
    summary=dict(cases=len(results),track_cases=sum(r['name'].startswith('track-') for r in results),
                 same_classification=sum(r['same_hit'] for r in results),bit_exact=sum(r['exact'] for r in results),
                 maximum_absolute_error=max(r['max_absolute_error'] for r in results),
                 running_game_state_captured=False,contact_solver_equivalence=False)
    with (output/'validation.json').open('x') as f:
        json.dump(dict(summary=summary,reference_sha256=source_hash,track_sha256=hashlib.sha256(raw).hexdigest(),
                       isolated_environment=environment,executions=executions,results=results),f,indent=2)
    print(json.dumps(summary))
    assert summary['same_classification']==summary['cases']
    assert summary['maximum_absolute_error']<=1e-5
