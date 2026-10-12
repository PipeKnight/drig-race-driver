"""Replay cold native evidence and measure retained-state effects/boundaries."""
import argparse
import json
import struct
from pathlib import Path

import validate_triangle_mesh_sequence as sequence
from validate_triangle_mesh_sequence import ROOT, digest, distance


def main():
    p = argparse.ArgumentParser()
    p.add_argument('sequence_reference', type=Path)
    p.add_argument('cold_reference', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    seq_path, cold_path = a.sequence_reference.resolve(), a.cold_reference.resolve()
    seq, cold = [json.loads(v.read_text()) for v in [seq_path, cold_path]]
    assert seq['passed'] and cold['passed'] and cold['cases'] == 1801
    assert digest(sequence.NATIVE) == seq['native_sha256'] and digest(distance.NATIVE) == cold['native_sha256']
    assert digest(distance.IMAGE) == seq['image_sha256'] == cold['image_sha256']
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    rows = []
    for row in cold['results']:
        i = row['index']; inp = cold_path.parent/f'{i:04}-input.bin'; assert digest(inp) == row['input_sha256']
        target = out/f'{i:04}-cold.bin'; e = distance.run(sequence.COLD, inp, target)
        assert e['exit'] == 0 and digest(target) == row['output_sha256'], (i, e)
        rows.append(dict(index=i, input_sha256=digest(inp), output_sha256=digest(target), bytes=target.stat().st_size, execution=e))
    effects = []
    rust = ROOT/'target/debug/grid-convex-distance.exe'
    for source_row in seq['results'][::max(1, seq['cases']//24)][:24]:
        i = source_row['index']; inp = seq_path.parent/f'{i:04}-input.bin'; assert digest(inp) == source_row['input_sha256']
        raw = inp.read_bytes(); count = struct.unpack_from('<I', raw, 40)[0]
        target = seq_path.parent/f'{i:04}-original.bin'; assert digest(target) == source_row['output_sha256']
        warm = sequence.projection(target.read_bytes(), raw)[4:]
        start = 44+count*24
        for step in [1, count-1]:
            r = warm[step*448:(step+1)*448]
            # Use exactly the carried input direction that preceded this call.
            control = raw[44+step*24:44+(step+1)*24]
            direction = control[8:] if struct.unpack_from('<I', control, 4)[0] else warm[(step-1)*448+0x130:(step-1)*448+0x140]
            cold_input = b'PC04'+raw[4:40]+control[:4]+direction+raw[start:]
            cold_in = out/f'effect-{i:04}-{step:02}-input.bin'; cold_in.write_bytes(cold_input)
            answers = []
            for name, exe, native in [('native', sequence.COLD, True), ('rust', rust, False)]:
                answer = out/f'effect-{i:04}-{step:02}-{name}.bin'; e = distance.run(exe, cold_in, answer, native)
                assert e['exit'] == 0, (i, step, name, e)
                answers.append(answer.read_bytes())
            assert distance.projection(answers[0]) == answers[1]
            distance.check(answers[0], cold_input)
            c = answers[1]
            effects.append(dict(sequence=i, step=step, input_sha256=digest(cold_in),
                                native_sha256=digest(out/f'effect-{i:04}-{step:02}-native.bin'),
                                rust_sha256=digest(out/f'effect-{i:04}-{step:02}-rust.bin'),
                                scratch_differs=c[:0x120] != r[:0x120], cache_differs=c[0x164:0x180] != r[0x164:0x180],
                                result_differs=c[0x128:0x160] != r[0x128:0x160]))
    # Explicit lower/upper sequence bounds, including an explicit zero direction
    # and repeated carry after an intersecting source fixture where available.
    positive = next((r for r in seq['results'] if r['witness_queries']), seq['results'][0])
    base = (seq_path.parent/f"{positive['index']:04}-input.bin").read_bytes()
    start = 44+struct.unpack_from('<I', base, 40)[0]*24
    source = b'PC01'+base[4:40]+base[start:]
    ti = struct.unpack_from('<I', base, 44)[0]
    bounds = []
    for n in [1, 64]:
        calls = [(ti, 1, (1., 0., 0., 0.))]+[(ti, int(i%4 == 0), distance.DIRECTIONS[i%8]) for i in range(1, n)]
        raw = sequence.request(source, calls, .5)
        inp = out/f'bound-{n:02}-input.bin'; inp.write_bytes(raw); answers = []
        executions = []
        for name, exe, native in [('original', sequence.NATIVE, True), ('repeat', sequence.NATIVE, True), ('debug', rust, False),
                                  ('release', ROOT/'target/release/grid-convex-distance.exe', False)]:
            target = out/f'bound-{n:02}-{name}.bin'; e = distance.run(exe, inp, target, native)
            assert e['exit'] == 0, (n, name, e); answers.append(target.read_bytes()); executions.append(e)
        assert answers[0] == answers[1] and sequence.projection(answers[0], raw) == answers[2] == answers[3]
        bounds.append(dict(count=n, input_sha256=digest(inp), output_sha256=digest(out/f'bound-{n:02}-original.bin'),
                           rust_sha256=digest(out/f'bound-{n:02}-debug.bin'), executions=executions, **sequence.check(answers[0], raw)))
    report = dict(passed=True, cold_cases=len(rows), cold_results=rows, effects=effects, bounds=bounds,
                  scratch_effects=sum(r['scratch_differs'] for r in effects), cache_effects=sum(r['cache_differs'] for r in effects),
                  result_effects=sum(r['result_differs'] for r in effects), observable_bytes=sum(r['bytes'] for r in rows),
                  sequence_reference=str(seq_path.relative_to(ROOT)), sequence_reference_sha256=digest(seq_path),
                  cold_reference=str(cold_path.relative_to(ROOT)), cold_reference_sha256=digest(cold_path),
                  native_sha256=digest(sequence.NATIVE), cold_native_sha256=digest(sequence.COLD),
                  debug_sha256=digest(rust), release_sha256=digest(ROOT/'target/release/grid-convex-distance.exe'),
                  script_sha256=digest(Path(__file__)))
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(f'passed cold_cases={len(rows)} retained_scratch_effects={report["scratch_effects"]}/{len(effects)} '
          f'cache_effects={report["cache_effects"]} result_effects={report["result_effects"]} bounds=1/64')


if __name__ == '__main__': main()
