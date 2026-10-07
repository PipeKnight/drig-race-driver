"""Controlled original four-wheel suspension block versus independent Rust."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def value(bits):
    return struct.unpack('<f', struct.pack('<I', bits))[0]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    wheel = [.1, 0., 50000., 0., .35, 3000., .2, .5, .8, 5000., 20000.]
    cases = []
    def add(name, wheels, scale=1., delta=.001, weight=.75):
        values=[delta,weight,scale]+[v for w in wheels for v in w]
        assert len(values)==47
        cases.append((name,[word(v) for v in values]))
    for index,speed in enumerate([-2.,-.2,-.19999999,-0.0,0.,.19999999,.2,2.]):
        w=wheel.copy();w[1]=speed
        add(f'damping_{index}',[w.copy() for _ in range(4)])
    for index,q in enumerate([-.5,0.,.1,.35,.5]):
        w=wheel.copy();w[0]=q
        add(f'travel_{index}',[w.copy() for _ in range(4)])
    for index in range(4):
        wheels=[wheel.copy() for _ in range(4)];wheels[index][0]=.15
        add(f'coupling_{index}',wheels)
    capture_path = ROOT / 'local/behavior/live-spawn-20261007-08/capture.json'
    capture = json.loads(capture_path.read_text(encoding='utf-8-sig'))
    assert capture['completed'] and capture['code_exact'] and len(capture['cases']) == 12
    assert digest(capture_path) == '7007c2feb3efc141673788634446af80251bd8835c83da1b3b82b253cd83d2c4'
    offsets = [0x5c,0x54,0x4c,0x60,0x64,0x68,0x114,0x58,0x70]
    for ordinal, case in enumerate(capture['cases']):
        raw = bytes.fromhex(case['wheels_hex'])
        for variant, speed in enumerate([-.5, -.1, .1, .5]):
            wheels = [[.03, speed] + [struct.unpack_from('<f',raw,i*0x150+offset)[0]
                       for offset in offsets] for i in range(4)]
            add(f'recorded_coefficients_{ordinal:02d}_{variant}', wheels)
    rng = random.Random(2008)
    for index in range(160):
        wheels=[]
        for _ in range(4):
            wheels.append([rng.uniform(-.5,.5),rng.uniform(-2,2),rng.uniform(1000,80000),
                rng.uniform(-100,100),rng.uniform(.1,.8),rng.uniform(100,7000),
                rng.uniform(.01,1),rng.uniform(0,1),rng.uniform(.1,1.5),
                rng.uniform(0,20000),rng.uniform(0,60000)])
        add(f'synthetic_{index:03d}',wheels,scale=rng.uniform(.5,2),
            delta=rng.uniform(.0001,.005),weight=rng.uniform(0,1))
    requests = out / 'input.tsv'
    with requests.open('x', encoding='ascii') as stream:
        for name, words in cases:
            assert len(words) == 47
            stream.write(name + '\t' + '\t'.join(f'{v:08x}' for v in words) + '\n')
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    programs = [(ROOT / 'local/behavior/suspension-probe.exe',
                 [image, requests, out / 'original.tsv']),
                (ROOT / f'target/{args.profile}/grid-suspension.exe',
                 [requests, out / 'native.tsv'])]
    executions = []
    for executable, arguments in programs:
        run = subprocess.run([str(executable), *map(str, arguments)], cwd=ROOT,
                             capture_output=True, text=True, timeout=30)
        executions.append({'path': str(executable), 'sha256': digest(executable), 'exit': run.returncode,
                           'stdout': run.stdout, 'stderr': run.stderr})
        if run.returncode:
            (out / 'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Controlled suspension failed; evidence retained')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            row=line.split('\t')
            assert row[0]=='P' and len(row)==10 and row[1] not in result
            result[row[1]]=row[2:]
        return result
    original,native=rows(out/'original.tsv'),rows(out/'native.tsv')
    assert set(original)==set(native)=={name for name,_ in cases}
    comparisons=[]
    for name,_ in cases:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(original[name],native[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    report={'profile':args.profile,'executions':executions,'image_sha256':digest(image),'input_sha256':digest(requests),
        'capture_sha256':digest(capture_path),
        'cases':comparisons,'summary':{'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
        'compared_float_words':len(cases)*8,'exact_float_words':len(cases)*8-sum(len(c['differences']) for c in comparisons)},
        'limits':'Original 0x9c1f60 substep runs without stubs using four active wheels. '
        'Controlled coefficient/state inputs; tyre/drivetrain flag zero, coupling matrices/axes zero, '
        'wide travel/contact limits and zero initial accumulators. Probe rejects contact impulse/limit activation. '
        'Only suspension raw and weighted impulse accumulators compared. Not live wheel conditions, '
        'production coefficient preparation, whole solver or driving equivalence.'}
    with (out / 'validation.json').open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
    print(json.dumps(report['summary']))
    assert report['summary']['exact_cases'] == len(cases)


if __name__ == '__main__':
    main()
