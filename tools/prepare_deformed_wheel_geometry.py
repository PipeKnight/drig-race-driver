"""Run whole wheel geometry with retained grids and explicit policy provenance."""
import argparse
import json
import struct
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '1afaf44d6f11ee60b508c8d964082bd3702156934172f3741fd6da8c18b94526'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('--captured-policy', action='store_true', help='Require captured thresholds, tyre flags and incoming radius caches')
    args = parser.parse_args()
    out, path = args.output.resolve(), args.snapshot.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and path.is_relative_to((ROOT / 'local').resolve())
    assert path.stat().st_size <= 8 * 1024 * 1024
    out.mkdir(exist_ok=False)
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformed-wheel-geometry-probe.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    capture = json.loads(path.read_text(encoding='utf-8-sig'))
    assert capture['read_only'] and not capture['input_sent'] and not capture['process_suspended']
    assert capture['live_count_code_exact'] and len(capture['managers']) == 1
    assert all(b['success'] and b['read'] == b['length'] for b in capture['blocks'])
    blocks = {b['name']: bytes.fromhex(b['hex']) for b in capture['blocks']}
    assert len(blocks['steering_code_9bb1e0']) == len(blocks['steering_code_9a39d0']) == 32
    inputs, prior, metadata = [], {}, []
    def floats(data, offset, count):
        return list(struct.unpack_from('<' + 'f' * count, data, offset))
    for car in capture['managers'][0]['cars']:
        i = car['index']
        assert car['wheel_count'] == 4
        descriptor, nodes = blocks[f'car_{i}-deformation_grid_descriptor'], blocks[f'car_{i}-deformation_grid_nodes']
        dimensions = list(struct.unpack_from('<3i', descriptor))
        spacing, minimum = floats(descriptor, 12, 1), floats(descriptor, 28, 4)
        grid = [v for offset in range(0, len(nodes), 80) for v in floats(nodes, offset, 4)]
        assert len(grid) == 4 * dimensions[0] * dimensions[1] * dimensions[2] <= 1600
        mutable, frame = blocks[f'car_{i}-mutable_steering'], blocks[f'car_{i}-frame_fields']
        neighborhood, wheels = blocks[f'car_{i}-parameter_neighborhood'], blocks[f'car_{i}-wheel_records']
        center, scale = floats(frame, 0x60, 4), floats(blocks[f'car_{i}-visual_damage_scale'], 0, 1)
        angle_limit = floats(mutable, 0x280, 1)
        active = float(blocks[f'car_{i}-deformation_flag'][0])
        old = floats(neighborhood, 0x10, 16) + floats(neighborhood, 0x150, 4) + floats(neighborhood, 0x50, 64)
        retained_count = len(old)
        cache = blocks.get(f'car_{i}-wheel_radius_samples')
        old += floats(cache, 0, 40) if cache is not None else [0.] * 40
        if cache is not None:
            retained_count += 40
        assert len(old) == 124
        if args.captured_policy:
            assert cache is not None, 'Captured-policy replay requires incoming radius cache'
            thresholds = floats(blocks[f'car_{i}-deformation_radius_thresholds'], 0, 2)
            flags = list(map(float, blocks[f'car_{i}-deformation_tyre_flags']))
            assert len(flags) == 4 and all(v in (0., 1.) for v in flags)
            policies = [(thresholds, flags)]
        else:
            policies = [([threshold] * 2, [flag] * 4) for threshold, flag in [(1., 0.), (.01, 0.), (1., 1.), (.01, 1.)]]
        for variant, (thresholds, flags) in enumerate(policies):
            wheel_values = []
            for wheel in range(4):
                wheel_values += floats(wheels, wheel * 0x150, 4)
                wheel_values += floats(wheels, wheel * 0x150 + 0xe0, 1)
                wheel_values += floats(wheels, wheel * 0x150 + 0xf4, 2) + [flags[wheel]]
            row = list(map(float, dimensions)) + spacing + minimum + center + scale + angle_limit + thresholds + [active]
            row += wheel_values + old + grid
            assert len(row) == 173 + len(grid)
            name = f'car_{i}_captured_policy' if args.captured_policy else f'car_{i}_authored_policy_{variant}'
            inputs.append(name + '\t' + '\t'.join(f'{word(v):08x}' for v in row) + '\n')
            prior[name] = [f'{word(v):08x}' for v in old[:retained_count]]
            if args.captured_policy:
                prior[name] += [str(int(v)) for v in flags]
            metadata.append(dict(case=name, car=i, active=active, thresholds=thresholds, initial_tyre_flags=flags,
                                 dimensions=dimensions, incoming_cache='captured' if cache is not None else 'authored zeros',
                                 retained_numeric_fields=retained_count))
    request = out / 'input.tsv'
    request.write_text(''.join(inputs))
    target = out / 'original.tsv'
    run = subprocess.run([str(probe), str(image), str(request), str(target)], cwd=ROOT,
                         capture_output=True, text=True, timeout=30)
    report = dict(snapshot=str(path), snapshot_sha256=digest(path), input_sha256=digest(request), analysis_sha256=PE_SHA,
                  probe_sha256=PROBE_SHA, execution=dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr), cases=metadata,
                  captured_policy=args.captured_policy,
                  limits='Whole original9bb1e0 with actual retained descriptors/nodes/wheel positions and supplied radius thresholds/tyre flags. '
                         'RX code, four private mapped lazy-data pages writable/restored. No patches/stubs/injection. '
                         + ('Policy fields and incoming caches captured. ' if args.captured_policy else
                            'Policies authored; missing incoming caches use authored zeros. ')
                         +
                         'No synchronized update or native comparison claim.')
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    if run.returncode:
        raise RuntimeError('Whole original wheel geometry failed')
    result = {}
    for line in target.read_text().splitlines():
        cells = line.split('\t')
        assert len(cells) == 130 and cells[0] == 'P' and cells[1] not in result
        result[cells[1]] = cells[2:]
    assert result.keys() == prior.keys()
    # Captured flags are compared; explicitly authored incoming flags excluded.
    differences = [dict(case=name, field=i, prior=a, replay=b) for name in result
                   for i, (a, b) in enumerate(zip(prior[name], result[name])) if a != b]
    retained_fields = sum(map(len, prior.values()))
    report.update(output_sha256=digest(target), retained_fields=retained_fields,
                  retained_differences=differences, replay_cases=len(result))
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(cases=len(result), retained_fields=retained_fields, different_retained_fields=len(differences), exit=run.returncode)))


if __name__ == '__main__':
    main()
