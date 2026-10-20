"""Original close-drift flag post geometry (0x7BBB70, 0x7BC370, 0x7BCDA0)
against the Rust port, FPP1.

Random posts (pivot, direction, length) and car-sized oriented boxes around
them; the probe runs the original functions in-process
(tools/probe_phantom_post.c), the Rust diagnostic reads the same file
(crates/grid-probes grid-phantom-post), and the outputs must match byte for
byte.

    python -I tools/validate_phantom_post.py [--cases N] [--profile release]
"""
import argparse
import hashlib
import json
import math
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31505046
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
if not IMAGE.exists():
    IMAGE = Path('C:/Dev/Reassembly/local/compatibility/grid-unpacked.exe')
CASE = 4 + 0x80 + 0x40 + 0x10 * 3
RESULT = 4 + 0x10 * 2 + 0x80


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def unit(v):
    n = math.sqrt(sum(x * x for x in v))
    return [x / n for x in v]


def cross(a, b):
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]


def rotation(rng):
    # A car-like yaw with some pitch and roll, or an arbitrary rotation.
    if rng.random() < 0.7:
        yaw = rng.uniform(-math.pi, math.pi)
        pitch, roll = rng.gauss(0, 0.05), rng.gauss(0, 0.05)
    else:
        yaw, pitch, roll = (rng.uniform(-math.pi, math.pi) for _ in range(3))
    cy, sy, cp, sp, cr, sr = (math.cos(yaw), math.sin(yaw), math.cos(pitch),
                              math.sin(pitch), math.cos(roll), math.sin(roll))
    x = [cy * cr + sy * sp * sr, cp * sr, -sy * cr + cy * sp * sr]
    y = [-cy * sr + sy * sp * cr, cp * cr, sy * sr + cy * sp * cr]
    z = cross(x, y)
    return [x, y, z]


def case(rng, mode):
    pivot = [rng.uniform(-50, 50), rng.uniform(0, 20), rng.uniform(-50, 50)]
    if rng.random() < 0.6:
        direction = unit([rng.gauss(0, 0.2), 1., rng.gauss(0, 0.2)])
    else:
        direction = unit([rng.gauss(0, 1) for _ in range(3)])
    length = rng.choice([1.465, 1.49, rng.uniform(0.2, 3.)])
    centre = [pivot[k] + length * direction[k] for k in range(3)]
    tip = [pivot[k] + 2 * length * direction[k] for k in range(3)]
    low = [min(pivot[k], tip[k]) for k in range(3)]
    high = [max(pivot[k], tip[k]) for k in range(3)]
    post = [0.] * 32
    post[4:8] = low + [0.]
    post[8:12] = high + [0.]
    post[12:16] = pivot + [0.]
    post[16:20] = tip + [0.]
    post[20:24] = centre + [0.]
    post[24:28] = direction + [0.]
    post[28] = length
    axes = rotation(rng)
    half = [rng.uniform(0.6, 1.1), rng.uniform(0.4, 0.8), rng.uniform(1.6, 2.6)]
    # Box centre near the pole: a point along it plus an offset that often overlaps.
    t = rng.uniform(-0.2, 2.2) * length
    spread = rng.choice([0.5, 1.5, 3.])
    near = [pivot[k] + t * direction[k] + rng.gauss(0, spread) for k in range(3)]
    box = axes[0] + [0.] + axes[1] + [0.] + axes[2] + [0.] + near + [1.]
    a = [rng.uniform(-3, 3) + near[k] for k in range(3)] + [0.]
    b = [rng.uniform(-3, 3) + near[k] for k in range(3)] + [0.]
    words = [f32(x) for x in post + box + half + [0.] + a + b]
    return struct.pack('<I', mode) + struct.pack('<60f', *words)


def run(command, source, target):
    if target.exists():
        target.unlink()
    r = subprocess.run(command + [str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        raise SystemExit(f'{command[0]} failed {r.returncode}: {r.stdout} {r.stderr}')
    return r.stdout.strip(), target.read_bytes()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cases', type=int, default=6144)
    parser.add_argument('--seed', type=int, default=20261020)
    parser.add_argument('--profile', default='release')
    parser.add_argument('--original-only', action='store_true')
    args = parser.parse_args()
    rng = random.Random(args.seed)
    out = ROOT / 'local/behavior/phantom-post'
    out.mkdir(parents=True, exist_ok=True)
    body = b''.join(case(rng, i % 3) for i in range(args.cases))
    source = out / 'cases.bin'
    source.write_bytes(struct.pack('<II', MAGIC, args.cases) + body)
    probe = ROOT / 'local/behavior/phantom-post-probe.exe'
    original, expected = run([str(probe), str(IMAGE)], source, out / 'original.bin')
    print(original)
    modes = [0, 0, 0]
    hits = [0, 0, 0]
    for i in range(args.cases):
        r = expected[8 + i * RESULT: 8 + (i + 1) * RESULT]
        modes[i % 3] += 1
        if i % 3 == 0:
            hits[0] += struct.unpack_from('<I', r)[0]
        elif i % 3 == 2:
            hits[2] += r[4 + 32 + 0x74] != 0
    print('cases by mode', modes, 'test hits', hits[0], 'pushes', hits[2])
    if args.original_only:
        return
    rust = ROOT / f'target/{args.profile}/grid-phantom-post.exe'
    port, actual = run([str(rust)], source, out / 'port.bin')
    print(port)
    mismatched = [0, 0, 0]
    first = None
    for i in range(args.cases):
        a = expected[8 + i * RESULT: 8 + (i + 1) * RESULT]
        b = actual[8 + i * RESULT: 8 + (i + 1) * RESULT]
        if a != b:
            mismatched[i % 3] += 1
            if first is None:
                first = i
    report = {
        'cases': args.cases, 'seed': args.seed, 'mismatched_by_mode': mismatched,
        'first_mismatch': first, 'test_hits': hits[0], 'pushes': hits[2],
        'cases_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
        'original_sha256': hashlib.sha256(expected).hexdigest(),
    }
    (out / 'validation.json').write_text(json.dumps(report, indent=1))
    print(json.dumps(report))
    if first is not None:
        a = expected[8 + first * RESULT: 8 + (first + 1) * RESULT]
        b = actual[8 + first * RESULT: 8 + (first + 1) * RESULT]
        for k in range(0, RESULT, 4):
            if a[k:k + 4] != b[k:k + 4]:
                print(f'case {first} mode {first % 3} word {k:#x}: original {a[k:k + 4].hex()} '
                      f'{struct.unpack("<f", a[k:k + 4])[0]!r} port {b[k:k + 4].hex()} '
                      f'{struct.unpack("<f", b[k:k + 4])[0]!r}')
        raise SystemExit(1)


if __name__ == '__main__':
    main()
