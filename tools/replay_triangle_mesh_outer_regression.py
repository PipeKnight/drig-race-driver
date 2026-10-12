"""Preserve earlier cold/retained distance proofs after outer-query integration."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_mesh_outer as outer
from validate_triangle_mesh_outer import ROOT, digest, distance, sequence

NATIVE = ROOT/'local/behavior/triangle-mesh-outer-distance-regression.exe'


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); a = p.parse_args()
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    rows, guards = [], []
    for kind, reference in [('cold', outer.COLD), ('retained', outer.RETAINED)]:
        previous = json.loads(reference.read_text()); assert previous['passed']
        original_exe = distance.NATIVE if kind == 'cold' else sequence.NATIVE
        assert digest(original_exe) == previous['native_sha256']
        for row in previous['results']:
            i = row['index']; inp = reference.parent/f'{i:04}-input.bin'; assert digest(inp) == row['input_sha256']
            original = reference.parent/f'{i:04}-original.bin'; repeat = reference.parent/f'{i:04}-repeat.bin'
            assert digest(original) == digest(repeat) == row['output_sha256']
            raw = inp.read_bytes(); expected = distance.projection(original.read_bytes()) if kind == 'cold' else sequence.projection(original.read_bytes(), raw)
            record = dict(kind=kind, index=i, input_sha256=digest(inp), reference_sha256=digest(original), outputs={})
            executions = {}
            if kind == 'retained':
                target = out/f'{kind}-{i:04}-native.bin'; e = distance.run(NATIVE, inp, target)
                assert e['exit'] == 0 and digest(target) == row['output_sha256'], (kind, i, e)
                record['outputs']['native'] = digest(target); executions['native'] = e
            for profile in ['debug', 'release']:
                exe = ROOT/f'target/{profile}/grid-convex-distance.exe'; target = out/f'{kind}-{i:04}-{profile}.bin'
                e = distance.run(exe, inp, target, False); assert e['exit'] == 0 and target.read_bytes() == expected, (kind, i, profile, e)
                record['outputs'][profile] = digest(target); executions[profile] = e
            record['executions'] = executions; rows.append(record)
            if (i+1)%200 == 0: print(f'{kind} cases={i+1}', flush=True)
        for g in previous['guards']:
            label = g['label']
            if label.startswith('existing-'): continue
            inp = reference.parent/f'guard-{label}-input.bin'; executions = {}
            for profile in ['debug', 'release']:
                target = out/f'guard-{kind}-{label}-{profile}.bin'
                e = distance.run(ROOT/f'target/{profile}/grid-convex-distance.exe', inp, target, False)
                assert 0 < e['exit'] < 256 and not target.exists(), (kind, label, profile, e); executions[profile] = e
            guards.append(dict(kind=kind, label=label, input_sha256=digest(inp), executions=executions, no_output=True))
        for profile in ['debug', 'release']:
            target = out/f'guard-{kind}-existing-{profile}.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
            e = distance.run(ROOT/f'target/{profile}/grid-convex-distance.exe', reference.parent/'0000-input.bin', target, False)
            assert e['exit'] and digest(target) == before
            guards.append(dict(kind=kind, label=f'existing-{profile}', executions={profile:e}, unchanged_sha256=before))
    # Reuse the prior source-only sequence boundary inputs, changing only mode.
    boundaries = ROOT/'local/behavior/triangle-mesh-sequence-regression-20261012-final-01/validation.json'
    old_bounds = json.loads(boundaries.read_text()); assert old_bounds['passed']
    bounds = []
    table = (outer.SAMPLING.parent/'directions.bin').read_bytes()
    for old in old_bounds['bounds']:
        n = old['count']; source = boundaries.parent/f'bound-{n:02}-input.bin'; assert digest(source) == old['input_sha256']
        raw = b'PC06'+source.read_bytes()[4:]; inp = out/f'bound-{n:02}-input.bin'; inp.write_bytes(raw)
        answers, executions = [], []
        for name, exe, native in [('original', outer.NATIVE_SEQUENCE, True), ('repeat', outer.NATIVE_SEQUENCE, True),
                                  ('debug', ROOT/'target/debug/grid-convex-distance.exe', False), ('release', ROOT/'target/release/grid-convex-distance.exe', False)]:
            target = out/f'bound-{n:02}-{name}.bin'; e = outer.run(exe, inp, target, native)
            assert e['exit'] == 0, (n, name, e); answers.append(target.read_bytes()); executions.append(e)
        assert answers[0] == answers[1] and outer.projection(answers[0], raw) == answers[2] == answers[3]
        bounds.append(dict(count=n, input_sha256=digest(inp), output_sha256=digest(out/f'bound-{n:02}-original.bin'),
                           rust_sha256=digest(out/f'bound-{n:02}-debug.bin'), executions=executions, **outer.check(answers[0], raw, table)))
    report = dict(passed=True, cold_cases=1801, retained_cases=1273, results=rows, guards=guards, guard_count=len(guards), bounds=bounds,
                  native_sha256=digest(NATIVE), outer_native_sha256=digest(outer.NATIVE_SEQUENCE),
                  debug_sha256=digest(ROOT/'target/debug/grid-convex-distance.exe'), release_sha256=digest(ROOT/'target/release/grid-convex-distance.exe'),
                  cold_reference_sha256=digest(outer.COLD), retained_reference_sha256=digest(outer.RETAINED),
                  boundary_reference_sha256=digest(boundaries), script_sha256=digest(Path(__file__)))
    assert len(rows) == 1801+1273
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(f'passed cold=1801 retained=1273 profiles=debug/release guards={len(guards)} bounds=1/64')


if __name__ == '__main__': main()
