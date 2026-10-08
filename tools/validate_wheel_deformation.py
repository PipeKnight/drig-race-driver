"""Whole original/Rust wheel geometry comparison with private authored fixtures."""
import argparse
import json
import random
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '1afaf44d6f11ee60b508c8d964082bd3702156934172f3741fd6da8c18b94526'


def read(path):
    result = {}
    for line in path.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == 130 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--prepared', type=Path, required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--retained-only', action='store_true')
    args = parser.parse_args()
    out, prepared = args.output.resolve(), args.prepared.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and prepared.is_relative_to((ROOT / 'local').resolve())
    source, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformed-wheel-geometry-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-wheel-deformation.exe'
    assert digest(source) == PE_SHA and digest(probe) == PROBE_SHA
    prepared_report = json.loads((prepared / 'execution.json').read_text())
    assert prepared_report['analysis_sha256'] == PE_SHA and prepared_report['probe_sha256'] == PROBE_SHA
    assert prepared_report['execution']['exit'] == 0 and digest(prepared / 'input.tsv') == prepared_report['input_sha256']
    out.mkdir(exist_ok=False)
    cases = [(cells[0], cells[1:]) for cells in (line.split('\t') for line in (prepared / 'input.tsv').read_text().splitlines())]
    rng = random.Random(134)
    for index in range(0 if args.retained_only else 640):
        # Smooth bounded maps keep the original normalization/acos outputs finite.
        dimensions = [3, 4, 5]
        spacing = rng.uniform(.7, 1.4)
        minimum = [-spacing, -spacing, -2 * spacing, 1.]
        center = [rng.uniform(-1., 1.) for _ in range(3)] + [1.]
        scale = [0., .25, 1., 2., 4.][index % 5]
        limit = [0., -0., -27., .01, 1., 27., 90.][index % 7]
        thresholds = [rng.choice([-1., -0., 0., .01, .2, 10.]) for _ in range(2)]
        active = float(index % 8 != 0)
        wheel_values = []
        for wheel in range(4):
            point = [center[0] + (.4 if wheel & 1 else -.4) * spacing,
                     center[1] + .2 * spacing, center[2] + (.6 if wheel < 2 else -.6) * spacing, 1.]
            radius = rng.uniform(.15, .4) * spacing
            wheel_values += point + [radius, radius * 1.1, radius * .8, float((index + wheel) % 3 == 0)]
        previous = [rng.uniform(-1., 1.) for _ in range(124)]
        shear = [rng.uniform(-.5, .5) for _ in range(6)]
        offset = [rng.uniform(-.2, .2) for _ in range(3)]
        warp = [rng.uniform(-.025, .025) for _ in range(4)] if index >= 256 else [0.] * 4
        if index < 32:
            shear = [0.] * 6
            offset = [0.] * 3
        elif index < 64:
            shear = [0.] * 6
        nodes = []
        for z in range(dimensions[2]):
            for y in range(dimensions[1]):
                for x in range(dimensions[0]):
                    a, b, c = [minimum[i] + q * spacing for i, q in enumerate((x, y, z))]
                    nodes += [a + shear[0] * b + shear[1] * c + offset[0] + warp[0] * b * c,
                              b + shear[2] * a + shear[3] * c + offset[1] + warp[1] * a * c,
                              c + shear[4] * a + shear[5] * b + offset[2] + warp[2] * a * b,
                              1. + warp[3] * a * b * c]
        values = list(map(float, dimensions)) + [spacing] + minimum + center + [scale, limit] + thresholds + [active]
        values += wheel_values + previous + nodes
        assert len(values) == 173 + 4 * 60
        cases.append((f'authored_{index}', [f'{word(v):08x}' for v in values]))
    request = out / 'input.tsv'
    request.write_text(''.join(name + '\t' + '\t'.join(v) + '\n' for name, v in cases))
    execution = {}
    for label, command in [('original', [str(probe), str(source)]), ('native', [str(native)])]:
        run = subprocess.run(command + [str(request), str(out / f'{label}.tsv')], cwd=ROOT,
                             capture_output=True, text=True, timeout=60)
        execution[label] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        (out / 'execution.json').write_text(json.dumps(execution, indent=2))
        assert run.returncode == 0, execution[label]
    original, rewritten = read(out / 'original.tsv'), read(out / 'native.tsv')
    assert original.keys() == rewritten.keys() == dict(cases).keys()
    differences = [dict(case=k, field=i, original=a, native=b) for k in original
                   for i, (a, b) in enumerate(zip(original[k], rewritten[k])) if a != b]
    guards = []
    for label, mutate in [
        ('zero_spacing', lambda v: v.__setitem__(3, '00000000')),
        ('invalid_flag', lambda v: v.__setitem__(16, '40000000')),
        ('invalid_radius', lambda v: v.__setitem__(21, '00000000')),
        ('nonfinite', lambda v: v.__setitem__(12, '7fc00000')),
        ('short_nodes', lambda v: v.pop()),
    ]:
        values = cases[0][1].copy()
        mutate(values)
        invalid = out / f'{label}.tsv'
        invalid.write_text(label + '\t' + '\t'.join(values) + '\n')
        records = {}
        for program, command in [('original', [str(probe), str(source)]), ('native', [str(native)])]:
            target = out / f'{label}-{program}.tsv'
            run = subprocess.run(command + [str(invalid), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            records[program] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
            assert run.returncode != 0 and (not target.exists() or target.stat().st_size == 0)
            if program == 'native':
                assert not target.exists()
        guards.append(dict(case=label, executions=records))
    report = dict(profile=args.profile, cases=len(cases), fields=len(cases) * 128, exact=not differences,
                  differences=differences, executions=execution, guards=guards,
                  analysis_sha256=PE_SHA, probe_sha256=PROBE_SHA, native_sha256=digest(native), input_sha256=digest(request),
                  prepared_report_sha256=digest(prepared / 'execution.json'), supplied_retained_cases=len(prepared_report['cases']),
                  limits='Whole original9bb1e0 unchanged RX, four restored private lazy pages, no patches/stubs/injection. '
                         f"{0 if args.retained_only else 640} authored maps/policies plus{len(prepared_report['cases'])} retained-grid cases; "
                         f"captured policy={prepared_report.get('captured_policy', False)}. "
                         'Not synchronized live replay, grid deformation production or simulation cadence.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(cases=len(cases), fields=report['fields'], exact=report['exact'], differences=len(differences))))
    assert not differences, differences[:8]


if __name__ == '__main__':
    main()
