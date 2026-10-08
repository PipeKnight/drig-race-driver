"""Independent source field conversion joined to whole original assisted steering.

Original CSV decoder/constructor are not executed; linkage is aligned static evidence.
"""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def f32(v): return struct.unpack('<f', struct.pack('<f', v))[0]
def word(v): return f'{struct.unpack("<I", struct.pack("<f", v))[0]:08x}'
def constant(bits): return struct.unpack('<f', struct.pack('<I', bits))[0]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    manifest = json.loads((ROOT/'local/behavior/native-body-prelude-validation/validation.json').read_text())
    game = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
    datasets = []
    for entry in manifest['csv_files']:
        path = game/entry['source']
        assert digest(path) == entry['sha256']
        rows = [r for r in csv.reader(path.read_text().splitlines()) if r]
        assert len(rows) == 1 and rows[0][2] == 'Standard'
        datasets.append((path, rows[0], True))
    assert len(datasets) == 44
    # Copied source rows stay private, including these independently selected changes.
    for i, text in enumerate(['', '0', '-0', '1e-46', '-1e-46', '1e-43',
                              '1e-30', '0.100000004', '30.000000953674315', '30.00000095367432', '45']):
        row = datasets[0][1].copy()
        row[77] = '30'
        row[131] = text
        row[132] = text
        row[165] = text
        row[111] = text
        path = out/f'authored-{i:02d}.csv'
        path.write_text(','.join(row)+'\r\n', encoding='ascii', newline='')
        datasets.append((path, row, False))
    seed_dir = ROOT/'local/behavior/assisted-steering-20261008-02'
    seed_report = json.loads((seed_dir/'validation.json').read_text())
    assert seed_report['cases'] == 2320 and not seed_report['differences']
    seed_input = seed_dir/'input.tsv'
    assert digest(seed_input) == seed_report['input_sha256']
    seeds = [line.split('\t')[1:] for line in seed_input.read_text().splitlines()]
    probe = ROOT/'local/behavior/assisted-steering-probe.exe'
    assert digest(probe) == 'e9203e0a327691888f8e8d45b3f6ee5b968039856ab99254a8189272884c1457'
    native = ROOT/f'target/{args.profile}/grid-assisted-steering.exe'
    reports, executions, differences = [], [], []
    for ordinal, (source, row, installed) in enumerate(datasets):
        smoothing = f32(float(row[165] or '0'))
        expected = [f32(float(row[77] or '0')*constant(0x3c8efa35)),
                    f32(float(row[131] or '0')*constant(0x3c23d70a)),
                    f32(float(row[132] or '0')*constant(0x3c8efa35)),
                    1. if smoothing == 0. else smoothing,
                    f32(float(row[111] or '0'))]
        indexes = [4, 92, 91, 104, 90]
        original_rows, native_rows = [], []
        for i in range(48):
            words = seeds[(i*37+ordinal*7)%len(seeds)].copy()
            # Distinct valid placeholders must be ignored by source mode.
            for index, value in zip(indexes, [.333333, .123456, .012345, .765432, 17.6543]):
                words[index] = word(value)
            name = f'condition_{i}'
            native_rows.append(name+'\t'+'\t'.join(words)+'\n')
            for index, value in zip(indexes, expected): words[index] = word(value)
            original_rows.append(name+'\t'+'\t'.join(words)+'\n')
        original_input, native_input = out/f'{ordinal:03d}-original-input.tsv', out/f'{ordinal:03d}-native-input.tsv'
        original_input.write_text(''.join(original_rows)); native_input.write_text(''.join(native_rows))
        original_output, native_output = out/f'{ordinal:03d}-original.tsv', out/f'{ordinal:03d}-native.tsv'
        for kind, exe, argv in [
            ('original', probe, [ROOT/'local/compatibility/grid-unpacked.exe', original_input, original_output]),
            ('native', native, [native_input, native_output, '--source', source])]:
            run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            executions.append(dict(dataset=ordinal, kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions, indent=2))
                raise RuntimeError('Source assisted execution failed')
        original_lines, native_lines = original_output.read_text().splitlines(), native_output.read_text().splitlines()
        assert len(original_lines) == len(native_lines) == 48
        for i, (original, actual) in enumerate(zip(original_lines, native_lines)):
            a, b = original.split('\t'), actual.split('\t')
            assert len(a) == len(b) == 22 and a[:2] == b[:2]
            differences.extend(dict(dataset=ordinal, case=i, field=j, original=x, native=y)
                               for j, (x,y) in enumerate(zip(a[2:], b[2:])) if x != y)
        reports.append(dict(source=str(source), source_sha256=digest(source), installed=installed,
                            expected_words=[word(v) for v in expected],
                            premature_f32_words=[word(f32(float(row[c] or '0'))*constant(f)) for c,f in [(77,0x3c8efa35),(131,0x3c23d70a),(132,0x3c8efa35)]],
                            original_input_sha256=digest(original_input), native_input_sha256=digest(native_input)))
    guards = []
    for name, column, text in [('overflow',131,'1e300'), ('nonfinite',132,'NaN'),
                               ('negative_smoothing',165,'-1'), ('unsupported',2,'6 Wheeler'),
                               ('multiple_rows',None,None)]:
        row = datasets[0][1].copy()
        if column is not None: row[column] = text
        source = out/f'guard-{name}.csv'
        source.write_text(','.join(row)+'\n'+(','.join(row)+'\n' if column is None else ''), encoding='ascii')
        output = out/f'guard-{name}.tsv'
        run = subprocess.run([str(native),str(native_input),str(output),'--source',str(source)],
                             cwd=ROOT,capture_output=True,text=True,timeout=30)
        guards.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_created=output.exists()))
        assert run.returncode and not output.exists()
    result = dict(profile=args.profile, datasets=len(datasets), cases=len(datasets)*48, fields=len(datasets)*48*20,
                  guards=guards,
                  differences=differences, datasets_report=reports, executions=executions,
                  limits='Independent CSV conversion vs whole original assisted writer; original CSV decoder/constructor not executed. '
                         'Remaining geometry/state/settings/delta supplied; no real input/live cadence/full driving claim.')
    (out/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k:result[k] for k in ('profile','datasets','cases','fields')}))
    print('different_fields='+str(len(differences))); assert not differences

if __name__ == '__main__': main()
