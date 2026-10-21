"""Original constraint template stage (0x7C7C00, 0x7C7B10/0x7C04D0) against
the Rust port, JTP1.

Random props and template transforms; the probe runs the original functions
in-process (tools/probe_joint_template.c), the Rust diagnostic reads the same
file (crates/grid-probes grid-joint-template), and the outputs must match
byte for byte.

    python -I tools/validate_joint_template.py [--cases N] [--profile release]
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
MAGIC = 0x3150544A
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
if not IMAGE.exists():
    IMAGE = Path('C:/Dev/Reassembly/local/compatibility/grid-unpacked.exe')
ANALYSIS = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
FLT_MAX = 3.4028234663852886e38


def quaternion(rng):
    kind = rng.random()
    if kind < 0.3:
        return [0., 0., 0., 1.]
    q = [rng.gauss(0, 1) for _ in range(4)]
    if kind < 0.9:
        n = math.sqrt(sum(x * x for x in q))
        q = [x / n for x in q]
    return q


def value(rng, scale=10.):
    return rng.choice([0., -0., 1., rng.uniform(-scale, scale), rng.uniform(-1e5, 1e5)])


def props(rng):
    p = bytearray(0x110)
    for o in range(0x30, 0x70, 0x10):
        struct.pack_into('<4f', p, o, value(rng), value(rng), value(rng), 0.)
    struct.pack_into('<6f', p, 0x70, *(value(rng) for _ in range(6)))
    struct.pack_into('<I', p, 0x88, rng.choice([0, 1, 2, 3]))
    struct.pack_into('<5f', p, 0x8c, *(value(rng, 2.) for _ in range(5)))
    struct.pack_into('<I', p, 0xa0, rng.choice([0, 1, 5]))
    struct.pack_into('<4f', p, 0xa4, *(value(rng, 2.) for _ in range(4)))
    for o in (0xb4, 0xb8):
        struct.pack_into('<f', p, o, rng.choice([FLT_MAX, 10., rng.uniform(0, 5000)]))
    struct.pack_into('<3f', p, 0xbc, *(value(rng) for _ in range(3)))
    struct.pack_into('<I', p, 0xc8, rng.choice([0, 0, 1]))
    for o in (0xcc, 0xdc, 0xec):
        struct.pack_into('<4f', p, o, *quaternion(rng))
    return bytes(p)


def template(rng):
    t = bytearray(rng.randbytes(0x150))
    q = quaternion(rng)
    rows = [[rng.uniform(-1, 1) for _ in range(3)] + [0.] for _ in range(3)]
    if rng.random() < 0.5:
        # A unit frame from a quaternion, as the reader leaves it.
        x, y, z, w = (q + [0.])[:4] if len(q) == 4 else q
        n = math.sqrt(x * x + y * y + z * z + w * w) or 1.
        x, y, z, w = x / n, y / n, z / n, w / n
        rows = [
            [1 - 2 * (y * y + z * z), 2 * (x * y + w * z), 2 * (x * z - w * y), 0.],
            [2 * (x * y - w * z), 1 - 2 * (x * x + z * z), 2 * (y * z + w * x), 0.],
            [2 * (x * z + w * y), 2 * (y * z - w * x), 1 - 2 * (x * x + y * y), 0.],
        ]
    position = [rng.uniform(-5, 5) for _ in range(3)] + [1.]
    struct.pack_into('<16f', t, 0x100, *(v for r in rows + [position] for v in r))
    t[0x140] = rng.choice([0, 1])
    return bytes(t)


def case(rng):
    scale = rng.choice([1., 1., 0.9718, 1.46, rng.uniform(0.5, 2.)])
    return props(rng) + template(rng) + struct.pack('<fI', scale, 0)


def run(command, source, target):
    target.unlink(missing_ok=True)
    r = subprocess.run(command + [str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=600)
    return r, target.read_bytes() if target.exists() else b''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cases', type=int, default=4096)
    parser.add_argument('--profile', default='release')
    parser.add_argument('--out', default='local/joint')
    args = parser.parse_args()
    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    if hashlib.sha256(IMAGE.read_bytes()).hexdigest() != ANALYSIS:
        raise SystemExit('analysis image hash mismatch')
    rng = random.Random(20081019)
    cases = [case(rng) for _ in range(args.cases)]
    source = out / 'template-cases.bin'
    source.write_bytes(struct.pack('<II', MAGIC, len(cases)) + b''.join(cases))
    probe = ROOT / 'local/behavior/joint-template-probe.exe'
    rust = ROOT / f'target/{args.profile}/grid-joint-template.exe'
    original, expected = run([str(probe), str(IMAGE)], source, out / 'template-original.bin')
    print(original.stdout.strip(), original.stderr.strip())
    port, actual = run([str(rust)], source, out / 'template-rust.bin')
    print(port.stdout.strip(), port.stderr.strip())
    record = 0xc0 + 0x174
    mismatched = []
    for k in range(len(cases)):
        a = expected[8 + k * record:8 + (k + 1) * record]
        b = actual[8 + k * record:8 + (k + 1) * record]
        words = [i * 4 for i in range(record // 4) if a[i * 4:i * 4 + 4] != b[i * 4:i * 4 + 4]]
        if words:
            mismatched.append({'case': k, 'offsets': [hex(o) for o in words[:12]]})
    result = {
        'cases': len(cases),
        'original_exit': original.returncode,
        'rust_exit': port.returncode,
        'sizes': [len(expected), len(actual)],
        'mismatched_cases': len(mismatched),
        'first': mismatched[:5],
    }
    (out / 'template-validation.json').write_text(json.dumps(result, indent=1))
    print(json.dumps(result, indent=1))
    raise SystemExit(0 if not mismatched and original.returncode == 0 and port.returncode == 0 and len(expected) == len(actual) else 1)


if __name__ == '__main__':
    main()
