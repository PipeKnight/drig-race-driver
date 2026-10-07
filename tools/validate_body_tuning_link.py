"""Compare observed body mass/inertia words to original positional CSV cells."""
import argparse
import csv
import hashlib
import json
import struct
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('game_root', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / 'local'
    assert args.output.resolve().is_relative_to(root.resolve())
    candidates = []
    for path in sorted((args.game_root / 'cars').rglob('*.csv')):
        raw = path.read_bytes()
        for ordinal, row in enumerate(csv.reader(raw.decode('utf-8').splitlines())):
            if len(row) < 7:
                continue
            try:
                values = [float(value) for value in row[3:7]]
                words = [struct.pack('<f', value).hex() for value in values]
            except (ValueError, OverflowError):
                continue
            candidates.append({'source': str(path.relative_to(args.game_root)),
                               'row': ordinal, 'sha256': hashlib.sha256(raw).hexdigest(),
                               'values': values, 'words': words, 'cells': row})
    snapshot = json.loads(args.snapshot.read_text())
    blocks = {block['name']: bytes.fromhex(block['hex']) for block in snapshot['blocks']}
    comparisons = []
    for car in snapshot['managers'][0]['cars']:
        block = blocks[f"car_{car['index']}-mass_inertia_fields"]
        actual = [block[offset:offset + 4].hex() for offset in (0, 8, 12, 16)]
        mass, inverse = struct.unpack_from('<ff', block)
        assert mass > 0
        expected_inverse = struct.pack('<f', 1.0 / mass)
        assert expected_inverse == block[4:8]
        matches = [candidate for candidate in candidates if candidate['words'] == actual]
        assert matches, f"No source tuple for car {car['index']}"
        # Direct copies only: wheel offsets -> (first-two column, last-two column).
        mapping = {0xe0: (146, 145), 0x108: (84, 83), 0x100: (86, 85),
                   0x50: (88, 87), 0x48: (90, 89), 0x5c: (96, 95),
                   0x60: (94, 93), 0x70: (80, 79)}
        wheel_block = blocks[f"car_{car['index']}-wheel_records"]
        wheel_matches = []
        for candidate in matches:
            exact = True
            for wheel in range(4):
                start = wheel * 0x150
                assert wheel_block[start + 0xfc] == (1 if wheel < 2 else 0)
                for offset, columns in mapping.items():
                    expected = struct.pack('<f', float(candidate['cells'][columns[0 if wheel < 2 else 1]]))
                    exact &= expected == wheel_block[start + offset:start + offset + 4]
            if exact:
                wheel_matches.append({key: value for key, value in candidate.items() if key != 'cells'})
        assert wheel_matches, f"No wheel-copy source for car {car['index']}"
        comparisons.append({'index': car['index'], 'kind': car['kind'],
                            'body': car['body'], 'words': actual, 'mass': mass,
                            'inverse_mass': inverse, 'inverse_word_exact': True,
                            'source_candidates': [{key: value for key, value in candidate.items() if key != 'cells'} for candidate in matches],
                            'wheel_source_candidates': wheel_matches,
                            'wheel_copy_fields_exact': len(mapping) * 4})
    assert len(comparisons) == 12
    report = {'snapshot': str(args.snapshot),
              'snapshot_sha256': hashlib.sha256(args.snapshot.read_bytes()).hexdigest(),
              'cars': comparisons, 'source_columns': [3, 4, 5, 6],
              'body_offsets': ['15a0', '15a8', '15ac', '15b0'],
              'wheel_mapping': {hex(offset): list(columns) for offset, columns in mapping.items()},
              'wheel_copy_words_exact': sum(car['wheel_copy_fields_exact'] for car in comparisons),
              'limits': 'Tuple agreement and original copy chain; no whole force-model or unit-fidelity claim.'}
    with args.output.open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({'cars': 12, 'tuples_exact': True, 'reciprocals_exact': True,
                      'sources_by_car': [[candidate['source'] for candidate in car['source_candidates']]
                                         for car in comparisons],
                      'wheel_copy_words_exact': report['wheel_copy_words_exact'],
                      'wheel_sources_by_car': [[candidate['source'] for candidate in car['wheel_source_candidates']]
                                               for car in comparisons]}))


if __name__ == '__main__':
    main()
