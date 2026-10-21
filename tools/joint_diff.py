"""Tally word differences between the original and Rust joint outputs.

    python -I tools/joint_diff.py MODE [CASE]
"""
import collections
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
mode = sys.argv[1]
a = (ROOT / f'local/joint/original-{mode}.bin').read_bytes()
b = (ROOT / f'local/joint/rust-{mode}.bin').read_bytes()
n = struct.unpack_from('<I', a, 4)[0]
record = (len(a) - 8) // n
tally = collections.Counter()
cases = collections.defaultdict(list)
for i in range(n):
    x = a[8 + i * record:8 + (i + 1) * record]
    y = b[8 + i * record:8 + (i + 1) * record]
    for o in range(0, record, 4):
        if x[o:o + 4] != y[o:o + 4]:
            tally[o] += 1
            cases[o].append(i)
for o, c in sorted(tally.items()):
    print(f'{o:#06x} {c:4d} cases e.g. {cases[o][:6]}')
if len(sys.argv) > 2:
    i = int(sys.argv[2])
    x = a[8 + i * record:8 + (i + 1) * record]
    y = b[8 + i * record:8 + (i + 1) * record]
    src = (ROOT / f'local/joint/cases-{mode}.bin').read_bytes()
    crecord = (len(src) - 8) // n
    head = struct.unpack_from('<IIII', src, 8 + i * crecord)
    print('case', i, 'header', head)
    for o in range(0, record, 4):
        if x[o:o + 4] != y[o:o + 4]:
            fx, = struct.unpack_from('<f', x, o)
            fy, = struct.unpack_from('<f', y, o)
            wx, = struct.unpack_from('<I', x, o)
            wy, = struct.unpack_from('<I', y, o)
            print(f'  {o:#06x} original {fx!r} ({wx:08x}) rust {fy!r} ({wy:08x})')
