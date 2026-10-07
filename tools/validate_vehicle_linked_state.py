"""Compare two captured original vehicle-state views without assuming equality."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if not args.output.resolve().is_relative_to((root / 'local').resolve()):
        raise ValueError('Evidence must remain under local/')
    raw = args.snapshot.read_bytes()
    snapshot = json.loads(raw.decode('utf-8-sig'))
    assert snapshot['read_only'] and not snapshot['process_suspended'] and not snapshot['input_sent']
    assert len(snapshot['managers']) == 1
    blocks = {block['name']: block for block in snapshot['blocks']}
    assert len(blocks) == len(snapshot['blocks'])

    def data(name):
        block = blocks[name]
        result = bytes.fromhex(block['hex'])
        assert block['success'] and block['read'] == block['length'] == len(result)
        return result, int(block['address'], 16)

    def floats(raw):
        values = struct.unpack('<' + 'f' * (len(raw) // 4), raw)
        assert all(math.isfinite(value) for value in values)
        return values

    cars = []
    for car in snapshot['managers'][0]['cars']:
        index = car['index']
        link, address = data(f'car_{index}-state_wrapper_link')
        wrapper, wrapper_address = data(f'car_{index}-state_wrapper_header')
        state, state_address = data(f'car_{index}-linked_state_fields')
        assert address == int(car['body'], 16) + 0xdb0
        assert struct.unpack('<I', link)[0] == wrapper_address
        assert struct.unpack_from('<I', wrapper, 8)[0] + 0x190 == state_address
        frame, _ = data(f'car_{index}-frame_fields')
        mass, _ = data(f'car_{index}-mass_inertia_fields')
        comparisons = {
            'pose_at_15e0_vs_linked_1d0': (frame[0x10:0x50], state[0x40:0x80]),
            'linear_at_15c0_vs_linked_220': (mass[0x20:0x30], state[0x90:0xa0]),
            'angular_at_15d0_vs_linked_210': (frame[:0x10], state[0x80:0x90]),
        }
        rows = {}
        for name, (body, linked) in comparisons.items():
            body_values, linked_values = floats(body), floats(linked)
            rows[name] = {
                'bits_equal': body == linked, 'body': body_values, 'linked': linked_values,
                'max_absolute_difference': max(abs(a - b) for a, b in zip(body_values, linked_values)),
            }
        cars.append({'index': index, 'kind': car['kind'], 'comparisons': rows})
    report = {
        'snapshot': str(args.snapshot), 'sha256': hashlib.sha256(raw).hexdigest(),
        'cars': cars,
        'exact_pose_cars': sum(row['comparisons']['pose_at_15e0_vs_linked_1d0']['bits_equal'] for row in cars),
        'exact_linear_cars': sum(row['comparisons']['linear_at_15c0_vs_linked_220']['bits_equal'] for row in cars),
        'exact_angular_cars': sum(row['comparisons']['angular_at_15d0_vs_linked_210']['bits_equal'] for row in cars),
        'limits': 'Unsuspended sequential reads; differences retained, no atomic state or tick timing claim.',
    }
    with args.output.open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({name: report[name] for name in ('exact_pose_cars', 'exact_linear_cars', 'exact_angular_cars')}))


if __name__ == '__main__':
    main()
