"""Original joint (NeMouse, AEFEB0..) against the Rust port, JNT1.

Authored descriptors and body poses; the probe runs the original functions on
them in-process (tools/probe_joint.c), the Rust diagnostic reads the same file
(crates/grid-probes grid-joint), and the outputs must match byte for byte.

    python -I tools/validate_joint.py [--cases N] [--profile release]
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
MAGIC = 0x31544E4A
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'
if not IMAGE.exists():
    IMAGE = Path('C:/Dev/Reassembly/local/compatibility/grid-unpacked.exe')
ANALYSIS = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
BODY = 0x424
DESCRIPTOR = 0x168
JOINT = 0x330


def f32(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def rotation(rng):
    """Rows of a random rotation (unit quaternion)."""
    q = [rng.gauss(0, 1) for _ in range(4)]
    n = math.sqrt(sum(x * x for x in q))
    w, x, y, z = (v / n for v in q)
    return [
        [1 - 2 * (y * y + z * z), 2 * (x * y + w * z), 2 * (x * z - w * y)],
        [2 * (x * y - w * z), 1 - 2 * (x * x + z * z), 2 * (y * z + w * x)],
        [2 * (x * z + w * y), 2 * (y * z - w * x), 1 - 2 * (x * x + y * y)],
    ]


def matrix(rng, spread):
    r = rotation(rng)
    p = [rng.uniform(-spread, spread) for _ in range(3)]
    return [*(row + [rng.choice([0., 0., 1., -2.5])] for row in r), p + [rng.choice([1., 1., 0., 3.])]]


def pack_matrix(m):
    return struct.pack('<16f', *(v for row in m for v in row))


def pose(rng, spread=50.):
    m = matrix(rng, spread)
    return [row[:3] + [0.] for row in m[:3]] + [m[3][:3] + [1.]]


def moved(rng, m):
    """The pose after a small turn and shift (or a new one)."""
    if rng.random() < 0.2:
        return pose(rng)
    t = rng.choice([0.0, 0.01, 0.2, 1.0])
    r = rotation(rng)
    turn = [[(1 - t) * (i == j) + t * r[i][j] for j in range(3)] for i in range(3)]
    rows = [[sum(turn[i][k] * m[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
    # Re-orthonormalise (Gram-Schmidt) so the pose stays rigid.
    def unit(v):
        n = math.sqrt(sum(x * x for x in v)) or 1.
        return [x / n for x in v]
    a = unit(rows[0])
    b = [rows[1][i] - sum(rows[1][k] * a[k] for k in range(3)) * a[i] for i in range(3)]
    b = unit(b)
    c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    p = [m[3][i] + rng.uniform(-t, t) for i in range(3)]
    return [a + [0.], b + [0.], c + [0.], p + [1.]]


def body(rng, m):
    b = bytearray(BODY)
    struct.pack_into('<16f', b, 0x190, *(v for row in m for v in row))
    struct.pack_into('<4f', b, 0x230, *(rng.uniform(-0.5, 0.5) for _ in range(3)), 0.)
    struct.pack_into('<f', b, 0x3f0, rng.choice([0., 0.01, 1 / 428, 1 / 17]))
    return bytes(b)


def case(rng, mode, i):
    flags = i % 4  # bit 0: body B present, bit 1: B fixed (+0xC)
    d = bytearray(DESCRIPTOR)
    for k in range(3):
        d[0x10 + k * 0x40:0x50 + k * 0x40] = pack_matrix(matrix(rng, 3.))
    if rng.random() < 0.35:
        # Aligned rest frames (the joint starts at rest): B's frame is A's.
        d[0x90:0xD0] = d[0x50:0x90]
    for o in range(0xD0, 0x164, 4):
        # Anchors (0xD0..0xEF) stay finite; limits and springs may be +inf as in the data.
        top = float('inf') if o >= 0xF0 else 1e4
        struct.pack_into('<f', d, o, rng.choice([0., 1., -1., rng.uniform(-10, 10), 1e4, top]))
    struct.pack_into('<I', d, 0x128, rng.randrange(3))
    struct.pack_into('<I', d, 0x140, rng.randrange(2))
    d[0x164] = rng.randrange(2) if i % 8 >= 4 else 0
    world = struct.unpack('<I', struct.pack('<f', rng.choice([1 / 60, 0.016, 0.02])))[0]
    poses = [pose(rng), pose(rng)]
    after = [moved(rng, m) for m in poses]
    reseat = int(rng.random() < 0.25)
    return (struct.pack('<IIII', mode, flags, world, reseat) + bytes(d) + body(rng, poses[0])
            + body(rng, poses[1]) + b''.join(struct.pack('<16f', *(v for row in m for v in row)) for m in after)
            + struct.pack('<4f4I', rng.choice([60., 1 / 0.0166]), rng.choice([0.2, 0.8, 1.0]), rng.choice([1 / 60, 0.0166]),
                          rng.choice([0.0, 0.01]), rng.randrange(8), 8 + rng.randrange(8), rng.randrange(3), rng.getrandbits(32)))


def run(command, source, target):
    target.unlink(missing_ok=True)
    r = subprocess.run(command + [str(source), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=600)
    return r, target.read_bytes() if target.exists() else b''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cases', type=int, default=512)
    parser.add_argument('--mode', type=int, default=0)
    parser.add_argument('--profile', default='release')
    parser.add_argument('--out', default='local/joint')
    args = parser.parse_args()
    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    if hashlib.sha256(IMAGE.read_bytes()).hexdigest() != ANALYSIS:
        raise ValueError('analysis image identity changed')
    rng = random.Random(20081016 + args.mode)
    cases = [case(rng, args.mode, i) for i in range(args.cases)]
    source = out / f'cases-{args.mode}.bin'
    source.write_bytes(struct.pack('<II', MAGIC, len(cases)) + b''.join(cases))
    probe = ROOT / 'local/behavior/joint-probe.exe'
    rust = ROOT / f'target/{args.profile}/grid-joint.exe'
    original, expected = run([str(probe), str(IMAGE)], source, out / f'original-{args.mode}.bin')
    print(original.stdout.strip(), original.stderr.strip())
    report = dict(cases=len(cases), original=original.returncode)
    if rust.exists():
        port, actual = run([str(rust)], source, out / f'rust-{args.mode}.bin')
        report['rust'] = port.returncode
        if port.returncode:
            print(port.stderr.strip())
        record = (len(expected) - 8) // len(cases) if cases else 0
        mismatches = []
        for i in range(len(cases)):
            a = expected[8 + i * record:8 + (i + 1) * record]
            b = actual[8 + i * record:8 + (i + 1) * record]
            words = [o for o in range(0, record, 4) if a[o:o + 4] != b[o:o + 4]]
            if words:
                mismatches.append((i, [hex(o) for o in words[:12]]))
        report['mismatched_cases'] = len(mismatches)
        report['first'] = mismatches[:6]
        report['passed'] = not mismatches and len(expected) == len(actual) and original.returncode == 0
    print(json.dumps(report, indent=1))


if __name__ == '__main__':
    main()
