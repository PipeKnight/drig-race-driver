"""Replay isolated whole impact kernel with retained pools and authored records."""
import argparse
import json
import struct
import subprocess
from pathlib import Path
from summarize_deformation_capture import load
from validate_steering_control import digest, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '1df4653c24e03fe0df9cc8e72d3adf785d552b0e7ab70fe5d7f04cb4a4ca095f'
DESCRIPTOR, SETTINGS = 0xa500, 0x1050


def record(index, direction, anchor, weight):
    value = bytearray(48)
    struct.pack_into('<8f', value, 0, *direction, *anchor)
    struct.pack_into('<h', value, 0x20, index)
    struct.pack_into('<f', value, 0x24, weight)
    return bytes(value)


def retained(blocks, index):
    prefix = f'car_{index}-'
    d, s = bytearray(DESCRIPTOR), bytearray(SETTINGS)
    for offset, name in ((0, 'deformation_edges'), (0xa410, 'deformation_edge_count'),
                         (0xa414, 'deformation_grid_descriptor'), (0xa280, 'deformation_node_flags'),
                         (0xa440, 'deformation_impact_state'), (0xa4a0, 'deformation_grid_state')):
        value = blocks[prefix + name]
        d[offset:offset + len(value)] = value
    pool = blocks[prefix + 'deformation_grid_nodes'] + blocks[prefix + 'deformation_grid_padding_nodes']
    assert len(pool) == 400 * 80
    d[0x2580:0x2580 + len(pool)] = pool
    s[0x1024:0x102c] = blocks[prefix + 'deformation_strain_settings']
    s[0x102c:0x1050] = blocks[prefix + 'deformation_impact_settings']
    # Replace the captured process address inside the isolated process only.
    struct.pack_into('<I', d, 0xa4f8, 0)
    return bytes(d), bytes(s)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('snapshot', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    capture, blocks, rows = load(args.snapshot)
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformation-impact-probe.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    globals_ = blocks['deformation_solver_globals'][:24]
    assert len(globals_) == 24
    clip = blocks.get('deformation_solver_clip_flag', b'\x01')[0]
    assert clip in (0, 1)
    # The first six words match the PE defaults. The two following captured
    # words are process pointers and are deliberately excluded.
    cases = []
    for row in rows:
        index = row['car']
        d, s = retained(blocks, index)
        active = next(i for i in range(row['sampled_nodes']) if struct.unpack_from('<f', d, 0x2580 + i * 80 + 0x40)[0] != 0.)
        position = struct.unpack_from('<4f', d, 0x2580 + active * 80)
        for label, records in (
            ('empty', b''),
            ('below_gate', record(active, (1., 0., 0., 0.), position, 0.0001)),
            ('x', record(active, (1., 0., 0., 0.), position, 0.1)),
            ('z', record(active, (0., 0., 1., 0.), position, 0.1)),
        ):
            cases.append(dict(name=f'car_{index}_{label}', descriptor=d, settings=s,
                              records=records, gain=1., globals=globals_, clip=clip,
                              kind='retained pool with authored record', node=active))
    out.mkdir(exist_ok=False)
    request = bytearray(struct.pack('<II', 0x314d4947, len(cases)))
    for case in cases:
        request += struct.pack('<If', len(case['records']) // 48, case['gain'])
        request += case['descriptor'] + case['settings'] + case['globals']
        request += struct.pack('<I', case['clip']) + case['records']
    source, target = out / 'input.bin', out / 'original.bin'
    source.write_bytes(request)
    run = subprocess.run([str(probe), str(image), str(source), str(target)], cwd=ROOT,
                         capture_output=True, text=True, timeout=60)
    report = dict(snapshot_sha256=digest(args.snapshot), analysis_sha256=PE_SHA,
                  probe_sha256=PROBE_SHA, input_sha256=digest(source), cases=len(cases),
                  execution=dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr),
                  clip_flag=clip, clip_flag_source='captured' if 'deformation_solver_clip_flag' in blocks else 'analysis PE default',
                  limits='Retained pools/settings with authored impacts; original whole9D4970 and9A26F0, three real clock imports; no game writes, captured impact provenance or same-tick claim.')
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    assert run.returncode == 0, report['execution']
    data = target.read_bytes()
    assert struct.unpack_from('<II', data) == (0x314d4947, len(cases)) and len(data) == 8 + DESCRIPTOR * len(cases)
    details = []
    for i, case in enumerate(cases):
        result = data[8 + i * DESCRIPTOR:8 + (i + 1) * DESCRIPTOR]
        before = case['descriptor']
        changes = [offset for offset, (a, b) in enumerate(zip(before, result)) if a != b]
        details.append(dict(name=case['name'], changed_bytes=len(changes),
                            changed_words=sum(before[j:j + 4] != result[j:j + 4] for j in range(0, DESCRIPTOR, 4)),
                            changed_offsets=changes, counter=struct.unpack_from('<i', result, 0xa450)[0],
                            dirty_a4f0=result[0xa4f0], authored_record_hex=case['records'].hex(), node=case['node']))
        if case['name'].endswith('_empty'):
            assert result == before, case['name']
        if case['name'].endswith('_below_gate'):
            # Counter reset occurs before the scalar gate; no numerical write.
            expected = bytearray(before)
            expected[0xa450:0xa454] = globals_[12:16]
            assert result == expected, case['name']
    report.update(output_sha256=digest(target), details=details)
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(cases=len(cases), exit=run.returncode,
                          changed_bytes={r['name']: r['changed_bytes'] for r in details})))


if __name__ == '__main__':
    main()
