"""Compare joined original wheel routines and Rust across supplied-geometry frames.

The native harness authors the observed schedule and post-solver rate additions;
it does not execute the whole vehicle wrapper or production contact preparation.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def cases():
    result = []
    identity = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.]

    def add(name, frames, body, delta, scale, alternate, rates, wheels):
        values = body + [delta, scale] + alternate + rates + sum(wheels, [])
        assert len(values) == 181 and all(math.isfinite(v) for v in values)
        result.append((name, [frames] + [word(v) for v in values]))

    def wheel(point, axis=None, projection=None):
        return (point + (axis or [0., 1., 0., 0.]) + (projection or [0., 1., 0., 0.])
                + [0., 0., 40000., -.07, .4, 1800., .15, .6, 1., 12000., 8000.,
                   0., -.1, 0., 0., 0., 0., 0., -.2, .4, .4, 0., 0.])

    deltas = [.0001, .001, .002, .01, 1/60, .02, .033, .25]
    for i in range(32):
        body = [1200., 600., 1700., 1800.] + identity + [0.]*8
        wheels = [wheel([(-.75 if j % 2 else .75), 0., (-1.2 if j < 2 else 1.3), 1.]) for j in range(4)]
        for j, w in enumerate(wheels):
            w[12] = -.002 if j % 2 else .002
            w[13] = -.5 if i % 2 else .5
            w[24] = -.5 if i < 8 else .003
            if i >= 16:
                w[30:32] = [-.001, .001]
            if i >= 24:
                w[26:30] = [1., -.5, .25, -.125]
                w[8:12] = [.1, 1., -.1, 0.]
        # Long controls use short steps; .25 is a bounded one-frame stress case.
        frames = 1 if i < 24 else 120
        delta = deltas[i % 8] if i < 24 else .001
        rates = [0.]*8 if i % 3 else [.1, -.2, .3, 0., .01, -.02, .03, 0.]
        add(f'control_{i:02d}', frames, body, delta, 1., [900., 2000., 2200.], rates, wheels)

    snapshots = []
    # Reuse previously verified numeric inputs at explicit supplied boundaries.
    # Combining them here does not make them synchronized live frame snapshots.
    source_dirs = ['isolated-wheel-step-20261007-05', 'isolated-finalize-20261007-03']
    source_rows = []
    for directory in source_dirs:
        path = ROOT/'local/behavior'/directory/'input.tsv'
        report = json.loads(path.with_name('validation.json').read_text())
        assert digest(path) == report['input_sha256']
        source_rows.append({r[0]: [struct.unpack('<f', struct.pack('<I', int(w, 16)))[0] for w in r[1:]]
                            for line in path.read_text().splitlines() if (r := line.split())})
        snapshots.append({'path': str(path), 'sha256': digest(path)})
    for label in ('paused', 'throttle'):
        for i in range(12):
            name = f'{label}_{i:02d}'
            a, b = source_rows[0][name], source_rows[1][name]
            wheels = []
            for j in range(4):
                w, f = a[35+j*26:61+j*26], b[32+j*22:54+j*22]
                wheels.append(f[:12] + w[:11] + [w[16], w[17], w[18]] + w[19:23]
                              + [w[24], w[25], w[4], f[21], w[23]])
            add('replay_' + name, 12, b[:28], 1/60, a[2], b[29:32], [0.]*8, wheels)

    rng = random.Random(2008163)
    for i in range(160):
        angle = rng.uniform(-math.pi, math.pi)
        c, s = math.cos(angle), math.sin(angle)
        position = [rng.uniform(-50, 50), rng.uniform(-2, 2), rng.uniform(-50, 50), 1.]
        basis = [c, 0., -s, 0., 0., 1., 0., 0., s, 0., c, 0.] + position
        params = [rng.uniform(600, 2000)] + [rng.uniform(400, 2500) for _ in range(3)]
        body = params + basis + [rng.uniform(-2, 2) for _ in range(3)] + [0.] + [rng.uniform(-.2, .2) for _ in range(3)] + [0.]
        wheels = []
        for j in range(4):
            point = [position[k]+rng.uniform(-1.5, 1.5) for k in range(3)] + [1.]
            w = wheel(point, [rng.uniform(-.1, .1), rng.uniform(.9, 1.), rng.uniform(-.1, .1), 0.],
                      [rng.uniform(-.15, .15), 1., rng.uniform(-.15, .15), 0.])
            w[12:16] = [rng.uniform(-.01, .01), rng.uniform(-.2, .2), rng.uniform(10000, 60000), rng.uniform(-.1, 0.)]
            w[17] = rng.uniform(500, 3000)
            w[23:26] = [rng.uniform(-.5, .5), rng.uniform(-.03, .03), rng.uniform(-.1, .1)]
            w[26:30] = [rng.uniform(-1, 1) for _ in range(4)]
            w[34] = float(i % 2)
            wheels.append(w)
        rates = [rng.uniform(-.1, .1) for _ in range(3)] + [0.] + [rng.uniform(-.01, .01) for _ in range(3)] + [0.]
        add(f'synthetic_{i:03d}', 8, body, rng.uniform(.002, .02), rng.uniform(.8, 1.2), [v*1.25 for v in params[1:]], rates, wheels)
    return result, snapshots


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    data, snapshots = cases()
    request = out/'input.tsv'
    request.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name, words in data), encoding='ascii')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions = []
    for exe, arguments in [
        (ROOT/'local/behavior/suspension-frame-probe.exe', [image, request, out/'original.tsv']),
        (ROOT/'local/behavior/suspension-frame-probe.exe', [image, request, out/'repeat.tsv']),
        (ROOT/f'target/{args.profile}/grid-suspension-frame.exe', [request, out/'rust.tsv'])]:
        run = subprocess.run([str(exe), *map(str, arguments)], cwd=ROOT, capture_output=True, text=True, timeout=90)
        executions.append({'path': str(exe), 'sha256': digest(exe), 'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Joined suspension comparison failed; evidence retained')

    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            row = line.split('\t')
            assert row[0] == 'P' and len(row) == 143
            key = (row[1], int(row[2]))
            assert key not in result
            result[key] = row[3:]
        return result

    original, repeat, rust = [rows(out/name) for name in ('original.tsv', 'repeat.tsv', 'rust.tsv')]
    expected = {(name, frame) for name, words in data for frame in range(words[0])}
    assert set(original) == set(repeat) == set(rust) == expected
    comparisons = []
    for key in sorted(expected):
        diffs = [{'field': i, 'original': a, 'repeat': b, 'rust': c}
                 for i, (a, b, c) in enumerate(zip(original[key], repeat[key], rust[key])) if a != b or a != c]
        comparisons.append({'case': key[0], 'frame': key[1], 'differences': diffs})
    def f(value):
        return struct.unpack('<f', struct.pack('<I', int(value, 16)))[0]

    initial = {name: values for name, values in data}
    force_changes = pose_changes = 0
    for (name, frame), row in original.items():
        assert row[139] == '00000000'  # Finalizer consumes its whole-frame clock.
        delta = f(f'{initial[name][29]:08x}')
        for i in range(4):
            a, w = 39+i*10, 107+i*8
            assert row[w:w+2] == row[a:a+2]  # q/speed retained at finalization.
            assert row[w+4:w+8] == ['00000000']*4  # No impulse replay next frame.
            previous = (f(original[(name, frame-1)][w+3]) if frame
                        else f(f'{initial[name][1+41+i*35+33]:08x}'))
            expected_force = word(struct.unpack('<f', struct.pack('<f', f(row[a+5])/delta))[0] + previous)
            assert int(row[w+3], 16) == expected_force
            force_changes += f(row[w+3]) != previous
        previous_pose = (original[(name, frame-1)][83:99] if frame
                         else [f'{v:08x}' for v in initial[name][5:21]])
        pose_changes += row[83:99] != previous_pose
    assert force_changes > 0 and pose_changes > 0
    malformed = []
    exe = ROOT/f'target/{args.profile}/grid-suspension-frame.exe'
    for name, index, bad in [('zero_frames', 0, 0), ('too_many_frames', 0, 121), ('zero_mass', 1, 0),
                             ('nan_delta', 29, 0x7fc00000), ('zero_delta', 29, 0), ('bad_scale', 30, 0),
                             ('zero_inertia', 2, 0), ('zero_alternate', 31, 0), ('bad_flag', 76, word(2.)),
                             ('shared_travel', 74, word(.2))]:
        values = data[0][1].copy()
        values[index] = bad
        path = out/(name+'.tsv')
        path.write_text('bad\t'+'\t'.join(f'{w:08x}' for w in values)+'\n', encoding='ascii')
        target = out/(name+'-result.tsv')
        run = subprocess.run([str(exe), str(path), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=15)
        malformed.append({'name': name, 'exit': run.returncode, 'stderr': run.stderr, 'no_output': not target.exists()})
        assert run.returncode != 0 and not target.exists()
    total = len(expected)*140
    summary = {'scenarios': len(data), 'frames': len(expected), 'compared_words': total,
               'exact_words': total-sum(len(c['differences']) for c in comparisons),
               'exact_frames': sum(not c['differences'] for c in comparisons), 'malformed_guards': len(malformed),
               'semantic_checks': 5, 'force_changes': force_changes, 'pose_changes': pose_changes,
               'substep_counts': sorted({int(row[0]) for row in original.values()})}
    report = {'profile': args.profile, 'summary': summary, 'executions': executions,
              'image_sha256': digest(image), 'input_sha256': digest(request), 'snapshots': snapshots,
              'comparisons': comparisons, 'malformed': malformed,
              'limits': 'Whole original 9E3970 coupling, 9C1F60 substeps, 9E4B10 finalizer and dependencies unstubbed. '
                        'Schedule and post-rate application authored from previously compared wrapper contract. '
                        'Fixed supplied world points/axes, carried wheel floors/contact speeds, no refreshed geometry; '
                        'four active wheels, extra tyre axes inactive, contact-object pointers null. '
                        'Source-input combinations are controlled replays, not synchronized live frames. '
                        'No whole vehicle wrapper, drivetrain, production contact preparation or driving equivalence.'}
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({'profile': args.profile, **summary}))
    assert summary['exact_frames'] == len(expected)


if __name__ == '__main__':
    main()
