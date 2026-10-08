"""HDM limits joined to visual wheel damage; independent original input conversion."""
import argparse
import json
import random
import re
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from validate_binxml import entries
from validate_steering_control import digest, word, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = 'c45b962f08dad43576157cf26a7ae7360dc3c7367ee109ce50bfb07372319724'
FIELDS = [('damageScaleFromVisual', 27.),
          ('damageScaleFromVisualDisplacement', struct.unpack('<f', bytes.fromhex('6666e63e'))[0]),
          ('terminalDamagePercentage', 150.)]


def read_rows(path):
    result = {}
    for line in path.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == 9 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    parser.add_argument('--snapshot', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    probe = ROOT / 'local/behavior/source-wheel-damage-probe.exe'
    native = ROOT / f'target/{args.profile}/grid-wheel-damage.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    baseline = json.loads((ROOT / 'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    records = [r for r in baseline['resources'] if Path(r['relative_path']).name == 'raceload.jpk']
    assert len(records) == 1
    archive = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid') / records[0]['relative_path']
    assert digest(archive) == records[0]['steam_sha256']
    datasets = []
    for name, payload, raw in entries(archive.read_bytes()):
        if name not in ['standard.hdm', 'openwheeler.hdm']:
            continue
        assert raw
        source = out / name
        source.write_bytes(payload)
        root = ET.fromstring(re.sub(rb'<![^>]*>', b'', payload))
        datasets.append((source, root, True))
    assert len(datasets) == 2
    controls = [{}, {'damageScaleFromVisual': '0'}, {'damageScaleFromVisual': '-0'},
                {'damageScaleFromVisual': '1e-46'}, {'damageScaleFromVisual': '-1e-46'},
                {'damageScaleFromVisual': '.100000004'}, {'damageScaleFromVisual': '25.687900000000001'},
                {'damageScaleFromVisual': '1.0000000000000001'},
                {'damageScaleFromVisualDisplacement': '.100000004'},
                {'damageScaleFromVisualDisplacement': '.49999999999999994'},
                {'damageScaleFromVisualDisplacement': '.5000000000000001'},
                {'damageScaleFromVisualDisplacement': '1e-30'},
                {'damageScaleFromVisual': '75', 'damageScaleFromVisualDisplacement': '2'},
                {'terminalDamagePercentage': '0'}, {'terminalDamagePercentage': '-0'},
                {'terminalDamagePercentage': '105.00000381469727'},
                dict(zip([f[0] for f in FIELDS], ['27', '.45', '150'])),
                dict(zip([f[0] for f in FIELDS], ['57.29577951308232', '.25', '50']))]
    for ordinal, attrs in enumerate(controls):
        root = ET.Element('VEHICLEHANDLINGDAMAGE')
        ET.SubElement(root, 'SUSPENSION')
        ET.SubElement(root, 'WHEEL', attrs)
        source = out / f'authored-{ordinal}.hdm'
        source.write_bytes(ET.tostring(root))
        datasets.append((source, root, False))
    rng = random.Random(132)
    cases = []
    for i in range(128):
        row = [rng.uniform(-.1, 1.1) for _ in range(4)]
        row += [float(i % 4), rng.uniform(-2., 2.)]
        row += [rng.uniform(-.5, .5) for _ in range(8)]
        row += [rng.uniform(-2., 2.), float(i % 2), 999., 998., rng.uniform(-1., 1.)]
        cases.append((f'authored_{i}', row))
    for wheel in range(4):
        for level in [-1., -0., 0., .25, .5, .75, 1., 2.]:
            row = [level] * 4 + [float(wheel), 0.] + [0.] * 8 + [0., 0., 999., 998., .016]
            cases.append((f'zero_{len(cases)}', row))
    executions, datasets_report, differences = [], [], []
    live_words = None
    if args.snapshot:
        path = args.snapshot.resolve()
        assert path.is_relative_to((ROOT / 'local').resolve()) and path.stat().st_size <= 8 * 1024 * 1024
        capture = json.loads(path.read_text(encoding='utf-8-sig'))
        assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
        assert capture['live_count_code_exact'] and len(capture['managers']) == 1
        assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
        blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
        live_words = []
        for car in capture['managers'][0]['cars']:
            mutable = blocks[f'car_{car["index"]}-mutable_steering']
            assert mutable[0x380:0x3a0].split(b'\0')[0].lower() == b'cars/standard.hdm'
            live_words.append([f'{v:08x}' for v in struct.unpack_from('<3I', mutable, 0x280)])
        assert len(live_words) == 12
        capture_report = dict(path=str(path), sha256=digest(path), utc=capture['utc'], pid=capture['pid'])
    else:
        capture_report = None
    for ordinal, (source, root, installed) in enumerate(datasets):
        wheel = root.find('WHEEL')
        parameters = [float(wheel.get(name, str(default))) for name, default in FIELDS]
        expected = [f'{word(v):08x}' for v in parameters]
        original_input, native_input = out / f'{ordinal}-original-input.tsv', out / f'{ordinal}-native-input.tsv'
        native_input.write_text(''.join(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n' for name, row in cases))
        original_rows = []
        for name, row in cases:
            original = row.copy()
            original[16:18] = parameters[:2]
            original.append(parameters[2])
            original_rows.append(name + '\t' + '\t'.join(f'{word(v):08x}' for v in original) + '\n')
        original_input.write_text(''.join(original_rows))
        for kind, exe, argv in [('original', probe, [image, original_input, out / f'{ordinal}-original.tsv']),
                               ('native', native, [native_input, out / f'{ordinal}-native.tsv', '--source', source])]:
            run = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            executions.append(dict(dataset=ordinal, kind=kind, sha256=digest(exe), exit=run.returncode,
                                   stdout=run.stdout, stderr=run.stderr))
            (out / 'executions.json').write_text(json.dumps(executions, indent=2))
            if run.returncode:
                raise RuntimeError('Source visual damage execution failed')
        original, actual = read_rows(out / f'{ordinal}-original.tsv'), read_rows(out / f'{ordinal}-native.tsv')
        assert original.keys() == actual.keys() == {name for name, _ in cases}
        assert all(v[:3] == expected for v in original.values())
        differences += [dict(dataset=ordinal, case=name, field=i, original=a, native=b) for name in original
                        for i, (a, b) in enumerate(zip(original[name], actual[name])) if a != b]
        if source.name == 'standard.hdm' and live_words is not None:
            assert all(values == expected == next(iter(actual.values()))[:3] for values in live_words)
        datasets_report.append(dict(path=str(source), sha256=digest(source), installed=installed,
                                    parameters=expected, cases=len(cases), input_sha256=digest(original_input)))
    guard_results = []
    for i, attrs in enumerate([{'damageScaleFromVisual': '-1'}, {'damageScaleFromVisualDisplacement': '0'},
                              {'terminalDamagePercentage': 'NaN'}, {'damageScaleFromVisual': '1e99'}]):
        source = out / f'guard-{i}.hdm'
        source.write_text('<VEHICLEHANDLINGDAMAGE><SUSPENSION/><WHEEL ' +
                          ' '.join(f'{k}="{v}"' for k, v in attrs.items()) + '/></VEHICLEHANDLINGDAMAGE>')
        output = out / f'guard-{i}-output.tsv'
        run = subprocess.run([str(native), str(native_input), str(output), '--source', str(source)],
                             cwd=ROOT, capture_output=True, text=True, timeout=30)
        guard_results.append(dict(exit=run.returncode, stderr=run.stderr, output_created=output.exists()))
        assert run.returncode and not output.exists()
    total = len(datasets) * len(cases)
    report = dict(profile=args.profile, datasets=datasets_report, cases=total, fields=total * 7,
                  differences=differences, captures=capture_report, live_parameter_fields=36 if live_words else 0,
                  guards=guard_results, executions=executions, analysis_sha256=PE_SHA,
                  limits='Source limits independently decoded for original9f2f80, native reads legacy HDM. '
                         'Deformation/correction/prior/delta supplied; status7/442=false. Original XML loader, '
                         'numeric converter, constructor, notifications, upstream damage and tick cadence unexecuted.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(profile=args.profile, datasets=len(datasets), cases=total, fields=total * 7,
                         differences=len(differences), live_parameter_fields=report['live_parameter_fields'])))
    assert not differences


if __name__ == '__main__':
    main()
