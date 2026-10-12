"""Rebuilt shared native variants/current Rust preserve earlier physical wires."""
import argparse
import json
from pathlib import Path

from validate_carried_geometry import ROOT, IMAGE, digest, run

SPECS = [
    ('physical-tick', 'physical-tick-20261010-debug-final-02', 'input.bin', 'original.bin'),
    ('world-physical-tick', 'world-physical-tick-debug-20261011-final-02', 'input.bin', 'original.bin'),
    ('grouped-physical-frame', 'grouped-physical-frame-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('grouped-physical-tick', 'grouped-physical-tick-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('shaped-physical-frame', 'shaped-physical-frame-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('shaped-physical-tick', 'shaped-physical-tick-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('whole-physical-world', 'whole-physical-world-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('carried-physical-world', 'carried-physical-world-debug-20261011-final-01', 'input.bin', 'original.bin'),
    ('reproduced-physical-world', 'reproduced-physical-world-debug-20261011-final-01', 'original-input.bin', 'original-output.bin'),
    ('geometric-physical-tick', 'packed-complete-handoff-debug-20261012-final-01-a', 'input.bin', 'original.bin'),
    ('geometric-physical-tick', 'packed-complete-handoff-debug-20261012-final-01-b', 'input.bin', 'original.bin'),
]


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); a = p.parse_args()
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    rows = []
    for i, (kind, directory, input_name, original_name) in enumerate(SPECS):
        reference = ROOT/'local/behavior'/directory
        report = reference/'validation.json'; old = json.loads(report.read_text()); assert old['passed']
        inp = reference/input_name; previous = reference/original_name
        expected = previous.read_bytes()
        import struct
        cases = struct.unpack_from('<I', inp.read_bytes(), 4)[0]
        executions = []
        native = ROOT/f'local/behavior/carried-geometry-{kind}-regression.exe'
        for mode, exe in [('native', native), ('debug', ROOT/'target/debug/grid-physical-tick.exe'), ('release', ROOT/'target/release/grid-physical-tick.exe')]:
            target = out/f'{i:02}-{mode}.bin'; e = run(exe, inp, target, mode == 'native')
            assert e['exit'] == 0 and not e['stderr'] and target.read_bytes() == expected, (kind, mode, e)
            executions.append(dict(kind=mode, sha256=digest(exe), execution=e, output_sha256=digest(target)))
        rows.append(dict(index=i, kind=kind, reference=str(report.relative_to(ROOT)), reference_sha256=digest(report),
                         input_name=input_name, original_name=original_name, input_sha256=digest(inp),
                         output_sha256=digest(previous), cases=cases, bytes=len(expected)-8, executions=executions))
        print(f'{kind} cases={cases} native/debug/release exact', flush=True)
    result = dict(passed=True, results=rows, cases_per_run=sum(r['cases'] for r in rows),
                  bytes_per_run=sum(r['bytes'] for r in rows), image_sha256=digest(IMAGE),
                  script_sha256=digest(Path(__file__)), shared_source_sha256=digest(ROOT/'tools/probe_physical_tick.c'))
    (out/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ['passed', 'cases_per_run', 'bytes_per_run']}))


if __name__ == '__main__': main()
