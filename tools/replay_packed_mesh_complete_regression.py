"""Preserve legacy fixture behavior and sample each shared direct boundary."""
import argparse
import json
import subprocess
from pathlib import Path

from replay_triangle_manifold_regression import SPECS
from validate_packed_mesh_complete import ROOT, IMAGE, digest


def run(exe, inp, out):
    r = subprocess.run([str(exe), str(IMAGE), str(inp), str(out)], cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=r.returncode, stdout=r.stdout, stderr=r.stderr)


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); a = p.parse_args()
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    specs, rows, guards = [], [], []
    for kind, reference, _ in SPECS:
        old = json.loads(reference.read_text()); assert old['passed']
        exe = ROOT/f'local/behavior/packed-complete-{kind}-regression.exe'
        count = len(old['results'])
        indices = range(count) if kind == 'packed' else sorted({i*(count-1)//9 for i in range(10)})
        specs.append(dict(kind=kind, reference=str(reference.relative_to(ROOT)), reference_sha256=digest(reference),
                          native_sha256=digest(exe), replayed_cases=len(indices), historical_cases=count))
        for i in indices:
            source = old['results'][i]; assert source['index'] == i
            inp = reference.parent/f'{i:04}-input.bin'; assert digest(inp) == source['input_sha256']
            previous = reference.parent/f'{i:04}-original.bin'; assert digest(previous) == source['output_sha256']
            target = out/f'{kind}-{i:04}.bin'; e = run(exe, inp, target)
            assert e['exit'] == 0 and not e['stderr'] and target.read_bytes() == previous.read_bytes(), (kind, i, e)
            rows.append(dict(kind=kind, index=i, input_sha256=digest(inp), output_sha256=digest(target), bytes=target.stat().st_size, execution=e))
        for g in old['guards']:
            label = g['label']
            if label.startswith('existing') or label.startswith('table-'): continue
            inp = reference.parent/f'guard-{label}-input.bin'
            target = out/f'guard-{kind}-{label}.bin'; e = run(exe, inp, target)
            assert 0 < e['exit'] < 256 and not target.exists() and not e['stderr'], (kind, label, e)
            guards.append(dict(kind=kind, label=label, input_sha256=digest(inp), no_output=True, execution=e))
        target = out/f'guard-{kind}-existing.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
        e = run(exe, reference.parent/'0000-input.bin', target)
        assert 0 < e['exit'] < 256 and digest(target) == before
        guards.append(dict(kind=kind, label='existing', unchanged_sha256=before, execution=e))
        print(f'{kind} complete_outputs={len(indices)}', flush=True)
    report = dict(passed=True, specs=specs, results=rows, guards=guards,
                  complete_outputs=len(rows), guard_count=len(guards), image_sha256=digest(IMAGE),
                  observable_bytes=sum(r['bytes'] for r in rows), script_sha256=digest(Path(__file__)),
                  shared_source_sha256=digest(ROOT/'tools/probe_packed_mesh_contact.c'),
                  header_sha256=digest(ROOT/'tools/owned_packed_shape_probe.h'),
                  scope='All 1417 historical packed cases; ten spaced cases per other shared boundary; native regressions only.')
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'complete_outputs', 'guard_count', 'observable_bytes']}))


if __name__ == '__main__': main()
