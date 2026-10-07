"""Interpret bounded read-only context snapshots; no synchronized tick claim."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def word(raw, offset):
    bits, = struct.unpack_from('<I', raw, offset)
    return {'word': f'{bits:08x}', 'value': struct.unpack_from('<f', raw, offset)[0]}


def load(path):
    if path.stat().st_size > 4*1024*1024:
        raise ValueError('Snapshot exceeds bounded inspector limit')
    data = json.loads(path.read_text(encoding='utf-8-sig'))
    assert data['read_only'] and not data['process_suspended'] and not data['input_sent']
    assert data['live_count_code_exact'] and len(data['managers']) == 1
    assert data['managers'][0]['count'] == 12
    blocks = {v['name']: v for v in data['blocks']}
    assert len(blocks)==len(data['blocks'])

    def read(name, length):
        b = blocks[name]
        assert b['success'] and b['read'] == b['length'] == length
        raw = bytes.fromhex(b['hex'])
        assert len(raw) == length
        return raw

    cars = []
    for car in data['managers'][0]['cars']:
        i = car['index']
        assert car['wheel_count'] == 4
        context = read(f'car_{i}-dynamics_context', 0x740)
        assert struct.unpack_from('<II', context, 0x5d4) == (int(car['body'], 16), 4)
        delta = read(f'car_{i}-delta_neighborhood', 0x30)
        # Body +0x3ac..+0x3bc = five values, +0x3c0 = next write index.
        ring = [word(delta, 0xc + j * 4) for j in range(5)]
        next_write, = struct.unpack_from('<I', delta, 0x20)
        assert next_write < 5 and all(0 < v['value'] <= .25 for v in ring)
        frame = read(f'car_{i}-frame_fields', 0x90)
        velocity = read(f'car_{i}-mass_inertia_fields', 0x30)
        cars.append({'index': i, 'kind': car['kind'], 'grid_index': car['grid_index'],
                     'scale': word(context, 0), 'context_flags': [context[0x5d0], context[0x70c]],
                     'clock': word(context, 0x610), 'delta_ring': ring, 'next_write': next_write,
                     'deformation_flag': read(f'car_{i}-deformation_flag', 1)[0],
                     'linear': [word(velocity, 0x20+j*4) for j in range(4)],
                     'angular': [word(frame, j*4) for j in range(4)],
                     'position': [word(frame, 0x40+j*4) for j in range(4)]})
    return data, blocks, cars


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--comparison', type=Path)
    args = parser.parse_args()
    for path in [args.snapshot, args.output] + ([args.comparison] if args.comparison else []):
        if not path.resolve().is_relative_to((ROOT/'local').resolve()):
            raise ValueError('Private evidence only')
    data, blocks, cars = load(args.snapshot)
    deltas = [v['value'] for c in cars for v in c['delta_ring']]
    result = {'snapshot': str(args.snapshot), 'sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              'utc': data['utc'], 'pid': data['pid'], 'created_utc': data['created_utc'], 'cars': cars,
              'summary': {'cars': len(cars), 'scale_one': sum(c['scale']['word']=='3f800000' for c in cars),
                          'delta_min': min(deltas), 'delta_max': max(deltas),
                          'delta_unique_words': sorted({v['word'] for c in cars for v in c['delta_ring']}),
                          'reciprocal_delta_range': [1/max(deltas),1/min(deltas)]},
              'limits': 'Read-only non-atomic snapshots while UI paused. Ring contains recently supplied '
                        'body-update deltas, not wall-clock frequency or a proven fixed simulation tick. '
                        'No breakpoint-boundary state, actual substep count or whole-solver validation.'}
    if args.comparison:
        second, other_blocks, other_cars = load(args.comparison)
        assert second['pid']==data['pid'] and second['created_utc']==data['created_utc']
        names = [f'car_{i}-{kind}' for i in range(12)
                 for kind in ['dynamics_context','delta_neighborhood','frame_fields','mass_inertia_fields']]
        result['comparison'] = {'path': str(args.comparison), 'utc': second['utc'],
            'sha256': hashlib.sha256(args.comparison.read_bytes()).hexdigest(),
            'selected_blocks':len(names), 'identical_blocks':sum(blocks[n]['hex']==other_blocks[n]['hex'] for n in names),
            'different_blocks':[n for n in names if blocks[n]['hex']!=other_blocks[n]['hex']],
            'cars':other_cars}
        first_player = next(c for c in cars if c['kind']==0)
        next_player = next(c for c in other_cars if c['kind']==0)
        position_a = [v['value'] for v in first_player['position'][:3]]
        position_b = [v['value'] for v in next_player['position'][:3]]
        result['comparison']['player_metrics'] = {
            'position_distance_source_units':math.dist(position_a,position_b),
            'after_linear_magnitude_source_units':math.sqrt(sum(v['value']**2 for v in next_player['linear'][:3])),
            'limits':'Source-coordinate/state units; no calibrated acceleration or fixed-duration simulation claim.'}
    with args.output.open('x',encoding='utf-8') as stream:
        json.dump(result,stream,indent=2,allow_nan=False)
    print(json.dumps(result['summary']))
    if 'comparison' in result:
        print(json.dumps({k:v for k,v in result['comparison'].items() if k not in ['cars']}))


if __name__=='__main__':
    main()
