"""Spaced current-source regressions and complete retained guard replay."""
import argparse
import json
from pathlib import Path

import replay_triangle_manifold_regression as cold
import validate_manifold_sequence as retained
from validate_manifold_sequence import ROOT, digest, run

RETAINED=ROOT/'local/behavior/manifold-sequence-debug-20261012-final-02/validation.json'
TURNS=ROOT/'local/behavior/manifold-turns-20261012-final-01/validation.json'
PACKED=ROOT/'local/behavior/packed-complete-20261012-final-01/validation.json'


def specs():
    names={'manifold':'triangle_manifold','distance':'triangle_mesh_distance','distance-sequence':'triangle_mesh_sequence',
           'outer':'triangle_mesh_query','outer-sequence':'triangle_mesh_outer_sequence','packed':'packed_mesh_contact',
           'retained':'manifold_sequence','turns':'manifold_sequence','packed-complete':'packed_mesh_complete'}
    for kind,path in [(k,p) for k,p,_ in cold.SPECS]+[('retained',RETAINED),('turns',TURNS),('packed-complete',PACKED)]:
        report=json.loads(path.read_text());assert report['passed'];n=40 if kind in ['manifold','retained'] else 20
        selected=[report['results'][i*len(report['results'])//n] for i in range(n)]
        yield kind,path,ROOT/f'local/behavior/oriented-regression-{names[kind]}.exe',report,selected


def projection(kind,answer,raw):
    return retained.projection(answer,raw) if kind in ['retained','turns'] else cold.projection(kind,answer,raw)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows=[];descriptors=[];rejections=[]
    for kind,path,native,report,selected in specs():
        descriptors.append(dict(kind=kind,reference=str(path.relative_to(ROOT)),reference_sha256=digest(path),native_sha256=digest(native),selected=[r['index'] for r in selected]))
        for source in selected:
            i=source['index'];inp=path.parent/f'{i:04}-input.bin';original=path.parent/f'{i:04}-original.bin'
            assert digest(inp)==source['input_sha256'] and digest(original)==source['output_sha256']
            expected=original.read_bytes();raw=inp.read_bytes();outputs={};executions={}
            for name,exe,is_native in [('native',native,True)]+([] if kind.startswith('packed') else [(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in ['debug','release']]):
                target=out/f'{kind}-{i:04}-{name}.bin'
                e=cold.outer.run(exe,inp,target,is_native) if kind.startswith('outer') else run(exe,inp,target,is_native)
                assert e['exit']==0 and target.read_bytes()==(expected if is_native else projection(kind,expected,raw)),(kind,i,name,e)
                outputs[name]=digest(target);executions[name]=e
            rows.append(dict(kind=kind,index=i,input_sha256=digest(inp),reference_sha256=digest(original),outputs=outputs,executions=executions))
        if kind=='retained':
            for source in report['guards']:
                label=source['label']
                if label.startswith('existing-'):continue
                inp=path.parent/f'guard-{label}-input.bin';assert digest(inp)==source['input_sha256'];executions={}
                for name,exe,is_native in [('native',native,True)]+[(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in ['debug','release']]:
                    target=out/f'guard-{label}-{name}.bin';e=run(exe,inp,target,is_native)
                    assert 0<e['exit']<256 and not target.exists(),(label,name,e);executions[name]=e
                rejections.append(dict(label=label,input_sha256=digest(inp),executions=executions,no_output=True))
            for name,exe,is_native in [('native',native,True)]+[(p,ROOT/f'target/{p}/grid-convex-distance.exe',False) for p in ['debug','release']]:
                target=out/f'guard-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
                e=run(exe,path.parent/'0000-input.bin',target,is_native);assert e['exit'] and digest(target)==before
                rejections.append(dict(label=f'existing-{name}',executions={name:e},unchanged_sha256=before))
    result=dict(passed=True,specs=descriptors,results=rows,guards=rejections,guard_count=len(rejections),image_sha256=digest(retained.IMAGE),
                rust_hashes={p:digest(ROOT/f'target/{p}/grid-convex-distance.exe') for p in ['debug','release']},script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(f'passed native={len(rows)} complete={sum(len(r["outputs"]) for r in rows)} guards={len(rejections)}',flush=True)


if __name__=='__main__':main()
