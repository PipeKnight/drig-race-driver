"""Whole original speed-clamped grip/table query against source and authored rows.

The table is supplied, not loaded by the original CSV reader. No live force claim.
"""
import argparse
import csv
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def f32(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def word(v):
    return struct.unpack('<I', struct.pack('<f', v))[0]


def code(v):
    return int.from_bytes(v.encode('ascii')[:4], 'little')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    game = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
    source = game / 'cars/tyreGrip.csv'
    source_hash = '19383cb996b1d534fd3e4643ebcdb99edb8071ffd85ec21a469a30214e672a5c'
    assert digest(source) == source_hash
    surfaces = game / 'surface_materials.xml'
    assert digest(surfaces) == '8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    data = list(csv.reader(source.read_text().splitlines()))
    keys = [code(k) if len(k) >= 4 else 0xffffffff for k in data[0][2:] if k]
    rows = [[f32(float(v)*f32(.01)) for v in r[2:2+len(keys)]] for r in data[2:] if r and r[0]]
    assert len(keys) == 15 and len(rows) == 13 and all(len(r) == len(keys) for r in rows)
    _, root, _ = decode(surfaces.read_bytes())
    cases = []
    for i, q in enumerate(root.findall('MATERIAL/MECHANICS')):
        a = q.attrib
        slope, maximum, base = [f32(float(a.get(k,d))) for k,d in [('grip_per_ms',0),('ms_for_max_effect',1),('grip',1)]]
        name = a.get('tyreTable', 'DEFA')
        key = code(name) if len(name) >= 4 else code('DEFA')
        for j, speed in enumerate([-1., -0., 0., maximum*.5, maximum, maximum+1.]):
            for active in [0,1]:
                cases.append((f'material_{i}_{j}_{active}',[active,key,1,i%13,word(slope),word(maximum),word(base),word(speed)]))
    installed = ('installed', keys, rows, cases)
    rng = random.Random(105)
    keys = [0, code('TEST'), code('TEST'), code('MUD '), code('MUD\0'), code('DEFA')]
    rows = [[f32(rng.uniform(-1.,3.)) for _ in keys] for _ in range(13)]
    cases = []
    for i in range(384):
        active = int(i%4 != 0)
        unresolved = int(i%2 == 0)
        key = (keys[(i//2)%len(keys)] if i%7 else 0xdeadbeef) if unresolved else i%len(keys)
        slope, maximum, base = f32(rng.uniform(-2,2)), f32(rng.uniform(0,10)), f32(rng.uniform(-2,2))
        speed = [-0., -1., maximum, f32(maximum+1), f32(rng.uniform(0,maximum))][i%5]
        cases.append((f'authored_{i}', [active,key,unresolved,i%13,word(slope),word(maximum),word(base),word(speed)]))
    # Empty active tables keep unresolved tokens and produce the default factor.
    empty = [('empty_active',[1,code('DEFA'),1,0,word(.3),word(1),word(1),word(.5)]),
             ('empty_inactive_cached',[0,99,0,12,word(.3),word(1),word(1),word(.5)])]
    datasets = [installed, ('authored', keys, rows, cases), ('empty', [], [[] for _ in range(13)], empty)]
    comparisons, executions = [], []
    for label, keys, rows, cases in datasets:
        fixture = out / f'{label}.private.bin'
        fixture.write_bytes(b'TYG1'+struct.pack('<I',len(keys))+b''.join(struct.pack('<I',k) for k in keys)+
                            b''.join(struct.pack('<f',v) for row in rows for v in row))
        requests = out / f'{label}-input.tsv'
        requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words in cases))
        independent, premature_differences = [], 0
        for name, words in cases:
            active, key, pending, row = words[:4]
            factor = 1.
            if active:
                if pending and key in keys:
                    key, pending = keys.index(key), 0
                if not pending:
                    factor = rows[row][key]
            slope, maximum, base, speed = [struct.unpack('<f',struct.pack('<I',w))[0] for w in words[4:]]
            speed = 0. if speed < 0. else maximum if speed > maximum else speed
            grip = f32((slope*speed+base)*factor)
            premature = f32(f32(slope*speed+base)*factor)
            premature_differences += word(grip) != word(premature)
            independent.append(f'P\t{name}\t{word(grip):08x}\t{key:08x}\t{pending:08x}')
        for kind, exe, arguments in [
            ('original', ROOT/'local/behavior/tyre-grip-probe.exe',
             [ROOT/'local/compatibility/grid-unpacked.exe',fixture,requests,out/f'{label}-original.tsv']),
            ('native',ROOT/f'target/{args.profile}/grid-tyre-grip.exe',
             [fixture,requests,out/f'{label}-native.tsv']),
        ]:
            result = subprocess.run([str(exe), *map(str,arguments)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            executions.append(dict(kind=kind,label=label,sha256=digest(exe),exit=result.returncode,
                                   stdout=result.stdout,stderr=result.stderr))
            if result.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2))
                raise RuntimeError('Grip comparison failed; evidence retained')
        original = (out/f'{label}-original.tsv').read_text().splitlines()
        native = (out/f'{label}-native.tsv').read_text().splitlines()
        assert len(original) == len(native) == len(independent)
        differences = [dict(case=cases[i][0],original=a,native=b,independent=c)
                       for i,(a,b,c) in enumerate(zip(original,native,independent)) if not a==b==c]
        comparisons.append(dict(label=label,cases=len(cases),exact=len(cases)-len(differences),
                                premature_f32_differences=premature_differences,differences=differences,
                                fixture_sha256=digest(fixture)))
        if label == 'authored':assert premature_differences == 39
    assert digest(source) == source_hash
    result = dict(profile=args.profile,source_sha256=source_hash,comparisons=comparisons,executions=executions,
                  exact_cases=sum(c['exact'] for c in comparisons), output_words=3*sum(c['cases'] for c in comparisons),
                  limits='Whole original 99c990/998690, supplied table/header/row selector and material input. '
                         'Private data page writable/restored; code RX without patches/stubs. x87 precision53/nearest. '
                         'Source table framing/scaling independently adapted; original CSV loader not executed. '
                         'No complete 9ba490, synchronized live material or force integration claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:v for k,v in result.items() if k in ('profile','exact_cases','output_words')}))
    assert all(not c['differences'] for c in comparisons)


if __name__ == '__main__':
    main()
