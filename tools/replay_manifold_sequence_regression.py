"""All cold manifold cases and spaced earlier boundary regressions."""
import argparse
import json
from pathlib import Path

import replay_triangle_manifold_regression as old
from validate_triangle_manifold import ROOT, digest, run
import validate_triangle_mesh_outer as outer


def selections():
    for kind,reference,native_name in old.SPECS:
        report=json.loads(reference.read_text());assert report['passed']
        rows=report['results'] if kind=='manifold' else [report['results'][i*len(report['results'])//20] for i in range(20)]
        yield kind,reference,ROOT/'local/behavior'/native_name,report,rows


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows=[];specs=[]
    for kind,reference,native,report,selected in selections():
        specs.append(dict(kind=kind,reference=str(reference.relative_to(ROOT)),reference_sha256=digest(reference),native_sha256=digest(native),selected=[r['index'] for r in selected]))
        for source in selected:
            i=source['index'];inp=reference.parent/f'{i:04}-input.bin';original=reference.parent/f'{i:04}-original.bin'
            assert digest(inp)==source['input_sha256'] and digest(original)==source['output_sha256']
            raw=inp.read_bytes();expected=original.read_bytes();outputs={};executions={}
            target=out/f'{kind}-{i:04}-native.bin';e=run(native,inp,target)
            assert e['exit']==0 and target.read_bytes()==expected,(kind,i,e)
            outputs['native']=digest(target);executions['native']=e
            if kind!='packed':
                expected=old.projection(kind,expected,raw)
                for profile in ['debug','release']:
                    target=out/f'{kind}-{i:04}-{profile}.bin';exe=ROOT/f'target/{profile}/grid-convex-distance.exe'
                    e=outer.run(exe,inp,target,False) if kind.startswith('outer') else run(exe,inp,target,False)
                    assert e['exit']==0 and target.read_bytes()==expected,(kind,i,profile,e)
                    outputs[profile]=digest(target);executions[profile]=e
            rows.append(dict(kind=kind,index=i,input_sha256=digest(inp),reference_sha256=digest(original),outputs=outputs,executions=executions))
            if len(rows)%100==0:print(f'cases={len(rows)}',flush=True)
    result=dict(passed=True,specs=specs,results=rows,image_sha256=digest(old.manifold.IMAGE),script_sha256=digest(Path(__file__)),
                debug_sha256=digest(ROOT/'target/debug/grid-convex-distance.exe'),release_sha256=digest(ROOT/'target/release/grid-convex-distance.exe'))
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(f'passed native={len(rows)} Rust-per-profile={sum(r["kind"]!="packed" for r in rows)}',flush=True)


if __name__=='__main__':main()
