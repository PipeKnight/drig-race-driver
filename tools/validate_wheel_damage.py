"""Whole original visual-deformation update versus Rust; private bounded fixtures."""
import argparse
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '6087bb502314bb9a20a4b866f776c621c1c918a935358969db1f33b47bb33aac'


def rows(path):
    result = {}
    for line in path.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == 6 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--snapshot', type=Path, action='append', default=[])
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and len(args.snapshot) <= 8
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    probe = ROOT / 'local/behavior/wheel-damage-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-wheel-damage.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    out.mkdir(exist_ok=False)
    cases, delta_pairs = [], []
    base = [.1, .25, .5, .75, 0., .1, .02, .03, .04, .05,
            .01, -.02, .03, -.04, .7, 0., 27., .45, .016]
    # Skip, all four lanes/ordinals, clamp/prior boundaries, correction and ignored delta.
    for wheel in range(4):
        for prior in [-1., -0., 0., .5, 1., 2.]:
            for scale in [-0., 0., 1., 27., 90.]:
                for corrected in [0., 1.]:
                    row = base.copy()
                    row[wheel], row[4], row[15], row[16] = prior, float(wheel), corrected, scale
                    cases.append((f'boundary_{len(cases)}', row))
    for lane in range(4):
        for amount in [-2., -.45, -0., 0., .1, .45, 2.]:
            row = base.copy()
            row[:4], row[5], row[6:10] = [0.] * 4, 0., [0.] * 4
            row[6 + lane] = amount
            cases.append((f'lane_{len(cases)}', row))
    for angle in [-2., -.4712388813495636, -0., 0., .1, .4712388813495636, 2.]:
        row = base.copy()
        row[:4], row[5], row[6:10] = [0.] * 4, angle, [0.] * 4
        cases.append((f'angle_{len(cases)}', row))
    for delta in [-1000., -0., 0., .001, .016, 1., 1000.]:
        row = base.copy()
        row[18] = delta
        name = f'delta_{len(delta_pairs)}'
        cases.append((name, row))
        delta_pairs.append(name)
    rng = random.Random(131)
    for i in range(2048):
        row = [rng.uniform(-.1, 1.1) for _ in range(4)]
        row += [float(i % 4), rng.uniform(-2., 2.)]
        row += [rng.uniform(-.5, .5) for _ in range(8)]
        row += [rng.uniform(-2., 2.), float(i % 2), rng.uniform(.01, 100.),
                rng.uniform(.01, 2.), rng.uniform(-1., 1.)]
        cases.append((f'authored_{i}', row))
    provenance, live_expected, stability, previous = [], {}, [], None
    for ordinal, path in enumerate(args.snapshot):
        path = path.resolve()
        assert path.is_relative_to((ROOT / 'local').resolve()) and path.stat().st_size <= 8 * 1024 * 1024
        capture = json.loads(path.read_text(encoding='utf-8-sig'))
        assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
        assert capture['live_count_code_exact'] and len(capture['managers']) == 1
        assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
        blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
        assert len(blocks['steering_code_9f2f80']) == 32
        selected = {name: data for name, data in blocks.items() if name.endswith((
            '-mutable_steering', '-parameter_neighborhood', '-visual_damage_correction',
            '-visual_damage_scale', '-visual_damage_flag'))}
        assert len(selected) == 60
        if previous is not None:
            assert previous.keys() == selected.keys()
            stability.append(dict(before=ordinal - 1, after=ordinal, blocks=60,
                                  exact_blocks=sum(selected[k] == previous[k] for k in selected)))
        previous = selected
        manager = capture['managers'][0]
        assert manager['count'] == len(manager['cars']) == 12
        for car in manager['cars']:
            i = car['index']
            assert car['wheel_count'] == 4
            mutable = blocks[f'car_{i}-mutable_steering']
            body = blocks[f'car_{i}-parameter_neighborhood']
            assert struct.unpack_from('<I', mutable, 0x214)[0] == int(car['vehicle'], 16)
            assert struct.unpack_from('<I', blocks[f'car_{i}-header'], 0x24)[0] == int(car['body'], 16)
            assert len(mutable) == 0x460 and len(body) == 0x350
            def floats(data, offset, count):
                return list(struct.unpack_from('<' + 'f' * count, data, offset))
            levels = floats(mutable, 0x18, 4)
            correction = floats(blocks[f'car_{i}-visual_damage_correction'], 0, 4)
            correction_scale = floats(blocks[f'car_{i}-visual_damage_scale'], 0, 1)
            enabled = blocks[f'car_{i}-visual_damage_flag'][0]
            assert enabled in (0, 1)
            for wheel in range(4):
                row = levels + [float(wheel)] + floats(body, 0x1790 - 0x1640 + wheel * 4, 1)
                row += floats(body, 0x1650 - 0x1640 + wheel * 16, 4)
                row += correction + correction_scale + [float(enabled)] + floats(mutable, 0x280, 2) + [.016]
                name = f'live_{ordinal}_{i}_{wheel}'
                cases.append((name, row))
                live_expected[name] = [f'{word(v):08x}' for v in levels]
        provenance.append(dict(path=str(path), sha256=digest(path), utc=capture['utc'], pid=capture['pid'],
                               created_utc=capture['created_utc'], blocks=len(blocks)))
    assert len(cases) <= 4096
    guards, guard_results = [], []
    for slot, value in [(4, 4.), (15, 2.), (16, -1.), (17, 0.), (6, float('nan'))]:
        row = base.copy()
        row[slot] = value
        guards.append(row)
    for ordinal, row in enumerate(guards):
        target = out / f'guard-{ordinal}-native.tsv'
        request = out / f'guard-{ordinal}.tsv'
        request.write_text('guard\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n')
        a = subprocess.run([str(probe), str(image), str(request), str(out / f'guard-{ordinal}-original.tsv')],
                           cwd=ROOT, capture_output=True, text=True, timeout=30)
        b = subprocess.run([str(native), str(request), str(target)], cwd=ROOT,
                           capture_output=True, text=True, timeout=30)
        guard_results.append(dict(original_exit=a.returncode, native_exit=b.returncode,
                                  native_output_created=target.exists(), original_stdout=a.stdout,
                                  native_stderr=b.stderr))
        assert a.returncode != 0 and b.returncode != 0 and not target.exists()
    request = out / 'input.tsv'
    request.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n' for name, row in cases))
    executions = []
    for kind, exe, argv in [('original', probe, [image, request, out / 'original.tsv']),
                            ('native', native, [request, out / 'native.tsv'])]:
        run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
        (out / 'executions.json').write_text(json.dumps(executions, indent=2))
        if run.returncode:
            raise RuntimeError('Visual wheel damage execution failed')
    original, actual = rows(out / 'original.tsv'), rows(out / 'native.tsv')
    assert original.keys() == actual.keys() == {name for name, _ in cases}
    differences = [dict(case=name, field=i, original=a, native=b) for name in original
                   for i, (a, b) in enumerate(zip(original[name], actual[name])) if a != b]
    live_differences = [dict(case=name, field=i, captured=a, replay=b) for name in live_expected
                        for i, (a, b) in enumerate(zip(live_expected[name], actual[name])) if a != b]
    assert all(original[name] == original[delta_pairs[0]] for name in delta_pairs)
    result = dict(profile=args.profile, cases=len(cases), fields=len(cases) * 4,
                  exact_cases=len(cases) - len({d['case'] for d in differences}), differences=differences,
                  live_cases=len(live_expected), live_fields=len(live_expected) * 4, live_differences=live_differences,
                  delta_controls=len(delta_pairs), captures=provenance, stability=stability,
                  guards=guard_results, executions=executions,
                  input_sha256=digest(request), analysis_sha256=PE_SHA,
                  limits='Whole original 9f2f80 with mutable442=false and status7; no stubs/code patches/injection. '
                         'Deformation, settings and prior levels supplied. E58000 lazy data page writable/restored. '
                         'Retained non-atomic paused states check idempotence, not incoming-tick evolution. '
                         'Impact/deformation producers, notifications, XML loader and full cadence unexecuted.')
    (out / 'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ('profile', 'cases', 'fields', 'exact_cases', 'live_cases', 'live_fields')}))
    print('different_fields=' + str(len(differences)) + ' live_different_fields=' + str(len(live_differences)))
    assert not differences and not live_differences


if __name__ == '__main__':
    main()
