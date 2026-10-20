"""Compare owned lamp-fade inputs against unchanged, hash-guarded GRID code.

Build tools/build_lamp_fades_probe.cmd first. Outputs must remain private.
No rendering, live-uniform or light-part damage equivalence is claimed.
"""
import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SOURCE_SHA = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def expected(row):
    dt = f32(abs(row[0]) * 20.)
    values = []
    for i, on in enumerate((row[7] != 0., row[8] > 0., row[9] == 9.)):
        value = f32(row[i + 1] + (dt if on else -dt))
        values.append(0. if value < 0. else row[i + 4] if value > row[i + 4] else value)
    return struct.pack('<4f', *values, min(2., f32(values[0] + values[1])))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--source', type=Path, required=True)
    ap.add_argument('--probe', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    out = args.out.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence must remain under this checkout local/')
    if hashlib.sha256(args.source.read_bytes()).hexdigest() != SOURCE_SHA:
        raise ValueError('Reference image identity mismatch')
    out.mkdir(parents=True, exist_ok=False)
    rows = []
    for dt, start, caps, head, brake, gear in itertools.product(
        (0., 1/300, 1/60, 1/30, .1, -.016),
        ((0., 0., 0.), (.4, .6, .8), (1., 1., 1.), (.99, .01, .25)),
        ((1., 1., 1.), (.6, 1.5, .8), (.7, .7, 1.)), (0., 1.), (0., 1.), (1., 9.)
    ):
        rows.append(tuple(map(f32, (dt, *start, *caps, head, brake, gear))))
    request = struct.pack('<II', 0x314c5047, len(rows)) + b''.join(struct.pack('<10f', *row) for row in rows)
    (out / 'request.bin').write_bytes(request)
    runs = []
    for name in ('original', 'repeat'):
        target = out / (name + '.bin')
        cmd = [str(args.probe.resolve()), str(args.source.resolve()), str(out / 'request.bin'), str(target)]
        q = subprocess.run(cmd, capture_output=True, timeout=30)
        runs.append(dict(command=cmd, exit=q.returncode, stdout=q.stdout.decode(errors='replace'), stderr=q.stderr.decode(errors='replace')))
        (out / 'execution.json').write_text(json.dumps(runs, indent=2))
        if q.returncode or b'image_unchanged=1 code_replacements=0' not in q.stdout:
            raise RuntimeError('Original probe failed; see execution.json')
    actual = (out / 'original.bin').read_bytes()
    reference = b''.join(expected(row) for row in rows)
    assert len(actual) == len(rows) * 16
    assert actual == (out / 'repeat.bin').read_bytes(), 'Original repeat differs'
    different = sum(a != b for a, b in zip(struct.iter_unpack('<I', actual), struct.iter_unpack('<I', reference)))
    report = dict(cases=len(rows), words=len(rows)*4, different_words=different, repeat_equal=True,
                  source_sha256=SOURCE_SHA, probe_sha256=hashlib.sha256(args.probe.read_bytes()).hexdigest(),
                  scope='Authored single updates with empty draw graph; not live uniforms, damage gating, section-state or rendering validation')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    assert different == 0, report
    invalid = [('truncated', request[:-1]), ('zero-count', struct.pack('<II', 0x314c5047, 0)),
               ('nan', struct.pack('<II10f', 0x314c5047, 1, math.nan, *rows[0][1:])),
               ('fractional-head', struct.pack('<II10f', 0x314c5047, 1, *rows[0][:7], .5, *rows[0][8:])),
               ('invalid-head', struct.pack('<II10f', 0x314c5047, 1, *rows[0][:7], 2., *rows[0][8:]))]
    failures = []
    for name, data in invalid:
        src = out / (name + '.request');src.write_bytes(data)
        dst = out / (name + '.bin')
        q = subprocess.run([str(args.probe.resolve()), str(args.source.resolve()), str(src), str(dst)], capture_output=True, timeout=30)
        failures.append(dict(name=name, exit=q.returncode, output_exists=dst.exists()))
        (out / 'negative.json').write_text(json.dumps(failures, indent=2))
        assert q.returncode != 0 and not dst.exists(), failures[-1]
    print(json.dumps(report))


if __name__ == '__main__':
    main()
