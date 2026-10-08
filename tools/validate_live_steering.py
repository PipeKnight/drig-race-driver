"""Replay bounded read-only mutable steering captures; all evidence stays private."""
import argparse
import json
import re
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from validate_binxml import entries
from validate_steering_control import digest, PE_SHA, PROBE_SHA
from validate_source_steering_reset import FIELDS

ROOT = Path(__file__).resolve().parents[1]
INPUT_OFFSETS = [0x18, 0x1c, 0x20, 0x24, 0x30, 0x34, 0x48, 0x4c,
                 0x50, 0x54, 0x58, 0x5c, 0x270, 0x274, 0x278, 0x27c, 0x254]
PARAMETER_OFFSETS = [0x26c, 0x250, 0x270, 0x274, 0x278, 0x27c, 0x254]
OUTPUT_OFFSETS = [0x3ac, 0x3b0, 0x3b4, 0x3d4, 0x3ec, 0x404, 0x41c]
FLAG_OFFSETS = [0x3dc, 0x3f4, 0x40c, 0x424]


def words(data, offsets):
    return [f'{struct.unpack_from("<I", data, offset)[0]:08x}' for offset in offsets]


def read_rows(path, count):
    result = {}
    for line in path.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == count + 2 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('snapshots', type=Path, nargs='+')
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and 1 <= len(args.snapshots) <= 32
    out.mkdir(exist_ok=False)
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    probe = ROOT / 'local/behavior/steering-control-probe.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    baseline = json.loads((ROOT / 'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    records = [r for r in baseline['resources'] if r['relative_path'].lower() == 'raceload.jpk']
    assert len(records) == 1
    archive = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid') / records[0]['relative_path']
    assert digest(archive) == records[0]['steam_sha256']
    sources = {}
    for name, payload, raw in entries(archive.read_bytes()):
        if name not in ('standard.hdm', 'openwheeler.hdm'):
            continue
        assert raw
        source = out / name
        source.write_bytes(payload)
        tree = ET.fromstring(re.sub(rb'<![^>]*>', b'', payload))
        parameters = []
        for node, key, default in FIELDS:
            selected = tree.findall(node)
            assert len(selected) == 1
            value = float(selected[0].get(key, str(default)))
            parameters.append(struct.pack('<f', value).hex())
        sources['cars/' + name] = dict(path=source, parameters=[f'{int.from_bytes(bytes.fromhex(x), "little"):08x}' for x in parameters])
    assert len(sources) == 2
    executions = []

    def run(kind, exe, argv):
        result = subprocess.run([str(exe), *map(str, argv)], cwd=ROOT,
                                capture_output=True, text=True, timeout=30)
        executions.append(dict(kind=kind, executable_sha256=digest(exe), exit=result.returncode,
                               stdout=result.stdout, stderr=result.stderr))
        (out / 'executions.json').write_text(json.dumps(executions, indent=2))
        if result.returncode:
            raise RuntimeError(f'{kind} execution failed')

    # Native source reading is tested separately; only its seven source fields
    # are compared here. The live pre-reset RNG state is not available.
    for source in sources.values():
        request = out / (source['path'].stem + '-source-input.tsv')
        request.write_text('source\t00000000\t' + '\t'.join(['00000000'] * 8) + '\n')
        target = out / (source['path'].stem + '-source-native.tsv')
        run('native_source', ROOT / f'target/{args.profile}/grid-steering-reset.exe',
            [request, target, '--source', source['path']])
        source['native_parameters'] = read_rows(target, 25)['source'][:7]
        assert source['native_parameters'] == source['parameters']

    inputs, observations, provenance, stability, previous = [], {}, [], [], None
    for ordinal, path in enumerate(args.snapshots):
        path = path.resolve()
        assert path.is_relative_to((ROOT / 'local').resolve()) and path.stat().st_size <= 8 * 1024 * 1024
        capture = json.loads(path.read_text(encoding='utf-8-sig'))
        assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
        assert capture['live_count_code_exact'] and len(capture['managers']) == 1
        manager = capture['managers'][0]
        assert manager['count'] == len(manager['cars']) == 12
        assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
        blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
        assert all(f'steering_code_{address:x}' in blocks for address in (0x9c5b90, 0x9bd7f0))
        selected = {}
        for car in manager['cars']:
            i = car['index']
            assert car['wheel_count'] == 4
            mutable = blocks[f'car_{i}-mutable_steering']
            copies = blocks[f'car_{i}-steering_copies']
            header = blocks[f'car_{i}-header']
            assert len(mutable) == 0x460 and len(copies) == 0x3c
            assert struct.unpack_from('<I', mutable, 0x214)[0] == int(car['vehicle'], 16)
            assert struct.unpack_from('<I', header, 0x24)[0] == int(car['body'], 16)
            name = mutable[0x380:0x3a0].split(b'\0')[0].decode('ascii')
            assert name.lower() in sources
            parameters = words(mutable, PARAMETER_OFFSETS)
            key = f'capture_{ordinal}_car_{i}'
            # Every captured vehicle has a nonnull wheel-view receiver.
            inputs.append(key + '\t' + '\t'.join(words(mutable, INPUT_OFFSETS) + ['3f800000']) + '\n')
            flags = [str(mutable[offset]) for offset in FLAG_OFFSETS]
            assert all(flag in ('0', '1') for flag in flags)
            stored = words(mutable, OUTPUT_OFFSETS) + flags
            copied = words(copies, [0, 4, 8, 12, 16, 20, 24]) + [str(x) for x in copies[0x38:0x3c]]
            observations[key] = dict(source=name, parameters=parameters,
                                     source_parameters=sources[name.lower()]['parameters'],
                                     stored=stored, copied=copied,
                                     level_words=words(mutable, INPUT_OFFSETS[:6]),
                                     seed_words=words(mutable, INPUT_OFFSETS[6:12]))
            selected[f'{i}-mutable'] = mutable.hex()
            selected[f'{i}-copies'] = copies.hex()
        if previous is not None:
            stability.append(dict(previous=ordinal - 1, current=ordinal,
                                  equal_blocks=sum(previous[k] == v for k, v in selected.items()),
                                  total_blocks=len(selected)))
        previous = selected
        provenance.append(dict(path=str(path), sha256=digest(path), utc=capture['utc'],
                               pid=capture['pid'], created_utc=capture['created_utc'],
                               manager=manager['address'], reads=len(capture['blocks'])))
    request = out / 'input.tsv'
    request.write_text(''.join(inputs))
    run('original', probe, [image, request, out / 'original.tsv'])
    run('native', ROOT / f'target/{args.profile}/grid-steering-control.exe', [request, out / 'native.tsv'])
    original, native = read_rows(out / 'original.tsv', 11), read_rows(out / 'native.tsv', 11)
    assert original.keys() == native.keys() == observations.keys()
    differences = []
    for key, observation in observations.items():
        for comparison, actual, expected in [
                ('original_vs_native', native[key], original[key]),
                ('producer_vs_live', native[key], observation['stored']),
                ('producer_vs_body_copy', observation['copied'], observation['stored']),
                ('source_vs_live', observation['parameters'], observation['source_parameters'])]:
            differences.extend(dict(case=key, comparison=comparison, field=i, actual=a, expected=b)
                               for i, (a, b) in enumerate(zip(actual, expected)) if a != b)
    report = dict(profile=args.profile, captures=provenance, cases=len(observations),
                  compared_fields=len(observations) * 40, differences=differences,
                  observations=observations, stability=stability, executions=executions,
                  input_sha256=digest(request), archive_sha256=digest(archive),
                  sources=[dict(name=s['path'].name, sha256=digest(s['path']),
                                parameters=s['parameters']) for s in sources.values()],
                  limits='Read-only non-atomic live snapshots, scoped producer and seven source fields. '
                         'Whole production copier and original XML reader not isolated/executed. '
                         'No observed pre-reset RNG, device transfer, tick cadence or full-driving equivalence.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(cases=report['cases'], compared_fields=report['compared_fields'],
                         different_fields=len(differences), stability=stability)))
    if differences:
        raise RuntimeError('Live steering differences retained in validation.json')


if __name__ == '__main__':
    main()
