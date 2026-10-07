"""Validate private live vehicle snapshots and report explicitly observed fields."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path


def validate(path):
    raw = path.read_bytes()
    snapshot = json.loads(raw)
    assert snapshot['read_only'] and not snapshot['input_sent'] and not snapshot['process_suspended']
    assert snapshot['live_count_code_exact']
    blocks = {block['name']: block for block in snapshot['blocks']}
    assert len(blocks) == len(snapshot['blocks'])
    for block in blocks.values():
        assert block['success'] and block['read'] == block['length']
        assert len(bytes.fromhex(block['hex'])) == block['length']
    assert len(snapshot['managers']) == 1
    manager = snapshot['managers'][0]
    assert manager['count'] == len(manager['cars']) == 12
    assert len({car['vehicle'] for car in manager['cars']}) == 12
    assert len({car['body'] for car in manager['cars']}) == 12
    assert sorted(car['grid_index'] for car in manager['cars']) == list(range(12))
    prefix = bytes.fromhex(blocks['manager_candidate_' + manager['address']]['hex'])
    assert struct.unpack_from('<I', prefix)[0] == 0xd199cc
    result = []
    for car in manager['cars']:
        index = car['index']
        assert car['wheel_count'] == 4
        frame = bytes.fromhex(blocks[f'car_{index}-frame_fields']['hex'])
        wheels = bytes.fromhex(blocks[f'car_{index}-wheel_records']['hex'])
        dimensions = bytes.fromhex(blocks[f'car_{index}-dimension_fields']['hex'])

        def floats(data, offset, count):
            values = list(struct.unpack_from('<' + 'f' * count, data, offset))
            assert all(math.isfinite(value) for value in values)
            return values

        wheel_rows = []
        for wheel in range(4):
            start = wheel * 0x150
            wheel_rows.append({
                'index': wheel,
                'base_vector': floats(wheels, start, 4),
                'y_addend_at_40': floats(wheels, start + 0x40, 1)[0],
                'clearance_operand_at_e0': floats(wheels, start + 0xe0, 1)[0],
            })
        result.append({
            **car,
            'frame_vectors_at_15e0_15f0_1600': [floats(frame, offset, 4) for offset in (0x10, 0x20, 0x30)],
            'vector_at_1610': floats(frame, 0x40, 4),
            'offset_vector_at_1630': floats(frame, 0x60, 4),
            'dimension_fields_from_ed64': floats(dimensions, 0, 16),
            'wheels': wheel_rows,
        })
    kind_zero = [car['index'] for car in result if car['kind'] == 0]
    assert kind_zero == [0]  # Original 9ee380(0) selects this kind, rather than assuming vector order.
    return {
        'snapshot': str(path), 'sha256': hashlib.sha256(raw).hexdigest(),
        'utc': snapshot['utc'], 'manager': manager['address'],
        'cars': result, 'kind_zero_indices': kind_zero,
        'validated_blocks': len(blocks), 'wheel_records': 48,
    }, blocks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('first', type=Path)
    parser.add_argument('second', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / 'local'
    assert args.output.resolve().is_relative_to(root.resolve())
    first, first_blocks = validate(args.first)
    second, second_blocks = validate(args.second)
    assert first_blocks.keys() == second_blocks.keys()
    changes = [name for name in first_blocks if first_blocks[name]['hex'] != second_blocks[name]['hex']]
    report = {'first': first, 'second': second, 'changed_blocks': changes,
              'unchanged_blocks': len(first_blocks) - len(changes),
              'limits': 'Unsuspended selected-field snapshots; no simulation-rate or full-state determinism claim.'}
    with args.output.open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({'cars': 12, 'wheels': 48, 'unchanged_blocks': report['unchanged_blocks'],
                      'changed_blocks': changes, 'kind_zero_indices': second['kind_zero_indices']}))


if __name__ == '__main__':
    main()
