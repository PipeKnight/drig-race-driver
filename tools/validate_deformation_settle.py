"""Compare whole supplied deformation settling with isolated original execution."""
import argparse
import json
import random
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA
from prepare_deformation_settle import PROBE_SHA

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--prepared', type=Path, required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--retained-only', action='store_true')
    args = parser.parse_args()
    out, prepared = args.output.resolve(), args.prepared.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and prepared.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformation-settle-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-deformation-settle.exe'
    report = json.loads((prepared / 'execution.json').read_text())
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    assert report['execution']['exit'] == 0 and digest(prepared / 'input.tsv') == report['input_sha256']
    cases = [(cells[0], cells[1:]) for cells in (line.split('\t') for line in (prepared / 'input.tsv').read_text().splitlines())]
    rng = random.Random(0x9a26f0)
    for i in range(0 if args.retained_only else 160):
        spacing = [.125, .5, 1., 2.][i % 4]
        threshold = [-.25, -0., 0., .01, .25][i % 5]
        gain = [-2., -0., 0., .5, 20.][i % 5]
        nodes = []
        for j in range(400):
            rest = [(j % 4) * spacing, ((j // 4) % 3) * spacing, (j // 12) * spacing, 1.]
            bounds = [rng.choice([0., .1, .5, 2.]) for _ in range(4)]
            position = [v + rng.uniform(-1., 1.) for v in rest]
            if i < 16:
                position = rest.copy()
            if j == 0 and i % 8 == 0:
                rest = [-0., 0., -0., 1.]
                position = [0., -0., 0., 1.]
                bounds = [0.] * 4
            nodes += position + rest + bounds
        edges = []
        if i < 32:
            for _ in range([0, 1, 63, 64, 65, 127, 128, 129][i % 8]):
                edges += [0., 0., -0.]
        else:
            for _ in range([1, 64, 200, 1200][i % 4]):
                edges += [float(rng.randrange(400)), float(rng.randrange(400)), rng.uniform(-1., 1.)]
        values = [4., 3., 5., spacing, float(i % 3), threshold, gain, float(len(edges) // 3)] + nodes + edges
        cases.append((f'authored_{i}', [f'{word(v):08x}' for v in values]))
    request = out / 'input.tsv'
    request.write_text(''.join(name + '\t' + '\t'.join(values) + '\n' for name, values in cases))
    assert request.stat().st_size <= 16 * 1024 * 1024
    execution, outputs = {}, {}
    for label, command in [('original', [str(probe), str(image)]), ('native', [str(native)])]:
        target = out / f'{label}.tsv'
        run = subprocess.run(command + [str(request), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        execution[label] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        (out / 'execution.json').write_text(json.dumps(execution, indent=2))
        assert run.returncode == 0, execution[label]
        outputs[label] = {}
        for line in target.read_text().splitlines():
            cells = line.split('\t')
            assert cells[0] == 'P' and cells[1] not in outputs[label]
            outputs[label][cells[1]] = cells[2:]
    original, rewritten = outputs['original'], outputs['native']
    assert original.keys() == rewritten.keys() == dict(cases).keys()
    assert all(len(original[name]) == len(rewritten[name]) == 1600 + (len(values)-4808)//3 for name, values in cases)
    differences = [dict(case=name, field=i, original=a, native=b) for name in original
                   for i, (a, b) in enumerate(zip(original[name], rewritten[name])) if a != b]
    guards = []
    for label, mutate in [
        ('zero_spacing', lambda v: v.__setitem__(3, '00000000')),
        ('invalid_mode', lambda v: v.__setitem__(4, '40400000')),
        ('negative_bound', lambda v: v.__setitem__(16, 'bf800000')),
        ('invalid_endpoint', lambda v: v.__setitem__(4808, f'{word(400.):08x}')),
        ('nonfinite', lambda v: v.__setitem__(6, '7fc00000')),
        ('short_pool', lambda v: v.pop()),
    ]:
        values = cases[0][1].copy()
        mutate(values)
        invalid = out / f'{label}.tsv'
        invalid.write_text(label + '\t' + '\t'.join(values) + '\n')
        records = {}
        for program, command in [('original', [str(probe), str(image)]), ('native', [str(native)])]:
            target = out / f'{label}-{program}.tsv'
            run = subprocess.run(command + [str(invalid), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            assert run.returncode != 0 and (not target.exists() or target.stat().st_size == 0)
            if program == 'native':
                assert not target.exists()
            records[program] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
        guards.append(dict(case=label, execution=records))
    final = dict(profile=args.profile, cases=len(cases), fields=sum(map(len, original.values())), exact=not differences,
                 differences=differences, guards=guards, execution=execution, analysis_sha256=PE_SHA, probe_sha256=PROBE_SHA,
                 native_sha256=digest(native), input_sha256=digest(request), prepared_report_sha256=digest(prepared / 'execution.json'),
                 authored_cases=0 if args.retained_only else 160,
                 limits='Supplied fixed400-node pool/graph/settings contract. Authored cases cover clipping, W reduction, modes, zero/negative gains, self-edges and wrapping signed incident counts when requested. No collision input, grid construction or live cadence proof.')
    (out / 'validation.json').write_text(json.dumps(final, indent=2))
    print(json.dumps(dict(cases=len(cases), fields=final['fields'], exact=final['exact'], differences=len(differences))))
    assert not differences, differences[:8]


if __name__ == '__main__':
    main()
