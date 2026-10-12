"""Advanced-pose provider versus complete original retained geometry stages.

Provider returns supplied world templates; this timing proof is not detection.
"""
import argparse
import json
import subprocess
from pathlib import Path

import validate_carried_geometry as source

ROOT, digest = source.ROOT, source.digest
REFERENCE = ROOT/'local/behavior/carried-geometry-debug-20261012-final-01/validation.json'


def run(exe, inp, target, provider):
    p = subprocess.run([str(exe), str(inp), str(target)]+(['--geometry-provider'] if provider else []),
                       cwd=ROOT, capture_output=True, text=True, timeout=60)
    return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr)


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); a = p.parse_args()
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    old = json.loads(REFERENCE.read_text()); assert old['passed'] and old['cases'] == 3648
    assert digest(source.NATIVE) == old['native_sha256'] and digest(source.IMAGE) == old['image_sha256']
    cases = source.cases(); assert len(cases) == 3648
    source.shaped.MAGIC = source.shaped.g.MAGIC = source.MAGIC
    rows = []; decoded = {p: [] for p in ['debug','release']}
    for previous in old['batches']:
        start, n = previous['start'], previous['cases']; batch = cases[start:start+n]
        inp = REFERENCE.parent/f'{start:04}-input.bin'
        assert digest(inp) == previous['input_sha256'] and inp.read_bytes() == source.pack(batch)
        original = REFERENCE.parent/f'{start:04}-original.bin'; repeat = REFERENCE.parent/f'{start:04}-repeat.bin'
        assert digest(original) == digest(repeat) == previous['output_sha256']
        expected = original.read_bytes(); assert expected == repeat.read_bytes()
        runs = {}
        target = out/f'{start:04}-native.bin'; e = source.run(source.NATIVE, inp, target, True)
        assert e['exit'] == 0 and not e['stderr'] and 'code_unchanged=1 no_imports=1' in e['stdout'] and target.read_bytes() == expected
        runs['native'] = dict(execution=e, sha256=digest(target))
        for profile in ['debug','release']:
            exe = ROOT/f'target/{profile}/grid-physical-tick.exe'
            for mode in ['legacy','provider']:
                target = out/f'{start:04}-{profile}-{mode}.bin'; e = run(exe, inp, target, mode=='provider')
                assert e['exit'] == 0 and not e['stderr'] and target.read_bytes() == expected, (start,profile,mode,e)
                runs[f'{profile}-{mode}'] = dict(execution=e, sha256=digest(target))
                if mode=='provider': decoded[profile].extend(source.shaped.decode(target.read_bytes(), batch))
        rows.append(dict(start=start, cases=n, input_sha256=digest(inp), output_sha256=digest(original), bytes=len(expected), runs=runs))
        print(f'cases={start+n}', flush=True)
    activity = {p: source.semantics(cases, outputs) for p, outputs in decoded.items()}
    assert activity['debug'] == activity['release'] == old['activity']
    guards = []
    for g in old['guards']:
        if g['kind'] != 'rust': continue
        label = g['label']; inp = REFERENCE.parent/f'guard-{label}-input.bin'
        for profile in ['debug','release']:
            exe = ROOT/f'target/{profile}/grid-physical-tick.exe'
            for provider in [False,True]:
                mode = 'provider' if provider else 'legacy'; target = out/f'guard-{label}-{profile}-{mode}.bin'
                if label=='existing': target.write_bytes(b'preserve existing evidence'); before=digest(target)
                else: assert digest(inp) == g['input_sha256']
                e = run(exe,inp,target,provider)
                assert 0<e['exit']<256
                if label=='existing': assert digest(target)==before
                else: assert not target.exists()
                guards.append(dict(label=label, profile=profile, mode=mode, execution=e,
                                   no_output=label!='existing', unchanged_sha256=before if label=='existing' else None))
    report = dict(passed=True, cases=len(cases), intervals=sum(c['steps'] for c in cases),
                  provider_calls_per_profile=sum(c['steps']-1 for c in cases),
                  observable_bytes_per_run=sum(row['bytes']-8 for row in rows), activity=activity, results=rows, guards=guards,
                  reference_sha256=digest(REFERENCE), source_script_sha256=digest(ROOT/'tools/validate_carried_geometry.py'),
                  native_sha256=digest(source.NATIVE), image_sha256=digest(source.IMAGE),
                  rust_hashes={p:digest(ROOT/f'target/{p}/grid-physical-tick.exe') for p in ['debug','release']},
                  script_sha256=digest(Path(__file__)), fresh_detection=False, runtime_barriers=False)
    (out/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report[k] for k in ['passed','cases','intervals','provider_calls_per_profile','observable_bytes_per_run']}))


if __name__=='__main__': main()
