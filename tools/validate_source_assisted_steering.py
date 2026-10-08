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
from validate_binxml import decode
from validate_wheel_geometry import layout

ROOT = Path(__file__).resolve().parents[1]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def f32(v): return struct.unpack('<f', struct.pack('<f', v))[0]
def word(v): return f'{struct.unpack("<I", struct.pack("<f", v))[0]:08x}'
def constant(bits): return struct.unpack('<f', struct.pack('<I', bits))[0]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--all-parameters', action='store_true')
    parser.add_argument('--geometry', action='store_true')
    parser.add_argument('--wheel-initialization', action='store_true')
    args = parser.parse_args()
    if args.wheel_initialization: args.geometry = True
    if args.geometry: args.all_parameters = True
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
    ned_paths = {}
    if args.geometry:
        wheel_manifest = ROOT/'local/behavior/native-wheel-geometry-validation-v3/validation.json'
        assert digest(wheel_manifest) == '0a0a2f45301cfe8f45cad722187b8b55cb42c1f6890f2ae456f5c5b9ec70c859'
        for entry in json.loads(wheel_manifest.read_text())['sources']:
            source = game/entry['source']; ned = source.with_suffix('.ned')
            assert digest(source) == entry['csv_sha256'] and digest(ned) == entry['ned_sha256']
            ned_paths[source] = ned
        datasets = [entry for entry in datasets if entry[0] in ned_paths]
        assert len(datasets) == 43
    # Copied source rows stay private, including these independently selected changes.
    controls = ['', '0', '-0', '1e-46', '-1e-46', '1e-43',
                '1e-30', '0.100000004', '30.000000953674315', '30.00000095367432', '45']
    if args.all_parameters: controls += ['0.5', '0.5000000000000001', '0.49999999999999994']
    for i, text in enumerate(controls):
        row = datasets[0][1].copy()
        row[77] = '30'
        row[131] = text
        row[132] = text
        row[165] = text
        row[111] = text
        if args.all_parameters:
            for column, value in {268:'.1',61:'-1',201:'20',202:'2',203:'15',303:'10',304:'10',
                                  305:'60',310:'5',333:'.1',332:text,204:'.1',206:'.4',
                                  205:'0',207:'20',273:text}.items(): row[column] = value
        path = out/f'authored-{i:02d}.csv'
        path.write_text(','.join(row)+'\r\n', encoding='ascii', newline='')
        if args.geometry: ned_paths[path] = ned_paths[datasets[0][0]]
        datasets.append((path, row, False))
    if args.wheel_initialization:
        for i, text in enumerate(['-0','0','1e-46','1e-43','5.0000002','-5.0000002',
                                  '2.0000001','-2.0000001','1e300','-1e300']):
            row = datasets[0][1].copy()
            row[91] = row[92] = text
            row[126] = ['0','1e-46','1e-43','.3','1'][i%5]
            row[149] = ['0','1','2','-1'][i%4]
            path = out/f'wheel-authored-{i:02d}.csv'
            path.write_text(','.join(row)+'\r\n',encoding='ascii',newline='')
            ned_paths[path] = ned_paths[datasets[0][0]]
            datasets.append((path,row,False))
    seed_dir = ROOT/'local/behavior/assisted-steering-20261008-02'
    seed_report = json.loads((seed_dir/'validation.json').read_text())
    assert seed_report['cases'] == 2320 and not seed_report['differences']
    seed_input = seed_dir/'input.tsv'
    assert digest(seed_input) == seed_report['input_sha256']
    seeds = [line.split('\t')[1:] for line in seed_input.read_text().splitlines()]
    probe = ROOT/'local/behavior'/('source-wheel-steering-probe.exe' if args.wheel_initialization else 'assisted-geometry-probe.exe' if args.geometry else 'assisted-steering-probe.exe')
    assert digest(probe) == ('f803bf5475f090fafedd82d988ebb077d461deef1dae2a60624c510e1747ad89' if args.wheel_initialization else '7080f0243e9c16104ae38949f89419626f49e7c2820aadb02ed0cc750ce47f93' if args.geometry else 'e9203e0a327691888f8e8d45b3f6ee5b968039856ab99254a8189272884c1457')
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
        if args.all_parameters:
            columns = [268,61,201,202,203,111,132,131,303,304,305,310,333,332,204,206,205,207,273,165]
            scalars = []
            for i, column in enumerate(columns):
                value = float(row[column] or '0')
                factor = constant(0x3c8efa35) if i in [2,6,10,11] else constant(0x3c23d70a) if i in [7,8] else 1.
                converted = f32(value*factor)
                if i == 13: converted = float(value > .5)
                if i == 18 and value == 0.: converted = 1.
                if i == 19 and converted == 0.: converted = 1.
                scalars.append(converted)
            expected = [expected[0], *scalars]
            indexes = [4, *range(85,105)]
        original_rows, native_rows = [], []
        points = None
        if args.geometry:
            wheels = [e.attrib for e in decode(ned_paths[source].read_bytes())[1].iter() if e.tag == 'WHEELS']
            assert len(wheels) == 1
            points = layout(wheels[0], row)[3]
        wheel_scalars = None
        if args.wheel_initialization:
            cap = f32(float(row[126] or '0'))
            wheel_scalars = [f32(max(-5.,min(5.,float(row[91] or '0')))),
                             f32(max(-2.,min(2.,float(row[92] or '0')))),
                             1. if cap == 0. else cap]
        for i in range(48):
            words = seeds[(i*37+ordinal*7)%len(seeds)].copy()
            if args.all_parameters:
                raw_command = struct.unpack('<f', struct.pack('<I', int(words[0],16)))[0]
                words[0] = word(max(-1.,min(1.,raw_command)))
                words[2] = word([0.,2.,4.,10.,20.,34.999996,35.,35.000004,40.][i%9])
            # Distinct valid placeholders must be ignored by source mode.
            placeholders = [.333333, *[float.fromhex('0x1.abcdep-4')]*20] if args.all_parameters else [.333333, .123456, .012345, .765432, 17.6543]
            for index, value in zip(indexes, placeholders):
                words[index] = word(value)
            name = f'condition_{i}'
            native_rows.append(name+'\t'+'\t'.join(words)+'\n')
            for index, value in zip(indexes, expected): words[index] = word(value)
            if args.wheel_initialization:
                words[9] = word(float(int(row[149] or '0') == 1))
            if args.geometry: words += [word(v) for p in points for v in p]
            if args.wheel_initialization: words += [word(v) for v in wheel_scalars]
            original_rows.append(name+'\t'+'\t'.join(words)+'\n')
        original_input, native_input = out/f'{ordinal:03d}-original-input.tsv', out/f'{ordinal:03d}-native-input.tsv'
        original_input.write_text(''.join(original_rows)); native_input.write_text(''.join(native_rows))
        original_output, native_output = out/f'{ordinal:03d}-original.tsv', out/f'{ordinal:03d}-native.tsv'
        for kind, exe, argv in [
            ('original', probe, [ROOT/'local/compatibility/grid-unpacked.exe', original_input, original_output]),
            ('native', native, [native_input, native_output, '--source-wheel-geometry' if args.wheel_initialization else '--source-geometry' if args.geometry else '--source-parameters' if args.all_parameters else '--source', source, *([ned_paths[source]] if args.geometry else [])])]:
            run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            executions.append(dict(dataset=ordinal, kind=kind, sha256=digest(exe), exit=run.returncode, stdout=run.stdout, stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions, indent=2))
                raise RuntimeError('Source assisted execution failed')
        original_lines, native_lines = original_output.read_text().splitlines(), native_output.read_text().splitlines()
        assert len(original_lines) == len(native_lines) == 48
        for i, (original, actual) in enumerate(zip(original_lines, native_lines)):
            a, b = original.split('\t'), actual.split('\t')
            assert len(a) == len(b) == (35 if args.wheel_initialization else 27 if args.geometry else 22) and a[:2] == b[:2]
            differences.extend(dict(dataset=ordinal, case=i, field=j, original=x, native=y)
                               for j, (x,y) in enumerate(zip(a[2:], b[2:])) if x != y)
        reports.append(dict(source=str(source), source_sha256=digest(source), installed=installed,
                            ned_sha256=digest(ned_paths[source]) if args.geometry else None,
                            geometry_points=[[word(v) for v in p] for p in points] if args.geometry else None,
                            wheel_scalars=[word(v) for v in wheel_scalars] if args.wheel_initialization else None,
                            expected_words=[word(v) for v in expected],
                            premature_f32_words=[word(f32(float(row[c] or '0'))*constant(f)) for c,f in [(77,0x3c8efa35),(131,0x3c23d70a),(132,0x3c8efa35)]],
                            original_input_sha256=digest(original_input), native_input_sha256=digest(native_input)))
    guards = []
    guard_inputs = [('overflow',131,'1e300'), ('nonfinite',132,'NaN'),
                               ('negative_smoothing',165,'-1'), ('unsupported',2,'6 Wheeler'),
                               ('multiple_rows',None,None)]
    if args.wheel_initialization: guard_inputs += [('fractional_mode',149,'1.5'),('negative_wheel_cap',126,'-1')]
    for name, column, text in guard_inputs:
        row = datasets[0][1].copy()
        if column is not None: row[column] = text
        source = out/f'guard-{name}.csv'
        source.write_text(','.join(row)+'\n'+(','.join(row)+'\n' if column is None else ''), encoding='ascii')
        output = out/f'guard-{name}.tsv'
        run = subprocess.run([str(native),str(native_input),str(output),'--source-wheel-geometry' if args.wheel_initialization else '--source-geometry' if args.geometry else '--source-parameters' if args.all_parameters else '--source',str(source), *([str(ned_paths[datasets[0][0]])] if args.geometry else [])],
                             cwd=ROOT,capture_output=True,text=True,timeout=30)
        guards.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_created=output.exists()))
        assert run.returncode and not output.exists()
    result = dict(profile=args.profile, all_parameters=args.all_parameters, geometry=args.geometry, wheel_initialization=args.wheel_initialization, datasets=len(datasets), cases=len(datasets)*48, fields=len(datasets)*48*(33 if args.wheel_initialization else 25 if args.geometry else 20),
                  guards=guards,
                  differences=differences, datasets_report=reports, executions=executions,
                  limits=('Whole original9b6350/reset initializes source toe/limit before supplied mutable wheel state is installed, followed by geometry and assisted update. Mode source conversion independently supplied. ' if args.wheel_initialization else '')
                         + ('Independent source CSV/.ned conversion vs whole original98ebf0->9b3110, real input/feedback/math. '
                          'Original source decoders/outer constructors not executed. Original gets independently converted points/parameters; native derives them from sources. '
                          if args.geometry else 'Independent CSV conversion vs whole original assisted writer; original CSV decoder/constructor not executed. ')
                         + 'Other state/caps/mode/rotations/delta supplied; no real input/live cadence/full driving claim.')
    (out/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k:result[k] for k in ('profile','datasets','cases','fields')}))
    print('different_fields='+str(len(differences))); assert not differences

if __name__ == '__main__': main()
