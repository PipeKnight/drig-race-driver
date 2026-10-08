"""Whole original unassisted steering coefficients versus independent Rust."""
import argparse
import hashlib
import itertools
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    base = [.5, 0., 8., .3, .5, 1.6, 2.6, 1., 1., 0., .016, 2., 3.]
    for i in range(4):
        base += [-.8 if i % 2 == 0 else .8, 0., 0., 0., 1., 0., 1., 0., .9, 0.]
    cases = []
    def add(name, row):
        assert len(row) == 53
        cases.append((name, row))
    for i, (command, mode, speed) in enumerate(itertools.product(
            [-1., -.5, -1.00000008e-7, -1e-7, -0., 0., 1e-7, 1.00000008e-7, .5, 1.],
            [0., 1.], [0., 14.999999, 15., 20.])):
        v = base.copy()
        v[0], v[4], v[9], v[2] = command, 1., mode, speed
        add(f'boundary_{i}', v)
    for i in range(64):
        v = base.copy()
        v[0] = -.7 if i % 2 else .7
        v[7], v[8] = [.0, .25, .75, 1.][i % 4], [.0, .25, .75, 1.][(i // 4) % 4]
        for w in range(4):
            k = 13 + w * 10
            v[k+3] = float(bool(i & (1 << w)))
            v[k+1], v[k+2], v[k+9] = [.0, .1, -.2, 1e-30][w], [.2, -.1, -0., 0.][w], [-.1, .2, 1e-8, 5e-8][w]
            v[k+4:k+8] = [.97, -.2 if w % 2 else .2, .95, -.1 if w % 2 else .1]
        add(f'caps_rotations_frozen_{i}', v)
    for i in range(8):
        v = base.copy()
        v[0], v[10] = -0. if i % 2 else 0., 0.
        for w in range(4):
            k = 13 + w * 10
            v[k+5], v[k+7], v[k+8], v[k+3] = -0., -0., 0., float(bool(i & (1 << (w % 3))))
        add(f'signed_zero_{i}', v)
    rng = random.Random(117)
    pairs = list(itertools.permutations(range(4), 2))
    for i in range(1024):
        v = base.copy()
        v[:11] = [rng.uniform(-1.2,1.2), rng.uniform(-1.,2.), rng.uniform(0.,25.), rng.uniform(-100.,100.),
                  rng.uniform(.1,1.2), rng.uniform(1.,2.5), rng.uniform(2.,4.), rng.uniform(0.,1.2),
                  rng.uniform(0.,1.2), float(i % 2), rng.uniform(0.,.1)]
        v[11:13] = pairs[i % len(pairs)]
        for w in range(4):
            k = 13 + w * 10
            v[k:k+10] = [rng.uniform(-1.,1.), rng.uniform(-.2,.2), rng.uniform(-.2,.2), float(bool(i & (1 << w))),
                         rng.uniform(.7,.9), rng.uniform(-.2,.2), rng.uniform(.7,1.), rng.uniform(-.5,.5),
                         rng.uniform(0.,1.), rng.uniform(-.3,.3)]
        add(f'authored_{i}', v)
    requests = out / 'input.tsv'
    requests.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n' for name,row in cases))
    executions = []
    for kind, exe, argv in [
        ('original', ROOT/'local/behavior/steering-coefficients-probe.exe', [ROOT/'local/compatibility/grid-unpacked.exe', requests, out/'original.tsv']),
        ('native', ROOT/f'target/{args.profile}/grid-steering-coefficients.exe', [requests, out/'native.tsv'])]:
        run = subprocess.run([str(exe),*map(str,argv)], cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Steering coefficient execution failed')
    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            cells = line.split('\t')
            assert len(cells) == 22 and cells[0] == 'P' and cells[1] not in result
            result[cells[1]] = cells[2:]
        return result
    original,native = rows(out/'original.tsv'),rows(out/'native.tsv')
    assert original.keys() == native.keys() == {name for name,_ in cases}
    differences = [dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
    result = dict(profile=args.profile,cases=len(cases),words=len(cases)*20,exact_cases=len(cases)-len({d['case'] for d in differences}),
                  differences=differences,executions=executions,input_sha256=digest(requests),reference_sha256=digest(ROOT/'local/compatibility/grid-unpacked.exe'),
                  limits='Whole original 9b3110 with e6c=false and actual input/trigonometry helpers. RX code/read-only mapped data, no patches/stubs/live injection. '
                         'Supplied finite geometry, pair identities, tuning, body/wheel state and delta. Source initialization, assistance and live cadence remain open.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','cases','words','exact_cases')}))
    print('different_words='+str(len(differences)))
    assert not differences
if __name__ == '__main__':
    main()
