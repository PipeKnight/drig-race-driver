"""Rebuilt native and current Rust preserve all preceding collision boundaries."""
import argparse
import json
from pathlib import Path

import validate_triangle_manifold as manifold
import validate_triangle_mesh_outer as outer
from validate_triangle_manifold import ROOT, digest

SPECS = [
    ('manifold', ROOT/'local/behavior/triangle-manifold-debug-20261012-final-01/validation.json', 'triangle-manifold-source-regression.exe'),
    ('distance', outer.COLD, 'triangle-manifold-distance-regression.exe'),
    ('distance-sequence', outer.RETAINED, 'triangle-manifold-distance-sequence-regression.exe'),
    ('outer', ROOT/'local/behavior/triangle-mesh-outer-debug-20261012-final-01/validation.json', 'triangle-manifold-outer-regression.exe'),
    ('outer-sequence', ROOT/'local/behavior/triangle-mesh-outer-sequence-debug-20261012-final-01/validation.json', 'triangle-manifold-outer-sequence-regression.exe'),
    ('packed', outer.distance.UPSTREAM, 'triangle-manifold-packed-regression.exe'),
]


def projection(kind, answer, raw):
    if kind=='manifold': return manifold.projection(answer)
    if kind.startswith('outer'): return outer.projection(answer, raw)
    if kind.endswith('sequence'): return outer.sequence.projection(answer, raw)
    return outer.distance.projection(answer)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows=[];guards=[];specs=[]
    for kind,reference,native_name in SPECS:
        old=json.loads(reference.read_text());assert old['passed']
        native=ROOT/'local/behavior'/native_name
        specs.append(dict(kind=kind,reference=str(reference.relative_to(ROOT)),reference_sha256=digest(reference),native_sha256=digest(native),cases=len(old['results'])))
        for source in old['results']:
            i=source['index'];inp=reference.parent/f'{i:04}-input.bin';assert digest(inp)==source['input_sha256']
            original=reference.parent/f'{i:04}-original.bin';repeat=reference.parent/f'{i:04}-repeat.bin'
            assert digest(original)==digest(repeat)==source['output_sha256']
            raw=inp.read_bytes();expected=original.read_bytes();outputs={};executions={}
            target=out/f'{kind}-{i:04}-native.bin';e=manifold.run(native,inp,target)
            assert e['exit']==0 and target.read_bytes()==expected,(kind,i,e)
            outputs['native']=digest(target);executions['native']=e
            if kind not in ['packed','manifold']:
                expected=projection(kind,expected,raw)
                for profile in ['debug','release']:
                    target=out/f'{kind}-{i:04}-{profile}.bin';exe=ROOT/f'target/{profile}/grid-convex-distance.exe'
                    e=outer.run(exe,inp,target,False) if kind.startswith('outer') else manifold.run(exe,inp,target,False)
                    assert e['exit']==0 and target.read_bytes()==expected,(kind,i,profile,e)
                    outputs[profile]=digest(target);executions[profile]=e
            rows.append(dict(kind=kind,index=i,input_sha256=digest(inp),reference_sha256=digest(original),outputs=outputs,executions=executions))
            if (i+1)%200==0:print(f'{kind} cases={i+1}',flush=True)
        for source in old.get('guards',[]):
            label=source['label']
            if label.startswith('existing-'):continue
            executions={}
            if label.startswith('table-'):
                sampling=reference.parent/f'guard-{label}.bin';assert digest(sampling)==source['sampling_sha256']
                inp=reference.parent/'0000-input.bin'
                targets=[(profile,ROOT/f'target/{profile}/grid-convex-distance.exe',False) for profile in ['debug','release']]
            else:
                sampling=None;inp=reference.parent/f'guard-{label}-input.bin'
                targets=[('native',native,True)]
                if kind!='packed':targets.extend((profile,ROOT/f'target/{profile}/grid-convex-distance.exe',False) for profile in ['debug','release'])
            for name,exe,is_native in targets:
                target=out/f'guard-{kind}-{label}-{name}.bin'
                e=outer.run(exe,inp,target,is_native,sampling) if kind.startswith('outer') else manifold.run(exe,inp,target,is_native)
                assert 0<e['exit']<256 and not target.exists(),(kind,label,name,e);executions[name]=e
            guards.append(dict(kind=kind,label=label,input_sha256=digest(inp),sampling_sha256=digest(sampling) if sampling else None,executions=executions,no_output=True))
        targets=[('native',native,True)]
        if kind!='packed':targets.extend((profile,ROOT/f'target/{profile}/grid-convex-distance.exe',False) for profile in ['debug','release'])
        for name,exe,is_native in targets:
            target=out/f'guard-{kind}-existing-{name}.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
            e=outer.run(exe,reference.parent/'0000-input.bin',target,is_native) if kind.startswith('outer') else manifold.run(exe,reference.parent/'0000-input.bin',target,is_native)
            assert e['exit'] and digest(target)==before
            guards.append(dict(kind=kind,label=f'existing-{name}',executions={name:e},unchanged_sha256=before))
    report=dict(passed=True,specs=specs,results=rows,guards=guards,guard_count=len(guards),
                debug_sha256=digest(ROOT/'target/debug/grid-convex-distance.exe'),release_sha256=digest(ROOT/'target/release/grid-convex-distance.exe'),
                image_sha256=digest(manifold.IMAGE),script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(f'passed {len(rows)} native cases plus current Rust in both profiles; guards={len(guards)}')


if __name__=='__main__':main()
