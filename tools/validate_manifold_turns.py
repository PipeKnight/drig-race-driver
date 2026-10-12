"""Retain a manifold while authored per-triangle normals actually change."""
import argparse
import json
import struct
from pathlib import Path

import validate_manifold_sequence as sequence
from validate_manifold_sequence import ROOT, digest, run


def cases():
    for k in range(24):
        for steps in [8,16,64]:
            for capacity in [3,16]:
                for pattern in [0,1]:
                    yield dict(source_index=k*1225//24,kind='turns',steps=steps,normal=0,capacity=capacity,pattern=pattern)


def request(raw, info):
    b=bytearray(sequence.request(raw,info))
    for i in range(info['steps']):
        o=44+i*48
        # All steps replace the normal. One pattern cycles all controls; the
        # other reverses the same normal each call, including signed zero lanes.
        n=i%8 if info['pattern']==0 else i%2
        struct.pack_into('<I4f',b,o+4,1,*sequence.cold.NORMALS[n])
    return bytes(b)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    previous=json.loads(sequence.cold.REFERENCE.read_text());assert previous['passed']
    rows=[]
    for info in cases():
        source=previous['results'][info['source_index']];origin=sequence.cold.REFERENCE.parent/f"{source['index']:04}-input.bin"
        assert digest(origin)==source['input_sha256'];raw=request(origin.read_bytes(),info);i=len(rows)
        inp=out/f'{i:04}-input.bin';inp.write_bytes(raw);answers=[];executions=[]
        for name,exe,native in [('original',sequence.NATIVE,True),('repeat',sequence.NATIVE,True),
                                ('debug',ROOT/'target/debug/grid-convex-distance.exe',False),('release',ROOT/'target/release/grid-convex-distance.exe',False)]:
            target=out/f'{i:04}-{name}.bin';e=run(exe,inp,target,native);assert e['exit']==0,(i,name,e)
            executions.append(e);answers.append(target.read_bytes())
        assert answers[0]==answers[1] and sequence.projection(answers[0],raw)==answers[2]==answers[3],i
        observed=sequence.check(answers[0],raw)
        rows.append(dict(index=i,**info,**observed,executions=executions,input_sha256=digest(inp),output_sha256=digest(out/f'{i:04}-original.bin'),rust_sha256=digest(out/f'{i:04}-debug.bin'),bytes=len(answers[0])))
        if len(rows)%50==0:print(f'cases={len(rows)}',flush=True)
    totals={k:sum(r[k] for r in rows) for k in ['intervals','contacts','depth_merges','preserved_records','full_intervals','bytes']}
    report=dict(passed=True,results=rows,cases=len(rows),totals=totals,reference_sha256=digest(sequence.cold.REFERENCE),
                native_sha256=digest(sequence.NATIVE),image_sha256=digest(sequence.IMAGE),
                debug_sha256=digest(ROOT/'target/debug/grid-convex-distance.exe'),release_sha256=digest(ROOT/'target/release/grid-convex-distance.exe'),
                script_sha256=digest(Path(__file__)),sequence_script_sha256=digest(ROOT/'tools/validate_manifold_sequence.py'))
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(dict(passed=True,**totals)),flush=True)


if __name__=='__main__':main()
