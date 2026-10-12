"""Replay current Rust against complete, hash-checked original distance evidence."""
import argparse
import json
from pathlib import Path

import validate_triangle_mesh_distance as distance

ROOT,digest=distance.ROOT,distance.digest


def main():
    p=argparse.ArgumentParser()
    p.add_argument('reference',type=Path)
    p.add_argument('output',type=Path)
    p.add_argument('--profile',choices=['debug','release'],default='debug')
    a=p.parse_args()
    reference=a.reference.resolve();r=json.loads(reference.read_text())
    assert r['passed'] and not r['regression_only']
    assert digest(distance.NATIVE)==r['native_sha256'] and digest(distance.IMAGE)==r['image_sha256']
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rust=ROOT/f'target/{a.profile}/grid-convex-distance.exe'
    rows=[]
    for row in r['results']:
        i=row['index'];inp=reference.parent/f'{i:04}-input.bin';assert digest(inp)==row['input_sha256']
        native=reference.parent/f'{i:04}-original.bin';repeat=reference.parent/f'{i:04}-repeat.bin'
        assert digest(native)==digest(repeat)==row['output_sha256']
        distance.check(native.read_bytes(),inp.read_bytes())
        target=out/f'{i:04}-rust.bin';e=distance.run(rust,inp,target,False)
        assert e['exit']==0 and target.read_bytes()==distance.projection(native.read_bytes()),(i,e)
        rows.append(dict(index=i,input_sha256=digest(inp),output_sha256=digest(target),bytes=target.stat().st_size,execution=e))
    guards=[]
    for g in r['guards']:
        if g['label'].startswith('existing-'):continue
        inp=reference.parent/f"guard-{g['label']}-input.bin";target=out/f"guard-{g['label']}-rust.bin"
        e=distance.run(rust,inp,target,False);assert 0<e['exit']<256 and not target.exists(),(g['label'],e)
        guards.append(dict(label=g['label'],input_sha256=digest(inp),execution=e,no_output=True))
    target=out/'guard-existing-rust.bin';target.write_bytes(b'preserve existing evidence');before=digest(target)
    e=distance.run(rust,reference.parent/'0000-input.bin',target,False);assert e['exit'] and digest(target)==before
    guards.append(dict(label='existing-rust',execution=e,unchanged_sha256=before))
    report=dict(passed=True,profile=a.profile,cases=len(rows),results=rows,guards=guards,guard_count=len(guards),
                rust_sha256=digest(rust),native_sha256=digest(distance.NATIVE),image_sha256=digest(distance.IMAGE),
                reference=str(reference.relative_to(ROOT)),reference_sha256=digest(reference),script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(f'passed {a.profile} cases={len(rows)} guards={len(guards)} complete_bytes={len(rows)*448}')


if __name__=='__main__':main()
