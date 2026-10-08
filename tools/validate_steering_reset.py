"""Whole original reset/producer vs deterministic independently authored Rust reset."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '60d9a698f2a9e17a03b754e60009554bbda1a2e0cecff8baf98e8ae575dc6ff1'
PE_SHA = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'


def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v): return struct.unpack('<I', struct.pack('<f', v))[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug','release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    probe = ROOT/'local/behavior/steering-reset-probe.exe'
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    native = ROOT/f'target/{args.profile}/grid-steering-reset.exe'
    assert digest(probe) == PROBE_SHA and digest(image) == PE_SHA
    cases = []
    seeds = [0,1,65534,65535,0x7fffffff,0x80000000,0xfffffffe,0xffffffff]
    for seed in seeds:
        for amplitude in [-2.,-0.,0.,.1,1.,2.]:
            for cap in [0.,.2,1.]:
                for active in [0.,1.]:
                    cases.append((f'boundary_{len(cases)}',seed,[amplitude,cap,1.5,.25,.75,.1,.5,active]))
    for seed in seeds:
        for start,end in [(.25,.75),(-.25,.75),(-1.,0.),(.75,.25),(-.75,-.25)]:
            for active in [0.,1.]:
                cases.append((f'ramp_{len(cases)}',seed,[.3,.8,3.,start,end,.3,-.25,active]))
    rng = random.Random(128)
    for i in range(1024):
        cases.append((f'authored_{i}',rng.getrandbits(32),[rng.uniform(-3.,3.),rng.random(),rng.uniform(-3.,3.),
                     rng.uniform(-1.,1.),rng.uniform(-1.,1.),rng.uniform(-.5,1.5),rng.uniform(-1.,2.),float(i%2)]))
    def request_line(name,seed,row):
        return name+'\t'+f'{seed:08x}'+'\t'+'\t'.join(f'{word(v):08x}' for v in row)+'\n'
    request = out/'input.tsv'
    request.write_text(''.join(request_line(*case) for case in cases))
    executions = []
    for kind,exe,argv in [('original',probe,[image,request,out/'original.tsv']),
                          ('native',native,[request,out/'native.tsv'])]:
        run = subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2))
            raise RuntimeError('Steering reset execution failed')
    def rows(path):
        result = {}
        for line in path.read_text().splitlines():
            cells = line.split('\t')
            assert len(cells) == 20 and cells[0] == 'P' and cells[1] not in result
            result[cells[1]] = cells[2:]
        return result
    original,actual = rows(out/'original.tsv'),rows(out/'native.tsv')
    assert original.keys() == actual.keys() == {name for name,_,_ in cases}
    differences = [dict(case=name,field=i,original=a,native=b) for name in original
                   for i,(a,b) in enumerate(zip(original[name],actual[name])) if a != b]
    guards = []
    for name,changes in [('nonfinite',{0:float('nan')}),('cap_below_zero',{1:-.1}),
                         ('cap_above_one',{1:1.1}),('bad_flag',{7:.5}),('unordered_ramp',{3:0.,4:0.})]:
        row = cases[0][2].copy()
        for index,value in changes.items(): row[index] = value
        request = out/f'guard-{name}-input.tsv'
        request.write_text(request_line(name,0,row))
        target = out/f'guard-{name}.tsv'
        run = subprocess.run([str(native),str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        guards.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_created=target.exists()))
        assert run.returncode and not target.exists()
    result = dict(profile=args.profile,cases=len(cases),fields=len(cases)*18,
                  exact_cases=len(cases)-len({d['case'] for d in differences}),differences=differences,
                  guards=guards,executions=executions,input_sha256=digest(out/'input.tsv'),analysis_sha256=PE_SHA,
                  limits='Whole original9e6070->9c5b90; supplied RNG seed and seven parameters, secondary component absent, '
                         'wheel component queries inactive, four wheels. Six seed words, seven steering scalars, four flags '
                         'and final RNG state compared. Original eighteen cleared words/five status fields also checked. '
                         'Private mapped RNG page writable/restored, code RX, no patches/stubs/injection/entry point. '
                         'Source setup/full constructor, live RNG/cadence, other damage outputs unverified.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','cases','fields','exact_cases')}))
    print('different_fields='+str(len(differences)))
    assert not differences


if __name__ == '__main__': main()
