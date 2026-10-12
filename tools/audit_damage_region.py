"""Read-only complete-output/current-hash/request rebuild audit for DM01 proof."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from validate_damage_region import inputs


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('report', type=Path)
    ap.add_argument('output', type=Path)
    args = ap.parse_args()
    private = Path('local').resolve()
    assert args.report.resolve().is_relative_to(private)
    assert args.output.resolve().is_relative_to(private)
    r = json.loads(args.report.read_text())
    assert r['passed'] and r['cases'] == 10847 and not r['runtime_damage']
    for path, digest in r['inputs'].items():
        assert sha(path) == digest, path
    items = list(inputs())
    request = b'DM01' + struct.pack('<I', len(items)) + b''.join(row for _, row in items)
    assert request == (args.report.parent / 'input.bin').read_bytes()
    assert hashlib.sha256(request).hexdigest() == r['request_sha256']
    files = {}
    for name, digest in r['outputs'].items():
        path = args.report.parent / f'{name}.bin'
        assert sha(path) == digest
        files[name] = path.read_bytes()
    assert len(files) == 4
    original = files['original']
    assert all(data == original for data in files.values())
    assert len(original) == r['observable_bytes_per_run'] == 10847 * 8
    draws = 0
    labels = {}
    for index, (label, row) in enumerate(items):
        labels[label] = labels.get(label, 0) + 1
        level, final = struct.unpack_from('<fI', original, index * 8)
        prior = struct.unpack_from('<f', row)[0]
        initial = struct.unpack_from('<I', row, 112)[0]
        following = (initial * 0x41c64e6d + 0x40584c0f) & 0xffffffff
        assert final in (initial, following) and 0 <= level <= 1.5
        if final == initial:
            assert original[index*8:index*8+4] == row[:4]
        else:
            draws += 1
            assert level <= 1
        if label == 'source-disabled-box':
            assert level == prior and final == initial
    assert labels == r['labels'] and draws == r['random_draws']
    for label in r['guards']:
        name, variant = label.split(':')
        if variant == 'existing':
            assert (args.report.parent / 'existing.bin').read_bytes() == b'KEEP'
        else:
            assert (args.report.parent / f'guard-{variant}.input').exists()
            assert not (args.report.parent / f'guard-{variant}-{name}.bin').exists()
    assert len(r['guards']) == 39
    result = dict(passed=True, report=str(args.report), report_sha256=sha(args.report),
                  complete_outputs=4, current_hashes=len(r['inputs']), cases=len(items),
                  bytes_per_run=len(original), random_draws=draws, guards=len(r['guards']),
                  source_boxes='disabled prototype startup; active boxes authored',
                  script_sha256=sha(__file__))
    with args.output.open('x') as f:
        json.dump(result, f, indent=2)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
